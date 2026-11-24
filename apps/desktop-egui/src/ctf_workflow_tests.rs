use super::*;
use std::{
    fs,
    sync::{Arc, Mutex},
    thread,
};

fn dispatch(request: &Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&request.to_string())).unwrap()
}

fn successful(request: Value) -> Value {
    let response = dispatch(&request);
    assert_eq!(response["ok"], true, "{request}: {response}");
    response["data"].clone()
}

fn seed_catalog(dir: &Path) -> (std::path::PathBuf, i64) {
    let db = dir.join("seed.sqlite");
    let media = dir.join("disk-a").join("Кино");
    fs::create_dir_all(&media).unwrap();
    fs::write(media.join("Фильм.mkv"), b"generated media metadata fixture").unwrap();
    let scan = successful(json!({"op":"scan","db":db,"root":media.parent().unwrap(),
        "label":"Диск A","hash":false}));
    let found = successful(json!({"op":"search","db":db,"query":"ext:mkv"}));
    let id = found["items"][0]["id"].as_i64().unwrap();
    assert_eq!(found["items"][0]["volume_id"], scan["id"]);
    successful(json!({"op":"update_entry","db":db,"entry_id":id,
        "description":"Original","tags":[]}));
    (db, id)
}

fn app(ctx: &egui::Context, dir: &Path) -> DesktopApp {
    let mut app = DesktopApp::with_context(ctx, None);
    let user_store = dir.join("fixture-user-store.sqlite");
    app.bus.set_handler(move |request| {
        let mut request = request.clone();
        // Send the real catalog operations to the service without reaching
        // outside the generated test directory for any user-sidecar state.
        if request.get("user_store").is_some() {
            request["user_store"] = json!(user_store);
        }
        if request["op"] == "physical_probe_os" {
            return json!({"ok":true,"data":{"platform":preference_platform(),
                "probe_ok":false,"observed_ids":[],"items":[]}});
        }
        dispatch(&request)
    });
    app
}

fn settle(app: &mut DesktopApp, ctx: &egui::Context) {
    while app.active_request.is_some() {
        let reply = next_reply(app);
        app.apply_reply(reply, ctx);
    }
}

fn next_reply(app: &DesktopApp) -> crate::bridge::Reply {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(reply) = app.bus.poll() {
            return reply;
        } else {
            assert!(Instant::now() < deadline, "Catalog worker did not reply");
            thread::sleep(Duration::from_millis(1));
        }
    }
}

static CTF_PREVIEW_TEST_LOCK: Mutex<()> = Mutex::new(());

fn link(app: &mut DesktopApp, ctx: &egui::Context, source: &Path) {
    let path = source.to_str().unwrap().to_owned();
    let cache_root = source
        .with_file_name("fixture-cache")
        .to_str()
        .unwrap()
        .to_owned();
    app.active_request = Some(app.bus.send(
        Purpose::CtfPreviewOpen {
            path: path.clone(),
            cache_root,
        },
        json!({"op":"ctf_preview_open","path":path}),
        ctx,
    ));
    settle(app, ctx);
    assert_eq!(
        app.integrity,
        CatalogIntegrity::LinkedReadOnly,
        "{:?}",
        app.error
    );
    assert!(app.ctf_job_id.is_none());
    assert_eq!(app.volumes[0].label, "Диск A");
}

fn start_import(app: &mut DesktopApp, ctx: &egui::Context, source: &Path, target: &Path) {
    app.ctf_dialog = Some(CtfDialog::Import);
    app.ctf_source_input = source.to_str().unwrap().to_owned();
    app.ctf_target_input = target.to_str().unwrap().to_owned();
    app.start_ctf_transfer(ctx);
}

