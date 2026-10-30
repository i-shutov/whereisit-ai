use serde_json::{json, Value};
use std::{
    path::PathBuf,
    ptr::{null, null_mut},
    sync::mpsc::{self, Receiver, Sender},
    time::{SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    Storage::FileSystem::FileTimeToLocalFileTime,
    System::{
        Com::*, LibraryLoader::*, SystemServices::SS_PATHELLIPSIS, Time::FileTimeToSystemTime,
    },
    UI::{
        Controls::Dialogs::*, Controls::*, Input::KeyboardAndMouse::*, Shell::*,
        WindowsAndMessaging::*,
    },
};

const COMPACT_COPY: usize = 200;
const VALIDATE_CATALOG: usize = 201;
const OPEN: usize = 101;
const NEW: usize = 102;
const SCAN: usize = 103;
const IMPORT: usize = 104;
const EXPORT: usize = 105;
const BACK: usize = 106;
const FIND: usize = 107;
const PREV: usize = 108;
const NEXT: usize = 109;
const CANCEL: usize = 110;
const DUPLICATES: usize = 111;
const DUP_MODE: usize = 112;
const EXPORT_FORMAT: usize = 113;
const LINK_CTF: usize = 114;
const ANNOTATION_PREVIEW: usize = 115;
const ANNOTATION_PAGE_SIZE: usize = 20;
const DISKS: usize = 120;
const FILES: usize = 121;
const CATEGORIES: usize = 122;
const CATEGORY_FILTER: usize = 123;
const CATEGORY_ASSIGN: usize = 124;
const CATEGORY_CREATE: usize = 125;
const LIMIT: usize = 250;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn string(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}
fn id(v: &Value) -> Option<i64> {
    v.get("id").and_then(Value::as_i64)
}
fn config_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
        .join("AtlasCatalog")
}
fn display_size(value: Option<u64>) -> String {
    let Some(bytes) = value else {
        return String::new();
    };
    let mut value = bytes as f64;
    let mut unit = 0;
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", units[unit])
    }
}
unsafe fn display_date(value: Option<i64>) -> String {
    let Some(ticks) = value
        .and_then(|v| v.checked_add(11_644_473_600))
        .and_then(|v| u64::try_from(v).ok())
        .and_then(|v| v.checked_mul(10_000_000))
    else {
        return String::new();
    };
    let utc = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut local: FILETIME = std::mem::zeroed();
    let mut date: SYSTEMTIME = std::mem::zeroed();
    if FileTimeToLocalFileTime(&utc, &mut local) == 0
        || FileTimeToSystemTime(&local, &mut date) == 0
    {
        return String::new();
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        date.wYear, date.wMonth, date.wDay, date.wHour, date.wMinute
    )
}
fn default_db() -> String {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    std::fs::read_to_string(dir.join("catalog-path.txt"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            dir.join("Catalog.atlas.sqlite")
                .to_string_lossy()
                .into_owned()
        })
}

// Pagination is purely navigational: a page never confirms an annotation.
fn annotation_page_controls(data: &Value, offset: usize) -> (bool, bool) {
    let preview = &data["result"];
    if preview["state"] != "generation_conflict" {
        return (false, false);
    }
    let total = preview["total"].as_u64().unwrap_or(0);
    let count = preview["items"].as_array().map_or(0, Vec::len);
    let previous = offset > 0;
    let next = count > 0 && (offset as u64).saturating_add(count as u64) < total;
    (previous, next)
}

// Render only a bounded local preview: opaque CTF and private notes never enter logs.
fn annotation_preview_text(data: &Value, offset: usize) -> String {
    let preview = &data["result"];
    let state = preview["state"].as_str().unwrap_or("unavailable");
    let total = preview["total"].as_u64().unwrap_or(0);
    let mut message = format!(
        "Linked CTF annotation review (page read-only)\nState: {state}\nSaved notes: {total}\n"
    );
    if state == "generation_conflict" {
        if let Some(rows) = preview["items"].as_array() {
            let start = if rows.is_empty() { 0 } else { offset + 1 };
            let end = offset.saturating_add(rows.len());
            message.push_str(&format!("Rows {start}–{end} of {total} (20 per page)\n"));
            for row in rows.iter().take(ANNOTATION_PAGE_SIZE) {
                let old_id = row["old_entry_id"].as_i64().unwrap_or(-1);
                let candidate_state = row["candidate_state"].as_str().unwrap_or("unknown");
                let count = row["candidate_count_lower_bound"].as_u64().unwrap_or(0);
                let suffix = if row["candidates_truncated"] == true {
                    "+"
                } else {
                    ""
                };
                let stale = if row["confirmation_revision_conflict"] == true
                    || row["confirmation_payload_conflict"] == true
                {
                    " [stale confirmation]"
                } else {
                    ""
                };
                let audit = match row["activation_state"].as_str() {
                    Some("active") => " [active audit; verify before displaying]",
                    Some("revoked") => " [revoked; audit retained]",
                    _ => "",
                };
                message.push_str(&format!(
                    "\nOld ID {old_id}: {candidate_state}, candidates {count}{suffix}{stale}{audit}"
                ));
            }
        }
    } else if state == "current_generation" {
        message.push_str("\nNo cross-generation reconciliation is required.");
    } else if state == "uninitialized" {
        message.push_str("\nNo user annotation database has been initialized.");
    }
    let (previous, next) = annotation_page_controls(data, offset);
    if previous || next {
        message.push_str(&format!(
            "\n\nYES = {}  |  NO = {}  |  CANCEL = close.",
            if next { "next page" } else { "not available" },
            if previous {
                "previous page"
            } else {
                "not available"
            }
        ));
    }
    message.push_str(
        "\n\nPREVIEW ONLY: opening a page never writes. Mapping, activation and revocation \
         each require separate consent. Every page re-verifies the external CTF generation.",
    );
    message
}

// Only independently verified fresh notes can be shown. Historical audit state
// must NEVER be treated as a verified live overlay.
fn verified_overlay_response_ok(data: &Value) -> bool {
    let view = &data["result"];
    match view["state"].as_str() {
        Some("partial_activation" | "activation_conflict") => {
            view["read_only_source"] == true
                && view["unresolved_unconfirmed_annotations_not_projected"] == true
                && view["total"].as_u64().is_some()
                && view["conflicts"].as_u64().is_some()
                && view["items"].as_array().is_some_and(|rows| {
                    rows.len() <= ANNOTATION_PAGE_SIZE
                        && rows.iter().all(|row| {
                            row["read_only"] == true
                                && row["activation"] == "explicit_reviewed_projection"
                                && row["entry_id"].as_i64().is_some_and(|id| id > 0)
                                && row["old_entry_id"].as_i64().is_some_and(|id| id > 0)
                                && row["revision"].as_i64().is_some_and(|id| id > 0)
                                && row["note"].as_str().is_some()
                                && row["tags"].as_str().is_some()
                        })
                })
        }
        Some("uninitialized" | "no_cross_generation_activations") => {
            view["total"] == 0
                && view["conflicts"] == 0
                && view["items"].as_array().is_some_and(Vec::is_empty)
        }
        _ => false,
    }
}

fn verified_overlay_page_controls(data: &Value, offset: usize) -> (bool, bool) {
    if !verified_overlay_response_ok(data) {
        return (false, false);
    }
    let view = &data["result"];
    if !matches!(
        view["state"].as_str(),
        Some("partial_activation" | "activation_conflict")
    ) {
        return (false, false);
    }
    let total = view["total"].as_u64().unwrap_or(0);
    let shown = view["items"].as_array().map_or(0, Vec::len);
    (
        offset > 0,
        shown > 0 && (offset as u64).saturating_add(shown as u64) < total,
    )
}

fn verified_overlay_summary(data: &Value, offset: usize) -> String {
    if !verified_overlay_response_ok(data) {
        return "Overlay response failed verification. No private notes are displayed.".into();
    }
    let view = &data["result"];
    if matches!(
        view["state"].as_str(),
        Some("uninitialized" | "no_cross_generation_activations")
    ) {
        return "No verified cross-generation overlays. No notes were copied or modified.".into();
    }
    let total = view["total"].as_u64().unwrap_or(0);
    let conflicts = view["conflicts"].as_u64().unwrap_or(0);
    let rows = view["items"].as_array().expect("response checked");
    let start = if rows.is_empty() { 0 } else { offset + 1 };
    let end = offset.saturating_add(rows.len());
    let mut summary = format!(
        "VERIFIED READ-ONLY OVERLAYS\n\
         Valid active: {total}\n\
         Stale/conflicting active audits excluded: {conflicts}\n\
         Unconfirmed mappings excluded (not counted).\n\
         Rows {start}–{end} of {total}\n"
    );
    for row in rows {
        let old = row["old_entry_id"].as_i64().unwrap_or(-1);
        let target = row["entry_id"].as_i64().unwrap_or(-1);
        let note = row["note"].as_str().unwrap_or("");
        let excerpt: String = note
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(100)
            .collect();
        let suffix = if note.chars().count() > 100 {
            "…"
        } else {
            ""
        };
        summary.push_str(&format!(
            "\nOld #{old} → target #{target}: {excerpt}{suffix}"
        ));
    }
    summary.push_str("\n\nEvery page freshly verifies the external CTF SHA and note revisions. Viewing never writes. An active audit is not proof of a valid overlay.");
    summary
}

#[cfg(test)]
mod annotation_preview_tests {
    use super::{
        annotation_overlay_request, annotation_page_controls, annotation_preview_text,
        annotation_selection_request, verified_overlay_page_controls, verified_overlay_response_ok,
        verified_overlay_summary, AnnotationOverlayAction,
    };
    use serde_json::{json, Value};

