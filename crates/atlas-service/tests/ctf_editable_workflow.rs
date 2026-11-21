use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

fn call(request: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&request.to_string())).unwrap()
}

fn fixture(entries: Vec<CtfEntry>) -> CtfCatalog {
    CtfCatalog {
        name: "Synthetic catalog".into(),
        disks: vec![CtfDisk {
            name: "Диск".into(),
            entries,
            opaque_header: Vec::new(),
        }],
        opaque_trailer: Vec::new(),
    }
}

fn file(name: &str) -> CtfEntry {
    CtfEntry {
        name: name.into(),
        description: String::new(),
        parent: None,
        kind: EntryKind::File,
        size: 5_000_000_000,
        attributes: 32,
        modified_dos: 0,
        created_dos: 0,
        accessed_dos: 0,
        opaque: Vec::new(),
    }
}

fn parse(bytes: &[u8]) -> CtfCatalog {
    ctf_format::parse_ctf(bytes, &Limits::default()).unwrap()
}

fn find(catalog: &CtfCatalog, name: &str) -> CtfEntry {
    catalog.disks[0]
        .entries
        .iter()
        .find(|entry| entry.name == name)
        .unwrap()
        .clone()
}

fn entry_id(db: &Path, query: &str) -> i64 {
    let found = call(json!({"op":"search","db":db,"query":query}));
    assert_eq!(found["ok"], true, "{found}");
    found["data"]["items"][0]["id"].as_i64().unwrap()
}

#[test]
fn linked_import_edit_export_preserves_shared_blob_and_unknown_record() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("Исходник.CTF");
    let db = temp.path().join("Копия.sqlite");
    let output = temp.path().join("Изменения.CTF");
    let cache = temp.path().join("cache");
    let seed = fixture(vec![
        file("Цель.mkv"),
        file("Контроль.flac"),
        file("Opaque.avi"),
    ]);
    let mut catalog = parse(&ctf_format::write_ctf(&seed).unwrap());
    let disk = &mut catalog.disks[0];
    let root_blob = b"\xffShared\0\xfe";
    disk.opaque_header[4..8].copy_from_slice(&(root_blob.len() as u32).to_le_bytes());
    disk.opaque_header.extend(root_blob);
    for entry in &mut disk.entries {
        entry.opaque[0] |= 0x80;
        if entry.name == "Opaque.avi" {
            entry.opaque.extend([1, 3, 0, 0, 0x10, 0xfe, 0x11, 0xff, 0]);
        } else {
            entry.description = "Shared".into();
            entry.opaque.extend([1, 7, 0, 0, 8, 1, 0, 0, 0, 0]);
        }
    }
    let original = ctf_format::write_ctf(&catalog).unwrap();
    fs::write(&source, &original).unwrap();

    let sync =
        call(json!({"op":"ctf_sync","path":source,"cache_root":cache,"job_id":"fixture-sync"}));
    assert_eq!(sync["ok"], true, "{sync}");
    let source_sha = &sync["data"]["source_sha256"];
    let linked = call(json!({
        "op":"ctf_search_pinned","path":source,"cache_root":cache,
        "generation":sync["data"]["generation"],"query":"ext:mkv"
    }));
    assert_eq!(
        linked["data"]["result"]["items"][0]["description"],
        "Shared"
    );
    let import = json!({"op":"import_ctf_new","path":source,"db":db,
        "expected_source_sha256":source_sha,"job_id":"fixture-import"});
    let copied = call(import.clone());
    assert_eq!(copied["ok"], true, "{copied}");
    assert_eq!(copied["data"]["entries"], 3);
    assert_eq!(copied["data"]["sha256"], *source_sha);
    assert_eq!(call(import)["error"]["code"], "file_exists");
    assert!(!db.with_file_name("Копия.sqlite-wal").exists());

    let target = entry_id(&db, "ext:mkv");
    let unknown = entry_id(&db, "ext:avi");
    let refusal = call(json!({"op":"update_ctf_description","db":db,
        "entry_id":unknown,"description":"Should refuse"}));
    assert_eq!(refusal["error"]["code"], "ctf_unsupported", "{refusal}");
    let edit = call(json!({"op":"update_ctf_description","db":db,
        "entry_id":target,"description":"Изменено 🎵"}));
    assert_eq!(edit["ok"], true, "{edit}");
    let updated = call(json!({"op":"search","db":db,"query":"description:Изменено"}));
    assert_eq!(updated["data"]["items"][0]["id"], target, "{updated}");
    let export = json!({"op":"export_ctf","db":db,"path":output,
        "overwrite":false,"allow_lossy":false,"job_id":"fixture-export"});
    let exported = call(export.clone());
    assert_eq!(exported["ok"], true, "{exported}");
    let bytes = fs::read(&output).unwrap();
    let result = parse(&bytes);
    assert_eq!(find(&result, "Цель.mkv").description, "Изменено 🎵");
    assert_eq!(find(&result, "Цель.mkv").size, 5_000_000_000);
    assert_eq!(
        find(&result, "Контроль.flac").opaque,
        find(&catalog, "Контроль.flac").opaque
    );
    assert_eq!(find(&result, "Контроль.flac").description, "Shared");
    assert_eq!(
        find(&result, "Opaque.avi").opaque,
        find(&catalog, "Opaque.avi").opaque
    );
    assert!(result.disks[0].opaque_header.ends_with(root_blob));
    assert_eq!(call(export)["error"]["code"], "file_exists");
    assert_eq!(fs::read(&output).unwrap(), bytes);

    // Older consumers can write directly to the SQLite description using
    // update_entry. The CTF writer must still protect the unresolved field.
    let legacy_edit = call(json!({"op":"update_entry","db":db,
        "entry_id":unknown,"description":"Legacy edit","tags":[]}));
    assert_eq!(legacy_edit["ok"], true, "{legacy_edit}");
    let unsafe_output = temp.path().join("must-not-publish.ctf");
    for allow_lossy in [false, true] {
        let refusal = call(json!({"op":"export_ctf","db":db,
            "path":unsafe_output,"allow_lossy":allow_lossy}));
        assert_eq!(refusal["error"]["code"], "ctf", "{refusal}");
        assert!(refusal["error"]["message"]
            .as_str()
            .unwrap()
            .contains("opaque entry metadata"));
        assert!(!unsafe_output.exists());
    }
    assert_eq!(fs::read(&source).unwrap(), original);
}

