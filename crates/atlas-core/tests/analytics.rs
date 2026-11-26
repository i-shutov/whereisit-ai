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

fn search_totals(catalog: &Catalog, query: &str, volume_id: Option<i64>) -> (u64, u64) {
    let page = catalog.search(query, volume_id, 0, 100).unwrap();
    (page.total, page.items.iter().map(|entry| entry.size).sum())
}

fn analytics_request(
    group_by: &str,
    highlight_query: &str,
    volume_id: Option<i64>,
    folder_entry_id: Option<i64>,
    category_id: Option<i64>,
) -> AnalyticsRequest {
    AnalyticsRequest {
        volume_id,
        folder_entry_id,
        group_by: group_by.into(),
        highlight_query: Some(highlight_query.into()),
        category_id,
        max_depth: 8,
        max_nodes: 100,
    }
}

#[test]
fn analytics_highlights_use_query_compatible_metadata_tables_and_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("analytics-highlight.sqlite");
    let mut catalog = Catalog::open(&db).unwrap();
    let mut branch = 0;
    let mut sigma_file = 0;
    let mut tag_file = 0;
    let mut outside_file = 0;
    catalog
        .bulk_import("Synthetic", "offline", |writer| {
            branch = writer.insert(&item(None, "Branch", "Branch", "directory", 0))?;
            sigma_file = writer.insert(&item(
                Some(branch),
                "Σigma.txt",
                "Branch/Σigma.txt",
                "file",
                17,
            ))?;
            tag_file = writer.insert(&item(
                Some(branch),
                "tagged.txt",
                "Branch/tagged.txt",
                "file",
                23,
            ))?;
            outside_file = writer.insert(&item(None, "outside.txt", "outside.txt", "file", 31))?;
            Ok(())
        })
        .unwrap();
    catalog
        .update_entry(sigma_file, "café description", "blue label")
        .unwrap();
    catalog
        .update_entry(tag_file, "plain description", "café tag")
        .unwrap();
    catalog
        .update_entry(outside_file, "café description", "green")
        .unwrap();
    let category = catalog.create_category("Synthetic category", None).unwrap();
    catalog.assign_category(sigma_file, category).unwrap();
    drop(catalog);

    let catalog = Catalog::open_readonly(&db).unwrap();
    let volume_id = 1;

    let description_query = "description:café OR name:Σ";
    let expected = search_totals(&catalog, description_query, Some(volume_id));
    assert_eq!(expected, (2, 48));
    let description = catalog
        .analytics(
            &analytics_request("volumes", description_query, Some(volume_id), None, None),
            || false,
        )
        .unwrap();
    assert_eq!(
        (
            description.root.matched_files,
            description.root.matched_bytes
        ),
        expected
    );

    let tag_query = "tag:café";
    let expected = search_totals(&catalog, tag_query, Some(volume_id));
    assert_eq!(expected, (1, 23));
    let tags = catalog
        .analytics(
            &analytics_request("extensions", tag_query, Some(volume_id), None, None),
            || false,
        )
        .unwrap();
    assert_eq!((tags.root.matched_files, tags.root.matched_bytes), expected);
    let text_group = tags
        .root
        .children
        .iter()
        .find(|group| group.id == "ext:txt")
        .unwrap();
    assert_eq!(
        (text_group.matched_files, text_group.matched_bytes),
        expected
    );

    let default_text_query = "café";
    let expected = search_totals(&catalog, default_text_query, Some(volume_id));
    assert_eq!(expected, (3, 71));
    let default_text = catalog
        .analytics(
            &analytics_request(
                "categories",
                default_text_query,
                Some(volume_id),
                None,
                None,
            ),
            || false,
        )
        .unwrap();
    assert_eq!(
        (
            default_text.root.matched_files,
            default_text.root.matched_bytes
        ),
        expected
    );

    let category_query = format!("({description_query}) AND category_id:{category}");
    let expected = search_totals(&catalog, &category_query, Some(volume_id));
    assert_eq!(expected, (1, 17));
    let categorized = catalog
        .analytics(
            &analytics_request(
                "categories",
                description_query,
                Some(volume_id),
                None,
                Some(category),
            ),
            || false,
        )
        .unwrap();
    assert_eq!(
        (
            categorized.root.matched_files,
            categorized.root.matched_bytes
        ),
        expected
    );
    let category_group = categorized
        .root
        .children
        .iter()
        .find(|group| group.id == format!("category:{category}"))
        .unwrap();
    assert_eq!(
        (category_group.matched_files, category_group.matched_bytes),
        expected
    );

    let branch_query = format!("branch_id:{branch}");
    let expected = search_totals(&catalog, &branch_query, Some(volume_id));
    assert_eq!(expected, (2, 40));
    let branch_data = catalog
        .analytics(
            &analytics_request(
                "volumes",
                &branch_query,
                Some(volume_id),
                Some(branch),
                None,
            ),
            || false,
        )
        .unwrap();
    assert_eq!(
        (
            branch_data.root.matched_files,
            branch_data.root.matched_bytes
        ),
        expected
    );
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
