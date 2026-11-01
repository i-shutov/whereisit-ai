//! Read-only, per-selected-entry metadata copy navigation.
//! NOT a content-hash match: the selected entry determines the comparison key,
//! not user-entered query text or historical drive letters.
use crate::{ctf_mirror, optional_id, unsigned};
use atlas_core::{Catalog, Error, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

pub(crate) fn page(catalog: &Catalog, request: &Value) -> Result<Value> {
    let entry_id = optional_id(request, "entry_id")?
        .ok_or_else(|| Error::new("invalid_request", "Select a positive file entry_id"))?;
    let offset = unsigned(request, "offset", 0)?.min(i64::MAX as u64);
    let limit = unsigned(request, "limit", 200)?.clamp(1, 1000);
    // Indexed base table for identity/count; full entries view for the page.
    // Both SQLite v1 and v3 schemas are supported without changing the source.
    let table = catalog.query_table("")?;
    let selected: Option<(String, i64, i64)> = catalog
        .conn()
        .query_row(
            &format!("SELECT name_norm,size,volume_id FROM {table} WHERE id=?1 AND kind='file'"),
            [entry_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (name_key, size, source_volume_id) = selected.ok_or_else(|| {
        Error::new(
            "not_found",
            "Selected file is unavailable in this catalog snapshot",
        )
    })?;
    let candidate_volume_id = optional_id(request, "candidate_volume_id")?;
    // All-match disk counts are independent of page focus; catalog labels are
    // not evidence that any USB disk is currently connected.
    let mut group_stmt = catalog.conn().prepare(&format!(
        "SELECT e.volume_id,v.label,COUNT(*) FROM {table} e
         LEFT JOIN volumes v ON v.id=e.volume_id
         WHERE e.kind='file' AND e.name_norm=?1 AND e.size=?2
         GROUP BY e.volume_id ORDER BY e.volume_id"
    ))?;
    let disk_groups: Vec<Value> = group_stmt
        .query_map(params![&name_key, size], |row| {
            let id: i64 = row.get(0)?;
            let label: Option<String> = row.get(1)?;
            let count: i64 = row.get(2)?;
            Ok(json!({"volume_id":id,
                "label":label.unwrap_or_else(|| format!("Disk #{id}")),
                "count":count}))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let all_total: i64 = disk_groups.iter().filter_map(|v| v["count"].as_i64()).sum();
    let distinct_disks = disk_groups.len();
    let total = candidate_volume_id.map_or(all_total, |focused| {
        disk_groups
            .iter()
            .find(|group| group["volume_id"] == focused)
            .and_then(|group| group["count"].as_i64())
            .unwrap_or(0)
    });
    let focus_clause = if candidate_volume_id.is_some() {
        " AND volume_id=?3"
    } else {
        ""
    };
    let limit_param = if candidate_volume_id.is_some() { 4 } else { 3 };
    let offset_param = limit_param + 1;
    let sql = format!(
        "SELECT id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,\
         description,tags FROM entries \
         WHERE kind='file' AND name_norm=?1 AND size=?2{focus_clause} \
         ORDER BY volume_id,relative_path,id LIMIT ?{limit_param} OFFSET ?{offset_param}"
    );
    let mut parameters = vec![
        rusqlite::types::Value::Text(name_key),
        rusqlite::types::Value::Integer(size),
    ];
    if let Some(volume_id) = candidate_volume_id {
        parameters.push(rusqlite::types::Value::Integer(volume_id));
    }
    parameters.push(rusqlite::types::Value::Integer(limit as i64));
    parameters.push(rusqlite::types::Value::Integer(offset as i64));
    let mut stmt = catalog.conn().prepare(&sql)?;
    let items = stmt
        .query_map(rusqlite::params_from_iter(parameters), |row| {
            let entry = atlas_core::entry_row(row)?;
            let mut value = serde_json::to_value(entry).expect("Entry serializes");
            // Count always refers to the all-disk metadata class.
            value["duplicate_count"] = json!(all_total);
            Ok(value)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(json!({
        "items": items, "total": total, "all_total": all_total,
        "offset": offset, "limit": limit,
        "has_more": offset.saturating_add(limit) < total as u64,
        "source_entry_id": entry_id, "source_volume_id": source_volume_id,
        "candidate_volume_id":candidate_volume_id,
        "distinct_disks": distinct_disks, "disk_groups": disk_groups,
        "basis": "exact_normalized_name_and_size", "verified": false,
        "read_only": true
    }))
}

/// Every linked page independently rechecks the CTF generation SHA.
pub(crate) fn linked(request: &Value) -> Result<Value> {
    ctf_mirror::read_pinned(request, |catalog| page(catalog, request))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_core::NewEntry;

    fn file(name: &str, size: u64, kind: &str) -> NewEntry {
        NewEntry {
            parent_id: None,
            name: name.into(),
            relative_path: name.into(),
            kind: kind.into(),
            size,
            modified_unix: None,
            description: String::new(),
            tags: String::new(),
        }
    }

    #[test]
    fn selected_file_returns_paginated_cross_disk_candidates_not_similar_names() {
        let tmp = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::open(tmp.path().join("catalog.sqlite")).unwrap();
        let mut selected = 0;
        let mut folder = 0;
        catalog
            .bulk_import("Shelf A", "disk-A", |writer| {
                selected = writer.insert(&file("Film.MKV", 512, "file"))?;
                writer.insert(&file("film.mkv", 512, "file"))?;
                writer.insert(&file("Film.MKV-other", 512, "file"))?;
                folder = writer.insert(&file("Films", 0, "directory"))?;
                Ok(())
            })
            .unwrap();
        catalog
            .bulk_import("Shelf B", "disk-B", |writer| {
                writer.insert(&file("film.mkv", 512, "file"))?;
                writer.insert(&file("film.mkv", 513, "file"))?;
                Ok(())
            })
            .unwrap();
        let first = page(&catalog, &json!({"entry_id":selected,"offset":0,"limit":2})).unwrap();
        assert_eq!(first["total"], 3);
        assert_eq!(first["distinct_disks"], 2);
        assert_eq!(first["disk_groups"].as_array().unwrap().len(), 2);
        assert_eq!(first["disk_groups"][0]["label"], "Shelf A");
        assert_eq!(first["disk_groups"][0]["count"], 2);
        assert_eq!(first["disk_groups"][1]["label"], "Shelf B");
        assert_eq!(first["disk_groups"][1]["count"], 1);
        assert_eq!(first["all_total"], 3);
        assert_eq!(first["items"].as_array().unwrap().len(), 2);
        assert_eq!(first["has_more"], true);
        assert_eq!(first["verified"], false);
        assert_eq!(first["basis"], "exact_normalized_name_and_size");
        assert_eq!(first["items"][0]["duplicate_count"], 3);
        let second = page(&catalog, &json!({"entry_id":selected,"offset":2,"limit":2})).unwrap();
        assert_eq!(second["items"].as_array().unwrap().len(), 1);
        assert_eq!(second["has_more"], false);
        let ids = first["items"]
            .as_array()
            .unwrap()
            .iter()
            .chain(second["items"].as_array().unwrap().iter())
            .map(|r| r["id"].as_i64().unwrap())
            .collect::<Vec<_>>();
        let disk_b = first["disk_groups"][1]["volume_id"].as_i64().unwrap();
        let focused = page(
            &catalog,
            &json!({"entry_id":selected,
            "candidate_volume_id":disk_b,"offset":0,"limit":2}),
        )
        .unwrap();
        assert_eq!(focused["total"], 1);
        assert_eq!(focused["all_total"], 3);
        assert_eq!(focused["distinct_disks"], 2);
        assert_eq!(focused["has_more"], false);
        assert_eq!(focused["items"][0]["volume_id"], disk_b);
        assert_eq!(focused["items"][0]["duplicate_count"], 3);
        assert!(page(
            &catalog,
            &json!({"entry_id":selected,
            "candidate_volume_id":0})
        )
        .is_err());
        assert!(ids.contains(&selected));
        assert_eq!(ids.len(), 3);
        assert!(page(&catalog, &json!({"entry_id":folder})).is_err());
        assert!(page(&catalog, &json!({"entry_id":-1})).is_err());
        assert!(page(&catalog, &json!({"entry_id":999999})).is_err());
    }
}
