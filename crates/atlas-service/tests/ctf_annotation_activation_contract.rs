//! Public, synthetic-only tests for auditable, reversible annotation activation.
use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn invoke(value: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&value.to_string())).unwrap()
}

fn make_ctf(path: &Path, names: &[&str]) -> Vec<u8> {
    let bytes = write_ctf(&CtfCatalog {
        name: "synthetic activation fixture".into(),
        opaque_trailer: Vec::new(),
        disks: vec![CtfDisk {
            name: "synthetic disk".into(),
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

fn cmd(op: &str, source: &Path, cache: &Path, generation: &str, store: &Path) -> Value {
    json!({"op":op,"path":source,"cache_root":cache,
        "generation":generation,"annotation_db":store})
}

fn sync(source: &Path, cache: &Path) -> String {
    let response = invoke(json!({"op":"ctf_sync","path":source,"cache_root":cache}));
    assert_eq!(response["ok"], true, "{response}");
    response["data"]["generation"].as_str().unwrap().into()
}

fn flat_ids(source: &Path, cache: &Path, generation: &str) -> Vec<(String, i64)> {
    let volumes = invoke(json!({"op":"ctf_volumes_pinned",
        "path":source,"cache_root":cache,"generation":generation}));
    assert_eq!(volumes["ok"], true, "{volumes}");
    let volume = volumes["data"]["result"][0]["id"].as_i64().unwrap();
    let children = invoke(json!({"op":"ctf_children_pinned","path":source,
        "cache_root":cache,"generation":generation,"volume_id":volume}));
    assert_eq!(children["ok"], true, "{children}");
    children["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["relative_path"].as_str().unwrap().to_owned(),
                item["id"].as_i64().unwrap(),
            )
        })
        .collect()
}

fn set_note(
    source: &Path,
    cache: &Path,
    generation: &str,
    store: &Path,
    entry_id: i64,
    note: &str,
) {
    let mut request = cmd(
        "ctf_annotations_set_pinned",
        source,
        cache,
        generation,
        store,
    );
    request["entry_id"] = json!(entry_id);
    request["note"] = json!(note);
    request["tags"] = json!("reviewed");
    request["expected_revision"] = json!(0);
    let response = invoke(request);
    assert_eq!(response["ok"], true, "{response}");
}

fn confirm(source: &Path, cache: &Path, generation: &str, store: &Path) -> (Value, i64, i64) {
    let preview = invoke(cmd(
        "ctf_annotations_preview_pinned",
        source,
        cache,
        generation,
        store,
    ));
    assert_eq!(preview["ok"], true, "{preview}");
    let row = preview["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["fingerprint"]["relative_path"] == "keep.mp3")
        .unwrap();
    let old_id = row["old_entry_id"].as_i64().unwrap();
    let new_id = row["candidates"][0]["entry_id"].as_i64().unwrap();
    let mut request = cmd(
        "ctf_annotations_confirm_pinned",
        source,
        cache,
        generation,
        store,
    );
    request["old_entry_id"] = json!(old_id);
    request["new_entry_id"] = json!(new_id);
    request["expected_revision"] = row["revision"].clone();
    request["expected_document_id"] = preview["data"]["result"]["document_id"].clone();
    request["confirmation"] = json!("user_selected_exact_candidate");
    let response = invoke(request.clone());
    assert_eq!(response["ok"], true, "{response}");
    request["op"] = json!("ctf_annotations_activate_pinned");
    request.as_object_mut().unwrap().remove("confirmation");
    request["activation"] = json!("user_approved_reversible_projection");
    (request, old_id, new_id)
}

#[test]
fn activated_projection_is_reversible_and_preserves_originals() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic.ctf");
    let cache = temp.path().join("derived");
    let store = temp.path().join("my-notes.wia.sqlite");
    let old_ctf = make_ctf(&source, &["keep.mp3", "gone.mp3"]);
    let first = sync(&source, &cache);
    let ids = flat_ids(&source, &cache, &first);
    for (name, id) in &ids {
        set_note(
            &source,
            &cache,
            &first,
            &store,
            *id,
            &format!("synthetic note: {name}"),
        );
    }
    let new_ctf = make_ctf(&source, &["keep.mp3", "new.mp3"]);
    let second = sync(&source, &cache);
    let mirror = cache.join(&second).join("catalog.sqlite");
    let mirror_original = fs::read(&mirror).unwrap();
    let (activation, old_id, new_id) = confirm(&source, &cache, &second, &store);
    let missing_intent = {
        let mut bad = activation.clone();
        bad["activation"] = json!("implicit");
        invoke(bad)
    };
    assert_eq!(
        missing_intent["error"]["code"], "activation_required",
        "{missing_intent}"
    );

    // No implicit activation is recorded just because the user selected a
    // structural candidate. Both the new audit status fields are nullable.
    let before = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    let before_row = before["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["old_entry_id"] == old_id)
        .unwrap();
    assert_eq!(before_row["activation_state"], Value::Null);
    assert_eq!(before_row["activation_target_entry_id"], Value::Null);

    let saved = invoke(activation.clone());
    assert_eq!(saved["ok"], true, "{saved}");
    assert_eq!(
        saved["data"]["result"]["state"],
        "activated_read_only_overlay"
    );
    assert_eq!(
        saved["data"]["result"]["original_annotations_preserved"],
        true
    );
    assert_eq!(saved["data"]["result"]["new_generation_writable"], false);
    assert_eq!(
        invoke(activation.clone())["error"]["code"],
        "annotation_activation_conflict"
    );
    let active_preview = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    let active_row = active_preview["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["old_entry_id"] == old_id)
        .unwrap();
    assert_eq!(active_row["activation_state"], "active");
    assert_eq!(active_row["activation_target_entry_id"], new_id);

    let listed = invoke(cmd(
        "ctf_annotations_activated_list_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    assert_eq!(listed["ok"], true, "{listed}");
    assert_eq!(listed["data"]["result"]["state"], "partial_activation");
    assert_eq!(listed["data"]["result"]["total"], 1);
    assert_eq!(listed["data"]["result"]["items"][0]["entry_id"], new_id);
    assert_eq!(
        listed["data"]["result"]["items"][0]["note"],
        "synthetic note: keep.mp3"
    );
    assert_eq!(
        listed["data"]["result"]["unresolved_unconfirmed_annotations_not_projected"],
        true
    );

    // The native verified viewer repeats a pinned read on every page. Reading
    // live overlays must never change the source, derived cache or user notes.
    let notes_before_view = fs::read(&store).unwrap();
    for _ in 0..3 {
        let current = invoke(cmd(
            "ctf_annotations_activated_list_pinned",
            &source,
            &cache,
            &second,
            &store,
        ));
        assert_eq!(current["ok"], true, "{current}");
        assert_eq!(
            current["data"]["result"]["items"],
            listed["data"]["result"]["items"]
        );
        assert_eq!(current["data"]["result"]["total"], 1);
        assert_eq!(current["data"]["result"]["conflicts"], 0);
        assert_eq!(current["data"]["result"]["read_only_source"], true);
        assert_eq!(
            current["data"]["result"]["unresolved_unconfirmed_annotations_not_projected"],
            true
        );
        assert_eq!(fs::read(&source).unwrap(), new_ctf);
        assert_eq!(fs::read(&mirror).unwrap(), mirror_original);
        assert_eq!(fs::read(&store).unwrap(), notes_before_view);
    }

    // A pending native view must not expose any old verified note when the
    // external CTF is edited, even with its derived mirror still on disk.
    let changed_ctf = make_ctf(&source, &["keep.mp3", "alt.mp3"]);
    assert_ne!(changed_ctf, new_ctf);
    let stale = invoke(cmd(
        "ctf_annotations_activated_list_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    assert_eq!(stale["error"]["code"], "source_changed", "{stale}");
    assert_eq!(fs::read(&store).unwrap(), notes_before_view);
    assert_eq!(fs::read(&mirror).unwrap(), mirror_original);
    fs::write(&source, &new_ctf).unwrap();
    let refreshed = invoke(cmd(
        "ctf_annotations_activated_list_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    assert_eq!(refreshed["ok"], true, "{refreshed}");
    assert_eq!(refreshed["data"]["result"]["items"], listed["data"]["result"]["items"]);
    assert_eq!(fs::read(&store).unwrap(), notes_before_view);

    let mut paged = cmd(
        "ctf_annotations_activated_list_pinned",
        &source,
        &cache,
        &second,
        &store,
    );
    paged["offset"] = json!(1);
    let page = invoke(paged);
    assert_eq!(page["data"]["result"]["total"], 1);
    assert_eq!(page["data"]["result"]["items"], json!([]));

    // Old-generation edits after activation invalidate the *read* overlay,
    // rather than silently projecting a stale private note.
    fs::write(&source, &old_ctf).unwrap();
    assert_eq!(sync(&source, &cache), first);
    let mut edit = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &first,
        &store,
    );
    edit["entry_id"] = json!(old_id);
    edit["expected_revision"] = json!(1);
    edit["note"] = json!("newer synthetic note");
    edit["tags"] = json!("reviewed");
    assert_eq!(invoke(edit)["ok"], true);
    fs::write(&source, &new_ctf).unwrap();
    assert_eq!(sync(&source, &cache), second);
    let conflicted = invoke(cmd(
        "ctf_annotations_activated_list_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    assert_eq!(conflicted["data"]["result"]["state"], "activation_conflict");
    assert_eq!(conflicted["data"]["result"]["conflicts"], 1);
    assert_eq!(conflicted["data"]["result"]["items"], json!([]));

    // A now-stale active journal row must still be visible for explicit
    // revocation. GUI must NOT treat this audit status as a verified overlay.
    let stale_audit = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    assert_eq!(stale_audit["ok"], true, "{stale_audit}");
    let stale_row = stale_audit["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["old_entry_id"] == old_id)
        .unwrap();
    assert_eq!(stale_row["activation_state"], "active");
    assert_eq!(stale_row["activation_target_entry_id"], new_id);
    assert_eq!(stale_row["confirmation_revision_conflict"], true);

    // Legacy list and set MUST NOT silently enable new-generation annotation edits.
    let original_list = invoke(cmd(
        "ctf_annotations_list_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    assert_eq!(
        original_list["data"]["result"]["state"],
        "generation_conflict"
    );
    assert_eq!(original_list["data"]["result"]["items"], json!([]));

    let mut revocation = cmd(
        "ctf_annotations_deactivate_pinned",
        &source,
        &cache,
        &second,
        &store,
    );
    revocation["expected_document_id"] = activation["expected_document_id"].clone();
    revocation["old_entry_id"] = json!(old_id);
    revocation["new_entry_id"] = json!(new_id);
    revocation["rollback"] = json!("user_confirmed_revoke_projection");
    let reverted = invoke(revocation.clone());
    assert_eq!(reverted["ok"], true, "{reverted}");
    assert_eq!(reverted["data"]["result"]["backup_snapshot_retained"], true);
    assert_eq!(
        invoke(revocation)["error"]["code"],
        "annotation_activation_conflict"
    );
    let revoked_preview = invoke(cmd(
        "ctf_annotations_preview_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    let revoked_row = revoked_preview["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["old_entry_id"] == old_id)
        .unwrap();
    assert_eq!(revoked_row["activation_state"], "revoked");
    assert_eq!(revoked_row["activation_target_entry_id"], new_id);
    // The old record was explicitly edited after activation: revision CAS
    // rejects a replay before the revoked-history check is consulted.
    assert_eq!(
        invoke(activation)["error"]["code"],
        "annotation_revision_conflict"
    );

    let cleared = invoke(cmd(
        "ctf_annotations_activated_list_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    assert_eq!(cleared["data"]["result"]["total"], 0);
    assert_eq!(cleared["data"]["result"]["items"], json!([]));
    assert_eq!(cleared["data"]["result"]["conflicts"], 0);
    let conn = rusqlite::Connection::open(&store).unwrap();
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM annotation_entries", [], |r| r.get(0))
        .unwrap();
    let archived: (String, String) = conn
        .query_row(
            "SELECT state,note FROM annotation_activations WHERE old_entry_id=?1",
            [old_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(rows, 2);
    assert_eq!(archived.0, "revoked");
    assert_eq!(archived.1, "synthetic note: keep.mp3");
    assert_eq!(fs::read(&source).unwrap(), new_ctf);
    assert_eq!(fs::read(&mirror).unwrap(), mirror_original);
}

#[test]
fn changed_note_or_source_generation_rejects_activation_without_side_effects() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic.ctf");
    let cache = temp.path().join("derived");
    let store = temp.path().join("my-notes.wia.sqlite");
    let old_ctf = make_ctf(&source, &["keep.mp3"]);
    let first = sync(&source, &cache);
    let old_id = flat_ids(&source, &cache, &first)[0].1;
    set_note(&source, &cache, &first, &store, old_id, "old note");
    let new_ctf = make_ctf(&source, &["keep.mp3", "extra.mp3"]);
    let second = sync(&source, &cache);
    let (activation, _, _) = confirm(&source, &cache, &second, &store);

    fs::write(&source, &old_ctf).unwrap();
    assert_eq!(sync(&source, &cache), first);
    let mut edit = cmd(
        "ctf_annotations_set_pinned",
        &source,
        &cache,
        &first,
        &store,
    );
    edit["entry_id"] = json!(old_id);
    edit["note"] = json!("edited after confirmation");
    edit["tags"] = json!("reviewed");
    edit["expected_revision"] = json!(1);
    assert_eq!(invoke(edit)["ok"], true);
    fs::write(&source, &new_ctf).unwrap();
    assert_eq!(sync(&source, &cache), second);

    let failed = invoke(activation.clone());
    assert_eq!(
        failed["error"]["code"], "annotation_revision_conflict",
        "{failed}"
    );
    let no_activations = invoke(cmd(
        "ctf_annotations_activated_list_pinned",
        &source,
        &cache,
        &second,
        &store,
    ));
    assert_eq!(no_activations["data"]["result"]["total"], 0);
    assert_eq!(
        no_activations["data"]["result"]["state"],
        "no_cross_generation_activations"
    );

    make_ctf(&source, &["new-ctf.mp3", "extra.mp3"]);
    let stale = invoke(activation);
    assert_eq!(stale["error"]["code"], "source_changed", "{stale}");
}