#[test]
fn new_import_failures_leave_existing_paths_and_source_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sample.CTF");
    let original = ctf_format::write_ctf(&fixture(vec![file("Sample.txt")])).unwrap();
    fs::write(&source, &original).unwrap();
    let db = temp.path().join("new.sqlite");
    let args = json!({"op":"import_ctf_new","db":db,"path":source});

    let mut stale = args.clone();
    stale["expected_source_sha256"] = json!("0".repeat(64));
    let changed = call(stale);
    assert_eq!(changed["error"]["code"], "source_changed", "{changed}");
    assert!(!db.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);

    let sidecar = temp.path().join("new.sqlite-wal");
    let mut reserved_name = args.clone();
    reserved_name["db"] = json!(sidecar);
    assert_eq!(call(reserved_name)["error"]["code"], "unsafe_output");
    assert!(!sidecar.exists());
    fs::write(&sidecar, b"existing contents").unwrap();
    assert_eq!(call(args.clone())["error"]["code"], "file_exists");
    assert_eq!(fs::read(&sidecar).unwrap(), b"existing contents");
    assert!(!db.exists());
    fs::remove_file(sidecar).unwrap();

    let mut same_target = args.clone();
    same_target["db"] = json!(source);
    assert_eq!(call(same_target)["error"]["code"], "file_exists");
    assert_eq!(fs::read(&source).unwrap(), original);

    let invalid = temp.path().join("invalid.CTF");
    fs::write(&invalid, b"invalid synthetic CTF file").unwrap();
    let mut malformed = args;
    malformed["path"] = json!(invalid);
    assert_eq!(call(malformed)["error"]["code"], "ctf");
    assert!(!db.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
}

