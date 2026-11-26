//! Shared command service for native clients and CLI.
use atlas_core::{Catalog, Error, Result};
use serde_json::{json, Value};
mod analytics;
mod copy_candidates;
mod ctf_annotation_backup;
mod ctf_annotations;
#[cfg(feature = "duckdb-query")]
mod ctf_duckdb;
mod ctf_duplicate_export;
mod ctf_duplicates;
mod ctf_export;
mod ctf_job;
mod ctf_mirror;
mod ctf_preview;
mod ctf_preview_analytics;
#[path = "../ctf_sidecar/mod.rs"]
mod ctf_sidecar;
mod dates;
mod disk_preferences;
mod duplicates;
mod exports;
mod focused_children;
mod legacy;
mod local_analyzer;
mod maintenance;
mod os_probe;
mod physical_disks;
// Retained for a later release; dispatch intentionally rejects the operation in v1.0.
#[allow(dead_code)]
mod rescan;
mod selection;

/// The actual SQLite library linked into each installed native executable.
/// Does not open any catalog or access the filesystem.
pub fn embedded_sqlite_version() -> &'static str {
    rusqlite::version()
}

/// Reserve an analytics cancellation slot before a native client launches its worker thread.
/// This is a bounded, in-memory operation and does not open the catalog or touch the filesystem.
pub fn prepare_analytics_job(job_id: &str) -> Result<()> {
    analytics::prepare(&json!({"job_id":job_id})).map(|_| ())
}
pub fn cancel_analytics_job(job_id: &str) -> Result<()> {
    analytics::cancel(&json!({"job_id":job_id})).map(|_| ())
}

/// A single-use CTF analytics request with an already registered cancellation
/// slot. Dropping it before dispatch releases that slot without filesystem I/O.
pub struct PreparedCtfPreviewAnalyticsJob {
    request: Value,
    job: ctf_job::Job,
}

/// Parse a bounded request and reserve its cancellation slot before launching a
/// worker. Does not open or hash the CTF, query a database, or evaluate metadata.
pub fn prepare_ctf_preview_analytics_job(input: &str) -> Result<PreparedCtfPreviewAnalyticsJob> {
    let request = parse_request(input)?;
    if request.get("op").and_then(Value::as_str) != Some("ctf_preview_analytics") {
        return Err(Error::new(
            "invalid_request",
            "Expected CTF preview analytics",
        ));
    }
    text(&request, "job_id")?;
    let job = ctf_job::Job::start(&request)?;
    Ok(PreparedCtfPreviewAnalyticsJob { request, job })
}

pub fn cancel_ctf_preview_analytics_job(job_id: &str) -> Result<()> {
    ctf_job::cancel(&json!({"job_id":job_id})).map(|_| ())
}

/// Consume the exact prepared request on a worker. There is no second request
/// argument that could accidentally reuse a cancellation slot for another source.
pub fn dispatch_json_with_prepared_ctf_preview_analytics_job(
    prepared: PreparedCtfPreviewAnalyticsJob,
) -> String {
    let PreparedCtfPreviewAnalyticsJob { request, job } = prepared;
    format_dispatch_result(std::panic::catch_unwind(|| {
        let data = ctf_preview_analytics::analytics(&request, job)?;
        Ok(json!({"ok":true,"data":data}))
    }))
}

/// Reserve a local analyzer job ID before the native client starts its worker thread.
/// Performs bounded in-memory validation only; it does not inspect the selected path.
pub fn prepare_local_analyzer_job(job_id: &str) -> Result<()> {
    local_analyzer::prepare(&json!({"job_id":job_id})).map(|_| ())
}

/// Request cancellation of a prepared or running local analyzer job.
pub fn cancel_local_analyzer_job(job_id: &str) -> Result<()> {
    local_analyzer::cancel(&json!({"job_id":job_id})).map(|_| ())
}

/// Reserve a one-shot local analyzer operation before dispatching its worker.
/// This is bounded in-memory state only and performs no filesystem or database I/O.
pub fn prepare_local_analyzer_operation(job_id: &str, operation_id: &str) -> Result<()> {
    local_analyzer::prepare_operation(&json!({"job_id":job_id,"operation_id":operation_id}))
        .map(|_| ())
}

pub fn dispatch_json(input: &str) -> String {
    format_dispatch_result(std::panic::catch_unwind(|| dispatch(input)))
}

fn format_dispatch_result(result: std::thread::Result<Result<Value>>) -> String {
    match result {
        Ok(Ok(value)) => value.to_string(),
        Ok(Err(e)) => json!({"ok":false,"error":{"code":e.code,"message":e.message}}).to_string(),
        Err(_) => json!({"ok":false,"error":{"code":"internal","message":"An internal error was contained"}}).to_string(),
    }
}

fn parse_request(input: &str) -> Result<Value> {
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
    Ok(v)
}

