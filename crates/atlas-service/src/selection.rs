use crate::optional_id;
use atlas_core::{Error, Result};
use rusqlite::types::Value as SqlValue;
use serde_json::Value;

pub(crate) struct Selection {
    pub cte: String,
    pub args: Vec<SqlValue>,
    pub duplicate: bool,
    pub candidate: bool,
}
pub(crate) const COLUMNS: &str =
    "id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,description,tags";

/// All predicates are generated from a bounded parser and values are bound.
/// A branch export includes the selected folder and every descendant.
pub(crate) fn select(v: &Value, force_duplicates: bool) -> Result<Selection> {
    let mode = match v.get("mode") {
        None => "search",
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(Error::new("invalid_request", "mode must be a string")),
    };
    if !matches!(
        mode,
        "search"
            | "children"
            | "duplicates"
            | "all"
            | "name_size"
            | "size"
            | "name"
            | "copy_candidates"
    ) {
        return Err(Error::new("invalid_request", "Unknown selection mode"));
    }
    let query = match v.get("query") {
        None => "",
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(Error::new("invalid_request", "query must be a string")),
    };
    let (filter, mut args) = atlas_core::query::compile(query)?;
    if mode == "copy_candidates" {
        if force_duplicates {
            return Err(Error::new(
                "invalid_request",
                "copy_candidates cannot use the duplicate-cache selection mode",
            ));
        }
        if !query.is_empty() {
            return Err(Error::new(
                "invalid_request",
                "copy_candidates is selected by entry_id and does not accept query text",
            ));
        }
        if v.get("volume_id").is_some() || v.get("parent_id").is_some() {
            return Err(Error::new(
                "invalid_request",
                "copy_candidates uses entry_id and candidate_volume_id scopes only",
            ));
        }
        let entry_id = optional_id(v, "entry_id")?
            .ok_or_else(|| Error::new("invalid_request", "Select a positive file entry_id"))?;
        let candidate_volume_id = optional_id(v, "candidate_volume_id")?;
        let other_disks_only = match v.get("other_disks_only") {
            None => false,
            Some(Value::Bool(flag)) => *flag,
            _ => {
                return Err(Error::new(
                    "invalid_request",
                    "other_disks_only must be a boolean",
                ));
            }
        };
        let mut predicates = vec!["e.kind='file'".to_owned()];
        if other_disks_only {
            predicates.push("e.volume_id<>k.source_volume_id".to_owned());
        }
        if let Some(volume_id) = candidate_volume_id {
            predicates.push(format!("e.volume_id=?{}", args.len() + 2));
            args.push(volume_id.into());
        }
        args.insert(0, entry_id.into());
        let cte = format!(
            "WITH selected_key AS (SELECT name_norm,size,volume_id AS source_volume_id FROM entries WHERE id=?1 AND kind='file'), matches AS (SELECT e.*,0 AS duplicate_count,'' AS duplicate_group FROM entries e JOIN selected_key k ON e.name_norm=k.name_norm AND e.size=k.size WHERE {})",
            predicates.join(" AND ")
        );
        return Ok(Selection {
            cte,
            args,
            duplicate: false,
            candidate: true,
        });
    }
    let mut clauses = vec![filter];
    if let Some(id) = optional_id(v, "volume_id")? {
        clauses.push("volume_id=?".to_owned());
        args.push(id.into());
    }
    let mut branch = String::new();
    if mode == "children" {
        if let Some(parent) = optional_id(v, "parent_id")? {
            // The branch seed is an integer literal produced from a validated i64.
            branch = format!(
                "branch(id) AS (SELECT id FROM entries WHERE id={parent} UNION SELECT e.id FROM entries e JOIN branch b ON e.parent_id=b.id),"
            );
            clauses.push("id IN (SELECT id FROM branch)".into());
        } else if optional_id(v, "volume_id")?.is_none() {
            return Err(Error::new(
                "invalid_request",
                "A branch needs volume_id or parent_id",
            ));
        }
    }
    let duplicate = force_duplicates || mode == "duplicates";
    if duplicate {
        // The export caller must first build the requested cache and hold the
        // read snapshot returned by duplicates::export_snapshot.
        return Ok(Selection { cte: "WITH matches AS (SELECT e.*,g.duplicate_count,g.duplicate_group FROM duplicate_cache_members m JOIN entries e ON e.id=m.entry_id JOIN duplicate_cache_groups g ON g.id=m.group_id)".into(), args: Vec::new(), duplicate: true, candidate: false });
    }
    let where_sql = clauses.join(" AND ");
    let cte = format!(
        "WITH RECURSIVE {branch} selected AS (SELECT * FROM entries WHERE {where_sql}), matches AS (SELECT *,0 AS duplicate_count,'' AS duplicate_group FROM selected)"
    );
    Ok(Selection {
        cte,
        args,
        duplicate,
        candidate: false,
    })
}
