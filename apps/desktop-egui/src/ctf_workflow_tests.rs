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
    while app.active_request.is_some()
        || app.tree_ancestors_request_id.is_some()
        || app
            .tree_children
            .values()
            .any(|page| page.loading_request.is_some())
    {
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
    assert!(!app.volumes.is_empty());
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
    assert!(app.error.is_none(), "open and volumes: {:?}", app.error);
    assert!(app.linked_ctf.as_ref().is_some_and(|link| link.preview));
    assert_eq!(app.route, Route::Catalog);
    assert!(app.page.items.is_empty());
    assert!(!operations
        .lock()
        .unwrap()
        .iter()
        .any(|request| request["op"] == "ctf_preview_search"
            || request["op"] == "ctf_preview_children"));
    assert!(
        app.ctf_job_id.is_none(),
        "opening does not start indexing or a long job"
    );

    app.browse_volume_root(4, &ctx);
    settle(&mut app, &ctx);
    assert_eq!(
        app.route,
        Route::Folder {
            volume_id: 4,
            parent_id: None
        }
    );
    assert_eq!(app.page.items[0].name, "film.mkv");

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
fn linked_ctf_analytics_uses_pinned_preview_and_rejects_stale_generation() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    let source_path = "C:/synthetic/source.ctf".to_owned();
    let generation = "a".repeat(64);
    let source_sha256 = "b".repeat(64);
    app.db = Some(source_path.clone());
    app.linked_ctf = Some(
        LinkedCtf::parse_preview(
            source_path.clone(),
            String::new(),
            &json!({"state":"browsable","generation":generation,
                "source_sha256":source_sha256,"original_ctf_read_only":true,
                "native_annotation_writes_allowed":false}),
        )
        .unwrap(),
    );
    app.analytics_volume = Some(4);
    app.analytics_folder = Some(19);
    app.analytics_group = "extensions".into();
    app.analytics_highlight = "ext:mkv".into();

    let observed = Arc::new(Mutex::new(None::<Value>));
    let observed_worker = observed.clone();
    let response_generation = generation.clone();
    app.bus.set_handler(move |request| {
        *observed_worker.lock().unwrap() = Some(request.clone());
        json!({"ok":true,"data":{"generation":response_generation,
            "source_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "read_only":true,"result":{"root":{"id":"volume-root"},
                "totals":{"files":1},"breadcrumbs":[]}}})
    });
    app.request_analytics(&ctx, false);
    let reply = next_reply(&app);
    app.apply_reply(reply, &ctx);

    let request = observed.lock().unwrap().clone().unwrap();
    assert_eq!(request["op"], "ctf_preview_analytics");
    assert_eq!(request["path"], source_path);
    assert_eq!(request["generation"], generation);
    assert_eq!(request["volume_id"], 4);
    assert_eq!(request["folder_entry_id"], 19);
    assert_eq!(request["group_by"], "extensions");
    assert_eq!(request["highlight_query"], "ext:mkv");
    assert!(request.get("db").is_none());
    assert_eq!(
        app.analytics_data.as_ref().unwrap()["root"]["id"],
        "volume-root"
    );

    let preserved = app.analytics_data.clone().unwrap();
    app.bus.set_handler(move |_| {
        json!({"ok":true,"data":{"generation":"c".repeat(64),
            "source_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "read_only":true,"result":{"root":{"id":"stale"},
                "totals":{"files":1},"breadcrumbs":[]}}})
    });
    app.request_analytics(&ctx, false);
    // Model a newer same-generation reply winning before an older envelope arrives.
    app.analytics_data = Some(preserved.clone());
    let stale_reply = next_reply(&app);
    app.apply_reply(stale_reply, &ctx);
    assert_eq!(
        app.analytics_data.unwrap()["root"]["id"],
        preserved["root"]["id"]
    );
    assert!(app.analytics_error.is_some());
}

