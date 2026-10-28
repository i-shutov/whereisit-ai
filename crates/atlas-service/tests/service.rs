use atlas_core::{Catalog, NewEntry};
use atlas_service::dispatch_json;
use serde_json::{json, Value};
use std::path::Path;

fn fixture(path: &Path) -> (i64, i64) {
    let mut c = Catalog::open(path).unwrap();
    let mut folder = 0;
    let v = c
        .bulk_import("Media A", "X:/", |w| {
            folder = w.insert(&NewEntry {
                parent_id: None,
                name: "Кино".into(),
                relative_path: "Кино".into(),
                kind: "directory".into(),
                size: 0,
                modified_unix: None,
                description: String::new(),
                tags: String::new(),
            })?;
            w.insert(&file(
                Some(folder),
                "Фильм, \"один\"\n.mkv",
                "Кино/Фильм, \"один\"\n.mkv",
                123,
            ))?;
            w.insert(&file(Some(folder), "track.flac", "Кино/track.flac", 456))?;
            w.insert(&file(None, "outside.txt", "outside.txt", 1))?;
            Ok(())
        })
        .unwrap();
    c.bulk_import("Media B", "Y:/", |w| {
        w.insert(&file(None, "TRACK.FLAC", "TRACK.FLAC", 456))?;
        Ok(())
    })
    .unwrap();
    (v.id, folder)
}
fn file(parent_id: Option<i64>, name: &str, relative_path: &str, size: u64) -> NewEntry {
    NewEntry {
        parent_id,
        name: name.into(),
        relative_path: relative_path.into(),
        kind: "file".into(),
        size,
        modified_unix: Some(1700000000),
        description: String::new(),
        tags: String::new(),
    }
}
fn call(v: Value) -> Value {
    serde_json::from_str(&dispatch_json(&v.to_string())).unwrap()
}

#[test]
fn offline_duplicate_candidates_and_export_scope() {
    let t = tempfile::tempdir().unwrap();
    let db = t.path().join("test.sqlite");
    let (v, folder) = fixture(&db);
    let d = call(json!({"op":"duplicates","db":db,"mode":"name_size"}));
    assert_eq!(d["ok"], true, "{d}");
    assert_eq!(d["data"]["total"], 2);
    assert_eq!(d["data"]["verified"], false);
    let out = t.path().join("branch.csv");
    let e = call(
        json!({"op":"export_csv","db":db,"path":out,"mode":"children","volume_id":v,"parent_id":folder}),
    );
    assert_eq!(e["ok"], true, "{e}");
    assert_eq!(e["data"]["rows"], 3);
    let mut rdr = csv::Reader::from_path(out).unwrap();
    assert_eq!(rdr.headers().unwrap().get(4), Some("relative_path"));
    let rows = rdr
        .records()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    assert!(rows
        .iter()
        .any(|r| r.get(3) == Some("Фильм, \"один\"\n.mkv")));
    assert!(!rows.iter().any(|r| r.get(3) == Some("outside.txt")));
}

#[test]
fn parquet_rows_and_empty_csv_schema() {
    use parquet::file::reader::{FileReader, SerializedFileReader};
    let t = tempfile::tempdir().unwrap();
    let db = t.path().join("test.sqlite");
    fixture(&db);
    let out = t.path().join("music.parquet");
    let r = call(json!({"op":"export_parquet","db":db,"path":out,"query":"ext:flac"}));
    assert_eq!(r["ok"], true, "{r}");
    let reader = SerializedFileReader::new(std::fs::File::open(out).unwrap()).unwrap();
    assert_eq!(reader.metadata().file_metadata().num_rows(), 2);
    assert_eq!(
        reader
            .metadata()
            .file_metadata()
            .schema_descr()
            .num_columns(),
        15
    );
    let out = t.path().join("empty.csv");
    let r = call(json!({"op":"export_csv","db":db,"path":out,"query":"name:impossible"}));
    assert_eq!(r["data"]["rows"], 0);
    assert!(std::fs::read_to_string(out)
        .unwrap()
        .starts_with("id,volume_id,parent_id,name,"));
}

