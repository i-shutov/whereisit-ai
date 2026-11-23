use atlas_core::{analytics::AnalyticsRequest, Catalog, NewEntry};

fn item(parent_id: Option<i64>, name: &str, path: &str, kind: &str, size: u64) -> NewEntry {
    NewEntry {
        parent_id,
        name: name.into(),
        relative_path: path.into(),
        kind: kind.into(),
        size,
        modified_unix: None,
        description: String::new(),
        tags: String::new(),
    }
}

#[test]
fn analytics_counts_logical_files_once_and_preserves_unicode_scope() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("catalog.sqlite");
    let mut c = Catalog::open(&db).unwrap();
    let mut folder = 0;
    let mut archive = 0;
    let mut file = 0;
    c.bulk_import("Données", "offline", |w| {
        folder = w.insert(&item(None, "資料", "資料", "directory", 0))?;
        archive = w.insert(&item(
            Some(folder),
            "backup.zip",
            "資料/backup.zip",
            "archive",
            999,
        ))?;
        file = w.insert(&item(
            Some(archive),
            "résumé.txt",
            "資料/backup.zip/résumé.txt",
            "file",
            12,
        ))?;
        w.insert(&item(None, "top.MP4", "top.MP4", "file", 7))?;
        Ok(())
    })
    .unwrap();
    let category = c.create_category("記録", None).unwrap();
    c.assign_category(file, category).unwrap();
    let secondary = c.create_category("Second", None).unwrap();
    c.assign_category(file, secondary).unwrap();
    let secondary = c.create_category("Second", None).unwrap();
    c.assign_category(file, secondary).unwrap();
    drop(c);
    let ro = Catalog::open_readonly(&db).unwrap();
    let all = ro
        .analytics(
            &AnalyticsRequest {
                volume_id: None,
                folder_entry_id: None,
                group_by: "volumes".into(),
                highlight_query: None,
                category_id: None,
                max_depth: 8,
                max_nodes: 100,
            },
            || false,
        )
        .unwrap();
    assert_eq!(all.totals.files, 2);
    assert_eq!(all.totals.indexed_bytes, 19);
    assert_eq!(all.root.size, 19);
    assert!(all.root.children[0]
        .children
        .iter()
        .any(|n| n.name == "資料"));
    assert_eq!(all.by_category.iter().map(|x| x.bytes).sum::<u64>(), 19);
    let category_scope = ro
        .analytics(
            &AnalyticsRequest {
                volume_id: None,
                folder_entry_id: None,
                group_by: "extensions".into(),
                highlight_query: None,
                category_id: Some(secondary),
                max_depth: 8,
                max_nodes: 100,
            },
            || false,
        )
        .unwrap();
    assert_eq!(category_scope.totals.indexed_bytes, 12);
    assert_eq!(
        category_scope
            .by_extension
            .iter()
            .map(|x| x.bytes)
            .sum::<u64>(),
        12
    );
    let filtered_tree = ro
        .analytics(
            &AnalyticsRequest {
                volume_id: None,
                folder_entry_id: None,
                group_by: "volumes".into(),
                highlight_query: None,
                category_id: Some(secondary),
                max_depth: 8,
                max_nodes: 2,
            },
            || false,
        )
        .unwrap();
    assert!(filtered_tree.root.children[0]
        .children
        .iter()
        .any(|n| n.name == "資料"));
    assert_eq!(filtered_tree.omitted_node_count, 2);
    assert_eq!(filtered_tree.remainder_bytes, 12);
    assert_eq!(
        category_scope
            .by_category
            .iter()
            .map(|x| x.bytes)
            .sum::<u64>(),
        12
    );
    let highlighted = ro
        .analytics(
            &AnalyticsRequest {
                volume_id: None,
                folder_entry_id: None,
                group_by: "volumes".into(),
                highlight_query: Some("ext:txt".into()),
                category_id: None,
                max_depth: 8,
                max_nodes: 100,
            },
            || false,
        )
        .unwrap();
    assert_eq!(highlighted.root.matched_bytes, 12);
    let ext_group = ro
        .analytics(
            &AnalyticsRequest {
                volume_id: None,
                folder_entry_id: None,
                group_by: "extensions".into(),
                highlight_query: Some("ext:txt".into()),
                category_id: None,
                max_depth: 8,
                max_nodes: 100,
            },
            || false,
        )
        .unwrap();
    assert_eq!(
        ext_group
            .root
            .children
            .iter()
            .find(|n| n.id == "ext:txt")
            .unwrap()
            .matched_bytes,
        12
    );
    let cat_group = ro
        .analytics(
            &AnalyticsRequest {
                volume_id: None,
                folder_entry_id: None,
                group_by: "categories".into(),
                highlight_query: Some("ext:txt".into()),
                category_id: None,
                max_depth: 8,
                max_nodes: 100,
            },
            || false,
        )
        .unwrap();
    assert_eq!(
        cat_group
            .root
            .children
            .iter()
            .find(|n| n.id == format!("category:{category}"))
            .unwrap()
            .matched_bytes,
        12
    );
    assert!(all
        .by_category
        .iter()
        .any(|x| x.label == "記録" && x.bytes == 12));
    let scoped = ro
        .analytics(
            &AnalyticsRequest {
                volume_id: Some(1),
                folder_entry_id: Some(folder),
                group_by: "extensions".into(),
                highlight_query: None,
                category_id: None,
                max_depth: 8,
                max_nodes: 100,
            },
            || false,
        )
        .unwrap();
    assert_eq!(scoped.totals.indexed_bytes, 12);
    assert_eq!(scoped.root.size, 12);
}

#[test]
fn analytics_cancellation_and_bounds_are_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("catalog.sqlite");
    let mut c = Catalog::open(&db).unwrap();
    c.bulk_import("Volume", "offline", |w| {
        for i in 0..8 {
            w.insert(&item(
                None,
                &format!("{i}.bin"),
                &format!("{i}.bin"),
                "file",
                i,
            ))?;
        }
        Ok(())
    })
    .unwrap();
    drop(c);
    let ro = Catalog::open_readonly(&db).unwrap();
    let req = AnalyticsRequest {
        volume_id: None,
        folder_entry_id: None,
        group_by: "volumes".into(),
        highlight_query: None,
        category_id: None,
        max_depth: 2,
        max_nodes: 1,
    };
    assert_eq!(ro.analytics(&req, || true).unwrap_err().code, "cancelled");
    let data = ro.analytics(&req, || false).unwrap();
    assert!(data.truncated);
    assert_eq!(data.omitted_node_count, 8);
    assert_eq!(data.remainder_bytes, 28);
    assert_eq!(data.totals.files, 8);
    assert_eq!(data.totals.indexed_bytes, 28);
    let checks = std::sync::atomic::AtomicUsize::new(0);
    let err = ro
        .analytics(
            &AnalyticsRequest {
                volume_id: None,
                folder_entry_id: None,
                group_by: "volumes".into(),
                highlight_query: None,
                category_id: None,
                max_depth: 8,
                max_nodes: 100,
            },
            || checks.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 2,
        )
        .unwrap_err();
    assert_eq!(err.code, "cancelled");
}
