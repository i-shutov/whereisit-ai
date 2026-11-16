//! Nonblocking native UI bridge to the existing Rust/SQLite service.
//! Every call executes on a worker; main thread never opens CTF/SQLite.
use eframe::egui;
use serde_json::{json, Value};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

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
            Self::Folder { volume_id, parent_id } => Some(json!({
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
    Page { route: Route },
    Export { path: String },
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
    fn exports_respect_scope_and_never_overwrite() {
        let filtered = Route::Search { query: "ext:mkv AND size:>1GiB".into(), volume_id: Some(17) };
        let json = filtered.export_request("/catalog.sqlite", "/review.json", ExportKind::Json).unwrap();
        assert_eq!(json["op"], "export_json");
        assert_eq!(json["mode"], "search");
        assert_eq!(json["volume_id"], 17);
        assert_eq!(json["query"], "ext:mkv AND size:>1GiB");
        assert_eq!(json["overwrite"], false);
        let folder = Route::Folder { volume_id: 17, parent_id: Some(204) };
        let csv = folder.export_request("/catalog.sqlite", "/branch.csv", ExportKind::Csv).unwrap();
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
}
