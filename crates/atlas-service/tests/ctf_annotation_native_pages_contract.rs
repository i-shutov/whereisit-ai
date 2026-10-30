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