#[test]
fn output_cannot_destroy_database_or_existing_file() {
    let t = tempfile::tempdir().unwrap();
    let db = t.path().join("test.sqlite");
    fixture(&db);
    for op in ["export_csv", "export_parquet", "export_json"] {
        let r = call(json!({"op":op,"db":db,"path":db,"overwrite":true}));
        assert_eq!(r["ok"], false, "{r}");
        assert_eq!(r["error"]["code"], "unsafe_output");
    }
    assert_eq!(Catalog::open(&db).unwrap().stats().unwrap()["entries"], 5);
    let out = t.path().join("keep.csv");
    std::fs::write(&out, b"keep me").unwrap();
    let r = call(json!({"op":"export_csv","db":db,"path":out}));
    assert_eq!(r["error"]["code"], "file_exists");
    assert_eq!(std::fs::read(out).unwrap(), b"keep me");
}

#[test]
fn ctf_create_import_edit_export_preserves_structure() {
    use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits};
    let t = tempfile::tempdir().unwrap();
    let db = t.path().join("legacy.sqlite");
    let source = t.path().join("source.ctf");
    let base = CtfEntry {
        name: "Музыка".into(),
        description: String::new(),
        parent: None,
        kind: EntryKind::Directory,
        size: 999999,
        attributes: 16,
        modified_dos: 0,
        created_dos: 0,
        accessed_dos: 0,
        opaque: Vec::new(),
    };
    let file = CtfEntry {
        name: "Трек 🎵.flac".into(),
        description: "Original description".into(),
        parent: Some(0),
        kind: EntryKind::File,
        size: 5_000_000_000,
        attributes: 32,
        ..base.clone()
    };
    let cat = CtfCatalog {
        name: "Test".into(),
        disks: vec![CtfDisk {
            name: "Disk".into(),
            entries: vec![base, file],
            opaque_header: Vec::new(),
        }],
        opaque_trailer: Vec::new(),
    };
    std::fs::write(&source, ctf_format::write_ctf(&cat).unwrap()).unwrap();
    let r = call(json!({"op":"import_ctf","db":db,"path":source}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["data"]["entries"], 2);
    let q = call(json!({"op":"search","db":db,"query":"ext:flac"}));
    let id = q["data"]["items"][0]["id"].as_i64().unwrap();
    let r =
        call(json!({"op":"update_entry","db":db,"entry_id":id,"description":"Изменено","tags":[]}));
    assert_eq!(r["ok"], true, "{r}");
    let out = t.path().join("new.ctf");
    let r = call(json!({"op":"export_ctf","db":db,"path":out}));
    assert_eq!(r["ok"], true, "{r}");
    let read = ctf_format::parse_ctf(&std::fs::read(out).unwrap(), &Limits::default()).unwrap();
    let track = read.disks[0]
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::File)
        .unwrap();
    assert_eq!(track.name, "Трек 🎵.flac");
    assert_eq!(track.description, "Изменено");
    assert_eq!(track.size, 5_000_000_000);
    assert_eq!(track.parent, Some(0));
    assert_eq!(
        Catalog::open(&db).unwrap().stats().unwrap()["total_bytes"],
        5_000_000_000u64
    );
    let r = call(json!({"op":"import_ctf","db":db,"path":source}));
    assert_eq!(r["error"]["code"], "already_imported");
}

#[test]
fn malformed_export_scope_fails_before_creating_file() {
    let t = tempfile::tempdir().unwrap();
    let db = t.path().join("test.sqlite");
    fixture(&db);
    let out = t.path().join("bad.csv");
    let r = call(json!({"op":"export_csv","db":db,"path":out,"query":12}));
    assert_eq!(r["error"]["code"], "invalid_request");
    assert!(!out.exists());
}

