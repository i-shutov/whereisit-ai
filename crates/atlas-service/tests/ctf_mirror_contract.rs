//! Generated-only CTF fixtures. No user names, catalog, or CTF bytes enter CI.
use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind};
use serde_json::{json, Value};
use std::fs::{self, File, FileTimes};
use std::path::Path;

fn entry(name: &str, parent: Option<usize>, kind: EntryKind, description: &str) -> CtfEntry {
    CtfEntry {
        name: name.into(),
        description: description.into(),
        parent,
        kind,
        size: if kind == EntryKind::File { 12_345 } else { 0 },
        attributes: if kind == EntryKind::File { 32 } else { 16 },
        modified_dos: 0,
        created_dos: 0,
        accessed_dos: 0,
        opaque: Vec::new(),
    }
}

fn write_original(path: &Path, file_name: &str) -> Vec<u8> {
    let catalog = CtfCatalog {
        name: "Synthetic only".into(),
        opaque_trailer: Vec::new(),
        disks: vec![CtfDisk {
            name: "Synthetic disk".into(),
            opaque_header: Vec::new(),
            entries: vec![
                entry("Audio", None, EntryKind::Directory, ""),
                entry(file_name, Some(0), EntryKind::File, "Performance"),
                entry("guide.pdf", Some(0), EntryKind::File, "Documentation"),
            ],
        }],
    };
    let data = write_ctf(&catalog).unwrap();
    fs::write(path, &data).unwrap();
    data
}

fn service(v: Value) -> Value {
    serde_json::from_str::<Value>(&atlas_service::dispatch_json(&v.to_string())).unwrap()
}

#[test]
fn sync_reuses_immutable_generation_and_follows_original_external_save() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("catalog.CTF");
    let root = dir.path().join("cache");
    let before = write_original(&source, "audio.mp3");
    let initial = service(json!({"op":"ctf_sync","path":source,"cache_root":root}));
    assert_eq!(initial["ok"], true, "{initial}");
    assert_eq!(initial["data"]["entries"], 3);
    assert_eq!(initial["data"]["volumes"], 1);
    assert_eq!(initial["data"]["rebuilt"], true);
    assert_eq!(initial["data"]["native_annotation_writes_allowed"], false);
    let first_key = initial["data"]["generation"].as_str().unwrap().to_string();
    assert_eq!(fs::read(&source).unwrap(), before);

    let again = service(json!({"op":"ctf_sync","path":source,"cache_root":root}));
    assert_eq!(again["ok"], true, "{again}");
    assert_eq!(again["data"]["rebuilt"], false);
    assert_eq!(again["data"]["generation"], first_key);
    let search = service(json!({"op":"ctf_search","path":source,"cache_root":root,
        "query":"ext:mp3","limit":10,"full_count":true}));
    assert_eq!(search["ok"], true, "{search}");
    assert_eq!(search["data"]["result"]["total"], 1);
    assert_eq!(search["data"]["result"]["items"][0]["name"], "audio.mp3");
    assert_eq!(search["data"]["generation"], first_key);

    let mtime = fs::metadata(&source).unwrap().modified().unwrap();
    let modified = write_original(&source, "other.mp3");
    assert_eq!(modified.len(), before.len(), "same-size edit fixture");
    File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_times(FileTimes::new().set_modified(mtime))
        .unwrap();
    let changed = service(json!({"op":"ctf_search","path":source,"cache_root":root,
        "query":"ext:mp3","limit":10,"full_count":true}));
    assert_eq!(changed["ok"], true, "{changed}");
    assert_eq!(changed["data"]["rebuilt"], true);
    assert_ne!(changed["data"]["generation"], first_key);
    assert_eq!(changed["data"]["result"]["items"][0]["name"], "other.mp3");
    assert!(
        root.join(first_key).is_dir(),
        "Old immutable generation retained"
    );
    assert_eq!(fs::read(&source).unwrap(), modified);
}

