use atlas_core::{Catalog, NewEntry};
use rusqlite::Connection;

fn file(name: &str, description: &str, tags: &str) -> NewEntry {
    NewEntry {
        parent_id: None,
        name: name.into(),
        relative_path: format!("Media/{name}"),
        kind: "file".into(),
        size: 5_000_000_001,
        modified_unix: Some(1_700_000_000),
        description: description.into(),
        tags: tags.into(),
    }
}

fn payload_rows(c: &Catalog) -> i64 {
    c.conn()
        .query_row("SELECT COUNT(*) FROM entry_payload", [], |r| r.get(0))
        .unwrap()
}

fn fts_check(c: &Catalog) {
    c.conn()
        .execute_batch(
            "INSERT INTO entries_trigram(entries_trigram,rank) VALUES('integrity-check',1)",
        )
        .unwrap();
    let integrity: String = c
        .conn()
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}

#[test]
fn compact_schema_sparsity_and_hot_query_plan() {
    let temp = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(temp.path().join("catalog.sqlite")).unwrap();
    let version: i64 = c
        .conn()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3);
    c.bulk_import("disk", "offline", |w| {
        w.insert(&file("Film.mkv", "", ""))?;
        w.insert(&file("Музыка.flac", "Редкое описание", "любимое"))?;
        w.insert(&file("Other.mkv", "", ""))?;
        Ok(())
    })
    .unwrap();
    assert_eq!(payload_rows(&c), 1);
    let columns: Vec<String> = c
        .conn()
        .prepare("PRAGMA table_info(entries_core)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for field in ["description", "tags", "search_text"] {
        assert!(
            !columns.iter().any(|v| v == field),
            "{field} leaked into hot rows"
        );
    }
    assert_eq!(
        c.query_table("name:Film AND size:>4GiB").unwrap(),
        "entries_core"
    );
    assert_eq!(c.query_table("description:редкое").unwrap(), "entries");
    let plan = c
        .explain_query("ext:mkv AND size:>4GiB")
        .unwrap()
        .to_string();
    assert!(plan.contains("entries_core"), "{plan}");
    assert!(
        !plan.contains("entry_payload"),
        "hot predicate touched payload: {plan}"
    );
    assert_eq!(
        c.search_fast("name:музыка", None, 0, 100).unwrap().items[0].description,
        "Редкое описание"
    );
    assert_eq!(
        c.search("NOT description:редкое", None, 0, 100)
            .unwrap()
            .total,
        2
    );
    assert_eq!(
        c.search("tag:любимое OR name:Film", None, 0, 100)
            .unwrap()
            .total,
        2
    );
    fts_check(&c);
}

#[test]
fn payload_insert_edit_remove_and_delete_keep_fts_consistent() {
    let temp = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(temp.path().join("catalog.sqlite")).unwrap();
    let mut id = 0;
    let volume = c
        .bulk_import("disk", "offline", |w| {
            id = w.insert(&file("Track.flac", "", ""))?;
            Ok(())
        })
        .unwrap();
    assert_eq!(payload_rows(&c), 0);
    c.update_entry(id, "Альбом один", "jazz").unwrap();
    assert_eq!(payload_rows(&c), 1);
    assert_eq!(
        c.search("description:альбом AND tag:jazz", None, 0, 10)
            .unwrap()
            .total,
        1
    );
    fts_check(&c);
    c.update_entry(id, "Второе описание", "rock").unwrap();
    assert_eq!(
        c.search("description:альбом OR tag:jazz", None, 0, 10)
            .unwrap()
            .total,
        0
    );
    fts_check(&c);
    c.update_entry(id, "", "").unwrap();
    assert_eq!(payload_rows(&c), 0);
    assert_eq!(c.search("NOT tag:rock", None, 0, 10).unwrap().total, 1);
    fts_check(&c);
    c.update_entry(id, "Уникальная аннотация", "tag").unwrap();
    c.conn()
        .execute("DELETE FROM volumes WHERE id=?1", [volume.id])
        .unwrap();
    assert_eq!(payload_rows(&c), 0);
    assert_eq!(c.search("аннотация", None, 0, 10).unwrap().total, 0);
    fts_check(&c);
}

#[test]
fn compact_pagination_hydrates_only_correct_rows() {
    let temp = tempfile::tempdir().unwrap();
    let mut c = Catalog::open(temp.path().join("catalog.sqlite")).unwrap();
    c.bulk_import("disk", "offline", |w| {
        for i in 0..17 {
            w.insert(&file(
                &format!("Track-{i:02}.flac"),
                if i % 2 == 0 { "note" } else { "" },
                "",
            ))?;
        }
        Ok(())
    })
    .unwrap();
    let expected = c.search("ext:flac", None, 0, 100).unwrap().items;
    let mut got = Vec::new();
    let mut offset = 0;
    loop {
        let page = c.search_fast("ext:flac", None, offset, 4).unwrap();
        offset += page.items.len() as u64;
        got.extend(page.items);
        if !page.has_more {
            break;
        }
    }
    assert_eq!(
        got.iter().map(|e| e.id).collect::<Vec<_>>(),
        expected.iter().map(|e| e.id).collect::<Vec<_>>()
    );
    assert_eq!(
        got.iter().map(|e| &e.description).collect::<Vec<_>>(),
        expected.iter().map(|e| &e.description).collect::<Vec<_>>()
    );
    fts_check(&c);
}

