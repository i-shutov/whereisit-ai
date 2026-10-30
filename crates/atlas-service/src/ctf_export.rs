//! Export a verified linked CTF snapshot without writing the original or its mirror.
//! Publication is deliberately no-clobber: an existing output is never replaced.
use crate::{ctf_duplicate_export, ctf_mirror, exports, selection, text};
use atlas_core::{Error, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

fn invalid(message: &'static str) -> Error {
    Error::new("invalid_request", message)
}

fn digest(path: &Path) -> Result<(String, u64)> {
    let mut file = File::open(path)?;
    let mut sha = Sha256::new();
    let mut length = 0u64;
    let mut block = [0u8; 256 * 1024];
    loop {
        let bytes = file.read(&mut block)?;
        if bytes == 0 {
            break;
        }
        length = length
            .checked_add(bytes as u64)
            .ok_or_else(|| Error::new("export", "Export length overflow"))?;
        sha.update(&block[..bytes]);
    }
    Ok((format!("{:x}", sha.finalize()), length))
}

/// The input `path` is an authoritative .CTF. `output` is a *new* CSV,
/// Parquet or JSON file. Even `overwrite=true` is rejected in linked mode.
/// No row ID is used without a successful source-generation pin check.
pub(crate) fn export(v: &Value) -> Result<Value> {
    let format = text(v, "format")?;
    let operation = match format {
        "csv" => "export_csv",
        "parquet" => "export_parquet",
        "json" => "export_json",
        _ => {
            return Err(invalid(
                "Linked CTF export supports csv, parquet or json only",
            ))
        }
    };
    if v.get("overwrite").and_then(Value::as_bool) == Some(true) {
        return Err(invalid(
            "Linked CTF exports never overwrite an existing file",
        ));
    }
    // Validate the full Boolean query and scope before hashing a large CTF.
    let ordinary_selection = selection::select(v, false)?;
    let mode = v.get("mode").and_then(Value::as_str).unwrap_or("search");
    if !matches!(mode, "search" | "children" | "all" | "duplicates") {
        return Err(invalid(
            "Linked CTF export supports search, children, all or duplicates",
        ));
    }
    let duplicate_plan = if mode == "duplicates" {
        Some(ctf_duplicate_export::Plan::parse(v)?)
    } else {
        if ordinary_selection.duplicate {
            return Err(invalid("Invalid linked CTF export selection"));
        }
        None
    };
    let source = PathBuf::from(text(v, "path")?);
    let output = PathBuf::from(text(v, "output")?);
    if output
        .extension()
        .and_then(|ext| ext.to_str())
        .is_none_or(|ext| !ext.eq_ignore_ascii_case(format))
    {
        return Err(invalid("Output filename must match the requested format"));
    }
    let parent = output
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let name = output
        .file_name()
        .ok_or_else(|| invalid("Output filename is required"))?;
    let target = parent.join(name);
    // The existing CTF and all immutable mirror-generation files are excluded
    // even if the output parent was supplied through a symlink.
    let original = source.canonicalize()?;
    let cache = PathBuf::from(text(v, "cache_root")?).canonicalize()?;
    if target == original || target.starts_with(&cache) {
        return Err(Error::new(
            "unsafe_output",
            "CTF export cannot target the original catalog or mirror cache",
        ));
    }
    if target.exists() || target.is_symlink() {
        return Err(Error::new(
            "file_exists",
            "Output already exists; choose a new filename",
        ));
    }

    // A second temporary writer is used by the existing streaming CSV/Parquet
    // exporter. Its finished bytes replace this owned staging name only; the
    // user destination is untouched until read_pinned validates both source
    // SHA and the immutable SQLite stamp *after* all rows have been streamed.
    let stage = tempfile::Builder::new()
        .prefix(".whereisit-ctf-export-")
        .tempfile_in(&parent)?
        .into_temp_path();
    let generation = text(v, "generation")?;
    let mut args = v.clone();
    args["path"] = json!(stage.to_string_lossy().as_ref());
    args["db"] = json!(cache.join(generation).join("catalog.sqlite"));
    args["overwrite"] = json!(true);
    let mut verified = ctf_mirror::read_pinned(v, |catalog| {
        if let Some(plan) = &duplicate_plan {
            exports::export_pinned_selected(catalog, &args, operation, plan.selection(catalog)?)
        } else {
            exports::export(catalog, &args, operation)
        }
    })?;

    let (sha256, bytes) = digest(&stage)?;
    if verified["result"]["bytes"].as_u64() != Some(bytes) || !verified["result"]["rows"].is_u64() {
        return Err(Error::new("export", "Staged export integrity check failed"));
    }
    // Atomic no-clobber final publication; a raced existing target is refused.
    stage.persist_noclobber(&target).map_err(|e| {
        let code = if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            "file_exists"
        } else {
            "export"
        };
        Error::new(
            code,
            format!("Export publication failed: {:?}", e.error.kind()),
        )
    })?;
    verified["result"]["path"] = json!(target);
    verified["result"]["sha256"] = json!(sha256);
    verified["result"]["source_snapshot_verified_before_publication"] = json!(true);
    verified["result"]["original_ctf_read_only"] = json!(true);
    verified["result"]["output_no_clobber"] = json!(true);
    if let Some(plan) = &duplicate_plan {
        verified["result"]["verified"] = json!(false);
        verified["result"]["basis"] = json!("metadata_candidates");
        verified["result"]["duplicate_mode"] = json!(plan.mode);
    }
    Ok(verified)
}
