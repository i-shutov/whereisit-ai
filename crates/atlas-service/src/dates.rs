//! Display legacy DOS wall-clock timestamps without inventing a timezone.
use atlas_core::{Catalog, Result};
use chrono::NaiveDate;
use rusqlite::params_from_iter;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Decode the DOS date/time word as local calendar fields, with no UTC suffix.
/// Invalid dates (including the zero sentinel) remain unknown.
pub fn dos_local(raw: u32) -> Option<String> {
    let date = raw >> 16;
    let time = raw & 0xffff;
    let year = 1980 + ((date >> 9) & 127) as i32;
    let month = (date >> 5) & 15;
    let day = date & 31;
    let hour = (time >> 11) & 31;
    let minute = (time >> 5) & 63;
    let second = (time & 31) * 2;
    NaiveDate::from_ymd_opt(year, month, day)?
        .and_hms_opt(hour, minute, second)
        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// Annotate a paged `data.items` response with original CTF local timestamps.
/// Uses a single bound-ID lookup for the page, not one query per entry.
pub fn annotate(c: &Catalog, data: &mut Value) -> Result<()> {
    let Some(items) = data.get_mut("items").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    if items.is_empty() {
        return Ok(());
    }
    let exists: bool = c.conn().query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ctf_entries')",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(());
    }
    let mut seen = HashSet::new();
    let ids: Vec<i64> = items
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_i64))
        .filter(|id| seen.insert(*id))
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut statement = c.conn().prepare(&format!(
        "SELECT entry_id,modified_dos FROM ctf_entries WHERE entry_id IN ({placeholders})"
    ))?;
    let rows = statement.query_map(params_from_iter(ids.iter()), |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, u32>(1)?))
    })?;
    let mut local = HashMap::new();
    for row in rows {
        let (id, raw) = row?;
        if let Some(date) = dos_local(raw) {
            local.insert(id, date);
        }
    }
    for item in items {
        let date = item
            .get("id")
            .and_then(Value::as_i64)
            .and_then(|id| local.get(&id));
        if let Some(date) = date {
            item["modified_local"] = Value::String(date.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dos_is_a_wall_clock_not_a_utc_instant() {
        let raw = (((2026 - 1980) << 9 | 10 << 5 | 28) << 16) | (23 << 11 | 59 << 5 | 29);
        assert_eq!(dos_local(raw).as_deref(), Some("2026-10-28 23:59:58"));
        assert_eq!(dos_local(0), None);
        let invalid = (((2025 - 1980) << 9 | 2 << 5 | 29) << 16) | (12 << 11);
        assert_eq!(dos_local(invalid), None);
    }
}
