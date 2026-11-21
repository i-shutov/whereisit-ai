#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod bridge;
mod model;
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
    #[cfg(feature = "qa-screenshots")]
    let qa_persistence_path = std::env::var_os("WHEREISIT_QA_SETTINGS_PATH")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute());
    let native_options = eframe::NativeOptions {
        persist_window: false,
        #[cfg(feature = "qa-screenshots")]
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
