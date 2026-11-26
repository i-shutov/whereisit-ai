//! Locate a recorded entry in the exact ordering used by `children`.
use atlas_core::{Catalog, Error, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashSet;

const MAX_ANCESTOR_DEPTH: usize = 4096;

pub(crate) fn page(catalog: &Catalog, request: &Value) -> Result<Value> {
    let entry_id = crate::optional_id(request, "entry_id")?
        .ok_or_else(|| Error::new("invalid_request", "Select an entry"))?;
    let volume_id = crate::optional_id(request, "volume_id")?
        .ok_or_else(|| Error::new("invalid_request", "Select a volume"))?;
    let parent_id = crate::optional_id(request, "parent_id")?;
    let limit = crate::unsigned(request, "limit", 250)?.clamp(1, 1000);
    let conn = catalog.conn();
    // Rank and page must see the same catalog snapshot.
    let tx = conn.unchecked_transaction()?;
    let table = catalog.query_table("")?;
    let target: Option<(String, String)> = tx
        .query_row(
            &format!(
                "SELECT name,kind FROM {table} WHERE id=?1 AND volume_id=?2 AND parent_id IS ?3"
            ),
            params![entry_id, volume_id, parent_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (name, kind) =
        target.ok_or_else(|| Error::new("not_found", "Entry is no longer in this folder"))?;
    let rank = if kind == "directory" { 0 } else { 1 };
    let position: i64 = tx.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE volume_id=?1 AND parent_id IS ?2 AND (CASE kind WHEN 'directory' THEN 0 ELSE 1 END < ?3 OR (CASE kind WHEN 'directory' THEN 0 ELSE 1 END = ?3 AND (name COLLATE NOCASE < ?4 COLLATE NOCASE OR (name COLLATE NOCASE = ?4 COLLATE NOCASE AND id < ?5))))"),
        params![volume_id, parent_id, rank, name, entry_id], |r| r.get(0),
    )?;
    let offset = (position as u64 / limit) * limit;
    let page = catalog.children(volume_id, parent_id, offset, limit)?;
    tx.commit()?;
    Ok(json!(page))
}

/// Return an entry's parent chain in root-to-self order. Keep this as a read
/// against the requested catalog; linked callers wrap it in `read_pinned`.
pub(crate) fn ancestors(catalog: &Catalog, request: &Value) -> Result<Value> {
    let entry_id = crate::optional_id(request, "entry_id")?
        .ok_or_else(|| Error::new("invalid_request", "Select an entry"))?;
    let volume_id = crate::optional_id(request, "volume_id")?
        .ok_or_else(|| Error::new("invalid_request", "Select a volume"))?;
    let table = catalog.query_table("")?;
    let conn = catalog.conn();
    let tx = conn.unchecked_transaction()?;
    let mut current = Some(entry_id);
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut expect_container = false;
    while let Some(id) = current {
        if items.len() >= MAX_ANCESTOR_DEPTH || !seen.insert(id) {
            return Err(Error::new(
                "ctf_tree",
                "Entry ancestry is cyclic or too deep",
            ));
        }
        let row = tx
            .query_row(
                &format!("SELECT id,volume_id,parent_id,name,kind FROM {table} WHERE id=?1"),
                [id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                if items.is_empty() {
                    Error::new("not_found", "Entry not found in requested volume")
                } else {
                    Error::new("ctf_tree", "Entry ancestry references a missing parent")
                }
            })?;
        let (row_id, row_volume, parent_id, name, kind) = row;
        if row_volume != volume_id {
            return Err(if items.is_empty() {
                Error::new("not_found", "Entry not found in requested volume")
            } else {
                Error::new("ctf_tree", "Entry ancestry crosses a volume boundary")
            });
        }
        if expect_container && !matches!(kind.as_str(), "directory" | "archive") {
            return Err(Error::new(
                "ctf_tree",
                "Entry parent cannot contain children",
            ));
        }
        if parent_id.is_some_and(|parent| parent <= 0) {
            return Err(Error::new(
                "ctf_tree",
                "Entry ancestry has an invalid parent ID",
            ));
        }
        items.push(json!({
            "id":row_id,
            "parent_id":parent_id,
            "name":name,
            "kind":kind
        }));
        expect_container = parent_id.is_some();
        current = parent_id;
    }
    items.reverse();
    tx.commit()?;
    Ok(json!({"volume_id":volume_id,"items":items}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_core::NewEntry;

    #[test]
    fn focused_page_uses_children_order_and_rejects_wrong_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::open(tmp.path().join("catalog.sqlite")).unwrap();
        let mut selected = 0;
        let volume = catalog
            .bulk_import("Synthetic", "root", |writer| {
                for n in 0..300 {
                    let name = format!("movie-{n:03}.mkv");
                    let id = writer.insert(&NewEntry {
                        parent_id: None,
                        name: name.clone(),
                        relative_path: name,
                        kind: "file".into(),
                        size: 1,
                        modified_unix: None,
                        description: String::new(),
                        tags: String::new(),
                    })?;
                    if n == 270 {
                        selected = id;
                    }
                }
                Ok(())
            })
            .unwrap();
        let request =
            json!({"entry_id":selected,"volume_id":volume.id,"parent_id":null,"limit":250});
        let result = page(&catalog, &request).unwrap();
        assert_eq!(result["offset"], 250);
        assert!(result["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == selected));
        let invalid =
            json!({"entry_id":selected,"volume_id":volume.id,"parent_id":999,"limit":250});
        assert_eq!(page(&catalog, &invalid).unwrap_err().code, "not_found");
    }

    #[test]
    fn entry_ancestors_returns_root_to_self_and_rejects_corrupt_chains() {
        let tmp = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::open(tmp.path().join("catalog.sqlite")).unwrap();
        let mut root_id = 0;
        let mut archive_id = 0;
        let mut nested_id = 0;
        let mut file_id = 0;
        let mut other_root_id = 0;
        catalog
            .bulk_import_many(|session| {
                let first = session.start_volume("first", "")?;
                root_id = session.insert(&NewEntry {
                    parent_id: None,
                    name: "root".into(),
                    relative_path: "root".into(),
                    kind: "directory".into(),
                    size: 0,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                archive_id = session.insert(&NewEntry {
                    parent_id: Some(root_id),
                    name: "inside.zip".into(),
                    relative_path: "root/inside.zip".into(),
                    kind: "archive".into(),
                    size: 0,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                nested_id = session.insert(&NewEntry {
                    parent_id: Some(archive_id),
                    name: "nested".into(),
                    relative_path: "root/inside.zip/nested".into(),
                    kind: "directory".into(),
                    size: 0,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                file_id = session.insert(&NewEntry {
                    parent_id: Some(nested_id),
                    name: "film.mkv".into(),
                    relative_path: "root/inside.zip/nested/film.mkv".into(),
                    kind: "file".into(),
                    size: 5,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                session.finish_volume()?;
                let second = session.start_volume("second", "")?;
                other_root_id = session.insert(&NewEntry {
                    parent_id: None,
                    name: "elsewhere".into(),
                    relative_path: "elsewhere".into(),
                    kind: "directory".into(),
                    size: 0,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                session.finish_volume()?;
                assert_ne!(first, second);
                Ok(())
            })
            .unwrap();

        let request = json!({"volume_id":1,"entry_id":file_id});
        let result = ancestors(&catalog, &request).unwrap();
        assert_eq!(
            result["items"],
            json!([
                {"id":root_id,"parent_id":null,"name":"root","kind":"directory"},
                {"id":archive_id,"parent_id":root_id,"name":"inside.zip","kind":"archive"},
                {"id":nested_id,"parent_id":archive_id,"name":"nested","kind":"directory"},
                {"id":file_id,"parent_id":nested_id,"name":"film.mkv","kind":"file"}
            ])
        );
        assert_eq!(
            ancestors(&catalog, &json!({"volume_id":2,"entry_id":file_id}))
                .unwrap_err()
                .code,
            "not_found"
        );
        assert_eq!(
            ancestors(&catalog, &json!({"volume_id":1,"entry_id":999999}))
                .unwrap_err()
                .code,
            "not_found"
        );

        catalog
            .conn()
            .execute(
                "UPDATE entries_core SET parent_id=?1 WHERE id=?2",
                params![other_root_id, file_id],
            )
            .unwrap();
        assert_eq!(ancestors(&catalog, &request).unwrap_err().code, "ctf_tree");
        catalog
            .conn()
            .execute(
                "UPDATE entries_core SET parent_id=?1 WHERE id=?2",
                params![nested_id, file_id],
            )
            .unwrap();
        catalog
            .conn()
            .execute(
                "UPDATE entries_core SET parent_id=?1 WHERE id=?2",
                params![root_id, root_id],
            )
            .unwrap();
        assert_eq!(ancestors(&catalog, &request).unwrap_err().code, "ctf_tree");

        let missing_db = tmp.path().join("missing-read-only.sqlite");
        let response = crate::dispatch_json(
            &json!({"op":"entry_ancestors","db":missing_db,"volume_id":1,"entry_id":1}).to_string(),
        );
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["ok"], false);
        assert!(!missing_db.exists());
    }
}
