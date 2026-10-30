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
    // Windows fails closed at the foreign SQLite header check; Unix also
    // rejects the multiply-linked inode before opening SQLite.
    #[cfg(any(unix, windows))]
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
        assert_eq!(denied["ok"], false, "{denied}");
        #[cfg(unix)]
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

fn make_flat_ctf(path: &Path, names: &[&str]) -> Vec<u8> {
    let bytes = write_ctf(&CtfCatalog {
        name: "synthetic reconciliation".into(),
        opaque_trailer: Vec::new(),
        disks: vec![CtfDisk {
            name: "synthetic removable".into(),
            opaque_header: Vec::new(),
            entries: names
                .iter()
                .map(|name| CtfEntry {
                    name: (*name).into(),
                    description: String::new(),
                    parent: None,
                    kind: EntryKind::File,
                    size: 1234,
                    attributes: 32,
                    modified_dos: 0,
                    created_dos: 0,
                    accessed_dos: 0,
                    opaque: Vec::new(),
                })
                .collect(),
        }],
    })
    .unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}

fn flat_ids(source: &Path, cache: &Path, generation: &str) -> Vec<(String, i64)> {
    let volumes = invoke(json!({
        "op":"ctf_volumes_pinned","path":source,"cache_root":cache,
        "generation":generation
    }));
    assert_eq!(volumes["ok"], true, "{volumes}");
    let volume = volumes["data"]["result"][0]["id"].as_i64().unwrap();
    let children = invoke(json!({
        "op":"ctf_children_pinned","path":source,"cache_root":cache,
        "generation":generation,"volume_id":volume,"limit":50
    }));
    assert_eq!(children["ok"], true, "{children}");
    children["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["relative_path"].as_str().unwrap().to_owned(),
                entry["id"].as_i64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn reconciliation_preview_is_read_only_bounded_and_never_automatically_rebinds() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic.ctf");
    let cache = temp.path().join("mirror");
    let store = temp.path().join("private-notes.wia.sqlite");
    make_flat_ctf(&source, &["keep.mp3", "dupe.mp3", "gone.mp3"]);
    let first = sync(&source, &cache);
    let previous = flat_ids(&source, &cache, &first);
    assert_eq!(previous.len(), 3);

    let not_created = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &first,
        &store,
    ));
    assert_eq!(not_created["ok"], true, "{not_created}");
    assert_eq!(not_created["data"]["result"]["state"], "uninitialized");
    assert!(!store.exists());

    for (name, entry_id) in &previous {
        let mut req = cmd(
            "ctf_annotations_set_pinned",
            &source,
            &cache,
            &first,
            &store,
        );
        req["entry_id"] = json!(entry_id);
        req["expected_revision"] = json!(0);
        req["note"] = json!(format!("synthetic note for {name}"));
        req["tags"] = json!("review");
        let saved = invoke(req);
        assert_eq!(saved["ok"], true, "{saved}");
    }
    let current = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &first,
        &store,
    ));
    assert_eq!(current["data"]["result"]["state"], "current_generation");
    assert_eq!(current["data"]["result"]["items"], json!([]));

    let before_store = fs::read(&store).unwrap();
    let next_ctf = make_flat_ctf(&source, &["keep.mp3", "dupe.mp3", "dupe.mp3", "new.mp3"]);
    let stale = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &first,
        &store,
    ));
    assert_eq!(stale["error"]["code"], "source_changed", "{stale}");

    let next = sync(&source, &cache);
    assert_ne!(first, next);
    let mirror = cache.join(&next).join("catalog.sqlite");
    let before_mirror = fs::read(&mirror).unwrap();

    let mut page = cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &next,
        &store,
    );
    page["limit"] = json!(2);
    let first_page = invoke(page.clone());
    assert_eq!(first_page["ok"], true, "{first_page}");
    assert_eq!(first_page["data"]["result"]["state"], "generation_conflict");
    assert_eq!(first_page["data"]["result"]["total"], 3);
    assert_eq!(
        first_page["data"]["result"]["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        first_page["data"]["result"]["requires_explicit_confirmation"],
        true
    );
    assert_eq!(first_page["data"]["result"]["automatic_rebinding"], false);
    page["offset"] = json!(2);
    let second_page = invoke(page);
    assert_eq!(second_page["ok"], true, "{second_page}");
    let mut items = first_page["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .clone();
    items.extend(
        second_page["data"]["result"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    assert_eq!(items.len(), 3);

    for item in &items {
        assert_eq!(item["requires_confirmation"], true);
        assert_eq!(item["revision"], 1);
        assert_eq!(item["tags"], "review");
        let name = item["fingerprint"]["relative_path"].as_str().unwrap();
        let candidates = item["candidates"].as_array().unwrap();
        match name {
            "keep.mp3" => {
                assert_eq!(item["candidate_state"], "unique_unconfirmed");
                assert_eq!(candidates.len(), 1);
                assert_eq!(item["candidate_count_exact"], true);
            }
            "dupe.mp3" => {
                assert_eq!(item["candidate_state"], "ambiguous");
                assert_eq!(candidates.len(), 2);
                assert_ne!(candidates[0]["entry_id"], candidates[1]["entry_id"]);
            }
            "gone.mp3" => {
                assert_eq!(item["candidate_state"], "no_match");
                assert!(candidates.is_empty());
            }
            other => panic!("unexpected synthetic annotation {other}"),
        }
    }
    let conflict = invoke(cmd(
        "ctf_annotations_list_pinned",
        &source,
        &cache,
        &next,
        &store,
    ));
    assert_eq!(conflict["data"]["result"]["state"], "generation_conflict");
    assert_eq!(conflict["data"]["result"]["items"], json!([]));
    assert_eq!(fs::read(&store).unwrap(), before_store);
    assert_eq!(fs::read(&source).unwrap(), next_ctf);
    assert_eq!(fs::read(&mirror).unwrap(), before_mirror);
}

#[test]
fn explicitly_confirmed_candidates_are_cas_guarded_and_never_activated() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("synthetic.ctf");
    let cache = tmp.path().join("derived");
    let store = tmp.path().join("notes.wia.sqlite");
    make_flat_ctf(&source, &["keep.mp3", "dupe.mp3", "lost.mp3"]);
    let before = sync(&source, &cache);
    let old_ids = flat_ids(&source, &cache, &before);
    for (name, id) in &old_ids {
        let mut set = cmd(
            "ctf_annotations_set_pinned",
            &source,
            &cache,
            &before,
            &store,
        );
        set["entry_id"] = json!(id);
        set["expected_revision"] = json!(0);
        set["note"] = json!(format!("reviewed: {name}"));
        set["tags"] = json!("synthetic");
        assert_eq!(invoke(set)["ok"], true);
    }

    let ctf_bytes = make_flat_ctf(&source, &["keep.mp3", "dupe.mp3", "dupe.mp3", "new.mp3"]);
    let after = sync(&source, &cache);
    let mirror = cache.join(&after).join("catalog.sqlite");
    let mirror_bytes = fs::read(&mirror).unwrap();
    let initial_store = fs::read(&store).unwrap();

    let preview = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &after,
        &store,
    ));
    assert_eq!(preview["ok"], true, "{preview}");
    let doc = preview["data"]["result"]["document_id"].as_str().unwrap();
    let entries = preview["data"]["result"]["items"].as_array().unwrap();
    let keep = entries
        .iter()
        .find(|v| v["fingerprint"]["relative_path"] == "keep.mp3")
        .unwrap();
    let dupe = entries
        .iter()
        .find(|v| v["fingerprint"]["relative_path"] == "dupe.mp3")
        .unwrap();
    let lost = entries
        .iter()
        .find(|v| v["fingerprint"]["relative_path"] == "lost.mp3")
        .unwrap();
    let keep_target = keep["candidates"][0]["entry_id"].as_i64().unwrap();
    let dupe_target = dupe["candidates"][1]["entry_id"].as_i64().unwrap();

    let mut confirm = cmd(
        "ctf_annotations_confirm_pinned",
        &source,
        &cache,
        &after,
        &store,
    );
    confirm["old_entry_id"] = keep["old_entry_id"].clone();
    confirm["new_entry_id"] = json!(keep_target);
    confirm["expected_revision"] = keep["revision"].clone();
    confirm["expected_document_id"] = json!(doc);
    confirm["confirmation"] = json!("user_selected_exact_candidate");

    let without_intent = {
        let mut req = confirm.clone();
        req["confirmation"] = json!("maybe");
        invoke(req)
    };
    assert_eq!(
        without_intent["error"]["code"], "confirmation_required",
        "{without_intent}"
    );
    let wrong_revision = {
        let mut req = confirm.clone();
        req["expected_revision"] = json!(42);
        invoke(req)
    };
    assert_eq!(
        wrong_revision["error"]["code"], "annotation_revision_conflict",
        "{wrong_revision}"
    );
    let wrong_document = {
        let mut req = confirm.clone();
        req["expected_document_id"] = json!("not-the-original-document");
        invoke(req)
    };
    assert_eq!(
        wrong_document["error"]["code"], "annotation_document_conflict",
        "{wrong_document}"
    );
    let wrong_candidate = {
        let mut req = confirm.clone();
        req["new_entry_id"] = json!(dupe_target);
        invoke(req)
    };
    assert_eq!(
        wrong_candidate["error"]["code"], "annotation_candidate_conflict",
        "{wrong_candidate}"
    );
    assert_eq!(fs::read(&store).unwrap(), initial_store);

    let accepted = invoke(confirm.clone());
    assert_eq!(accepted["ok"], true, "{accepted}");
    assert_eq!(
        accepted["data"]["result"]["state"],
        "confirmed_not_activated"
    );
    assert_eq!(
        accepted["data"]["result"]["annotation_rows_migrated"],
        false
    );
    assert_eq!(
        invoke(confirm)["error"]["code"],
        "annotation_confirmation_conflict"
    );
    let mut other = cmd(
        "ctf_annotations_confirm_pinned",
        &source,
        &cache,
        &after,
        &store,
    );
    other["old_entry_id"] = dupe["old_entry_id"].clone();
    other["new_entry_id"] = json!(dupe_target);
    other["expected_revision"] = dupe["revision"].clone();
    other["expected_document_id"] = json!(doc);
    other["confirmation"] = json!("user_selected_exact_candidate");
    assert_eq!(invoke(other)["ok"], true);

    let second_preview = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &after,
        &store,
    ));
    let items = second_preview["data"]["result"]["items"]
        .as_array()
        .unwrap();
    let accepted_keep = items
        .iter()
        .find(|v| v["fingerprint"]["relative_path"] == "keep.mp3")
        .unwrap();
    assert_eq!(
        accepted_keep["confirmed_selection"]["new_entry_id"],
        keep_target
    );
    assert_eq!(accepted_keep["requires_confirmation"], false);
    let still_unresolved = items
        .iter()
        .find(|v| v["old_entry_id"] == lost["old_entry_id"])
        .unwrap();
    assert_eq!(still_unresolved["candidate_state"], "no_match");
    assert_eq!(still_unresolved["confirmed_selection"], Value::Null);
    assert_eq!(still_unresolved["requires_confirmation"], true);

    // A recorded confirmation is NOT an annotation migration or a silent
    // rebind in the ordinary LIST/SET operations.
    let listed = invoke(cmd(
        "ctf_annotations_list_pinned",
        &source,
        &cache,
        &after,
        &store,
    ));
    assert_eq!(listed["data"]["result"]["state"], "generation_conflict");
    assert_eq!(listed["data"]["result"]["items"], json!([]));

    let conn = rusqlite::Connection::open(&store).unwrap();
    let audits: i64 = conn
        .query_row("SELECT COUNT(*) FROM annotation_confirmations", [], |r| {
            r.get(0)
        })
        .unwrap();
    let originals: i64 = conn
        .query_row("SELECT COUNT(*) FROM annotation_entries", [], |r| r.get(0))
        .unwrap();
    assert_eq!(audits, 2);
    assert_eq!(originals, 3);
    drop(conn);
    assert_eq!(fs::read(&source).unwrap(), ctf_bytes);
    assert_eq!(fs::read(&mirror).unwrap(), mirror_bytes);

    // A later external replacement invalidates this old generation pin,
    // even if a confirmation has already been recorded in the user store.
    make_flat_ctf(&source, &["next.mp3", "dupe.mp3", "dupe.mp3", "new.mp3"]);
    let stale = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &after,
        &store,
    ));
    assert_eq!(stale["error"]["code"], "source_changed", "{stale}");
}

