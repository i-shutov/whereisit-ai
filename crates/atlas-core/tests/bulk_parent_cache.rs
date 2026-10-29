//! The bulk-import parent cache must preserve reference, volume and kind checks.
use atlas_core::{Catalog, NewEntry};

fn row(name: &str, path: &str, kind: &str, parent: Option<i64>) -> NewEntry {
    NewEntry {
        parent_id: parent,
        name: name.into(),
        relative_path: path.into(),
        kind: kind.into(),
        size: if kind == "file" { 1024 } else { 0 },
        modified_unix: None,
        description: String::new(),
        tags: String::new(),
    }
}

#[test]
fn thousands_of_siblings_and_nested_folders_preserve_all_rows_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let mut catalog = Catalog::open(dir.path().join("catalog.sqlite")).unwrap();
    let mut first_parent = 0_i64;
    let a = catalog
        .bulk_import("first", "offline", |writer| {
            first_parent = writer.insert(&row("Audio", "Audio", "directory", None))?;
            for i in 0..2_000 {
                writer.insert(&row(
                    &format!("track-{i:05}.flac"),
                    &format!("Audio/track-{i:05}.flac"),
                    "file",
                    Some(first_parent),
                ))?;
            }
            let nested = writer.insert(&row(
                "Live",
                "Audio/Live",
                "directory",
                Some(first_parent),
            ))?;
            writer.insert(&row(
                "recording.flac",
                "Audio/Live/recording.flac",
                "file",
                Some(nested),
            ))?;
            let archive = writer.insert(&row(
                "Package",
                "Package",
                "archive",
                None,
            ))?;
            writer.insert(&row(
                "inside.mp3",
                "Package/inside.mp3",
                "file",
                Some(archive),
            ))?;
            Ok(())
        })
        .unwrap();
    assert_eq!(a.entry_count, 2_005);
    assert_eq!(catalog.search("ext:flac", Some(a.id), 0, 1).unwrap().total, 2_001);
    assert_eq!(catalog.search("ext:mp3", None, 0, 5).unwrap().total, 1);
    assert_eq!(catalog.children(a.id, Some(first_parent), 0, 10).unwrap().total, 2_001);
    let foreign_keys: i64 = catalog
        .conn()
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(foreign_keys, 0);
    let integrity: String = catalog
        .conn()
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}

#[test]
fn files_missing_parents_and_other_volume_parents_are_still_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut catalog = Catalog::open(dir.path().join("catalog.sqlite")).unwrap();
    let mut previously_verified_parent = 0_i64;
    catalog.bulk_import("first", "offline", |w| {
        previously_verified_parent = w.insert(&row("Folder", "Folder", "directory", None))?;
        w.insert(&row("valid.mp3", "Folder/valid.mp3", "file", Some(previously_verified_parent)))?;
        Ok(())
    }).unwrap();
    let bad_file = catalog.bulk_import("file-parent", "offline", |w| {
        let file = w.insert(&row("not-a-folder", "not-a-folder", "file", None))?;
        w.insert(&row("invalid.mp3", "not-a-folder/invalid.mp3", "file", Some(file)))?;
        Ok(())
    });
    assert!(bad_file.is_err(), "File parent bypassed validation");
    let cross_volume = catalog.bulk_import("cross-volume", "offline", |w| {
        w.insert(&row("not-my-parent.mp3", "not-my-parent.mp3", "file",
                      Some(previously_verified_parent)))?;
        Ok(())
    });
    assert!(cross_volume.is_err(), "Cached parent from another volume was reused");
    let missing = catalog.bulk_import("missing", "offline", |w| {
        w.insert(&row("missing.mp3", "missing.mp3", "file", Some(i64::MAX)))?;
        Ok(())
    });
    assert!(missing.is_err(), "Unseen parent skipped SQLite validation");
    assert_eq!(catalog.volumes().unwrap().len(), 1);
    assert_eq!(catalog.search("ext:mp3", None, 0, 20).unwrap().total, 1);
}
