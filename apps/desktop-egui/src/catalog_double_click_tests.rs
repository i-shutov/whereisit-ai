use super::*;
use serde_json::json;

fn paint_results(
    app: &mut DesktopApp,
    ctx: &egui::Context,
    time: f64,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 700.0),
            )),
            time: Some(time),
            events,
            ..Default::default()
        },
        |root| app.results_table(root, ctx),
    );
    output.textures_delta.clear();
    output.shapes
}

fn pointer_frame(position: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(position),
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        },
    ]
}

fn row_name_center(shapes: &[egui::epaint::ClippedShape], expected: &str) -> egui::Pos2 {
    shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == expected => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("folder button text {expected:?} was not painted"))
}

#[test]
fn result_table_folder_single_click_selects_and_double_click_opens_it() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    app.db = Some("synthetic-double-click.sqlite".into());
    app.route = Route::Search {
        query: "folder".into(),
        volume_id: None,
    };
    let folder = Entry {
        id: 42,
        volume_id: 7,
        parent_id: None,
        name: "Folder".into(),
        relative_path: "Folder".into(),
        kind: "directory".into(),
        size: 0,
        modified_unix: None,
        description: String::new(),
        tags: String::new(),
    };
    app.page.items = vec![folder.clone()];
    app.bus.set_handler(
        |_| json!({"ok":true,"data":{"items":[],"offset":0,"limit":250,"has_more":false}}),
    );

    let initial = paint_results(&mut app, &ctx, 1.0, Vec::new());
    let position = row_name_center(&initial, "▸ Folder");

    // A regular click selects the folder but does not navigate.
    paint_results(&mut app, &ctx, 1.1, pointer_frame(position, true));
    paint_results(&mut app, &ctx, 1.11, pointer_frame(position, false));
    assert!(matches!(&app.route, Route::Search { .. }));
    assert_eq!(app.selected.as_ref().map(|entry| entry.id), Some(folder.id));

    // Start a fresh pair outside egui's double-click interval, then release
    // twice at the same widget position within the default 300 ms window.
    paint_results(&mut app, &ctx, 1.5, Vec::new());
    paint_results(&mut app, &ctx, 1.6, pointer_frame(position, true));
    paint_results(&mut app, &ctx, 1.61, pointer_frame(position, false));
    paint_results(&mut app, &ctx, 1.75, pointer_frame(position, true));
    paint_results(&mut app, &ctx, 1.76, pointer_frame(position, false));

    assert_eq!(
        app.route,
        Route::Folder {
            volume_id: 7,
            parent_id: Some(42),
        }
    );
    assert_eq!(app.selected_volume, Some(7));
}
