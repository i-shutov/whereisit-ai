use super::*;
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

fn install_empty_page_handler(app: &mut DesktopApp) -> Arc<Mutex<Vec<Value>>> {
    let requests = Arc::new(Mutex::new(Vec::new()));
    app.bus.set_handler({
        let requests = requests.clone();
        move |request| {
            requests.lock().unwrap().push(request.clone());
            json!({"ok":true,"data":{"items":[],"offset":0,"limit":250,"has_more":false}})
        }
    });
    requests
}

fn settle_page(app: &mut DesktopApp, ctx: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.active_request.is_some() {
        if let Some(reply) = app.bus.poll() {
            app.apply_reply(reply, ctx);
        } else {
            assert!(
                Instant::now() < deadline,
                "navigation request did not finish"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }
}

fn volume(id: i64) -> Volume {
    Volume {
        id,
        label: format!("Synthetic disk {id}"),
        root_path: String::new(),
        entry_count: 1,
        total_bytes: 0,
    }
}

fn tree_directory(id: i64, parent_id: Option<i64>, name: String) -> Entry {
    Entry {
        id,
        volume_id: 30,
        parent_id,
        name: name.clone(),
        relative_path: name,
        kind: "directory".into(),
        size: 0,
        modified_unix: None,
        description: String::new(),
        tags: String::new(),
    }
}

fn render_tree_once(app: &mut DesktopApp, ctx: &egui::Context, time: f64) -> (f32, f32) {
    let mut result = None;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(640.0, 360.0),
        )),
        time: Some(time),
        ..Default::default()
    };
    let mut output = ctx.run_pass(input, |ctx| {
        egui::Area::new(egui::Id::unique("late-ancestor-tree-area")).show(ctx, |ui| {
            let scroll = egui::ScrollArea::vertical()
                .id_salt("late-ancestor-tree-regression")
                .max_height(120.0)
                .show(ui, |ui| app.draw_tree_children(ui, (30, Some(70)), 2, ctx));
            result = Some((scroll.content_size.y, scroll.state.clamped_offset().y));
        });
    });
    output.textures_delta.clear();
    result.expect("tree ScrollArea should be rendered")
}

#[test]
fn initial_overview_is_not_a_global_empty_search() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    let requests = install_empty_page_handler(&mut app);

    assert_eq!(app.route, Route::Catalog);
    assert!(app.query_input.is_empty());
    app.load_page(&ctx);

    assert!(app.active_request.is_none());
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn choosing_a_disk_opens_its_root_folder() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    app.db = Some("synthetic.sqlite".into());
    app.volumes = vec![volume(30)];
    app.query_input = "stale query".into();
    let requests = install_empty_page_handler(&mut app);

    app.navigate_disk(Some(30), &ctx);
    settle_page(&mut app, &ctx);

    assert_eq!(
        app.route,
        Route::Folder {
            volume_id: 30,
            parent_id: None
        }
    );
    assert_eq!(app.selected_volume, Some(30));
    assert!(app.query_input.is_empty());
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["op"], "children");
    assert_eq!(requests[0]["volume_id"], 30);
    assert!(requests[0]["parent_id"].is_null());
}

#[test]
fn clearing_a_branch_search_returns_to_the_remembered_folder() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    app.db = Some("synthetic.sqlite".into());
    app.volumes = vec![volume(30)];
    let expected_crumbs = vec![
        BrowseCrumb {
            id: 70,
            parent_id: None,
            name: "Кино".into(),
            kind: "directory".into(),
        },
        BrowseCrumb {
            id: 9464,
            parent_id: Some(70),
            name: "Сериалы".into(),
            kind: "directory".into(),
        },
    ];
    app.folder_crumbs = expected_crumbs.clone();
    app.route = Route::Folder {
        volume_id: 30,
        parent_id: Some(9464),
    };
    app.query_input = "episode".into();
    let requests = install_empty_page_handler(&mut app);

    app.submit_search(&ctx);
    settle_page(&mut app, &ctx);
    assert_eq!(
        app.route,
        Route::BranchSearch {
            volume_id: 30,
            parent_id: 9464,
            query: "episode".into(),
            exact_category_id: None,
        }
    );
    assert_eq!(app.folder_crumbs, expected_crumbs);

    app.query_input.clear();
    app.submit_search(&ctx);
    settle_page(&mut app, &ctx);

    assert_eq!(
        app.route,
        Route::Folder {
            volume_id: 30,
            parent_id: Some(9464)
        }
    );
    assert_eq!(app.folder_crumbs, expected_crumbs);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["op"], "search_fast");
    assert_eq!(requests[1]["op"], "children");
    assert_eq!(requests[1]["volume_id"], 30);
    assert_eq!(requests[1]["parent_id"], 9464);
}

