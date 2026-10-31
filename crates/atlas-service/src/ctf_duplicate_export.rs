//! Streaming export selection for read-only CTF metadata duplicate candidates.
//! It must use precisely the same partition keys as ctf_duplicates_pinned;
//! no mutable duplicate_cache_* table may be populated in linked mode.
use crate::{ctf_duplicates, optional_id, selection::Selection};
use atlas_core::{Catalog, Error, Result};
use rusqlite::types::Value as SqlValue;
use serde_json::Value;

pub(crate) struct Plan {
    pub(crate) mode: &'static str,
    query: String,
    filter: String,
    args: Vec<SqlValue>,
    volume_id: Option<i64>,
    partition: &'static str,
    label: &'static str,
}

impl Plan {
    /// Refuse ambiguous/paginated exports *before* source hashing or staging.
    pub(crate) fn parse(v: &Value) -> Result<Self> {
        if v.get("offset").is_some() || v.get("limit").is_some() {
            return Err(Error::new(
                "invalid_request",
                "Linked duplicate export always exports the full matching candidate set",
            ));
        }
        if !matches!(v.get("parent_id"), None | Some(Value::Null)) {
            return Err(Error::new(
                "invalid_request",
                "Linked duplicate export accepts query and volume scope, not parent_id",
            ));
        }
        let (mode, partition, label) = ctf_duplicates::grouping(v)?;
        let query = match v.get("query") {
            None => String::new(),
            Some(Value::String(s)) => s.clone(),
            _ => return Err(Error::new("invalid_request", "query must be a string")),
        };
        let (filter, args) = atlas_core::query::compile(&query)?;
        let volume_id = optional_id(v, "volume_id")?;
        Ok(Self {
            mode,
            query,
            filter,
            args,
            volume_id,
            partition,
            label,
        })
    }

    pub(crate) fn selection(&self, catalog: &Catalog) -> Result<Selection> {
        let table = catalog.query_table(&self.query)?;
        let mut args = self.args.clone();
        let volume_clause = if let Some(volume_id) = self.volume_id {
            args.push(SqlValue::Integer(volume_id));
            "AND volume_id = ?"
        } else {
            ""
        };
        // GROUPING OCCURS AFTER query and volume filtering, BEFORE the
        // duplicate_count > 1 predicate. The final exporter streams *all*
        // candidate entries, never a paginated GUI sample or a broader search.
        let cte = format!(
            "WITH counted AS (
                SELECT id,COUNT(*) OVER (PARTITION BY {partition}) AS duplicate_count,
                       {label} AS duplicate_group
                FROM {table}
                WHERE kind = 'file' AND ({filter}) {volume_clause}
             ), matches AS (
                SELECT e.id,e.volume_id,e.parent_id,e.name,e.relative_path,
                       e.kind,e.size,e.modified_unix,e.description,e.tags,
                       c.duplicate_count,c.duplicate_group
                FROM counted c JOIN entries e ON e.id=c.id
                WHERE c.duplicate_count > 1
             )",
            partition = self.partition,
            filter = self.filter,
            label = self.label,
        );
        Ok(Selection {
            cte,
            args,
            duplicate: true,
        })
    }
}