    #[test]
    fn ambiguity_and_stale_confirmation_remain_visible() {
        let result = json!({"result":{"state":"generation_conflict","total":21,
            "items":[{"old_entry_id":4,"candidate_state":"ambiguous",
                "candidate_count_lower_bound":6,"candidates_truncated":true,
                "confirmation_revision_conflict":true}]}});
        let text = annotation_preview_text(&result, 0);
        assert!(text.contains("ambiguous, candidates 6+ [stale confirmation]"));
        assert!(text.contains("Rows 1–1 of 21"));
        assert!(text.contains("opening a page never writes"));
        assert_eq!(annotation_page_controls(&result, 0), (false, true));
    }

    #[test]
    fn matching_generation_never_implies_approval() {
        let result = json!({"result":{"state":"current_generation","total":1,"items":[]}});
        let text = annotation_preview_text(&result, 0);
        assert!(text.contains("No cross-generation reconciliation"));
        assert!(text.contains("PREVIEW ONLY"));
        assert_eq!(annotation_page_controls(&result, 0), (false, false));
    }

    #[test]
    fn no_default_or_stale_candidate_is_ever_implicitly_selected() {
        let preview = json!({"result":{
            "state":"generation_conflict","document_id":"synthetic-document",
            "total":1,"items":[{"old_entry_id":7,"revision":2,
                "candidates":[{"entry_id":19,"volume_id":3},{"entry_id":23,"volume_id":3}],
                "confirmed_selection":null,"confirmation_revision_conflict":false,
                "confirmation_payload_conflict":false}]
        }});
        let first = annotation_selection_request(&preview, 0, 0).unwrap();
        let second = annotation_selection_request(&preview, 0, 1).unwrap();
        assert_eq!(first["new_entry_id"], 19);
        assert_eq!(second["new_entry_id"], 23);
        assert_eq!(first["confirmation"], "user_selected_exact_candidate");
        assert!(annotation_selection_request(&preview, 0, 2).is_none());
        assert!(annotation_selection_request(&preview, 1, 0).is_none());
        let mut stale = preview.clone();
        stale["result"]["items"][0]["confirmation_revision_conflict"] = json!(true);
        assert!(annotation_selection_request(&stale, 0, 0).is_none());
        stale = preview.clone();
        stale["result"]["items"][0]["revision"] = json!(0);
        assert!(annotation_selection_request(&stale, 0, 0).is_none());
        stale = preview.clone();
        stale["result"]["items"][0]["confirmed_selection"] = json!({"new_entry_id":19});
        assert!(annotation_selection_request(&stale, 0, 0).is_none());
        stale = preview.clone();
        stale["result"]["document_id"] = Value::Null;
        assert!(annotation_selection_request(&stale, 0, 0).is_none());
        stale = preview;
        stale["result"]["state"] = json!("current_generation");
        assert!(annotation_selection_request(&stale, 0, 0).is_none());
    }

    #[test]
    fn overlay_requires_explicit_confirmed_pair_and_fresh_note() {
        let preview = json!({"result":{
            "state":"generation_conflict","document_id":"synthetic-document",
            "items":[
                {"old_entry_id":11,"revision":3,
                 "candidates":[{"entry_id":37}],
                 "confirmed_selection":{"new_entry_id":37},
                 "confirmation_revision_conflict":false,
                 "confirmation_payload_conflict":false,
                 "activation_state":null,"activation_target_entry_id":null},
                {"old_entry_id":12,"revision":4,
                 "candidates":[{"entry_id":38}],
                 "confirmed_selection":{"new_entry_id":38},
                 "confirmation_revision_conflict":true,
                 "confirmation_payload_conflict":false,
                 "activation_state":"active","activation_target_entry_id":38}
            ]
        }});
        let activate = annotation_overlay_request(&preview, 0, AnnotationOverlayAction::Activate)
            .expect("explicit fresh confirmation");
        assert_eq!(activate["op"], "ctf_annotations_activate_pinned");
        assert_eq!(activate["new_entry_id"], 37);
        assert_eq!(activate["expected_revision"], 3);
        assert_eq!(activate["expected_document_id"], "synthetic-document");
        assert_eq!(
            activate["activation"],
            "user_approved_reversible_projection"
        );
        assert!(annotation_overlay_request(&preview, 0, AnnotationOverlayAction::Revoke).is_none());
        assert!(
            annotation_overlay_request(&preview, 1, AnnotationOverlayAction::Activate).is_none()
        );
        // Stale notes must remain revocable; the backend checks the exact audit pair.
        let revoke = annotation_overlay_request(&preview, 1, AnnotationOverlayAction::Revoke)
            .expect("stale active audit is revocable");
        assert_eq!(revoke["op"], "ctf_annotations_deactivate_pinned");
        assert_eq!(revoke["new_entry_id"], 38);
        assert_eq!(revoke["rollback"], "user_confirmed_revoke_projection");
        assert!(revoke.get("expected_revision").is_none());
        assert_eq!(revoke["expected_document_id"], "synthetic-document");

        let mut changed = preview.clone();
        changed["result"]["items"][0]["confirmation_payload_conflict"] = json!(true);
        assert!(
            annotation_overlay_request(&changed, 0, AnnotationOverlayAction::Activate).is_none()
        );
        changed = preview.clone();
        changed["result"]["items"][0]["candidates"] = json!([{"entry_id":99}]);
        assert!(
            annotation_overlay_request(&changed, 0, AnnotationOverlayAction::Activate).is_none()
        );
        changed = preview.clone();
        changed["result"]["items"][0]["confirmed_selection"] = Value::Null;
        assert!(
            annotation_overlay_request(&changed, 0, AnnotationOverlayAction::Activate).is_none()
        );
        changed = preview.clone();
        changed["result"]["items"][1]["activation_state"] = json!("revoked");
        assert!(annotation_overlay_request(&changed, 1, AnnotationOverlayAction::Revoke).is_none());
        assert!(
            annotation_overlay_request(&changed, 1, AnnotationOverlayAction::Activate).is_none()
        );
        changed = preview.clone();
        changed["result"]["items"][1]["activation_target_entry_id"] = Value::Null;
        assert!(annotation_overlay_request(&changed, 1, AnnotationOverlayAction::Revoke).is_none());
        changed = preview.clone();
        changed["result"]["document_id"] = json!("");
        assert!(
            annotation_overlay_request(&changed, 0, AnnotationOverlayAction::Activate).is_none()
        );
        changed = preview;
        changed["result"]["state"] = json!("current_generation");
        assert!(
            annotation_overlay_request(&changed, 0, AnnotationOverlayAction::Activate).is_none()
        );
        assert!(annotation_overlay_request(&changed, 1, AnnotationOverlayAction::Revoke).is_none());
    }

    #[test]
    fn middle_last_and_empty_pages_are_bounded() {
        let middle = json!({"result":{"state":"generation_conflict","total":43,
            "items": vec![json!({"old_entry_id":1}); 20]}});
        assert_eq!(annotation_page_controls(&middle, 20), (true, true));
        assert!(annotation_preview_text(&middle, 20).contains("Rows 21–40 of 43"));
        let last = json!({"result":{"state":"generation_conflict","total":43,
            "items": vec![json!({"old_entry_id":1}); 3]}});
        assert_eq!(annotation_page_controls(&last, 40), (true, false));
        let empty = json!({"result":{"state":"generation_conflict","total":43,"items":[]}});
        assert_eq!(annotation_page_controls(&empty, 60), (true, false));
    }
    #[test]
    fn verified_view_fail_closed_and_show_exact_conflict_count() {
        let original = json!({"result":{"state":"activation_conflict","total":1,"conflicts":2,
            "read_only_source":true,"unresolved_unconfirmed_annotations_not_projected":true,
            "items":[{"entry_id":9,"old_entry_id":3,"revision":1,"read_only":true,
                "activation":"explicit_reviewed_projection",
                "note":"synthetic private note","tags":"synthetic"}]}});
        assert!(verified_overlay_response_ok(&original));
        let summary = verified_overlay_summary(&original, 0);
        assert!(summary.contains("Valid active: 1"));
        assert!(summary.contains("active audits excluded: 2"));
        assert!(summary.contains("synthetic private note"));
        for key in [
            "read_only_source",
            "unresolved_unconfirmed_annotations_not_projected",
        ] {
            let mut invalid = original.clone();
            invalid["result"][key] = json!(false);
            assert!(!verified_overlay_response_ok(&invalid));
            assert!(!verified_overlay_summary(&invalid, 0).contains("synthetic private note"));
        }
        let mut invalid = original.clone();
        invalid["result"]["items"][0]["activation"] = json!("unverified_audit");
        assert!(!verified_overlay_response_ok(&invalid));
        assert!(!verified_overlay_summary(&invalid, 0).contains("synthetic private note"));
    }

    #[test]
    fn verified_view_pages_are_bounded_and_empty_is_read_only() {
        let item = json!({"entry_id":9,"old_entry_id":3,"revision":1,"read_only":true,
            "activation":"explicit_reviewed_projection","note":"synthetic","tags":""});
        let mut page = json!({"result":{"state":"partial_activation","total":21,
            "conflicts":0,"read_only_source":true,
            "unresolved_unconfirmed_annotations_not_projected":true,
            "items":vec![item.clone(); 20]}});
        assert_eq!(verified_overlay_page_controls(&page, 0), (false, true));
        page["result"]["items"] = json!([item]);
        assert_eq!(verified_overlay_page_controls(&page, 20), (true, false));
        assert!(verified_overlay_summary(&page, 20).contains("Rows 21–21 of 21"));
        let empty = json!({"result":{"state":"no_cross_generation_activations",
            "items":[],"total":0,"conflicts":0}});
        assert!(verified_overlay_response_ok(&empty));
        assert_eq!(verified_overlay_page_controls(&empty, 0), (false, false));
    }
}

