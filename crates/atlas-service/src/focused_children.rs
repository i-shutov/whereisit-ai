//! Locate a recorded entry in the exact ordering used by `children`.
use atlas_core::{Catalog, Error, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

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
}
