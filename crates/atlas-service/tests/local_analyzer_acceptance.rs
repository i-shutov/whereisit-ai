use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static SERIAL: AtomicU64 = AtomicU64::new(1);
fn request(value: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&value.to_string())).unwrap()
}
fn ok(value: Value) -> Value {
    let response = request(value);
    assert_eq!(response["ok"], true, "{response}");
    response["data"].clone()
}
struct Session(String);
impl Session {
    fn start(root: &Path) -> Self {
        let id = format!(
            "acceptance-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        ok(json!({"op":"local_analyzer_prepare","job_id":id}));
        ok(json!({"op":"local_analyzer_start","job_id":id,"root":root}));
        let session = Self(id);
        let began = Instant::now();
        loop {
            let progress = ok(json!({"op":"local_analyzer_progress","job_id":session.0}));
            if progress["phase"] == "completed" {
                break;
            }
            assert_ne!(progress["phase"], "failed", "{progress}");
            assert!(began.elapsed() < Duration::from_secs(15), "{progress}");
            std::thread::sleep(Duration::from_millis(5));
        }
        session
    }
    fn page(&self, view: &str, extra: Value) -> Value {
        let mut v = json!({"op":"local_analyzer_page","job_id":self.0,"view":view});
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        ok(v)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        request(json!({"op":"local_analyzer_close","job_id":self.0}));
    }
}
fn file(root: &Path, path: &str, size: u64) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let handle = fs::File::create(path).unwrap();
    handle.set_len(size).unwrap();
    handle
        .set_times(
            fs::FileTimes::new()
                .set_modified(std::time::SystemTime::now() - Duration::from_secs(100 * 86400)),
        )
        .unwrap();
}
fn fixture(root: &Path) {
    for (path, size) in [
        ("a/deep/Film.mkv", 10),
        ("b/FILM.MKV", 10),
        ("c/Film.mkv", 11),
        ("Unicode/café.txt", 17),
        ("Unicode/nfd/cafe\u{301}.txt", 17),
        ("zero.txt", 0),
        ("Caches_Temp/demo.tmp", 2),
        ("Projects/node_modules/pkg.bin", 3),
    ] {
        file(root, path, size);
    }
    fs::create_dir(root.join("Empty")).unwrap();
}

#[test]
fn tree_rollups_empty_dirs_bounds_and_pages_count_files_once() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path());
    let session = Session::start(temp.path());
    let page = session.page("tree_map", json!({"max_nodes":3}));
    assert_eq!(page["totals"]["files"], 8);
    assert_eq!(page["totals"]["logical_bytes"], 70);
    assert_eq!(page["selected_totals"]["logical_bytes"], 70);
    assert_eq!(page["nodes"].as_array().unwrap().len(), 3);
    assert!(page["omitted_node_count"].as_i64().unwrap() > 0);
    assert!(page["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["name"] == "Empty" && n["bytes"] == 0));
    let a = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "a")
        .unwrap();
    assert_eq!(a["bytes"], 10);
    assert_eq!(a["file_count"], 1);
    let mut ids = Vec::new();
    for offset in [0, 2, 4, 6] {
        let page = session.page("largest", json!({"offset":offset,"limit":2}));
        assert_eq!(page["total"], 8);
        ids.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n["id"].as_str().unwrap().to_owned()),
        );
    }
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 8);
}

#[test]
fn filters_and_candidate_modes_are_exact_and_unicode_preserving() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path());
    let session = Session::start(temp.path());
    let candidates = session.page("metadata_candidates", json!({}));
    assert_eq!(candidates["total"], 4);
    let keys: std::collections::HashSet<_> = candidates["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["metadata_group_key"].to_string())
        .collect();
    assert_eq!(
        keys.len(),
        2,
        "NFC-equivalent names must share the native grouping key"
    );
    let names = session.page("metadata_candidates", json!({"criterion":"name"}));
    assert_eq!(names["total"], 3);
    assert!(names["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["metadata_group_count"] == 3));
    assert_eq!(
        session.page("metadata_candidates", json!({"criterion":"size"}))["total"],
        0
    );
    let filtered = session.page("tree_map", json!({"path_contains":"Unicode"}));
    assert_eq!(filtered["selected_totals"]["files"], 2);
    assert_eq!(filtered["selected_totals"]["logical_bytes"], 34);
    assert_eq!(session.page("largest", json!({"min_size":11}))["total"], 3);
    assert_eq!(
        session.page("largest", json!({"min_age_days":500}))["total"],
        0
    );
    assert_eq!(
        session.page("largest", json!({"min_age_days":50}))["total"],
        8
    );
    let bloat = session.page("bloat", json!({}));
    assert_eq!(bloat["total"], 2);
    let classes: Vec<_> = bloat["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["classification"].as_str().unwrap())
        .collect();
    assert!(classes.contains(&"dev"));
    assert!(classes.contains(&"cache"));
    let bad = request(
        json!({"op":"local_analyzer_page","job_id":session.0,"view":"largest","offset":-1}),
    );
    assert_eq!(bad["error"]["code"], "invalid_request");
}

#[test]
fn pre_epoch_modified_time_remains_old_metadata_instead_of_unknown() {
    let temp = tempfile::tempdir().unwrap();
    file(temp.path(), "old.bin", 1);
    fs::OpenOptions::new()
        .write(true)
        .open(temp.path().join("old.bin"))
        .unwrap()
        .set_times(
            fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH - Duration::from_secs(1)),
        )
        .unwrap();
    let session = Session::start(temp.path());
    let page = session.page("largest", json!({"min_age_days":50}));
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["modified_unix"], -1);
}

#[cfg(unix)]
#[test]
fn special_files_are_reported_without_opening_or_blocking_on_them() {
    let temp = tempfile::tempdir().unwrap();
    let _socket =
        std::os::unix::net::UnixListener::bind(temp.path().join("metadata.socket")).unwrap();
    let session = Session::start(temp.path());
    let page = session.page("tree_map", json!({}));
    assert_eq!(page["totals"]["files"], 0);
    assert_eq!(page["totals"]["complete"], false);
    assert_eq!(page["totals"]["read_errors"], 1);
    assert_eq!(page["issues"][0]["reason"], "unsupported_file_type");
    let baseline = request(json!({"op":"local_analyzer_save_baseline","job_id":session.0}));
    assert_eq!(baseline["error"]["code"], "incomplete");
}

#[test]
fn baseline_survives_rescan_and_rejects_a_different_root() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path());
    let session = Session::start(temp.path());
    let baseline = ok(json!({"op":"local_analyzer_save_baseline","job_id":session.0}));
    let baseline_id = baseline["baseline_id"].as_str().unwrap();
    assert_eq!(
        session.page("growth_diff", json!({"baseline_id":baseline_id}))["items"][0]["delta_bytes"],
        0
    );
    drop(session);
    file(temp.path(), "added.bin", 7);
    let new = Session::start(temp.path());
    let diff = new.page("growth_diff", json!({"baseline_id":baseline_id}));
    let root = diff["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["relative_path"] == "")
        .unwrap();
    assert_eq!(root["previous_bytes"], 70);
    assert_eq!(root["bytes"], 77);
    assert_eq!(root["delta_bytes"], 7);
    let other = tempfile::tempdir().unwrap();
    file(other.path(), "other.bin", 20);
    let other = Session::start(other.path());
    let mismatch = request(
        json!({"op":"local_analyzer_page","job_id":other.0,"view":"growth_diff","baseline_id":baseline_id}),
    );
    assert_eq!(mismatch["error"]["code"], "baseline_scope");
    ok(json!({"op":"local_analyzer_clear_baseline","baseline_id":baseline_id}));
}

