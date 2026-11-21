//! Fail-closed side-by-side Windows x64 QA paths. Never mutate production sidecars.
use std::path::{Path, PathBuf};

const QA_FOLDER: &str = "WhereIsIt AI egui Win64 QA";

fn root_for_localappdata(local_app_data: &Path) -> Option<PathBuf> {
    local_app_data
        .is_absolute()
        .then(|| local_app_data.join(QA_FOLDER))
}

fn qa_root() -> Option<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")?;
    root_for_localappdata(Path::new(&local_app_data))
}

pub fn user_store_path() -> Option<PathBuf> {
    qa_root().map(|root| root.join("physical-volume-bindings.sqlite"))
}

pub fn persistence_path() -> PathBuf {
    let Some(root) = qa_root() else {
        eprintln!("Isolated Win64 QA requires an absolute LOCALAPPDATA path.");
        std::process::exit(2);
    };
    if let Err(error) = std::fs::create_dir_all(&root) {
        eprintln!("Could not initialize isolated Win64 QA settings: {error}");
        std::process::exit(2);
    }
    root.join("egui-settings.ron")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_settings_and_sidecar_are_not_shared_with_production() {
        let local = PathBuf::from(r"C:\SyntheticLocalAppData");
        let root = root_for_localappdata(&local).unwrap();
        assert_eq!(root, local.join(QA_FOLDER));
        let sidecar = root.join("physical-volume-bindings.sqlite");
        let production = local
            .join("AtlasCatalog")
            .join("physical-volume-bindings.sqlite");
        assert_ne!(sidecar, production);
    }

    #[test]
    fn relative_local_app_data_is_refused() {
        assert!(root_for_localappdata(Path::new("relative-path")).is_none());
    }
}