#[test]
fn legacy_linked_ctf_analytics_stays_blocked_without_preview_pinning() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    app.db = Some("C:/synthetic/legacy-source.ctf".into());
    app.linked_ctf = Some(
        LinkedCtf::parse_sync(
            "C:/synthetic/legacy-source.ctf".into(),
            "C:/synthetic/cache".into(),
            &json!({"state":"ready","generation":"a".repeat(64),
                "source_sha256":"b".repeat(64),"original_ctf_read_only":true,
                "native_annotation_writes_allowed":false}),
        )
        .unwrap(),
    );

    app.request_analytics(&ctx, false);
    app.request_analytics(&ctx, true);
    assert!(app.analytics_loaded);
    assert!(app.slices_loaded);
    assert!(app.analytics_error.is_some());
    assert!(app.slices_error.is_some());
    assert!(app.analytics_job_id.is_none());
    assert!(app.slices_job_id.is_none());
}

#[test]
fn pending_sql_analytics_cancels_sql_registry_after_ctf_link_is_current() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    let job_id = format!("analytics-origin-test-{}", std::process::id());
    atlas_service::prepare_analytics_job(&job_id).unwrap();
    app.analytics_job_id = Some(job_id.clone());
    app.analytics_preview_job = false;
    app.linked_ctf = Some(
        LinkedCtf::parse_preview(
            "C:/synthetic/source.ctf".into(),
            String::new(),
            &json!({"state":"browsable","generation":"a".repeat(64),
                "source_sha256":"b".repeat(64),"original_ctf_read_only":true,
                "native_annotation_writes_allowed":false}),
        )
        .unwrap(),
    );
    let (sent, received) = std::sync::mpsc::channel();
    app.bus.set_handler(move |request| {
        sent.send(request.clone()).unwrap();
        json!({"ok":true,"data":{"cancelled":true}})
    });

    app.cancel_analytics(&ctx, false, true);
    let reply = next_reply(&app);
    let request = received.recv().unwrap();
    assert_eq!(request["op"], "analytics_cancel");
    assert_eq!(request["job_id"], job_id);
    app.apply_reply(reply, &ctx);
    atlas_service::cancel_analytics_job(&job_id).unwrap();
}

#[test]
fn pending_ctf_analytics_cancels_preview_registry_after_link_is_cleared() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    let job_id = format!("ctf-analytics-origin-test-{}", std::process::id());
    let prepared = atlas_service::prepare_ctf_preview_analytics_job(
        &json!({"op":"ctf_preview_analytics","job_id":job_id,
            "path":"synthetic-does-not-open.ctf","generation":"synthetic"})
        .to_string(),
    )
    .unwrap();
    app.analytics_job_id = Some(job_id.clone());
    app.analytics_preview_job = true;
    // Simulate a pending cancellation that arrives after the catalog scope changed.
    app.linked_ctf = None;

    app.cancel_analytics(&ctx, false, true);
    atlas_service::cancel_ctf_preview_analytics_job(&job_id).unwrap();
    drop(prepared);
    assert_eq!(
        atlas_service::cancel_ctf_preview_analytics_job(&job_id)
            .unwrap_err()
            .code,
        "not_found"
    );
}