#[test]
fn ctf_open_and_search_use_direct_read_only_preview_without_sync() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("snapshot.ctf");
    fs::write(&source, b"synthetic CTF source identity fixture").unwrap();
    let path = source.to_string_lossy().into_owned();
    let cache_root = temp.path().join("ctf-cache").to_string_lossy().into_owned();
    let generation = "c".repeat(64);
    let source_sha = "b".repeat(64);
    let operations = Arc::new(Mutex::new(Vec::<Value>::new()));
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    app.bus.set_handler({
        let operations = operations.clone();
        let generation = generation.clone();
        let source_sha = source_sha.clone();
        move |request| {
            operations.lock().unwrap().push(request.clone());
            match request["op"].as_str().unwrap_or("") {
                "ctf_preview_open" => json!({"ok":true,"data":{
                    "state":"browsable","generation":generation,"source_sha256":source_sha,
                    "source_bytes":38,"entries":2,"volumes":1,
                    "original_ctf_read_only":true,"native_annotation_writes_allowed":false
                }}),
                "ctf_preview_volumes" => json!({"ok":true,"data":{
                    "generation":request["generation"],"source_sha256":source_sha,"read_only":true,
                    "original_ctf_read_only":true,"annotations_supported":false,
                    "result":[{"id":4,"label":"Disk A","root_path":"","entry_count":2,"total_bytes":99}]
                }}),
                "ctf_preview_children" => json!({"ok":true,"data":{
                    "generation":request["generation"],"source_sha256":source_sha,"read_only":true,
                    "result":{"items":[{"id":71,"volume_id":4,"parent_id":null,"name":"film.mkv",
                        "relative_path":"film.mkv","kind":"file","size":99,"modified_unix":null,
                        "description":"","tags":""}],"total":1,"offset":request["offset"],"limit":250}
                }}),
                "ctf_preview_search" => json!({"ok":true,"data":{
                    "generation":request["generation"],"source_sha256":source_sha,"read_only":true,
                    "result":{"items":[{"id":71,"volume_id":4,"parent_id":null,"name":"film.mkv",
                        "relative_path":"film.mkv","kind":"file","size":99,"modified_unix":null,
                        "description":"","tags":""}],"offset":request["offset"],"limit":request["limit"],"has_more":false}
                }}),
                "ctf_preview_close" => json!({"ok":true,"data":{"closed":true,"generation":request["generation"]}}),
                other => json!({"ok":false,"error":{"code":"unexpected_test_operation","message":other}}),
            }
        }
    });

    app.active_request = Some(app.bus.send(
        Purpose::CtfPreviewOpen {
            path: path.clone(),
            cache_root,
        },
        json!({"op":"ctf_preview_open","path":path}),
        &ctx,
    ));
    settle(&mut app, &ctx);
    assert!(
        app.error.is_none(),
        "open and initial page: {:?}",
        app.error
    );
    assert!(app.linked_ctf.as_ref().is_some_and(|link| link.preview));
    assert_eq!(
        app.route,
        Route::Search {
            query: String::new(),
            volume_id: None
        }
    );
    assert_eq!(app.page.items[0].name, "film.mkv");
    assert!(
        app.ctf_job_id.is_none(),
        "opening does not start indexing or a long job"
    );

    let query = "ext:mkv AND name:film".to_owned();
    app.route = Route::Search {
        query: query.clone(),
        volume_id: Some(4),
    };
    app.query_input = query.clone();
    app.offset = 0;
    app.load_page(&ctx);
    settle(&mut app, &ctx);
    assert!(app.error.is_none(), "direct query: {:?}", app.error);
    assert_eq!(app.page.items[0].name, "film.mkv");
    assert!(
        app.ctf_job_id.is_none(),
        "finished direct search releases its cancellable job"
    );
    let ops = operations.lock().unwrap();
    assert!(!ops.iter().any(|request| request["op"] == "ctf_sync"));
    let search = ops
        .iter()
        .find(|request| request["op"] == "ctf_preview_search" && request["query"] == query)
        .unwrap();
    assert_eq!(search["path"], path);
    assert_eq!(search["generation"], generation);
    assert_eq!(search["query"], query);
    assert_eq!(search["volume_id"], 4);
    assert_eq!(search["offset"], 0);
    assert_eq!(search["limit"], 250);
    assert!(search["job_id"].as_str().is_some_and(|id| !id.is_empty()));
}

