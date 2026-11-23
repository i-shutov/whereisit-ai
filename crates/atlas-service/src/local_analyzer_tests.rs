use super::*;
use atlas_core::{Catalog, NewEntry};
use serde_json::json;

fn test_job() -> Arc<Job> {
    Arc::new(Job {
        cancel: AtomicBool::new(false),
        operation_cancel: AtomicBool::new(false),
        operation_active: AtomicBool::new(false),
        interrupt: Mutex::new(Vec::new()),
        inner: Mutex::new(JobState::default()),
        began: Instant::now(),
    })
}

// macOS commonly exposes the temporary directory through /var, a symlink to
// /private/var. Positive fixtures must avoid that alias because the analyzer
// intentionally rejects roots with symlinked ancestors.
fn test_tempdir() -> tempfile::TempDir {
    let parent = std::env::temp_dir().canonicalize().unwrap();
    let dir = tempfile::Builder::new()
        .prefix("la-")
        .tempdir_in(parent)
        .unwrap();
    assert_eq!(dir.path().canonicalize().unwrap(), dir.path());
    dir
}

#[test]
fn scan_preserves_empty_directories_unicode_and_exact_logical_totals() {
    let root = test_tempdir();
    fs::create_dir(root.path().join("empty")).unwrap();
    fs::create_dir_all(root.path().join("資料/é")).unwrap();
    fs::write(root.path().join("資料/é/同名.bin"), vec![1u8; 17]).unwrap();
    fs::write(root.path().join("top.txt"), b"abc").unwrap();
    let j = test_job();
    scan(&j, root.path().to_path_buf()).unwrap();
    let s = j.inner.lock().unwrap();
    assert_eq!(s.phase, COMPLETE);
    assert_eq!((s.files, s.dirs, s.bytes), (2, 3, 20));
    let snap = s.snapshot.as_ref().unwrap();
    let c = Connection::open(&snap.db).unwrap();
    let root_stats: (i64, i64, i64) = c
        .query_row(
            "SELECT files,dirs,bytes FROM folders WHERE parent_id IS NULL",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(root_stats, (2, 3, 20));
    let empty: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM folders WHERE name='empty' AND files=0 AND dirs=0 AND bytes=0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(empty, 1);
    let names: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM files WHERE norm_name=?1",
            [normalize_name("同名.bin")],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(names, 1);
}

#[test]
fn cancelled_scan_never_publishes_snapshot() {
    let root = test_tempdir();
    fs::write(root.path().join("a"), b"x").unwrap();
    let j = test_job();
    j.cancel.store(true, Ordering::Release);
    assert_eq!(
        scan(&j, root.path().to_path_buf()).unwrap_err().code,
        "cancelled"
    );
    assert!(j.inner.lock().unwrap().snapshot.is_none());
}

#[cfg(unix)]
#[test]
fn symlink_child_and_cycle_are_skipped_without_following() {
    use std::os::unix::fs::symlink;
    let root = test_tempdir();
    let outside = test_tempdir();
    fs::write(outside.path().join("secret.bin"), vec![0; 31]).unwrap();
    fs::create_dir(root.path().join("child")).unwrap();
    fs::write(root.path().join("child/ok.bin"), vec![0; 7]).unwrap();
    symlink(outside.path(), root.path().join("outside-link")).unwrap();
    symlink(root.path(), root.path().join("child/cycle")).unwrap();
    let j = test_job();
    scan(&j, root.path().to_path_buf()).unwrap();
    let s = j.inner.lock().unwrap();
    assert_eq!((s.files, s.dirs, s.bytes), (1, 1, 7));
    assert_eq!(s.skipped_links, 2);
}

#[cfg(unix)]
#[test]
fn symlink_root_or_ancestor_is_rejected() {
    use std::os::unix::fs::symlink;
    let base = test_tempdir();
    let target = base.path().join("target");
    fs::create_dir_all(target.join("child")).unwrap();
    let alias = base.path().join("alias");
    symlink(&target, &alias).unwrap();
    let j = test_job();
    assert_eq!(
        scan(&j, alias.join("child")).unwrap_err().code,
        "unsafe_path"
    );
    assert_eq!(scan(&j, alias).unwrap_err().code, "unsafe_path");
}

#[test]
fn invalid_catalog_check_does_not_leave_job_in_checking_phase() {
    let root = test_tempdir();
    let catalog_dir = test_tempdir();
    let catalog = catalog_dir.path().join("catalog.sqlite");
    let job_id = "analyzer-invalid-check-test";
    prepare(&json!({"job_id":job_id})).unwrap();
    start(&json!({"job_id":job_id,"root":root.path()})).unwrap();
    wait_for_phase(job_id, COMPLETE);
    let error = check_catalog(&json!({"job_id":job_id,"db":catalog,"volume_id":-2})).unwrap_err();
    assert_eq!(error.code, "invalid_request");
    assert_eq!(
        progress(&json!({"job_id":job_id})).unwrap()["phase"],
        COMPLETE
    );
    close(&json!({"job_id":job_id})).unwrap();
}

#[test]
fn catalog_check_matches_nfc_name_and_size_with_selected_volume_only() {
    let root = test_tempdir();
    let local_name = "Caf\u{0065}\u{0301}.MKV";
    fs::write(root.path().join(local_name), vec![1u8; 17]).unwrap();
    let dbdir = test_tempdir();
    let db = dbdir.path().join("catalog.sqlite");
    let mut catalog = Catalog::open(&db).unwrap();
    let a = catalog
        .bulk_import("Volume A", "A:/", |w| {
            w.insert(&NewEntry {
                parent_id: None,
                name: "Café.mkv".into(),
                relative_path: "Café.mkv".into(),
                kind: "file".into(),
                size: 17,
                modified_unix: None,
                description: String::new(),
                tags: String::new(),
            })?;
            Ok(())
        })
        .unwrap();
    catalog
        .bulk_import("Volume B", "B:/", |w| {
            w.insert(&NewEntry {
                parent_id: None,
                name: "CAFÉ.MKV".into(),
                relative_path: "CAFÉ.MKV".into(),
                kind: "file".into(),
                size: 18,
                modified_unix: None,
                description: String::new(),
                tags: String::new(),
            })?;
            Ok(())
        })
        .unwrap();
    drop(catalog);
    let job_id = "analyzer-catalog-nfc-test";
    prepare(&json!({"job_id":job_id})).unwrap();
    start(&json!({"job_id":job_id,"root":root.path()})).unwrap();
    wait_for_phase(job_id, COMPLETE);
    check_catalog(&json!({"job_id":job_id,"db":db,"volume_id":a.id})).unwrap();
    wait_for_phase(job_id, COMPLETE);
    let s = job(&json!({"job_id":job_id})).unwrap().1;
    let state = s.inner.lock().unwrap();
    assert!(
        state
            .catalog_generation
            .as_ref()
            .is_some_and(|g| !g.is_empty()),
        "catalog compare failed: {:?}",
        state.error_code
    );
    let snap = state.snapshot.as_ref().unwrap();
    let matches: i64 = Connection::open(&snap.db)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM matches", [], |r| r.get(0))
        .unwrap();
    assert_eq!(matches, 1);
    let vol: i64 = Connection::open(&snap.db)
        .unwrap()
        .query_row("SELECT volume_id FROM matches LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(vol, a.id);
    drop(state);
    let mut wal_os = db.as_os_str().to_os_string();
    wal_os.push("-wal");
    let wal = PathBuf::from(wal_os);
    if wal.exists() {
        fs::remove_file(&wal).unwrap();
    }
    assert!(!wal.exists());
    let unsafe_export =
        export_csv(&json!({"job_id":job_id,"db":db,"path":wal,"selected_ids":["file:1"]}))
            .unwrap_err();
    assert_eq!(unsafe_export.code, "unsafe_output");
    assert!(!wal.exists());
    close(&json!({"job_id":job_id})).unwrap();
}

#[test]
fn catalog_generation_change_is_rejected_without_publishing_new_matches() {
    let root = test_tempdir();
    fs::write(root.path().join("match.bin"), b"x").unwrap();
    let dbdir = test_tempdir();
    let db = dbdir.path().join("catalog.sqlite");
    let mut catalog = Catalog::open(&db).unwrap();
    catalog
        .bulk_import("Initial", "I:/", |w| {
            w.insert(&NewEntry {
                parent_id: None,
                name: "match.bin".into(),
                relative_path: "match.bin".into(),
                kind: "file".into(),
                size: 1,
                modified_unix: None,
                description: String::new(),
                tags: String::new(),
            })?;
            Ok(())
        })
        .unwrap();
    drop(catalog);
    let job_id = "analyzer-stale-generation-test";
    prepare(&json!({"job_id":job_id})).unwrap();
    start(&json!({"job_id":job_id,"root":root.path()})).unwrap();
    wait_for_phase(job_id, COMPLETE);
    check_catalog(&json!({"job_id":job_id,"db":db})).unwrap();
    wait_for_phase(job_id, COMPLETE);
    let j = job(&json!({"job_id":job_id})).unwrap().1;
    let generation = j
        .inner
        .lock()
        .unwrap()
        .catalog_generation
        .clone()
        .expect("initial catalog compare failed");
    let writer = Connection::open(&db).unwrap();
    writer
        .create_scalar_function(
            "unicode_lower",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8
                | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| Ok(ctx.get::<String>(0)?.to_lowercase()),
        )
        .unwrap();
    writer
        .execute(
            "UPDATE entries_core SET name='renamed.bin', name_norm='renamed.bin' WHERE id=1",
            [],
        )
        .unwrap();
    drop(writer);
    check_catalog(&json!({"job_id":job_id,"db":db,"expected_generation":generation})).unwrap();
    wait_for_phase(job_id, COMPLETE);
    let s = j.inner.lock().unwrap();
    assert_eq!(s.error_code.as_deref(), Some("stale_generation"));
    assert!(s.catalog_generation.is_none());
    let snap = s.snapshot.as_ref().unwrap();
    let matches: i64 = Connection::open(&snap.db)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM matches", [], |r| r.get(0))
        .unwrap();
    assert_eq!(matches, 0, "stale matches must not remain published");
    drop(s);
    close(&json!({"job_id":job_id})).unwrap();
}

#[test]
fn interrupt_registry_prunes_handles_after_connections_close() {
    let j = test_job();
    {
        let c = Connection::open_in_memory().unwrap();
        let handle = register_interrupt(&j, &c).unwrap();
        assert_eq!(j.interrupt.lock().unwrap().len(), 1);
        drop(handle);
    }
    let c = Connection::open_in_memory().unwrap();
    let _handle = register_interrupt(&j, &c).unwrap();
    assert_eq!(j.interrupt.lock().unwrap().len(), 1);
}

// The macOS CI filesystem rejects invalid-byte components, so this fixture is valid on
// Unix filesystems that support arbitrary filename bytes.
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn non_unicode_name_is_reported_as_read_error_instead_of_lossy_row() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = test_tempdir();
    let bad = OsString::from_vec(vec![b'x', 0xff, b'.', b't', b'x', b't']);
    fs::write(root.path().join(bad), b"payload").unwrap();
    let j = test_job();
    scan(&j, root.path().to_path_buf()).unwrap();
    let s = j.inner.lock().unwrap();
    assert_eq!(s.files, 0);
    assert_eq!(s.errors, 1);
    let snap = s.snapshot.as_ref().unwrap();
    let c = Connection::open(&snap.db).unwrap();
    let reason: String = c
        .query_row("SELECT reason FROM errors LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(reason, "non_unicode_path");
}

#[test]
fn active_cancel_and_close_prevent_snapshot_publication() {
    let root = test_tempdir();
    for i in 0..3000 {
        fs::write(root.path().join(format!("f{i:05}.bin")), b"x").unwrap();
    }
    let job_id = "analyzer-active-cancel-test";
    prepare(&json!({"job_id":job_id})).unwrap();
    start(&json!({"job_id":job_id,"root":root.path()})).unwrap();
    let j = job(&json!({"job_id":job_id})).unwrap().1;
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if j.interrupt
            .lock()
            .unwrap()
            .iter()
            .any(|weak| weak.strong_count() > 0)
        {
            break;
        }
        assert!(Instant::now() < deadline, "scan worker did not start");
        std::thread::yield_now();
    }
    cancel(&json!({"job_id":job_id})).unwrap();
    wait_for_phase(job_id, CANCELLED);
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    while j
        .interrupt
        .lock()
        .unwrap()
        .iter()
        .any(|weak| weak.strong_count() > 0)
    {
        assert!(Instant::now() < deadline, "cancelled scan did not stop");
        std::thread::yield_now();
    }
    assert!(j.inner.lock().unwrap().snapshot.is_none());
    close(&json!({"job_id":job_id})).unwrap();
    assert_eq!(
        progress(&json!({"job_id":job_id})).unwrap_err().code,
        "not_found"
    );

    let close_id = "analyzer-active-close-test";
    prepare(&json!({"job_id":close_id})).unwrap();
    start(&json!({"job_id":close_id,"root":root.path()})).unwrap();
    let close_job = job(&json!({"job_id":close_id})).unwrap().1;
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if close_job
            .interrupt
            .lock()
            .unwrap()
            .iter()
            .any(|weak| weak.strong_count() > 0)
        {
            break;
        }
        assert!(Instant::now() < deadline, "close scan worker did not start");
        std::thread::yield_now();
    }
    close(&json!({"job_id":close_id})).unwrap();
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    while close_job
        .interrupt
        .lock()
        .unwrap()
        .iter()
        .any(|weak| weak.strong_count() > 0)
    {
        assert!(Instant::now() < deadline, "closed scan did not stop");
        std::thread::yield_now();
    }
    assert!(close_job.inner.lock().unwrap().snapshot.is_none());
}