#[test]
fn baseline_rejects_a_replacement_directory_at_the_identical_path() {
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("source");
    fs::create_dir(&root).unwrap();
    file(&root, "before.bin", 3);
    let session = Session::start(&root);
    let baseline = ok(json!({"op":"local_analyzer_save_baseline","job_id":session.0}));
    let baseline_id = baseline["baseline_id"].as_str().unwrap();
    drop(session);
    fs::rename(&root, outer.path().join("moved-source")).unwrap();
    fs::create_dir(&root).unwrap();
    file(&root, "foreign.bin", 20);
    let replacement = Session::start(&root);
    let result = request(
        json!({"op":"local_analyzer_page","job_id":replacement.0,"view":"growth_diff","baseline_id":baseline_id}),
    );
    assert_eq!(result["error"]["code"], "baseline_scope");
    ok(json!({"op":"local_analyzer_clear_baseline","baseline_id":baseline_id}));
}

#[test]
fn review_export_deduplicates_nested_selection_and_never_overwrites() {
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("source");
    fs::create_dir(&root).unwrap();
    fixture(&root);
    file(&root, "  =1+1.txt", 4);
    file(&root, "comma,東京.txt", 6);
    let session = Session::start(&root);
    let tree = session.page("tree_map", json!({}));
    let a = tree["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "a")
        .unwrap()["id"]
        .clone();
    let all = session.page("largest", json!({}));
    let ids: Vec<_> = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| {
            ["a/deep/Film.mkv", "  =1+1.txt", "comma,東京.txt"]
                .contains(&n["relative_path"].as_str().unwrap())
        })
        .map(|n| n["id"].clone())
        .collect();
    let mut selected = vec![a];
    selected.extend(ids);
    let out = outer.path().join("review.csv");
    let export = ok(
        json!({"op":"local_analyzer_export_review_csv","job_id":session.0,"path":out,"selected_ids":selected}),
    );
    assert_eq!(export["rows"], 3);
    let mut reader = csv::Reader::from_path(&out).unwrap();
    let rows: Vec<_> = reader.records().map(|r| r.unwrap()).collect();
    assert!(rows.iter().any(|r| r.get(1) == Some("'  =1+1.txt")));
    assert!(rows.iter().any(|r| r.get(1) == Some("comma,東京.txt")));
    let before = fs::read(&out).unwrap();
    let overwrite = request(
        json!({"op":"local_analyzer_export_review_csv","job_id":session.0,"path":out,"selected_ids":selected}),
    );
    assert_eq!(overwrite["error"]["code"], "file_exists");
    assert_eq!(fs::read(&out).unwrap(), before);
    let inside = request(
        json!({"op":"local_analyzer_export_review_csv","job_id":session.0,"path":root.join("new.csv"),"selected_ids":selected}),
    );
    assert_eq!(inside["error"]["code"], "unsafe_output");
    assert!(!root.join("new.csv").exists());
    let invalid = request(
        json!({"op":"local_analyzer_export_review_csv","job_id":session.0,"path":outer.path().join("bad.csv"),"selected_ids":["file:999999"]}),
    );
    assert_eq!(invalid["error"]["code"], "not_found");
    assert!(!outer.path().join("bad.csv").exists());
    assert_eq!(
        fs::metadata(root.join("a/deep/Film.mkv")).unwrap().len(),
        10
    );
}