#[test]
fn cancelling_an_import_discards_its_staged_database() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("cancel.CTF");
    let db = temp.path().join("cancel.sqlite");
    let entries = (0..30_000)
        .map(|n| file(&format!("sample-{n:05}.txt")))
        .collect();
    let original = ctf_format::write_ctf(&fixture(entries)).unwrap();
    fs::write(&source, &original).unwrap();
    let args = json!({"op":"import_ctf_new","db":db,"path":source,"job_id":"cancel-staged-import"});
    let worker = thread::spawn(move || call(args));
    let deadline = Instant::now() + Duration::from_secs(30);
    let progress = json!({"op":"ctf_progress","job_id":"cancel-staged-import"});
    while Instant::now() < deadline && !worker.is_finished() {
        if call(progress.clone())["data"]["entries"]
            .as_u64()
            .unwrap_or(0)
            > 0
        {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    let cancel = call(json!({"op":"ctf_cancel","job_id":"cancel-staged-import"}));
    let result = worker.join().unwrap();
    assert_eq!(cancel["ok"], true, "cancel: {cancel}; result: {result}");
    assert_eq!(result["error"]["code"], "cancelled", "{result}");
    assert_eq!(call(progress)["error"]["code"], "not_found");
    assert!(!db.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    assert_eq!(fs::read(&source).unwrap(), original);
}

#[test]
fn edited_source_after_import_read_is_rejected_before_publication() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("changing.CTF");
    let db = temp.path().join("discard.sqlite");
    let entries = (0..30_000)
        .map(|n| file(&format!("sample-{n:05}.txt")))
        .collect();
    let mut catalog = fixture(entries);
    let original = ctf_format::write_ctf(&catalog).unwrap();
    catalog.disks[0].entries[0].name = "edited-00000.txt".into();
    let replacement = ctf_format::write_ctf(&catalog).unwrap();
    assert_eq!(original.len(), replacement.len());
    fs::write(&source, original).unwrap();

    let args = json!({"op":"import_ctf_new","db":db,"path":source,"job_id":"changing-source"});
    let worker = thread::spawn(move || call(args));
    let progress = json!({"op":"ctf_progress","job_id":"changing-source"});
    let deadline = Instant::now() + Duration::from_secs(30);
    while call(progress.clone())["data"]["entries"]
        .as_u64()
        .unwrap_or(0)
        == 0
    {
        assert!(
            !worker.is_finished(),
            "Import ended before the fixture changed"
        );
        assert!(
            Instant::now() < deadline,
            "Import did not reach entry insertion"
        );
        thread::sleep(Duration::from_millis(1));
    }
    // Entry insertion starts only after import has read and parsed its source.
    fs::write(&source, &replacement).unwrap();
    let result = worker.join().unwrap();
    assert_eq!(result["error"]["code"], "source_changed", "{result}");
    assert!(!db.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    assert_eq!(fs::read(source).unwrap(), replacement);
}

#[test]
fn editable_native_description_preserves_tags() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("scanned");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("native.mp3"), b"fixture").unwrap();
    let db = temp.path().join("native.sqlite");
    let scanned = call(json!({"op":"scan","root":root,"db":db,"label":"Native"}));
    assert_eq!(scanned["ok"], true, "{scanned}");
    let id = entry_id(&db, "ext:mp3");
    let tagged = call(json!({"op":"update_entry","db":db,"entry_id":id,
        "description":"old","tags":"existing"}));
    assert_eq!(tagged["ok"], true, "{tagged}");
    let changed = call(json!({"op":"update_ctf_description","db":db,"entry_id":id,
        "description":"Native changes"}));
    assert_eq!(changed["ok"], true, "{changed}");
    let found = call(json!({"op":"search","db":db,"query":"ext:mp3"}));
    assert_eq!(found["data"]["items"][0]["description"], "Native changes");
    assert_eq!(found["data"]["items"][0]["tags"], "existing");
}