#[test]
fn confirmation_never_creates_missing_user_store_and_rejects_double_binding() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("fixture.ctf");
    let cache = tmp.path().join("cache");
    let store = tmp.path().join("user.wia.sqlite");
    make_flat_ctf(&source, &["same.mp3", "same.mp3"]);
    let old_generation = sync(&source, &cache);
    let ids = flat_ids(&source, &cache, &old_generation);

    let new_bytes = make_flat_ctf(&source, &["same.mp3", "same.mp3", "extra.mp3"]);
    let new_generation = sync(&source, &cache);
    let mut missing = cmd(
        "ctf_annotations_confirm_pinned",
        &source,
        &cache,
        &new_generation,
        &store,
    );
    missing["old_entry_id"] = json!(ids[0].1);
    missing["new_entry_id"] = json!(1);
    missing["expected_revision"] = json!(1);
    missing["expected_document_id"] = json!("synthetic");
    missing["confirmation"] = json!("user_selected_exact_candidate");
    assert_eq!(invoke(missing)["error"]["code"], "annotation_store_missing");
    assert!(!store.exists());

    // Use an independent synthetic old catalog with saved annotations.
    make_flat_ctf(&source, &["same.mp3", "same.mp3"]);
    let restored = sync(&source, &cache);
    for (_, id) in &ids {
        let mut request = cmd(
            "ctf_annotations_set_pinned",
            &source,
            &cache,
            &restored,
            &store,
        );
        request["entry_id"] = json!(id);
        request["expected_revision"] = json!(0);
        request["note"] = json!("synthetic");
        request["tags"] = json!("");
        assert_eq!(invoke(request)["ok"], true);
    }
    fs::write(&source, new_bytes).unwrap();
    let fresh = sync(&source, &cache);
    let preview = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &fresh,
        &store,
    ));
    let rows = preview["data"]["result"]["items"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let target = rows[0]["candidates"][0]["entry_id"].as_i64().unwrap();
    let document = preview["data"]["result"]["document_id"].clone();
    let mut choose = cmd(
        "ctf_annotations_confirm_pinned",
        &source,
        &cache,
        &fresh,
        &store,
    );
    choose["confirmation"] = json!("user_selected_exact_candidate");
    choose["expected_document_id"] = document;
    choose["expected_revision"] = json!(1);
    choose["new_entry_id"] = json!(target);
    choose["old_entry_id"] = rows[0]["old_entry_id"].clone();
    assert_eq!(invoke(choose.clone())["ok"], true);
    choose["old_entry_id"] = rows[1]["old_entry_id"].clone();
    let collision = invoke(choose);
    assert_eq!(
        collision["error"]["code"], "annotation_confirmation_conflict",
        "{collision}"
    );
}


