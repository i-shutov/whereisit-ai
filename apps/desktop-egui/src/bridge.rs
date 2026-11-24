//! Nonblocking native UI bridge to the existing Rust/SQLite service.
//! Every call executes on a worker; main thread never opens CTF/SQLite.
use eframe::egui;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

static NEXT_SCAN_JOB: AtomicU64 = AtomicU64::new(1);

/// Unique only within the running process, which is the scope of JobGuard.
pub fn new_scan_job_id() -> String {
    let serial = NEXT_SCAN_JOB.fetch_add(1, Ordering::Relaxed);
    format!("egui-{}-{serial}", std::process::id())
}

/// Deliberately metadata-only. A scan never reads a source file's content.
pub fn scan_request(db: &str, root: &str, label: &str, job_id: &str) -> Value {
    json!({
        "op":"scan", "db":db, "root":root, "label":label, "job_id":job_id,
        "hash":false
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Search {
        query: String,
        volume_id: Option<i64>,
    },
    Folder {
        volume_id: i64,
        parent_id: Option<i64>,
    },
    BranchSearch {
        volume_id: i64,
        parent_id: i64,
        query: String,
        exact_category_id: Option<i64>,
    },
    Copies {
        entry_id: i64,
    },
}

impl Route {
    pub fn request(&self, db: &str, offset: u64, limit: u64) -> Value {
        match self {
            Self::Search { query, volume_id } => json!({
                "op":"search_fast", "db":db, "query":query,
                "volume_id":volume_id, "offset":offset, "limit":limit
            }),
            Self::Folder {
                volume_id,
                parent_id,
            } => json!({
                "op":"children", "db":db, "volume_id":volume_id,
                "parent_id":parent_id, "offset":offset, "limit":limit
            }),
            Self::BranchSearch {
                volume_id,
                parent_id,
                query,
                exact_category_id,
            } => json!({
                "op":"search_fast", "db":db,
                "query":branch_search_query(*parent_id, &exact_category_query(query, *exact_category_id)),
                "volume_id":volume_id, "offset":offset, "limit":limit
            }),
            Self::Copies { entry_id } => json!({
                "op":"copy_candidates", "db":db, "entry_id":entry_id,
                "other_disks_only":true, "offset":offset, "limit":limit
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Csv,
    Json,
    Parquet,
}

impl ExportKind {
    pub fn operation(self) -> &'static str {
        match self {
            Self::Csv => "export_csv",
            Self::Json => "export_json",
            Self::Parquet => "export_parquet",
        }
    }
}

impl Route {
    /// Exact current selection, never just the rendered page. The linked CTF
    /// adapter pins these same scope fields to its immutable source generation.
    pub fn export_request(&self, db: &str, path: &str, kind: ExportKind) -> Option<Value> {
        match self {
            Self::Search { query, volume_id } => Some(json!({
                "op":kind.operation(), "db":db, "path":path, "mode":"search",
                "query":query, "volume_id":volume_id, "overwrite":false
            })),
            Self::Folder {
                volume_id,
                parent_id,
            } => Some(json!({
                "op":kind.operation(), "db":db, "path":path, "mode":"children",
                "volume_id":volume_id, "parent_id":parent_id, "overwrite":false
            })),
            Self::BranchSearch {
                volume_id,
                parent_id,
                query,
                exact_category_id,
            } => Some(json!({
                "op":kind.operation(), "db":db, "path":path, "mode":"search",
                "query":branch_search_query(*parent_id, &exact_category_query(query, *exact_category_id)),
                "volume_id":volume_id, "overwrite":false
            })),
            Self::Copies { entry_id } => Some(json!({
                "op":kind.operation(), "db":db, "path":path, "mode":"copy_candidates",
                "entry_id":entry_id, "other_disks_only":true, "overwrite":false
            })),
        }
    }
}

fn branch_search_query(parent_id: i64, query: &str) -> String {
    if query.trim().is_empty() {
        format!("branch_id:{parent_id}")
    } else {
        format!("branch_id:{parent_id} AND ({})", query.trim())
    }
}

pub fn exact_category_query(query: &str, category_id: Option<i64>) -> String {
    let query = query.trim();
    match category_id.filter(|id| *id > 0) {
        Some(id) if query.is_empty() => format!("category_exact_id:{id}"),
        Some(id) => format!("category_exact_id:{id} AND ({query})"),
        None => query.to_owned(),
    }
}

pub fn extension_search_query(extension: &str) -> String {
    if extension.is_empty() {
        "ext:\"\"".to_owned()
    } else {
        format!("ext:{extension}")
    }
}

pub fn primary_category_search_query(row_id: &str) -> Option<String> {
    let id = row_id.strip_prefix("category:")?;
    if id == "uncategorized" {
        return Some("primary_category_id:0".to_owned());
    }
    let id = id.parse::<i64>().ok().filter(|id| *id > 0)?;
    Some(format!("primary_category_id:{id}"))
}

#[derive(Clone, Debug)]
pub enum Purpose {
    Open {
        path: String,
    },
    Create {
        path: String,
    },
    CheckCatalogIntegrity {
        path: String,
    },
    // Retained for the complete-sync regression harness.
    #[allow(dead_code)]
    LinkCtf {
        path: String,
        cache_root: String,
    },
    CtfPreviewOpen {
        path: String,
        cache_root: String,
    },
    CtfIndexBackground {
        path: String,
        cache_root: String,
        job_id: String,
    },
    CtfPreviewClose,
    ImportCtf {
        output: String,
    },
    ExportCtf {
        output: String,
    },
    SaveDescription {
        entry_id: i64,
        db: String,
    },
    LinkedRead {
        inner: Box<Purpose>,
        generation: String,
    },
    CtfProgress {
        job_id: String,
    },
    CtfCancel {
        job_id: String,
    },
    Volumes,
    DiskPreferences,
    SaveDiskProperties {
        volume_id: i64,
    },
    Page {
        route: Route,
    },
    Categories,
    EntryCategoryIds {
        entry_id: i64,
    },
    CreateCategory {
        name: String,
    },
    AssignCategory {
        entry_id: i64,
        category_id: i64,
    },
    UnassignCategory {
        entry_id: i64,
        category_id: i64,
    },
    Export {
        path: String,
    },
    Scan {
        root: String,
        job_id: String,
    },
    // Kept for the later bound-rescan UI flow; v1.0 does not construct it.
    #[allow(dead_code)]
    Rescan {
        db: String,
        volume_id: i64,
        job_id: String,
    },
    ScanProgress {
        job_id: String,
    },
    ScanCancel {
        job_id: String,
    },
    ShelfLabels {
        catalog_key: String,
        initial: bool,
    },
    ShelfSave {
        catalog_key: String,
        volume_id: i64,
    },
    Analytics {
        job_id: String,
        generation: u64,
        db: String,
        slices: bool,
    },
    AnalyticsCancel,
    LocalAnalyzer {
        job_id: String,
        generation: u64,
        scope_generation: u64,
        op: String,
    },
}

pub fn analytics_request(
    db: &str,
    job_id: &str,
    volume_id: Option<i64>,
    folder_entry_id: Option<i64>,
    group_by: &str,
    highlight_query: &str,
    category_id: Option<i64>,
) -> Value {
    json!({
        "op":"analytics", "db":db, "job_id":job_id,
        "volume_id":volume_id, "folder_entry_id":folder_entry_id,
        "group_by":group_by, "highlight_query":highlight_query,
        "category_id":category_id,
        "max_depth":4, "max_nodes":128
    })
}

pub fn analytics_cancel_request(job_id: &str) -> Value {
    json!({"op":"analytics_cancel", "job_id":job_id})
}

pub struct Reply {
    pub id: u64,
    pub purpose: Purpose,
    pub response: Value,
}

pub struct CommandBus {
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
    next_id: u64,
    #[cfg(test)]
    handler: std::sync::Arc<dyn Fn(&Value) -> Value + Send + Sync>,
}

impl CommandBus {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            next_id: 0,
            #[cfg(test)]
            handler: std::sync::Arc::new(dispatch),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_handler(
        &mut self,
        handler: impl Fn(&Value) -> Value + Send + Sync + 'static,
    ) {
        self.handler = std::sync::Arc::new(handler);
    }

    pub fn send(&mut self, purpose: Purpose, request: Value, ctx: &egui::Context) -> u64 {
        self.next_id = self.next_id.wrapping_add(1);
        let id = self.next_id;
        if let Purpose::Analytics { job_id, .. } = &purpose {
            // The backend prepare call only registers a process-local cancellation token. It
            // must complete before the query worker starts so cancel-before-first-row is safe.
            if let Err(error) = atlas_service::prepare_analytics_job(job_id) {
                let response = json!({"ok":false,"error":{"code":"analytics_prepare_failed","message":error.to_string()}});
                let _ = self.tx.send(Reply {
                    id,
                    purpose,
                    response,
                });
                ctx.request_repaint();
                return id;
            }
        }
        let tx = self.tx.clone();
        let repaint = ctx.clone();
        #[cfg(test)]
        let handler = self.handler.clone();
        thread::spawn(move || {
            #[cfg(test)]
            let response = handler(&request);
            #[cfg(not(test))]
            let response = dispatch(&request);
            let _ = tx.send(Reply {
                id,
                purpose,
                response,
            });
            repaint.request_repaint();
        });
        id
    }

    pub fn poll(&self) -> Option<Reply> {
        self.rx.try_recv().ok()
    }
}

fn dispatch(request: &Value) -> Value {
    let raw = atlas_service::dispatch_json(&request.to_string());
    serde_json::from_str(&raw).unwrap_or_else(|_| {
        json!({
            "ok":false, "error":{"code":"invalid_backend_response",
                "message":"Service returned invalid JSON"}
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_request_is_explicitly_metadata_only_and_job_ids_are_unique() {
        let first = new_scan_job_id();
        let second = new_scan_job_id();
        assert_ne!(first, second);
        assert!(!first.is_empty());
        let request = scan_request("catalog.sqlite", "/mnt/usb", "DVD Shelf", &first);
        assert_eq!(request["op"], "scan");
        assert_eq!(request["root"], "/mnt/usb");
        assert_eq!(request["label"], "DVD Shelf");
        assert_eq!(request["job_id"], first);
        assert_eq!(request["hash"], false);
        assert_eq!(request["db"], "catalog.sqlite");
    }

    #[test]
    fn copy_navigation_requests_other_disks_before_pagination() {
        let request = Route::Copies { entry_id: 42 }.request("catalog.sqlite", 250, 250);
        assert_eq!(request["other_disks_only"], true);
        assert_eq!(request["offset"], 250);
        assert_eq!(request["entry_id"], 42);
    }

    #[test]
    fn exports_respect_scope_and_never_overwrite() {
        let filtered = Route::Search {
            query: "ext:mkv AND size:>1GiB".into(),
            volume_id: Some(17),
        };
        let json = filtered
            .export_request("/catalog.sqlite", "/review.json", ExportKind::Json)
            .unwrap();
        assert_eq!(json["op"], "export_json");
        assert_eq!(json["mode"], "search");
        assert_eq!(json["volume_id"], 17);
        assert_eq!(json["query"], "ext:mkv AND size:>1GiB");
        assert_eq!(json["overwrite"], false);
        let folder = Route::Folder {
            volume_id: 17,
            parent_id: Some(204),
        };
        let csv = folder
            .export_request("/catalog.sqlite", "/branch.csv", ExportKind::Csv)
            .unwrap();
        assert_eq!(csv["mode"], "children");
        assert_eq!(csv["parent_id"], 204);
        assert_eq!(csv["op"], "export_csv");
        assert_eq!(ExportKind::Parquet.operation(), "export_parquet");
        let candidates = Route::Copies { entry_id: 22 }
            .export_request("/catalog.sqlite", "/copies.csv", ExportKind::Csv)
            .unwrap();
        assert_eq!(candidates["mode"], "copy_candidates");
        assert_eq!(candidates["entry_id"], 22);
        assert_eq!(candidates["other_disks_only"], true);
        assert_eq!(candidates["overwrite"], false);
        assert!(candidates.get("limit").is_none());
    }

    #[test]
    fn analytics_request_is_bounded_scoped_and_metadata_only() {
        let request = analytics_request(
            "catalog.sqlite",
            "egui-job",
            Some(7),
            Some(43),
            "categories",
            "ext:mkv",
            Some(9),
        );
        assert_eq!(request["op"], "analytics");
        assert_eq!(request["db"], "catalog.sqlite");
        assert_eq!(request["job_id"], "egui-job");
        assert_eq!(request["volume_id"], 7);
        assert_eq!(request["folder_entry_id"], 43);
        assert_eq!(request["group_by"], "categories");
        assert_eq!(request["highlight_query"], "ext:mkv");
        assert_eq!(request["category_id"], 9);
        assert_eq!(request["max_depth"], 4);
        assert_eq!(request["max_nodes"], 128);
        assert_eq!(
            analytics_cancel_request("egui-job")["op"],
            "analytics_cancel"
        );
    }

    #[test]
    fn branch_search_preserves_folder_predicate_in_pages_and_exports() {
        let route = Route::BranchSearch {
            volume_id: 4,
            parent_id: 81,
            query: "ext:mkv OR category:\"Movies\"".into(),
            exact_category_id: Some(12),
        };
        let page = route.request("catalog.sqlite", 0, 250);
        assert_eq!(page["op"], "search_fast");
        assert_eq!(page["volume_id"], 4);
        assert_eq!(
            page["query"],
            "branch_id:81 AND (category_exact_id:12 AND (ext:mkv OR category:\"Movies\"))"
        );
        let export = route
            .export_request("catalog.sqlite", "out.csv", ExportKind::Csv)
            .unwrap();
        assert_eq!(export["mode"], "search");
        assert_eq!(export["query"], page["query"]);
        assert_eq!(export["volume_id"], 4);
        assert_eq!(export["overwrite"], false);
    }

    #[test]
    fn category_scope_and_no_extension_queries_are_unambiguous() {
        assert_eq!(exact_category_query("", Some(17)), "category_exact_id:17");
        assert_eq!(
            exact_category_query("ext:mkv", Some(17)),
            "category_exact_id:17 AND (ext:mkv)"
        );
        assert_eq!(extension_search_query("mkv"), "ext:mkv");
        assert_eq!(extension_search_query(""), "ext:\"\"");
        assert_eq!(
            primary_category_search_query("category:42").as_deref(),
            Some("primary_category_id:42")
        );
        assert_eq!(
            primary_category_search_query("category:uncategorized").as_deref(),
            Some("primary_category_id:0")
        );
        assert_eq!(primary_category_search_query("category:bad"), None);
    }

    #[test]
    fn search_scope_is_explicit_and_copies_never_claim_byte_equality() {
        let search = Route::Search {
            query: "name:фильм".into(),
            volume_id: None,
        };
        assert!(search.request("/tmp/x.sqlite", 0, 250)["volume_id"].is_null());
        let copies = Route::Copies { entry_id: 42 };
        let r = copies.request("/tmp/x.sqlite", 100, 100);
        assert_eq!(r["op"], "copy_candidates");
        assert_eq!(r["entry_id"], 42);
        assert_eq!(r["offset"], 100);
    }

    #[test]
    fn real_service_scoped_export_preserves_data_and_refuses_clobber() {
        use std::fs;

        fn service(request: Value) -> Value {
            serde_json::from_str(&atlas_service::dispatch_json(&request.to_string())).unwrap()
        }

        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("sample-usb");
        let folder = media.join("Кино");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("Фильм.mkv"), b"local-fixture-only").unwrap();
        fs::write(media.join("notes.txt"), b"private fixture test").unwrap();
        let db = temp.path().join("catalog.sqlite");
        let scan = service(json!({"op":"scan","db":db,"root":media,"label":"Test USB"}));
        assert_eq!(scan["ok"], true, "scan: {scan}");
        let volume_id = scan["data"]["id"].as_i64().unwrap();

        let root = service(json!({
            "op":"children","db":db,"volume_id":volume_id,
            "parent_id":null,"offset":0,"limit":20
        }));
        assert_eq!(root["ok"], true, "root: {root}");
        let folder_id = root["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["kind"] == "directory")
            .unwrap()["id"]
            .as_i64()
            .unwrap();

        let db_text = db.to_str().unwrap();
        let json_path = temp.path().join("filtered.json");
        let search = Route::Search {
            query: "ext:mkv".to_owned(),
            volume_id: Some(volume_id),
        };
        let filtered = search
            .export_request(db_text, json_path.to_str().unwrap(), ExportKind::Json)
            .unwrap();
        assert_eq!(filtered["overwrite"], false);
        let completed = service(filtered.clone());
        assert_eq!(completed["ok"], true, "export json: {completed}");
        assert_eq!(completed["data"]["rows"], 1);
        let exported_json: Value = serde_json::from_slice(&fs::read(&json_path).unwrap()).unwrap();
        assert_eq!(exported_json["entries"].as_array().unwrap().len(), 1);
        assert_eq!(exported_json["entries"][0]["name"], "Фильм.mkv");
        let prior_bytes = fs::read(&json_path).unwrap();
        let repeated = service(filtered);
        assert_eq!(repeated["ok"], false);
        assert_eq!(repeated["error"]["code"], "file_exists");
        assert_eq!(fs::read(&json_path).unwrap(), prior_bytes);

        let csv_path = temp.path().join("branch.csv");
        let branch = Route::Folder {
            volume_id,
            parent_id: Some(folder_id),
        };
        let result = service(
            branch
                .export_request(db_text, csv_path.to_str().unwrap(), ExportKind::Csv)
                .unwrap(),
        );
        assert_eq!(result["ok"], true, "branch: {result}");
        assert_eq!(result["data"]["rows"], 2);
        let csv = fs::read_to_string(&csv_path).unwrap();
        assert!(csv.starts_with("id,volume_id,parent_id,name,"));
        assert!(csv.contains("Фильм.mkv"));
        assert!(!csv.contains("notes.txt"));

        let parquet_path = temp.path().join("complete.parquet");
        let all = Route::Search {
            query: String::new(),
            volume_id: None,
        };
        let result = service(
            all.export_request(db_text, parquet_path.to_str().unwrap(), ExportKind::Parquet)
                .unwrap(),
        );
        assert_eq!(result["ok"], true, "parquet: {result}");
        assert_eq!(result["data"]["rows"], 3);
        let bytes = fs::read(parquet_path).unwrap();
        assert!(bytes.starts_with(b"PAR1") && bytes.ends_with(b"PAR1"));
    }

    #[test]
    fn native_analytics_request_bounds_large_flat_folder_and_preserves_totals() {
        use std::fs;

        fn service(request: Value) -> Value {
            serde_json::from_str(&atlas_service::dispatch_json(&request.to_string())).unwrap()
        }

        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("generated-media");
        let flat = media.join("flat-folder");
        fs::create_dir_all(&flat).unwrap();
        let file_count: u64 = 300;
        for index in 1..=file_count {
            fs::write(
                flat.join(format!("item-{index:03}.dat")),
                vec![b'x'; index as usize],
            )
            .unwrap();
        }

        let db = temp.path().join("generated.sqlite");
        let scan = service(scan_request(
            db.to_str().unwrap(),
            media.to_str().unwrap(),
            "Generated analytics fixture",
            "native-analytics-bounds-scan",
        ));
        assert_eq!(scan["ok"], true, "scan: {scan}");
        let volume_id = scan["data"]["id"].as_i64().unwrap();
        let root = service(json!({
            "op":"children", "db":db, "volume_id":volume_id,
            "parent_id":null, "offset":0, "limit":20
        }));
        assert_eq!(root["ok"], true, "root children: {root}");
        let folder_id = root["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == "flat-folder")
            .expect("generated flat folder")["id"]
            .as_i64()
            .unwrap();

        let request = analytics_request(
            db.to_str().unwrap(),
            "native-analytics-bounds-job",
            Some(volume_id),
            Some(folder_id),
            "volumes",
            "",
            None,
        );
        assert_eq!(request["max_nodes"], 128);
        assert_eq!(
            service(json!({"op":"analytics_prepare", "job_id":request["job_id"]}))["ok"],
            true
        );
        let result = service(request);
        assert_eq!(result["ok"], true, "analytics: {result}");
        let data = &result["data"];
        let expected_bytes = file_count * (file_count + 1) / 2;
        let returned_nodes = data["root"]["children"].as_array().unwrap();
        let returned_bytes: u64 = returned_nodes
            .iter()
            .map(|node| node["size"].as_u64().unwrap())
            .sum();
        assert_eq!(returned_nodes.len(), 128);
        assert_eq!(data["totals"]["files"], file_count);
        assert_eq!(data["totals"]["indexed_bytes"], expected_bytes);
        assert_eq!(data["root"]["item_count"], file_count);
        assert_eq!(data["root"]["size"], expected_bytes);
        assert_eq!(data["truncated"], true);
        assert_eq!(data["omitted_node_count"], file_count - 128);
        assert_eq!(
            data["remainder_bytes"].as_u64().unwrap() + returned_bytes,
            expected_bytes
        );
    }
}
