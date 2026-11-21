//! Inter-process category membership regression on generated SQLite v3 only.
use atlas_core::{api, Catalog};
use rusqlite::{Transaction, TransactionBehavior};
use serde_json::{json, Value};
use std::sync::{Arc, Barrier};

#[test]
fn deleting_an_entry_before_competing_category_write_returns_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("catalog.sqlite");
    let media = dir.path().join("synthetic");
    std::fs::create_dir(&media).unwrap();
    std::fs::write(media.join("film.mkv"), b"generated fixture only").unwrap();

    let call = |req: Value| -> Value {
        serde_json::from_str(&api::dispatch_json(&req.to_string())).unwrap()
    };
    assert_eq!(call(json!({"op":"init","db":db}))["ok"], true);
    let scanned = call(json!({"op":"scan","db":db,"root":media,
        "label":"Synthetic","hash":false}));
    assert_eq!(scanned["ok"], true, "{scanned}");
    let found = call(json!({"op":"search_fast","db":db,
        "query":"ext:mkv","offset":0,"limit":250}));
    let entry_id = found["data"]["items"][0]["id"].as_i64().unwrap();
    let created = call(json!({"op":"create_category","db":db,"name":"Видео"}));
    assert_eq!(created["ok"], true, "{created}");
    let category_id = created["data"]["id"].as_i64().unwrap();

    // Both processes obtain their own SQLite connection before the lock.
    let blocker = Catalog::open(&db).unwrap();
    let competitor = Catalog::open(&db).unwrap();
    let tx =
        Transaction::new_unchecked(blocker.conn(), TransactionBehavior::Immediate).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = Arc::clone(&barrier);
    let competing = std::thread::spawn(move || {
        barrier2.wait();
        competitor.assign_category(entry_id, category_id)
    });
    barrier.wait();
    // The competing BEGIN IMMEDIATE must wait for our writer, so the
    // existence check cannot observe a row before we commit its deletion.
    std::thread::sleep(std::time::Duration::from_millis(60));
    tx.execute("DELETE FROM entries_core WHERE id=?1", [entry_id])
        .unwrap();
    tx.commit().unwrap();

    let error = competing.join().unwrap().unwrap_err();
    assert_eq!(error.code, "not_found");
    let after = call(json!({"op":"entry_category_ids","db":db,"entry_id":entry_id}));
    assert_eq!(after["ok"], false, "{after}");
    assert_eq!(after["error"]["code"], "not_found");
}

#[test]
fn category_reads_observe_one_sqlite_snapshot_and_reject_deleted_entries() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("catalog.sqlite");
    let media = dir.path().join("generated");
    std::fs::create_dir(&media).unwrap();
    std::fs::write(media.join("entry.mkv"), b"generated").unwrap();
    let call = |req: Value| -> Value {
        serde_json::from_str(&api::dispatch_json(&req.to_string())).unwrap()
    };
    assert_eq!(call(json!({"op":"init","db":db}))["ok"], true);
    assert_eq!(call(json!({"op":"scan","db":db,"root":media,
        "label":"Synthetic","hash":false}))["ok"], true);
    let found = call(json!({"op":"search_fast","db":db,"query":"ext:mkv",
        "offset":0,"limit":250}));
    let id = found["data"]["items"][0]["id"].as_i64().unwrap();

    let reader = Catalog::open(&db).unwrap();
    let writer = Catalog::open(&db).unwrap();
    let snapshot = reader.conn().unchecked_transaction().unwrap();
    let exists: i64 = snapshot
        .query_row("SELECT EXISTS(SELECT 1 FROM entries WHERE id=?1)", [id], |r| r.get(0))
        .unwrap();
    assert_eq!(exists, 1);
    writer
        .conn()
        .execute("DELETE FROM entries_core WHERE id=?1", [id])
        .unwrap();
    let still_visible: i64 = snapshot
        .query_row("SELECT EXISTS(SELECT 1 FROM entries WHERE id=?1)", [id], |r| r.get(0))
        .unwrap();
    assert_eq!(still_visible, 1, "read snapshot must not mix old and new states");
    snapshot.commit().unwrap();

    let result = reader.entry_category_ids(id);
    assert_eq!(result.unwrap_err().code, "not_found");
}