#[test]
fn ctf_export_rejects_native_tags_unless_loss_is_explicit() {
    let t = tempfile::tempdir().unwrap();
    let db = t.path().join("tags.sqlite");
    let (_, folder) = fixture(&db);
    let empty_tags_out = t.path().join("without-tags.ctf");
    let r = call(json!({"op":"export_ctf","db":db,"path":empty_tags_out}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["data"]["new_tags_omitted"], false);

    let r = call(
        json!({"op":"update_entry","db":db,"entry_id":folder,"description":"Tagged folder","tags":["watchlist","priority"]}),
    );
    assert_eq!(r["ok"], true, "{r}");
    let out = t.path().join("with-tags.ctf");
    for allow_lossy in [Value::Null, json!(false)] {
        let mut request = json!({"op":"export_ctf","db":db,"path":out});
        if !allow_lossy.is_null() {
            request["allow_lossy"] = allow_lossy;
        }
        let r = call(request);
        assert_eq!(r["ok"], false, "{r}");
        assert_eq!(r["error"]["code"], "ctf_tags");
        assert!(!out.exists());
    }
    let r = call(json!({"op":"export_ctf","db":db,"path":out,"allow_lossy":true}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["data"]["new_tags_omitted"], true);
    assert_eq!(r["data"]["new_categories_omitted"], false);
    let exported = ctf_format::parse_ctf(
        &std::fs::read(&out).unwrap(),
        &ctf_format::Limits::default(),
    )
    .unwrap();
    assert_eq!(
        exported
            .disks
            .iter()
            .map(|d| d.entries.len())
            .sum::<usize>(),
        5
    );
    let tags: String = Catalog::open(&db)
        .unwrap()
        .conn()
        .query_row("SELECT tags FROM entries WHERE id=?1", [folder], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(tags.contains("watchlist"));
    assert!(tags.contains("priority"));
}

/// v2 creates no opaque legacy row for an ordinary file; v1 remains readable
/// and writable without silently migrating or rewriting the user's database.
#[test]
fn ctf_sparse_v2_and_existing_v1_layout_are_both_supported() {
    use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits};
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("legacy-source.ctf");
    let fixture = CtfCatalog {
        name: "Compatibility test".into(),
        disks: vec![CtfDisk {
            name: "Offline disk".into(),
            opaque_header: Vec::new(),
            entries: vec![CtfEntry {
                name: "sample.flac".into(),
                description: String::new(),
                parent: None,
                kind: EntryKind::File,
                size: 1234,
                attributes: 32,
                modified_dos: 0,
                created_dos: 0,
                accessed_dos: 0,
                opaque: Vec::new(),
            }],
        }],
        opaque_trailer: Vec::new(),
    };
    std::fs::write(&source, ctf_format::write_ctf(&fixture).unwrap()).unwrap();
    // The CTF reader can expose nonempty raw fields even when a synthetic
    // input specified no optional extensions: use the parsed source as oracle.
    let parsed =
        ctf_format::parse_ctf(&std::fs::read(&source).unwrap(), &Limits::default()).unwrap();
    let expected_payload_rows: i64 = parsed
        .disks
        .iter()
        .map(|disk| {
            disk.entries
                .iter()
                .filter(|entry| !entry.opaque.is_empty())
                .count() as i64
        })
        .sum();
    for legacy in [false, true] {
        let db = t.path().join(if legacy {
            "existing-v1.sqlite"
        } else {
            "new-v2.sqlite"
        });
        {
            let catalog = Catalog::open(&db).unwrap();
            let version: i64 = catalog
                .conn()
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(version, 2);
            if legacy {
                // Simulates a previously created v1 database before its first CTF import.
                catalog
                    .conn()
                    .pragma_update(None, "user_version", 1i64)
                    .unwrap();
            }
        }
        let result = call(json!({"op":"import_ctf","db":db,"path":source}));
        assert_eq!(result["ok"], true, "{result}");
        let catalog = Catalog::open(&db).unwrap();
        let version: i64 = catalog
            .conn()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, if legacy { 1 } else { 2 });
        let columns = catalog
            .conn()
            .prepare("PRAGMA table_info(ctf_entries)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(columns.contains(&"opaque".to_owned()), legacy);
        if !legacy {
            let rows: i64 = catalog
                .conn()
                .query_row("SELECT COUNT(*) FROM ctf_entry_payload", [], |r| r.get(0))
                .unwrap();
            assert_eq!(
                rows, expected_payload_rows,
                "Sparse CTF storage must match the parsed original nonempty byte payloads"
            );
        }
        drop(catalog);
        let output = t
            .path()
            .join(if legacy { "back-v1.ctf" } else { "back-v2.ctf" });
        let result = call(json!({"op":"export_ctf","db":db,"path":output}));
        assert_eq!(result["ok"], true, "{result}");
        let exported =
            ctf_format::parse_ctf(&std::fs::read(&output).unwrap(), &Limits::default()).unwrap();
        assert_eq!(exported.disks.len(), 1);
        assert_eq!(exported.disks[0].entries.len(), 1);
        assert_eq!(exported.disks[0].entries[0].name, "sample.flac");
        assert_eq!(exported.disks[0].entries[0].size, 1234);
    }
}