#[test]
fn stale_ancestor_reply_from_an_old_catalog_is_ignored() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    app.db = Some("synthetic.sqlite".into());
    app.catalog_generation = 9;
    app.route = Route::Folder {
        volume_id: 30,
        parent_id: Some(9464),
    };
    app.tree_ancestors_request_id = Some(77);
    app.tree_ancestors_target = Some((30, 9464));
    app.error = None;
    let reply = crate::bridge::Reply {
        id: 77,
        purpose: Purpose::TreeAncestors {
            db: "synthetic.sqlite".into(),
            catalog_generation: 8,
            linked_generation: None,
            volume_id: 30,
            entry_id: 9464,
        },
        response: json!({"ok":true,"data":{"volume_id":30,"items":[{
            "id":70,"parent_id":null,"name":"Кино","kind":"directory"
        },{
            "id":9464,"parent_id":70,"name":"Сериалы","kind":"directory"
        }]}}),
    };

    app.apply_tree_reply(reply, &ctx);

    assert!(app.folder_crumbs.is_empty());
    assert!(app.tree_path_nodes.is_empty());
    assert_eq!(app.tree_ancestors_request_id, Some(77));
    assert_eq!(app.tree_ancestors_target, Some((30, 9464)));
    assert!(app.error.is_none());
}

#[test]
fn late_ancestor_reveal_survives_loading_sibling_pages_and_scrolls_after_reflow() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    let parent_key = (30, Some(70));
    let late = BrowseCrumb {
        id: 9_999,
        parent_id: Some(70),
        name: "zz-late-folder".into(),
        kind: "directory".into(),
    };
    app.tree_children.insert(
        parent_key,
        TreeChildren {
            loading_request: Some(41),
            ..TreeChildren::default()
        },
    );
    app.tree_path_nodes.insert(parent_key, vec![late.clone()]);
    app.tree_reveal_target = Some((30, Some(late.id)));

    let first = render_tree_once(&mut app, &ctx, 1.0);
    assert_eq!(app.tree_reveal_target, Some((30, Some(late.id))));
    assert_eq!(app.tree_path_nodes[&parent_key], vec![late.clone()]);

    let page = app.tree_children.get_mut(&parent_key).unwrap();
    page.items = (0..250)
        .map(|index| tree_directory(1000 + index, Some(70), format!("branch-{index:03}")))
        .collect();
    page.next_offset = 250;
    page.has_more = true;
    page.loading_request = None;

    let mut final_frame = first;
    for frame in 0..8 {
        final_frame = render_tree_once(&mut app, &ctx, 1.1 + frame as f64 * 0.1);
    }

    assert_eq!(app.tree_reveal_target, None);
    assert_eq!(app.tree_path_nodes[&parent_key], vec![late]);
    assert!(
        final_frame.0 > 120.0,
        "the 250 siblings and pinned late row must reflow the tree"
    );
    assert!(
        final_frame.1 > 0.0,
        "the reflowed tree should scroll to the pinned late row"
    );
}

#[test]
fn empty_map_jumps_browse_known_scopes_and_preserve_explicit_category_filters() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    app.db = Some("synthetic.sqlite".into());
    app.volumes = vec![volume(30)];
    let requests = install_empty_page_handler(&mut app);
    app.jump_analytics_to_search(&ctx, Some(30), None, "");
    settle_page(&mut app, &ctx);
    assert_eq!(
        app.route,
        Route::Folder {
            volume_id: 30,
            parent_id: None
        }
    );
    assert_eq!(requests.lock().unwrap()[0]["op"], "children");
    app.jump_analytics_to_search(&ctx, None, None, "");
    assert_eq!(app.route, Route::Catalog);
    assert_eq!(requests.lock().unwrap().len(), 1);

    app.workspace_tab = WorkspaceTab::VolumeMap;
    app.analytics_category_id = Some(7);
    app.jump_analytics_to_search(&ctx, Some(30), Some(9464), "");
    assert_eq!(
        app.route,
        Route::BranchSearch {
            volume_id: 30,
            parent_id: 9464,
            query: String::new(),
            exact_category_id: Some(7),
        }
    );
}
