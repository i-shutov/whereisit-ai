//! Metadata duplicate candidates from synthetic, source-verified original CTF.
//! Never send user CTF, cached SQLite, or media paths to CI.
use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind};
use parquet::file::reader::{FileReader, SerializedFileReader};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn entry(name: &str, size: u64, parent: Option<usize>, kind: EntryKind) -> CtfEntry {
    CtfEntry {
        name: name.to_owned(),
        description: String::new(),
        parent,
        kind,
        size: if kind == EntryKind::File { size } else { 0 },
        attributes: if kind == EntryKind::File { 32 } else { 16 },
        modified_dos: 0,
        created_dos: 0,
        accessed_dos: 0,
        opaque: Vec::new(),
    }
}

fn write_source(path: &Path, first: &str) -> Vec<u8> {
    let bytes = write_ctf(&CtfCatalog {
        name: "synthetic duplicate fixture".into(),
        opaque_trailer: Vec::new(),
        disks: vec![
            CtfDisk {
                name: "disc one".into(),
                opaque_header: Vec::new(),
                entries: vec![
                    entry("Films", 0, None, EntryKind::Directory),
                    entry(first, 512, Some(0), EntryKind::File),
                    entry("film.mkv", 512, Some(0), EntryKind::File),
                    entry("intro.mp3", 900, Some(0), EntryKind::File),
                ],
            },
            CtfDisk {
                name: "disc two".into(),
                opaque_header: Vec::new(),
                entries: vec![
                    entry("film.mkv", 900, None, EntryKind::File),
                    entry("live.mp3", 900, None, EntryKind::File),
                    entry("book.pdf", 33, None, EntryKind::File),
                ],
            },
        ],
    })
    .unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}

fn call(args: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&args.to_string())).unwrap()
}

fn sync(source: &Path, root: &Path) -> String {
    let reply = call(json!({"op":"ctf_sync","path":source,"cache_root":root}));
    assert_eq!(reply["ok"], true, "{reply}");
    reply["data"]["generation"].as_str().unwrap().to_owned()
}

fn duplicate(source: &Path, root: &Path, generation: &str, args: Value) -> Value {
    let mut req = json!({
        "op":"ctf_duplicates_pinned","path":source,"cache_root":root,
        "generation":generation
    });
    for (key, value) in args.as_object().unwrap() {
        req[key] = value.clone();
    }
    call(req)
}

#[test]
fn name_and_size_modes_return_only_matching_file_candidates_across_disks() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("albums.ctf");
    let cache = dir.path().join("mirrors");
    let before = write_source(&source, "Film.MKV");
    let generation = sync(&source, &cache);
    let sql_path = cache.join(&generation).join("catalog.sqlite");
    let db_before = fs::read(&sql_path).unwrap();
    let manifest_before = fs::read(cache.join(&generation).join("manifest.json")).unwrap();

    let both = duplicate(
        &source,
        &cache,
        &generation,
        json!({"duplicate_mode":"name_size","limit":20}),
    );
    assert_eq!(both["ok"], true, "{both}");
    assert_eq!(both["data"]["read_only"], true);
    assert_eq!(both["data"]["source_sha_checked"], true);
    assert_eq!(both["data"]["result"]["verified"], false);
    assert_eq!(both["data"]["result"]["basis"], "metadata_candidates");
    assert_eq!(both["data"]["result"]["has_more"], false);
    let rows = both["data"]["result"]["items"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{both}");
    assert!(rows.iter().all(|r| r["duplicate_count"] == 2));
    assert!(rows.iter().all(|r| r["kind"] == "file"));
    assert!(rows.iter().all(|r| r["size"] == 512));

    let by_name = duplicate(
        &source,
        &cache,
        &generation,
        json!({"duplicate_mode":"name","query":"ext:mkv"}),
    );
    assert_eq!(by_name["ok"], true, "{by_name}");
    let names = by_name["data"]["result"]["items"].as_array().unwrap();
    assert_eq!(names.len(), 3);
    assert!(names.iter().all(|r| r["duplicate_count"] == 3));
    assert!(names.iter().any(|r| r["size"] == 900));

    let by_size = duplicate(
        &source,
        &cache,
        &generation,
        json!({"duplicate_mode":"size"}),
    );
    assert_eq!(by_size["ok"], true, "{by_size}");
    let sizes = by_size["data"]["result"]["items"].as_array().unwrap();
    assert_eq!(sizes.len(), 5);
    assert_eq!(
        sizes.iter().filter(|r| r["duplicate_count"] == 3).count(),
        3
    );
    assert_eq!(
        sizes.iter().filter(|r| r["duplicate_count"] == 2).count(),
        2
    );
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(fs::read(&sql_path).unwrap(), db_before);
    assert_eq!(
        fs::read(cache.join(&generation).join("manifest.json")).unwrap(),
        manifest_before
    );
}

