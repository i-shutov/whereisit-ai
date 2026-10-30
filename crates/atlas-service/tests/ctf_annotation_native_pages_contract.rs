//! Synthetic-only page contract for native read-only annotation reconciliation.
use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn invoke(value: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&value.to_string())).unwrap()
}

fn fixture(path: &Path, marker: &str) {
    let entries = (0..24)
        .map(|n| CtfEntry {
            name: format!("synthetic-{n:02}.txt"),
            description: String::new(),
            parent: None,
            kind: EntryKind::File,
            size: 1000 + n,
            attributes: 32,
            modified_dos: 0,
            created_dos: 0,
            accessed_dos: 0,
            opaque: Vec::new(),
        })
        .collect();
    let bytes = write_ctf(&CtfCatalog {
        name: marker.into(),
        opaque_trailer: Vec::new(),
        disks: vec![CtfDisk {
            name: "synthetic removable disk".into(),
            opaque_header: Vec::new(),
            entries,
        }],
    })
    .unwrap();
    fs::write(path, bytes).unwrap();
}

fn sync(path: &Path, cache: &Path) -> String {
    let response = invoke(json!({
        "op":"ctf_sync","path":path,"cache_root":cache
    }));
    assert_eq!(response["ok"], true, "{response}");
    response["data"]["generation"].as_str().unwrap().to_owned()
}

fn pinned(path: &Path, cache: &Path, generation: &str, store: &Path, op: &str) -> Value {
    json!({
        "op":op,"path":path,"cache_root":cache,
        "generation":generation,"annotation_db":store
    })
}