#[test]
fn renamed_identical_original_reuses_content_bound_mirror() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old-source.ctf");
    let new = dir.path().join("renamed-source.CTF");
    let root = dir.path().join("cache");
    let original = write_original(&old, "track.mp3");
    let build = service(json!({"op":"ctf_sync","path":old,"cache_root":root}));
    assert_eq!(build["ok"], true, "{build}");
    let digest = build["data"]["source_sha256"].clone();
    let key = build["data"]["generation"].clone();
    fs::rename(&old, &new).unwrap();
    let reused = service(json!({"op":"ctf_sync","path":new,"cache_root":root}));
    assert_eq!(reused["ok"], true, "{reused}");
    assert_eq!(reused["data"]["rebuilt"], false);
    assert_eq!(reused["data"]["generation"], key);
    assert_eq!(reused["data"]["source_sha256"], digest);
    let rows = service(json!({"op":"ctf_search","path":new,"cache_root":root,
        "query":"ext:mp3","full_count":true}));
    assert_eq!(rows["ok"], true, "{rows}");
    assert_eq!(rows["data"]["result"]["total"], 1);
    assert_eq!(fs::read(&new).unwrap(), original);
}

#[test]
fn invalid_query_never_creates_an_expensive_mirror() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("input.ctf");
    let root = dir.path().join("cache");
    let original = write_original(&source, "track.mp3");
    for query in ["(", "name:", "ext:mp3 AND"] {
        let result = service(json!({"op":"ctf_search","path":source,
            "cache_root":root,"query":query}));
        assert_eq!(result["ok"], false, "{query}: {result}");
        assert!(
            !root.exists(),
            "Bad query started an expensive mirror build"
        );
    }
    let missing = service(json!({"op":"ctf_search","path":source,"cache_root":root}));
    assert_eq!(missing["ok"], false, "{missing}");
    assert!(!root.exists());
    assert_eq!(fs::read(&source).unwrap(), original);
}

#[test]
fn corrupt_cache_is_rejected_without_automatic_deletion_or_source_write() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("catalog.ctf");
    let root = dir.path().join("cache");
    let original = write_original(&source, "file.mp3");
    let sync = service(json!({"op":"ctf_sync","path":source,"cache_root":root}));
    assert_eq!(sync["ok"], true, "{sync}");
    let key = sync["data"]["generation"].as_str().unwrap();
    let db = root.join(key).join("catalog.sqlite");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&db, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[cfg(windows)]
    {
        let mut perms = fs::metadata(&db).unwrap().permissions();
        perms.set_readonly(false);
        fs::set_permissions(&db, perms).unwrap();
    }
    fs::write(&db, b"changed SQLite mirror").unwrap();
    let result = service(json!({"op":"ctf_sync","path":source,"cache_root":root}));
    assert_eq!(result["ok"], false, "{result}");
    assert_eq!(result["error"]["code"], "cache_corrupt");
    assert_eq!(fs::read(&db).unwrap(), b"changed SQLite mirror");
    assert_eq!(fs::read(&source).unwrap(), original);
}

#[test]
fn malformed_ctf_does_not_publish_a_partial_or_mutate_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("broken.ctf");
    let root = dir.path().join("cache");
    fs::write(&source, b"invalid CTF").unwrap();
    let result = service(json!({"op":"ctf_sync","path":source,"cache_root":root}));
    assert_eq!(result["ok"], false, "{result}");
    assert!(root.exists());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert_eq!(fs::read(&source).unwrap(), b"invalid CTF");
}

#[test]
fn read_only_mirror_rejects_annotation_write_but_supports_query() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.ctf");
    let root = dir.path().join("cache");
    let original = write_original(&source, "song.mp3");
    let result = service(json!({"op":"ctf_sync","path":source,"cache_root":root}));
    assert_eq!(result["ok"], true, "{result}");
    let path = root
        .join(result["data"]["generation"].as_str().unwrap())
        .join("catalog.sqlite");
    let catalog = atlas_core::Catalog::open_readonly(path).unwrap();
    let page = catalog.search("ext:mp3", None, 0, 10).unwrap();
    assert_eq!(page.total, 1);
    assert!(catalog
        .update_entry(page.items[0].id, "must not persist", "")
        .is_err());
    assert_eq!(
        catalog
            .search("description:must", None, 0, 10)
            .unwrap()
            .total,
        0
    );
    assert_eq!(fs::read(&source).unwrap(), original);
}