#[test]
fn cancelling_active_view_keeps_completed_local_snapshot_usable() {
    let root = test_tempdir();
    fs::write(root.path().join("x.bin"), b"x").unwrap();
    let job_id = "analyzer-view-cancel-test";
    prepare(&json!({"job_id":job_id})).unwrap();
    start(&json!({"job_id":job_id,"root":root.path()})).unwrap();
    wait_for_phase(job_id, COMPLETE);
    let j = job(&json!({"job_id":job_id})).unwrap().1;
    begin_operation(&j, &json!({})).unwrap();
    let c = Connection::open_in_memory().unwrap();
    let _handle = register_interrupt(&j, &c).unwrap();
    let response = cancel(&json!({"job_id":job_id})).unwrap();
    assert_eq!(response["scope"], "operation");
    assert!(j.operation_cancel.load(Ordering::Acquire));
    assert!(!j.cancel.load(Ordering::Acquire));
    assert_eq!(check(&j).unwrap_err().code, "cancelled");
    drop(_handle);
    drop(c);
    j.operation_cancel.store(false, Ordering::Release);
    j.operation_active.store(false, Ordering::Release);
    assert_eq!(check(&j).unwrap(), ());
    assert!(j.inner.lock().unwrap().snapshot.is_some());
    close(&json!({"job_id":job_id})).unwrap();
}

