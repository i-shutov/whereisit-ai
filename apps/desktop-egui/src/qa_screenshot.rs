//! Opt-in QA builds only: capture the egui renderer, never desktop pixels.
use eframe::egui;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

pub fn capture_on_f12(ctx: &egui::Context) {
    if !ctx.input(|i| i.key_pressed(egui::Key::F12)) {
        return;
    }
    let Some(dir) = std::env::var_os("WHEREISIT_QA_SCREENSHOT_DIR").map(PathBuf::from) else {
        return;
    };
    if !dir.is_absolute() || !dir.is_dir() {
        return;
    }
    let file = dir.join(format!(
        "egui-{}-{}.png",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    ctx.request_screenshot(move |image| {
        if let Err(error) = write_png(&file, &image) {
            eprintln!("QA screenshot failed: {error}");
        }
    });
}

fn write_png(
    path: &std::path::Path,
    image: &egui::ColorImage,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut encoder = png::Encoder::new(file, image.size[0].try_into()?, image.size[1].try_into()?);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_png_without_overwriting_existing_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("synthetic.png");
        let image = egui::ColorImage::filled([2, 1], egui::Color32::RED);
        write_png(&path, &image).unwrap();
        let original = std::fs::read(&path).unwrap();
        assert!(original.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(write_png(&path, &image).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}
