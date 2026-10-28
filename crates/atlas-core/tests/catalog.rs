use atlas_core::{Catalog, NewEntry};
use std::{fs, path::Path};
fn entry(name: &str, size: u64) -> NewEntry {
    NewEntry {
        parent_id: None,
        name: name.into(),
        relative_path: name.into(),
        kind: "file".into(),
        size,
        modified_unix: None,
        description: String::new(),
        tags: String::new(),
    }
}
#[test]
fn search_categories_and_queries() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(d.path().join("db")).unwrap();
    let mut id = 0;
    c.bulk_import("v", "offline", |w| {
        id = w.insert(&entry("Учебник.PDF", 11 * 1024 * 1024))?;
        w.insert(&entry("book.epub", 100))?;
        Ok(())
    })
    .unwrap();
    let cat = c.create_category("Книги", None).unwrap();
    c.assign_category(id, cat).unwrap();
    let p = c
        .search(
            "(ext:pdf OR ext:epub) AND size:>10MiB AND category:\"Книги\"",
            None,
            0,
            20,
        )
        .unwrap();
    assert_eq!(p.total, 1);
    assert_eq!(p.items[0].id, id);
    assert_eq!(c.search("name:УЧЕБ", None, 0, 20).unwrap().total, 1);
    assert_eq!(c.search("name:уч*", None, 0, 20).unwrap().total, 1);
    for bad in ["(", "name:", "a AND", "foo:bar", "size:>wat", "\"oops"] {
        assert!(c.search(bad, None, 0, 5).is_err(), "{bad}");
    }
    c.update_entry(id, "Описание книги", "Русский").unwrap();
    assert_eq!(c.search("description:ОПИС", None, 0, 5).unwrap().total, 1);
}
#[test]
fn scans_offline_and_rescan_preserves_snapshots() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("disk");
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("папка")).unwrap();
    fs::write(root.join("папка/файл.txt"), b"abc").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
    let mut c = Catalog::open(d.path().join("db")).unwrap();
    let a = c.scan(&root, "disk", None).unwrap();
    assert_eq!(c.search("файл", Some(a.id), 0, 5).unwrap().total, 1);
    #[cfg(unix)]
    assert_eq!(
        c.search("name:loop", Some(a.id), 0, 5).unwrap().items[0].kind,
        "symlink"
    );
    fs::remove_dir_all(&root).unwrap();
    assert!(c.scan(&root, "bad", None).is_err());
    assert_eq!(c.volumes().unwrap().len(), 1);
    assert_eq!(c.search("файл", None, 0, 5).unwrap().total, 1);
    fs::create_dir(&root).unwrap();
    fs::write(root.join("новый"), b"x").unwrap();
    let b = c.scan(&root, "disk", None).unwrap();
    assert_ne!(a.id, b.id);
    assert_eq!(c.volumes().unwrap().len(), 2);
    assert_eq!(
        c.children(a.id, None, 0, 20).unwrap().total,
        a.entry_count - 1
    );
    let export = d.path().join("all.json");
    c.export_json(Path::new(&export)).unwrap();
    let val: serde_json::Value = serde_json::from_slice(&fs::read(export).unwrap()).unwrap();
    assert_eq!(val["volumes"].as_array().unwrap().len(), 2);
}
#[test]
fn bulk_failure_rolls_back() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(d.path().join("db")).unwrap();
    let result = c.bulk_import("bad", "x", |w| {
        w.insert(&entry("a", 1))?;
        w.insert(&NewEntry {
            kind: "invalid".into(),
            ..entry("b", 2)
        })?;
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(c.volumes().unwrap().len(), 0);
    assert_eq!(c.search("", None, 0, 5).unwrap().total, 0);
}
#[test]
fn api_rejects_bad_json_and_offsets() {
    let invalid: serde_json::Value =
        serde_json::from_str(&atlas_core::api::dispatch_json("{")).unwrap();
    assert_eq!(invalid["error"]["code"], "invalid_json");
    let d = tempfile::tempdir().unwrap();
    let req =
        serde_json::json!({"command":"search","db":d.path().join("db"),"query":"x","offset":-1});
    let result: serde_json::Value =
        serde_json::from_str(&atlas_core::api::dispatch_json(&req.to_string())).unwrap();
    assert_eq!(result["error"]["code"], "invalid_request");
}
#[test]
fn cancel_scan_rolls_back_snapshot() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("disk");
    fs::create_dir(&root).unwrap();
    for i in 0..3000 {
        fs::write(root.join(format!("{i}.txt")), b"x").unwrap();
    }
    let db = d.path().join("db");
    let job = format!("cancel-{}", std::process::id());
    let worker_db = db.clone();
    let worker_job = job.clone();
    let handle = std::thread::spawn(move || {
        let mut c = Catalog::open(worker_db).unwrap();
        c.scan(&root, "disk", Some(&worker_job))
    });
    let start = std::time::Instant::now();
    loop {
        if atlas_core::progress_job(&job).is_ok() {
            atlas_core::cancel_job(&job).unwrap();
            break;
        }
        if start.elapsed().as_secs() > 10 {
            panic!("scan job did not start")
        }
        std::thread::yield_now();
    }
    let result = handle.join().unwrap();
    assert_eq!(result.unwrap_err().code, "cancelled");
    let c = Catalog::open(db).unwrap();
    assert!(c.volumes().unwrap().is_empty());
}
#[test]
fn exact_size_bounds_wildcards_and_query_limits() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(d.path().join("db")).unwrap();
    c.bulk_import("test", "offline", |w| {
        w.insert(&entry("Film_A.mkv", 1536))?;
        w.insert(&entry("Film_B.mp4", 1537))?;
        Ok(())
    })
    .unwrap();
    assert_eq!(c.search("size:=1.5KiB", None, 0, 10).unwrap().total, 1);
    assert_eq!(c.search("size:>1536.1B", None, 0, 10).unwrap().total, 1);
    assert_eq!(c.search("size:=1536.1B", None, 0, 10).unwrap().total, 0);
    assert_eq!(c.search("name:*.mkv", None, 0, 10).unwrap().total, 1);
    assert_eq!(
        c.search("name:film_[ab].mkv", None, 0, 10).unwrap().total,
        1
    );
    assert!(c
        .search(
            &format!("{}x{}", "(".repeat(33), ")".repeat(33)),
            None,
            0,
            10
        )
        .is_err());
    assert!(c
        .search(&format!("{}x", "NOT ".repeat(33)), None, 0, 10)
        .is_err());
    let req = serde_json::json!({"op":"stats","db":d.path().join("db")});
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&atlas_core::api::dispatch_json(
            &req.to_string()
        ))
        .unwrap()["ok"],
        true
    );
}
#[test]
fn multiple_volumes_commit_or_roll_back_together() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(d.path().join("db")).unwrap();
    let result: atlas_core::Result<()> = c.bulk_import_many(|batch| {
        batch.start_volume("one", "offline:one")?;
        batch.insert(&entry("first", 1))?;
        batch.finish_volume()?;
        batch.start_volume("two", "offline:two")?;
        batch.insert(&entry("second", 2))?;
        Err(atlas_core::Error::new("invalid_import", "bad source"))
    });
    assert!(result.is_err());
    assert!(c.volumes().unwrap().is_empty());
    let result: atlas_core::Result<()> = c.bulk_import_many(|batch| {
        batch.start_volume("unfinished", "offline")?;
        Ok(())
    });
    assert!(result.is_err());
    assert!(c.volumes().unwrap().is_empty());
    c.bulk_import_many(|batch| {
        for label in ["one", "two"] {
            batch.start_volume(label, "offline")?;
            batch.insert(&entry(label, 1))?;
            batch.finish_volume()?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(c.volumes().unwrap().len(), 2);
    assert_eq!(c.search("", None, 0, 10).unwrap().total, 2);
}
#[test]
fn fast_search_paginates_without_losing_results() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(d.path().join("db")).unwrap();
    c.bulk_import("one", "offline", |w| {
        for n in ["a", "b", "c"] {
            w.insert(&entry(n, 1))?;
        }
        Ok(())
    })
    .unwrap();
    let first = c.search_fast("", None, 0, 2).unwrap();
    assert_eq!(first.items.len(), 2);
    assert!(first.has_more);
    let second = c.search_fast("", None, 2, 2).unwrap();
    assert_eq!(second.items.len(), 1);
    assert!(!second.has_more);
    assert_eq!(second.items[0].name, "c");
}
#[test]
fn volume_size_counts_logical_files_not_directory_or_archive_aggregates() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(d.path().join("db")).unwrap();
    let volume = c
        .bulk_import("one", "offline", |w| {
            let parent = w.insert(&NewEntry {
                kind: "directory".into(),
                size: 1000,
                ..entry("folder", 0)
            })?;
            w.insert(&NewEntry {
                parent_id: Some(parent),
                ..entry("file", 100)
            })?;
            let archive = w.insert(&NewEntry {
                kind: "archive".into(),
                size: 500,
                ..entry("archive.zip", 0)
            })?;
            w.insert(&NewEntry {
                parent_id: Some(archive),
                ..entry("archive_member", 200)
            })?;
            Ok(())
        })
        .unwrap();
    assert_eq!(volume.total_bytes, 300);
    let folders = c.children(volume.id, None, 0, 10).unwrap();
    assert_eq!(
        folders
            .items
            .iter()
            .find(|e| e.kind == "directory")
            .unwrap()
            .size,
        1000
    );
}
#[test]
fn folder_categories_apply_to_descendants_without_crossing_volumes() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(d.path().join("db")).unwrap();
    let mut folder = 0;
    let mut nested_file = 0;
    let mut other_file = 0;
    let first = c
        .bulk_import("first", "offline1", |w| {
            folder = w.insert(&NewEntry {
                kind: "directory".into(),
                ..entry("Music", 0)
            })?;
            let child = w.insert(&NewEntry {
                parent_id: Some(folder),
                kind: "directory".into(),
                relative_path: "Music/Album".into(),
                ..entry("Album", 0)
            })?;
            nested_file = w.insert(&NewEntry {
                parent_id: Some(child),
                relative_path: "Music/Album/song.flac".into(),
                ..entry("song.flac", 123)
            })?;
            Ok(())
        })
        .unwrap();
    let second = c
        .bulk_import("second", "offline2", |w| {
            let parent = w.insert(&NewEntry {
                kind: "directory".into(),
                ..entry("Music", 0)
            })?;
            other_file = w.insert(&NewEntry {
                parent_id: Some(parent),
                relative_path: "Music/song.flac".into(),
                ..entry("song.flac", 123)
            })?;
            Ok(())
        })
        .unwrap();
    let cat = c.create_category("Music", None).unwrap();
    c.assign_category(folder, cat).unwrap();
    let inherited = c
        .search("category:Music AND ext:flac", None, 0, 20)
        .unwrap();
    assert_eq!(inherited.total, 1);
    assert_eq!(inherited.items[0].id, nested_file);
    assert_eq!(
        c.search("category:Music", Some(first.id), 0, 20)
            .unwrap()
            .total,
        3
    );
    assert_eq!(
        c.search("category:Music", Some(second.id), 0, 20)
            .unwrap()
            .total,
        0
    );
    let excluded = c
        .search("NOT category:Music AND ext:flac", None, 0, 20)
        .unwrap();
    assert_eq!(excluded.total, 1);
    assert_eq!(excluded.items[0].id, other_file);
    c.assign_category(other_file, cat).unwrap();
    assert_eq!(
        c.search("category:Music AND ext:flac", None, 0, 20)
            .unwrap()
            .total,
        2
    );
}

#[test]
fn bulk_import_collects_query_planner_statistics() {
    let d = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(d.path().join("db")).unwrap();
    c.bulk_import("one", "offline", |w| {
        for i in 0..100 {
            w.insert(&entry(&format!("file{i}.flac"), i))?;
        }
        Ok(())
    })
    .unwrap();
    let n: i64 = c
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_stat1 WHERE tbl='entries_core'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(n > 0, "planner statistics missing after bulk import");
    assert_eq!(c.search("ext:flac", None, 0, 10).unwrap().total, 100);
}