#[test]
fn focused_file_reveal_keeps_duplicate_disk_scope_and_late_ancestor_visible() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    let path = "C:/synthetic/navigation.ctf".to_owned();
    let generation = "a".repeat(64);
    let source_sha256 = "b".repeat(64);
    app.db = Some(path.clone());
    app.linked_ctf = Some(
        LinkedCtf::parse_preview(
            path,
            String::new(),
            &json!({"state":"browsable","generation":generation,
                "source_sha256":source_sha256,"original_ctf_read_only":true,
                "native_annotation_writes_allowed":false}),
        )
        .unwrap(),
    );
    app.integrity = CatalogIntegrity::LinkedReadOnly;
    app.volumes = vec![
        Volume {
            id: 1,
            label: "Disk 1".into(),
            root_path: String::new(),
            entry_count: 400,
            total_bytes: 0,
        },
        Volume {
            id: 30,
            label: "Disk 30".into(),
            root_path: String::new(),
            entry_count: 400,
            total_bytes: 0,
        },
    ];
    app.route = Route::Search {
        query: "name:same-name.mkv".into(),
        volume_id: None,
    };

    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    app.bus.set_handler({
        let requests = requests.clone();
        move |request| {
            requests.lock().unwrap().push(request.clone());
            let result = match request["op"].as_str().unwrap_or("") {
                "ctf_preview_children_focused" => json!({
                    "items":[{"id":9766,"volume_id":30,"parent_id":9464,"name":"same-name.mkv",
                        "relative_path":"zz-late-folder/same-name.mkv","kind":"file","size":8,
                        "modified_unix":null,"description":"","tags":""}],
                    "total":600,"offset":250,"limit":250
                }),
                "ctf_preview_ancestors" => json!({
                    "volume_id":30,
                    "items":[
                        {"id":9000,"parent_id":null,"name":"Movies","kind":"directory"},
                        {"id":9464,"parent_id":9000,"name":"zz-late-folder","kind":"directory"}
                    ]
                }),
                "ctf_preview_children" => {
                    if request["parent_id"].is_null() {
                        let items = (1000..1250).map(|id| json!({
                            "id":id,"volume_id":30,"parent_id":null,"name":format!("folder-{id}"),
                            "relative_path":format!("folder-{id}"),"kind":"directory","size":0,
                            "modified_unix":null,"description":"","tags":""
                        })).collect::<Vec<_>>();
                        json!({"items":items,"total":321,"offset":0,"limit":250,"has_more":true})
                    } else {
                        json!({"items":[],"total":0,"offset":0,"limit":250,"has_more":false})
                    }
                }
                "ctf_preview_close" => json!({"closed":true}),
                other => panic!("unexpected navigation request: {other}: {request}"),
            };
            json!({"ok":true,"data":{"generation":request["generation"],
                "source_sha256":"b".repeat(64),"read_only":true,"result":result}})
        }
    });

    let selected = Entry::parse(&json!({
        "id":9766,"volume_id":30,"parent_id":9464,"name":"same-name.mkv",
        "relative_path":"zz-late-folder/same-name.mkv","kind":"file","size":8
    }))
    .unwrap();
    app.navigate_containing_folder(&selected, &ctx);

    // The ancestor reply schedules three lazy tree pages. Keep the known
    // ancestor visible even though it falls beyond the first 250 siblings.
    while app.active_request.is_some() {
        let reply = next_reply(&app);
        app.apply_reply(reply, &ctx);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let done = app
            .tree_children
            .values()
            .all(|page| page.loading_request.is_none())
            && app.tree_ancestors_request_id.is_none()
            && requests.lock().unwrap().len() >= 5;
        if done {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Navigation tree workers did not settle"
        );
        if let Some(reply) = app.bus.poll() {
            app.apply_reply(reply, &ctx);
        } else {
            thread::sleep(Duration::from_millis(1));
        }
    }

    assert_eq!(
        app.route,
        Route::Folder {
            volume_id: 30,
            parent_id: Some(9464)
        }
    );
    assert_eq!(app.selected.as_ref().map(|entry| entry.id), Some(9766));
    assert_eq!(app.page.offset, 250);
    assert!(app.page.has_more);
    assert_eq!(app.folder_crumbs.last().map(|crumb| crumb.id), Some(9464));
    assert!(app.tree_path_nodes[&(30, None)]
        .iter()
        .any(|node| node.id == 9000));
    assert!(app.tree_path_nodes[&(30, Some(9000))]
        .iter()
        .any(|node| node.id == 9464));
    assert!(
        !app.tree_path_nodes.contains_key(&(1, None)),
        "same names on another disk must not share tree state"
    );
    assert!(app.tree_children[&(30, None)].has_more);
    assert_eq!(app.tree_children[&(30, None)].items.len(), 250);

    let old_catalog_generation = app.catalog_generation;
    app.clear_catalog();
    app.apply_tree_reply(
        crate::bridge::Reply {
            id: 999_999,
            purpose: Purpose::TreeChildren {
                db: "C:/synthetic/navigation.ctf".into(),
                catalog_generation: old_catalog_generation,
                linked_generation: Some(generation),
                volume_id: 30,
                parent_id: None,
                offset: 0,
            },
            response: json!({"ok":true,"data":{
                "generation":"a".repeat(64),"source_sha256":"b".repeat(64),"read_only":true,
                "result":{"items":[],"total":0,"offset":0,"limit":250,"has_more":false}
            }}),
        },
        &ctx,
    );
    assert_eq!(app.route, Route::Catalog);
    assert!(app.tree_children.is_empty());
}