#[test]
fn linked_ctf_browse_import_edit_and_export_through_the_service() {
    let _preview_lock = CTF_PREVIEW_TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let (seed, _) = seed_catalog(temp.path());
    let source = temp.path().join("Исходник.CTF");
    successful(json!({"op":"export_ctf","db":seed,"path":source}));
    let original = fs::read(&source).unwrap();
    let ctx = egui::Context::default();
    let mut app = app(&ctx, temp.path());
    link(&mut app, &ctx, &source);

    let folder = app
        .page
        .items
        .iter()
        .find(|row| row.is_directory())
        .unwrap()
        .clone();
    app.navigate_folder(&folder, &ctx);
    settle(&mut app, &ctx);
    let file = app.page.items.first().unwrap().clone();
    assert_eq!(file.relative_path, "Кино/Фильм.mkv");
    assert_eq!(file.description, "Original");
    assert_eq!(file.parent_id, Some(folder.id));

    app.go_back(&ctx);
    settle(&mut app, &ctx);
    app.query_input = "ext:mkv AND name:фильм".into();
    app.navigate_search(&ctx);
    settle(&mut app, &ctx);
    assert_eq!(app.page.items, vec![file.clone()]);
    app.navigate_containing_folder(&file, &ctx);
    settle(&mut app, &ctx);
    assert!(app.error.is_none(), "{:?}", app.error);
    assert_eq!(
        app.route,
        Route::Folder {
            volume_id: file.volume_id,
            parent_id: Some(folder.id)
        }
    );
    assert_eq!(app.selected.as_ref().map(|row| row.id), Some(file.id));

    app.description_entry_id = Some(file.id);
    app.description_input = "Read-only source".into();
    app.save_description(&ctx);
    assert!(app.active_request.is_none());
    assert_eq!(fs::read(&source).unwrap(), original);

    let working = temp.path().join("Копия.sqlite");
    start_import(&mut app, &ctx, &source, &working);
    settle(&mut app, &ctx);
    assert_eq!(app.db.as_deref(), working.to_str(), "{:?}", app.error);
    assert!(app.linked_ctf.is_none());
    let row = app
        .page
        .items
        .iter()
        .find(|row| row.name == "Фильм.mkv")
        .unwrap();
    assert_eq!(row.description, "Original");
    app.description_entry_id = Some(row.id);
    app.description_input = "Новое описание 🎵".into();
    app.save_description(&ctx);
    settle(&mut app, &ctx);
    assert!(app.description_entry_id.is_none(), "{:?}", app.error);
    assert!(app
        .page
        .items
        .iter()
        .any(|row| row.description == "Новое описание 🎵"));

    let output = temp.path().join("Результат.CTF");
    app.ctf_dialog = Some(CtfDialog::Export);
    app.ctf_target_input = output.to_str().unwrap().to_owned();
    app.start_ctf_transfer(&ctx);
    settle(&mut app, &ctx);
    assert!(app.error.is_none(), "{:?}", app.error);
    let exported = fs::read(&output).unwrap();
    let readback = temp.path().join("readback.sqlite");
    successful(json!({"op":"import_ctf_new","path":output,"db":readback}));
    let found = successful(json!({"op":"search","db":readback,"query":"ext:mkv"}));
    assert_eq!(found["items"][0]["relative_path"], "Кино/Фильм.mkv");
    assert_eq!(found["items"][0]["description"], "Новое описание 🎵");

    app.ctf_dialog = Some(CtfDialog::Export);
    app.start_ctf_transfer(&ctx);
    assert!(app.active_request.is_none());
    assert!(app.error.is_some());
    assert_eq!(fs::read(&output).unwrap(), exported);

    let raced_output = temp.path().join("raced.CTF");
    let competing_path = raced_output.clone();
    app.bus.set_handler(move |request| {
        if request["op"] == "export_ctf" {
            use std::io::Write;
            assert_eq!(request["path"], json!(competing_path));
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&competing_path)
                .unwrap()
                .write_all(b"competing file")
                .unwrap();
        }
        dispatch(request)
    });
    app.ctf_target_input = raced_output.to_str().unwrap().to_owned();
    app.start_ctf_transfer(&ctx);
    settle(&mut app, &ctx);
    assert!(app.error.is_some());
    assert!(app.ctf_job_id.is_none());
    assert_eq!(fs::read(raced_output).unwrap(), b"competing file");
    assert_eq!(fs::read(&source).unwrap(), original);
}