#[derive(Debug)]
enum AnnotationReviewAction {
    Close,
    Previous,
    Next,
    Confirm(Value),
    Overlay(Value),
    Verified,
}

// Pure selection contract: a radio choice is mandatory at both stages.
fn annotation_selection_request(
    data: &Value,
    row_index: usize,
    candidate_index: usize,
) -> Option<Value> {
    let review = &data["result"];
    if review["state"] != "generation_conflict" {
        return None;
    }
    let document_id = review["document_id"].as_str()?;
    let row = review["items"].as_array()?.get(row_index)?;
    let revision = row["revision"].as_i64().filter(|revision| *revision > 0)?;
    let old_entry_id = row["old_entry_id"].as_i64()?;
    if row["confirmed_selection"].is_object()
        || row["confirmation_revision_conflict"] == true
        || row["confirmation_payload_conflict"] == true
    {
        return None;
    }
    let candidate = row["candidates"].as_array()?.get(candidate_index)?;
    let new_entry_id = candidate["entry_id"].as_i64()?;
    Some(json!({
        "old_entry_id": old_entry_id,
        "new_entry_id": new_entry_id,
        "expected_revision": revision,
        "expected_document_id": document_id,
        "confirmation": "user_selected_exact_candidate"
    }))
}

// A journal state is not proof of a valid live overlay. Activation requires
// a fresh confirmed pair; revocation deliberately also permits a stale note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnnotationOverlayAction {
    Activate,
    Revoke,
}

fn annotation_overlay_request(
    data: &Value,
    row_index: usize,
    action: AnnotationOverlayAction,
) -> Option<Value> {
    let review = &data["result"];
    if review["state"] != "generation_conflict" {
        return None;
    }
    let document_id = review["document_id"].as_str().filter(|id| !id.is_empty())?;
    let row = review["items"].as_array()?.get(row_index)?;
    let old_entry_id = row["old_entry_id"].as_i64().filter(|id| *id > 0)?;
    match action {
        AnnotationOverlayAction::Activate => {
            let revision = row["revision"].as_i64().filter(|revision| *revision > 0)?;
            if !row["activation_state"].is_null()
                || row["confirmation_revision_conflict"] == true
                || row["confirmation_payload_conflict"] == true
            {
                return None;
            }
            let new_entry_id = row["confirmed_selection"]["new_entry_id"]
                .as_i64()
                .filter(|id| *id > 0)?;
            // A confirmation is only eligible if its exact candidate still
            // appears in this SHA-verified read-only page.
            if !row["candidates"]
                .as_array()?
                .iter()
                .any(|candidate| candidate["entry_id"].as_i64() == Some(new_entry_id))
            {
                return None;
            }
            Some(json!({
                "op":"ctf_annotations_activate_pinned",
                "old_entry_id":old_entry_id,
                "new_entry_id":new_entry_id,
                "expected_revision":revision,
                "expected_document_id":document_id,
                "activation":"user_approved_reversible_projection"
            }))
        }
        AnnotationOverlayAction::Revoke => {
            if row["activation_state"] != "active" {
                return None;
            }
            let new_entry_id = row["activation_target_entry_id"]
                .as_i64()
                .filter(|id| *id > 0)?;
            Some(json!({
                "op":"ctf_annotations_deactivate_pinned",
                "old_entry_id":old_entry_id,
                "new_entry_id":new_entry_id,
                "expected_document_id":document_id,
                "rollback":"user_confirmed_revoke_projection"
            }))
        }
    }
}

// TaskDialogIndirect provides native Windows radio buttons and explicitly
// disables the default radio choice; no candidate is approved by navigation.
unsafe fn annotation_radio_dialog(
    owner: HWND,
    instruction: &str,
    content: &str,
    options: &[String],
    actions: &[(i32, &str)],
) -> Option<(i32, Option<usize>)> {
    let title = wide("WhereIsIt AI — CTF annotation review");
    let instruction = wide(instruction);
    let content = wide(content);
    let radio_labels: Vec<Vec<u16>> = options.iter().map(|label| wide(label)).collect();
    let radio_buttons: Vec<TASKDIALOG_BUTTON> = radio_labels
        .iter()
        .enumerate()
        .map(|(index, label)| TASKDIALOG_BUTTON {
            nButtonID: 1000 + index as i32,
            pszButtonText: label.as_ptr(),
        })
        .collect();
    let button_labels: Vec<Vec<u16>> = actions.iter().map(|(_, label)| wide(label)).collect();
    let buttons: Vec<TASKDIALOG_BUTTON> = actions
        .iter()
        .zip(button_labels.iter())
        .map(|((id, _), label)| TASKDIALOG_BUTTON {
            nButtonID: *id,
            pszButtonText: label.as_ptr(),
        })
        .collect();
    let mut config: TASKDIALOGCONFIG = std::mem::zeroed();
    config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
    config.hwndParent = owner;
    config.dwFlags = TDF_NO_DEFAULT_RADIO_BUTTON | TDF_ALLOW_DIALOG_CANCELLATION;
    config.dwCommonButtons = TDCBF_CANCEL_BUTTON;
    config.pszWindowTitle = title.as_ptr();
    config.pszMainInstruction = instruction.as_ptr();
    config.pszContent = content.as_ptr();
    config.cRadioButtons = radio_buttons.len() as u32;
    config.pRadioButtons = radio_buttons.as_ptr();
    config.nDefaultRadioButton = 0;
    config.cButtons = buttons.len() as u32;
    config.pButtons = buttons.as_ptr();
    config.nDefaultButton = IDCANCEL;
    config.cxWidth = 320;
    let mut button = IDCANCEL;
    let mut radio = 0;
    if TaskDialogIndirect(&config, &mut button, &mut radio, null_mut()) < 0 {
        return None;
    }
    let selected = usize::try_from(radio - 1000)
        .ok()
        .filter(|index| *index < options.len());
    Some((button, selected))
}

