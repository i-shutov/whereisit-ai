//! Synthetic-only backup/restore acceptance: no CTF, mirror or live store writes.
use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn call(request: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&request.to_string())).unwrap()
}

fn fixture(source: &Path, file: &str) -> Vec<u8> {
    let contents = write_ctf(&CtfCatalog {
        name: "synthetic backup fixture".into(),
        opaque_trailer: Vec::new(),
        disks: vec![CtfDisk {
            name: "synthetic removable".into(),
            opaque_header: Vec::new(),
            entries: vec![CtfEntry {
                name: file.into(),
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
    })
    .unwrap();
    fs::write(source, &contents).unwrap();
    contents
}

fn pinned(source: &Path, cache: &Path) -> String {
    let result = call(json!({"op":"ctf_sync", "path":source, "cache_root":cache}));
    assert_eq!(result["ok"], true, "{result}");
    result["data"]["generation"].as_str().unwrap().into()
}

fn command(op: &str, source: &Path, cache: &Path, generation: &str, db: &Path) -> Value {
    json!({"op":op, "path":source, "cache_root":cache,
           "generation":generation, "annotation_db":db})
}

fn seeded(source: &Path, cache: &Path, generation: &str, store: &Path) -> (i64, String) {
    let volumes = call(json!({"op":"ctf_volumes_pinned","path":source,
        "cache_root":cache,"generation":generation}));
    let id = volumes["data"]["result"][0]["id"].as_i64().unwrap();
    let children = call(json!({"op":"ctf_children_pinned","path":source,
        "cache_root":cache,"generation":generation,"volume_id":id}));
    let entry = children["data"]["result"]["items"][0]["id"].as_i64().unwrap();
    let mut set = command(
        "ctf_annotations_set_pinned", source, cache, generation, store,
    );
    set["entry_id"] = json!(entry);
    set["expected_revision"] = json!(0);
    set["note"] = json!("synthetic private note");
    set["tags"] = json!("synthetic tag");
    assert_eq!(call(set)["ok"], true);
    let shown = call(command(
        "ctf_annotations_list_pinned", source, cache, generation, store,
    ));
    (
        entry,
        shown["data"]["result"]["document_id"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}

fn backup_request(
    source: &Path,
    cache: &Path,
    generation: &str,
    store: &Path,
    backup: &Path,
    document: &str,
) -> Value {
    let mut value = command(
        "ctf_annotations_backup_pinned", source, cache, generation, store,
    );
    value["backup_path"] = json!(backup);
    value["expected_document_id"] = json!(document);
    value["backup"] = json!("user_approved_new_snapshot");
    value
}

fn restore_request(
    source: &Path,
    cache: &Path,
    generation: &str,
    destination: &Path,
    backup: &Path,
    document: &str,
    sha: &str,
) -> Value {
    let mut request = command(
        "ctf_annotations_restore_pinned", source, cache, generation, destination,
    );
    request["backup_path"] = json!(backup);
    request["expected_document_id"] = json!(document);
    request["expected_backup_sha256"] = json!(sha);
    request["restore"] = json!("user_approved_new_sidecar");
    request
}

#[test]
fn whole_store_backup_and_restore_preserve_optional_audit_tables() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic.ctf");
    let cache = temp.path().join("derivative");
    let store = temp.path().join("annotations.wia.sqlite");
    let backup_path = temp.path().join("annotations.wia.backup.sqlite");
    let restored = temp.path().join("restored.wia.sqlite");
    let original_ctf = fixture(&source, "example.flac");
    let generation = pinned(&source, &cache);
    let mirror_path = cache.join(&generation).join("catalog.sqlite");
    let original_mirror = fs::read(&mirror_path).unwrap();
    let (entry, document) = seeded(&source, &cache, &generation, &store);
    let db = rusqlite::Connection::open(&store).unwrap();
    db.execute_batch(
        "CREATE TABLE hypothetical_future_audit(id INTEGER PRIMARY KEY, payload TEXT);
         INSERT INTO hypothetical_future_audit(payload) VALUES('synthetic audit record');",
    )
    .unwrap();
    drop(db);

    let before_store = fs::read(&store).unwrap();
    let request = backup_request(
        &source, &cache, &generation, &store, &backup_path, &document,
    );
    let backup = call(request.clone());
    assert_eq!(backup["ok"], true, "{backup}");
    assert_eq!(backup["data"]["result"]["state"], "snapshot_published");
    assert_eq!(backup["data"]["result"]["annotation_count"], 1);
    assert_eq!(backup["data"]["result"]["table_count"], 3);
    let digest = backup["data"]["result"]["backup_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(digest.len(), 64);
    assert_eq!(fs::read(&store).unwrap(), before_store);
    assert_eq!(fs::read(&source).unwrap(), original_ctf);
    assert_eq!(fs::read(&mirror_path).unwrap(), original_mirror);

    let refused = call(request);
    assert_eq!(
        refused["error"]["code"], "annotation_snapshot_exists", "{refused}"
    );
    let original_snapshot = fs::read(&backup_path).unwrap();

    // Mutate the LIVE private store after snapshot. Restore must not overwrite it.
    let mut edit = command(
        "ctf_annotations_set_pinned", &source, &cache, &generation, &store,
    );
    edit["entry_id"] = json!(entry);
    edit["expected_revision"] = json!(1);
    edit["note"] = json!("later private edit");
    edit["tags"] = json!("changed");
    assert_eq!(call(edit)["ok"], true);

    let restore = restore_request(
        &source, &cache, &generation, &restored, &backup_path, &document, &digest,
    );
    let output = call(restore.clone());
    assert_eq!(output["ok"], true, "{output}");
    assert_eq!(output["data"]["result"]["state"], "restored_to_new_sidecar");
    assert_eq!(
        output["data"]["result"]["requires_explicit_reconciliation"], false
    );
    let inspect = call(command(
        "ctf_annotations_list_pinned", &source, &cache, &generation, &restored,
    ));
    assert_eq!(inspect["ok"], true, "{inspect}");
    assert_eq!(
        inspect["data"]["result"]["items"][0]["note"], "synthetic private note"
    );
    let original = call(command(
        "ctf_annotations_list_pinned", &source, &cache, &generation, &store,
    ));
    assert_eq!(original["data"]["result"]["items"][0]["note"], "later private edit");
    let audit = rusqlite::Connection::open(&restored).unwrap();
    let record: String = audit
        .query_row("SELECT payload FROM hypothetical_future_audit LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(record, "synthetic audit record");
    assert_eq!(fs::read(&backup_path).unwrap(), original_snapshot);
    assert_eq!(fs::read(&source).unwrap(), original_ctf);
    assert_eq!(fs::read(&mirror_path).unwrap(), original_mirror);

    let second = call(restore);
    assert_eq!(second["error"]["code"], "annotation_snapshot_exists");
}

#[test]
fn bad_cas_corruption_and_changed_ctf_are_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic.ctf");
    let cache = temp.path().join("derivative");
    let store = temp.path().join("annotations.wia.sqlite");
    let backup_path = temp.path().join("private.wia.backup.sqlite");
    let target = temp.path().join("new.wia.sqlite");
    let original_ctf = fixture(&source, "example.flac");
    let generation = pinned(&source, &cache);
    let (_, document) = seeded(&source, &cache, &generation, &store);

    let mut wrong_doc = backup_request(
        &source, &cache, &generation, &store, &backup_path, &document,
    );
    wrong_doc["expected_document_id"] = json!("00000000000000000000000000000000");
    let error = call(wrong_doc);
    assert_eq!(error["error"]["code"], "annotation_document_conflict");
    assert!(!backup_path.exists());

    let backup = call(backup_request(
        &source, &cache, &generation, &store, &backup_path, &document,
    ));
    assert_eq!(backup["ok"], true, "{backup}");
    let digest = backup["data"]["result"]["backup_sha256"]
        .as_str()
        .unwrap();

    let bad_hash = call(restore_request(
        &source, &cache, &generation, &target, &backup_path,
        &document, &"0".repeat(64),
    ));
    assert_eq!(bad_hash["error"]["code"], "annotation_backup_changed");
    assert!(!target.exists());

    // Corrupt the selected snapshot: an attacker-supplied matching checksum
    // must not evade SQLite integrity/application checks.
    fs::write(&backup_path, b"not a SQLite database").unwrap();
    let tampered = call(restore_request(
        &source, &cache, &generation, &target, &backup_path, &document, digest,
    ));
    assert_eq!(tampered["ok"], false, "{tampered}");
    assert!(!target.exists());

    // Simulate an external save of the authoritative CTF without repinning.
    fixture(&source, "renamed.flac");
    let new_backup = temp.path().join("never.wia.backup.sqlite");
    let conflict = call(backup_request(
        &source, &cache, &generation, &store, &new_backup, &document,
    ));
    assert_eq!(conflict["error"]["code"], "source_changed", "{conflict}");
    assert!(!new_backup.exists());
    assert_ne!(fs::read(&source).unwrap(), original_ctf);
}

#[test]
fn backup_path_under_derivative_and_absent_sidecar_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic.ctf");
    let cache = temp.path().join("derivative");
    let store = temp.path().join("missing.wia.sqlite");
    fixture(&source, "example.flac");
    let generation = pinned(&source, &cache);
    let under_cache = cache.join("not-allowed.wia.backup.sqlite");
    let unsafe_request = call(backup_request(
        &source, &cache, &generation, &store,
        &under_cache, "00000000000000000000000000000000",
    ));
    assert_eq!(
        unsafe_request["error"]["code"], "unsafe_annotation_snapshot",
        "{unsafe_request}"
    );

    let ordinary = temp.path().join("absent.wia.backup.sqlite");
    let missing = call(backup_request(
        &source, &cache, &generation, &store,
        &ordinary, "00000000000000000000000000000000",
    ));
    assert_eq!(missing["error"]["code"], "annotation_store_missing");
    assert!(!ordinary.exists());
}