#[test]
fn linked_ctf_browse_import_edit_and_export_through_the_service() {
    let _preview_lock = CTF_PREVIEW_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let (seed, _) = seed_catalog(temp.path());
    let source = temp.path().join("Исходник.CTF");
    successful(json!({"op":"export_ctf","db":seed,"path":source}));
    let original = fs::read(&source).unwrap();
    let ctx = egui::Context::default();
    let mut app = app(&ctx, temp.path());
    link(&mut app, &ctx, &source);

    // Opening a linked catalog starts on its disk register. Browsing is an
    // explicit disk selection; it does not issue a global empty search.
    assert_eq!(app.route, Route::Catalog);
    app.browse_volume_root(app.volumes[0].id, &ctx);
    settle(&mut app, &ctx);

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

    app.browse_up(&ctx);
    settle(&mut app, &ctx);
    assert_eq!(
        app.route,
        Route::Folder {
            volume_id: folder.volume_id,
            parent_id: folder.parent_id,
        }
    );
    assert!(app.page.items.iter().any(|row| row.id == folder.id));
    app.navigate_folder(&folder, &ctx);
    settle(&mut app, &ctx);

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
    assert_eq!(app.route, Route::Catalog);
    app.browse_volume_root(app.volumes[0].id, &ctx);
    settle(&mut app, &ctx);
    let imported_folder = app
        .page
        .items
        .iter()
        .find(|row| row.name == "Кино" && row.is_container())
        .unwrap()
        .clone();
    app.navigate_folder(&imported_folder, &ctx);
    settle(&mut app, &ctx);
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
    let _preview_lock = CTF_PREVIEW_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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

    // Keep an external writer open before linking so Windows cannot acquire
    // the source pin and must use the full-content hash fallback.
    #[cfg(windows)]
    let _external_writer = std::fs::OpenOptions::new()
        .write(true)
        .open(&source)
        .unwrap();

    let ctx = egui::Context::default();
    let mut app = app(&ctx, temp.path());
    link(&mut app, &ctx, &source);
    assert_eq!(app.route, Route::Catalog);
    assert!(app.page.items.is_empty());

    app.query_input = "ext:mkv".into();
    app.navigate_search(&ctx);
    settle(&mut app, &ctx);
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
fn direct_ctf_copy_candidates_and_exports_keep_scope_and_never_clobber() {
    let _preview_lock = CTF_PREVIEW_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let disk_a = temp.path().join("disk-a");
    let disk_b = temp.path().join("disk-b");
    fs::create_dir_all(&disk_a).unwrap();
    fs::create_dir_all(&disk_b).unwrap();
    fs::write(disk_a.join("film.mkv"), b"same-metadata-size").unwrap();
    fs::write(disk_b.join("film.mkv"), b"same-metadata-size").unwrap();
    let db = temp.path().join("catalog.sqlite");
    successful(json!({"op":"scan","db":db,"root":disk_a,"label":"Disk A","hash":false}));
    successful(json!({"op":"scan","db":db,"root":disk_b,"label":"Disk B","hash":false}));
    let source = temp.path().join("source.CTF");
    successful(json!({"op":"export_ctf","db":db,"path":source}));
    let source_bytes = fs::read(&source).unwrap();

    let ctx = egui::Context::default();
    let mut app = app(&ctx, temp.path());
    link(&mut app, &ctx, &source);
    app.query_input = "ext:mkv".into();
    app.navigate_search(&ctx);
    settle(&mut app, &ctx);
    let original = app
        .page
        .items
        .iter()
        .find(|entry| entry.name == "film.mkv")
        .unwrap()
        .clone();
    app.selected = Some(original.clone());
    app.navigate_copies(&ctx);
    settle(&mut app, &ctx);
    assert!(
        app.error.is_none(),
        "direct copy candidates: {:?}",
        app.error
    );
    assert!(
        !app.page.items.is_empty(),
        "fixture should produce a metadata candidate"
    );
    assert!(app
        .page
        .items
        .iter()
        .all(|candidate| candidate.id != original.id));
    assert!(app
        .page
        .items
        .iter()
        .all(|candidate| candidate.volume_id != original.volume_id));

    let copies_csv = temp.path().join("copy-candidates.csv");
    app.export_path = copies_csv.to_string_lossy().into_owned();
    app.export_kind = ExportKind::Csv;
    app.start_export(&ctx);
    settle(&mut app, &ctx);
    assert!(app.error.is_none(), "copy CSV export: {:?}", app.error);
    let csv_bytes = fs::read(&copies_csv).unwrap();
    let csv = String::from_utf8(csv_bytes.clone()).unwrap();
    assert!(csv.starts_with("id,volume_id,parent_id,name,"));
    assert!(csv.contains("film.mkv"));

    app.route = Route::Search {
        query: "ext:mkv".into(),
        volume_id: None,
    };
    app.query_input = "ext:mkv".into();
    app.offset = 0;
    app.load_page(&ctx);
    settle(&mut app, &ctx);
    assert!(
        app.error.is_none(),
        "search before exports: {:?}",
        app.error
    );

    let results_json = temp.path().join("results.json");
    app.export_path = results_json.to_string_lossy().into_owned();
    app.export_kind = ExportKind::Json;
    app.start_export(&ctx);
    settle(&mut app, &ctx);
    assert!(app.error.is_none(), "JSON export: {:?}", app.error);
    let json_data: Value = serde_json::from_slice(&fs::read(&results_json).unwrap()).unwrap();
    assert_eq!(json_data["entries"].as_array().unwrap().len(), 2);

    let results_parquet = temp.path().join("results.parquet");
    app.export_path = results_parquet.to_string_lossy().into_owned();
    app.export_kind = ExportKind::Parquet;
    app.start_export(&ctx);
    settle(&mut app, &ctx);
    assert!(app.error.is_none(), "Parquet export: {:?}", app.error);
    let parquet = fs::read(&results_parquet).unwrap();
    assert!(parquet.starts_with(b"PAR1") && parquet.ends_with(b"PAR1"));

    let prior_csv = fs::read(&copies_csv).unwrap();
    app.export_path = copies_csv.to_string_lossy().into_owned();
    app.export_kind = ExportKind::Csv;
    app.start_export(&ctx);
    assert!(app.active_request.is_none());
    assert!(
        app.error.is_some(),
        "existing result path must not be overwritten"
    );
    assert_eq!(fs::read(&copies_csv).unwrap(), prior_csv);

    let raced_export = temp.path().join("raced.csv");
    let competing_path = raced_export.clone();
    app.bus.set_handler(move |request| {
        if request["op"] == "ctf_preview_export" {
            use std::io::Write;
            assert_eq!(request["output"], json!(competing_path));
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&competing_path)
                .unwrap()
                .write_all(b"competing output")
                .unwrap();
        }
        dispatch(request)
    });
    app.export_path = raced_export.to_string_lossy().into_owned();
    app.export_kind = ExportKind::Csv;
    app.start_export(&ctx);
    settle(&mut app, &ctx);
    assert!(
        app.error.is_some(),
        "racing output creation must be refused"
    );
    assert_eq!(fs::read(raced_export).unwrap(), b"competing output");
    assert_eq!(
        fs::read(&source).unwrap(),
        source_bytes,
        "preview and exports keep CTF source unchanged"
    );
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