fn dispatch(input: &str) -> Result<Value> {
    let v = parse_request(input)?;
    let op = v
        .get("op")
        .or_else(|| v.get("command"))
        .or_else(|| v.get("cmd"))
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("invalid_request", "op must be a string"))?;
    let mut data = match op {
        "analytics" => analytics::run(&v)?,
        "analytics_prepare" => analytics::prepare(&v)?,
        "analytics_cancel" => analytics::cancel(&v)?,
        "local_analyzer_prepare" => local_analyzer::prepare(&v)?,
        "local_analyzer_prepare_operation" => local_analyzer::prepare_operation(&v)?,
        "local_analyzer_start" => local_analyzer::start(&v)?,
        "local_analyzer_progress" => local_analyzer::progress(&v)?,
        "local_analyzer_cancel" => local_analyzer::cancel(&v)?,
        "local_analyzer_page" => local_analyzer::page(&v)?,
        "local_analyzer_check_catalog" => local_analyzer::check_catalog(&v)?,
        "local_analyzer_save_baseline" => local_analyzer::save_baseline(&v)?,
        "local_analyzer_export_review_csv" => local_analyzer::export_csv(&v)?,
        "local_analyzer_close" => local_analyzer::close(&v)?,
        "local_analyzer_clear_baseline" => local_analyzer::clear_baseline(&v)?,
        "ctf_sync" => ctf_mirror::sync(&v)?,
        "ctf_preview_open" => ctf_preview::open(&v)?,
        "ctf_preview_volumes" => ctf_preview::volumes(&v)?,
        "ctf_preview_children" => ctf_preview::children(&v)?,
        "ctf_preview_children_focused" => ctf_preview::children_focused(&v)?,
        "ctf_preview_search" => ctf_preview::search(&v)?,
        "ctf_preview_copy_candidates" => ctf_preview::copy_candidates(&v)?,
        "ctf_preview_export" => ctf_preview::export(&v)?,
        "ctf_preview_analytics" => ctf_preview_analytics::analytics(&v, ctf_job::Job::start(&v)?)?,
        "ctf_preview_close" => ctf_preview::close(&v)?,
        "ctf_progress" => ctf_job::progress(&v)?,
        "ctf_cancel" => ctf_job::cancel(&v)?,
        "ctf_search" => ctf_mirror::search(&v)?,
        "ctf_search_pinned" => ctf_mirror::search_pinned(&v)?,
        "ctf_volumes_pinned" => ctf_mirror::volumes_pinned(&v)?,
        "ctf_children_pinned" => ctf_mirror::children_pinned(&v)?,
        "ctf_children_focused_pinned" => {
            ctf_mirror::read_pinned(&v, |db| focused_children::page(db, &v))?
        }
        "ctf_export_pinned" => ctf_export::export(&v)?,
        "ctf_annotations_list_pinned" => ctf_annotations::list(&v)?,
        "ctf_annotations_preview_pinned" => ctf_annotations::preview(&v)?,
        "ctf_annotations_confirm_pinned" => ctf_annotations::confirm(&v)?,
        "ctf_annotations_activate_pinned" => ctf_annotations::activate(&v)?,
        "ctf_annotations_activated_list_pinned" => ctf_annotations::activated_list(&v)?,
        "ctf_annotations_backup_pinned" => ctf_annotation_backup::backup(&v)?,
        "ctf_annotations_restore_pinned" => ctf_annotation_backup::restore(&v)?,
        "ctf_annotations_deactivate_pinned" => ctf_annotations::deactivate(&v)?,
        "ctf_annotations_set_pinned" => ctf_annotations::set(&v)?,
        "ctf_duplicates_pinned" => ctf_duplicates::duplicates(&v)?,
        "ctf_copy_candidates_pinned" => copy_candidates::linked(&v)?,
        "compact_catalog" => maintenance::compact_copy(&v)?,
        "probe_catalog" => maintenance::probe(&v)?,
        "validate_catalog" => maintenance::validate(&v)?,
        "import_ctf" => legacy::import(&mut Catalog::open(text(&v, "db")?)?, &v)?,
        "import_ctf_new" => legacy::import_new(&v)?,
        "export_ctf" => legacy::export(&v)?,
        "update_ctf_description" => legacy::update_description(&v)?,
        "ctf_info" => legacy::info(&v)?,
        "duplicates" => duplicates::duplicates(&Catalog::open(text(&v, "db")?)?, &v)?,
        "copy_candidates" => copy_candidates::page(&Catalog::open(text(&v, "db")?)?, &v)?,
        "children_focused" => focused_children::page(&Catalog::open(text(&v, "db")?)?, &v)?,
        "disk_preferences_list" => disk_preferences::list(&v)?,
        "disk_preferences_get" => disk_preferences::get(&v)?,
        "disk_preferences_set" => disk_preferences::set(&v)?,
        "disk_preferences_legacy_shelf_set" => disk_preferences::legacy_shelf_set(&v)?,
        "physical_status" => physical_disks::status(&v)?,
        // Windows 1.0 does not expose physical USB pairing or paired-device
        // rescans. Reject these before invoking OS probes or opening either
        // catalog/private sidecar; implementations remain available for a
        // later release.
        "rescan_volume" | "physical_pair" | "physical_unpair" => {
            return Err(Error::new(
                "feature_unavailable",
                "Physical USB pairing and bound-device rescans are unavailable in this release",
            ));
        }
        "physical_probe_os" => os_probe::probe(),
        "physical_shelf_labels" => physical_disks::shelf_labels(&v)?,
        "physical_shelf_label_set" => physical_disks::set_shelf_label(&v)?,
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

pub(crate) use ctf_sidecar::digest_hex;

#[cfg(test)]
mod prepared_ctf_analytics_contract {
    use super::*;

    fn request(id: &str) -> String {
        json!({"op":"ctf_preview_analytics","job_id":id,
            "path":"nonexistent-synthetic.ctf","generation":"synthetic"})
        .to_string()
    }

    #[test]
    fn cancellation_before_worker_dispatch_prevents_source_access_and_releases_slot() {
        let id = "prepared-ctf-analytics-before-dispatch";
        let prepared = prepare_ctf_preview_analytics_job(&request(id)).unwrap();
        cancel_ctf_preview_analytics_job(id).unwrap();
        let response: Value = serde_json::from_str(
            &dispatch_json_with_prepared_ctf_preview_analytics_job(prepared),
        )
        .unwrap();
        assert_eq!(response["error"]["code"], "cancelled");
        assert_eq!(
            cancel_ctf_preview_analytics_job(id).unwrap_err().code,
            "not_found"
        );
        drop(prepare_ctf_preview_analytics_job(&request(id)).unwrap());
    }

    #[test]
    fn abandoned_and_duplicate_preparations_keep_slot_ownership_correct() {
        let id = "prepared-ctf-analytics-slot-ownership";
        let prepared = prepare_ctf_preview_analytics_job(&request(id)).unwrap();
        assert_eq!(
            prepare_ctf_preview_analytics_job(&request(id))
                .err()
                .unwrap()
                .code,
            "job_exists"
        );
        cancel_ctf_preview_analytics_job(id).unwrap();
        drop(prepared);
        drop(prepare_ctf_preview_analytics_job(&request(id)).unwrap());
        assert_eq!(
            cancel_ctf_preview_analytics_job(id).unwrap_err().code,
            "not_found"
        );
    }

    #[test]
    fn preparation_rejects_other_operations_missing_ids_and_oversize_requests() {
        for input in [
            r#"{"op":"analytics","job_id":"wrong-kind"}"#,
            r#"{"op":"ctf_preview_analytics"}"#,
            "not JSON",
        ] {
            assert!(prepare_ctf_preview_analytics_job(input).is_err());
        }
        let too_large = " ".repeat(1024 * 1024 + 1);
        assert_eq!(
            prepare_ctf_preview_analytics_job(&too_large)
                .err()
                .unwrap()
                .code,
            "invalid_request"
        );
    }
}

#[cfg(test)]
mod modern_sha2_regression {
    use super::digest_hex;
    use sha2::{Digest, Sha256};

    #[test]
    fn sha2_011_known_vector_and_lowercase_exact_size() {
        let digest = Sha256::digest(b"abc");
        let encoded = digest_hex(digest);
        assert_eq!(
            encoded,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(encoded.len(), 64);
    }
}

#[cfg(test)]
mod release_scope_regression {
    use super::dispatch_json;
    use serde_json::{json, Value};

    #[test]
    fn physical_pairing_and_bound_rescan_fail_before_touching_paths() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = dir.path().join("catalog.sqlite");
        let user_store = dir.path().join("private.sqlite");
        let requests = [
            json!({"op":"physical_pair","user_store":user_store,"catalog_key":"x","volume_id":1,"confirmed":true}),
            json!({"op":"physical_unpair","user_store":user_store,"catalog_key":"x","volume_id":1,"confirmed":true}),
            json!({"op":"rescan_volume","db":catalog,"user_store":user_store,"catalog_key":"x","volume_id":1,"confirmed":true}),
        ];
        for request in requests {
            let first: Value = serde_json::from_str(&dispatch_json(&request.to_string())).unwrap();
            let second: Value = serde_json::from_str(&dispatch_json(&request.to_string())).unwrap();
            assert_eq!(
                first, second,
                "unavailable response should be deterministic"
            );
            assert_eq!(first["ok"], false);
            assert_eq!(first["error"]["code"], "feature_unavailable");
            assert_eq!(
                first["error"]["message"],
                "Physical USB pairing and bound-device rescans are unavailable in this release"
            );
        }
        assert!(!catalog.exists());
        assert!(!user_store.exists());
    }
}
