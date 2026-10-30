//! Public synthetic-only tests: durable user annotations never enter CTF/mirror.
use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn invoke(value: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&value.to_string())).unwrap()
}

fn make_ctf(path: &Path, filename: &str) -> Vec<u8> {
    let bytes = write_ctf(&CtfCatalog {
        name: "synthetic annotations fixture".into(),
        opaque_trailer: Vec::new(),
        disks: vec![CtfDisk {
            name: "synthetic removable".into(),
            opaque_header: Vec::new(),
            entries: vec![
                CtfEntry {
                    name: "folder".into(),
                    description: String::new(),
                    parent: None,
                    kind: EntryKind::Directory,
                    size: 0,
                    attributes: 16,
                    modified_dos: 0,
                    created_dos: 0,
                    accessed_dos: 0,
                    opaque: Vec::new(),
                },
                CtfEntry {
                    name: filename.into(),
                    description: String::new(),
                    parent: Some(0),
                    kind: EntryKind::File,
                    size: 1234,
                    attributes: 32,
                    modified_dos: 0,
                    created_dos: 0,
                    accessed_dos: 0,
                    opaque: Vec::new(),
                },
            ],
        }],
    })
    .unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}

fn sync(source: &Path, cache: &Path) -> String {
    let v = invoke(json!({"op":"ctf_sync","path":source,"cache_root":cache}));
    assert_eq!(v["ok"], true, "{v}");
    v["data"]["generation"].as_str().unwrap().to_owned()
}

fn file_id(source: &Path, cache: &Path, generation: &str) -> i64 {
    let disks = invoke(json!({"op":"ctf_volumes_pinned",
        "path":source,"cache_root":cache,"generation":generation}));
    assert_eq!(disks["ok"], true, "{disks}");
    let disk_id = disks["data"]["result"][0]["id"].as_i64().unwrap();
    let root = invoke(json!({"op":"ctf_children_pinned",
        "path":source,"cache_root":cache,"generation":generation,"volume_id":disk_id}));
    assert_eq!(root["ok"], true, "{root}");
    let folder_id = root["data"]["result"]["items"][0]["id"].as_i64().unwrap();
    let children = invoke(json!({"op":"ctf_children_pinned",
        "path":source,"cache_root":cache,"generation":generation,
        "volume_id":disk_id,"parent_id":folder_id}));
    assert_eq!(children["ok"], true, "{children}");
    children["data"]["result"]["items"][0]["id"]
        .as_i64()
        .unwrap()
}

fn cmd(op: &str, source: &Path, cache: &Path, generation: &str, store: &Path) -> Value {
    json!({"op":op,"path":source,"cache_root":cache,
        "generation":generation,"annotation_db":store})
}

#[test]
fn user_owned_annotations_are_durable_and_revision_guarded() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("data.CTF");
    let cache = tmp.path().join("derived");
    let store = tmp.path().join("my-notes.wia.sqlite");
    let source_bytes = make_ctf(&source, "alpha.mp3");
    let generation = sync(&source, &cache);
    let entry = file_id(&source, &cache, &generation);
    let mirror = cache.join(&generation).join("catalog.sqlite");
    let original_mirror = fs::read(&mirror).unwrap();

    let missing = invoke(cmd(
        "ctf_annotations_list_pinned",
        &source,
        &cache,
        &generation,
        &store,
    ));
    assert_eq!(
        missing["data"]["result"]["state"], "uninitialized",
        "{missing}"
    );
    assert!(!store.exists(), "Listing must not create a sidecar");

    let mut request = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &generation,
        &store,
    );
    request["entry_id"] = json!(entry);
    request["note"] = json!("Private note (synthetic)");
    request["tags"] = json!("favorite");
    request["expected_revision"] = json!(0);
    let created = invoke(request.clone());
    assert_eq!(created["ok"], true, "{created}");
    assert_eq!(created["data"]["result"]["revision"], 1);

    let stale = invoke(request);
    assert_eq!(
        stale["error"]["code"], "annotation_revision_conflict",
        "{stale}"
    );
    let listed = invoke(cmd(
        "ctf_annotations_list_pinned",
        &source,
        &cache,
        &generation,
        &store,
    ));
    assert_eq!(listed["ok"], true, "{listed}");
    assert_eq!(listed["data"]["result"]["total"], 1);
    let record = &listed["data"]["result"]["items"][0];
    assert_eq!(record["entry_id"], entry);
    assert_eq!(record["note"], "Private note (synthetic)");
    assert_eq!(record["tags"], "favorite");
    assert_eq!(record["revision"], 1);

    let mut update = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &generation,
        &store,
    );
    update["entry_id"] = json!(entry);
    update["note"] = json!("Second revision");
    update["tags"] = json!("favorite");
    update["expected_revision"] = json!(1);
    let saved = invoke(update);
    assert_eq!(saved["data"]["result"]["revision"], 2, "{saved}");

    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(fs::read(&mirror).unwrap(), original_mirror);
    // Connection teardown and reopening are part of the test, not just RAM.
    let persisted = invoke(cmd(
        "ctf_annotations_list_pinned",
        &source,
        &cache,
        &generation,
        &store,
    ));
    assert_eq!(
        persisted["data"]["result"]["items"][0]["note"],
        "Second revision"
    );
    assert_eq!(persisted["data"]["result"]["items"][0]["revision"], 2);

    let mut clear = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &generation,
        &store,
    );
    clear["entry_id"] = json!(entry);
    clear["note"] = json!("");
    clear["tags"] = json!("");
    clear["expected_revision"] = json!(2);
    let removed = invoke(clear);
    assert_eq!(removed["data"]["result"]["revision"], 0, "{removed}");
    let empty = invoke(cmd(
        "ctf_annotations_list_pinned",
        &source,
        &cache,
        &generation,
        &store,
    ));
    assert_eq!(empty["data"]["result"]["total"], 0, "{empty}");
}