unsafe fn annotation_review_action(
    owner: HWND,
    data: &Value,
    offset: usize,
) -> AnnotationReviewAction {
    let preview = &data["result"];
    if preview["state"] != "generation_conflict" {
        MessageBoxW(
            owner,
            wide(&annotation_preview_text(data, offset)).as_ptr(),
            wide("CTF annotation review — no changes").as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
        return AnnotationReviewAction::Close;
    }
    let Some(items) = preview["items"].as_array() else {
        return AnnotationReviewAction::Close;
    };
    let (previous, next) = annotation_page_controls(data, offset);
    let can_map = |index| annotation_selection_request(data, index, 0).is_some();
    let can_activate = |index| {
        annotation_overlay_request(data, index, AnnotationOverlayAction::Activate).is_some()
    };
    let can_revoke =
        |index| annotation_overlay_request(data, index, AnnotationOverlayAction::Revoke).is_some();
    let selectable: Vec<usize> = (0..items.len())
        .filter(|index| can_map(*index) || can_activate(*index) || can_revoke(*index))
        .collect();
    let labels: Vec<String> = selectable
        .iter()
        .map(|index| {
            let row = &items[*index];
            let id = row["old_entry_id"].as_i64().unwrap_or(-1);
            let state = row["candidate_state"].as_str().unwrap_or("unknown");
            let count = row["candidate_count_lower_bound"].as_u64().unwrap_or(0);
            let truncated = if row["candidates_truncated"] == true {
                "+"
            } else {
                ""
            };
            let audit = if can_revoke(*index) {
                "ACTIVE audit — eligible for explicit revocation"
            } else if can_activate(*index) {
                "confirmed, inactive — eligible for separate activation"
            } else {
                "unconfirmed — choose a structural candidate"
            };
            format!("Old #{id} — {state} ({count}{truncated} candidates) — {audit}")
        })
        .collect();
    let total = preview["total"].as_u64().unwrap_or(0);
    let start = if items.is_empty() { 0 } else { offset + 1 };
    let end = offset.saturating_add(items.len());
    let content = format!(
        "Rows {start}–{end} of {total}. Select one old note and a separate action. \
         {} rows have unresolved or stale conflicts; open All conflicts to inspect them. \
         The CTF is read-only. Every action re-verifies source SHA and document CAS.",
        items.len().saturating_sub(selectable.len())
    );
    loop {
        let mut actions = vec![
            (301, "Show all conflicts"),
            (306, "Verified overlays (read-only)…"),
        ];
        if selectable.iter().any(|index| can_map(*index)) {
            actions.insert(0, (300, "Choose target…"));
        }
        if selectable.iter().any(|index| can_activate(*index)) {
            actions.push((304, "Activate confirmed mapping…"));
        }
        if selectable.iter().any(|index| can_revoke(*index)) {
            actions.push((305, "Revoke active mapping…"));
        }
        if previous {
            actions.push((302, "Previous page"));
        }
        if next {
            actions.push((303, "Next page"));
        }
        let Some((button, selected)) = annotation_radio_dialog(
            owner,
            "Generation-pinned annotation review",
            &content,
            &labels,
            &actions,
        ) else {
            return AnnotationReviewAction::Close;
        };
        match button {
            302 if previous => return AnnotationReviewAction::Previous,
            303 if next => return AnnotationReviewAction::Next,
            306 => return AnnotationReviewAction::Verified,
            301 => {
                MessageBoxW(
                    owner,
                    wide(&annotation_preview_text(data, offset)).as_ptr(),
                    wide("All annotation conflicts — read-only").as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            300 => {
                let Some(row_index) = selected.and_then(|index| selectable.get(index).copied())
                else {
                    MessageBoxW(
                        owner,
                        wide("Select an old note first; nothing was written.").as_ptr(),
                        wide("Explicit choice required").as_ptr(),
                        MB_OK | MB_ICONWARNING,
                    );
                    continue;
                };
                if !can_map(row_index) {
                    MessageBoxW(
                        owner,
                        wide("This note cannot be mapped from the current page. Open All conflicts and refresh the source.").as_ptr(),
                        wide("Mapping ineligible").as_ptr(),
                        MB_OK | MB_ICONWARNING,
                    );
                    continue;
                }
                let row = &items[row_index];
                let Some(candidates) = row["candidates"].as_array() else {
                    continue;
                };
                let choices: Vec<String> = candidates
                    .iter()
                    .take(8)
                    .map(|candidate| {
                        format!(
                            "Target #{} • volume #{} • parent #{} • modified {}",
                            candidate["entry_id"].as_i64().unwrap_or(-1),
                            candidate["volume_id"].as_i64().unwrap_or(-1),
                            candidate["parent_id"].as_i64().unwrap_or(-1),
                            candidate["modified_unix"].as_i64().unwrap_or(-1)
                        )
                    })
                    .collect();
                let old_id = row["old_entry_id"].as_i64().unwrap_or(-1);
                let Some((301, Some(candidate_index))) = annotation_radio_dialog(
                    owner,
                    &format!("Choose target for old entry #{old_id}"),
                    "Select the exact structural candidate. No radio is preselected; source CTF remains read-only.",
                    &choices,
                    &[(301, "Review selected mapping")],
                ) else {
                    continue;
                };
                let Some(request) = annotation_selection_request(data, row_index, candidate_index)
                else {
                    continue;
                };
                let new_id = request["new_entry_id"].as_i64().unwrap_or(-1);
                let message = format!(
                    "Confirm old entry #{old_id} → target #{new_id}?\n\n\
                     This records an INACTIVE user-owned audit mapping only. \
                     It never migrates notes or changes source CTF/derived cache. \
                     Source SHA, document, candidate and revision are rechecked."
                );
                if MessageBoxW(
                    owner,
                    wide(&message).as_ptr(),
                    wide("Confirm exact annotation mapping").as_ptr(),
                    MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                ) == IDYES
                {
                    return AnnotationReviewAction::Confirm(request);
                }
            }
            304 | 305 => {
                let Some(row_index) = selected.and_then(|index| selectable.get(index).copied())
                else {
                    MessageBoxW(
                        owner,
                        wide(
                            "Select an old note first. No activation or revocation was performed.",
                        )
                        .as_ptr(),
                        wide("Explicit choice required").as_ptr(),
                        MB_OK | MB_ICONWARNING,
                    );
                    continue;
                };
                let action = if button == 304 {
                    AnnotationOverlayAction::Activate
                } else {
                    AnnotationOverlayAction::Revoke
                };
                let Some(request) = annotation_overlay_request(data, row_index, action) else {
                    MessageBoxW(
                        owner,
                        wide("Selected audit is not eligible for this action. Stale, ambiguous and revoked mappings are never activated automatically. Review conflicts again.").as_ptr(),
                        wide("Action rejected — conflict").as_ptr(),
                        MB_OK | MB_ICONWARNING,
                    );
                    continue;
                };
                let old_id = request["old_entry_id"].as_i64().unwrap_or(-1);
                let new_id = request["new_entry_id"].as_i64().unwrap_or(-1);
                let (title, message) = if action == AnnotationOverlayAction::Activate {
                    (
                        "Approve a read-only annotation overlay",
                        format!(
                            "ACTIVATE old note #{old_id} → target #{new_id}?\n\n\
                             This is a SEPARATE approval from mapping confirmation. \
                             Original notes remain unchanged. Source SHA, document, old revision/digest \
                             and exact candidate are rechecked by the service."
                        ),
                    )
                } else {
                    (
                        "Revoke an audited annotation overlay",
                        format!(
                            "REVOKE active mapping #{old_id} → #{new_id}?\n\n\
                             The audit and full original note snapshot are retained. \
                             This may revoke a stale audit, but a revoked mapping cannot be replayed. \
                             Source SHA, document and exact active pair are checked."
                        ),
                    )
                };
                if MessageBoxW(
                    owner,
                    wide(&message).as_ptr(),
                    wide(title).as_ptr(),
                    MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                ) == IDYES
                {
                    return AnnotationReviewAction::Overlay(request);
                }
            }
            _ => return AnnotationReviewAction::Close,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum VerifiedOverlayNavigation {
    Close,
    Previous,
    Next,
}

// TaskDialog has no default choice; the modal has no write actions.
unsafe fn verified_overlay_action(
    owner: HWND,
    data: &Value,
    offset: usize,
) -> VerifiedOverlayNavigation {
    let (previous, next) = verified_overlay_page_controls(data, offset);
    let mut actions = Vec::new();
    if previous {
        actions.push((302, "Previous verified page"));
    }
    if next {
        actions.push((303, "Next verified page"));
    }
    let Some((button, _)) = annotation_radio_dialog(
        owner,
        "Verified active notes — read-only",
        &verified_overlay_summary(data, offset),
        &[],
        &actions,
    ) else {
        return VerifiedOverlayNavigation::Close;
    };
    match button {
        302 if previous => VerifiedOverlayNavigation::Previous,
        303 if next => VerifiedOverlayNavigation::Next,
        _ => VerifiedOverlayNavigation::Close,
    }
}

struct Reply {
    database: String,
    tag: &'static str,
    generation: u64,
    result: Value,
}
struct App {
    hwnd: HWND,
    disks: HWND,
    files: HWND,
    search: HWND,
    status: HWND,
    location: HWND,
    buttons: Vec<(usize, HWND)>,
    duplicate_mode: HWND,
    export_format: HWND,
    category_list: HWND,
    category_name: HWND,
    categories: Vec<Value>,
    duplicates: bool,
    applied_duplicate_mode: &'static str,
    db: String,
    linked_ctf: Option<String>,
    linked_generation: Option<String>,
    pending_link_path: Option<String>,
    annotation_db: Option<String>,
    annotation_offset: usize,
    verified_overlay_offset: usize,
    volumes: Vec<Value>,
    entries: Vec<Value>,
    volume: Option<i64>,
    stack: Vec<(Option<i64>, String)>,
    query: String,
    offset: usize,
    total: usize,
    total_known: bool,
    has_more: bool,
    generation: u64,
    pending: usize,
    job: Option<String>,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
}
impl App {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            hwnd: null_mut(),
            disks: null_mut(),
            files: null_mut(),
            search: null_mut(),
            status: null_mut(),
            location: null_mut(),
            buttons: vec![],
            duplicate_mode: null_mut(),
            export_format: null_mut(),
            category_list: null_mut(),
            category_name: null_mut(),
            categories: vec![],
            duplicates: false,
            applied_duplicate_mode: "name_size",
            db: std::env::args().nth(1).unwrap_or_else(default_db),
            linked_ctf: None,
            linked_generation: None,
            pending_link_path: None,
            annotation_db: None,
            annotation_offset: 0,
            verified_overlay_offset: 0,
            volumes: vec![],
            entries: vec![],
            volume: None,
            stack: vec![(None, "/".into())],
            query: String::new(),
            offset: 0,
            total: 0,
            total_known: true,
            has_more: false,
            generation: 0,
            pending: 0,
            job: None,
            tx,
            rx,
        }
    }
    unsafe fn text(&self, target: HWND, text: &str) {
        SetWindowTextW(target, wide(text).as_ptr());
    }
    unsafe fn status(&self, text: &str) {
        self.text(self.status, text);
    }
    fn linked_request(&self, operation: &str) -> Option<Value> {
        let source = self.linked_ctf.as_ref()?;
        let generation = self.linked_generation.as_ref()?;
        let cache = config_dir()
            .join("CTF-Mirrors")
            .to_string_lossy()
            .into_owned();
        Some(json!({"op":operation,"path":source,"cache_root":cache,"generation":generation}))
    }
    fn linked_mode(&self) -> bool {
        self.linked_ctf.is_some() && self.linked_generation.is_some()
    }
    fn request_annotation_page(&mut self) {
        let Some(store) = self.annotation_db.as_ref().cloned() else {
            return;
        };
        let Some(mut pinned) = self.linked_request("ctf_annotations_preview_pinned") else {
            return;
        };
        pinned["annotation_db"] = json!(store);
        pinned["offset"] = json!(self.annotation_offset);
        pinned["limit"] = json!(ANNOTATION_PAGE_SIZE);
        self.call("annotation_preview", pinned);
    }
    fn request_verified_overlay_page(&mut self) {
        let Some(store) = self.annotation_db.as_ref().cloned() else {
            return;
        };
        let Some(mut pinned) = self.linked_request("ctf_annotations_activated_list_pinned") else {
            return;
        };
        pinned["annotation_db"] = json!(store);
        pinned["offset"] = json!(self.verified_overlay_offset);
        pinned["limit"] = json!(ANNOTATION_PAGE_SIZE);
        self.call("overlay_verified", pinned);
    }
    fn call(&mut self, tag: &'static str, mut request: Value) {
        request["db"] = json!(self.db);
        let database = self.db.clone();
        let tx = self.tx.clone();
        let generation = self.generation;
        self.pending += 1;
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| atlas_service::dispatch_json(&request.to_string()))
                .ok().and_then(|s|serde_json::from_str(&s).ok())
                .unwrap_or_else(||json!({"ok":false,"error":{"message":"Core operation failed or returned invalid JSON"}}));
            let _ = tx.send(Reply {
                database,
                tag,
                generation,
                result,
            });
        });
    }
    unsafe fn init(&mut self) {
        self.linked_ctf = None;
        self.linked_generation = None;
        self.pending_link_path = None;
        self.annotation_db = None;
        self.annotation_offset = 0;
        self.verified_overlay_offset = 0;
        let _ = std::fs::create_dir_all(config_dir());
        self.generation += 1;
        self.volumes.clear();
        self.entries.clear();
        self.categories.clear();
        self.total = 0;
        self.has_more = false;
        SendMessageW(self.disks, LB_RESETCONTENT, 0, 0);
        SendMessageW(self.category_list, LB_RESETCONTENT, 0, 0);
        SendMessageW(self.files, LVM_DELETEALLITEMS, 0, 0);
        self.volume = None;
        self.stack = vec![(None, "/".into())];
        self.query.clear();
        self.offset = 0;
        self.duplicates = false;
        self.text(self.search, "");
        self.status("Opening catalog…");
        self.call("init", json!({"op":"init"}));
    }
    unsafe fn refresh(&mut self) {
        self.generation += 1;
        if let Some(linked) = self.linked_request("ctf_volumes_pinned") {
            self.categories.clear();
            SendMessageW(self.category_list, LB_RESETCONTENT, 0, 0);
            self.call("linked_volumes", linked);
            self.load();
            return;
        }
        self.call("volumes", json!({"op":"volumes"}));
        self.call("categories", json!({"op":"categories"}));
        self.load();
    }
    unsafe fn load(&mut self) {
        self.generation += 1;
        self.status(if self.duplicates {
            "Finding metadata duplicate candidates…"
        } else {
            "Loading…"
        });
        self.text(
            self.location,
            &format!(
                "{}  —  {}",
                self.linked_ctf.as_deref().unwrap_or(&self.db),
                if self.query.is_empty() {
                    self.stack.last().map(|p| p.1.as_str()).unwrap_or("/")
                } else {
                    "Search results"
                }
            ),
        );
        if let Some(mut linked) = self.linked_request(if self.duplicates {
            "ctf_duplicates_pinned"
        } else if !self.query.is_empty() {
            "ctf_search_pinned"
        } else {
            "ctf_children_pinned"
        }) {
            linked["offset"] = json!(self.offset);
            linked["limit"] = json!(LIMIT);
            linked["volume_id"] = json!(self.volume);
            if self.duplicates {
                linked["query"] = json!(self.query);
                linked["duplicate_mode"] = json!(self.applied_duplicate_mode);
                self.call("rows", linked);
            } else if !self.query.is_empty() {
                linked["query"] = json!(self.query);
                self.call("rows", linked);
            } else if self.volume.is_some() {
                linked["parent_id"] = json!(self.stack.last().and_then(|p| p.0));
                self.call("rows", linked);
            } else {
                self.entries.clear();
                self.total = 0;
                self.has_more = false;
                SendMessageW(self.files, LVM_DELETEALLITEMS, 0, 0);
                self.status("Choose a linked CTF disk or search all disks (read-only).");
            }
            return;
        }
        if self.duplicates {
            self.call("rows",json!({"op":"duplicates","mode":self.applied_duplicate_mode,"query":self.query,"volume_id":self.volume,"offset":self.offset,"limit":LIMIT}));
        } else if !self.query.is_empty() {
            self.call("rows",json!({"op":"search_fast","query":self.query,"volume_id":self.volume,"offset":self.offset,"limit":LIMIT}));
        } else if let Some(volume) = self.volume {
            self.call("rows",json!({"op":"children","volume_id":volume,"parent_id":self.stack.last().and_then(|p|p.0),"offset":self.offset,"limit":LIMIT}));
        } else {
            self.entries.clear();
            self.total = 0;
            self.has_more = false;
            SendMessageW(self.files, LVM_DELETEALLITEMS, 0, 0);
            self.status("Choose a disk or search all disks.");
        }
    }
    unsafe fn poll(&mut self) {
        while let Ok(reply) = self.rx.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            if reply.database != self.db {
                continue;
            }
            let data = &reply.result["data"];
            if reply.tag == "cancel" {
                if reply.result["ok"] != true {
                    self.error(&reply.result);
                } else {
                    self.status("Cancellation requested…");
                }
                continue;
            }
            if matches!(
                reply.tag,
                "mutation" | "maintenance" | "linked_sync" | "linked_export"
            ) {
                self.job = None;
            }
            if matches!(
                reply.tag,
                "rows"
                    | "annotation_preview"
                    | "annotation_confirm"
                    | "annotation_overlay"
                    | "overlay_verified"
            ) && reply.generation != self.generation
            {
                continue;
            }
            if reply.tag == "annotation_confirm" {
                if reply.result["ok"] == true {
                    self.status("Selected mapping saved in user notes audit; remains inactive.");
                } else {
                    let code = reply.result["error"]["code"]
                        .as_str()
                        .unwrap_or("verification_failed");
                    self.status(&format!(
                        "Mapping rejected ({code}); reread conflicts before choosing again."
                    ));
                }
                self.request_annotation_page();
                continue;
            }
            if reply.tag == "annotation_overlay" {
                if reply.result["ok"] == true {
                    let state = reply.result["data"]["result"]["state"]
                        .as_str()
                        .unwrap_or("unexpected");
                    if state == "activated_read_only_overlay" {
                        self.status("Activation recorded in private audit; valid overlays require independent read verification.");
                    } else if state == "revoked_projection" {
                        self.status("Projection revoked; private original notes and audit snapshot retained.");
                    } else {
                        self.status(
                            "Unexpected activation response; review the audit state again.",
                        );
                    }
                } else {
                    let code = reply.result["error"]["code"]
                        .as_str()
                        .unwrap_or("verification_failed");
                    self.status(&format!(
                        "Overlay action rejected ({code}); reload conflicts before another decision."
                    ));
                }
                self.request_annotation_page();
                continue;
            }
            if reply.result["ok"] != true {
                if reply.tag == "linked_sync" {
                    self.pending_link_path = None;
                }
                self.error(&reply.result);
                continue;
            }
            match reply.tag {
                "overlay_verified" => {
                    match verified_overlay_action(self.hwnd, data, self.verified_overlay_offset) {
                        VerifiedOverlayNavigation::Next => {
                            self.verified_overlay_offset += ANNOTATION_PAGE_SIZE;
                            self.status("Freshly verifying next live overlay page…");
                            self.request_verified_overlay_page();
                        }
                        VerifiedOverlayNavigation::Previous => {
                            self.verified_overlay_offset = self
                                .verified_overlay_offset
                                .saturating_sub(ANNOTATION_PAGE_SIZE);
                            self.status("Freshly verifying previous live overlay page…");
                            self.request_verified_overlay_page();
                        }
                        VerifiedOverlayNavigation::Close => {
                            self.status("Verified view closed without changing private notes.");
                        }
                    }
                }
                "annotation_preview" => {
                    match annotation_review_action(self.hwnd, data, self.annotation_offset) {
                        AnnotationReviewAction::Next => {
                            self.annotation_offset += ANNOTATION_PAGE_SIZE;
                            self.status("Re-verifying next notes page…");
                            self.request_annotation_page();
                        }
                        AnnotationReviewAction::Previous => {
                            self.annotation_offset =
                                self.annotation_offset.saturating_sub(ANNOTATION_PAGE_SIZE);
                            self.status("Re-verifying previous notes page…");
                            self.request_annotation_page();
                        }
                        AnnotationReviewAction::Confirm(selection) => {
                            if let Some(mut pinned) =
                                self.linked_request("ctf_annotations_confirm_pinned")
                            {
                                pinned["annotation_db"] = json!(self.annotation_db);
                                if let Some(fields) = selection.as_object() {
                                    for (key, value) in fields {
                                        pinned[key] = value.clone();
                                    }
                                }
                                self.status("Revalidating exact CTF generation and note revision…");
                                self.call("annotation_confirm", pinned);
                            }
                        }
                        AnnotationReviewAction::Overlay(request) => {
                            if let Some(mut pinned) = request["op"]
                                .as_str()
                                .and_then(|op| self.linked_request(op))
                            {
                                pinned["annotation_db"] = json!(self.annotation_db);
                                if let Some(fields) = request.as_object() {
                                    for (key, value) in fields {
                                        pinned[key] = value.clone();
                                    }
                                }
                                self.status("Verifying source SHA and exact audit CAS before user-approved overlay action…");
                                self.call("annotation_overlay", pinned);
                            }
                        }
                        AnnotationReviewAction::Verified => {
                            self.verified_overlay_offset = 0;
                            self.status("Independently checking verified overlays and conflicts…");
                            self.request_verified_overlay_page();
                        }
                        AnnotationReviewAction::Close => {
                            self.status(
                                "Annotation review closed; no implicit mapping or migration.",
                            );
                        }
                    }
                }
                "linked_sync" => {
                    let path = self.pending_link_path.take();
                    let generation = data["generation"].as_str().map(str::to_owned);
                    if let (Some(path), Some(generation)) = (path, generation) {
                        self.linked_ctf = Some(path);
                        self.linked_generation = Some(generation);
                        self.annotation_db = None;
                        self.annotation_offset = 0;
                        self.verified_overlay_offset = 0;
                        SendMessageW(self.export_format, CB_SETCURSEL, 0, 0);
                        self.volume = None;
                        self.stack = vec![(None, "/".into())];
                        self.query.clear();
                        self.offset = 0;
                        self.duplicates = false;
                        self.entries.clear();
                        self.text(self.search, "");
                        self.status("External CTF verified; original kept read-only.");
                        self.refresh();
                    } else {
                        self.status("Linked CTF response is incomplete.");
                    }
                }
                "categories" => {
                    self.categories = data.as_array().cloned().unwrap_or_default();
                    SendMessageW(self.category_list, LB_RESETCONTENT, 0, 0);
                    for category in &self.categories {
                        SendMessageW(
                            self.category_list,
                            LB_ADDSTRING,
                            0,
                            wide(&string(category, "name")).as_ptr() as isize,
                        );
                    }
                }
                "category_created" => {
                    self.text(self.category_name, "");
                    self.call("categories", json!({"op":"categories"}));
                    self.status("Category created.");
                }
                "category_assigned" => self.status("Category assigned to selected entry."),
                "init" => {
                    if let Err(e) = std::fs::write(config_dir().join("catalog-path.txt"), &self.db)
                    {
                        self.status(&format!(
                            "Catalog opened; preference could not be saved: {e}"
                        ));
                    }
                    self.refresh();
                }
                "volumes" | "linked_volumes" => {
                    let values = if reply.tag == "linked_volumes" {
                        &data["result"]
                    } else {
                        data
                    };
                    self.volumes = values.as_array().cloned().unwrap_or_default();
                    SendMessageW(self.disks, LB_RESETCONTENT, 0, 0);
                    SendMessageW(
                        self.disks,
                        LB_ADDSTRING,
                        0,
                        wide("All disks / global search").as_ptr() as isize,
                    );
                    for disk in &self.volumes {
                        let label = format!(
                            "{}  ({} items)",
                            string(disk, "label"),
                            disk["entry_count"].as_u64().unwrap_or(0)
                        );
                        SendMessageW(self.disks, LB_ADDSTRING, 0, wide(&label).as_ptr() as isize);
                    }
                    let index = self
                        .volumes
                        .iter()
                        .position(|v| id(v) == self.volume)
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    SendMessageW(self.disks, LB_SETCURSEL, index, 0);
                    if self.entries.is_empty() {
                        self.status(&format!("{} disks in catalog", self.volumes.len()));
                    } else {
                        self.render_rows();
                    }
                }
                "rows" => {
                    let page = if self.linked_mode() {
                        &data["result"]
                    } else {
                        data
                    };
                    self.entries = page["items"].as_array().cloned().unwrap_or_default();
                    self.total_known = page["total"].is_u64();
                    self.total = page["total"]
                        .as_u64()
                        .map(|v| v as usize)
                        .unwrap_or(self.offset + self.entries.len());
                    self.has_more = page["has_more"]
                        .as_bool()
                        .unwrap_or(self.offset + self.entries.len() < self.total);
                    self.render_rows();
                }
                "maintenance" => {
                    self.status(
                        "Catalog verification completed. The original catalog was not replaced.",
                    );
                    MessageBoxW(
                        self.hwnd,
                        wide(&serde_json::to_string_pretty(data).unwrap_or_default()).as_ptr(),
                        wide("Catalog maintenance result").as_ptr(),
                        MB_OK,
                    );
                }
                "mutation" => {
                    self.status("Operation completed.");
                    self.refresh();
                }
                "linked_export" => {
                    let rows = data["result"]["rows"].as_u64().unwrap_or(0);
                    self.status(&format!(
                        "Verified linked CTF export: {rows} rows; source unchanged."
                    ));
                }
                _ => {}
            }
        }
        for (command, hwnd) in &self.buttons {
            EnableWindow(
                *hwnd,
                if *command == CANCEL {
                    self.job.as_ref().is_some_and(|j| j.starts_with("win-")) as i32
                } else if *command == ANNOTATION_PREVIEW {
                    (self.linked_mode() && self.job.is_none() && self.pending == 0) as i32
                } else if [OPEN, NEW, SCAN, IMPORT, LINK_CTF, EXPORT].contains(command) {
                    (self.job.is_none()
                        && self.pending == 0
                        && (!self.linked_mode()
                            || [OPEN, NEW, LINK_CTF].contains(command)
                            || (*command == EXPORT
                                && !self.duplicates
                                && SendMessageW(self.export_format, CB_GETCURSEL, 0, 0) != 2)))
                        as i32
                } else if [
                    CATEGORY_FILTER,
                    CATEGORY_ASSIGN,
                    CATEGORY_CREATE,
                    COMPACT_COPY,
                    VALIDATE_CATALOG,
                ]
                .contains(command)
                {
                    (!self.linked_mode()) as i32
                } else {
                    1
                },
            );
        }
        EnableWindow(self.export_format, 1);
        EnableWindow(self.category_list, (!self.linked_mode()) as i32);
        EnableWindow(self.category_name, (!self.linked_mode()) as i32);
    }
    unsafe fn error(&self, result: &Value) {
        let text = result["error"]["message"]
            .as_str()
            .unwrap_or("Unknown catalog error");
        self.status(text);
        MessageBoxW(
            self.hwnd,
            wide(text).as_ptr(),
            wide("WhereIsIt AI — error").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
    fn volume_label(&self, volume_id: Option<i64>) -> String {
        let volume_id = volume_id.unwrap_or(0);
        self.volumes
            .iter()
            .find(|v| id(v) == Some(volume_id))
            .map(|v| string(v, "label"))
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| format!("Disk #{volume_id}"))
    }
    unsafe fn render_rows(&self) {
        SendMessageW(self.files, LVM_DELETEALLITEMS, 0, 0);
        for (row, e) in self.entries.iter().enumerate() {
            let cols = [
                string(e, "name"),
                self.volume_label(e["volume_id"].as_i64()),
                string(e, "kind"),
                display_size(e["size"].as_u64()),
                e.get("modified_local")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| display_date(e["modified_unix"].as_i64())),
                e["duplicate_count"]
                    .as_u64()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                string(e, "relative_path"),
                string(e, "description"),
            ];
            for (col, text) in cols.iter().enumerate() {
                let mut w = wide(text);
                let mut item: LVITEMW = std::mem::zeroed();
                item.mask = LVIF_TEXT;
                item.iItem = row as i32;
                item.iSubItem = col as i32;
                item.pszText = w.as_mut_ptr();
                SendMessageW(
                    self.files,
                    if col == 0 {
                        LVM_INSERTITEMW
                    } else {
                        LVM_SETITEMW
                    },
                    0,
                    &item as *const _ as isize,
                );
            }
        }
        self.status(&format!(
            "{}–{}{}{}{}",
            if self.total == 0 { 0 } else { self.offset + 1 },
            self.offset + self.entries.len(),
            if self.total_known {
                format!(" of {} items", self.total)
            } else if self.has_more {
                " items • more available".into()
            } else {
                " items".into()
            },
            if self.duplicates {
                " • metadata candidates; content not verified"
            } else {
                ""
            },
            if self.job.is_some() {
                " • operation in progress"
            } else {
                ""
            }
        ));
    }
    unsafe fn activate(&mut self) {
        let selected = SendMessageW(
            self.files,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_SELECTED as isize,
        );
        if selected < 0 {
            return;
        }
        if let Some(e) = self.entries.get(selected as usize).cloned() {
            if string(&e, "kind") == "directory" || string(&e, "kind") == "archive" {
                self.volume = e["volume_id"].as_i64();
                self.stack.push((id(&e), string(&e, "relative_path")));
                self.query.clear();
                self.duplicates = false;
                self.text(self.search, "");
                self.offset = 0;
                self.load();
            } else {
                MessageBoxW(
                    self.hwnd,
                    wide(&format!(
                        "{}\n\n{}\n\nCatalog metadata; the original disk may be offline.",
                        string(&e, "relative_path"),
                        string(&e, "description")
                    ))
                    .as_ptr(),
                    wide("File details").as_ptr(),
                    MB_OK,
                );
            }
        }
    }
    unsafe fn duplicate_mode_name(&self) -> &'static str {
        match SendMessageW(self.duplicate_mode, CB_GETCURSEL, 0, 0) {
            1 => "size",
            2 => "name",
            _ => "name_size",
        }
    }
    unsafe fn command(&mut self, command: usize, notification: usize) {
        match command {
            COMPACT_COPY | VALIDATE_CATALOG
                if self.job.is_none() && self.pending == 0 && !self.linked_mode() =>
            {
                let request = if command == COMPACT_COPY {
                    let Some(path) = file_dialog(self.hwnd, true, "sqlite") else {
                        return;
                    };
                    json!({"op":"compact_catalog","path":path})
                } else {
                    json!({"op":"validate_catalog"})
                };
                self.job = Some("maintenance".into());
                self.status(
                    "Verifying catalog / writing a compact copy. Original stays unchanged...",
                );
                self.call("maintenance", request);
            }
            CATEGORY_FILTER | CATEGORY_ASSIGN if !self.linked_mode() => {
                let index = SendMessageW(self.category_list, LB_GETCURSEL, 0, 0);
                if index < 0 {
                    self.status("Select a category first.");
                    return;
                }
                let category = self
                    .categories
                    .get(index as usize)
                    .cloned()
                    .unwrap_or(Value::Null);
                if command == CATEGORY_FILTER {
                    self.query = format!(
                        "category:\"{}\"",
                        string(&category, "name")
                            .replace('\\', "\\\\")
                            .replace('"', "\\\"")
                    );
                    self.text(self.search, &self.query);
                    self.duplicates = false;
                    self.offset = 0;
                    self.load();
                } else {
                    let row = SendMessageW(
                        self.files,
                        LVM_GETNEXTITEM,
                        usize::MAX,
                        LVNI_SELECTED as isize,
                    );
                    if row < 0 {
                        self.status("Select a file or folder to assign this category.");
                        return;
                    }
                    if let Some(entry) = self.entries.get(row as usize) {
                        self.call("category_assigned",json!({"op":"assign_category","entry_id":id(entry),"category_id":id(&category)}));
                    }
                }
            }
            CATEGORY_CREATE if !self.linked_mode() => {
                let len = GetWindowTextLengthW(self.category_name);
                let mut text = vec![0u16; len as usize + 1];
                GetWindowTextW(self.category_name, text.as_mut_ptr(), len + 1);
                let name = String::from_utf16_lossy(&text[..len as usize])
                    .trim()
                    .to_owned();
                if name.is_empty() {
                    self.status("Enter a new category name in the sidebar.");
                } else {
                    self.call(
                        "category_created",
                        json!({"op":"create_category","name":name}),
                    );
                }
            }
            DISKS if notification == LBN_SELCHANGE as usize => {
                let index = SendMessageW(self.disks, LB_GETCURSEL, 0, 0);
                if index < 0 {
                    return;
                }
                self.volume = if index == 0 {
                    None
                } else {
                    self.volumes.get(index as usize - 1).and_then(id)
                };
                self.stack = vec![(None, "/".into())];
                self.query.clear();
                self.duplicates = false;
                self.text(self.search, "");
                self.offset = 0;
                self.load();
            }
            FIND | DUPLICATES => {
                let len = GetWindowTextLengthW(self.search);
                let mut text = vec![0u16; len as usize + 1];
                GetWindowTextW(self.search, text.as_mut_ptr(), len + 1);
                self.query = String::from_utf16_lossy(&text[..len as usize])
                    .trim()
                    .into();
                self.duplicates = command == DUPLICATES;
                if self.duplicates {
                    self.applied_duplicate_mode = self.duplicate_mode_name();
                }
                self.offset = 0;
                self.load();
            }
            BACK => {
                if !self.query.is_empty() || self.duplicates {
                    self.query.clear();
                    self.duplicates = false;
                    self.text(self.search, "");
                } else if self.stack.len() > 1 {
                    self.stack.pop();
                }
                self.offset = 0;
                self.load();
            }
            PREV => {
                self.offset = self.offset.saturating_sub(LIMIT);
                self.load();
            }
            NEXT => {
                if self.has_more {
                    self.offset += LIMIT;
                    self.load();
                }
            }
            CANCEL => {
                if let Some(job) = self.job.clone() {
                    self.call("cancel", json!({"op":"cancel","job_id":job}));
                }
            }
            ANNOTATION_PREVIEW if self.linked_mode() && self.job.is_none() && self.pending == 0 => {
                if let Some(path) = file_dialog(self.hwnd, false, "annotation") {
                    self.annotation_db = Some(path);
                    self.annotation_offset = 0;
                    self.verified_overlay_offset = 0;
                    self.status("Verifying CTF generation and annotation conflicts…");
                    self.request_annotation_page();
                }
            }
            OPEN | NEW | SCAN | IMPORT | LINK_CTF | EXPORT
                if self.job.is_none() && self.pending == 0 =>
            {
                if self.linked_mode() && [SCAN, IMPORT].contains(&command) {
                    self.status("Linked CTF is read-only; open a writable SQLite catalog first.");
                    return;
                }
                if self.linked_mode() && command == EXPORT {
                    if self.duplicates {
                        self.status(
                            "Linked duplicate candidates cannot be exported until a verified duplicate report is implemented.",
                        );
                        return;
                    }
                    if SendMessageW(self.export_format, CB_GETCURSEL, 0, 0) == 2 {
                        self.status(
                            "Linked CTF supports only CSV/Parquet export; original stays read-only.",
                        );
                        return;
                    }
                }
                if command == SCAN {
                    if let Some(root) = folder_dialog(self.hwnd) {
                        let label = PathBuf::from(&root)
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| root.clone());
                        let job = format!(
                            "win-{}",
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos()
                        );
                        self.job = Some(job.clone());
                        self.status("Scanning disk… You can browse or cancel while the scan runs.");
                        self.call(
                            "mutation",
                            json!({"op":"scan","root":root,"label":label,"job_id":job}),
                        );
                    }
                } else if let Some(path) = file_dialog(
                    self.hwnd,
                    command == NEW || command == EXPORT,
                    if command == IMPORT || command == LINK_CTF {
                        "ctf"
                    } else if command == EXPORT {
                        match SendMessageW(self.export_format, CB_GETCURSEL, 0, 0) {
                            0 => "csv",
                            1 => "parquet",
                            _ => "ctf",
                        }
                    } else {
                        "sqlite"
                    },
                ) {
                    match command {
                        OPEN | NEW => {
                            self.db = path;
                            self.init();
                        }
                        LINK_CTF => {
                            let cache_root = config_dir()
                                .join("CTF-Mirrors")
                                .to_string_lossy()
                                .into_owned();
                            self.pending_link_path = Some(path.clone());
                            self.job = Some("linked-sync".into());
                            self.status("Verifying original CTF and creating read-only mirror…");
                            self.call(
                                "linked_sync",
                                json!({"op":"ctf_sync","path":path,"cache_root":cache_root}),
                            );
                        }
                        IMPORT | EXPORT => {
                            self.job = Some("file-operation".into());
                            self.status(if command == IMPORT {
                                "Importing CTF…"
                            } else {
                                "Exporting selected scope…"
                            });
                            let operation = if command == IMPORT {
                                "import_ctf"
                            } else {
                                match SendMessageW(self.export_format, CB_GETCURSEL, 0, 0) {
                                    0 => "export_csv",
                                    1 => "export_parquet",
                                    _ => "export_ctf",
                                }
                            };
                            if command == EXPORT && self.linked_mode() {
                                let Some(mut linked) = self.linked_request("ctf_export_pinned")
                                else {
                                    self.job = None;
                                    self.status("Linked generation is unavailable; refresh CTF.");
                                    return;
                                };
                                linked["output"] = json!(path);
                                linked["format"] = json!(if operation == "export_csv" {
                                    "csv"
                                } else {
                                    "parquet"
                                });
                                linked["mode"] = json!(if self.duplicates {
                                    "duplicates"
                                } else if self.query.is_empty() && self.volume.is_some() {
                                    "children"
                                } else {
                                    "search"
                                });
                                linked["query"] = json!(self.query);
                                linked["volume_id"] = json!(self.volume);
                                linked["duplicate_mode"] = json!(self.applied_duplicate_mode);
                                if !self.duplicates {
                                    linked["parent_id"] =
                                        json!(self.stack.last().and_then(|p| p.0));
                                }
                                self.call("linked_export", linked);
                            } else {
                                self.call("mutation", json!({"op":operation,"path":path,"overwrite":command==EXPORT,"mode":if self.duplicates {"duplicates"} else if self.query.is_empty()&&self.volume.is_some(){"children"}else{"search"},"query":self.query,"volume_id":self.volume,"parent_id":self.stack.last().and_then(|p|p.0),"duplicate_mode":self.applied_duplicate_mode,"recursive":true}));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    unsafe fn layout(&self) {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(self.hwnd, &mut rect);
        let w = rect.right;
        let h = rect.bottom;
        let mut x = 8;
        for (i, hwnd) in &self.buttons {
            if [OPEN, NEW, SCAN, IMPORT, LINK_CTF, EXPORT, CANCEL].contains(i) {
                MoveWindow(*hwnd, x, 6, 86, 24, 1);
                x += 90;
            }
        }
        MoveWindow(self.export_format, 648, 6, 188, 200, 1);
        MoveWindow(self.search, 8, 34, (w - 470).max(100), 24, 1);
        MoveWindow(self.duplicate_mode, w - 268, 34, 136, 200, 1);
        for (i, hwnd) in &self.buttons {
            if *i == FIND {
                MoveWindow(*hwnd, w - 456, 34, 80, 24, 1);
            }
            if *i == BACK {
                MoveWindow(*hwnd, w - 370, 34, 96, 24, 1);
            }
            if *i == DUPLICATES {
                MoveWindow(*hwnd, w - 126, 34, 118, 24, 1);
            }
            if *i == PREV {
                MoveWindow(*hwnd, w - 200, h - 32, 90, 24, 1);
            }
            if *i == NEXT {
                MoveWindow(*hwnd, w - 102, h - 32, 94, 24, 1);
            }
        }
        MoveWindow(self.location, 8, 83, (w - 16).max(50), 20, 1);
        let sidebar_height = (h - 146).max(200);
        let disk_height = (sidebar_height / 2).max(70);
        MoveWindow(self.disks, 8, 106, 200, disk_height, 1);
        MoveWindow(
            self.category_list,
            8,
            112 + disk_height,
            200,
            (sidebar_height - disk_height - 66).max(40),
            1,
        );
        MoveWindow(self.category_name, 8, h - 92, 150, 24, 1);
        for (command, control) in &self.buttons {
            match *command {
                CATEGORY_CREATE => {
                    MoveWindow(*control, 164, h - 92, 44, 24, 1);
                }
                ANNOTATION_PREVIEW => {
                    MoveWindow(*control, w - 144, 62, 136, 23, 1);
                }
                CATEGORY_FILTER => {
                    MoveWindow(*control, 8, h - 64, 88, 24, 1);
                }
                CATEGORY_ASSIGN => {
                    MoveWindow(*control, 102, h - 64, 106, 24, 1);
                }
                _ => {}
            }
        }
        MoveWindow(
            self.files,
            214,
            106,
            (w - 222).max(100),
            (h - 146).max(60),
            1,
        );
        MoveWindow(self.status, 8, h - 32, (w - 216).max(50), 24, 1);
    }
}

unsafe fn file_dialog(owner: HWND, save: bool, format: &str) -> Option<String> {
    let mut buffer = vec![0u16; 32768];
    let filter=wide(match format {"annotation"=>"User notes (*.wia.sqlite)\0*.wia.sqlite\0","ctf"=>"CTF catalogs (*.ctf)\0*.ctf\0All files\0*.*\0","csv"=>"CSV table (*.csv)\0*.csv\0All files\0*.*\0","parquet"=>"Apache Parquet (*.parquet)\0*.parquet\0All files\0*.*\0",_=>"Atlas SQLite catalog (*.atlas.sqlite;*.sqlite)\0*.atlas.sqlite;*.sqlite\0All files\0*.*\0"});
    let ext = wide(if format == "sqlite" {
        "atlas.sqlite"
    } else if format == "annotation" {
        "wia.sqlite"
    } else {
        format
    });
    let mut ofn: OPENFILENAMEW = std::mem::zeroed();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = buffer.as_mut_ptr();
    ofn.nMaxFile = buffer.len() as u32;
    ofn.lpstrDefExt = ext.as_ptr();
    ofn.Flags = OFN_EXPLORER
        | OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | if save {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };
    let ok = if save {
        GetSaveFileNameW(&mut ofn)
    } else {
        GetOpenFileNameW(&mut ofn)
    };
    if ok != 0 {
        Some(String::from_utf16_lossy(
            &buffer[..buffer.iter().position(|v| *v == 0).unwrap_or(0)],
        ))
    } else {
        None
    }
}
unsafe fn folder_dialog(owner: HWND) -> Option<String> {
    let title = wide("Choose a disk or folder to catalog");
    let mut bi: BROWSEINFOW = std::mem::zeroed();
    bi.hwndOwner = owner;
    bi.lpszTitle = title.as_ptr();
    bi.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
    let pidl = SHBrowseForFolderW(&bi);
    if pidl.is_null() {
        return None;
    }
    let mut path = vec![0u16; 32768];
    let ok = SHGetPathFromIDListEx(pidl, path.as_mut_ptr(), path.len() as u32, 0);
    CoTaskMemFree(pidl as _);
    if ok != 0 {
        Some(String::from_utf16_lossy(
            &path[..path.iter().position(|v| *v == 0).unwrap_or(0)],
        ))
    } else {
        None
    }
}
unsafe fn control(parent: HWND, class: &str, text: &str, style: u32, id: usize) -> HWND {
    let hwnd = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(text).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        0,
        0,
        0,
        0,
        parent,
        id as _,
        GetModuleHandleW(null()),
        null(),
    );
    SendMessageW(
        hwnd,
        WM_SETFONT,
        GetStockObject(DEFAULT_GUI_FONT) as usize,
        1,
    );
    hwnd
}
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = &*(lp as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let app = &mut *ptr;
    match msg {
        WM_CREATE => {
            app.hwnd = hwnd;
            for (id, text) in [
                (OPEN, "&Open catalog"),
                (NEW, "&New catalog"),
                (SCAN, "&Add disk"),
                (IMPORT, "&Import CTF"),
                (LINK_CTF, "&Link CTF"),
                (ANNOTATION_PREVIEW, "Review &notes…"),
                (EXPORT, "&Export…"),
                (CANCEL, "&Cancel scan"),
                (FIND, "&Search"),
                (BACK, "&Back"),
                (PREV, "&Previous"),
                (NEXT, "Ne&xt"),
                (DUPLICATES, "&Duplicates"),
                (CATEGORY_FILTER, "Filter category"),
                (CATEGORY_ASSIGN, "Assign category"),
                (CATEGORY_CREATE, "+"),
            ] {
                app.buttons.push((
                    id,
                    control(hwnd, "BUTTON", text, WS_TABSTOP | BS_PUSHBUTTON as u32, id),
                ));
            }
            app.search = control(
                hwnd,
                "EDIT",
                "",
                WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                130,
            );
            app.category_list = control(
                hwnd,
                "LISTBOX",
                "",
                WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY as u32,
                CATEGORIES,
            );
            app.category_name = control(
                hwnd,
                "EDIT",
                "",
                WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                126,
            );
            SendMessageW(
                app.category_name,
                EM_SETCUEBANNER,
                0,
                wide("New category…").as_ptr() as isize,
            );
            app.duplicate_mode = control(
                hwnd,
                "COMBOBOX",
                "",
                WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                DUP_MODE,
            );
            for label in ["Name + size", "Size", "Name"] {
                SendMessageW(
                    app.duplicate_mode,
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
                );
            }
            SendMessageW(app.duplicate_mode, CB_SETCURSEL, 0, 0);
            app.export_format = control(
                hwnd,
                "COMBOBOX",
                "",
                WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                EXPORT_FORMAT,
            );
            for label in [
                "CSV — current branch/query",
                "Parquet — current branch/query",
                "CTF — entire catalog",
            ] {
                SendMessageW(
                    app.export_format,
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
                );
            }
            SendMessageW(app.export_format, CB_SETCURSEL, 0, 0);
            let help=control(hwnd,"STATIC","AND  OR  NOT  ( )   name: path: ext: size: category:   Example: ext:mkv AND size:>1GB",0,142);
            MoveWindow(help, 8, 62, 700, 18, 1);
            app.disks = control(
                hwnd,
                "LISTBOX",
                "",
                WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY as u32,
                DISKS,
            );
            app.files = control(
                hwnd,
                "SysListView32",
                "",
                WS_TABSTOP | WS_BORDER | LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS,
                FILES,
            );
            SendMessageW(
                app.files,
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                0,
                (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | LVS_EX_LABELTIP) as isize,
            );
            for (i, (name, width)) in [
                ("Name", 220),
                ("Disk", 150),
                ("Kind", 80),
                ("Size", 100),
                ("Modified", 140),
                ("Matches", 70),
                ("Path", 280),
                ("Description", 260),
            ]
            .iter()
            .enumerate()
            {
                let mut name = wide(name);
                let mut col: LVCOLUMNW = std::mem::zeroed();
                col.mask = LVCF_TEXT | LVCF_WIDTH;
                col.pszText = name.as_mut_ptr();
                col.cx = *width;
                SendMessageW(app.files, LVM_INSERTCOLUMNW, i, &col as *const _ as isize);
            }
            app.status = control(hwnd, "STATIC", "", 0, 140);
            app.location = control(hwnd, "STATIC", "", SS_PATHELLIPSIS, 141);
            SetTimer(hwnd, 1, 80, None);
            app.init();
            0
        }
        WM_SIZE => {
            app.layout();
            0
        }
        WM_COMMAND => {
            app.command(wp & 0xffff, (wp >> 16) & 0xffff);
            0
        }
        WM_NOTIFY => {
            let header = &*(lp as *const NMHDR);
            if header.idFrom == FILES && header.code == NM_DBLCLK {
                app.activate();
            } else if header.idFrom == FILES && header.code == LVN_KEYDOWN {
                let key = &*(lp as *const NMLVKEYDOWN);
                if key.wVKey == VK_RETURN {
                    app.activate();
                }
            }
            0
        }
        WM_TIMER => {
            app.poll();
            0
        }
        WM_GETMINMAXINFO => {
            let info = &mut *(lp as *mut MINMAXINFO);
            info.ptMinTrackSize = POINT { x: 880, y: 430 };
            0
        }
        WM_CLOSE => {
            if app.job.is_some()
                && MessageBoxW(
                    hwnd,
                    wide("An operation is in progress. Cancel it and close WhereIsIt AI?").as_ptr(),
                    wide("Close WhereIsIt AI").as_ptr(),
                    MB_YESNO | MB_ICONQUESTION,
                ) != IDYES
            {
                return 0;
            }
            if let Some(job) = app.job.clone() {
                app.call("cancel", json!({"op":"cancel","job_id":job}));
            }
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, 1);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn run() {
    unsafe {
        CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES,
        });
        let instance = GetModuleHandleW(null());
        let class = wide("AtlasCatalogWindow");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(window_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.hbrBackground = (COLOR_WINDOW + 1) as _;
        RegisterClassW(&wc);
        let app = Box::into_raw(Box::new(App::new()));
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide(concat!("WhereIsIt AI ", env!("CARGO_PKG_VERSION"))).as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1180,
            740,
            null_mut(),
            null_mut(),
            instance,
            app as _,
        );
        if hwnd.is_null() {
            drop(Box::from_raw(app));
            MessageBoxW(
                null_mut(),
                wide("Unable to create the native application window.").as_ptr(),
                wide(concat!("WhereIsIt AI ", env!("CARGO_PKG_VERSION"))).as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            return;
        }
        let menu = CreateMenu();
        let maintenance = CreatePopupMenu();
        if !menu.is_null() && !maintenance.is_null() {
            AppendMenuW(
                maintenance,
                MF_STRING,
                VALIDATE_CATALOG,
                wide("Verify catalog").as_ptr(),
            );
            AppendMenuW(
                maintenance,
                MF_STRING,
                COMPACT_COPY,
                wide("Create compact copy...").as_ptr(),
            );
            AppendMenuW(
                menu,
                MF_POPUP,
                maintenance as usize,
                wide("Catalog").as_ptr(),
            );
            SetMenu(hwnd, menu);
            DrawMenuBar(hwnd);
        }
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            if message.message == WM_KEYDOWN
                && message.wParam == VK_RETURN as usize
                && GetFocus() == (*app).search
            {
                (*app).command(FIND, 0);
                continue;
            }
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        drop(Box::from_raw(app));
        CoUninitialize();
    }
}