#[test]
fn changed_linked_source_refuses_import_and_hides_stale_rows() {
    let _preview_lock = CTF_PREVIEW_TEST_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let (seed, id) = seed_catalog(temp.path());
    let source = temp.path().join("original.CTF");
    successful(json!({"op":"export_ctf","db":seed,"path":source}));
    let original = fs::read(&source).unwrap();
    successful(json!({"op":"update_entry","db":seed,"entry_id":id,
        "description":"External","tags":[]}));
    let replacement = temp.path().join("replacement.CTF");
    successful(json!({"op":"export_ctf","db":seed,"path":replacement}));
    let modified = fs::read(replacement).unwrap();
    assert_eq!(modified.len(), original.len());
    assert_ne!(modified, original);

    let ctx = egui::Context::default();
    let mut app = app(&ctx, temp.path());
    link(&mut app, &ctx, &source);
    assert!(app
        .page
        .items
        .iter()
        .any(|row| row.description == "Original"));

    app.query_input = "ext:mkv AND (".into();
    app.navigate_search(&ctx);
    settle(&mut app, &ctx);
    assert!(app.error.is_some());
    assert!(
        app.linked_ctf.is_some(),
        "A query error should preserve the source"
    );
    app.query_input = "ext:mkv".into();
    app.navigate_search(&ctx);
    settle(&mut app, &ctx);
    assert_eq!(app.page.items[0].description, "Original");

    // Simulate a same-size external save to the generated source fixture.
    fs::write(&source, &modified).unwrap();
    let working = temp.path().join("should-not-publish.sqlite");
    let source_alias = temp.path().join(".").join(source.file_name().unwrap());
    start_import(&mut app, &ctx, &source_alias, &working);
    settle(&mut app, &ctx);
    assert!(app.error.is_some());
    assert!(app.linked_ctf.is_some());
    assert!(!working.exists());
    app.load_page(&ctx);
    settle(&mut app, &ctx);
    assert!(app.error.is_some());
    assert!(app.linked_ctf.is_none());
    assert!(app.selected.is_none());
    assert!(app.page.items.is_empty());
    assert_eq!(fs::read(&source).unwrap(), modified);
}

#[test]
fn ctf_progress_and_cancel_leave_the_import_in_charge_until_its_reply() {
    let temp = tempfile::tempdir().unwrap();
    let (seed, _) = seed_catalog(temp.path());
    let source = temp.path().join("cancel.CTF");
    successful(json!({"op":"export_ctf","db":seed,"path":source}));
    let original = fs::read(&source).unwrap();
    let working = temp.path().join("cancelled.sqlite");
    let ctx = egui::Context::default();
    let mut app = app(&ctx, temp.path());
    app.lang = Language::English;

    // Hold the long worker until after the UI handles both auxiliary replies.
    let (finish, wait) = std::sync::mpsc::channel();
    let wait = std::sync::Mutex::new(wait);
    app.bus
        .set_handler(move |request| match request["op"].as_str() {
            Some("import_ctf_new") => {
                wait.lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
                json!({"ok":false,"error":{"code":"cancelled","message":"Cancelled"}})
            }
            Some("ctf_progress") => json!({"ok":true,"data":{"phase":"importing","entries":42}}),
            Some("ctf_cancel") => json!({"ok":true,"data":{}}),
            _ => panic!("Unexpected request {request}"),
        });
    start_import(&mut app, &ctx, &source, &working);
    let active_import = app.active_request.unwrap();
    let job_id = app.ctf_job_id.clone().unwrap();
    app.poll_ctf_progress(&ctx);
    let progress_request = app.ctf_progress_request.unwrap();
    assert_ne!(progress_request, active_import);

    app.apply_reply(
        crate::bridge::Reply {
            id: progress_request,
            purpose: Purpose::CtfProgress {
                job_id: "stale-job".into(),
            },
            response: json!({"ok":true,"data":{"phase":"publishing","entries":999}}),
        },
        &ctx,
    );
    assert_eq!(app.active_request, Some(active_import));
    assert_eq!(app.ctf_progress_request, Some(progress_request));
    assert_eq!(app.ctf_entries, 0);
    let progress = next_reply(&app);
    app.apply_reply(progress, &ctx);
    assert_eq!(app.active_request, Some(active_import));
    assert_eq!(app.ctf_job_id.as_deref(), Some(job_id.as_str()));
    assert_eq!(app.ctf_phase, "importing");
    assert_eq!(app.ctf_entries, 42);
    assert!(app.ctf_job_registered);

    app.cancel_ctf_job(&ctx);
    let cancel = next_reply(&app);
    app.apply_reply(cancel, &ctx);
    assert_eq!(app.active_request, Some(active_import));
    assert!(app.ctf_cancel_requested);
    assert!(app.ctf_cancel_request.is_none());
    finish.send(()).unwrap();
    settle(&mut app, &ctx);
    assert!(app.ctf_job_id.is_none());
    assert!(app.error.is_none());
    assert_eq!(
        app.status.text(app.lang),
        "CTF operation cancelled; no result was published."
    );
    assert!(!working.exists());
    assert_eq!(fs::read(source).unwrap(), original);
}
