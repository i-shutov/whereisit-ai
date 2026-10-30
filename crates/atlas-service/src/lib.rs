//! Shared command service for native clients and CLI.
use atlas_core::{Catalog, Error, Result};
use serde_json::{json, Value};
mod ctf_annotations;
mod ctf_duplicate_export;
mod ctf_duplicates;
mod ctf_export;
mod ctf_mirror;
#[path = "../ctf_sidecar/mod.rs"]
mod ctf_sidecar;
mod dates;
mod duplicates;
mod exports;
mod legacy;
mod maintenance;
mod selection;

/// The actual SQLite library linked into each installed native executable.
/// Does not open any catalog or access the filesystem.
pub fn embedded_sqlite_version() -> &'static str {
    rusqlite::version()
}

pub fn dispatch_json(input: &str) -> String {
    let result = std::panic::catch_unwind(|| dispatch(input));
    match result {
        Ok(Ok(value)) => value.to_string(),
        Ok(Err(e)) => json!({"ok":false,"error":{"code":e.code,"message":e.message}}).to_string(),
        Err(_) => json!({"ok":false,"error":{"code":"internal","message":"An internal error was contained"}}).to_string(),
    }
}

fn dispatch(input: &str) -> Result<Value> {
    if input.len() > 1024 * 1024 {
        return Err(Error::new("invalid_request", "Request exceeds 1 MiB"));
    }
    let v: Value = serde_json::from_str(input).map_err(|e| Error::new("invalid_json", e))?;
    for key in ["mode", "query", "duplicate_mode"] {
        if let Some(value) = v.get(key) {
            if !value.is_string() {
                return Err(Error::new(
                    "invalid_request",
                    format!("{key} must be a string when provided"),
                ));
            }
        }
    }
    for key in ["overwrite", "bom", "allow_lossy"] {
        if let Some(value) = v.get(key) {
            if !value.is_boolean() {
                return Err(Error::new(
                    "invalid_request",
                    format!("{key} must be a boolean when provided"),
                ));
            }
        }
    }
    let op = v
        .get("op")
        .or_else(|| v.get("command"))
        .or_else(|| v.get("cmd"))
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("invalid_request", "op must be a string"))?;
    let mut data = match op {
        "ctf_sync" => ctf_mirror::sync(&v)?,
        "ctf_search" => ctf_mirror::search(&v)?,
        "ctf_search_pinned" => ctf_mirror::search_pinned(&v)?,
        "ctf_volumes_pinned" => ctf_mirror::volumes_pinned(&v)?,
        "ctf_children_pinned" => ctf_mirror::children_pinned(&v)?,
        "ctf_export_pinned" => ctf_export::export(&v)?,
        "ctf_annotations_list_pinned" => ctf_annotations::list(&v)?,
        "ctf_annotations_preview_pinned" => ctf_annotations::preview(&v)?,
        "ctf_annotations_set_pinned" => ctf_annotations::set(&v)?,
        "ctf_duplicates_pinned" => ctf_duplicates::duplicates(&v)?,
        "compact_catalog" => maintenance::compact_copy(&v)?,
        "validate_catalog" => maintenance::validate(&v)?,
        "import_ctf" => legacy::import(&mut Catalog::open(text(&v, "db")?)?, &v)?,
        "export_ctf" => legacy::export(&Catalog::open(text(&v, "db")?)?, &v)?,
        "ctf_info" => legacy::info(&v)?,
        "duplicates" => duplicates::duplicates(&Catalog::open(text(&v, "db")?)?, &v)?,
        "export_csv" | "export_parquet" | "export_json" => {
            exports::export(&Catalog::open(text(&v, "db")?)?, &v, op)?
        }
        _ => {
            let mut response: Value = serde_json::from_str(&atlas_core::api::dispatch_json(input))
                .map_err(|e| Error::new("internal", e))?;
            if matches!(op, "children" | "search" | "search_fast") && response["ok"] == true {
                if let Some(data) = response.get_mut("data") {
                    dates::annotate(&Catalog::open(text(&v, "db")?)?, data)?;
                }
            }
            return Ok(response);
        }
    };
    if op == "duplicates" {
        dates::annotate(&Catalog::open(text(&v, "db")?)?, &mut data)?;
    }
    Ok(json!({"ok":true,"data":data}))
}

pub(crate) fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("invalid_request", format!("{key} must be a string")))
}
pub(crate) fn optional_id(v: &Value, key: &str) -> Result<Option<i64>> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(x) => x.as_i64().filter(|x| *x > 0).map(Some).ok_or_else(|| {
            Error::new(
                "invalid_request",
                format!("{key} must be a positive integer or null"),
            )
        }),
    }
}
pub(crate) fn unsigned(v: &Value, key: &str, default: u64) -> Result<u64> {
    match v.get(key) {
        None => Ok(default),
        Some(x) => x.as_u64().ok_or_else(|| {
            Error::new(
                "invalid_request",
                format!("{key} must be a non-negative integer"),
            )
        }),
    }
}