#[test]
fn edited_old_annotation_invalidates_an_earlier_confirmation_revision() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("synthetic.ctf");
    let cache = tmp.path().join("mirror");
    let store = tmp.path().join("user.wia.sqlite");
    let original_bytes = make_flat_ctf(&source, &["keep.mp3"]);
    let original_gen = sync(&source, &cache);
    let old_id = flat_ids(&source, &cache, &original_gen)[0].1;

    let mut edit = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &original_gen,
        &store,
    );
    edit["entry_id"] = json!(old_id);
    edit["expected_revision"] = json!(0);
    edit["note"] = json!("Original synthetic note");
    edit["tags"] = json!("");
    assert_eq!(invoke(edit.clone())["ok"], true);

    let next_bytes = make_flat_ctf(&source, &["keep.mp3", "other.mp3"]);
    let next_gen = sync(&source, &cache);
    let review = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &next_gen,
        &store,
    ));
    let row = &review["data"]["result"]["items"][0];
    let mut approve = cmd(
        "ctf_annotations_confirm_pinned",
        &source,
        &cache,
        &next_gen,
        &store,
    );
    approve["old_entry_id"] = row["old_entry_id"].clone();
    approve["new_entry_id"] = row["candidates"][0]["entry_id"].clone();
    approve["expected_revision"] = row["revision"].clone();
    approve["expected_document_id"] = review["data"]["result"]["document_id"].clone();
    approve["confirmation"] = json!("user_selected_exact_candidate");
    assert_eq!(invoke(approve)["ok"], true);

    fs::write(&source, &original_bytes).unwrap();
    let returned_gen = sync(&source, &cache);
    assert_eq!(returned_gen, original_gen);
    edit["expected_revision"] = json!(1);
    edit["note"] = json!("Revised note invalidates the previous selection");
    assert_eq!(invoke(edit)["data"]["result"]["revision"], 2);

    fs::write(&source, &next_bytes).unwrap();
    let latest_gen = sync(&source, &cache);
    assert_eq!(latest_gen, next_gen);
    let changed = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &latest_gen,
        &store,
    ));
    assert_eq!(changed["ok"], true, "{changed}");
    let selected = &changed["data"]["result"]["items"][0];
    assert_eq!(selected["revision"], 2);
    assert_eq!(selected["confirmed_selection"]["old_revision"], 1);
    assert_eq!(selected["confirmation_revision_conflict"], true);
    assert_eq!(selected["requires_confirmation"], true);
    assert_eq!(selected["confirmed_selection"]["user_confirmed"], true);
    assert_eq!(
        changed["data"]["result"]["automatic_rebinding"],
        false
    );

    // Delete and recreate a row with revision 1: the revision number alone
    // has an ABA hazard. The reviewed payload digest must invalidate it.
    fs::write(&source, &original_bytes).unwrap();
    let back = sync(&source, &cache);
    assert_eq!(back, original_gen);
    let mut clear = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &back,
        &store,
    );
    clear["entry_id"] = json!(old_id);
    clear["expected_revision"] = json!(2);
    clear["note"] = json!("");
    clear["tags"] = json!("");
    assert_eq!(invoke(clear)["data"]["result"]["revision"], 0);
    let mut recreated = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &back,
        &store,
    );
    recreated["entry_id"] = json!(old_id);
    recreated["expected_revision"] = json!(0);
    recreated["note"] = json!("Different note, same revision number");
    recreated["tags"] = json!("");
    assert_eq!(invoke(recreated)["data"]["result"]["revision"], 1);
    fs::write(&source, &next_bytes).unwrap();
    let once_more = sync(&source, &cache);
    assert_eq!(once_more, next_gen);
    let aba = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &once_more,
        &store,
    ));
    assert_eq!(aba["ok"], true, "{aba}");
    let row = &aba["data"]["result"]["items"][0];
    assert_eq!(row["revision"], 1);
    assert_eq!(row["confirmed_selection"]["old_revision"], 1);
    assert_eq!(row["confirmation_revision_conflict"], false);
    assert_eq!(row["confirmation_payload_conflict"], true);
    assert_eq!(row["requires_confirmation"], true);
}
