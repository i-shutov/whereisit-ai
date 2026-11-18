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
            Self::Copies { entry_id } => json!({
                "op":"copy_candidates", "db":db, "entry_id":entry_id,
                "offset":offset, "limit":limit
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
    /// Exact current selection, never just the rendered page. CTF operations are
    /// intentionally excluded until the real native linked-source contract is wired.
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
            Self::Copies { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Purpose {
    Open { path: String },
    Create { path: String },
    Volumes,
    DiskPreferences,
    SaveDiskProperties { volume_id: i64 },
    Page { route: Route },
    Export { path: String },
    Scan { root: String, job_id: String },
    ScanProgress { job_id: String },
    ScanCancel { job_id: String },
    ShelfLabels { catalog_key: String, initial: bool },
    ShelfSave { catalog_key: String, volume_id: i64 },
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
}

impl CommandBus {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx, next_id: 0 }
    }

    pub fn send(&mut self, purpose: Purpose, request: Value, ctx: &egui::Context) -> u64 {
        self.next_id = self.next_id.wrapping_add(1);
        let id = self.next_id;
        let tx = self.tx.clone();
        let repaint = ctx.clone();
        thread::spawn(move || {
            let raw = atlas_service::dispatch_json(&request.to_string());
            let response = serde_json::from_str(&raw).unwrap_or_else(|_| {
                json!({
                    "ok":false, "error":{"code":"invalid_backend_response",
                        "message":"Service returned invalid JSON"}
                })
            });
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
        assert!(Route::Copies { entry_id: 22 }
            .export_request("/catalog.sqlite", "/copies.csv", ExportKind::Csv)
            .is_none());
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
}
