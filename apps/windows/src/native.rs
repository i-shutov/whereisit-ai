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
        if let Some(mut linked) = self.linked_request(if !self.query.is_empty() {
            "ctf_search_pinned"
        } else {
            "ctf_children_pinned"
        }) {
            self.duplicates = false;
            linked["offset"] = json!(self.offset);
            linked["limit"] = json!(LIMIT);
            linked["volume_id"] = json!(self.volume);
            if !self.query.is_empty() {
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
            if reply.tag == "rows" && reply.generation != self.generation {
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
                "linked_sync" => {
                    let path = self.pending_link_path.take();
                    let generation = data["generation"].as_str().map(str::to_owned);
                    if let (Some(path), Some(generation)) = (path, generation) {
                        self.linked_ctf = Some(path);
                        self.linked_generation = Some(generation);
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
                } else if [OPEN, NEW, SCAN, IMPORT, LINK_CTF, EXPORT].contains(command) {
                    (self.job.is_none()
                        && self.pending == 0
                        && (!self.linked_mode()
                            || [OPEN, NEW, LINK_CTF].contains(command)
                            || (*command == EXPORT
                                && SendMessageW(self.export_format, CB_GETCURSEL, 0, 0) != 2)))
                        as i32
                } else if [
                    CATEGORY_FILTER,
                    CATEGORY_ASSIGN,
                    CATEGORY_CREATE,
                    DUPLICATES,
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
                if command == DUPLICATES && self.linked_mode() {
                    self.status(
                        "Duplicate candidates in a linked CTF require a separate approved mode.",
                    );
                    return;
                }
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
            OPEN | NEW | SCAN | IMPORT | LINK_CTF | EXPORT
                if self.job.is_none() && self.pending == 0 =>
            {
                if self.linked_mode() && [SCAN, IMPORT].contains(&command) {
                    self.status("Linked CTF is read-only; open a writable SQLite catalog first.");
                    return;
                }
                if self.linked_mode()
                    && command == EXPORT
                    && SendMessageW(self.export_format, CB_GETCURSEL, 0, 0) == 2
                {
                    self.status(
                        "Linked CTF supports only CSV/Parquet export; original stays read-only.",
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
                                linked["mode"] =
                                    json!(if self.query.is_empty() && self.volume.is_some() {
                                        "children"
                                    } else {
                                        "search"
                                    });
                                linked["query"] = json!(self.query);
                                linked["volume_id"] = json!(self.volume);
                                linked["parent_id"] = json!(self.stack.last().and_then(|p| p.0));
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
    let filter=wide(match format {"ctf"=>"CTF catalogs (*.ctf)\0*.ctf\0All files\0*.*\0","csv"=>"CSV table (*.csv)\0*.csv\0All files\0*.*\0","parquet"=>"Apache Parquet (*.parquet)\0*.parquet\0All files\0*.*\0",_=>"Atlas SQLite catalog (*.atlas.sqlite;*.sqlite)\0*.atlas.sqlite;*.sqlite\0All files\0*.*\0"});
    let ext = wide(if format == "sqlite" {
        "atlas.sqlite"
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
            MoveWindow(help, 8, 62, 1000, 18, 1);
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