#[test]
fn twenty_record_pages_are_complete_repeatable_and_source_pinned() {
    let temp = tempfile::tempdir().unwrap();
    let ctf = temp.path().join("synthetic.ctf");
    let cache = temp.path().join("derived");
    let notes = temp.path().join("annotations.wia.sqlite");
    fixture(&ctf, "before external update");
    let original = sync(&ctf, &cache);
    let volumes = invoke(json!({
        "op":"ctf_volumes_pinned","path":ctf,"cache_root":cache,"generation":original
    }));
    assert_eq!(volumes["ok"], true, "{volumes}");
    let volume_id = volumes["data"]["result"][0]["id"].as_i64().unwrap();
    let items = invoke(json!({
        "op":"ctf_children_pinned","path":ctf,"cache_root":cache,
        "generation":original,"volume_id":volume_id
    }));
    assert_eq!(items["ok"], true, "{items}");
    let ids: Vec<i64> = items["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids.len(), 24);

    for (n, id) in ids.iter().enumerate() {
        let mut request = pinned(
            &ctf,
            &cache,
            &original,
            &notes,
            "ctf_annotations_set_pinned",
        );
        request["entry_id"] = json!(id);
        request["note"] = json!(format!("synthetic note {n}"));
        request["tags"] = json!("fixture");
        request["expected_revision"] = json!(0);
        let saved = invoke(request);
        assert_eq!(saved["ok"], true, "{saved}");
    }
    let original_notes = fs::read(&notes).unwrap();

    // Simulates another program saving an externally authoritative CTF.
    fixture(&ctf, "after external update");
    let second = sync(&ctf, &cache);
    assert_ne!(second, original);
    let original_source = fs::read(&ctf).unwrap();
    let mirror = cache.join(&second).join("catalog.sqlite");
    let original_mirror = fs::read(&mirror).unwrap();

    let mut page = pinned(
        &ctf,
        &cache,
        &second,
        &notes,
        "ctf_annotations_preview_pinned",
    );
    page["limit"] = json!(20);
    page["offset"] = json!(0);
    let first = invoke(page.clone());
    assert_eq!(first["ok"], true, "{first}");
    let first_result = &first["data"]["result"];
    assert_eq!(first_result["state"], "generation_conflict");
    assert_eq!(first_result["total"], 24);
    assert_eq!(first_result["items"].as_array().unwrap().len(), 20);
    assert_eq!(first_result["preview_only"], true);
    assert_eq!(first_result["requires_explicit_confirmation"], true);

    page["offset"] = json!(20);
    let second_page = invoke(page.clone());
    assert_eq!(second_page["ok"], true, "{second_page}");
    assert_eq!(second_page["data"]["result"]["total"], 24);
    let last_items = second_page["data"]["result"]["items"].as_array().unwrap();
    assert_eq!(last_items.len(), 4);
    let first_ids: Vec<i64> = first_result["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["old_entry_id"].as_i64().unwrap())
        .collect();
    let last_ids: Vec<i64> = last_items
        .iter()
        .map(|v| v["old_entry_id"].as_i64().unwrap())
        .collect();
    assert!(first_ids.iter().all(|id| !last_ids.contains(id)));

    page["offset"] = json!(40);
    let past_end = invoke(page.clone());
    assert_eq!(past_end["ok"], true, "{past_end}");
    assert!(past_end["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .is_empty());
    page["offset"] = json!(0);
    let repeated = invoke(page.clone());
    assert_eq!(repeated["data"]["result"]["items"], first_result["items"]);
    assert_eq!(fs::read(&ctf).unwrap(), original_source);
    assert_eq!(fs::read(&notes).unwrap(), original_notes);
    assert_eq!(fs::read(&mirror).unwrap(), original_mirror);

    // A hash-changing source edit must fail the *next page* closed.
    fixture(&ctf, "new external change during paged review");
    page["offset"] = json!(20);
    let stale = invoke(page);
    assert_eq!(stale["ok"], false, "{stale}");
    assert_eq!(fs::read(&notes).unwrap(), original_notes);
}

#[test]
fn explicit_native_candidate_choice_is_cas_guarded_and_not_activated() {
    let temp = tempfile::tempdir().unwrap();
    let ctf = temp.path().join("synthetic.ctf");
    let cache = temp.path().join("derived");
    let notes = temp.path().join("my-notes.wia.sqlite");
    fixture(&ctf, "original generation");
    let original = sync(&ctf, &cache);
    let volumes = invoke(json!({"op":"ctf_volumes_pinned","path":ctf,
        "cache_root":cache,"generation":original}));
    assert_eq!(volumes["ok"], true, "{volumes}");
    let disk = volumes["data"]["result"][0]["id"].as_i64().unwrap();
    let entries = invoke(json!({"op":"ctf_children_pinned","path":ctf,
        "cache_root":cache,"generation":original,"volume_id":disk}));
    assert_eq!(entries["ok"], true, "{entries}");
    let old_id = entries["data"]["result"]["items"][0]["id"]
        .as_i64()
        .unwrap();
    let mut create = pinned(
        &ctf,
        &cache,
        &original,
        &notes,
        "ctf_annotations_set_pinned",
    );
    create["entry_id"] = json!(old_id);
    create["note"] = json!("synthetic local note");
    create["tags"] = json!("synthetic");
    create["expected_revision"] = json!(0);
    assert_eq!(invoke(create)["ok"], true);

    fixture(&ctf, "modified generation");
    let current = sync(&ctf, &cache);
    let source_before = fs::read(&ctf).unwrap();
    let mirror = cache.join(&current).join("catalog.sqlite");
    let mirror_before = fs::read(&mirror).unwrap();
    let mut preview = pinned(
        &ctf,
        &cache,
        &current,
        &notes,
        "ctf_annotations_preview_pinned",
    );
    preview["limit"] = json!(20);
    preview["offset"] = json!(0);
    let report = invoke(preview.clone());
    assert_eq!(report["ok"], true, "{report}");
    let result = &report["data"]["result"];
    assert_eq!(result["state"], "generation_conflict");
    let row = &result["items"][0];
    assert_eq!(row["candidate_state"], "unique_unconfirmed");
    assert_eq!(row["requires_confirmation"], true);
    let destination = row["candidates"][0]["entry_id"].as_i64().unwrap();
    assert_ne!(destination, 0);
    let mut confirm = pinned(
        &ctf,
        &cache,
        &current,
        &notes,
        "ctf_annotations_confirm_pinned",
    );
    confirm["old_entry_id"] = row["old_entry_id"].clone();
    confirm["new_entry_id"] = json!(destination);
    confirm["expected_revision"] = row["revision"].clone();
    confirm["expected_document_id"] = result["document_id"].clone();
    confirm["confirmation"] = json!("user_selected_exact_candidate");

    let mut wrong_document = confirm.clone();
    wrong_document["expected_document_id"] = json!("stale");
    assert_eq!(
        invoke(wrong_document)["error"]["code"],
        "annotation_document_conflict"
    );
    let mut wrong_revision = confirm.clone();
    wrong_revision["expected_revision"] = json!(999);
    assert_eq!(
        invoke(wrong_revision)["error"]["code"],
        "annotation_revision_conflict"
    );
    let mut wrong_candidate = confirm.clone();
    wrong_candidate["new_entry_id"] = json!(999999);
    assert_eq!(
        invoke(wrong_candidate)["error"]["code"],
        "annotation_candidate_conflict"
    );
    let mut no_consent = confirm.clone();
    no_consent["confirmation"] = json!("none");
    assert_eq!(invoke(no_consent)["error"]["code"], "confirmation_required");

    let recorded = invoke(confirm.clone());
    assert_eq!(recorded["ok"], true, "{recorded}");
    assert_eq!(
        recorded["data"]["result"]["state"],
        "confirmed_not_activated"
    );
    assert_eq!(
        recorded["data"]["result"]["annotation_rows_migrated"],
        false
    );
    let repeat = invoke(confirm.clone());
    assert_eq!(repeat["error"]["code"], "annotation_confirmation_conflict");
    let reloaded = invoke(preview);
    assert_eq!(reloaded["ok"], true, "{reloaded}");
    let reloaded_row = &reloaded["data"]["result"]["items"][0];
    assert_eq!(
        reloaded_row["confirmed_selection"]["new_entry_id"],
        destination
    );
    assert_eq!(reloaded_row["requires_confirmation"], false);
    assert_eq!(fs::read(&ctf).unwrap(), source_before);
    assert_eq!(fs::read(&mirror).unwrap(), mirror_before);

    fixture(&ctf, "third generation - stale selection");
    let drift = invoke(confirm);
    assert_eq!(drift["ok"], false, "{drift}");
    assert_eq!(drift["error"]["code"], "source_changed");
}