#[test]
fn query_filter_volume_scope_and_pagination_stay_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("volume.CTF");
    let cache = dir.path().join("cache");
    write_source(&source, "Film.MKV");
    let generation = sync(&source, &cache);
    let volumes = call(json!({"op":"ctf_volumes_pinned","path":source,
        "cache_root":cache,"generation":generation}));
    assert_eq!(volumes["ok"], true, "{volumes}");
    let disks = volumes["data"]["result"].as_array().unwrap();
    assert_eq!(disks.len(), 2);
    let first_volume = disks.iter().find(|v| v["label"] == "disc one").unwrap()["id"]
        .as_i64()
        .unwrap();
    let second_volume = disks.iter().find(|v| v["label"] == "disc two").unwrap()["id"]
        .as_i64()
        .unwrap();

    let one = duplicate(
        &source,
        &cache,
        &generation,
        json!({
            "duplicate_mode":"name_size","volume_id":first_volume,"limit":1,"offset":0
        }),
    );
    assert_eq!(one["ok"], true, "{one}");
    assert_eq!(one["data"]["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(one["data"]["result"]["has_more"], true);
    assert!(one["data"]["result"]["total"].is_null());
    let one_id = one["data"]["result"]["items"][0]["id"].as_i64().unwrap();
    let two = duplicate(
        &source,
        &cache,
        &generation,
        json!({
            "duplicate_mode":"name_size","volume_id":first_volume,"limit":1,"offset":1
        }),
    );
    assert_eq!(two["ok"], true, "{two}");
    assert_eq!(two["data"]["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(two["data"]["result"]["has_more"], false);
    assert_ne!(
        one_id,
        two["data"]["result"]["items"][0]["id"].as_i64().unwrap()
    );
    assert_eq!(
        two["data"]["result"]["items"][0]["duplicate_group"],
        one["data"]["result"]["items"][0]["duplicate_group"]
    );
    let only_second = duplicate(
        &source,
        &cache,
        &generation,
        json!({
            "duplicate_mode":"name","volume_id":second_volume,"query":"ext:mkv"
        }),
    );
    assert_eq!(only_second["ok"], true, "{only_second}");
    assert!(only_second["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .is_empty());
    let filtered = duplicate(
        &source,
        &cache,
        &generation,
        json!({
            "duplicate_mode":"size","query":"ext:mp3"
        }),
    );
    assert_eq!(filtered["ok"], true, "{filtered}");
    assert_eq!(
        filtered["data"]["result"]["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn unsupported_hash_and_bad_query_fail_before_mutating_a_cache() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("music.ctf");
    let cache = dir.path().join("cache");
    write_source(&source, "Film.MKV");
    let generation = sync(&source, &cache);
    let database = cache.join(&generation).join("catalog.sqlite");
    let before = fs::read(&database).unwrap();
    let hash = duplicate(
        &source,
        &cache,
        &generation,
        json!({"duplicate_mode":"hash"}),
    );
    assert_eq!(hash["error"]["code"], "unsupported", "{hash}");
    let invalid = duplicate(
        &source,
        &cache,
        &generation,
        json!({"duplicate_mode":"name","query":"name:("}),
    );
    assert_eq!(invalid["ok"], false, "{invalid}");
    let bad_limit = duplicate(
        &source,
        &cache,
        &generation,
        json!({"duplicate_mode":"name","offset":-1}),
    );
    assert_eq!(bad_limit["error"]["code"], "invalid_request", "{bad_limit}");
    assert_eq!(fs::read(&database).unwrap(), before);
}

#[test]
fn original_ctf_edit_invalidates_stale_duplicate_generation() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("catalog.ctf");
    let cache = dir.path().join("cache");
    let before = write_source(&source, "Film.MKV");
    let generation = sync(&source, &cache);
    let after = write_source(&source, "File.MKV");
    assert_eq!(before.len(), after.len());
    let stale = duplicate(
        &source,
        &cache,
        &generation,
        json!({"duplicate_mode":"name"}),
    );
    assert_eq!(stale["ok"], false, "{stale}");
    assert_eq!(stale["error"]["code"], "source_changed", "{stale}");
    assert_eq!(fs::read(&source).unwrap(), after);
}

#[test]
fn exported_duplicate_candidates_match_full_pinned_pages_and_remain_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("synthetic.ctf");
    let cache = dir.path().join("cache");
    let original = write_source(&source, "Film.MKV");
    let generation = sync(&source, &cache);
    let database = cache.join(&generation).join("catalog.sqlite");
    let db_before = fs::read(&database).unwrap();
    let manifest = fs::read(cache.join(&generation).join("manifest.json")).unwrap();

    // The export includes the full candidate set (never the 1-row GUI page).
    let listed = duplicate(
        &source,
        &cache,
        &generation,
        json!({
            "duplicate_mode":"name_size","limit":1
        }),
    );
    assert_eq!(listed["data"]["result"]["has_more"], true, "{listed}");
    let csv_path = dir.path().join("candidates.csv");
    let csv_result = call(json!({
        "op":"ctf_export_pinned","path":source,"cache_root":cache,
        "generation":generation,"output":csv_path,"format":"csv",
        "mode":"duplicates","duplicate_mode":"name_size"
    }));
    assert_eq!(csv_result["ok"], true, "{csv_result}");
    assert_eq!(csv_result["data"]["result"]["rows"], 2);
    assert_eq!(csv_result["data"]["result"]["duplicate_candidates"], true);
    assert_eq!(csv_result["data"]["result"]["verified"], false);
    assert_eq!(csv_result["data"]["result"]["basis"], "metadata_candidates");
    assert_eq!(csv_result["data"]["result"]["duplicate_mode"], "name_size");
    assert_eq!(csv_result["data"]["source_sha_checked"], true);
    let mut reader = csv::Reader::from_path(&csv_path).unwrap();
    let rows: Vec<_> = reader.records().collect::<Result<_, _>>().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .all(|r| r.get(5) == Some("file") && r.get(12) == Some("2")));
    assert!(rows.iter().all(|r| r.get(13) == Some("film.mkv:512")));

    let json_path = dir.path().join("names.json");
    let json_result = call(json!({
        "op":"ctf_export_pinned","path":source,"cache_root":cache,
        "generation":generation,"output":json_path,"format":"json",
        "mode":"duplicates","duplicate_mode":"name","query":"ext:mkv"
    }));
    assert_eq!(json_result["ok"], true, "{json_result}");
    let data: Value = serde_json::from_slice(&fs::read(&json_path).unwrap()).unwrap();
    let items = data["entries"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|r| r["duplicate_count"] == 3));
    assert!(items.iter().all(|r| r["duplicate_group"] == "film.mkv"));

    let pq_path = dir.path().join("sizes.parquet");
    let pq = call(json!({
        "op":"ctf_export_pinned","path":source,"cache_root":cache,
        "generation":generation,"output":pq_path,"format":"parquet",
        "mode":"duplicates","duplicate_mode":"size","query":"ext:mp3"
    }));
    assert_eq!(pq["ok"], true, "{pq}");
    assert_eq!(pq["data"]["result"]["rows"], 2);
    let parquet = SerializedFileReader::new(fs::File::open(&pq_path).unwrap()).unwrap();
    assert_eq!(parquet.metadata().file_metadata().num_rows(), 2);

    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(fs::read(&database).unwrap(), db_before);
    assert_eq!(
        fs::read(cache.join(&generation).join("manifest.json")).unwrap(),
        manifest
    );
}

#[test]
fn duplicate_export_rejects_wrong_scope_overwrites_stale_sources_and_hash_claims() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.ctf");
    let cache = dir.path().join("cache");
    let before = write_source(&source, "Film.MKV");
    let generation = sync(&source, &cache);
    let output = dir.path().join("test.csv");
    let base = json!({
        "op":"ctf_export_pinned","path":source,"cache_root":cache,
        "generation":generation,"output":output,"format":"csv",
        "mode":"duplicates"
    });
    for (field, value, expected) in [
        ("duplicate_mode", json!("hash"), "unsupported"),
        ("query", json!("name:("), "invalid_query"),
        ("parent_id", json!(1), "invalid_request"),
        ("limit", json!(1), "invalid_request"),
        ("offset", json!(0), "invalid_request"),
    ] {
        let mut req = base.clone();
        req[field] = value;
        let result = call(req);
        assert_eq!(result["ok"], false, "{field}: {result}");
        if expected != "invalid_query" {
            assert_eq!(result["error"]["code"], expected, "{field}: {result}");
        }
        assert!(!output.exists(), "{field}: file was created");
    }
    fs::write(&output, b"USER OWNED").unwrap();
    let refuse = call(base.clone());
    assert_eq!(refuse["error"]["code"], "file_exists", "{refuse}");
    assert_eq!(fs::read(&output).unwrap(), b"USER OWNED");

    // Same-size replacement by the external source application invalidates
    // the pinned generation; neither output nor stage is published.
    let changed = write_source(&source, "File.MKV");
    assert_eq!(before.len(), changed.len());
    let stale_path = dir.path().join("stale.csv");
    let mut stale_req = base.clone();
    stale_req["output"] = json!(stale_path);
    let stale = call(stale_req);
    assert_eq!(stale["error"]["code"], "source_changed", "{stale}");
    assert!(!stale_path.exists());
    assert_eq!(fs::read(&source).unwrap(), changed);
    for item in fs::read_dir(dir.path()).unwrap() {
        let name = item.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.starts_with(".whereisit-ctf-export-"), "{name}");
    }
}