#[test]
fn external_ctf_generation_change_is_conflict_not_silent_reassociation() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("catalog.ctf");
    let cache = tmp.path().join("mirror");
    let store = tmp.path().join("user.wia.sqlite");
    let original = make_ctf(&source, "alpha.mp3");
    let first = sync(&source, &cache);
    let id = file_id(&source, &cache, &first);

    let mut edit = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &first,
        &store,
    );
    edit["entry_id"] = json!(id);
    edit["expected_revision"] = json!(0);
    edit["note"] = json!("Keep this, not in CTF");
    edit["tags"] = json!("offline");
    assert_eq!(invoke(edit)["ok"], true);
    let sidecar_bytes = fs::read(&store).unwrap();

    let modified = make_ctf(&source, "bravo.mp3");
    assert_eq!(
        original.len(),
        modified.len(),
        "Same-size replacement fixture"
    );
    let stale = invoke(cmd(
        "ctf_annotations_list_pinned",
        &source,
        &cache,
        &first,
        &store,
    ));
    assert_eq!(stale["error"]["code"], "source_changed", "{stale}");

    let next = sync(&source, &cache);
    assert_ne!(first, next);
    let conflict = invoke(cmd(
        "ctf_annotations_list_pinned",
        &source,
        &cache,
        &next,
        &store,
    ));
    assert_eq!(conflict["ok"], true, "{conflict}");
    assert_eq!(conflict["data"]["result"]["state"], "generation_conflict");
    assert_eq!(
        conflict["data"]["result"]["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        conflict["data"]["result"]["requires_explicit_reconciliation"],
        true
    );
    assert_eq!(conflict["data"]["result"]["total"], 1);
    assert_eq!(fs::read(&store).unwrap(), sidecar_bytes);
    assert_eq!(fs::read(&source).unwrap(), modified);

    let new_id = file_id(&source, &cache, &next);
    let mut illegal = cmd("ctf_annotations_set_pinned", &source, &cache, &next, &store);
    illegal["entry_id"] = json!(new_id);
    illegal["expected_revision"] = json!(0);
    illegal["note"] = json!("Should not be reassigned");
    illegal["tags"] = json!("");
    let denied = invoke(illegal);
    assert_eq!(
        denied["error"]["code"], "annotation_generation_conflict",
        "{denied}"
    );
    assert_eq!(fs::read(&store).unwrap(), sidecar_bytes);
}

#[test]
fn cache_path_and_foreign_sqlite_are_never_annotation_destinations() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("synthetic.ctf");
    let cache = tmp.path().join("cache");
    make_ctf(&source, "alpha.mp3");
    let generation = sync(&source, &cache);
    let id = file_id(&source, &cache, &generation);

    let inside_cache = cache.join("notes.wia.sqlite");
    let mut bad = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &generation,
        &inside_cache,
    );
    bad["entry_id"] = json!(id);
    bad["expected_revision"] = json!(0);
    bad["note"] = json!("test");
    bad["tags"] = json!("");
    let rejected = invoke(bad);
    assert_eq!(
        rejected["error"]["code"], "unsafe_annotation_store",
        "{rejected}"
    );
    assert!(!inside_cache.exists());

    // A hardlink has a different name but the SAME bytes and inode as source.
    // The .wia.sqlite extension must not permit writing through that alias.
    #[cfg(unix)]
    {
        let alias = tmp.path().join("alias.wia.sqlite");
        fs::hard_link(&source, &alias).unwrap();
        let mut hardlink = cmd(
            "ctf_annotations_set_pinned",
            &source,
            &cache,
            &generation,
            &alias,
        );
        hardlink["entry_id"] = json!(id);
        hardlink["expected_revision"] = json!(0);
        hardlink["note"] = json!("MUST NOT WRITE");
        hardlink["tags"] = json!("");
        let denied = invoke(hardlink);
        assert_eq!(
            denied["error"]["code"], "unsafe_annotation_store",
            "{denied}"
        );
        let original = fs::read(&source).unwrap();
        assert_eq!(original, fs::read(&alias).unwrap());
    }

    let foreign = tmp.path().join("foreign.wia.sqlite");
    let conn = rusqlite::Connection::open(&foreign).unwrap();
    conn.execute_batch(
        "CREATE TABLE valuables (v TEXT); INSERT INTO valuables VALUES('leave me alone');",
    )
    .unwrap();
    drop(conn);
    let mut attempt = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &generation,
        &foreign,
    );
    attempt["entry_id"] = json!(id);
    attempt["expected_revision"] = json!(0);
    attempt["note"] = json!("test");
    attempt["tags"] = json!("");
    let denial = invoke(attempt);
    assert_eq!(
        denial["error"]["code"], "annotation_store_invalid",
        "{denial}"
    );
    let conn = rusqlite::Connection::open(&foreign).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM valuables", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let app: i64 = conn
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .unwrap();
    assert_eq!(app, 0);
}
