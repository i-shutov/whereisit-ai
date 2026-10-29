//! Synthetic-only FTS bulk build equivalence and atomic rollback gates.
use atlas_core::{Catalog, NewEntry, Result};

fn entry(name: &str, path: &str, parent: Option<i64>, kind: &str, description: &str) -> NewEntry {
    NewEntry {
        parent_id: parent,
        name: name.to_owned(),
        relative_path: path.to_owned(),
        kind: kind.to_owned(),
        size: if kind == "file" { 4096 } else { 0 },
        modified_unix: None,
        description: description.to_owned(),
        tags: String::new(),
    }
}

fn write_synthetic(c: &mut Catalog, fast: bool) -> Result<()> {
    let write = |session: &mut atlas_core::ImportSession<'_>| {
        let first = session.start_volume("Synthetic music", "offline")?;
        let root = session.insert(&entry("Albums", "Albums", None, "directory", ""))?;
        for index in 0..250 {
            let name = format!("recording-{index:04}.flac");
            let path = format!("Albums/{name}");
            let description = if index % 17 == 0 {
                "Rare performance notes"
            } else {
                ""
            };
            session.insert(&entry(&name, &path, Some(root), "file", description))?;
        }
        session.finish_volume()?;
        let second = session.start_volume("Synthetic books", "offline2")?;
        let root = session.insert(&entry("Books", "Books", None, "directory", ""))?;
        session.insert(&entry(
            "guide.pdf",
            "Books/guide.pdf",
            Some(root),
            "file",
            "Searchable metadata",
        ))?;
        session.finish_volume()?;
        assert!(second > first);
        Ok(())
    };
    if fast {
        c.bulk_import_many_fast_fts(write)
    } else {
        c.bulk_import_many(write)
    }
}

fn fts_integrity(c: &Catalog) {
    c.conn()
        .execute(
            "INSERT INTO entries_trigram(entries_trigram,rank) VALUES('integrity-check',1)",
            [],
        )
        .expect("full FTS5 content integrity");
    let result: String = c
        .conn()
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(result, "ok");
}

#[test]
fn fast_fts_and_triggers_equal_original_rows_search_and_late_updates() {
    let dir = tempfile::tempdir().unwrap();
    let mut regular = Catalog::open(dir.path().join("regular.sqlite")).unwrap();
    let mut optimized = Catalog::open(dir.path().join("bulk.sqlite")).unwrap();
    write_synthetic(&mut regular, false).unwrap();
    write_synthetic(&mut optimized, true).unwrap();
    fts_integrity(&regular);
    fts_integrity(&optimized);
    for query in [
        "",
        "name:recording",
        "ext:flac",
        "description:Rare",
        "metadata",
        "(ext:flac OR ext:pdf) AND NOT name:missing",
    ] {
        let left = regular.search(query, None, 0, 1000).unwrap();
        let right = optimized.search(query, None, 0, 1000).unwrap();
        assert_eq!(left.total, right.total, "query type parity");
        assert_eq!(
            left.items.iter().map(|r| r.id).collect::<Vec<_>>(),
            right.items.iter().map(|r| r.id).collect::<Vec<_>>(),
            "first-page IDs"
        );
    }
    let before: i64 = optimized.conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name IN ('core_ai','payload_ai')",
            [], |r| r.get(0),
        ).unwrap();
    assert_eq!(before, 2, "both insert triggers restored");
    let entry_id: i64 = optimized
        .conn()
        .query_row(
            "SELECT id FROM entries_core WHERE name='guide.pdf'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    optimized
        .update_entry(entry_id, "Updated searchable terms", "")
        .unwrap();
    assert_eq!(
        optimized
            .search("description:Updated", None, 0, 5)
            .unwrap()
            .total,
        1,
        "post-import FTS update triggers must continue working"
    );
    // A previously sparse row exercises the restored payload INSERT trigger.
    let sparse_id: i64 = optimized
        .conn()
        .query_row(
            "SELECT id FROM entries_core WHERE name='recording-0001.flac'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    optimized
        .update_entry(sparse_id, "NewUniquePayload", "")
        .unwrap();
    assert_eq!(
        optimized
            .search("description:NewUniquePayload", None, 0, 5)
            .unwrap()
            .total,
        1,
        "payload_ai must be restored when the bulk transaction commits"
    );
    optimized
        .bulk_import("after", "offline3", |w| {
            w.insert(&entry("later.pdf", "later.pdf", None, "file", ""))?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        optimized.search("name:later", None, 0, 5).unwrap().total,
        1,
        "core_ai must be restored when the bulk transaction commits"
    );
    fts_integrity(&optimized);
}

#[test]
fn failed_initial_fast_import_restores_every_trigger_and_all_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(dir.path().join("failed.sqlite")).unwrap();
    let result: Result<()> = c.bulk_import_many_fast_fts(|s| {
        s.start_volume("synthetic", "offline")?;
        s.insert(&entry("ok.flac", "ok.flac", None, "file", ""))?;
        // Invalid parent: failures must roll back both the data and DDL.
        s.insert(&entry("orphan", "orphan", Some(i64::MAX), "file", ""))?;
        s.finish_volume()?;
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(c.volumes().unwrap().len(), 0);
    assert_eq!(c.search("", None, 0, 10).unwrap().total, 0);
    let triggers: i64 = c.conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name IN ('core_ai','payload_ai')",
            [], |r| r.get(0)
        ).unwrap();
    assert_eq!(triggers, 2, "failed import must not remove triggers");
    // Re-open and insert through the original path after the failed transaction.
    c.bulk_import("recovery", "offline", |w| {
        w.insert(&entry("after.pdf", "after.pdf", None, "file", ""))?;
        Ok(())
    })
    .unwrap();
    fts_integrity(&c);
    assert_eq!(c.search("name:after", None, 0, 5).unwrap().total, 1);
}

#[test]
fn already_populated_v3_falls_back_to_normal_incremental_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(dir.path().join("existing.sqlite")).unwrap();
    c.bulk_import("existing", "offline", |w| {
        w.insert(&entry("previous.flac", "previous.flac", None, "file", ""))?;
        Ok(())
    })
    .unwrap();
    write_synthetic(&mut c, true).unwrap();
    fts_integrity(&c);
    assert_eq!(c.search("ext:flac", None, 0, 5).unwrap().total, 251);
}
