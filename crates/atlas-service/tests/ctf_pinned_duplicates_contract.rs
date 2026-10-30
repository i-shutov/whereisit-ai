//! Metadata duplicate candidates from synthetic, source-verified original CTF.
//! Never send user CTF, cached SQLite, or media paths to CI.
use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind};
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

    let both = duplicate(&source, &cache, &generation,
        json!({"duplicate_mode":"name_size","limit":20}));
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

    let by_name = duplicate(&source, &cache, &generation,
        json!({"duplicate_mode":"name","query":"ext:mkv"}));
    assert_eq!(by_name["ok"], true, "{by_name}");
    let names = by_name["data"]["result"]["items"].as_array().unwrap();
    assert_eq!(names.len(), 3);
    assert!(names.iter().all(|r| r["duplicate_count"] == 3));
    assert!(names.iter().any(|r| r["size"] == 900));

    let by_size = duplicate(&source, &cache, &generation,
        json!({"duplicate_mode":"size"}));
    assert_eq!(by_size["ok"], true, "{by_size}");
    let sizes = by_size["data"]["result"]["items"].as_array().unwrap();
    assert_eq!(sizes.len(), 5);
    assert_eq!(sizes.iter().filter(|r| r["duplicate_count"] == 3).count(), 3);
    assert_eq!(sizes.iter().filter(|r| r["duplicate_count"] == 2).count(), 2);
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(fs::read(&sql_path).unwrap(), db_before);
    assert_eq!(fs::read(cache.join(&generation).join("manifest.json")).unwrap(), manifest_before);
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
    let first_volume = disks.iter().find(|v| v["label"] == "disc one").unwrap()["id"].as_i64().unwrap();
    let second_volume = disks.iter().find(|v| v["label"] == "disc two").unwrap()["id"].as_i64().unwrap();

    let one = duplicate(&source, &cache, &generation, json!({
        "duplicate_mode":"name_size","volume_id":first_volume,"limit":1,"offset":0
    }));
    assert_eq!(one["ok"], true, "{one}");
    assert_eq!(one["data"]["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(one["data"]["result"]["has_more"], true);
    assert!(one["data"]["result"]["total"].is_null());
    let one_id = one["data"]["result"]["items"][0]["id"].as_i64().unwrap();
    let two = duplicate(&source, &cache, &generation, json!({
        "duplicate_mode":"name_size","volume_id":first_volume,"limit":1,"offset":1
    }));
    assert_eq!(two["ok"], true, "{two}");
    assert_eq!(two["data"]["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(two["data"]["result"]["has_more"], false);
    assert_ne!(one_id, two["data"]["result"]["items"][0]["id"].as_i64().unwrap());
    assert_eq!(two["data"]["result"]["items"][0]["duplicate_group"],
               one["data"]["result"]["items"][0]["duplicate_group"]);
    let only_second = duplicate(&source, &cache, &generation, json!({
        "duplicate_mode":"name","volume_id":second_volume,"query":"ext:mkv"
    }));
    assert_eq!(only_second["ok"], true, "{only_second}");
    assert!(only_second["data"]["result"]["items"].as_array().unwrap().is_empty());
    let filtered = duplicate(&source, &cache, &generation, json!({
        "duplicate_mode":"size","query":"ext:mp3"
    }));
    assert_eq!(filtered["ok"], true, "{filtered}");
    assert_eq!(filtered["data"]["result"]["items"].as_array().unwrap().len(), 2);
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
    let hash = duplicate(&source, &cache, &generation, json!({"duplicate_mode":"hash"}));
    assert_eq!(hash["error"]["code"], "unsupported", "{hash}");
    let invalid = duplicate(&source, &cache, &generation,
        json!({"duplicate_mode":"name","query":"name:("}));
    assert_eq!(invalid["ok"], false, "{invalid}");
    let bad_limit = duplicate(&source, &cache, &generation,
        json!({"duplicate_mode":"name","offset":-1}));
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
    let stale = duplicate(&source, &cache, &generation, json!({"duplicate_mode":"name"}));
    assert_eq!(stale["ok"], false, "{stale}");
    assert_eq!(stale["error"]["code"], "source_changed", "{stale}");
    assert_eq!(fs::read(&source).unwrap(), after);
}
