#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod bridge;
mod categories;
mod model;
#[cfg(all(feature = "qa-isolated", target_os = "windows"))]
mod qa_isolation;
#[cfg(feature = "qa-screenshots")]
mod qa_screenshot;
mod shelf;
mod ui;

fn main() -> eframe::Result<()> {
    let mut startup_catalog = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--catalog" => startup_catalog = args.next(),
            "--help" | "-h" => {
                println!("WhereIsIt AI native egui preview\n  --catalog <existing.sqlite>");
                return Ok(());
            }
            other => {
                eprintln!("Unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }
    #[cfg(all(feature = "qa-screenshots", not(feature = "qa-isolated")))]
    let qa_persistence_path = std::env::var_os("WHEREISIT_QA_SETTINGS_PATH")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute());
    #[cfg(all(feature = "qa-isolated", target_os = "windows"))]
    let qa_settings_path = crate::qa_isolation::persistence_path();
    let native_options = eframe::NativeOptions {
        persist_window: false,
        #[cfg(all(feature = "qa-isolated", target_os = "windows"))]
        persistence_path: Some(qa_settings_path),
        #[cfg(all(feature = "qa-screenshots", not(feature = "qa-isolated")))]
        persistence_path: qa_persistence_path,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 760.0])
            .with_min_inner_size([880.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "WhereIsIt AI — egui",
        native_options,
        Box::new(move |creation| {
            Ok(Box::new(ui::DesktopApp::new(
                creation,
                startup_catalog.clone(),
            )))
        }),
    )
}
