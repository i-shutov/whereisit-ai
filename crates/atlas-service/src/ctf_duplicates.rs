//! Read-only duplicate candidates from a source-verified CTF mirror.
//! No writeable duplicate cache, native annotations or content hash claims.
//! The grouping is query-scoped and uses the same normalized keys as SQLite
//! duplicate candidates, but performs a single read-only window query.
use crate::{ctf_mirror, optional_id, unsigned};
use atlas_core::{Error, Result};
use rusqlite::types::Value as SqlValue;
use serde_json::{json, Value};

/// Canonical grouping specification shared by linked duplicate *pages* and
/// linked duplicate *exports*, so they cannot silently disagree on membership.
pub(crate) fn grouping(v: &Value) -> Result<(&'static str, &'static str, &'static str)> {
    let mode = match v.get("duplicate_mode") {
        None => "name_size",
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(Error::new(
                "invalid_request",
                "duplicate_mode must be a string",
            ))
        }
    };
    match mode {
        "name_size" => Ok(("name_size", "name_norm,size", "name_norm || ':' || size")),
        "name" => Ok(("name", "name_norm", "name_norm")),
        "size" => Ok(("size", "size", "CAST(size AS TEXT)")),
        "hash" => Err(Error::new(
            "unsupported",
            "File content hashes are not available for offline CTF entries",
        )),
        _ => Err(Error::new(
            "invalid_request",
            "Duplicate mode must be name_size, name or size",
        )),
    }
}

pub fn duplicates(v: &Value) -> Result<Value> {
    let (mode, partition, label) = grouping(v)?;
    // User query errors must fail before hashing any potentially huge source.
    let query = match v.get("query") {
        None => "",
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(Error::new("invalid_request", "query must be a string")),
    };
    let (filter, mut args) = atlas_core::query::compile(query)?;
    let volume = optional_id(v, "volume_id")?;
    let restrict_volume = if let Some(volume) = volume {
        args.push(SqlValue::Integer(volume));
        "AND volume_id = ?"
    } else {
        ""
    };
    let offset = unsigned(v, "offset", 0)?.min(i64::MAX as u64);
    let limit = unsigned(v, "limit", 200)?.clamp(1, 1000);
    let fetch = limit + 1;
    args.push(SqlValue::Integer(fetch as i64));
    args.push(SqlValue::Integer(offset as i64));

    ctf_mirror::read_pinned(v, |catalog| {
        let table = catalog.query_table(query)?;
        // Window counts every *matching file* before pagination. Unlike
        // duplicate_cache_members this SELECT never updates the immutable
        // mirror. For cold multi-million-row input it may be expensive; the
        // UI must run it on a worker and must not claim a warm-query SLO.
        let sql = format!(
            "WITH counted AS (
                SELECT id, volume_id, relative_path, name_norm, size,
                    COUNT(*) OVER (PARTITION BY {partition}) AS duplicate_count
                FROM {table}
                WHERE kind = 'file' AND ({filter}) {restrict_volume}
             ),
             page AS MATERIALIZED (
                SELECT id, volume_id, relative_path, duplicate_count,
                    {label} AS duplicate_group
                FROM counted WHERE duplicate_count > 1
                ORDER BY duplicate_group COLLATE NOCASE, volume_id, relative_path, id
                LIMIT ? OFFSET ?
             )
             SELECT e.id,e.volume_id,e.parent_id,e.name,e.relative_path,e.kind,
                    e.size,e.modified_unix,e.description,e.tags,
                    p.duplicate_count,p.duplicate_group
             FROM page p JOIN entries e ON e.id=p.id
             ORDER BY p.duplicate_group COLLATE NOCASE,p.volume_id,p.relative_path,p.id"
        );
        let mut statement = catalog.conn().prepare(&sql)?;
        let mut items = statement
            .query_map(rusqlite::params_from_iter(args.iter()), |row| {
                let entry = atlas_core::entry_row(row)?;
                let mut result = serde_json::to_value(entry).expect("Entry serializes");
                result["duplicate_count"] = json!(row.get::<_, i64>(10)?);
                result["duplicate_group"] = json!(row.get::<_, String>(11)?);
                Ok(result)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = items.len() > limit as usize;
        if has_more {
            items.pop();
        }
        drop(statement);
        Ok(json!({
            "items": items,
            "total": null,
            "offset": offset,
            "limit": limit,
            "has_more": has_more,
            "verified": false,
            "basis": "metadata_candidates",
            "duplicate_mode": mode,
            "read_only": true
        }))
    })
}
