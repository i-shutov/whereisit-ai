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
    Ok((crate::digest_hex(sha.finalize()), length))
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
            ));
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
    if !matches!(
        mode,
        "search" | "children" | "all" | "duplicates" | "copy_candidates"
    ) {
        return Err(invalid(
            "Linked CTF export supports search, children, all, duplicates or copy_candidates",
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
        let mut result = if let Some(plan) = &duplicate_plan {
            exports::export_pinned_selected(
                catalog,
                &args,
                operation,
                plan.selection(catalog)?,
                true,
            )
        } else {
            exports::export_pinned_selected(catalog, &args, operation, ordinary_selection, true)
        }?;
        let (sha256, bytes) = digest(&stage)?;
        if result["bytes"].as_u64() != Some(bytes) || !result["rows"].is_u64() {
            return Err(Error::new("export", "Staged export integrity check failed"));
        }
        result["sha256"] = json!(sha256);
        Ok(result)
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind};
    use sha2::{Digest, Sha256};
    use std::fs;

    fn synthetic_entry(name: &str) -> CtfEntry {
        CtfEntry {
            name: name.into(),
            description: String::new(),
            parent: Some(0),
            kind: EntryKind::File,
            size: 900,
            attributes: 32,
            modified_dos: 0,
            created_dos: 0,
            accessed_dos: 0,
            opaque: Vec::new(),
        }
    }

    fn synthetic_root() -> CtfEntry {
        CtfEntry {
            name: "Root".into(),
            description: String::new(),
            parent: None,
            kind: EntryKind::Directory,
            size: 0,
            attributes: 16,
            modified_dos: 0,
            created_dos: 0,
            accessed_dos: 0,
            opaque: Vec::new(),
        }
    }

    #[test]
    fn linked_candidate_export_verifies_source_before_publish_and_preserves_ctf() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source.ctf");
        let output = tmp.path().join("candidates.csv");
        let cache_root = tmp.path().join("cache");
        fs::create_dir(&cache_root).unwrap();
        let ctf = CtfCatalog {
            name: "Synthetic candidate export fixture".into(),
            disks: vec![
                CtfDisk {
                    name: "Disk A".into(),
                    entries: vec![synthetic_root(), synthetic_entry("映画.mkv")],
                    opaque_header: Vec::new(),
                },
                CtfDisk {
                    name: "Disk B".into(),
                    entries: vec![synthetic_root(), synthetic_entry("映画.mkv")],
                    opaque_header: Vec::new(),
                },
            ],
            opaque_trailer: Vec::new(),
        };
        fs::write(&source, ctf_format::write_ctf(&ctf).unwrap()).unwrap();
        let before = Sha256::digest(fs::read(&source).unwrap());
        let synced = crate::ctf_mirror::sync(&json!({
            "path":source,
            "cache_root":cache_root,
            "job_id":"synthetic-candidate-export"
        }))
        .unwrap();
        let generation = synced["generation"].as_str().unwrap();
        let mirrored =
            atlas_core::Catalog::open_readonly(cache_root.join(generation).join("catalog.sqlite"))
                .unwrap();
        let selected: i64 = mirrored
            .conn()
            .query_row(
                "SELECT id FROM entries WHERE name='映画.mkv' ORDER BY id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let exported = export(&json!({
            "format":"csv",
            "path":source,
            "output":output,
            "cache_root":cache_root,
            "generation":generation,
            "mode":"copy_candidates",
            "entry_id":selected,
            "other_disks_only":true,
            "overwrite":false
        }))
        .unwrap();
        assert_eq!(exported["result"]["rows"], 1);
        assert_eq!(
            exported["result"]["basis"],
            "exact_normalized_name_and_size"
        );
        assert_eq!(exported["result"]["verified"], false);
        assert_eq!(exported["result"]["original_ctf_read_only"], true);
        assert_eq!(exported["result"]["output_no_clobber"], true);
        assert_eq!(Sha256::digest(fs::read(source).unwrap()), before);
        let mut csv = csv::Reader::from_path(output).unwrap();
        assert_eq!(csv.records().count(), 1);
    }
}
