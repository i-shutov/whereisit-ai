//! WAL durability acceptance on isolated temporary files. No user CTF or catalog is read.
//! This is a regression test, not a proof against untested OS/power-loss failures.
use atlas_core::Catalog;
use rusqlite::{params, Connection, TransactionBehavior};
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

type TestResult = Result<(), Box<dyn Error>>;

fn connection(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(15))?;
    Ok(db)
}

fn integrity_check(conn: &Connection) -> rusqlite::Result<()> {
    let result: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    assert_eq!(result, "ok", "SQLite integrity_check must succeed");
    let fk_errors: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_foreign_key_check",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(fk_errors, 0, "SQLite foreign key validation must succeed");
    Ok(())
}

#[test]
fn embedded_sqlite_includes_wal_reset_fix() {
    // See https://www.sqlite.org/wal.html#the_wal_reset_bug .
    // The fixed standard branch begins at 3.51.3. This test deliberately
    // rejects an older bundled copy even if cargo tests otherwise succeed.
    assert!(
        rusqlite::version_number() >= 3_051_003,
        "WAL-reset-fix gate failed: embedded SQLite {}",
        rusqlite::version()
    );
}

#[test]
fn negative_catalog_aggregates_cannot_wrap_as_u64() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let catalog = Catalog::open(temporary.path().join("negative.sqlite"))?;
    catalog.conn().execute(
        "INSERT INTO volumes(label,root_path,scanned_at,entry_count,total_bytes,status)
         VALUES('broken','',0,-1,-42,'complete')",
        [],
    )?;
    assert!(
        catalog.volumes().is_err(),
        "Negative entry counts must not wrap to an unsigned API value"
    );
    assert!(
        catalog.stats().is_err(),
        "Negative total sizes must not wrap in aggregate JSON"
    );
    Ok(())
}

#[test]
fn concurrent_writers_reader_checkpoint_and_rollback_preserve_rows() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("multi-connection.sqlite");
    let catalog = Catalog::open(&path)?;
    catalog.conn().execute_batch(
        "CREATE TABLE durability_events(
             writer INTEGER NOT NULL,
             ordinal INTEGER NOT NULL,
             PRIMARY KEY(writer, ordinal)
         );",
    )?;
    let mode: String =
        catalog.conn().query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
    assert_eq!(mode, "wal", "Test must exercise real WAL mode");
    drop(catalog);

    let mut threads = Vec::new();
    for writer in 0_i64..2 {
        let path = path.clone();
        threads.push(thread::spawn(move || -> rusqlite::Result<()> {
            let mut db = connection(&path)?;
            for ordinal in 0_i64..60 {
                let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute(
                    "INSERT INTO durability_events(writer, ordinal) VALUES(?1, ?2)",
                    params![writer, ordinal],
                )?;
                if ordinal % 12 == 0 {
                    // An intentionally abandoned transaction must not appear.
                    drop(tx);
                } else {
                    tx.commit()?;
                }
            }
            Ok(())
        }));
    }
    let reader_path = path.clone();
    let reader = thread::spawn(move || -> rusqlite::Result<()> {
        let db = connection(&reader_path)?;
        // A read transaction must retain its own snapshot across other commits.
        let tx = db.unchecked_transaction()?;
        let initial: i64 = tx.query_row(
            "SELECT COUNT(*) FROM durability_events",
            [],
            |r| r.get(0),
        )?;
        thread::sleep(Duration::from_millis(30));
        let same_snapshot: i64 = tx.query_row(
            "SELECT COUNT(*) FROM durability_events",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(initial, same_snapshot);
        tx.commit()?;
        Ok(())
    });
    let checkpoint_path = path.clone();
    let checkpointer = thread::spawn(move || -> rusqlite::Result<()> {
        let db = connection(&checkpoint_path)?;
        for _ in 0..40 {
            let _: (i64, i64, i64) =
                db.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
            thread::yield_now();
        }
        Ok(())
    });
    for worker in threads {
        worker.join().expect("writer panicked")?;
    }
    reader.join().expect("reader panicked")?;
    checkpointer.join().expect("checkpointer panicked")?;

    let db = connection(&path)?;
    let committed: i64 = db.query_row(
        "SELECT COUNT(*) FROM durability_events",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(committed, 110, "Two writers each commit 55 records");
    let distinct: i64 = db.query_row(
        "SELECT COUNT(*) FROM (
             SELECT writer, ordinal FROM durability_events GROUP BY writer, ordinal
         )",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(distinct, committed);
    let _: (i64, i64, i64) =
        db.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
    integrity_check(&db)?;
    drop(db);
    let reopened = connection(&path)?;
    let persisted: i64 = reopened.query_row(
        "SELECT COUNT(*) FROM durability_events",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(persisted, 110, "Committed rows must survive reopen");
    integrity_check(&reopened)?;
    Ok(())
}

#[test]
fn abrupt_child_exit_drops_only_uncommitted_wal_transaction() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("crash-recovery.sqlite");
    let child_executable = std::env::current_exe()?;
    let mut child = Command::new(child_executable)
        .arg("--exact")
        .arg("wal_crash_child")
        .arg("--nocapture")
        .env("WHEREISIT_WAL_CRASH_CHILD", &path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().expect("captured child output");
    let mut ready = false;
    for line in BufReader::new(stdout).lines() {
        if line?.contains("WAL_UNCOMMITTED_READY") {
            ready = true;
            break;
        }
    }
    if !ready {
        let output = child.wait_with_output()?;
        return Err(format!("Crash test helper failed: {}", output.status).into());
    }
    child.kill()?;
    let exit = child.wait()?;
    assert!(!exit.success(), "Child was not terminated");

    let conn = connection(&path)?;
    let committed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM crash_events WHERE value = 'committed'",
        [],
        |r| r.get(0),
    )?;
    let abandoned: i64 = conn.query_row(
        "SELECT COUNT(*) FROM crash_events WHERE value = 'uncommitted'",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(committed, 1, "Committed WAL record disappeared on process exit");
    assert_eq!(abandoned, 0, "Uncommitted WAL record leaked after process exit");
    integrity_check(&conn)?;
    Ok(())
}

/// Only the parent test sets the guard. The regular cargo-test invocation
/// leaves this test inert and does not terminate the test harness.
#[test]
fn wal_crash_child() -> TestResult {
    let Some(path) = std::env::var_os("WHEREISIT_WAL_CRASH_CHILD") else {
        return Ok(());
    };
    let mut conn = connection(std::path::Path::new(&path))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         CREATE TABLE crash_events(value TEXT NOT NULL);",
    )?;
    conn.execute(
        "INSERT INTO crash_events(value) VALUES ('committed')",
        [],
    )?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(
        "INSERT INTO crash_events(value) VALUES ('uncommitted')",
        [],
    )?;
    println!("WAL_UNCOMMITTED_READY");
    std::io::stdout().flush()?;
    thread::sleep(Duration::from_secs(90));
    drop(tx);
    Ok(())
}
