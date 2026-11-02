use serde_json::{json, Value};
use std::cmp::Ordering;
use std::{
    collections::HashMap,
    path::PathBuf,
    ptr::{null, null_mut},
    sync::mpsc::{self, Receiver, Sender},
    time::{SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    Storage::FileSystem::{
        FileTimeToLocalFileTime, FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, GetDriveTypeW,
        GetVolumeInformationW, GetVolumePathNamesForVolumeNameW,
    },
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
const FIND_DISK: usize = 127;
const PREV: usize = 108;
const NEXT: usize = 109;
const CANCEL: usize = 110;
const DUPLICATES: usize = 111;
const FILE_COPIES: usize = 116;
const PHYSICAL_CHECK: usize = 117;
const PHYSICAL_PAIR: usize = 118;
const PHYSICAL_UNPAIR: usize = 119;
const SHELF_SAVE: usize = 133;
const COPY_LOCATOR: usize = 135;
const REVEAL_DISK: usize = 136;
const COPY_PICKUP_PLAN: usize = 137;
const COPY_PAGE_MANIFEST: usize = 138;
const COPY_REGISTER_FILTER: usize = 139;
const FOCUS_RESULTS: usize = 140;
const VIDEO_FILTER: usize = 141;
const SAME_FILENAME: usize = 143;
const SAME_RECORDED_SIZE: usize = 144;
const PHYSICAL_CONNECTED_FILTER: usize = 145;
const PHYSICAL_UNVERIFIED_FILTER: usize = 146;
const STOP_PAGE_WAIT: usize = 147;
const RETRY_PAGE: usize = 148;
const COPY_VERIFIED_OS_ROOT: usize = 149;
const SEARCH_BOX: usize = 130;
// Win32 GetDriveTypeW standard result codes; windows-sys 0.61 does not export these here.
// Unknown/missing roots mean a potentially incomplete OS snapshot, never "offline".
const DRIVE_UNKNOWN: u32 = 0;
const DRIVE_NO_ROOT_DIR: u32 = 1;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DUP_MODE: usize = 112;
const EXPORT_FORMAT: usize = 113;
const LINK_CTF: usize = 114;
const ANNOTATION_PREVIEW: usize = 115;
const ANNOTATION_PAGE_SIZE: usize = 20;
const DISKS: usize = 120;
const DISK_FILTER: usize = 128;
const TOGGLE_LOCATOR: usize = 129;
const COPY_DISK_FILTER: usize = 131;
const COPY_CONNECTED: usize = 132;
const FILES: usize = 121;
const CATEGORIES: usize = 122;
const CATEGORY_FILTER: usize = 123;
const CATEGORY_ASSIGN: usize = 124;
const CATEGORY_CREATE: usize = 125;
const LIMIT: usize = 250;

// A dismissed asynchronous catalog page must never regain authority, even
// when the old worker finishes successfully after a new request or a stop.
fn page_response_current(reply_generation: u64, active_generation: u64, waiting: bool) -> bool {
    waiting && reply_generation == active_generation
}

#[cfg(test)]
mod page_wait_controls_tests {
    use super::page_response_current;

    #[test]
    fn stopped_or_superseded_page_can_never_reactivate_old_rows() {
        assert!(page_response_current(8, 8, true));
        assert!(!page_response_current(8, 8, false));
        assert!(!page_response_current(8, 9, true));
        assert!(!page_response_current(9, 8, true));
        // Generation wrap after many future requests still rejects the old token.
        assert!(!page_response_current(u64::MAX, 0, true));
    }
}

fn linked_export_allowed(index: isize) -> bool {
    matches!(index, 0 | 1 | 3)
}

#[cfg(test)]
mod linked_export_policy_tests {
    use super::linked_export_allowed;
    #[test]
    fn csv_parquet_json_enabled_but_ctf_and_unknown_refused() {
        for index in [0, 1, 3] {
            assert!(linked_export_allowed(index));
        }
        for index in [-1, 2, 4, isize::MAX] {
            assert!(!linked_export_allowed(index));
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn string(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}
fn id(v: &Value) -> Option<i64> {
    v.get("id").and_then(Value::as_i64)
}

// Reversible visible query predicate; this does not inspect live USB file bytes.
fn video_filter_active(raw: &str) -> bool {
    let query = raw.trim();
    query == "type:video"
        || query
            .strip_suffix(" AND type:video")
            .is_some_and(|base| base.starts_with('(') && base.ends_with(')'))
}
fn toggle_video_query(raw: &str) -> Option<String> {
    let query = raw.trim();
    let next = if query == "type:video" {
        String::new()
    } else if video_filter_active(query) {
        let wrapped = query.strip_suffix(" AND type:video")?;
        wrapped[1..wrapped.len() - 1].to_owned()
    } else if query.is_empty() {
        "type:video".to_owned()
    } else {
        format!("({query}) AND type:video")
    };
    (next.len() <= 4096).then_some(next)
}
#[cfg(test)]
mod video_query_tests {
    use super::{toggle_video_query, video_filter_active};

    #[test]
    fn reversible_unicode_or_query_and_empty_query() {
        let base = "name:Герой OR name:Hero";
        let expanded = toggle_video_query(base).unwrap();
        assert_eq!(expanded, "(name:Герой OR name:Hero) AND type:video");
        assert!(video_filter_active(&expanded));
        assert_eq!(toggle_video_query(&expanded).unwrap(), base);
        assert_eq!(toggle_video_query("").unwrap(), "type:video");
        assert_eq!(toggle_video_query("type:video").unwrap(), "");
    }

    #[test]
    fn no_truncation_of_overlong_query() {
        assert!(toggle_video_query(&"я".repeat(2048)).is_none());
        assert!(!video_filter_active("name:video"));
    }
}

// Quote the selected catalogued name as a literal (not a glob or SQL fragment).
// The user can edit the resulting global query; CTF pinning is checked by load().
fn exact_name_query(name: &str) -> Option<String> {
    if name.is_empty() || name.contains('\0') {
        return None;
    }
    let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
    let result = format!("name_exact:\"{escaped}\"");
    (result.len() <= 4096).then_some(result)
}
#[cfg(test)]
mod exact_name_query_tests {
    use super::exact_name_query;

    #[test]
    fn filename_is_quoted_without_globs_or_truncation() {
        assert_eq!(
            exact_name_query("Герой [HD] \"alt\".mkv").as_deref(),
            Some("name_exact:\"Герой [HD] \\\"alt\\\".mkv\"")
        );
        assert!(exact_name_query("").is_none());
        assert!(exact_name_query(&"я".repeat(2048)).is_none());
        assert!(exact_name_query("bad\0name").is_none());
    }
}

// Find potential renamed media by exact recorded *byte count* only.
// Refuse unknown, zero and non-SQLite signed-64-bit sizes; this is not hashing.
fn exact_recorded_size_query(size: Option<u64>) -> Option<String> {
    let bytes = size.filter(|bytes| *bytes > 0 && *bytes <= i64::MAX as u64)?;
    Some(format!("size:{bytes} AND kind:file"))
}
#[cfg(test)]
mod exact_recorded_size_query_tests {
    use super::exact_recorded_size_query;

    #[test]
    fn positive_exact_bytes_only_and_overflow_refused() {
        assert_eq!(
            exact_recorded_size_query(Some(1_048_576)).as_deref(),
            Some("size:1048576 AND kind:file")
        );
        assert!(exact_recorded_size_query(None).is_none());
        assert!(exact_recorded_size_query(Some(0)).is_none());
        assert!(exact_recorded_size_query(Some(i64::MAX as u64 + 1)).is_none());
    }
}

// Header sorting is intentionally limited to the loaded result page.
fn cmp_text(left: &str, right: &str, descending: bool) -> Ordering {
    let result = left.to_lowercase().cmp(&right.to_lowercase());
    if descending {
        result.reverse()
    } else {
        result
    }
}

fn cmp_present<T: Ord>(left: Option<T>, right: Option<T>, descending: bool) -> Ordering {
    match (left, right) {
        (Some(a), Some(b)) => {
            let result = a.cmp(&b);
            if descending {
                result.reverse()
            } else {
                result
            }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

// Never focus a stale ListView row while a page or linked CTF SHA check is pending.
fn focusable_result_row(page_ready: bool, rows: &[Value], selected_row: isize) -> Option<usize> {
    if !page_ready || rows.is_empty() {
        return None;
    }
    usize::try_from(selected_row)
        .ok()
        .filter(|index| *index < rows.len())
        .or(Some(0))
}

const SORT_LABELS: [&str; 7] = [
    "Name",
    "Disk",
    "Kind",
    "Size",
    "Modified",
    "Candidates",
    "Path",
];

fn sort_page_rows(
    rows: &mut [Value],
    column: usize,
    descending: bool,
    labels: &HashMap<i64, String>,
) {
    rows.sort_by(|a, b| {
        let order = match column {
            0 => cmp_text(&string(a, "name"), &string(b, "name"), descending),
            1 => {
                let label = |row: &Value| {
                    labels
                        .get(&row["volume_id"].as_i64().unwrap_or_default())
                        .map(String::as_str)
                        .unwrap_or("")
                };
                cmp_text(label(a), label(b), descending)
            }
            2 => cmp_text(&string(a, "kind"), &string(b, "kind"), descending),
            3 => cmp_present(a["size"].as_u64(), b["size"].as_u64(), descending),
            4 => cmp_present(
                a["modified_unix"].as_i64(),
                b["modified_unix"].as_i64(),
                descending,
            ),
            5 => cmp_present(
                a["duplicate_count"].as_u64(),
                b["duplicate_count"].as_u64(),
                descending,
            ),
            6 => cmp_text(
                &string(a, "relative_path"),
                &string(b, "relative_path"),
                descending,
            ),
            _ => Ordering::Equal,
        };
        order.then_with(|| id(a).cmp(&id(b)))
    });
}

// Versioned UI-only preference: no search query, source path/SHA or pairing ID.
fn parse_ui_layout_preferences(raw: &str) -> (bool, i32) {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return (true, 282);
    };
    if value["version"].as_u64() != Some(1) {
        return (true, 282);
    }
    (
        value["locator_visible"].as_bool().unwrap_or(true),
        value["locator_width"]
            .as_i64()
            .unwrap_or(282)
            .clamp(220, 420) as i32,
    )
}
fn read_ui_layout_preferences() -> (bool, i32) {
    std::fs::read_to_string(config_dir().join("ui-layout.json"))
        .ok()
        .map(|raw| parse_ui_layout_preferences(&raw))
        .unwrap_or((true, 282))
}

#[cfg(test)]
mod page_sort_and_layout_tests {
    use super::{focusable_result_row, id, parse_ui_layout_preferences, sort_page_rows};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn sort_only_loaded_entries_with_stable_ids_and_missing_metadata_last() {
        let labels = HashMap::from([(7, "Zebra".to_owned()), (8, "Alpha".to_owned())]);
        let mut rows = vec![
            json!({"id":3,"name":"Movie","volume_id":7,"size":null}),
            json!({"id":2,"name":"movie","volume_id":8,"size":500}),
            json!({"id":1,"name":"Other","volume_id":7,"size":300}),
        ];
        sort_page_rows(&mut rows, 3, true, &labels);
        assert_eq!(
            rows.iter()
                .map(|e| e["id"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            vec![2, 1, 3]
        );
        sort_page_rows(&mut rows, 3, false, &labels);
        assert_eq!(
            rows.iter()
                .map(|e| e["id"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        sort_page_rows(&mut rows, 1, false, &labels);
        assert_eq!(rows[0]["id"], 2);
    }

    #[test]
    fn kind_page_sort_distinguishes_catalog_entry_kinds_without_content_probing() {
        let labels = HashMap::new();
        let mut rows = vec![
            json!({"id":10,"kind":"file"}),
            json!({"id":12,"kind":"directory"}),
            json!({"id":11,"kind":"archive"}),
        ];
        sort_page_rows(&mut rows, 2, false, &labels);
        assert_eq!(
            rows.iter().filter_map(id).collect::<Vec<_>>(),
            vec![11, 12, 10]
        );
        sort_page_rows(&mut rows, 2, true, &labels);
        assert_eq!(
            rows.iter().filter_map(id).collect::<Vec<_>>(),
            vec![10, 12, 11]
        );
    }

    #[test]
    fn results_focus_rejects_pending_or_empty_page_and_ignores_stale_row() {
        let rows = vec![json!({"id":42}), json!({"id":142})];
        assert_eq!(focusable_result_row(false, &rows, 0), None);
        assert_eq!(focusable_result_row(true, &[], -1), None);
        assert_eq!(focusable_result_row(true, &rows, -1), Some(0));
        assert_eq!(focusable_result_row(true, &rows, 90), Some(0));
        assert_eq!(focusable_result_row(true, &rows, 1), Some(1));
    }

    #[test]
    fn corrupt_layout_falls_back_and_user_splitter_width_is_bounded() {
        assert_eq!(parse_ui_layout_preferences("broken"), (true, 282));
        assert_eq!(parse_ui_layout_preferences(r#"{"version":2}"#), (true, 282));
        assert_eq!(
            parse_ui_layout_preferences(
                r#"{"version":1,"locator_visible":false,"locator_width":9000}"#
            ),
            (false, 420)
        );
        assert_eq!(
            parse_ui_layout_preferences(r#"{"version":1,"locator_width":0}"#),
            (true, 220)
        );
    }
}

// This is a view-only filter for the physical disk register; it must never
// become a query-volume filter or rewrite the selected catalog disk.
fn filtered_disk_indices(
    volumes: &[Value],
    shelf_labels: &HashMap<i64, String>,
    filter: &str,
) -> Vec<usize> {
    let needle = filter.trim().to_lowercase();
    // Exact catalog ID: no partial number, misleading label or shelf match.
    if let Some(raw_id) = needle.strip_prefix("id:") {
        let Ok(requested_id) = raw_id.trim().parse::<i64>() else {
            return Vec::new();
        };
        if requested_id <= 0 {
            return Vec::new();
        }
        return volumes
            .iter()
            .enumerate()
            .filter_map(|(index, volume)| (id(volume) == Some(requested_id)).then_some(index))
            .collect();
    }
    volumes
        .iter()
        .enumerate()
        .filter_map(|(index, volume)| {
            let label = string(volume, "label").to_lowercase();
            let catalog_id = id(volume).map(|v| v.to_string()).unwrap_or_default();
            let shelf = id(volume)
                .and_then(|disk| shelf_labels.get(&disk))
                .map(|text| text.to_lowercase())
                .unwrap_or_default();
            if needle.is_empty()
                || label.contains(&needle)
                || catalog_id.contains(&needle)
                || shelf.contains(&needle)
            {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

// Register audit works on an OS-probed, explicitly paired catalog ID only.
// Historical mount paths, catalog labels and shelf tags cannot assert presence.
fn audited_physical_state<'a>(
    disk: i64,
    states: &'a HashMap<i64, String>,
    probe_ok: bool,
) -> &'a str {
    if !probe_ok {
        return "unverified";
    }
    match states.get(&disk).map(String::as_str) {
        Some("connected") => "connected",
        Some("offline") => "offline",
        Some("conflict") => "conflict",
        _ => "unverified",
    }
}

// None means an ordinary label/ID filter handled by filtered_disk_indices.
// Invalid reserved prefixes match NOTHING; they never fall through to a fuzzy ID.
fn audited_register_filter_indices(
    volumes: &[Value],
    shelves: &HashMap<i64, String>,
    shared_stickers: &HashMap<i64, Vec<i64>>,
    states: &HashMap<i64, String>,
    probe_ok: bool,
    filter: &str,
) -> Option<Vec<usize>> {
    let needle = filter.trim().to_lowercase();
    let state = needle.strip_prefix("state:");
    let tag = needle.strip_prefix("tag:");
    if state.is_none() && tag.is_none() {
        return None;
    }
    Some(
        volumes
            .iter()
            .enumerate()
            .filter_map(|(index, volume)| {
                let disk = id(volume).filter(|disk| *disk > 0)?;
                let matched = if let Some(requested) = state {
                    match requested.trim() {
                        "connected" | "offline" | "conflict" | "unverified" => {
                            audited_physical_state(disk, states, probe_ok) == requested.trim()
                        }
                        _ => false,
                    }
                } else {
                    match tag.unwrap_or_default().trim() {
                        "missing" => !shelves
                            .get(&disk)
                            .is_some_and(|label| !label.trim().is_empty()),
                        "shared" => shared_stickers
                            .get(&disk)
                            .is_some_and(|members| members.iter().any(|other| *other != disk)),
                        _ => false,
                    }
                };
                matched.then_some(index)
            })
            .collect(),
    )
}

// Complete registered catalog, not just the currently filtered/selected row.
// Counts are snapshots from the last successfully checked OS response; "unknown"
// includes all disks if the native OS scan was incomplete.
fn register_audit_counts(
    volumes: &[Value],
    shelves: &HashMap<i64, String>,
    shared_stickers: &HashMap<i64, Vec<i64>>,
    states: &HashMap<i64, String>,
    probe_ok: bool,
) -> [usize; 6] {
    // connected, paired offline, conflict, unverified, no sticker, shared sticker
    let mut counts = [0usize; 6];
    for volume in volumes {
        let Some(disk) = id(volume).filter(|disk| *disk > 0) else {
            continue;
        };
        let state_index = match audited_physical_state(disk, states, probe_ok) {
            "connected" => 0,
            "offline" => 1,
            "conflict" => 2,
            _ => 3,
        };
        counts[state_index] += 1;
        if !shelves
            .get(&disk)
            .is_some_and(|label| !label.trim().is_empty())
        {
            counts[4] += 1;
        }
        if shared_stickers
            .get(&disk)
            .is_some_and(|members| members.iter().any(|other| *other != disk))
        {
            counts[5] += 1;
        }
    }
    counts
}

// A view-only shortcut for the physical pickup workflow: IDs must come from
// the current catalogued copy groups, never fuzzy labels or historical paths.
// Candidate-register order is the physical pickup order: last OS observation,
// then the COMPLETE per-disk metadata count, then exact catalog ID. The
// returned indices still point into the original volume array.
fn candidate_register_indices(
    volumes: &[Value],
    groups: &[Value],
    states: &HashMap<i64, String>,
) -> Vec<usize> {
    let mut candidates: HashMap<i64, &Value> = HashMap::new();
    for group in groups {
        let Some(disk) = group["volume_id"].as_i64().filter(|disk| *disk > 0) else {
            continue;
        };
        let Some(count) = group["count"].as_u64().filter(|count| *count > 0) else {
            continue;
        };
        let incumbent = candidates.entry(disk).or_insert(group);
        if count > incumbent["count"].as_u64().unwrap_or(0) {
            *incumbent = group;
        }
    }
    let mut indices: Vec<usize> = volumes
        .iter()
        .enumerate()
        .filter_map(|(index, volume)| {
            id(volume)
                .filter(|disk| candidates.contains_key(disk))
                .map(|_| index)
        })
        .collect();
    indices.sort_by_key(|index| {
        let disk = id(&volumes[*index]).unwrap_or_default();
        candidate_disk_priority(candidates[&disk], states)
    });
    indices
}

// Row zero always means global browse. Any other row must resolve through
// the *filtered* mapping, never through the original volume array.
fn filtered_disk_index_at_row(indices: &[usize], row: isize) -> Option<usize> {
    usize::try_from(row)
        .ok()
        .filter(|row| *row > 0)
        .and_then(|row| indices.get(row - 1).copied())
}

// Resolve a selected entry on the CURRENT page to an existing catalog disk.
// Never infer an OS mount, a historical drive letter or a file's live presence.
fn reveal_disk_filter_for_row(
    entries: &[Value],
    selected_row: isize,
    volumes: &[Value],
) -> Option<String> {
    let index = usize::try_from(selected_row).ok()?;
    let disk_id = entries
        .get(index)?
        .get("volume_id")?
        .as_i64()
        .filter(|id| *id > 0)?;
    volumes
        .iter()
        .any(|volume| id(volume) == Some(disk_id))
        .then(|| format!("id:{disk_id}"))
}

#[cfg(test)]
mod disk_register_filter_tests {
    use super::{
        audited_register_filter_indices, candidate_register_indices, filtered_disk_index_at_row,
        filtered_disk_indices, register_audit_counts, reveal_disk_filter_for_row,
    };
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn label_and_catalog_id_filter_keep_stable_original_indices() {
        let volumes = vec![
            json!({"id":17,"label":"Медиатека А"}),
            json!({"id":42,"label":"Archive B"}),
            json!({"id":73,"label":"Медиатека C"}),
        ];
        let mut shelves = HashMap::new();
        shelves.insert(42, "Стеллаж Д / наклейка 04".to_owned());
        assert_eq!(filtered_disk_indices(&volumes, &shelves, ""), vec![0, 1, 2]);
        assert_eq!(
            filtered_disk_indices(&volumes, &shelves, "мЕДИАТЕКА"),
            vec![0, 2]
        );
        assert_eq!(filtered_disk_indices(&volumes, &shelves, " 42 "), vec![1]);
        assert!(filtered_disk_indices(&volumes, &shelves, "missing").is_empty());
        assert_eq!(
            filtered_disk_indices(&volumes, &shelves, "нАкЛеЙкА 04"),
            vec![1]
        );
        assert_eq!(
            filtered_disk_indices(&volumes, &shelves, "стеллаж д"),
            vec![1]
        );
        // The shelf tag lives in a user-side map; original catalog rows are untouched.
        assert!(volumes[1].get("shelf_label").is_none());

        let visible = filtered_disk_indices(&volumes, &shelves, "мЕДИАТЕКА");
        assert_eq!(filtered_disk_index_at_row(&visible, 0), None);
        assert_eq!(filtered_disk_index_at_row(&visible, 1), Some(0));
        assert_eq!(filtered_disk_index_at_row(&visible, 2), Some(2));
        assert_eq!(filtered_disk_index_at_row(&visible, 3), None);
        assert_eq!(filtered_disk_index_at_row(&visible, -1), None);
    }

    #[test]
    fn physical_audit_filters_keep_exact_ids_and_fail_closed_on_scan_failure() {
        let volumes = vec![
            json!({"id":4,"label":"Copy of disk 42"}),
            json!({"id":42,"label":"Archive"}),
            json!({"id":142,"label":"Archive"}),
            json!({"id":99,"label":"Other"}),
        ];
        let shelves = HashMap::from([(42, "Полка А".to_owned()), (142, "Полка А".to_owned())]);
        let shared = HashMap::from([(42, vec![42, 142]), (142, vec![42, 142])]);
        let states = HashMap::from([
            (4, "offline".to_owned()),
            (42, "connected".to_owned()),
            (142, "conflict".to_owned()),
        ]);
        let audit = |probe_ok, filter| {
            audited_register_filter_indices(&volumes, &shelves, &shared, &states, probe_ok, filter)
                .unwrap()
        };
        // Never match a label containing 42 to the exact observed disk #42.
        assert_eq!(audit(true, "state:connected"), vec![1]);
        assert_eq!(audit(true, "state:offline"), vec![0]);
        assert_eq!(audit(true, "state:conflict"), vec![2]);
        assert_eq!(audit(true, "state:unverified"), vec![3]);
        assert!(audit(true, "state:unknown").is_empty());
        assert_eq!(audit(true, "tag:missing"), vec![0, 3]);
        assert_eq!(audit(true, "tag:shared"), vec![1, 2]);
        assert!(audit(true, "tag:not-valid").is_empty());
        assert_eq!(
            filtered_disk_index_at_row(&audit(true, "state:connected"), 1),
            Some(1)
        );
        // Failed/incomplete OS probe must clear every connected/offline claim.
        assert!(audit(false, "state:connected").is_empty());
        assert!(audit(false, "state:offline").is_empty());
        assert_eq!(audit(false, "state:unverified"), vec![0, 1, 2, 3]);
        assert_eq!(
            register_audit_counts(&volumes, &shelves, &shared, &states, true),
            [1, 1, 1, 1, 2, 2]
        );
        assert_eq!(
            register_audit_counts(&volumes, &shelves, &shared, &states, false),
            [0, 0, 0, 4, 2, 2]
        );
    }

    #[test]
    fn candidate_only_register_uses_exact_ids_and_preserves_original_row_mapping() {
        let volumes = vec![
            json!({"id":4,"label":"Disk #42 archive"}),
            json!({"id":42,"label":"Archive"}),
            json!({"id":142,"label":"Archive #42 clone label"}),
            json!({"id":9,"label":"Other"}),
        ];
        let groups = vec![
            json!({"volume_id":42,"count":3}),
            json!({"volume_id":142,"count":1}),
            json!({"volume_id":4,"count":0}),
            json!({"volume_id":999,"count":7}),
            json!({"volume_id":42,"count":3}),
        ];
        let visible = candidate_register_indices(&volumes, &groups, &HashMap::new());
        assert_eq!(visible, vec![1, 2]);
        assert_eq!(filtered_disk_index_at_row(&visible, 0), None);
        assert_eq!(filtered_disk_index_at_row(&visible, 1), Some(1));
        assert_eq!(filtered_disk_index_at_row(&visible, 2), Some(2));
        assert_eq!(filtered_disk_index_at_row(&visible, 3), None);
        assert!(candidate_register_indices(&volumes, &[], &HashMap::new()).is_empty());
        assert!(candidate_register_indices(
            &volumes,
            &[json!({"volume_id":-1,"count":1})],
            &HashMap::new(),
        )
        .is_empty());
    }

    #[test]
    fn candidate_register_prioritizes_os_verified_then_counts_without_fuzzy_ids() {
        let volumes = vec![
            json!({"id":4,"label":"Disk 42"}),
            json!({"id":42,"label":"Archive"}),
            json!({"id":142,"label":"Archive"}),
            json!({"id":9,"label":"Other"}),
        ];
        let groups = vec![
            json!({"volume_id":42,"count":30}),
            json!({"volume_id":142,"count":2}),
            json!({"volume_id":9,"count":5}),
            json!({"volume_id":4,"count":0}),
            json!({"volume_id":142,"count":1}),
        ];
        let states = HashMap::from([
            (42, "offline".to_owned()),
            (142, "connected".to_owned()),
            (9, "unverified".to_owned()),
        ]);
        let rows = candidate_register_indices(&volumes, &groups, &states);
        assert_eq!(rows, vec![2, 1, 3]);
        assert_eq!(filtered_disk_index_at_row(&rows, 1), Some(2));
        assert_eq!(filtered_disk_index_at_row(&rows, 2), Some(1));
        assert_eq!(filtered_disk_index_at_row(&rows, 3), Some(3));
        let ambiguous = HashMap::from([(142, "conflict".to_owned())]);
        assert_eq!(
            candidate_register_indices(&volumes, &groups, &ambiguous),
            vec![1, 3, 2]
        );
        assert_eq!(
            candidate_register_indices(&volumes, &groups, &HashMap::new()),
            vec![1, 3, 2]
        );
    }

    #[test]
    fn selected_file_reveals_only_its_exact_existing_catalog_disk() {
        let volumes = vec![
            json!({"id":4,"label":"Disk 42 in description"}),
            json!({"id":42,"label":"Archive"}),
            json!({"id":142,"label":"id:42 misleading label"}),
        ];
        let entries = vec![
            json!({"volume_id":42,"name":"Example.mkv"}),
            json!({"volume_id":99,"name":"Unknown.mkv"}),
            json!({"volume_id":-1,"name":"Invalid.mkv"}),
        ];
        let shelves = HashMap::new();
        assert_eq!(
            reveal_disk_filter_for_row(&entries, 0, &volumes).as_deref(),
            Some("id:42")
        );
        assert_eq!(filtered_disk_indices(&volumes, &shelves, "id:42"), vec![1]);
        assert_eq!(filtered_disk_indices(&volumes, &shelves, "ID:4"), vec![0]);
        assert!(filtered_disk_indices(&volumes, &shelves, "id:").is_empty());
        assert!(filtered_disk_indices(&volumes, &shelves, "id:-1").is_empty());
        assert!(filtered_disk_indices(&volumes, &shelves, "id:999").is_empty());
        for row in [-1, 1, 2, 3] {
            assert_eq!(reveal_disk_filter_for_row(&entries, row, &volumes), None);
        }
    }
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

#[derive(Clone)]
struct MountedPhysicalVolume {
    id: String,
    description: String,
    // Collected from the CURRENT native OS mount-manager snapshot only.
    // No historical catalog root, label or drive letter is used as a path.
    mount_roots: Vec<String>,
}

/// A successful live Windows Mount Manager probe provides a host-scoped
/// volume GUID and serial; no historic drive letter is used as identity.
/// USB HDDs can report DRIVE_FIXED, so show every accessible local volume
/// to the operator and require explicit physical-label confirmation.
// Do not offer simultaneously duplicated OS identities as pairing choices.
// The service receives the unfiltered observations to signal conflicts.
fn unique_pairable_mounts(volumes: &[MountedPhysicalVolume]) -> Vec<MountedPhysicalVolume> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for mount in volumes {
        *counts.entry(mount.id.as_str()).or_insert(0) += 1;
    }
    volumes
        .iter()
        .filter(|mount| counts.get(mount.id.as_str()) == Some(&1))
        .cloned()
        .collect()
}

#[cfg(test)]
mod physical_mount_picker_tests {
    use super::{unique_pairable_mounts, MountedPhysicalVolume};

    #[test]
    fn cloned_and_multi_mounted_identity_is_not_a_pairing_choice() {
        let mounts = vec![
            MountedPhysicalVolume {
                id: "win:{aaaa}:11111111".into(),
                description: "first".into(),
                mount_roots: vec![format!("X:\\\\{description}")],
            },
            MountedPhysicalVolume {
                id: "win:{aaaa}:11111111".into(),
                description: "clone".into(),
                mount_roots: vec![format!("X:\\\\{description}")],
            },
            MountedPhysicalVolume {
                id: "win:{bbbb}:22222222".into(),
                description: "unique".into(),
                mount_roots: vec![format!("X:\\\\{description}")],
            },
        ];
        let offered = unique_pairable_mounts(&mounts);
        assert_eq!(offered.len(), 1);
        assert_eq!(offered[0].description, "unique");
        assert!(unique_pairable_mounts(&mounts[..2]).is_empty());
    }
}

// Copy-location badges are metadata + explicit live-OS pairing states only.
// An OS mount identity is not proof of file bytes or manufacturer serial.
fn candidate_mount_badge(state: Option<&str>) -> &'static str {
    match state {
        Some("connected") => "OS verified now",
        Some("offline") => "paired • offline",
        Some("conflict") => "identity conflict",
        _ => "not verified",
    }
}

// The picker is a physical retrieval aid: catalog ID and owner sticker come
// before its potentially non-unique historical label. OS status is independent
// of the tag and never establishes file-byte or hash identity.
fn candidate_disk_choice_caption(
    group: &Value,
    shelf_labels: &HashMap<i64, String>,
    states: &HashMap<i64, String>,
) -> String {
    let disk_id = group["volume_id"].as_i64().unwrap_or_default();
    let label = string(group, "label").replace(['\r', '\n'], " ");
    let catalog_name = if label.trim().is_empty() {
        "Unnamed disk"
    } else {
        label.trim()
    };
    let sticker = shelf_labels
        .get(&disk_id)
        .filter(|tag| !tag.trim().is_empty())
        .map(|tag| format!("sticker: {tag}"))
        .unwrap_or_else(|| "no sticker tag".to_owned());
    let badge = candidate_mount_badge(states.get(&disk_id).map(String::as_str));
    format!(
        "#{disk_id} • {sticker} • {catalog_name} • {} metadata candidates • {badge}",
        group["count"].as_u64().unwrap_or(0)
    )
}

// Left register is a retrieval shortlist, never a current filesystem view.
// Keep exact ID and count first so they remain visible in a narrow native pane.
fn candidate_register_row_caption(
    volume: &Value,
    group: &Value,
    shelves: &HashMap<i64, String>,
    states: &HashMap<i64, String>,
    shared_sticker: bool,
) -> String {
    let disk = id(volume).unwrap_or_default();
    let count = group["count"].as_u64().unwrap_or(0);
    let badge = candidate_mount_badge(states.get(&disk).map(String::as_str));
    let sticker = shelves
        .get(&disk)
        .filter(|tag| !tag.trim().is_empty())
        .map(|tag| format!("sticker: {}", tag.replace(['\r', '\n', '\t'], " ")))
        .unwrap_or_else(|| "no sticker".to_owned());
    let label = string(volume, "label").replace(['\r', '\n', '\t'], " ");
    let collision = if shared_sticker {
        " • SHARED STICKER"
    } else {
        ""
    };
    format!(
        "#{disk} • {count} name+size metadata candidates • {badge} • {sticker} • {label}{collision}"
    )
}

#[cfg(test)]
mod candidate_register_caption_tests {
    use super::candidate_register_row_caption;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn exact_id_count_and_unverified_state_precede_historical_label() {
        let volume = json!({"id":142,"label":"Film\nShelf 42"});
        let group = json!({"volume_id":142,"count":6});
        let shelves = HashMap::from([(142, "Полка Б".to_owned())]);
        let no_states = HashMap::new();
        let uncertain = candidate_register_row_caption(&volume, &group, &shelves, &no_states, true);
        assert!(uncertain.starts_with("#142 • 6 name+size metadata candidates • not verified"));
        assert!(uncertain.contains("sticker: Полка Б"));
        assert!(uncertain.contains("SHARED STICKER"));
        assert!(!uncertain.contains("\nShelf"));
        let states = HashMap::from([(142, "connected".to_owned())]);
        let observed = candidate_register_row_caption(&volume, &group, &shelves, &states, false);
        assert!(observed.contains("OS verified now"));
        assert!(!observed.contains("SHARED STICKER"));
    }
}

// Different disks may share a manually entered tag; never infer USB identity.
fn shelf_collision_warning(disk: i64, groups: &HashMap<i64, Vec<i64>>) -> Option<String> {
    let others: Vec<_> = groups
        .get(&disk)?
        .iter()
        .copied()
        .filter(|other| *other != disk)
        .map(|other| format!("#{other}"))
        .collect();
    if others.is_empty() {
        return None;
    }
    Some(format!(
        "SHARED STICKER also on {}. Check physical labels; this is NOT USB identity.",
        others.join(", ")
    ))
}

#[cfg(test)]
mod shelf_collision_view_tests {
    use super::*;

    #[test]
    fn shared_tag_warns_without_claiming_os_connection() {
        let groups = HashMap::from([(42, vec![4, 42]), (4, vec![4, 42])]);
        let warning = shelf_collision_warning(42, &groups).unwrap();
        assert!(warning.contains("#4"));
        assert!(warning.contains("SHARED STICKER"));
        assert!(!warning.contains("OS verified"));
        assert!(shelf_collision_warning(17, &groups).is_none());
    }
}

// Order the physical shortlist by last explicit OS pairing state, metadata
// candidate count and exact disk ID. Neither shelf tags nor drive letters
// are USB identity or proof of file contents.
fn candidate_disk_priority(
    group: &Value,
    states: &HashMap<i64, String>,
) -> (u8, std::cmp::Reverse<u64>, i64) {
    let disk = group["volume_id"].as_i64().unwrap_or_default();
    let rank = match states.get(&disk).map(String::as_str) {
        Some("connected") => 0,
        Some("offline") => 1,
        Some("conflict") => 3,
        _ => 2,
    };
    (
        rank,
        std::cmp::Reverse(group["count"].as_u64().unwrap_or(0)),
        disk,
    )
}

fn ordered_candidate_disks<'a>(
    groups: &'a [Value],
    states: &HashMap<i64, String>,
) -> Vec<&'a Value> {
    let mut sorted: Vec<&Value> = groups.iter().collect();
    sorted.sort_by_key(|group| candidate_disk_priority(group, states));
    sorted
}

// Local clipboard-only retrieval list. Status is from the last complete
// independent OS check, not a claim that any candidate bytes still exist.
fn candidate_disk_pickup_list(
    file_name: &str,
    groups: &[Value],
    shelves: &HashMap<i64, String>,
    states: &HashMap<i64, String>,
    collisions: &HashMap<i64, Vec<i64>>,
) -> Option<String> {
    if groups.is_empty() {
        return None;
    }
    let clean = |text: &str| text.replace(['\r', '\n'], " ");
    let total: u64 = groups.iter().filter_map(|g| g["count"].as_u64()).sum();
    let mut lines = vec![
        "WhereIsIt AI — offline USB pickup list".to_owned(),
        format!("File (recorded catalog name): {}", clean(file_name)),
        format!(
            "{total} name+size metadata candidates on {} catalog disks; selected file included.",
            groups.len()
        ),
        "Order: last OS pairing check, then candidate count; exact disk IDs are authoritative."
            .to_owned(),
    ];
    for (index, group) in ordered_candidate_disks(groups, states).iter().enumerate() {
        let warning = group["volume_id"]
            .as_i64()
            .and_then(|disk| shelf_collision_warning(disk, collisions))
            .map(|text| format!(" • {text}"))
            .unwrap_or_default();
        lines.push(format!(
            "{}. {}{warning}",
            index + 1,
            clean(&candidate_disk_choice_caption(group, shelves, states))
        ));
    }
    lines.push(
        "Last OS probe may be stale; recheck the actual physical disk before connecting."
            .to_owned(),
    );
    lines.push(
        "Sticker/shelf is an owner note, NOT USB identity. Recorded paths are historical."
            .to_owned(),
    );
    lines.push(
        "Matching name+size never verifies file bytes or hashes; do NOT auto-delete.".to_owned(),
    );
    Some(lines.join("\r\n"))
}

// Clipboard-only manifest for the CURRENT, already-loaded page. Never claim
// this is the complete set across the catalogue, or that any bytes were read.
// Linked CTF generation is checked by the page request, not by clipboard copy.
fn candidate_page_manifest(
    file_name: &str,
    entries: &[Value],
    offset: usize,
    total: usize,
    focused_disk: Option<i64>,
) -> String {
    let clean = |value: &str| value.replace(['\r', '\n', '\t'], " ");
    let first = if entries.is_empty() { 0 } else { offset + 1 };
    let last = offset.saturating_add(entries.len());
    let scope = focused_disk
        .map(|disk| format!("Catalog disk #{disk}"))
        .unwrap_or_else(|| "All catalogued disks".to_owned());
    let mut lines = vec![
        "WhereIsIt AI — recorded candidate paths, CURRENT PAGE ONLY".to_owned(),
        format!("File (recorded catalog name): {}", clean(file_name)),
        format!("Scope: {scope}; loaded rows {first}–{last} of {total}."),
        "The clipboard contains only this loaded page (maximum 250 records), not all copies."
            .to_owned(),
    ];
    for (index, entry) in entries.iter().take(LIMIT).enumerate() {
        let disk = entry["volume_id"]
            .as_i64()
            .map(|id| format!("#{id}"))
            .unwrap_or_else(|| "unknown".to_owned());
        let size = entry["size"]
            .as_u64()
            .map(|bytes| format!("{bytes} bytes"))
            .unwrap_or_else(|| "size not recorded".to_owned());
        lines.push(format!(
            "{}. Disk {} • {} • {} • {}",
            index + 1,
            disk,
            clean(&string(entry, "name")),
            clean(&string(entry, "relative_path")),
            size
        ));
    }
    lines.push(
        "Paths are HISTORICAL catalog metadata, not current mounted file locations.".to_owned(),
    );
    lines.push("Linked CTF SHA was checked when the page loaded; copying does not recheck source generation.".to_owned());
    lines.push(
        "Matching filename+size does NOT verify file contents or hashes; never auto-delete."
            .to_owned(),
    );
    lines.join("\r\n")
}

#[cfg(test)]
mod candidate_page_manifest_tests {
    use super::candidate_page_manifest;
    use serde_json::json;

    #[test]
    fn page_manifest_uses_current_slice_and_scrubs_untrusted_line_breaks() {
        let rows = vec![
            json!({"id":1,"volume_id":42,"name":"Film\nFake row","relative_path":"/Movies/\r\nScene.mkv","size":1024}),
            json!({"id":2,"volume_id":142,"name":"Film.mkv","relative_path":"/Other/Film.mkv","size":1024}),
        ];
        let text = candidate_page_manifest("Film\nFake header", &rows, 250, 1000, Some(42));
        assert!(text.contains("CURRENT PAGE ONLY"));
        assert!(text.contains("Scope: Catalog disk #42; loaded rows 251–252 of 1000."));
        assert!(text.contains("Film Fake header"));
        assert!(text.contains("1. Disk #42"));
        assert!(text.contains("2. Disk #142"));
        assert!(text.contains("/Movies/  Scene.mkv"));
        assert!(!text.contains("\r\nFake row"));
        assert!(!text.contains("\r\nFake header"));
        assert!(text.contains("not all copies"));
        assert!(text.contains("does not recheck source generation"));
        assert!(text.contains("never auto-delete"));
    }

    #[test]
    fn empty_loaded_page_never_implies_complete_catalog() {
        let text = candidate_page_manifest("Missing", &[], 0, 0, None);
        assert!(text.contains("All catalogued disks; loaded rows 0–0 of 0"));
        assert!(text.contains("CURRENT PAGE ONLY"));
    }
}

// Multiple observed candidate volumes require explicit human selection.
// Repeated/corrupt group IDs are conservatively treated as ambiguous.
fn sole_connected_candidate_disk(groups: &[Value], states: &HashMap<i64, String>) -> Option<i64> {
    let mut found = None;
    for group in groups {
        let Some(volume) = group["volume_id"].as_i64().filter(|v| *v > 0) else {
            continue;
        };
        if group["count"].as_u64().unwrap_or(0) == 0
            || states.get(&volume).map(String::as_str) != Some("connected")
        {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(volume);
    }
    found
}

#[cfg(test)]
mod verified_copy_focus_tests {
    use super::{
        candidate_disk_choice_caption, candidate_mount_badge, sole_connected_candidate_disk,
    };
    use serde_json::json;
    use std::collections::HashMap;
    #[test]
    fn only_one_freshly_os_verified_candidate_can_be_focused() {
        let groups = vec![
            json!({"volume_id":11,"count":2}),
            json!({"volume_id":12,"count":1}),
        ];
        let mut states = HashMap::from([(11, "connected".to_owned()), (12, "offline".to_owned())]);
        assert_eq!(sole_connected_candidate_disk(&groups, &states), Some(11));
        states.insert(12, "connected".into());
        assert_eq!(sole_connected_candidate_disk(&groups, &states), None);
        states.insert(11, "conflict".into());
        assert_eq!(sole_connected_candidate_disk(&groups, &states), Some(12));
        states.clear();
        assert_eq!(sole_connected_candidate_disk(&groups, &states), None);
        assert_eq!(
            sole_connected_candidate_disk(
                &[json!({"volume_id":12,"count":0})],
                &HashMap::from([(12, "connected".to_owned())])
            ),
            None
        );
        assert_eq!(
            sole_connected_candidate_disk(
                &[json!({"volume_id":0,"count":1})],
                &HashMap::from([(0, "connected".to_owned())])
            ),
            None
        );
    }
    #[test]
    fn candidate_picker_keeps_exact_disk_sticker_and_uncertain_os_state() {
        let mut tags = HashMap::new();
        tags.insert(42, "Полка Д / USB-04".to_owned());
        let group = json!({"volume_id":42,"label":"Archive A","count":3});
        let connected = HashMap::from([(42, "connected".to_owned())]);
        let caption = candidate_disk_choice_caption(&group, &tags, &connected);
        assert!(caption.starts_with("#42 • sticker: Полка Д / USB-04 • Archive A"));
        assert!(caption.contains("3 metadata candidates"));
        assert!(caption.ends_with("OS verified now"));

        // The sticker/label never make an unpaired or conflicted drive online.
        let unknown = candidate_disk_choice_caption(&group, &tags, &HashMap::new());
        assert!(unknown.ends_with("not verified"));
        let conflict = candidate_disk_choice_caption(
            &group,
            &tags,
            &HashMap::from([(42, "conflict".to_owned())]),
        );
        assert!(conflict.ends_with("identity conflict"));
        assert!(!unknown.contains("OS verified now"));

        let other = json!({"volume_id":142,"label":"Archive A","count":1});
        let missing = candidate_disk_choice_caption(&other, &tags, &HashMap::new());
        assert!(missing.starts_with("#142 • no sticker tag • Archive A"));
        assert!(!missing.contains("Полка Д"));
    }

    #[test]
    fn pickup_list_prioritizes_confirmed_mounts_and_keeps_conflicts_explicit() {
        let groups = vec![
            json!({"volume_id":142,"label":"Same title","count":5}),
            json!({"volume_id":42,"label":"Same title","count":1}),
            json!({"volume_id":7,"label":"Other","count":3}),
            json!({"volume_id":44,"label":"Conflicted","count":20}),
        ];
        let tags = HashMap::from([(42, "Полка Д".to_owned())]);
        let states = HashMap::from([
            (42, "connected".to_owned()),
            (7, "offline".to_owned()),
            (44, "conflict".to_owned()),
        ]);
        let plan = super::candidate_disk_pickup_list(
            "Movie\r\nspoof.mkv",
            &groups,
            &tags,
            &states,
            &HashMap::new(),
        )
        .unwrap();
        let positions = [42, 7, 142, 44].map(|disk| plan.find(&format!("#{disk} •")).unwrap());
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(plan.contains("#42 • sticker: Полка Д • Same title"));
        assert!(plan.contains("#142 • no sticker tag • Same title"));
        assert!(plan.contains(
            "#44 • no sticker tag • Conflicted • 20 metadata candidates • identity conflict"
        ));
        assert!(plan.contains("Movie  spoof.mkv"));
        assert!(!plan.contains("Movie\r\nspoof.mkv"));
        assert!(plan.contains("do NOT auto-delete"));
        assert!(
            super::candidate_disk_pickup_list("x", &[], &tags, &states, &HashMap::new()).is_none()
        );
    }

    #[test]
    fn unfamiliar_state_fails_closed() {
        assert_eq!(candidate_mount_badge(Some("connected")), "OS verified now");
        assert_eq!(candidate_mount_badge(Some("offline")), "paired • offline");
        assert_eq!(candidate_mount_badge(Some("conflict")), "identity conflict");
        assert_eq!(candidate_mount_badge(None), "not verified");
        assert_eq!(candidate_mount_badge(Some("stale")), "not verified");
    }
}

/// None means that this OS mount snapshot is incomplete. Never translate it
/// into an offline claim; only skip drive types known not to be local volumes.
fn scannable_windows_drive_type(kind: u32) -> Option<bool> {
    match kind {
        DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR => None,
        DRIVE_FIXED | DRIVE_REMOVABLE => Some(true),
        _ => Some(false),
    }
}

#[cfg(test)]
mod physical_probe_completeness_tests {
    use super::*;
    #[test]
    fn unknown_or_disappearing_root_is_not_a_successful_scan() {
        assert_eq!(scannable_windows_drive_type(DRIVE_UNKNOWN), None);
        assert_eq!(scannable_windows_drive_type(DRIVE_NO_ROOT_DIR), None);
        assert_eq!(scannable_windows_drive_type(DRIVE_FIXED), Some(true));
        assert_eq!(scannable_windows_drive_type(DRIVE_REMOVABLE), Some(true));
        assert_eq!(scannable_windows_drive_type(4), Some(false)); // remote
        assert_eq!(scannable_windows_drive_type(5), Some(false)); // optical
    }
}

// A live filesystem volume can have no letter or be mounted ONLY in a folder.
// Keep the native volume GUID and current serial; never match historical paths.
fn windows_volume_guid_identity(path: &str, serial: u32) -> Option<String> {
    let guid = path.strip_prefix(r"\\?\Volume")?.strip_suffix('\\')?;
    if guid.len() != 38 || !guid.starts_with('{') || !guid.ends_with('}') {
        return None;
    }
    let body = &guid[1..guid.len() - 1];
    if body.len() != 36
        || !body.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
    {
        return None;
    }
    Some(format!("win:{guid}:{serial:08x}").to_ascii_lowercase())
}

#[cfg(test)]
mod all_volume_probe_tests {
    use super::windows_volume_guid_identity;

    #[test]
    fn guid_identity_is_stable_without_a_drive_letter() {
        let live = r"\\?\Volume{12345678-1234-abcd-ABCD-1234567890ab}\";
        assert_eq!(
            windows_volume_guid_identity(live, 0x1234_abcd).as_deref(),
            Some("win:{12345678-1234-abcd-abcd-1234567890ab}:1234abcd")
        );
        assert!(windows_volume_guid_identity(r"C:\", 1).is_none());
        assert!(windows_volume_guid_identity(r"\\?\Volume{not-a-uuid}\", 1).is_none());
        assert!(windows_volume_guid_identity(
            r"\\?\Volume{12345678-1234-abcd-abcd-1234567890ab}",
            1
        )
        .is_none());
    }
}

// GetLogicalDriveStringsW ignores folder-only and letterless OS volumes.
// A complete volume-GUID scan prevents a still-connected paired USB from
// being marked offline just because its previous letter disappeared.
unsafe fn windows_mounted_volumes() -> Option<Vec<MountedPhysicalVolume>> {
    let mut name = [0u16; 1024];
    let handle = FindFirstVolumeW(name.as_mut_ptr(), name.len() as u32);
    if handle == INVALID_HANDLE_VALUE {
        return None;
    }
    let probe = (|| {
        let mut result = Vec::new();
        loop {
            let end = name.iter().position(|unit| *unit == 0)?;
            let volume_path = String::from_utf16_lossy(&name[..end]);
            match scannable_windows_drive_type(GetDriveTypeW(name.as_ptr())) {
                Some(false) => {}    // recognized nonlocal/nonfilesystem volume
                None => return None, // incomplete OS probe: cannot assert offline
                Some(true) => {
                    let mut label_buffer = [0u16; 128];
                    let mut serial = 0u32;
                    if GetVolumeInformationW(
                        name.as_ptr(),
                        label_buffer.as_mut_ptr(),
                        label_buffer.len() as u32,
                        &mut serial,
                        null_mut(),
                        null_mut(),
                        null_mut(),
                        0,
                    ) == 0
                    {
                        return None;
                    }
                    let identity = windows_volume_guid_identity(&volume_path, serial)?;
                    let label_end = label_buffer.iter().position(|unit| *unit == 0)?;
                    let label = String::from_utf16_lossy(&label_buffer[..label_end]);
                    // Mount-point names aid identification but NEVER define UUID.
                    let mut paths = vec![0u16; 32_768];
                    let mut needed = 0u32;
                    let mount_found = GetVolumePathNamesForVolumeNameW(
                        name.as_ptr(),
                        paths.as_mut_ptr(),
                        paths.len() as u32,
                        &mut needed,
                    ) != 0;
                    let mounts = if mount_found {
                        paths
                            .split(|unit| *unit == 0)
                            .filter(|units| !units.is_empty())
                            .map(String::from_utf16_lossy)
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    let location = if mounts.is_empty() {
                        "no letter / folder mount reported".to_owned()
                    } else {
                        mounts.join(", ")
                    };
                    result.push(MountedPhysicalVolume {
                        id: identity,
                        mount_roots: mounts,
                        description: format!(
                            "{} • current OS volume {location}",
                            if label.is_empty() {
                                "(unnamed)"
                            } else {
                                &label
                            },
                        ),
                    });
                }
            }
            if result.len() > 256 {
                return None;
            }
            if FindNextVolumeW(handle, name.as_mut_ptr(), name.len() as u32) == 0 {
                return if GetLastError() == ERROR_NO_MORE_FILES {
                    Some(result)
                } else {
                    None
                };
            }
        }
    })();
    if FindVolumeClose(handle) == 0 {
        return None;
    }
    probe
}

// A live OS mount hint is available ONLY after service-side confirmation
// of an explicitly paired volume identity and a unique current OS observation.
// This never constructs a path from a recorded catalog drive letter.
fn verified_mount_root(
    catalog_id: i64,
    states: &HashMap<i64, String>,
    verified_ids: &HashMap<i64, String>,
    observed: &[MountedPhysicalVolume],
) -> Option<String> {
    if states.get(&catalog_id).map(String::as_str) != Some("connected") {
        return None;
    }
    let expected = verified_ids.get(&catalog_id)?;
    let mut matches = observed.iter().filter(|mount| &mount.id == expected);
    let mounted = matches.next()?;
    if matches.next().is_some() {
        return None; // cloned/colliding identities can never select a path
    }
    mounted
        .mount_roots
        .iter()
        .find(|path| !path.is_empty() && !path.contains('\0'))
        .cloned()
}

#[cfg(test)]
mod verified_mount_root_tests {
    use super::{verified_mount_root, MountedPhysicalVolume};
    use std::collections::HashMap;

    #[test]
    fn only_explicit_unique_os_verified_identity_exposes_current_mount_root() {
        let mounts = vec![MountedPhysicalVolume {
            id: "win:{physical}:12345678".into(),
            description: "current OS volume".into(),
            mount_roots: vec![r"R:\\".into()],
        }];
        let mut states = HashMap::from([(42, "connected".into())]);
        let verified = HashMap::from([(42, "win:{physical}:12345678".into())]);
        assert_eq!(verified_mount_root(42, &states, &verified, &mounts), Some(r"R:\\".into()));
        assert_eq!(verified_mount_root(142, &states, &verified, &mounts), None);
        assert_eq!(verified_mount_root(42, &states, &HashMap::new(), &mounts), None);
        states.insert(42, "offline".into());
        assert_eq!(verified_mount_root(42, &states, &verified, &mounts), None);
        states.insert(42, "conflict".into());
        assert_eq!(verified_mount_root(42, &states, &verified, &mounts), None);
        states.insert(42, "connected".into());
        let duplicates = [mounts.clone(), mounts.clone()].concat();
        assert_eq!(verified_mount_root(42, &states, &verified, &duplicates), None);
        let without_mount = vec![MountedPhysicalVolume {
            mount_roots: vec![],
            ..mounts[0].clone()
        }];
        assert_eq!(verified_mount_root(42, &states, &verified, &without_mount), None);
    }
}

fn physical_state_label(state: &str) -> &'static str {
    match state {
        "connected" => "Physical connection: CONNECTED — live OS identity matched",
        "offline" => "Physical connection: OFFLINE — bound OS identity absent",
        "conflict" => "Physical connection: CONFLICT — identity ambiguous",
        _ => "Physical connection: NOT VERIFIED",
    }
}

// A catalog entry is not proof that the original USB disk is connected.
// This pure formatter is deliberately shared by the selection change path
// and unit tests; it never accesses the filesystem or exposes private data.

// Preserve readable columns while displaying the USB disk locator at standard
// laptop resolutions. The full recorded path remains in the inspector when its
// redundant table column is collapsed.
fn explorer_geometry(window_width: i32) -> (i32, i32, i32) {
    explorer_geometry_for_width(window_width, 282, true)
}

// A real Win32 three-pane splitter; the disk register stays 200px wide.
// Retain at least 600px for results, and collapse the inspector below 1120px.
fn explorer_geometry_for_width(
    window_width: i32,
    requested_locator: i32,
    visible: bool,
) -> (i32, i32, i32) {
    let maximum = (window_width - 222 - 8 - 600).min(420);
    let locator_width = if visible && window_width >= 1120 && maximum >= 220 {
        requested_locator.clamp(220, maximum)
    } else {
        0
    };
    let file_width = (window_width
        - 222
        - if locator_width > 0 {
            locator_width + 8
        } else {
            0
        })
    .max(100);
    let path_column_width = if locator_width == 0 || file_width >= 840 {
        185
    } else {
        0
    };
    (locator_width, file_width, path_column_width)
}

// Keep the historical path visible when the locator cannot fit, even at
// 880px, instead of leaving an unscrollable fixed-width results table.
fn result_column_widths(file_width: i32, locator_visible: bool) -> [i32; 7] {
    if locator_visible && file_width >= 820 {
        [
            220,
            112,
            68,
            83,
            110,
            65,
            if file_width >= 840 { 185 } else { 0 },
        ]
    } else if locator_visible {
        [210, 95, 64, 75, 95, 61, 0]
    } else if file_width >= 900 {
        [220, 118, 70, 87, 115, 65, 185]
    } else if file_width >= 815 {
        [205, 100, 65, 80, 100, 65, file_width - 615]
    } else {
        [170, 86, 56, 74, 86, 60, (file_width - 532).max(104)]
    }
}

fn locator_splitter_hit_test(
    window_width: i32,
    window_height: i32,
    requested_locator: i32,
    visible: bool,
    x: i32,
    y: i32,
) -> bool {
    let (locator_width, _, _) =
        explorer_geometry_for_width(window_width, requested_locator, visible);
    locator_width > 0
        && x >= window_width - locator_width - 8
        && x <= window_width - locator_width
        && y >= 106
        && y < window_height - 146
}

#[cfg(test)]
mod locator_splitter_tests {
    use super::{explorer_geometry_for_width, locator_splitter_hit_test, result_column_widths};

    #[test]
    fn pane_resizing_respects_minimum_result_space_and_can_hide() {
        assert_eq!(explorer_geometry_for_width(1180, 500, true), (350, 600, 0));
        assert_eq!(
            explorer_geometry_for_width(1366, 220, true),
            (220, 916, 185)
        );
        assert_eq!(
            explorer_geometry_for_width(1366, 282, false),
            (0, 1144, 185)
        );
        assert_eq!(explorer_geometry_for_width(880, 282, true), (0, 658, 185));
        assert_eq!(explorer_geometry_for_width(1120, 282, true), (282, 608, 0));
        assert_eq!(explorer_geometry_for_width(1119, 282, true), (0, 897, 185));
    }

    #[test]
    fn splitter_only_captures_its_actual_visible_gap() {
        assert!(locator_splitter_hit_test(1180, 740, 282, true, 894, 300));
        assert!(!locator_splitter_hit_test(1180, 740, 282, true, 800, 300));
        assert!(!locator_splitter_hit_test(1180, 740, 282, false, 894, 300));
        assert!(!locator_splitter_hit_test(1100, 740, 282, true, 814, 300));
    }

    #[test]
    fn narrow_result_columns_keep_recorded_path_legible() {
        let narrow = result_column_widths(658, false);
        assert_eq!(narrow, [170, 86, 56, 74, 86, 60, 126]);
        assert_eq!(narrow.iter().sum::<i32>(), 658);
        assert_eq!(result_column_widths(600, true)[6], 0);
        assert_eq!(result_column_widths(854, true)[6], 185);
        assert_eq!(result_column_widths(815, false).iter().sum::<i32>(), 815);
        assert_eq!(result_column_widths(608, true)[6], 0);
    }
}

#[cfg(test)]
mod explorer_geometry_tests {
    use super::explorer_geometry;

    #[test]
    fn two_column_window_keeps_browsing_path_in_table() {
        assert_eq!(explorer_geometry(1100), (0, 878, 185));
    }

    #[test]
    fn standard_laptop_window_prioritizes_name_disk_and_locator() {
        assert_eq!(explorer_geometry(1180), (282, 668, 0));
    }

    #[test]
    fn window_1366px_shows_selected_disk_and_recorded_path() {
        assert_eq!(explorer_geometry(1366), (282, 854, 185));
    }
}

fn disk_locator_text(
    entry: Option<&Value>,
    disk_label: &str,
    linked_ctf: bool,
    physical_state: &str,
) -> String {
    let Some(entry) = entry else {
        return "DISK LOCATOR\r\n\r\nSelect a file in the result list to locate its catalogued USB disk.\r\n\r\nSearch spans all catalogued disks when \"All disks\" is selected.".into();
    };
    let disk_id = entry["volume_id"].as_i64().unwrap_or(0);
    let file_name = string(entry, "name");
    let recorded_path = string(entry, "relative_path");
    let size = display_size(entry["size"].as_u64());
    let kind = string(entry, "kind");
    let description = string(entry, "description");
    let description_details = if description.is_empty() {
        String::new()
    } else {
        format!("\r\nDESCRIPTION\r\n{description}\r\n")
    };
    let candidates = entry["duplicate_count"]
        .as_u64()
        .map(|n| format!("{n} metadata candidates (not content-verified)"))
        .unwrap_or_else(|| "Not calculated".into());
    let mode = if linked_ctf {
        "Linked CTF • source read-only; SHA generation pinned"
    } else {
        "SQLite catalog snapshot"
    };
    let presence = physical_state_label(physical_state);
    format!(
        "DISK TO LOCATE\r\n\r\n{disk_label}\r\nCatalog disk ID: {disk_id}\r\n\r\n{presence}\r\nCurrent mount state depends on explicit OS-volume pairing; historical drive letters are not evidence.\r\n\r\nFILE\r\n{file_name}\r\n\r\nRECORDED LOCATION\r\n{recorded_path}\r\n\r\nSize: {size}\r\nKind: {kind}\r\nCopies: {candidates}\r\nSource: {mode}\r\n{description_details}\r\nSelect text and press Ctrl+C to copy. Ctrl+D finds catalogued name+size copies. Ctrl+J reveals its exact catalog disk in the left register without changing search scope."
    )
}

#[cfg(test)]
mod disk_locator_tests {
    use super::disk_locator_text;
    use serde_json::json;

    #[test]
    fn locator_uses_exact_catalog_label_without_claiming_usb_presence() {
        let row = json!({"name":"Example.mkv","relative_path":"Films/Example.mkv",
            "volume_id":42,"size":1024,"duplicate_count":3});
        let text = disk_locator_text(Some(&row), "Shelf B • USB-42", true, "unverified");
        assert!(text.contains("Shelf B • USB-42"));
        assert!(text.contains("Catalog disk ID: 42"));
        assert!(text.contains("Films/Example.mkv"));
        assert!(text.contains("3 metadata candidates (not content-verified)"));
        assert!(text.contains("NOT VERIFIED"));
        assert!(text.contains("source read-only"));
        assert!(!text.contains("Connected"));
    }

    #[test]
    fn only_verified_os_match_can_use_connected_wording() {
        assert!(super::physical_state_label("connected").contains("CONNECTED"));
        assert!(!super::physical_state_label("unverified").contains("CONNECTED"));
        assert!(!super::physical_state_label("conflict").contains("CONNECTED"));
    }

    #[test]
    fn locator_placeholder_and_unknown_duplicates_are_honest() {
        assert!(disk_locator_text(None, "", false, "unverified").contains("Select a file"));
        let row = json!({"name":"Folder","relative_path":"Videos/","volume_id":1});
        let text = disk_locator_text(Some(&row), "Backup disk", false, "unverified");
        assert!(text.contains("Not calculated"));
        assert!(text.contains("SQLite catalog snapshot"));
        assert!(text.contains("NOT VERIFIED"));
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
fn verified_overlay_response_ok(data: &Value, expected_offset: usize) -> bool {
    let view = &data["result"];
    match view["state"].as_str() {
        Some("partial_activation" | "activation_conflict") => {
            view["offset"].as_u64() == Some(expected_offset as u64)
                && view["limit"].as_u64() == Some(ANNOTATION_PAGE_SIZE as u64)
                && view["read_only_source"] == true
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
    if !verified_overlay_response_ok(data, offset) {
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
    if !verified_overlay_response_ok(data, offset) {
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
        annotation_reply_current, annotation_selection_request, verified_overlay_page_controls,
        verified_overlay_response_ok, verified_overlay_summary, AnnotationOverlayAction,
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
            "offset":0,"limit":20,
            "read_only_source":true,"unresolved_unconfirmed_annotations_not_projected":true,
            "items":[{"entry_id":9,"old_entry_id":3,"revision":1,"read_only":true,
                "activation":"explicit_reviewed_projection",
                "note":"synthetic private note","tags":"synthetic"}]}});
        assert!(verified_overlay_response_ok(&original, 0));
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
            assert!(!verified_overlay_response_ok(&invalid, 0));
            assert!(!verified_overlay_summary(&invalid, 0).contains("synthetic private note"));
        }
        let mut invalid = original.clone();
        invalid["result"]["items"][0]["activation"] = json!("unverified_audit");
        assert!(!verified_overlay_response_ok(&invalid, 0));
        assert!(!verified_overlay_summary(&invalid, 0).contains("synthetic private note"));
        let mut wrong_page = original.clone();
        wrong_page["result"]["offset"] = json!(20);
        assert!(!verified_overlay_response_ok(&wrong_page, 0));
        assert!(!verified_overlay_summary(&wrong_page, 0).contains("synthetic private note"));
        wrong_page["result"]["offset"] = json!(0);
        wrong_page["result"]["limit"] = json!(250);
        assert!(!verified_overlay_response_ok(&wrong_page, 0));
    }

    #[test]
    fn closed_or_switched_annotation_session_drops_late_responses() {
        for tag in [
            "annotation_preview",
            "annotation_confirm",
            "annotation_overlay",
            "overlay_verified",
        ] {
            assert!(annotation_reply_current(tag, 7, 7));
            assert!(!annotation_reply_current(tag, 7, 8));
        }
        assert!(annotation_reply_current("rows", 7, 8));
        assert!(annotation_reply_current("linked_sync", 7, 8));
    }

    #[test]
    fn verified_view_pages_are_bounded_and_empty_is_read_only() {
        let item = json!({"entry_id":9,"old_entry_id":3,"revision":1,"read_only":true,
            "activation":"explicit_reviewed_projection","note":"synthetic","tags":""});
        let mut page = json!({"result":{"state":"partial_activation","total":21,
            "offset":0,"limit":20,"conflicts":0,"read_only_source":true,
            "unresolved_unconfirmed_annotations_not_projected":true,
            "items":vec![item.clone(); 20]}});
        assert_eq!(verified_overlay_page_controls(&page, 0), (false, true));
        page["result"]["items"] = json!([item]);
        page["result"]["offset"] = json!(20);
        assert_eq!(verified_overlay_page_controls(&page, 20), (true, false));
        assert!(verified_overlay_summary(&page, 20).contains("Rows 21–21 of 21"));
        let empty = json!({"result":{"state":"no_cross_generation_activations",
            "items":[],"total":0,"conflicts":0}});
        assert!(verified_overlay_response_ok(&empty, 0));
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

// Late replies from a closed review or another selected user sidecar must not
// populate the current review or trigger a CAS action on the wrong sidecar.
fn annotation_reply_current(tag: &str, request_session: u64, active_session: u64) -> bool {
    !matches!(
        tag,
        "annotation_preview" | "annotation_confirm" | "annotation_overlay" | "overlay_verified"
    ) || request_session == active_session
}

struct Reply {
    database: String,
    tag: &'static str,
    generation: u64,
    annotation_session: u64,
    result: Value,
}
struct CopyReturn {
    volume: Option<i64>,
    stack: Vec<(Option<i64>, String)>,
    query: String,
    duplicates: bool,
    offset: usize,
}
struct App {
    hwnd: HWND,
    disks: HWND,
    disk_filter: HWND,
    shelf_edit: HWND,
    shelf_edit_disk: Option<i64>,
    shelf_labels: HashMap<i64, String>,
    shelf_tag_conflicts: HashMap<i64, Vec<i64>>,
    disk_filter_query: String,
    visible_disk_indices: Vec<usize>,
    files: HWND,
    search: HWND,
    status: HWND,
    location: HWND,
    locator: HWND,
    locator_requested_width: i32,
    locator_visible: bool,
    dragging_locator: bool,
    page_sort_column: Option<usize>,
    page_sort_descending: bool,
    buttons: Vec<(usize, HWND)>,
    physical_states: HashMap<i64, String>,
    verified_physical_ids: HashMap<i64, String>,
    mounted_volumes: Vec<MountedPhysicalVolume>,
    physical_probe_ok: bool,
    duplicate_mode: HWND,
    help: HWND,
    copy_disk_filter: HWND,
    export_format: HWND,
    category_list: HWND,
    category_name: HWND,
    categories: Vec<Value>,
    duplicates: bool,
    copy_source: Option<i64>,
    copy_source_name: Option<String>,
    copy_focus_volume_id: Option<i64>,
    copy_disk_groups: Vec<Value>,
    copy_return: Option<CopyReturn>,
    applied_duplicate_mode: &'static str,
    db: String,
    linked_ctf: Option<String>,
    linked_generation: Option<String>,
    pending_link_path: Option<String>,
    annotation_db: Option<String>,
    annotation_session: u64,
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
    page_ready: bool,
    page_awaiting: bool,
    page_retry_available: bool,
    pending_mount_root: Option<(i64, u64)>,
    generation: u64,
    pending: usize,
    job: Option<String>,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
}
impl App {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let (locator_visible, locator_width) = read_ui_layout_preferences();
        Self {
            hwnd: null_mut(),
            disks: null_mut(),
            disk_filter: null_mut(),
            shelf_edit: null_mut(),
            shelf_edit_disk: None,
            shelf_labels: HashMap::new(),
            shelf_tag_conflicts: HashMap::new(),
            disk_filter_query: String::new(),
            visible_disk_indices: Vec::new(),
            files: null_mut(),
            search: null_mut(),
            status: null_mut(),
            location: null_mut(),
            locator: null_mut(),
            locator_requested_width: locator_width,
            locator_visible,
            dragging_locator: false,
            page_sort_column: None,
            page_sort_descending: false,
            buttons: vec![],
            physical_states: HashMap::new(),
            verified_physical_ids: HashMap::new(),
            mounted_volumes: Vec::new(),
            physical_probe_ok: false,
            duplicate_mode: null_mut(),
            help: null_mut(),
            copy_disk_filter: null_mut(),
            export_format: null_mut(),
            category_list: null_mut(),
            category_name: null_mut(),
            categories: vec![],
            duplicates: false,
            copy_source: None,
            copy_source_name: None,
            copy_focus_volume_id: None,
            copy_disk_groups: vec![],
            copy_return: None,
            applied_duplicate_mode: "name_size",
            db: std::env::args().nth(1).unwrap_or_else(default_db),
            linked_ctf: None,
            linked_generation: None,
            pending_link_path: None,
            annotation_db: None,
            annotation_session: 0,
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
            page_ready: false,
            page_awaiting: false,
            page_retry_available: false,
            pending_mount_root: None,
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
    unsafe fn search_edit_text(&self) -> String {
        let len = GetWindowTextLengthW(self.search);
        let mut buffer = vec![0u16; len as usize + 1];
        GetWindowTextW(self.search, buffer.as_mut_ptr(), len + 1);
        String::from_utf16_lossy(&buffer[..len as usize])
    }
    unsafe fn update_video_filter_caption(&self) {
        if let Some((_, button)) = self.buttons.iter().find(|(id, _)| *id == VIDEO_FILTER) {
            self.text(
                *button,
                if video_filter_active(&self.search_edit_text()) {
                    "All &formats"
                } else {
                    "&Video files"
                },
            );
        }
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
        let annotation_session = self.annotation_session;
        if tag == "rows" {
            self.page_awaiting = true;
            self.page_retry_available = false;
        }
        self.pending += 1;
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| atlas_service::dispatch_json(&request.to_string()))
                .ok().and_then(|s|serde_json::from_str(&s).ok())
                .unwrap_or_else(||json!({"ok":false,"error":{"message":"Core operation failed or returned invalid JSON"}}));
            let _ = tx.send(Reply {
                database,
                tag,
                generation,
                annotation_session,
                result,
            });
        });
    }
    unsafe fn init(&mut self) {
        self.linked_ctf = None;
        self.linked_generation = None;
        self.pending_link_path = None;
        self.annotation_db = None;
        self.annotation_session = self.annotation_session.wrapping_add(1);
        self.annotation_offset = 0;
        self.verified_overlay_offset = 0;
        let _ = std::fs::create_dir_all(config_dir());
        self.generation += 1;
        self.volumes.clear();
        self.disk_filter_query.clear();
        self.shelf_labels.clear();
        self.shelf_tag_conflicts.clear();
        self.shelf_edit_disk = None;
        self.text(self.shelf_edit, "");
        self.visible_disk_indices.clear();
        self.text(self.disk_filter, "");
        self.physical_states.clear();
        self.verified_physical_ids.clear();
        self.mounted_volumes.clear();
        self.physical_probe_ok = false;
        self.entries.clear();
        self.page_ready = false;
        self.page_awaiting = false;
        self.page_retry_available = false;
        self.pending_mount_root = None;
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
        self.copy_source = None;
        self.copy_source_name = None;
        self.copy_return = None;
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
        // The candidate-only register is a temporary view, not a persistent
        // search scope: restore the full register upon leaving copy mode.
        if self.copy_source.is_none()
            && self
                .disk_filter_query
                .trim()
                .eq_ignore_ascii_case("copies:")
        {
            self.disk_filter_query.clear();
            self.text(self.disk_filter, "");
            self.populate_physical_disk_list();
        }
        self.generation += 1;
        self.page_awaiting = false;
        self.page_retry_available = false;
        // Hide all rows from the preceding scope/page before the async reply.
        // Never permit an old selection to masquerade as the new catalog page.
        self.page_ready = false;
        self.entries.clear();
        self.total = 0;
        self.total_known = false;
        self.has_more = false;
        SendMessageW(self.files, LVM_DELETEALLITEMS, 0, 0);
        self.update_disk_locator();
        let (copy_visibility, help_visibility) = if self.copy_source.is_some() {
            (SW_SHOW, SW_HIDE)
        } else {
            (SW_HIDE, SW_SHOW)
        };
        ShowWindow(self.copy_disk_filter, copy_visibility);
        ShowWindow(self.help, help_visibility);
        self.status(if self.copy_source.is_some() {
            "Comparing exact name + size metadata across all catalogued disks…"
        } else if self.duplicates {
            "Finding metadata duplicate candidates…"
        } else {
            "Loading…"
        });
        self.text(
            self.location,
            &format!(
                "{}  —  {}",
                self.linked_ctf.as_deref().unwrap_or(&self.db),
                if self.copy_source.is_some() {
                    "OTHER catalog disks: name+size metadata candidates (not verified)"
                } else if self.query.is_empty() {
                    self.stack.last().map(|p| p.1.as_str()).unwrap_or("/")
                } else if self.volume.is_some() {
                    "Search within selected catalog disk"
                } else if self.query.starts_with("size:") && self.query.ends_with(" AND kind:file")
                {
                    "GLOBAL exact byte-size file candidates (metadata only; contents unverified)"
                } else {
                    "Global search across all catalogued disks"
                }
            ),
        );
        if let Some(source) = self.copy_source {
            if let Some(mut linked) = self.linked_request("ctf_copy_candidates_pinned") {
                linked["entry_id"] = json!(source);
                linked["candidate_volume_id"] = json!(self.copy_focus_volume_id);
                linked["other_disks_only"] = json!(true);
                linked["offset"] = json!(self.offset);
                linked["limit"] = json!(LIMIT);
                self.call("rows", linked);
            } else {
                self.call(
                    "rows",
                    json!({"op":"copy_candidates","entry_id":source,
                    "candidate_volume_id":self.copy_focus_volume_id,
                    "other_disks_only":true,
                    "offset":self.offset,"limit":LIMIT}),
                );
            }
            return;
        }
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
    unsafe fn stop_waiting_for_page(&mut self) {
        if !self.page_awaiting {
            self.status("There is no pending catalog page to dismiss.");
            return;
        }
        // This is a UI stop, NOT a core/CTF I/O interrupt. A worker may
        // finish, but its generation cannot populate or authorize old rows.
        self.generation = self.generation.wrapping_add(1);
        self.page_awaiting = false;
        self.page_retry_available = true;
        self.page_ready = false;
        self.entries.clear();
        self.copy_disk_groups.clear();
        self.total = 0;
        self.total_known = false;
        self.has_more = false;
        SendMessageW(self.files, LVM_DELETEALLITEMS, 0, 0);
        self.populate_candidate_disk_picker();
        if self
            .disk_filter_query
            .trim()
            .eq_ignore_ascii_case("copies:")
        {
            self.populate_physical_disk_list();
        }
        self.update_disk_locator();
        self.status("Stopped waiting: old page discarded; source read may continue. Retry page rechecks pinned CTF SHA.");
    }

    unsafe fn poll(&mut self) {
        while let Ok(reply) = self.rx.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            if reply.database != self.db {
                continue;
            }
            if reply.tag == "rows"
                && !page_response_current(reply.generation, self.generation, self.page_awaiting)
            {
                continue;
            }
            let data = &reply.result["data"];
            if reply.tag == "mount_root_source" {
                let pending = self.pending_mount_root.take();
                if let Some((disk, generation)) = pending {
                    if generation == reply.generation
                        && generation == self.generation
                        && self.selected_physical_catalog_disk() == Some(disk)
                    {
                        if reply.result["ok"] == true {
                            self.copy_verified_os_mount_root(disk);
                        } else {
                            self.status("Linked CTF generation could not be verified; OS root NOT copied. Refresh source.");
                        }
                    }
                }
                continue;
            }
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
            if !annotation_reply_current(
                reply.tag,
                reply.annotation_session,
                self.annotation_session,
            ) {
                continue;
            }
            if reply.tag == "rows" {
                self.page_awaiting = false;
                self.page_retry_available = reply.result["ok"] != true;
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
                            self.annotation_session = self.annotation_session.wrapping_add(1);
                            self.annotation_db = None;
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
                            self.annotation_session = self.annotation_session.wrapping_add(1);
                            self.annotation_db = None;
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
                        self.physical_states.clear();
                        self.shelf_labels.clear();
                        self.shelf_tag_conflicts.clear();
                        self.shelf_edit_disk = None;
                        self.text(self.shelf_edit, "");
                        self.annotation_db = None;
                        self.annotation_session = self.annotation_session.wrapping_add(1);
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
                    self.refresh_physical_presence();
                    self.refresh_shelf_labels();
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
                    self.page_ready = true;
                    self.copy_disk_groups = if self.copy_source.is_some() {
                        page["disk_groups"].as_array().cloned().unwrap_or_default()
                    } else {
                        vec![]
                    };
                    // A new pinned page may carry a different disk-group set.
                    // Keep the candidate-only register synchronized with the
                    // current authoritative page, never the previous selection.
                    if self
                        .disk_filter_query
                        .trim()
                        .eq_ignore_ascii_case("copies:")
                    {
                        self.populate_physical_disk_list();
                    }
                    self.populate_candidate_disk_picker();
                    self.sort_loaded_page();
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
                if *command == STOP_PAGE_WAIT {
                    self.page_awaiting as i32
                } else if *command == RETRY_PAGE {
                    self.page_retry_available as i32
                } else if *command == CANCEL {
                    self.job.as_ref().is_some_and(|j| j.starts_with("win-")) as i32
                } else if *command == ANNOTATION_PREVIEW {
                    (self.linked_mode() && self.job.is_none() && self.pending == 0) as i32
                } else if [OPEN, NEW, SCAN, IMPORT, LINK_CTF, EXPORT].contains(command) {
                    (self.job.is_none()
                        && self.pending == 0
                        && (!self.linked_mode()
                            || [OPEN, NEW, LINK_CTF].contains(command)
                            || (*command == EXPORT
                                && linked_export_allowed(SendMessageW(
                                    self.export_format,
                                    CB_GETCURSEL,
                                    0,
                                    0,
                                ))))) as i32
                } else if [COPY_PICKUP_PLAN, COPY_PAGE_MANIFEST, COPY_CONNECTED].contains(command) {
                    (self.copy_source.is_some()
                        && self.page_ready
                        && !self.copy_disk_groups.is_empty()) as i32
                } else if *command == SHELF_SAVE {
                    self.shelf_edit_disk.is_some() as i32
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

    fn physical_request(
        &self,
        op: &str,
        volume: Option<i64>,
        identity: Option<&str>,
        shelf_label: Option<&str>,
    ) -> Value {
        let key = if let Some(generation) = self.linked_generation.as_deref() {
            format!("ctf:{generation}")
        } else {
            format!("sqlite:{}", self.db)
        };
        let request = json!({
            "op":op,
            "user_store":config_dir().join("physical-volume-bindings.sqlite").to_string_lossy(),
            "catalog_key":key,
            "platform":"windows",
            "volume_ids":self.volumes.iter().filter_map(id).collect::<Vec<_>>(),
            "observed_ids":self.mounted_volumes.iter().map(|v| &v.id).collect::<Vec<_>>(),
            "probe_ok":self.physical_probe_ok,
            "volume_id":volume,
            "physical_id":identity,
            "shelf_label":shelf_label,
            "confirmed":true
        });
        serde_json::from_str(&atlas_service::dispatch_json(&request.to_string()))
            .unwrap_or_else(|_| json!({"ok":false}))
    }

    // A user sticker tag is an offline retrieval hint, never proof of USB presence.
    unsafe fn shelf_edit_target(&self) -> Option<i64> {
        self.volume
            .or_else(|| self.selected_physical_catalog_disk())
    }

    unsafe fn sync_shelf_editor(&mut self) {
        let target = self.shelf_edit_target();
        if self.shelf_edit_disk != target {
            self.shelf_edit_disk = target;
            let label = target
                .and_then(|disk| self.shelf_labels.get(&disk))
                .cloned()
                .unwrap_or_default();
            self.text(self.shelf_edit, &label);
        }
        EnableWindow(self.shelf_edit, target.is_some() as i32);
        if let Some((_, save)) = self.buttons.iter().find(|(id, _)| *id == SHELF_SAVE) {
            EnableWindow(*save, target.is_some() as i32);
        }
    }

    unsafe fn refresh_shelf_labels(&mut self) {
        let reply = self.physical_request("physical_shelf_labels", None, None, None);
        self.shelf_labels.clear();
        self.shelf_tag_conflicts.clear();
        if reply["ok"] == true {
            if let Some(items) = reply["data"]["items"].as_array() {
                for item in items {
                    if let (Some(disk), Some(label)) =
                        (item["volume_id"].as_i64(), item["shelf_label"].as_str())
                    {
                        self.shelf_labels.insert(disk, label.to_owned());
                    }
                }
            }
            if let Some(collisions) = reply["data"]["collisions"].as_array() {
                for group in collisions {
                    let members: Vec<i64> = group["volume_ids"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_i64)
                        .collect();
                    if members.len() > 1 {
                        for disk in &members {
                            self.shelf_tag_conflicts.insert(*disk, members.clone());
                        }
                    }
                }
            }
        }
        self.shelf_edit_disk = None;
        self.populate_physical_disk_list();
        self.populate_candidate_disk_picker();
        self.sync_shelf_editor();
        self.update_disk_locator();
    }

    unsafe fn save_shelf_label(&mut self) {
        let Some(target) = self.shelf_edit_disk else {
            self.status("Select a catalog disk or file before saving a shelf tag.");
            return;
        };
        if self.shelf_edit_target() != Some(target) {
            self.sync_shelf_editor();
            self.status("Disk selection changed; sticker tag was not saved.");
            return;
        }
        let len = GetWindowTextLengthW(self.shelf_edit);
        let mut buffer = vec![0u16; len as usize + 1];
        GetWindowTextW(self.shelf_edit, buffer.as_mut_ptr(), len + 1);
        let label = String::from_utf16_lossy(&buffer[..len as usize]);
        let reply =
            self.physical_request("physical_shelf_label_set", Some(target), None, Some(&label));
        if reply["ok"] == true && reply["data"]["saved"] == true {
            self.refresh_shelf_labels();
            if shelf_collision_warning(target, &self.shelf_tag_conflicts).is_some() {
                self.status("Tag saved, but SHARED STICKER also labels another disk; check physical hardware.");
            } else {
                self.status("Private sticker/shelf tag saved; not proof of USB presence.");
            }
        } else {
            self.status("Sticker/shelf tag not saved (one line, max 80 characters).");
        }
    }

    unsafe fn populate_physical_disk_list(&mut self) {
        let candidate_mode = self
            .disk_filter_query
            .trim()
            .eq_ignore_ascii_case("copies:");
        self.visible_disk_indices = if candidate_mode {
            if self.copy_source.is_some() {
                candidate_register_indices(
                    &self.volumes,
                    &self.copy_disk_groups,
                    &self.physical_states,
                )
            } else {
                vec![]
            }
        } else if let Some(indices) = audited_register_filter_indices(
            &self.volumes,
            &self.shelf_labels,
            &self.shelf_tag_conflicts,
            &self.physical_states,
            self.physical_probe_ok,
            &self.disk_filter_query,
        ) {
            indices
        } else {
            filtered_disk_indices(&self.volumes, &self.shelf_labels, &self.disk_filter_query)
        };
        SendMessageW(self.disks, LB_RESETCONTENT, 0, 0);
        let audit = register_audit_counts(
            &self.volumes,
            &self.shelf_labels,
            &self.shelf_tag_conflicts,
            &self.physical_states,
            self.physical_probe_ok,
        );
        let global_label = format!(
            "All disks ({}/{}) | OS +{} -{} !{} ?{} | stickers missing {} shared {}",
            self.visible_disk_indices.len(),
            self.volumes.len(),
            audit[0],
            audit[1],
            audit[2],
            audit[3],
            audit[4],
            audit[5],
        );
        SendMessageW(
            self.disks,
            LB_ADDSTRING,
            0,
            wide(&global_label).as_ptr() as isize,
        );
        for &volume_index in &self.visible_disk_indices {
            let disk = &self.volumes[volume_index];
            let badge = match self
                .physical_states
                .get(&id(disk).unwrap_or_default())
                .map(String::as_str)
            {
                Some("connected") => "✓ OS verified",
                Some("offline") => "○ offline",
                Some("conflict") => "! conflict",
                _ => "? not verified",
            };
            let shelf = id(disk)
                .and_then(|disk| self.shelf_labels.get(&disk))
                .map(|label| format!(" • shelf: {label}"))
                .unwrap_or_default();
            let warning = id(disk)
                .and_then(|disk_id| shelf_collision_warning(disk_id, &self.shelf_tag_conflicts))
                .map(|_| " ! SHARED STICKER")
                .unwrap_or("");
            let label = if candidate_mode {
                let disk_id = id(disk).unwrap_or_default();
                let group = self
                    .copy_disk_groups
                    .iter()
                    .filter(|group| group["volume_id"].as_i64() == Some(disk_id))
                    .max_by_key(|group| group["count"].as_u64().unwrap_or(0));
                if let Some(group) = group {
                    candidate_register_row_caption(
                        disk,
                        group,
                        &self.shelf_labels,
                        &self.physical_states,
                        warning != "",
                    )
                } else {
                    format!("#{disk_id} • candidate data unavailable")
                }
            } else {
                format!(
                    "{}{shelf}  ({} items)  {badge}{warning}",
                    string(disk, "label"),
                    disk["entry_count"].as_u64().unwrap_or(0)
                )
            };
            SendMessageW(self.disks, LB_ADDSTRING, 0, wide(&label).as_ptr() as isize);
        }
        // Keep the underlying browsing/search scope even when its row is
        // temporarily hidden by the view-only filter. Never select All by
        // accident while a scoped disk is filtered out.
        let selected = if let Some(volume_id) = self.volume {
            self.visible_disk_indices
                .iter()
                .position(|&index| id(&self.volumes[index]) == Some(volume_id))
                .map(|index| index + 1)
                .unwrap_or(usize::MAX)
        } else {
            0
        };
        SendMessageW(self.disks, LB_SETCURSEL, selected, 0);
    }

    unsafe fn refresh_physical_presence(&mut self) {
        let probe = windows_mounted_volumes();
        self.physical_probe_ok = probe.is_some();
        self.mounted_volumes = probe.unwrap_or_default();
        // Fail closed until the service has checked scope, mappings and collisions.
        let reply = self.physical_request("physical_status", None, None, None);
        let old_states = self.physical_states.clone();
        self.physical_states.clear();
        self.verified_physical_ids.clear();
        if reply["ok"] == true {
            if let Some(items) = reply["data"]["items"].as_array() {
                for item in items {
                    if let (Some(volume_id), Some(state)) =
                        (item["volume_id"].as_i64(), item["state"].as_str())
                    {
                        self.physical_states.insert(volume_id, state.to_owned());
                        if state == "connected" {
                            if let Some(physical_id) = item["verified_physical_id"].as_str() {
                                self.verified_physical_ids.insert(volume_id, physical_id.to_owned());
                            }
                        }
                    }
                }
            }
        }
        if old_states != self.physical_states {
            self.populate_physical_disk_list();
            // USB reconnect/unplug must also repaint current copy disk badges.
            if self.copy_source.is_some() {
                self.populate_candidate_disk_picker();
            }
            self.update_disk_locator();
        }
    }

    // Never copy a historical catalog path as a live OS root. Reconfirm the
    // explicit UUID binding, then independently repeat native OS enumeration
    // before placing the mount root on the local clipboard.
    unsafe fn copy_verified_os_mount_root(&mut self, catalog_id: i64) {
        self.refresh_physical_presence();
        let from_verified_probe = verified_mount_root(
            catalog_id,
            &self.physical_states,
            &self.verified_physical_ids,
            &self.mounted_volumes,
        );
        let from_fresh_probe = windows_mounted_volumes().and_then(|now| {
            verified_mount_root(
                catalog_id,
                &self.physical_states,
                &self.verified_physical_ids,
                &now,
            )
        });
        let Some(root) = from_verified_probe.filter(|root| Some(root) == from_fresh_probe.as_ref()) else {
            self.status("Current OS mount path NOT verified. Pair the physical disk, refresh and retry; no historical drive letters are used.");
            return;
        };
        self.text(self.locator, &root);
        SendMessageW(self.locator, EM_SETSEL, 0, -1);
        SendMessageW(self.locator, WM_COPY, 0, 0);
        SendMessageW(self.locator, EM_SETSEL, 0, 0);
        self.update_disk_locator();
        self.status("CURRENT OS volume root copied locally after a new UUID probe. File existence and bytes NOT verified.");
    }

    unsafe fn selected_physical_catalog_disk(&self) -> Option<i64> {
        let index = SendMessageW(
            self.files,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_SELECTED as isize,
        );
        if index >= 0 {
            self.entries
                .get(index as usize)
                .and_then(|row| row["volume_id"].as_i64())
        } else {
            self.volume
        }
    }

    unsafe fn pair_selected_physical_disk(&mut self) {
        let Some(volume_id) = self.selected_physical_catalog_disk() else {
            self.status("Select a result or catalog disk before pairing a real USB volume.");
            return;
        };
        self.refresh_physical_presence();
        if !self.physical_probe_ok || self.mounted_volumes.is_empty() {
            self.status("No reliable OS-mounted volume identities available; pairing refused.");
            return;
        }
        let name = self.volume_label(Some(volume_id));
        // Keep raw observation collisions in the Rust service, but never
        // display a duplicate UUID as an unambiguous pairing candidate.
        let candidates = unique_pairable_mounts(&self.mounted_volumes);
        if candidates.is_empty() {
            self.status("OS volume identities collide; pairing refused until resolved.");
            return;
        }
        for candidate in candidates {
            let question = format!(
                "CATALOG DISK: {name}\r\n\r\nLIVE OS VOLUME: {}\r\n\r\nThis may be an INTERNAL/system drive. Inspect the actual USB disk sticker and current mount before confirming this OS volume pairing.\r\n\r\nYES: pair this disk   NO: inspect next mount   CANCEL: stop",
                candidate.description
            );
            let answer = MessageBoxW(
                self.hwnd,
                wide(&question).as_ptr(),
                wide("Explicit physical disk pairing").as_ptr(),
                MB_YESNOCANCEL | MB_ICONQUESTION,
            );
            if answer == IDCANCEL {
                return;
            }
            if answer != IDYES {
                continue;
            }
            // The mount may have disappeared or been replaced during the
            // operator's confirmation dialog. Probe AGAIN before binding.
            self.refresh_physical_presence();
            if !self.physical_probe_ok
                || self
                    .mounted_volumes
                    .iter()
                    .filter(|v| v.id == candidate.id && v.description == candidate.description)
                    .count()
                    != 1
            {
                self.status("OS volume changed during pairing; no pairing saved. Retry after checking the USB device.");
                return;
            }
            let response =
                self.physical_request("physical_pair", Some(volume_id), Some(&candidate.id), None);
            if response["ok"] == true {
                self.refresh_physical_presence();
                self.status(
                    "Physical disk paired to live OS identity. Automatic reconnect checks enabled.",
                );
            } else {
                self.status("Pairing refused: disk is already bound, ambiguous, or OS identity changed. Unpair explicitly to replace.");
            }
            return;
        }
        self.status("No physical volume pairing made.");
    }

    unsafe fn unpair_selected_physical_disk(&mut self) {
        let Some(volume_id) = self.selected_physical_catalog_disk() else {
            self.status("Select a catalog disk to clear its stored physical pairing.");
            return;
        };
        let question = format!(
            "Remove the explicit OS-volume pairing for catalog disk {}?\r\nNo catalog entries, CTF bytes, or files will be changed.",
            self.volume_label(Some(volume_id))
        );
        if MessageBoxW(
            self.hwnd,
            wide(&question).as_ptr(),
            wide("Remove physical disk pairing").as_ptr(),
            MB_YESNO | MB_ICONQUESTION,
        ) != IDYES
        {
            return;
        }
        let response = self.physical_request("physical_unpair", Some(volume_id), None, None);
        self.refresh_physical_presence();
        self.status(if response["ok"] == true {
            "Physical identity pairing cleared; disk returns to NOT VERIFIED."
        } else {
            "Pairing could not be cleared; check user-side identity store."
        });
    }

    unsafe fn focus_sole_connected_copy(&mut self) {
        if !self.page_ready || self.copy_source.is_none() || self.copy_disk_groups.is_empty() {
            self.status("Find copies of a file first (Ctrl+D).");
            return;
        }
        // No stale cached green badge: probe the OS again before navigating.
        self.refresh_physical_presence();
        if !self.physical_probe_ok {
            self.status("Incomplete OS mount scan: physical copy location unverified.");
            return;
        }
        let Some(volume) =
            sole_connected_candidate_disk(&self.copy_disk_groups, &self.physical_states)
        else {
            self.status(
                "Zero or several candidate disks are OS-verified; choose a specific disk manually.",
            );
            return;
        };
        self.copy_focus_volume_id = Some(volume);
        self.offset = 0;
        self.load();
    }

    unsafe fn populate_candidate_disk_picker(&self) {
        SendMessageW(self.copy_disk_filter, CB_RESETCONTENT, 0, 0);
        if let Some((_, button)) = self.buttons.iter().find(|(id, _)| *id == COPY_CONNECTED) {
            ShowWindow(
                *button,
                if self.copy_source.is_some() {
                    SW_SHOW
                } else {
                    SW_HIDE
                },
            );
        }
        if self.copy_source.is_none() {
            ShowWindow(self.copy_disk_filter, SW_HIDE);
            return;
        }
        let all: u64 = self
            .copy_disk_groups
            .iter()
            .filter_map(|group| group["count"].as_u64())
            .sum();
        let caption = format!(
            "All candidate disks • {all} files / {} disks",
            self.copy_disk_groups.len()
        );
        SendMessageW(
            self.copy_disk_filter,
            CB_ADDSTRING,
            0,
            wide(&caption).as_ptr() as isize,
        );
        for group in ordered_candidate_disks(&self.copy_disk_groups, &self.physical_states) {
            let caption =
                candidate_disk_choice_caption(group, &self.shelf_labels, &self.physical_states);
            let warning = group["volume_id"]
                .as_i64()
                .and_then(|disk| shelf_collision_warning(disk, &self.shelf_tag_conflicts))
                .map(|_| " ! SHARED STICKER")
                .unwrap_or("");
            SendMessageW(
                self.copy_disk_filter,
                CB_ADDSTRING,
                0,
                wide(&format!("{caption}{warning}")).as_ptr() as isize,
            );
        }
        let selection = self
            .copy_focus_volume_id
            .and_then(|focused| {
                ordered_candidate_disks(&self.copy_disk_groups, &self.physical_states)
                    .iter()
                    .position(|group| group["volume_id"].as_i64() == Some(focused))
            })
            .map(|index| index + 1)
            .unwrap_or(0);
        SendMessageW(self.copy_disk_filter, CB_SETCURSEL, selection, 0);
        ShowWindow(self.copy_disk_filter, SW_SHOW);
    }

    unsafe fn update_disk_locator(&self) {
        let index = SendMessageW(
            self.files,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_SELECTED as isize,
        );
        let selected = if index >= 0 {
            self.entries.get(index as usize)
        } else {
            None
        };
        let disk = selected
            .map(|row| self.volume_label(row["volume_id"].as_i64()))
            .unwrap_or_default();
        let mut inspector = disk_locator_text(
            selected,
            &disk,
            self.linked_mode(),
            self.physical_states
                .get(
                    &selected
                        .and_then(|row| row["volume_id"].as_i64())
                        .unwrap_or_default(),
                )
                .map(String::as_str)
                .unwrap_or("unverified"),
        );
        if let Some(shelf) = selected
            .and_then(|row| row["volume_id"].as_i64())
            .and_then(|disk| self.shelf_labels.get(&disk))
        {
            inspector.push_str(&format!(
                "\r\n\r\nOWNER-ENTERED SHELF/STICKER TAG (not USB identity)\r\n{shelf}"
            ));
        }

        if let Some(warning) = selected
            .and_then(|row| row["volume_id"].as_i64())
            .and_then(|disk| shelf_collision_warning(disk, &self.shelf_tag_conflicts))
        {
            inspector.push_str(&format!("\r\n\r\n{warning}"));
        }
        if self.copy_source.is_some() && !self.copy_disk_groups.is_empty() {
            inspector.push_str("\r\n\r\nCOPIES BY CATALOG DISK (metadata only)");
            for group in ordered_candidate_disks(&self.copy_disk_groups, &self.physical_states) {
                let mark = if group["volume_id"].as_i64() == self.copy_focus_volume_id {
                    " [FILTERED]"
                } else {
                    ""
                };
                let caption =
                    candidate_disk_choice_caption(group, &self.shelf_labels, &self.physical_states);
                inspector.push_str(&format!("\r\n{caption}{mark}"));
            }
        }
        if self.copy_source.is_some() {
            inspector.push_str("\r\n\r\nCtrl+G: focus sole OS-verified disk after fresh probe.\r\nMetadata only: candidate file contents not checked.");
        }
        self.text(self.locator, &inspector);
    }
    fn sort_loaded_page(&mut self) {
        let Some(column) = self.page_sort_column else {
            return;
        };
        let labels: HashMap<i64, String> = self
            .volumes
            .iter()
            .filter_map(|volume| id(volume).map(|key| (key, string(volume, "label"))))
            .collect();
        sort_page_rows(
            &mut self.entries,
            column,
            self.page_sort_descending,
            &labels,
        );
    }

    fn save_ui_layout_preferences(&self) {
        let dir = config_dir();
        if std::fs::create_dir_all(&dir).is_ok() {
            let data = json!({"version":1, "locator_visible":self.locator_visible,
                "locator_width":self.locator_requested_width.clamp(220, 420)});
            let _ = std::fs::write(dir.join("ui-layout.json"), data.to_string());
        }
    }

    unsafe fn focus_current_page_results(&mut self) {
        let selected = SendMessageW(
            self.files,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_SELECTED as isize,
        );
        let Some(index) = focusable_result_row(self.page_ready, &self.entries, selected) else {
            self.status("No verified current result page to focus. Search or wait for loading.");
            return;
        };
        if selected != index as isize {
            let mut state: LVITEMW = std::mem::zeroed();
            state.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
            state.state = LVIS_SELECTED | LVIS_FOCUSED;
            SendMessageW(
                self.files,
                LVM_SETITEMSTATE,
                index,
                &state as *const _ as isize,
            );
        }
        SetFocus(self.files);
        SendMessageW(self.files, LVM_ENSUREVISIBLE, index, 0);
        self.update_disk_locator();
    }

    unsafe fn sort_current_page_by(&mut self, column: i32) {
        let Ok(column) = usize::try_from(column) else {
            return;
        };
        if column >= SORT_LABELS.len() {
            return;
        }
        let selected_row = SendMessageW(
            self.files,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_SELECTED as isize,
        );
        let selected_id = usize::try_from(selected_row)
            .ok()
            .and_then(|index| self.entries.get(index))
            .and_then(id);
        if self.page_sort_column == Some(column) {
            self.page_sort_descending = !self.page_sort_descending;
        } else {
            self.page_sort_column = Some(column);
            self.page_sort_descending = false;
        }
        self.sort_loaded_page();
        self.render_rows();
        if let Some(selected_id) = selected_id {
            if let Some(row) = self
                .entries
                .iter()
                .position(|entry| id(entry) == Some(selected_id))
            {
                let mut state: LVITEMW = std::mem::zeroed();
                state.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
                state.state = LVIS_SELECTED | LVIS_FOCUSED;
                SendMessageW(
                    self.files,
                    LVM_SETITEMSTATE,
                    row,
                    &state as *const _ as isize,
                );
                SendMessageW(self.files, LVM_ENSUREVISIBLE, row, 0);
                self.update_disk_locator();
            }
        }
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
        self.update_disk_locator();
        self.status(&format!(
            "{}–{}{}{}{}{}",
            if self.total == 0 { 0 } else { self.offset + 1 },
            self.offset + self.entries.len(),
            if self.total_known {
                format!(" of {} items", self.total)
            } else if self.has_more {
                " items • more available".into()
            } else {
                " items".into()
            },
            if self.copy_source.is_some() {
                if self.copy_focus_volume_id.is_some() {
                    " • selected catalog disk; name+size only; contents not compared"
                } else {
                    " • OTHER catalog disks only; name+size; contents not compared"
                }
            } else if self.duplicates {
                " • metadata candidates; content not verified"
            } else {
                ""
            },
            if self.job.is_some() {
                " • operation in progress"
            } else {
                ""
            },
            self.page_sort_column
                .map(|column| format!(
                    " • {} {} (current page only)",
                    SORT_LABELS[column],
                    if self.page_sort_descending {
                        "descending"
                    } else {
                        "ascending"
                    }
                ))
                .unwrap_or_default()
        ));
    }
    unsafe fn search_same_filename(&mut self) {
        // Never act on a stale search page or silently leave the copy inspector.
        if !self.page_ready || self.copy_source.is_some() {
            self.status("Return to a verified search result before finding same filenames.");
            return;
        }
        let row = SendMessageW(
            self.files,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_SELECTED as isize,
        );
        let Some(entry) = (row >= 0).then(|| self.entries.get(row as usize)).flatten() else {
            self.status("Select a current file result to search its filename.");
            return;
        };
        if string(entry, "kind") != "file" {
            self.status("Select a file, not a folder, for exact filename search.");
            return;
        }
        let Some(query) = exact_name_query(&string(entry, "name")) else {
            self.status("Filename is invalid or exceeds the 4096-byte search limit.");
            return;
        };
        self.copy_return = Some(CopyReturn {
            volume: self.volume,
            stack: self.stack.clone(),
            query: self.query.clone(),
            duplicates: self.duplicates,
            offset: self.offset,
        });
        self.copy_source = None;
        self.copy_source_name = None;
        self.duplicates = false;
        self.query = query;
        self.text(self.search, &self.query);
        self.volume = None;
        self.stack = vec![(None, "/".into())];
        self.offset = 0;
        SendMessageW(self.disks, LB_SETCURSEL, 0, 0);
        self.load(); // linked CTF uses its existing fresh source-generation SHA check.
    }
    unsafe fn search_same_recorded_size(&mut self) {
        // Follow the current visible file row, never stale asynchronous results
        // or a candidate sub-page. This is a GLOBAL metadata search, not proof
        // of renamed-file identity, mounted USB state or matching file bytes.
        if !self.page_ready || self.copy_source.is_some() {
            self.status("Return to a current search page before finding same-size files.");
            return;
        }
        let row = SendMessageW(
            self.files,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_SELECTED as isize,
        );
        let Some(entry) = (row >= 0).then(|| self.entries.get(row as usize)).flatten() else {
            self.status("Select a current file result to search by recorded byte size.");
            return;
        };
        if string(entry, "kind") != "file" {
            self.status("Select a file, not a folder, for recorded-size comparison.");
            return;
        }
        let Some(query) = exact_recorded_size_query(entry["size"].as_u64()) else {
            self.status("Recorded size must be positive, known and within signed 64-bit range.");
            return;
        };
        self.copy_return = Some(CopyReturn {
            volume: self.volume,
            stack: self.stack.clone(),
            query: self.query.clone(),
            duplicates: self.duplicates,
            offset: self.offset,
        });
        self.copy_source = None;
        self.copy_source_name = None;
        self.duplicates = false;
        self.query = query;
        self.text(self.search, &self.query);
        self.volume = None;
        self.stack = vec![(None, "/".into())];
        self.offset = 0;
        SendMessageW(self.disks, LB_SETCURSEL, 0, 0);
        self.load(); // linked CTF rechecks original SHA independently on each page.
    }
    unsafe fn show_copies_of_selected(&mut self) {
        let index = SendMessageW(
            self.files,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_SELECTED as isize,
        );
        let selected = if index >= 0 {
            self.entries.get(index as usize)
        } else {
            None
        };
        let Some(entry) = selected else {
            self.status("Select a file to compare its catalogued metadata across disks.");
            return;
        };
        if string(entry, "kind") != "file" || entry["size"].as_u64().is_none() {
            self.status("Only file entries with recorded byte size can be compared.");
            return;
        }
        let Some(source) = id(entry) else {
            return;
        };
        if self.copy_source.is_none() {
            self.copy_return = Some(CopyReturn {
                volume: self.volume,
                stack: self.stack.clone(),
                query: self.query.clone(),
                duplicates: self.duplicates,
                offset: self.offset,
            });
        }
        self.copy_source = Some(source);
        self.copy_source_name = Some(string(entry, "name"));
        self.copy_focus_volume_id = None;
        self.copy_disk_groups.clear();
        if self
            .disk_filter_query
            .trim()
            .eq_ignore_ascii_case("copies:")
        {
            // Prevent a prior file's candidate rows remaining in the register.
            self.populate_physical_disk_list();
        }
        self.populate_candidate_disk_picker();
        self.volume = None;
        self.offset = 0;
        self.stack = vec![(None, "/".into())];
        SendMessageW(self.disks, LB_SETCURSEL, 0, 0);
        self.load();
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
            SHELF_SAVE => self.save_shelf_label(),
            SEARCH_BOX if notification == EN_CHANGE as usize => {
                self.update_video_filter_caption();
            }
            VIDEO_FILTER => {
                if let Some(next) = toggle_video_query(&self.search_edit_text()) {
                    self.text(self.search, &next);
                    self.update_video_filter_caption();
                    self.command(FIND, 0);
                } else {
                    self.status("Query would exceed 4096 UTF-8 bytes. Shorten it first.");
                }
            }
            DISK_FILTER if notification == EN_CHANGE as usize => {
                let len = GetWindowTextLengthW(self.disk_filter);
                let mut buffer = vec![0u16; len as usize + 1];
                GetWindowTextW(self.disk_filter, buffer.as_mut_ptr(), len + 1);
                self.disk_filter_query = String::from_utf16_lossy(&buffer[..len as usize]);
                self.populate_physical_disk_list();
            }
            COPY_DISK_FILTER
                if notification == CBN_SELCHANGE as usize && self.copy_source.is_some() =>
            {
                let selected = SendMessageW(self.copy_disk_filter, CB_GETCURSEL, 0, 0);
                if selected < 0 {
                    return;
                }
                self.copy_focus_volume_id = if selected == 0 {
                    None
                } else {
                    ordered_candidate_disks(&self.copy_disk_groups, &self.physical_states)
                        .get(selected as usize - 1)
                        .and_then(|group| group["volume_id"].as_i64())
                };
                self.offset = 0;
                self.load();
            }
            DISKS if notification == LBN_SELCHANGE as usize => {
                self.copy_source = None;
                self.copy_source_name = None;
                self.copy_return = None;
                let index = SendMessageW(self.disks, LB_GETCURSEL, 0, 0);
                if index < 0 {
                    return;
                }
                self.volume = if index == 0 {
                    None
                } else {
                    filtered_disk_index_at_row(&self.visible_disk_indices, index)
                        .and_then(|volume_index| self.volumes.get(volume_index))
                        .and_then(id)
                };
                self.stack = vec![(None, "/".into())];
                self.query.clear();
                self.duplicates = false;
                self.text(self.search, "");
                self.offset = 0;
                self.load();
                self.sync_shelf_editor();
            }
            FIND | FIND_DISK | DUPLICATES => {
                if command == FIND_DISK && self.volume.is_none() {
                    self.status("Choose a disk in the register for an explicit disk-only search.");
                    return;
                }
                self.copy_source = None;
                self.copy_source_name = None;
                self.copy_return = None;
                let len = GetWindowTextLengthW(self.search);
                let mut text = vec![0u16; len as usize + 1];
                GetWindowTextW(self.search, text.as_mut_ptr(), len + 1);
                self.query = String::from_utf16_lossy(&text[..len as usize])
                    .trim()
                    .into();
                self.duplicates = command == DUPLICATES;
                if command == FIND || command == DUPLICATES {
                    // Enter/Search and Duplicates are always cross-disk.
                    // "This disk" is the only way to retain the selected filter.
                    self.volume = None;
                    self.stack = vec![(None, "/".into())];
                    SendMessageW(self.disks, LB_SETCURSEL, 0, 0);
                }
                if self.duplicates {
                    self.applied_duplicate_mode = self.duplicate_mode_name();
                }
                self.offset = 0;
                self.load();
            }
            BACK => {
                if let Some(context) = self.copy_return.take() {
                    self.copy_source = None;
                    self.copy_source_name = None;
                    self.volume = context.volume;
                    self.stack = context.stack;
                    self.query = context.query;
                    self.duplicates = context.duplicates;
                    self.offset = context.offset;
                    self.text(self.search, &self.query);
                    self.populate_physical_disk_list();
                    self.load();
                    return;
                }
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
            STOP_PAGE_WAIT => self.stop_waiting_for_page(),
            RETRY_PAGE => {
                if self.page_retry_available {
                    self.load(); // New request, new generation and fresh linked CTF SHA check.
                } else {
                    self.status("No dismissed or failed page to retry.");
                }
            }
            FOCUS_RESULTS => self.focus_current_page_results(),
            FILE_COPIES => self.show_copies_of_selected(),
            SAME_FILENAME => self.search_same_filename(),
            SAME_RECORDED_SIZE => self.search_same_recorded_size(),
            REVEAL_DISK => {
                let selected_row = SendMessageW(
                    self.files,
                    LVM_GETNEXTITEM,
                    usize::MAX,
                    LVNI_SELECTED as isize,
                );
                let Some(filter) =
                    reveal_disk_filter_for_row(&self.entries, selected_row, &self.volumes)
                else {
                    self.status("Select a current file result with a registered catalog disk.");
                    return;
                };
                // View filter only: never change query, browse scope, OS pairing,
                // linked source, selection or candidate-copy navigation.
                self.disk_filter_query = filter.clone();
                self.text(self.disk_filter, &filter);
                self.populate_physical_disk_list();
                SetFocus(self.disk_filter);
                self.status("Showing selected file's exact disk in register. Search scope and OS pairing unchanged; click disk to browse.");
            }
            COPY_REGISTER_FILTER => {
                if self.copy_source.is_none() || self.copy_disk_groups.is_empty() {
                    self.status("Find metadata copies of a file first (Ctrl+D).");
                    return;
                }
                self.disk_filter_query = "copies:".to_owned();
                self.text(self.disk_filter, "copies:");
                self.populate_physical_disk_list();
                SetFocus(self.disks);
                self.status("Register shows exact IDs of candidate disks only. Search, selected file and OS pairing are unchanged; Ctrl+L clears filter.");
            }
            COPY_CONNECTED => self.focus_sole_connected_copy(),
            PHYSICAL_CONNECTED_FILTER | PHYSICAL_UNVERIFIED_FILTER => {
                // First check current OS volumes. A failed probe cannot retain
                // a prior connected result in the left view.
                self.refresh_physical_presence();
                let filter = if command == PHYSICAL_CONNECTED_FILTER {
                    "state:connected"
                } else {
                    "state:unverified"
                };
                self.disk_filter_query = filter.to_owned();
                self.text(self.disk_filter, filter);
                self.populate_physical_disk_list();
                SetFocus(self.disks);
                self.status("Physical register audit: only last OS-checked IDs, no recorded path/label inference. Search scope unchanged; Ctrl+L clears.");
            }
            COPY_PAGE_MANIFEST => {
                if !self.page_ready || self.copy_source.is_none() {
                    self.status("Wait for the current candidate page to finish verifying before copying paths.");
                    return;
                }
                let Some(name) = self.copy_source_name.as_deref() else {
                    self.status("No current catalogued file for the page manifest.");
                    return;
                };
                let manifest = candidate_page_manifest(
                    name,
                    &self.entries,
                    self.offset,
                    self.total,
                    self.copy_focus_volume_id,
                );
                self.text(self.locator, &manifest);
                SendMessageW(self.locator, EM_SETSEL, 0, -1);
                SendMessageW(self.locator, WM_COPY, 0, 0);
                SendMessageW(self.locator, EM_SETSEL, 0, 0);
                self.update_disk_locator();
                self.status("Current candidate PAGE paths copied locally. Other pages, physical presence and file bytes remain unverified.");
            }
            COPY_PICKUP_PLAN => {
                if !self.page_ready {
                    self.status("Candidate page still loading or source verification failed; nothing copied.");
                    return;
                }
                let Some(plan) = self.copy_source_name.as_deref().and_then(|name| {
                    candidate_disk_pickup_list(
                        name,
                        &self.copy_disk_groups,
                        &self.shelf_labels,
                        &self.physical_states,
                        &self.shelf_tag_conflicts,
                    )
                }) else {
                    self.status("Find copies and wait for catalogued candidate disks first.");
                    return;
                };
                // Existing native EDIT is a transient clipboard source; restore
                // the live inspector immediately, without disk/catalog writes.
                self.text(self.locator, &plan);
                SendMessageW(self.locator, EM_SETSEL, 0, -1);
                SendMessageW(self.locator, WM_COPY, 0, 0);
                SendMessageW(self.locator, EM_SETSEL, 0, 0);
                self.update_disk_locator();
                self.status("Disk pickup checklist copied locally. OS badges are last observations; file bytes unverified.");
            }
            COPY_VERIFIED_OS_ROOT => {
                let Some(catalog_id) = self.selected_physical_catalog_disk() else {
                    self.status("Select a current file or catalog disk before copying a live OS root.");
                    return;
                };
                if let Some(pinned) = self.linked_request("ctf_volumes_pinned") {
                    // Revalidate linked CTF generation asynchronously; never
                    // freeze the Win32 UI waiting for a large source read.
                    self.pending_mount_root = Some((catalog_id, self.generation));
                    self.call("mount_root_source", pinned);
                } else {
                    self.copy_verified_os_mount_root(catalog_id);
                }
            }
            COPY_LOCATOR => {
                let row = SendMessageW(
                    self.files,
                    LVM_GETNEXTITEM,
                    usize::MAX,
                    LVNI_SELECTED as isize,
                );
                if row < 0 || self.entries.get(row as usize).is_none() {
                    self.status("Select a catalogued file before copying its retrieval note.");
                    return;
                }
                // Copy only the selected row's current read-only inspector text.
                // The Win32 EDIT owns local clipboard handling; no CTF, cache or
                // annotation write, network request or historical path probe.
                self.update_disk_locator();
                SendMessageW(self.locator, EM_SETSEL, 0, -1);
                SendMessageW(self.locator, WM_COPY, 0, 0);
                SendMessageW(self.locator, EM_SETSEL, 0, 0);
                self.status("Locator text sent to the local clipboard. Mount status can change; file contents were not checked.");
            }
            TOGGLE_LOCATOR => {
                self.locator_visible = !self.locator_visible;
                self.save_ui_layout_preferences();
                self.layout();
                InvalidateRect(self.hwnd, null(), 0);
                self.status(if self.locator_visible {
                    "Disk locator enabled. Drag the vertical grip to resize it (Ctrl+I toggles)."
                } else {
                    "Disk locator hidden; recorded paths remain in the results table (Ctrl+I)."
                });
            }
            PHYSICAL_CHECK => {
                self.refresh_physical_presence();
                self.status(if self.physical_probe_ok {
                    "Live OS mount scan complete; only explicitly paired UUIDs can show connected."
                } else {
                    "OS mount probe failed; all physical disk statuses are NOT VERIFIED."
                });
            }
            PHYSICAL_PAIR => self.pair_selected_physical_disk(),
            PHYSICAL_UNPAIR => self.unpair_selected_physical_disk(),
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
                    self.annotation_session = self.annotation_session.wrapping_add(1);
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
                if self.linked_mode()
                    && command == EXPORT
                    && !linked_export_allowed(SendMessageW(self.export_format, CB_GETCURSEL, 0, 0))
                {
                    self.status(
                        "Linked CTF supports CSV/Parquet/JSON export; original stays read-only.",
                    );
                    return;
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
                            3 => "json",
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
                                    3 => "export_json",
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
                                linked["format"] = json!(match operation {
                                    "export_csv" => "csv",
                                    "export_parquet" => "parquet",
                                    "export_json" => "json",
                                    _ => "ctf",
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
    unsafe fn locator_splitter_hit(&self, x: i32, y: i32) -> bool {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(self.hwnd, &mut rect);
        locator_splitter_hit_test(
            rect.right,
            rect.bottom,
            self.locator_requested_width,
            self.locator_visible,
            x,
            y,
        )
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
        // Search occupies the dominant space; scope is explicit on small laptops.
        MoveWindow(self.search, 8, 34, (w - 618).max(100), 24, 1);
        MoveWindow(self.duplicate_mode, w - 312, 34, 136, 200, 1);
        // Candidate focus occupies the query-help row only in copy mode.
        // Search syntax help is hidden then; Notes stays on the right.
        // Leave room for three explicit copy actions without covering the disk selector.
        let copy_filter_width = if self.copy_source.is_some() {
            (w - 634).max(100)
        } else {
            (w - 466).max(100)
        };
        MoveWindow(self.copy_disk_filter, 8, 62, copy_filter_width, 240, 1);
        MoveWindow(self.help, 8, 62, (w - 340).max(150), 18, 1);
        for (i, hwnd) in &self.buttons {
            if *i == FIND {
                MoveWindow(*hwnd, w - 604, 34, 104, 24, 1);
            }
            if *i == FIND_DISK {
                MoveWindow(*hwnd, w - 494, 34, 88, 24, 1);
            }
            if *i == BACK {
                MoveWindow(*hwnd, w - 400, 34, 82, 24, 1);
            }
            if *i == DUPLICATES {
                MoveWindow(*hwnd, w - 170, 34, 162, 24, 1);
            }
            if *i == STOP_PAGE_WAIT {
                MoveWindow(*hwnd, w - 410, h - 32, 100, 24, 1);
            }
            if *i == RETRY_PAGE {
                MoveWindow(*hwnd, w - 305, h - 32, 100, 24, 1);
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
        MoveWindow(self.disk_filter, 8, 106, 200, 24, 1);
        MoveWindow(self.shelf_edit, 8, 134, 140, 24, 1);
        MoveWindow(self.disks, 8, 162, 200, (disk_height - 56).max(42), 1);
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
                SHELF_SAVE => {
                    MoveWindow(*control, 154, 134, 54, 24, 1);
                }
                VIDEO_FILTER => {
                    MoveWindow(*control, w - 290, 62, 132, 23, 1);
                    ShowWindow(
                        *control,
                        if self.copy_source.is_some() {
                            SW_HIDE
                        } else {
                            SW_SHOW
                        },
                    );
                }
                ANNOTATION_PREVIEW => {
                    MoveWindow(*control, w - 144, 62, 136, 23, 1);
                }
                COPY_PAGE_MANIFEST => {
                    MoveWindow(*control, w - 616, 62, 146, 23, 1);
                    ShowWindow(
                        *control,
                        if self.copy_source.is_some() {
                            SW_SHOW
                        } else {
                            SW_HIDE
                        },
                    );
                }
                COPY_PICKUP_PLAN => {
                    MoveWindow(*control, w - 460, 62, 146, 23, 1);
                    ShowWindow(
                        *control,
                        if self.copy_source.is_some() {
                            SW_SHOW
                        } else {
                            SW_HIDE
                        },
                    );
                }
                COPY_CONNECTED => {
                    MoveWindow(*control, w - 304, 62, 142, 23, 1);
                    ShowWindow(
                        *control,
                        if self.copy_source.is_some() {
                            SW_SHOW
                        } else {
                            SW_HIDE
                        },
                    );
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
        // Keep the legacy full-width table on narrow screens; the locator
        // becomes a third pane only when it does not obscure the file list.
        let (locator_width, file_width, _) =
            explorer_geometry_for_width(w, self.locator_requested_width, self.locator_visible);
        MoveWindow(self.files, 214, 106, file_width, (h - 146).max(60), 1);
        for (index, width) in result_column_widths(file_width, locator_width > 0)
            .iter()
            .enumerate()
        {
            SendMessageW(self.files, LVM_SETCOLUMNWIDTH, index, *width as isize);
        }
        if locator_width > 0 {
            MoveWindow(
                self.locator,
                214 + file_width + 8,
                106,
                locator_width,
                (h - 306).max(60),
                1,
            );
            ShowWindow(self.locator, SW_SHOW);
        } else {
            ShowWindow(self.locator, SW_HIDE);
        }
        for (command, y) in [
            (PHYSICAL_CHECK, h - 185),
            (PHYSICAL_PAIR, h - 155),
            (PHYSICAL_UNPAIR, h - 125),
            (FILE_COPIES, h - 78),
            (REVEAL_DISK, h - 78),
            (COPY_LOCATOR, h - 48),
            (COPY_VERIFIED_OS_ROOT, h - 48),
        ] {
            if let Some((_, button)) = self.buttons.iter().find(|(id, _)| *id == command) {
                if locator_width > 0 {
                    let left = 214 + file_width + 8;
                    let half = (locator_width - 6) / 2;
                    let (button_left, button_width) = match command {
                        FILE_COPIES => (left, half),
                        REVEAL_DISK => (left + half + 6, locator_width - half - 6),
                        COPY_LOCATOR => (left, half),
                        COPY_VERIFIED_OS_ROOT => (left + half + 6, locator_width - half - 6),
                        _ => (left, locator_width),
                    };
                    MoveWindow(*button, button_left, y, button_width, 26, 1);
                    ShowWindow(*button, SW_SHOW);
                } else {
                    ShowWindow(*button, SW_HIDE);
                }
            }
        }
        MoveWindow(self.status, 8, h - 32, (w - 426).max(50), 24, 1);
    }
}

unsafe fn file_dialog(owner: HWND, save: bool, format: &str) -> Option<String> {
    let mut buffer = vec![0u16; 32768];
    let filter=wide(match format {"annotation"=>"User notes (*.wia.sqlite)\0*.wia.sqlite\0","ctf"=>"CTF catalogs (*.ctf)\0*.ctf\0All files\0*.*\0","csv"=>"CSV table (*.csv)\0*.csv\0All files\0*.*\0","parquet"=>"Apache Parquet (*.parquet)\0*.parquet\0All files\0*.*\0","json"=>"JSON table (*.json)\0*.json\0All files\0*.*\0",_=>"Atlas SQLite catalog (*.atlas.sqlite;*.sqlite)\0*.atlas.sqlite;*.sqlite\0All files\0*.*\0"});
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
                (STOP_PAGE_WAIT, "Stop &waiting"),
                (RETRY_PAGE, "&Retry page"),
                (FIND, "Search &all"),
                (FIND_DISK, "&This disk"),
                (BACK, "&Back"),
                (PREV, "&Previous"),
                (NEXT, "Ne&xt"),
                (DUPLICATES, "&Duplicates"),
                (VIDEO_FILTER, "&Video files"),
                (FILE_COPIES, "&Find copies"),
                (REVEAL_DISK, "&Show disk"),
                (COPY_LOCATOR, "Copy &locator text"),
                (COPY_VERIFIED_OS_ROOT, "Copy OS &root"),
                (COPY_CONNECTED, "&Verified copy"),
                (COPY_PICKUP_PLAN, "Copy &pickup list"),
                (COPY_PAGE_MANIFEST, "Copy page &paths"),
                (PHYSICAL_CHECK, "Refresh &USB state"),
                (PHYSICAL_PAIR, "&Pair physical disk…"),
                (PHYSICAL_UNPAIR, "Unpair disk"),
                (SHELF_SAVE, "&Save tag"),
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
                "JSON — current branch/query",
            ] {
                SendMessageW(
                    app.export_format,
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
                );
            }
            SendMessageW(app.export_format, CB_SETCURSEL, 0, 0);
            app.copy_disk_filter = control(
                hwnd,
                "COMBOBOX",
                "",
                WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                COPY_DISK_FILTER,
            );
            ShowWindow(app.copy_disk_filter, SW_HIDE);
            app.help = control(
                hwnd,
                "STATIC",
                "AND  OR  NOT  ( )   name: path: ext: type:video size: category:",
                0,
                142,
            );
            MoveWindow(app.help, 8, 62, 700, 18, 1);
            app.disk_filter = control(
                hwnd,
                "EDIT",
                "",
                WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                DISK_FILTER,
            );
            SendMessageW(
                app.disk_filter,
                EM_SETCUEBANNER,
                0,
                wide("Filter labels, IDs, shelf tags • Ctrl+L").as_ptr() as isize,
            );
            app.shelf_edit = control(
                hwnd,
                "EDIT",
                "",
                WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                134,
            );
            SendMessageW(
                app.shelf_edit,
                EM_SETCUEBANNER,
                0,
                wide("Sticker / shelf • selected disk").as_ptr() as isize,
            );
            SendMessageW(app.shelf_edit, EM_LIMITTEXT, 80, 0);
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
                ("Name", 230),
                ("Disk", 120),
                ("Kind", 70),
                ("Size", 90),
                ("Modified", 120),
                ("Candidates", 70),
                ("Path", 185),
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
            app.locator = control(
                hwnd,
                "EDIT",
                "",
                WS_TABSTOP
                    | WS_BORDER
                    | WS_VSCROLL
                    | ES_MULTILINE as u32
                    | ES_AUTOVSCROLL as u32
                    | ES_READONLY as u32,
                143,
            );
            SetTimer(hwnd, 1, 80, None);
            SetTimer(hwnd, 2, 6_000, None);
            app.init();
            0
        }
        WM_SIZE => {
            app.layout();
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONDOWN => {
            let x = (lp as u16 as i16) as i32;
            let y = ((lp >> 16) as u16 as i16) as i32;
            if app.locator_splitter_hit(x, y) {
                app.dragging_locator = true;
                SetCapture(hwnd);
                return 0;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_MOUSEMOVE if app.dragging_locator => {
            let x = (lp as u16 as i16) as i32;
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            app.locator_requested_width = rect.right - x - 4;
            app.layout();
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONUP if app.dragging_locator => {
            app.dragging_locator = false;
            app.save_ui_layout_preferences();
            ReleaseCapture();
            0
        }
        WM_CAPTURECHANGED => {
            app.dragging_locator = false;
            0
        }
        WM_SETCURSOR => {
            let mut cursor: POINT = std::mem::zeroed();
            if GetCursorPos(&mut cursor) != 0 {
                ScreenToClient(hwnd, &mut cursor);
                if app.dragging_locator || app.locator_splitter_hit(cursor.x, cursor.y) {
                    SetCursor(LoadCursorW(null_mut(), IDC_SIZEWE));
                    return 1;
                }
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_PAINT => {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let (locator_width, _, _) = explorer_geometry_for_width(
                rect.right,
                app.locator_requested_width,
                app.locator_visible,
            );
            if locator_width > 0 {
                let left = rect.right - locator_width - 8;
                let middle = (rect.bottom - 40) / 2;
                for offset in [2, 5] {
                    let grip = RECT {
                        left: left + offset,
                        top: middle - 20,
                        right: left + offset + 1,
                        bottom: middle + 20,
                    };
                    FillRect(dc, &grip, GetSysColorBrush(COLOR_3DSHADOW));
                }
            }
            EndPaint(hwnd, &paint);
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
            } else if header.idFrom == FILES && header.code == LVN_ITEMCHANGED {
                app.update_disk_locator();
                app.sync_shelf_editor();
            } else if header.idFrom == FILES && header.code == LVN_COLUMNCLICK {
                let event = &*(lp as *const NMLISTVIEW);
                app.sort_current_page_by(event.iSubItem);
            } else if header.idFrom == FILES && header.code == LVN_KEYDOWN {
                let key = &*(lp as *const NMLVKEYDOWN);
                if key.wVKey == VK_RETURN {
                    app.activate();
                }
            }
            0
        }
        WM_TIMER => {
            if wp == 2 {
                app.refresh_physical_presence();
            } else {
                app.poll();
            }
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
            app.save_ui_layout_preferences();
            KillTimer(hwnd, 1);
            KillTimer(hwnd, 2);
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
            let physical = CreatePopupMenu();
            if !physical.is_null() {
                AppendMenuW(
                    physical,
                    MF_STRING,
                    PHYSICAL_CHECK,
                    wide("Recheck connected disks").as_ptr(),
                );
                AppendMenuW(
                    physical,
                    MF_STRING,
                    PHYSICAL_CONNECTED_FILTER,
                    wide("Show OS-verified connected disks in register\tCtrl+Shift+O").as_ptr(),
                );
                AppendMenuW(
                    physical,
                    MF_STRING,
                    PHYSICAL_UNVERIFIED_FILTER,
                    wide("Show unverified disks in register\tCtrl+Shift+U").as_ptr(),
                );
                AppendMenuW(
                    physical,
                    MF_STRING,
                    PHYSICAL_PAIR,
                    wide("Pair selected catalog disk with OS volume...").as_ptr(),
                );
                AppendMenuW(
                    physical,
                    MF_STRING,
                    PHYSICAL_UNPAIR,
                    wide("Clear physical pairing...").as_ptr(),
                );
                AppendMenuW(
                    menu,
                    MF_POPUP,
                    physical as usize,
                    wide("Physical disks").as_ptr(),
                );
            }
            let view = CreatePopupMenu();
            if !view.is_null() {
                AppendMenuW(
                    view,
                    MF_STRING,
                    FOCUS_RESULTS,
                    wide("Focus loaded results\tCtrl+E").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    STOP_PAGE_WAIT,
                    wide("Stop waiting for catalog page\tCtrl+Shift+X").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    RETRY_PAGE,
                    wide("Retry dismissed/failed catalog page\tCtrl+Shift+Y").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    TOGGLE_LOCATOR,
                    wide("Show / hide disk locator\tCtrl+I").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    SAME_FILENAME,
                    wide("Find same filename across all disks (any size)\tCtrl+Shift+N").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    SAME_RECORDED_SIZE,
                    wide("Find same recorded byte size (any filename)\tCtrl+Shift+S").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    COPY_LOCATOR,
                    wide("Copy selected locator text\tCtrl+Shift+C").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    COPY_VERIFIED_OS_ROOT,
                    wide("Copy freshly verified OS volume root\tCtrl+Shift+H").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    REVEAL_DISK,
                    wide("Show selected file's disk in register\tCtrl+J").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    PHYSICAL_CHECK,
                    wide("Recheck physical volumes\tCtrl+R").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    COPY_CONNECTED,
                    wide("Focus sole OS-verified copy disk (Ctrl+G)").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    COPY_REGISTER_FILTER,
                    wide("Show candidate disks in register\tCtrl+Shift+K").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    COPY_PICKUP_PLAN,
                    wide("Copy all candidate disks pickup list\tCtrl+Shift+P").as_ptr(),
                );
                AppendMenuW(
                    view,
                    MF_STRING,
                    COPY_PAGE_MANIFEST,
                    wide("Copy CURRENT candidate page paths\tCtrl+Shift+M").as_ptr(),
                );
                AppendMenuW(menu, MF_POPUP, view as usize, wide("View").as_ptr());
            }
            SetMenu(hwnd, menu);
            DrawMenuBar(hwnd);
        }
        SetFocus((*app).search);
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            // Ctrl+F focuses global search from disk list, file results and inspector.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && message.wParam == b'F' as usize
            {
                SetFocus((*app).search);
                SendMessageW((*app).search, EM_SETSEL, 0, -1);
                continue;
            }
            // Ctrl+E moves to the currently SHA-verified result page; it never
            // selects rows remaining from a previous asynchronous search.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && message.wParam == b'E' as usize
            {
                (*app).command(FOCUS_RESULTS, 0);
                continue;
            }
            // Ctrl+R explicitly rechecks OS volume identities from anywhere in
            // the native shell. It never assumes a historical mount letter.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && message.wParam == b'R' as usize
            {
                (*app).command(PHYSICAL_CHECK, 0);
                continue;
            }
            // Ctrl+I collapses or restores the native disk locator.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && message.wParam == b'I' as usize
            {
                (*app).command(TOGGLE_LOCATOR, 0);
                continue;
            }
            // Dismiss only the current page response; the background source
            // read may still finish, but never restore outdated results.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && (message.wParam == b'X' as usize || message.wParam == b'Y' as usize)
            {
                (*app).command(
                    if message.wParam == b'X' as usize {
                        STOP_PAGE_WAIT
                    } else {
                        RETRY_PAGE
                    },
                    0,
                );
                continue;
            }
            // Ctrl+Shift+V toggles a catalogued-extension global filter, not film recognition.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && message.wParam == b'V' as usize
            {
                (*app).command(VIDEO_FILTER, 0);
                continue;
            }
            // Ctrl+Shift+C copies the selected catalog retrieval text locally.
            // Ctrl+C in the native inspector remains normal selected-text copy.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && message.wParam == b'C' as usize
            {
                (*app).command(COPY_LOCATOR, 0);
                continue;
            }
            // Ctrl+Shift+H copies ONLY the independently OS-verified mount
            // root of an explicitly paired catalog disk, never a file path.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && message.wParam == b'H' as usize
            {
                (*app).command(COPY_VERIFIED_OS_ROOT, 0);
                continue;
            }
            // Ctrl+Shift+P copies only local catalog metadata, never live
            // file bytes, and never proposes duplicate auto-deletion.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && message.wParam == b'P' as usize
            {
                (*app).command(COPY_PICKUP_PLAN, 0);
                continue;
            }
            // Ctrl+Shift+K filters the left disk register by EXACT copy IDs.
            // It never changes search scope or claims physical USB presence.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && message.wParam == b'K' as usize
            {
                (*app).command(COPY_REGISTER_FILTER, 0);
                continue;
            }
            // Ctrl+Shift+M copies only the verified loaded candidate page,
            // never paths from a prior async search scope.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && message.wParam == b'M' as usize
            {
                (*app).command(COPY_PAGE_MANIFEST, 0);
                continue;
            }
            // Ctrl+Shift+O/U perform a fresh OS scan and filter ONLY the
            // register to paired connected or not-verified catalog disk IDs.
            // Explicitly never alter the search query, disk scope, or pairing.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && (message.wParam == b'O' as usize || message.wParam == b'U' as usize)
            {
                (*app).command(
                    if message.wParam == b'O' as usize {
                        PHYSICAL_CONNECTED_FILTER
                    } else {
                        PHYSICAL_UNVERIFIED_FILTER
                    },
                    0,
                );
                continue;
            }
            // Ctrl+L filters the disk register, including private shelf tags,
            // without changing the catalog file-search scope.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && message.wParam == b'L' as usize
            {
                SetFocus((*app).disk_filter);
                SendMessageW((*app).disk_filter, EM_SETSEL, 0, -1);
                continue;
            }
            if message.message == WM_KEYDOWN
                && message.wParam == VK_ESCAPE as usize
                && GetFocus() == (*app).disk_filter
            {
                SetWindowTextW((*app).disk_filter, wide("").as_ptr());
                continue;
            }
            // Ctrl+J shows the selected file's exact catalog disk in the
            // view-only left register without modifying global search scope.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && message.wParam == b'J' as usize
            {
                (*app).command(REVEAL_DISK, 0);
                continue;
            }
            // Ctrl+G reprobes OS identities and focuses only one unambiguous
            // paired, currently mounted catalog-copy disk.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && message.wParam == b'G' as usize
            {
                (*app).command(COPY_CONNECTED, 0);
                continue;
            }
            // Ctrl+Shift+N: exact filename across all catalog disks, any recorded size.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && message.wParam == b'N' as usize
            {
                (*app).command(SAME_FILENAME, 0);
                continue;
            }
            // Ctrl+Shift+S: find renamed-file candidates by exact recorded bytes.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && GetKeyState(VK_SHIFT as i32) < 0
                && message.wParam == b'S' as usize
            {
                (*app).command(SAME_RECORDED_SIZE, 0);
                continue;
            }
            // Ctrl+D compares selected file metadata across catalogued disks.
            if message.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL as i32) < 0
                && message.wParam == b'D' as usize
            {
                (*app).command(FILE_COPIES, 0);
                continue;
            }
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