#[test]
fn old_catalog_versions_are_not_migrated_on_open() {
    for version in [1, 2] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("legacy.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(include_str!("../migrations/001.sql"))
            .unwrap();
        conn.pragma_update(None, "user_version", version).unwrap();
        drop(conn);
        let mut c = Catalog::open(&path).unwrap();
        let actual: i64 = c
            .conn()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(actual, version);
        assert_eq!(c.query_table("name:Legacy").unwrap(), "entries");
        c.bulk_import("old", "offline", |w| {
            w.insert(&file("Legacy.mkv", "old annotation", "keep"))?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            c.search("description:annotation", None, 0, 10)
                .unwrap()
                .total,
            1
        );
        fts_check(&c);
    }
}

#[test]
fn counted_search_keeps_exact_totals_and_pages_on_supported_schemas() {
    for version in [1, 2, 3] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("catalog.sqlite");
        if version < 3 {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(include_str!("../migrations/001.sql"))
                .unwrap();
            conn.pragma_update(None, "user_version", version).unwrap();
        }
        let mut c = Catalog::open(path).unwrap();
        let mut folder_id = 0;
        let first = c
            .bulk_import("first", "offline", |w| {
                folder_id = w.insert(&NewEntry {
                    kind: "directory".into(),
                    modified_unix: None,
                    ..file("Media", "", "")
                })?;
                for (name, description) in [
                    ("Сон.ФлАк", "Любимое исполнение"),
                    ("Film.mp4", ""),
                    ("Scratch.TMP", ""),
                ] {
                    w.insert(&NewEntry {
                        parent_id: Some(folder_id),
                        ..file(name, description, "")
                    })?;
                }
                w.insert(&file("Readme", "", ""))?;
                Ok(())
            })
            .unwrap();
        let second = c
            .bulk_import("second", "offline", |w| {
                for name in ["Trailer.mkv", "Second.tmp", "Loose.флак"] {
                    w.insert(&file(name, "", ""))?;
                }
                Ok(())
            })
            .unwrap();
        let parent_category = c.create_category("Избранное", None).unwrap();
        let child_category = c.create_category("Музыка", Some(parent_category)).unwrap();
        c.assign_category(folder_id, child_category).unwrap();

        // Search totals must come from current entry rows, even if stored
        // volume totals are stale.
        c.conn()
            .execute("UPDATE volumes SET entry_count = 100", [])
            .unwrap();

        let cases: [(&str, Option<i64>, &[&str]); 10] = [
            (
                "NOT ext:tmp",
                None,
                &[
                    "Film.mp4",
                    "Loose.флак",
                    "Media",
                    "Readme",
                    "Trailer.mkv",
                    "Сон.ФлАк",
                ],
            ),
            (
                "(NOT (ExT:\".ФЛАК\"))",
                None,
                &[
                    "Film.mp4",
                    "Media",
                    "Readme",
                    "Scratch.TMP",
                    "Second.tmp",
                    "Trailer.mkv",
                ],
            ),
            (
                "NOT ext:.",
                None,
                &[
                    "Film.mp4",
                    "Loose.флак",
                    "Scratch.TMP",
                    "Second.tmp",
                    "Trailer.mkv",
                    "Сон.ФлАк",
                ],
            ),
            (
                "NOT ext:tmp",
                Some(first.id),
                &["Film.mp4", "Media", "Readme", "Сон.ФлАк"],
            ),
            (
                "NOT ext:tmp",
                Some(second.id),
                &["Loose.флак", "Trailer.mkv"],
            ),
            ("NOT ext:tmp", Some(i64::MAX), &[]),
            (
                "NOT ext:tmp AND (category:ИЗБРАННОЕ OR ext:mkv)",
                None,
                &["Film.mp4", "Media", "Trailer.mkv", "Сон.ФлАк"],
            ),
            (
                "NOT (ext:tmp OR (ext:флак AND category:ИЗБРАННОЕ))",
                None,
                &["Film.mp4", "Loose.флак", "Media", "Readme", "Trailer.mkv"],
            ),
            ("NOT NOT ext:tmp", None, &["Scratch.TMP", "Second.tmp"]),
            ("NOT ext:tmp AND description:ЛЮБИМОЕ", None, &["Сон.ФлАк"]),
        ];
        for (query, volume, names) in cases {
            for offset in (0..=names.len() + 1).step_by(2) {
                let counted = c.search(query, volume, offset as u64, 2).unwrap();
                let fast = c.search_fast(query, volume, offset as u64, 2).unwrap();
                let actual: Vec<_> = counted.items.iter().map(|e| e.name.as_str()).collect();
                let fast_names: Vec<_> = fast.items.iter().map(|e| e.name.as_str()).collect();
                assert_eq!(counted.total, names.len() as u64, "v{version}: {query}");
                assert_eq!(
                    actual,
                    &names[offset.min(names.len())..(offset + 2).min(names.len())],
                    "v{version}: {query}, offset={offset}"
                );
                assert_eq!(actual, fast_names, "v{version}: {query}, offset={offset}");
                assert_eq!(fast.has_more, offset + 2 < names.len());
            }
        }
    }
}

#[test]
fn query_plan_hint_respects_field_names_and_quoted_values() {
    use atlas_core::query::needs_payload;
    for q in [
        "",
        "name:description",
        "path:*description*",
        "category:\"Music tag\" AND ext:flac",
    ] {
        assert!(!needs_payload(q).unwrap(), "{q}");
    }
    for q in [
        "notes",
        "text:notes",
        "NOT description:notes",
        "name:track OR tag:favourite",
    ] {
        assert!(needs_payload(q).unwrap(), "{q}");
    }
}