#[test]
fn pending_operation_cancel_blocks_late_export_and_claim_is_one_shot() {
    let root = test_tempdir();
    fs::write(root.path().join("row.bin"), b"x").unwrap();
    let output = test_tempdir();
    let job_id = "analyzer-operation-reservation-test";
    prepare(&json!({"job_id":job_id})).unwrap();
    start(&json!({"job_id":job_id,"root":root.path()})).unwrap();
    wait_for_phase(job_id, COMPLETE);

    let pending_id = "op-pending-export-once";
    prepare_operation(&json!({"job_id":job_id,"operation_id":pending_id})).unwrap();
    let cancelled = cancel(&json!({"job_id":job_id})).unwrap();
    assert_eq!(cancelled["scope"], "operation");
    assert_eq!(cancelled["pending"], true);
    let operation_id = "op-page-once";
    prepare_operation(&json!({"job_id":job_id,"operation_id":operation_id})).unwrap();
    assert_eq!(
        cancel(&json!({"job_id":job_id,"operation_id":pending_id}))
            .unwrap_err()
            .code,
        "conflict"
    );
    let target = output.path().join("must-not-appear.csv");
    let late = export_csv(
        &json!({"job_id":job_id,"operation_id":pending_id,"path":target,"selected_ids":["file:1"]}),
    )
    .unwrap_err();
    assert_eq!(late.code, "cancelled");
    assert!(!target.exists());
    assert_eq!(
        progress(&json!({"job_id":job_id})).unwrap()["operation_reserved"],
        true
    );
    assert!(job(&json!({"job_id":job_id}))
        .unwrap()
        .1
        .inner
        .lock()
        .unwrap()
        .snapshot
        .is_some());

    let result =
        page(&json!({"job_id":job_id,"operation_id":operation_id,"view":"largest","limit":10}))
            .unwrap();
    assert_eq!(result["total"], 1);
    let j = job(&json!({"job_id":job_id})).unwrap().1;
    let p = progress(&json!({"job_id":job_id})).unwrap();
    assert_eq!(p["operation_active"], false);
    assert_eq!(p["cancelled"], false);
    let duplicate =
        page(&json!({"job_id":job_id,"operation_id":operation_id,"view":"largest"})).unwrap_err();
    assert_eq!(duplicate.code, "cancelled");
    assert_eq!(
        prepare_operation(&json!({"job_id":job_id,"operation_id":operation_id}))
            .unwrap_err()
            .code,
        "conflict"
    );
    assert!(j.inner.lock().unwrap().snapshot.is_some());
    close(&json!({"job_id":job_id})).unwrap();
}

fn wait_for_phase(job_id: &str, expected: &str) {
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let p = progress(&json!({"job_id":job_id})).unwrap();
        if p["phase"] == expected {
            return;
        }
        assert_ne!(p["phase"], FAILED, "analyzer failed: {p}");
        assert!(Instant::now() < deadline, "analyzer phase timeout: {p}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
