use atlas_core::{Catalog, NewEntry};
use serde_json::{json, Value};
use std::path::Path;

fn call(v: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&v.to_string())).unwrap()
}

fn old_catalog(path: &Path, version: i64) -> Catalog {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(include_str!("../../atlas-core/migrations/001.sql")).unwrap();
    conn.pragma_update(None, "user_version", version).unwrap();
    drop(conn);
    Catalog::open(path).unwrap()
}

fn seed(c: &mut Catalog) {
    let mut ids = Vec::new();
    c.bulk_import("Synthetic offline disk", "", |w| {
        for i in 0..8 {
            ids.push(w.insert(&NewEntry {
                parent_id: None,
                name: if i < 2 { "Movie.mkv".into() } else { format!("Track-{i}.flac") },
                relative_path: if i < 2 { "Movie.mkv".into() } else { format!("Track-{i}.flac") },
                kind: "file".into(),
                size: 5_000_000_001,
                modified_unix: None,
                description: if i == 4 { "Редкая аннотация".into() } else { "".into() },
                tags: if i == 6 { "favorite".into() } else { "".into() },
            })?);
        }
        Ok(())
    }).unwrap();
    let category = c.create_category("Музыка", None).unwrap();
    c.assign_category(ids[4], category).unwrap();
}

#[test]
fn compact_copy_preserves_source_and_creates_a_runnable_catalog() {
    for version in [1, 2, 3] {
        let dir = tempfile::tempdir().unwrap();
        // '#' exercises correct percent escaping; '?' is invalid on Windows.
        let source = dir.path().join("source #.sqlite");
        let target = dir.path().join("compact.sqlite");
        let mut c = if version == 3 { Catalog::open(&source).unwrap() } else { old_catalog(&source, version) };
        seed(&mut c);
        let before_queries: Vec<_> = ["ext:mkv", "description:редкая", "NOT tag:favorite", "category:музыка", "аннотация OR name:Movie"]
            .iter().map(|q| c.search(q, None, 0, 100).unwrap().items.into_iter().map(|e| e.id).collect::<Vec<_>>()).collect();
        drop(c);
        let before_bytes = std::fs::read(&source).unwrap();
        let report = call(json!({"op":"compact_catalog","db":source,"path":target}));
        assert_eq!(report["ok"], true, "{report}");
        assert_eq!(report["data"]["entries"], 8);
        assert_eq!(report["data"]["payload_rows"], 2);
        assert_eq!(report["data"]["schema_version"], 3);
        assert_eq!(before_bytes, std::fs::read(&source).unwrap(), "Source database bytes changed");
        let c = Catalog::open(&target).unwrap();
        for (i, q) in ["ext:mkv", "description:редкая", "NOT tag:favorite", "category:музыка", "аннотация OR name:Movie"].iter().enumerate() {
            let ids: Vec<_> = c.search(q, None, 0, 100).unwrap().items.into_iter().map(|e| e.id).collect();
            assert_eq!(ids, before_queries[i], "{q}");
        }
        drop(c);
        let duplicates = call(json!({"op":"duplicates","db":target,"mode":"name_size"}));
        assert_eq!(duplicates["ok"], true, "{duplicates}");
        assert_eq!(duplicates["data"]["total"], 2);
        let validation = call(json!({"op":"validate_catalog","db":target}));
        assert_eq!(validation["ok"], true, "{validation}");
        assert_eq!(validation["data"]["readonly"], true);
    }
}

#[test]
fn compaction_never_overwrites_existing_files_or_source_sidecars() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.sqlite");
    drop(old_catalog(&source, 1));
    let original = std::fs::read(&source).unwrap();
    let existing = dir.path().join("existing.sqlite");
    std::fs::write(&existing, b"keep me").unwrap();
    let hardlink = dir.path().join("hardlink.sqlite");
    std::fs::hard_link(&source, &hardlink).unwrap();
    for target in [source.clone(), existing.clone(), hardlink, dir.path().join("source.sqlite-wal"), dir.path().join("source.sqlite-shm"), dir.path().join("source.sqlite-journal")] {
        let r = call(json!({"op":"compact_catalog","db":source,"path":target}));
        assert_eq!(r["ok"], false, "Unexpected unsafe output: {r}");
    }
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep me");
    assert_eq!(std::fs::read(&source).unwrap(), original);
}

#[test]
fn unknown_tables_fail_closed_without_publishing_output() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.sqlite");
    let target = dir.path().join("target.sqlite");
    let c = old_catalog(&source, 1);
    c.conn().execute_batch("CREATE TABLE custom_user_data(value TEXT); INSERT INTO custom_user_data VALUES('retain');").unwrap();
    drop(c);
    let r = call(json!({"op":"compact_catalog","db":source,"path":target}));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["error"]["code"], "unsupported_schema");
    assert!(!target.exists());
}

#[test]
fn legacy_ctf_bytes_survive_compaction_from_v1_and_v2() {
    use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind};
    for version in [1, 2] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.sqlite");
        let target = dir.path().join("compact.sqlite");
        let input = dir.path().join("input.ctf");
        let before = dir.path().join("before.ctf");
        let after = dir.path().join("after.ctf");
        drop(old_catalog(&source, version));
        let fixture = CtfCatalog {
            name: "Synthetic CTF fixture".into(),
            disks: vec![CtfDisk { name: "Disk".into(), opaque_header: vec![], entries: vec![CtfEntry {
                name: "Музыка 🎵.flac".into(), description: "Rare description".into(), parent: None,
                kind: EntryKind::File, size: 6_000_000_000, attributes: 32,
                modified_dos: 0, created_dos: 0, accessed_dos: 0, opaque: vec![],
            }] }],
            opaque_trailer: vec![],
        };
        std::fs::write(&input, ctf_format::write_ctf(&fixture).unwrap()).unwrap();
        for v in [json!({"op":"import_ctf","db":source,"path":input}), json!({"op":"export_ctf","db":source,"path":before}), json!({"op":"compact_catalog","db":source,"path":target}), json!({"op":"export_ctf","db":target,"path":after})] {
            let r = call(v);
            assert_eq!(r["ok"], true, "{r}");
        }
        assert_eq!(std::fs::read(&before).unwrap(), std::fs::read(&after).unwrap());
        assert_eq!(std::fs::read(&input).unwrap(), std::fs::read(&after).unwrap());
    }
}
