//! Synthetic linked CTF export contracts: source and index remain immutable.
use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind};
use parquet::file::reader::{FileReader, SerializedFileReader};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn request(input: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&input.to_string())).unwrap()
}

fn fixture(path: &Path, music: &str) -> Vec<u8> {
    let entry = |name: &str, parent: Option<usize>, kind: EntryKind| CtfEntry {
        name: name.to_owned(),
        description: String::new(),
        parent,
        kind,
        size: if kind == EntryKind::File { 1024 } else { 0 },
        attributes: if kind == EntryKind::File { 32 } else { 16 },
        modified_dos: 0,
        created_dos: 0,
        accessed_dos: 0,
        opaque: Vec::new(),
    };
    let bytes = write_ctf(&CtfCatalog {
        name: "synthetic only".into(),
        opaque_trailer: Vec::new(),
        disks: vec![CtfDisk {
            name: "disconnected media".into(),
            opaque_header: Vec::new(),
            entries: vec![
                entry("Аудио", None, EntryKind::Directory),
                entry(music, Some(0), EntryKind::File),
                entry("guide.pdf", Some(0), EntryKind::File),
                entry("film.mkv", None, EntryKind::File),
            ],
        }],
    })
    .unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}

fn synced(source: &Path, cache: &Path) -> String {
    let result = request(json!({"op":"ctf_sync","path":source,"cache_root":cache}));
    assert_eq!(result["ok"], true, "{result}");
    result["data"]["generation"].as_str().unwrap().to_owned()
}

fn export(source: &Path, cache: &Path, generation: &str, output: &Path, format: &str) -> Value {
    request(json!({
        "op":"ctf_export_pinned","path":source,"cache_root":cache,
        "generation":generation,"output":output,"format":format,
        "mode":"search","query":"ext:mp3"
    }))
}

fn assert_no_stage(dir: &Path) {
    for file in fs::read_dir(dir).unwrap() {
        let name = file.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.starts_with(".whereisit-ctf-export-"), "{name}");
    }
}

#[test]
fn exports_csv_parquet_and_json_from_one_verified_pinned_generation() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.CTF");
    let cache = dir.path().join("cache");
    let original = fixture(&source, "песня.mp3");
    let generation = synced(&source, &cache);

    let csv_path = dir.path().join("results.csv");
    let csv = export(&source, &cache, &generation, &csv_path, "csv");
    assert_eq!(csv["ok"], true, "{csv}");
    assert_eq!(csv["data"]["result"]["rows"], 1);
    assert_eq!(csv["data"]["result"]["original_ctf_read_only"], true);
    assert_eq!(csv["data"]["result"]["output_no_clobber"], true);
    assert_eq!(csv["data"]["source_sha_checked"], true);
    let mut reader = csv::Reader::from_path(&csv_path).unwrap();
    let rows: Vec<_> = reader
        .records()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get(3), Some("песня.mp3"));

    let parquet_path = dir.path().join("all.parquet");
    let pq = request(json!({"op":"ctf_export_pinned","path":source,
        "cache_root":cache,"generation":generation,"output":parquet_path,
        "format":"parquet","mode":"all"}));
    assert_eq!(pq["ok"], true, "{pq}");
    assert_eq!(pq["data"]["result"]["rows"], 4);
    let parquet = SerializedFileReader::new(fs::File::open(&parquet_path).unwrap()).unwrap();
    assert_eq!(parquet.metadata().file_metadata().num_rows(), 4);

    let json_path = dir.path().join("selected.json");
    let js = export(&source, &cache, &generation, &json_path, "json");
    assert_eq!(js["ok"], true, "{js}");
    let json_data: Value = serde_json::from_slice(&fs::read(&json_path).unwrap()).unwrap();
    assert_eq!(json_data["entries"].as_array().unwrap().len(), 1);
    assert_eq!(json_data["entries"][0]["name"], "песня.mp3");
    assert_eq!(fs::read(&source).unwrap(), original);
    assert_no_stage(dir.path());
}

#[test]
fn exports_selected_branch_recursively_with_no_mirror_modification() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.ctf");
    let cache = dir.path().join("cache");
    let source_bytes = fixture(&source, "music.mp3");
    let generation = synced(&source, &cache);
    let manifest = fs::read(cache.join(&generation).join("manifest.json")).unwrap();
    let disk = request(json!({"op":"ctf_volumes_pinned","path":source,
        "cache_root":cache,"generation":generation}));
    let disk_id = disk["data"]["result"][0]["id"].as_i64().unwrap();
    let root = request(json!({"op":"ctf_children_pinned","path":source,
        "cache_root":cache,"generation":generation,"volume_id":disk_id}));
    let folder_id = root["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "Аудио")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let output = dir.path().join("branch.csv");
    let result = request(json!({"op":"ctf_export_pinned","path":source,
        "cache_root":cache,"generation":generation,"output":output,
        "format":"csv","mode":"children","volume_id":disk_id,"parent_id":folder_id}));
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["data"]["result"]["rows"], 3);
    let data = fs::read_to_string(&output).unwrap();
    assert!(data.contains("music.mp3"));
    assert!(data.contains("guide.pdf"));
    assert!(!data.contains("film.mkv"));
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(
        fs::read(cache.join(&generation).join("manifest.json")).unwrap(),
        manifest
    );
    assert_no_stage(dir.path());
}

#[test]
fn invalid_or_existing_targets_fail_closed_and_do_not_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.CTF");
    let cache = dir.path().join("cache");
    let source_bytes = fixture(&source, "music.mp3");
    let generation = synced(&source, &cache);
    let output = dir.path().join("existing.csv");
    fs::write(&output, "USER CONTENT").unwrap();
    let occupied = export(&source, &cache, &generation, &output, "csv");
    assert_eq!(occupied["error"]["code"], "file_exists", "{occupied}");
    let overwrite = request(json!({"op":"ctf_export_pinned","path":source,
        "cache_root":cache,"generation":generation,"output":output,
        "format":"csv","mode":"all","overwrite":true}));
    assert_eq!(overwrite["error"]["code"], "invalid_request", "{overwrite}");
    assert_eq!(fs::read_to_string(&output).unwrap(), "USER CONTENT");

    let bad_suffix = export(&source, &cache, &generation, &source, "csv");
    assert_eq!(bad_suffix["ok"], false, "{bad_suffix}");
    let inside_cache = export(
        &source,
        &cache,
        &generation,
        &cache.join("untrusted.csv"),
        "csv",
    );
    assert_eq!(
        inside_cache["error"]["code"], "unsafe_output",
        "{inside_cache}"
    );
    let duplicates = request(json!({"op":"ctf_export_pinned","path":source,
        "cache_root":cache,"generation":generation,"output":dir.path().join("dup.csv"),
        "format":"csv","mode":"duplicates"}));
    assert_eq!(
        duplicates["error"]["code"], "invalid_request",
        "{duplicates}"
    );
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_no_stage(dir.path());
}

#[test]
fn externally_changed_ctf_rejects_stale_export_without_partial_output() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.ctf");
    let cache = dir.path().join("cache");
    let before = fixture(&source, "music.mp3");
    let generation = synced(&source, &cache);
    let after = fixture(&source, "other.mp3");
    assert_eq!(before.len(), after.len());
    let output = dir.path().join("stale.csv");
    let rejected = export(&source, &cache, &generation, &output, "csv");
    assert_eq!(rejected["ok"], false, "{rejected}");
    assert_eq!(rejected["error"]["code"], "source_changed", "{rejected}");
    assert!(!output.exists());
    assert_eq!(fs::read(&source).unwrap(), after);
    assert_no_stage(dir.path());
}
