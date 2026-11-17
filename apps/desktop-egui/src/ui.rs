//! PR58-first native workbench. Every user-visible catalog fact comes from atlas-service;
//! no fictitious USB mount badges, byte-duplicate claims or on-disk delete operations.
use crate::bridge::{new_scan_job_id, scan_request, CommandBus, ExportKind, Purpose, Route};
use crate::model::{bytes_compact, parse_volumes, DiskPreferences, Entry, Page, Volume};
use eframe::egui::{self, Color32, RichText};
use egui_extras::{Column, TableBuilder};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

const PAGE_LIMIT: u64 = 250;
const SEARCH_INPUT_ID: &str = "global_search_input";
const DISK_NAME_INPUT_ID: &str = "disk_local_name_input";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    Russian,
    English,
}

#[derive(Clone, Debug, Default)]
struct DiskPropertiesDraft {
    values: DiskPreferences,
    original_shelf: String,
    capacity: String,
}

fn user_store() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("WHEREISIT_USER_STORE") {
        let path = std::path::PathBuf::from(path);
        return path.is_absolute().then_some(path);
    }
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|p| p.join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root: Option<std::path::PathBuf> = None;
    root.filter(|p| p.is_absolute()).map(|p| {
        p.join("AtlasCatalog")
            .join("physical-volume-bindings.sqlite")
    })
}

fn preference_platform() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        Some("windows")
    } else if cfg!(target_os = "macos") {
        Some("macos")
    } else {
        None
    }
}
impl Language {
    fn text(self, ru: &'static str, en: &'static str) -> &'static str {
        if self == Self::Russian {
            ru
        } else {
            en
        }
    }
}

pub struct DesktopApp {
    bus: CommandBus,
    active_request: Option<u64>,
    db: Option<String>,
    path_input: String,
    query_input: String,
    volumes: Vec<Volume>,
    route: Route,
    history: Vec<(Route, u64)>,
    page: Page,
    offset: u64,
    selected: Option<Entry>,
    selected_volume: Option<i64>,
    disk_filter: String,
    show_catalog_dialog: bool,
    create_catalog: bool,
    show_export_dialog: bool,
    show_scan_dialog: bool,
    scan_root: String,
    scan_label: String,
    scan_job_id: Option<String>,
    scan_progress_request: Option<u64>,
    scan_cancel_request: Option<u64>,
    scan_registered: bool,
    scan_cancel_requested: bool,
    scan_progress_entries: u64,
    last_progress_poll: Option<Instant>,
    export_path: String,
    export_kind: ExportKind,
    show_disk_properties: Option<i64>,
    focus_disk_name: bool,
    disk_draft: DiskPropertiesDraft,
    disk_preferences: HashMap<i64, DiskPreferences>,
    shelf_collisions: HashSet<i64>,
    preferences_ready: bool,
    status: String,
    error: Option<String>,
    lang: Language,
    dark: bool,
    auto_open: bool,
}

impl DesktopApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup_catalog: Option<String>) -> Self {
        Self::with_context(&cc.egui_ctx, startup_catalog)
    }

    fn with_context(ctx: &egui::Context, startup_catalog: Option<String>) -> Self {
        ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = egui::vec2(6.0, 5.0);
            style.spacing.button_padding = egui::vec2(7.0, 4.0);
        });
        Self {
            bus: CommandBus::new(),
            active_request: None,
            db: None,
            path_input: startup_catalog.clone().unwrap_or_default(),
            query_input: String::new(),
            volumes: Vec::new(),
            route: Route::Search {
                query: String::new(),
                volume_id: None,
            },
            history: Vec::new(),
            page: Page::default(),
            offset: 0,
            selected: None,
            selected_volume: None,
            disk_filter: String::new(),
            show_catalog_dialog: false,
            create_catalog: false,
            show_export_dialog: false,
            show_scan_dialog: false,
            scan_root: String::new(),
            scan_label: String::new(),
            scan_job_id: None,
            scan_progress_request: None,
            scan_cancel_request: None,
            scan_registered: false,
            scan_cancel_requested: false,
            scan_progress_entries: 0,
            last_progress_poll: None,
            export_path: String::new(),
            export_kind: ExportKind::Csv,
            show_disk_properties: None,
            focus_disk_name: false,
            disk_draft: DiskPropertiesDraft::default(),
            disk_preferences: HashMap::new(),
            shelf_collisions: HashSet::new(),
            preferences_ready: false,
            status: "Ready · local metadata only".to_owned(),
            error: None,
            lang: Language::Russian,
            dark: true,
            auto_open: startup_catalog.is_some(),
        }
    }

    fn t(&self, ru: &'static str, en: &'static str) -> &'static str {
        self.lang.text(ru, en)
    }

    fn queue(&mut self, purpose: Purpose, request: Value, ctx: &egui::Context) {
        // Do not supersede an I/O worker: a stale write could still complete
        // even if its reply were discarded. Navigation remains disabled while busy.
        if self.active_request.is_some() {
            self.error = Some("Previous catalog operation is still running.".to_owned());
            return;
        }
        self.error = None;
        self.active_request = Some(self.bus.send(purpose, request, ctx));
    }

    fn start_catalog(&mut self, create: bool, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        let path = self.path_input.trim().to_owned();
        if path.is_empty() {
            self.error = Some(
                self.t(
                    "Введите путь к SQLite-каталогу.",
                    "Enter a SQLite catalog path.",
                )
                .to_owned(),
            );
            return;
        }
        if path.to_ascii_lowercase().ends_with(".ctf") {
            self.error = Some(self.t(
                "Пока поддерживается SQLite. CTF откройте в существующем нативном клиенте; интеграция последует после каркаса.",
                "SQLite is supported now. Open CTF in the existing native client; CTF integration follows the desktop foundation."
            ).to_owned());
            return;
        }
        let exists = Path::new(&path).exists();
        if create && exists {
            self.error = Some(
                self.t(
                    "Файл уже существует: создание отменено.",
                    "File exists: create refused.",
                )
                .to_owned(),
            );
            return;
        }
        if !create && !Path::new(&path).is_file() {
            self.error = Some(
                self.t("Файл каталога не найден.", "Catalog file not found.")
                    .to_owned(),
            );
            return;
        }
        let path = if Path::new(&path).is_absolute() {
            path
        } else {
            match std::env::current_dir() {
                Ok(current) => current.join(path).to_string_lossy().into_owned(),
                Err(e) => {
                    self.error = Some(e.to_string());
                    return;
                }
            }
        };
        let purpose = if create {
            Purpose::Create { path: path.clone() }
        } else {
            Purpose::Open { path: path.clone() }
        };
        let op = if create { "init" } else { "validate_catalog" };
        self.status = self.t("Проверка каталога…", "Opening catalog…").to_owned();
        self.queue(purpose, json!({"op":op,"db":path}), ctx);
        self.show_catalog_dialog = false;
    }

    fn start_export(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        let Some(db) = self.db.clone() else {
            return;
        };
        self.status = self.t("Операция отклонена", "Operation refused").to_owned();
        let path = self.export_path.trim().to_owned();
        if path.is_empty() {
            self.error = Some(
                self.t(
                    "Введите путь для нового файла экспорта.",
                    "Enter a new export file path.",
                )
                .to_owned(),
            );
            return;
        }
        let file = Path::new(&path);
        let parent = file
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if !parent.is_dir() || file.exists() || file.is_symlink() {
            self.error = Some(
                self.t(
                    "Нужна существующая папка и НОВОЕ имя файла: перезапись запрещена.",
                    "Choose an existing folder and a NEW filename: overwrite is forbidden.",
                )
                .to_owned(),
            );
            return;
        }
        let Some(request) = self.route.export_request(&db, &path, self.export_kind) else {
            self.error = Some(
                self.t(
                    "Экспорт копий из этого экрана не поддержан: вернитесь к поиску или папке.",
                    "Copy-candidate export is unavailable here: return to search or folder.",
                )
                .to_owned(),
            );
            return;
        };
        self.status = self
            .t(
                "Экспорт полного набора метаданных…",
                "Exporting full metadata scope…",
            )
            .to_owned();
        self.show_export_dialog = false;
        self.queue(Purpose::Export { path }, request, ctx);
    }

    fn start_scan(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        let Some(db) = self.db.clone() else {
            return;
        };
        let label = self.scan_label.trim().to_owned();
        if label.is_empty() {
            self.error = Some(
                self.t("Укажите название диска.", "Enter a disk label.")
                    .to_owned(),
            );
            return;
        }
        let root = Path::new(self.scan_root.trim());
        let valid_root = std::fs::symlink_metadata(root)
            .map(|m| m.file_type().is_dir())
            .unwrap_or(false);
        if !valid_root {
            self.error = Some(
                self.t(
                    "Нужна существующая папка, не символическая ссылка.",
                    "Choose an existing folder, not a symbolic link.",
                )
                .to_owned(),
            );
            return;
        }
        let Ok(root_abs) = std::fs::canonicalize(root) else {
            self.error = Some(
                self.t("Нет доступа к папке.", "Folder is inaccessible.")
                    .to_owned(),
            );
            return;
        };
        let Ok(db_abs) = std::fs::canonicalize(&db) else {
            self.error = Some(
                self.t("Каталог не найден.", "Catalog unavailable.")
                    .to_owned(),
            );
            return;
        };
        // A catalog under its own scan root would re-index its WAL and itself.
        if db_abs.starts_with(&root_abs) {
            self.error = Some(
                self.t(
                    "Каталог SQLite расположен внутри сканируемой папки. Выберите другой источник.",
                    "The SQLite catalog is inside the scan root. Choose a different source.",
                )
                .to_owned(),
            );
            return;
        }
        let root_text = root_abs.to_string_lossy().into_owned();
        let job_id = new_scan_job_id();
        self.scan_registered = false;
        self.scan_cancel_requested = false;
        self.scan_progress_entries = 0;
        self.scan_progress_request = None;
        self.scan_cancel_request = None;
        self.last_progress_poll = None;
        self.scan_job_id = Some(job_id.clone());
        self.status = self
            .t(
                "Сканирование метаданных запущено…",
                "Metadata scan started…",
            )
            .to_owned();
        self.show_scan_dialog = false;
        self.queue(
            Purpose::Scan {
                root: root_text.clone(),
                job_id: job_id.clone(),
            },
            scan_request(&db, &root_text, &label, &job_id),
            ctx,
        );
    }

    fn poll_scan_progress(&mut self, ctx: &egui::Context) {
        let Some(job_id) = self.scan_job_id.clone() else {
            return;
        };
        ctx.request_repaint_after(Duration::from_millis(500));
        let should_poll = self.scan_progress_request.is_none()
            && self
                .last_progress_poll
                .is_none_or(|last| last.elapsed() >= Duration::from_millis(500));
        if should_poll {
            self.last_progress_poll = Some(Instant::now());
            self.scan_progress_request = Some(self.bus.send(
                Purpose::ScanProgress {
                    job_id: job_id.clone(),
                },
                json!({"op":"progress","job_id":job_id}),
                ctx,
            ));
        }
    }

    fn request_scan_cancel(&mut self, ctx: &egui::Context) {
        if self.scan_cancel_requested || !self.scan_registered {
            return;
        }
        let Some(job_id) = self.scan_job_id.clone() else {
            return;
        };
        self.scan_cancel_requested = true;
        self.status = self
            .t(
                "Запрос отмены сканирования…",
                "Requesting scan cancellation…",
            )
            .to_owned();
        self.scan_cancel_request = Some(self.bus.send(
            Purpose::ScanCancel {
                job_id: job_id.clone(),
            },
            json!({"op":"cancel","job_id":job_id}),
            ctx,
        ));
    }

    fn load_volumes(&mut self, ctx: &egui::Context) {
        if let Some(db) = self.db.clone() {
            self.status = self
                .t("Загрузка реестра дисков…", "Loading disk register…")
                .to_owned();
            self.queue(Purpose::Volumes, json!({"op":"volumes","db":db}), ctx);
        }
    }

    fn load_disk_preferences(&mut self, ctx: &egui::Context) {
        self.preferences_ready = false;
        let (Some(db), Some(store), Some(platform)) =
            (self.db.clone(), user_store(), preference_platform())
        else {
            let error = Some(
                self.t(
                    "Локальные свойства дисков недоступны.",
                    "Local disk properties are unavailable.",
                )
                .to_owned(),
            );
            self.load_page(ctx);
            self.error = error;
            return;
        };
        self.queue(
            Purpose::DiskPreferences,
            json!({"op":"disk_preferences_list","db":db,
            "user_store":store.to_string_lossy(),"platform":platform}),
            ctx,
        );
    }

    fn display_name<'a>(&'a self, volume: &'a Volume) -> &'a str {
        self.disk_preferences
            .get(&volume.id)
            .map(|p| p.display_name.as_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(&volume.label)
    }

    fn open_disk_properties(&mut self, id: i64) {
        let Some(volume) = self.volumes.iter().find(|v| v.id == id) else {
            return;
        };
        let mut values = self.disk_preferences.get(&id).cloned().unwrap_or_default();
        if values.display_name.is_empty() {
            values.display_name = volume.label.clone();
        }
        self.disk_draft = DiskPropertiesDraft {
            original_shelf: values.shelf_label.clone(),
            capacity: values
                .capacity_bytes
                .map(|n| n.to_string())
                .unwrap_or_default(),
            values,
        };
        self.error = None;
        self.show_disk_properties = Some(id);
        self.focus_disk_name = true;
    }

    fn save_disk_properties(&mut self, ctx: &egui::Context) {
        if !self.preferences_ready || self.active_request.is_some() {
            return;
        }
        let (Some(id), Some(db), Some(store), Some(platform)) = (
            self.show_disk_properties,
            self.db.clone(),
            user_store(),
            preference_platform(),
        ) else {
            return;
        };
        let capacity = if self.disk_draft.capacity.trim().is_empty() {
            None
        } else {
            match self.disk_draft.capacity.trim().parse::<i64>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    self.error = Some(
                        self.t(
                            "Ёмкость должна быть положительным числом байт.",
                            "Capacity must be a positive byte count.",
                        )
                        .to_owned(),
                    );
                    return;
                }
            }
        };
        let values = &self.disk_draft.values;
        self.queue(Purpose::SaveDiskProperties { volume_id: id }, json!({
            "op":"disk_preferences_set", "db":db, "user_store":store.to_string_lossy(),
            "platform":platform, "volume_id":id, "display_name":values.display_name,
            "shelf_label":values.shelf_label, "model":values.model, "filesystem":values.filesystem,
            "capacity_bytes":capacity, "note":values.note, "expected_revision":values.revision,
            "expected_shelf_label":self.disk_draft.original_shelf, "confirmed":true
        }), ctx);
    }

    fn load_page(&mut self, ctx: &egui::Context) {
        if let Some(db) = self.db.clone() {
            let route = self.route.clone();
            let request = route.request(&db, self.offset, PAGE_LIMIT);
            self.status = self
                .t("Поиск по каталогу…", "Searching catalog…")
                .to_owned();
            self.queue(Purpose::Page { route }, request, ctx);
        }
    }

    fn navigate_search(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        self.history.clear();
        self.offset = 0;
        self.selected = None;
        self.route = Route::Search {
            query: self.query_input.trim().to_owned(),
            volume_id: self.selected_volume,
        };
        self.load_page(ctx);
    }

    fn navigate_disk(&mut self, volume_id: Option<i64>, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        self.selected_volume = volume_id;
        self.navigate_search(ctx);
    }

    fn navigate_folder(&mut self, entry: &Entry, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        if !entry.is_directory() {
            return;
        }
        self.history.push((self.route.clone(), self.offset));
        self.offset = 0;
        self.selected = None;
        self.route = Route::Folder {
            volume_id: entry.volume_id,
            parent_id: Some(entry.id),
        };
        self.load_page(ctx);
    }

    fn navigate_copies(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        if let Some(entry) = self.selected.clone() {
            if entry.is_directory() {
                return;
            }
            self.history.push((self.route.clone(), self.offset));
            self.offset = 0;
            self.selected = None;
            self.route = Route::Copies { entry_id: entry.id };
            self.load_page(ctx);
        }
    }

    fn go_back(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        if let Some((route, offset)) = self.history.pop() {
            self.route = route;
            self.offset = offset;
            self.selected = None;
            self.load_page(ctx);
        }
    }

    fn apply_reply(&mut self, reply: crate::bridge::Reply, ctx: &egui::Context) {
        // Progress and cancel use independent short workers. They must NEVER
        // supersede the active scan worker or its transactional response.
        match &reply.purpose {
            Purpose::ScanProgress { job_id } => {
                if self.scan_progress_request == Some(reply.id) {
                    self.scan_progress_request = None;
                    if self.scan_job_id.as_deref() == Some(job_id.as_str())
                        && reply.response["ok"] == true
                    {
                        self.scan_registered = true;
                        self.scan_progress_entries = reply.response["data"]["entries"]
                            .as_u64()
                            .unwrap_or(self.scan_progress_entries);
                    }
                }
                return;
            }
            Purpose::ScanCancel { job_id } => {
                if self.scan_cancel_request == Some(reply.id) {
                    self.scan_cancel_request = None;
                    if self.scan_job_id.as_deref() == Some(job_id.as_str()) {
                        self.status = if reply.response["ok"] == true {
                            self.t(
                                "Отмена запрошена; ожидается откат транзакции.",
                                "Cancellation requested; waiting for rollback.",
                            )
                        } else {
                            self.t(
                                "Отмена уже невозможна: операция завершается.",
                                "Cancellation is too late; operation is finishing.",
                            )
                        }
                        .to_owned();
                    }
                }
                return;
            }
            _ => {}
        }
        // A long I/O worker is never replaced while it is active.
        if self.active_request != Some(reply.id) {
            return;
        }
        self.active_request = None;
        let scan_finished = matches!(&reply.purpose, Purpose::Scan { .. });
        if scan_finished {
            self.scan_job_id = None;
            self.scan_progress_request = None;
            self.scan_cancel_request = None;
            self.scan_registered = false;
            self.scan_cancel_requested = false;
        }
        if scan_finished
            && reply
                .response
                .pointer("/error/code")
                .and_then(Value::as_str)
                == Some("cancelled")
        {
            self.status = self
                .t(
                    "Сканирование отменено: снимок не сохранён.",
                    "Scan cancelled: no snapshot was saved.",
                )
                .to_owned();
            return;
        }
        if reply.response.get("ok").and_then(Value::as_bool) != Some(true) {
            let message = reply
                .response
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Unknown service error");
            let error_message = message.to_owned();
            self.status = self.t("Операция отклонена", "Operation refused").to_owned();
            if matches!(reply.purpose, Purpose::DiskPreferences) {
                self.preferences_ready = false;
                self.disk_preferences.clear();
                self.shelf_collisions.clear();
                self.load_page(ctx);
            }
            self.error = Some(error_message);
            return;
        }
        let data = &reply.response["data"];
        match reply.purpose {
            Purpose::Open { path } | Purpose::Create { path } => {
                self.db = Some(path);
                self.disk_preferences.clear();
                self.shelf_collisions.clear();
                self.preferences_ready = false;
                self.selected_volume = None;
                self.query_input.clear();
                self.route = Route::Search {
                    query: String::new(),
                    volume_id: None,
                };
                self.history.clear();
                self.offset = 0;
                self.selected = None;
                self.load_volumes(ctx);
            }
            Purpose::Volumes => match parse_volumes(data) {
                Ok(volumes) => {
                    self.volumes = volumes;
                    self.status = self.t("Каталог открыт", "Catalog opened").to_owned();
                    self.load_disk_preferences(ctx);
                }
                Err(e) => self.error = Some(e),
            },
            Purpose::DiskPreferences => {
                let parsed = data
                    .get("items")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "Missing disk preferences".to_owned())
                    .and_then(|items| {
                        items
                            .iter()
                            .map(DiskPreferences::parse)
                            .collect::<Result<HashMap<_, _>, _>>()
                    });
                let error = match parsed {
                    Ok(preferences) => {
                        self.disk_preferences = preferences;
                        self.shelf_collisions = data
                            .get("collisions")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .flat_map(|c| {
                                c["volume_ids"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(Value::as_i64)
                            })
                            .collect();
                        self.preferences_ready = true;
                        None
                    }
                    Err(e) => {
                        self.preferences_ready = false;
                        Some(e)
                    }
                };
                self.load_page(ctx);
                if error.is_some() {
                    self.error = error;
                }
            }
            Purpose::SaveDiskProperties { volume_id } => {
                let _ = volume_id;
                self.show_disk_properties = None;
                self.status = self
                    .t("Локальные свойства сохранены", "Local properties saved")
                    .to_owned();
                self.load_volumes(ctx);
            }
            Purpose::Scan { root, job_id } => {
                let _ = (root, job_id);
                if let Some(id) = data["id"].as_i64() {
                    self.selected_volume = Some(id);
                    self.query_input.clear();
                    self.history.clear();
                    self.offset = 0;
                    self.selected = None;
                    self.route = Route::Search {
                        query: String::new(),
                        volume_id: Some(id),
                    };
                }
                self.status = self
                    .t(
                        "Сканирование закончено; обновляем реестр.",
                        "Scan complete; refreshing disk register.",
                    )
                    .to_owned();
                self.load_volumes(ctx);
            }
            Purpose::ScanProgress { .. } | Purpose::ScanCancel { .. } => {
                unreachable!("Auxiliary scan reply must be processed before active worker");
            }
            Purpose::Export { path } => {
                let rows = data.get("rows").and_then(Value::as_u64).unwrap_or(0);
                self.status = format!(
                    "{}: {} ({} {})",
                    self.t("Экспорт завершён", "Export complete"),
                    path,
                    rows,
                    self.t("записей", "rows")
                );
            }
            Purpose::Page { route } => match Page::parse(data) {
                Ok(page) => {
                    self.route = route;
                    self.offset = page.offset;
                    self.status = format!(
                        "{} {}–{}",
                        self.t("Показаны строки", "Showing rows"),
                        page.offset
                            .saturating_add(if page.items.is_empty() { 0 } else { 1 }),
                        page.offset.saturating_add(page.items.len() as u64),
                    );
                    self.page = page;
                    self.selected = None;
                }
                Err(e) => self.error = Some(e),
            },
        }
    }

    fn menu(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::top("menu").show(root, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.strong("WhereIsIt AI");
                ui.separator();
                ui.menu_button(self.t("Каталог", "Catalog"), |ui| {
                    if ui
                        .button(self.t("Открыть SQLite…", "Open SQLite…"))
                        .clicked()
                    {
                        self.show_catalog_dialog = true;
                        self.create_catalog = false;
                        ui.close();
                    }
                    if ui
                        .button(self.t("Новый каталог…", "New catalog…"))
                        .clicked()
                    {
                        self.show_catalog_dialog = true;
                        self.create_catalog = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.db.is_some() && self.active_request.is_none(),
                            egui::Button::new(
                                self.t("Сканировать диск/папку…", "Scan disk/folder…"),
                            ),
                        )
                        .clicked()
                    {
                        self.show_scan_dialog = true;
                        self.scan_root.clear();
                        self.scan_label.clear();
                        ui.close();
                    }
                    let can_export = self.db.is_some()
                        && self.active_request.is_none()
                        && !matches!(self.route, Route::Copies { .. });
                    if ui
                        .add_enabled(
                            can_export,
                            egui::Button::new(
                                self.t("Экспорт текущей выборки…", "Export current scope…"),
                            ),
                        )
                        .clicked()
                    {
                        self.export_path.clear();
                        self.error = None;
                        self.show_export_dialog = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.db.is_some(),
                            egui::Button::new(self.t("Обновить", "Refresh")),
                        )
                        .clicked()
                    {
                        self.load_volumes(ctx);
                        ui.close();
                    }
                });
                ui.menu_button(self.t("Вид", "View"), |ui| {
                    if ui.button(self.t("Все диски", "All disks")).clicked() {
                        self.navigate_disk(None, ctx);
                        ui.close();
                    }
                    let theme_label = self.t("Тёмная тема", "Dark mode");
                    ui.checkbox(&mut self.dark, theme_label);
                });
                ui.menu_button(self.t("Справка", "Help"), |ui| {
                    ui.label("WhereIsIt AI · native egui R2");
                    ui.label(self.t(
                        "Локальные метаданные, без чтения видео.",
                        "Local metadata only; no video reads.",
                    ));
                    ui.label(self.t(
                        "Совпадения — кандидаты, не точные копии.",
                        "Matches are candidates, not byte-verified copies.",
                    ));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let language_label = if self.lang == Language::Russian {
                        "RU"
                    } else {
                        "EN"
                    };
                    if ui.small_button(language_label).clicked() {
                        self.lang = if self.lang == Language::Russian {
                            Language::English
                        } else {
                            Language::Russian
                        };
                    }
                    ui.label(RichText::new("native egui · preview").small().weak());
                });
            });
        });
    }

    fn search_toolbar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::top("search_toolbar").show(root, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                let back = ui.add_enabled(
                    !self.history.is_empty() && self.active_request.is_none(),
                    egui::Button::new(self.t("← Назад", "← Back")),
                );
                if back.clicked() {
                    self.go_back(ctx);
                }
                let scope = self
                    .selected_volume
                    .and_then(|id| self.volumes.iter().find(|v| v.id == id))
                    .map(|v| format!("#{} · {}", v.id, self.display_name(v)))
                    .unwrap_or_else(|| self.t("Все диски", "All disks").to_owned());
                ui.label(RichText::new(scope).strong());
                ui.separator();
                let query_hint = self.t(
                    "Поиск: имя, размер, ext, AND/OR/NOT…",
                    "Search: name, size, ext, AND/OR/NOT…",
                );
                let response = ui.add_sized(
                    [ui.available_width().max(220.0) - 185.0, 25.0],
                    egui::TextEdit::singleline(&mut self.query_input)
                        .id(egui::Id::unique(SEARCH_INPUT_ID))
                        .hint_text(query_hint),
                );
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let clicked = ui
                    .add_enabled(
                        self.db.is_some(),
                        egui::Button::new(self.t("Найти", "Search")),
                    )
                    .clicked();
                if enter || clicked {
                    self.navigate_search(ctx);
                }
                if ui
                    .add_enabled(self.db.is_some(), egui::Button::new(self.t("Все", "All")))
                    .clicked()
                {
                    self.selected_volume = None;
                    self.navigate_search(ctx);
                }
            });
            ui.add_space(3.0);
        });
    }

    fn disk_sidebar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::left("disk_register")
            .resizable(true)
            .default_size(245.0)
            .min_size(180.0)
            .max_size(420.0)
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(self.t("Диски каталога", "Catalog disks"));
                    ui.label(RichText::new(self.volumes.len().to_string()).weak());
                });
                let disk_filter_hint = self.t("Фильтр дисков", "Filter disks");
                ui.add(
                    egui::TextEdit::singleline(&mut self.disk_filter).hint_text(disk_filter_hint),
                );
                ui.separator();
                if ui
                    .selectable_label(
                        self.selected_volume.is_none(),
                        format!(
                            "{} ({})",
                            self.t("Все диски", "All disks"),
                            self.volumes.len()
                        ),
                    )
                    .clicked()
                {
                    self.navigate_disk(None, ctx);
                }
                ui.separator();

                let filter = self.disk_filter.to_lowercase();
                let visible: Vec<_> = self
                    .volumes
                    .iter()
                    .filter(|v| {
                        filter.is_empty()
                            || self.display_name(v).to_lowercase().contains(&filter)
                            || self
                                .disk_preferences
                                .get(&v.id)
                                .is_some_and(|p| p.shelf_label.to_lowercase().contains(&filter))
                            || v.id.to_string().contains(&filter)
                    })
                    .cloned()
                    .collect();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for volume in &visible {
                            let selected = self.selected_volume == Some(volume.id);
                            ui.horizontal(|ui| {
                                let shelf = self
                                    .disk_preferences
                                    .get(&volume.id)
                                    .map(|p| p.shelf_label.as_str())
                                    .filter(|s| !s.is_empty());
                                let title = match shelf {
                                    Some(shelf) => format!(
                                        "#{} · {} [{}]{}",
                                        volume.id,
                                        self.display_name(volume),
                                        shelf,
                                        if self.shelf_collisions.contains(&volume.id) {
                                            " ⚠"
                                        } else {
                                            ""
                                        }
                                    ),
                                    None => {
                                        format!("#{} · {}", volume.id, self.display_name(volume))
                                    }
                                };
                                let label = ui.selectable_label(selected, title);
                                if label.clicked() {
                                    self.navigate_disk(Some(volume.id), ctx);
                                }
                                if label.double_clicked() && self.active_request.is_none() {
                                    self.history.push((self.route.clone(), self.offset));
                                    self.route = Route::Folder {
                                        volume_id: volume.id,
                                        parent_id: None,
                                    };
                                    self.offset = 0;
                                    self.load_page(ctx);
                                }
                                if ui
                                    .small_button("⋯")
                                    .on_hover_text(self.t("Свойства диска", "Disk properties"))
                                    .clicked()
                                {
                                    self.open_disk_properties(volume.id);
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        volume.entry_count,
                                        bytes_compact(volume.total_bytes)
                                    ))
                                    .small()
                                    .weak(),
                                );
                            });
                            ui.add_space(1.0);
                        }
                    });
            });
    }

    fn inspector(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::bottom("inspector")
            .resizable(true)
            .default_size(138.0)
            .min_size(95.0)
            .max_size(320.0)
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(self.t("Инспектор", "Inspector"));
                    ui.separator();
                    ui.label(RichText::new(&self.status).small().weak());
                    if self.active_request.is_some() { ui.spinner(); }
                });
                if self.scan_job_id.is_some() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{}: {}",
                            self.t("Обработано записей", "Entries processed"),
                            self.scan_progress_entries,
                        ));
                        if ui.add_enabled(
                            self.scan_registered && !self.scan_cancel_requested,
                            egui::Button::new(self.t("Отменить сканирование", "Cancel scan")),
                        ).clicked() {
                            self.request_scan_cancel(ctx);
                        }
                        if self.scan_cancel_requested {
                            ui.label(self.t("Отмена запрошена", "Cancellation requested"));
                        }
                    });
                }
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::from_rgb(224, 96, 88), error);
                }
                ui.separator();
                if let Some(selected) = self.selected.clone() {
                    ui.horizontal(|ui| {
                        ui.strong(&selected.name);
                        ui.separator();
                        ui.label(bytes_compact(selected.size));
                        ui.label(RichText::new(format!("#{}", selected.volume_id)).weak());
                    });
                    ui.label(format!("{}: {}", self.t("Записанный путь", "Recorded path"), selected.relative_path));
                    if !selected.description.is_empty() {
                        ui.label(format!("{}: {}", self.t("Описание", "Description"), selected.description));
                    }
                    ui.horizontal(|ui| {
                        let copies = ui.add_enabled(!selected.is_directory(),
                            egui::Button::new(self.t("Кандидаты на других дисках", "Copies on other disks")));
                        if copies.clicked() { self.navigate_copies(ctx); }
                        if selected.is_directory() && ui.button(self.t("Открыть папку", "Open folder")).clicked() {
                            self.navigate_folder(&selected, ctx);
                        }
                        if ui.small_button(self.t("Копировать путь", "Copy path")).clicked() {
                            ui.ctx().copy_text(selected.relative_path.clone());
                        }
                    });
                } else {
                    ui.label(RichText::new(self.t(
                        "Выберите файл. Показаны только записанные метаданные, физическое наличие USB не подтверждено.",
                        "Select a file. These are recorded metadata only; physical USB presence is unverified."
                    )).weak());
                }
            });
    }

    fn results_table(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::CentralPanel::default().show(root, |ui| {
            if self.db.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(75.0);
                    ui.heading("WhereIsIt AI");
                    ui.label(self.t(
                        "Офлайн-видеокаталог · PR58 → Rust/egui",
                        "Offline video catalog · PR58 → Rust/egui",
                    ));
                    ui.add_space(10.0);
                    if ui
                        .button(self.t(
                            "Открыть существующий SQLite-каталог",
                            "Open existing SQLite catalog",
                        ))
                        .clicked()
                    {
                        self.show_catalog_dialog = true;
                        self.create_catalog = false;
                    }
                    ui.label(
                        RichText::new(self.t(
                            "Это настоящий нативный клиент, не демонстрация на вымышленных дисках.",
                            "This is the native client, not a fake-disk simulator.",
                        ))
                        .small()
                        .weak(),
                    );
                });
                return;
            }
            ui.horizontal(|ui| {
                ui.strong(match &self.route {
                    Route::Search { .. } => self.t("Результаты поиска", "Search results"),
                    Route::Folder { .. } => self.t("Содержимое каталога", "Folder contents"),
                    Route::Copies { .. } => self.t(
                        "Кандидаты по имени и размеру · НЕ байтовая проверка",
                        "Name+size candidates · NOT byte verified",
                    ),
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            self.page.has_more && self.active_request.is_none(),
                            egui::Button::new(self.t("Далее →", "Next →")),
                        )
                        .clicked()
                    {
                        self.offset = self.offset.saturating_add(self.page.limit.max(1));
                        self.load_page(ctx);
                    }
                    if ui
                        .add_enabled(
                            self.offset > 0 && self.active_request.is_none(),
                            egui::Button::new(self.t("← Ранее", "← Previous")),
                        )
                        .clicked()
                    {
                        self.offset = self.offset.saturating_sub(self.page.limit.max(1));
                        self.load_page(ctx);
                    }
                    ui.label(
                        RichText::new(format!(
                            "{}–{}",
                            self.offset.saturating_add(if self.page.items.is_empty() {
                                0
                            } else {
                                1
                            }),
                            self.offset.saturating_add(self.page.items.len() as u64)
                        ))
                        .weak(),
                    );
                });
            });
            ui.separator();
            let rows = self.page.items.clone();
            let selection = self.selected.as_ref().map(|e| e.id);
            let mut chosen: Option<Entry> = None;
            let mut open_folder: Option<Entry> = None;
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::initial(42.0).at_least(35.0))
                .column(Column::remainder().at_least(180.0))
                .column(Column::initial(86.0).at_least(72.0))
                .column(Column::initial(92.0).at_least(80.0))
                .column(Column::initial(235.0).at_least(90.0))
                .header(24.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Disk");
                    });
                    header.col(|ui| {
                        ui.strong(self.t("Имя", "Name"));
                    });
                    header.col(|ui| {
                        ui.strong(self.t("Размер", "Size"));
                    });
                    header.col(|ui| {
                        ui.strong(self.t("Тип", "Kind"));
                    });
                    header.col(|ui| {
                        ui.strong(self.t("Путь", "Path"));
                    });
                })
                .body(|body| {
                    body.rows(24.0, rows.len(), |mut row| {
                        let entry = &rows[row.index()];
                        row.col(|ui| {
                            ui.label(format!("#{}", entry.volume_id));
                        });
                        row.col(|ui| {
                            let name = if entry.is_directory() {
                                format!("▸ {}", entry.name)
                            } else {
                                entry.name.clone()
                            };
                            let result = ui.selectable_label(selection == Some(entry.id), name);
                            if result.clicked() {
                                chosen = Some(entry.clone());
                            }
                            if result.double_clicked() && entry.is_directory() {
                                open_folder = Some(entry.clone());
                            }
                        });
                        row.col(|ui| {
                            ui.label(bytes_compact(entry.size));
                        });
                        row.col(|ui| {
                            let kind = if entry.is_directory() {
                                "Folder"
                            } else if ["mkv", "mp4", "avi", "m2ts", "mov", "webm"]
                                .iter()
                                .any(|ext| entry.name.to_lowercase().ends_with(&format!(".{ext}")))
                            {
                                "Video*"
                            } else {
                                "File"
                            };
                            ui.label(kind)
                                .on_hover_text("Video* = filename extension only");
                        });
                        row.col(|ui| {
                            ui.label(&entry.relative_path);
                        });
                    });
                });
            if let Some(entry) = chosen {
                self.selected = Some(entry);
            }
            if let Some(folder) = open_folder {
                self.navigate_folder(&folder, ctx);
            }
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.show_scan_dialog {
            let mut open = self.show_scan_dialog;
            egui::Window::new(self.t("Сканирование диска", "Scan disk"))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(560.0)
                .show(ctx, |ui| {
                    ui.label(self.t(
                        "Папка или корень диска (только метаданные):",
                        "Folder or disk root (metadata only):",
                    ));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.scan_root)
                            .desired_width(f32::INFINITY),
                    );
                    ui.label(self.t("Название диска в каталоге:", "Catalog disk label:"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.scan_label)
                            .desired_width(f32::INFINITY),
                    );
                    ui.label(RichText::new(self.t(
                        "Файлы не читаются и не изменяются. Создаётся новый снимок в SQLite.",
                        "File contents are not read or changed. A new SQLite snapshot is created.",
                    )).small().weak());
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                self.db.is_some() && self.active_request.is_none(),
                                egui::Button::new(self.t("Начать сканирование", "Start scan")),
                            )
                            .clicked()
                        {
                            self.start_scan(ctx);
                        }
                        if ui.button(self.t("Закрыть", "Close")).clicked() {
                            self.show_scan_dialog = false;
                        }
                    });
                });
            self.show_scan_dialog &= open;
        }
        if self.show_export_dialog {
            let mut open = self.show_export_dialog;
            egui::Window::new(self.t("Экспорт метаданных", "Metadata export"))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(520.0)
                .show(ctx, |ui| {
                    ui.label(self.t(
                        "Экспортируется весь текущий поиск или ветка, а не только видимая страница.",
                        "Exports the full current search or branch, not only the visible page.",
                    ));
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.export_kind, ExportKind::Csv, "CSV");
                        ui.selectable_value(&mut self.export_kind, ExportKind::Json, "JSON");
                        ui.selectable_value(&mut self.export_kind, ExportKind::Parquet, "Parquet");
                    });
                    ui.add(egui::TextEdit::singleline(&mut self.export_path)
                        .desired_width(f32::INFINITY));
                    ui.label(self.t(
                        "Укажите НОВЫЙ файл; существующие файлы не перезаписываются.",
                        "Enter a NEW file path; existing files are never overwritten.",
                    ));
                    if let Some(error) = &self.error {
                        ui.colored_label(Color32::from_rgb(224, 96, 88), error);
                    }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.active_request.is_none(),
                            egui::Button::new(self.t("Экспортировать", "Export"))
                        ).clicked() {
                            self.start_export(ctx);
                        }
                        if ui.button(self.t("Отмена", "Cancel")).clicked() {
                            self.show_export_dialog = false;
                        }
                    });
                });
            self.show_export_dialog &= open;
        }
        if self.show_catalog_dialog {
            let mut open = self.show_catalog_dialog;
            egui::Window::new(self.t("Каталог SQLite", "SQLite catalog"))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(510.0)
                .show(ctx, |ui| {
                    ui.label(self.t(
                        "Локальный путь к каталогу (SQLite). Не открывайте исходный CTF здесь.",
                        "Local SQLite catalog path. Do not open source CTF here.",
                    ));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.path_input)
                            .desired_width(f32::INFINITY),
                    );
                    ui.horizontal(|ui| {
                        if ui.button(self.t("Открыть", "Open")).clicked() {
                            self.start_catalog(false, ctx);
                        }
                        if ui.button(self.t("Создать новый", "Create new")).clicked() {
                            self.start_catalog(true, ctx);
                        }
                        if ui.button(self.t("Закрыть", "Close")).clicked() {
                            self.show_catalog_dialog = false;
                        }
                    });
                });
            self.show_catalog_dialog &= open;
        }
        if let Some(id) = self.show_disk_properties {
            if let Some(volume) = self.volumes.iter().find(|v| v.id == id).cloned() {
                let mut open = true;
                let normalized = self
                    .disk_draft
                    .values
                    .shelf_label
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase();
                let collision = !normalized.is_empty()
                    && self.disk_preferences.iter().any(|(other, p)| {
                        *other != id
                            && p.shelf_label
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                                .to_lowercase()
                                == normalized
                    });
                let local_name = self.t("Локальное имя", "Local display name");
                let shelf = self.t("Код / наклейка", "Sticker / code");
                let model = self.t("Модель (указана пользователем)", "Model (user supplied)");
                let filesystem = self.t(
                    "Файловая система (указана пользователем)",
                    "Filesystem (user supplied)",
                );
                let capacity = self.t(
                    "Ёмкость в байтах (необязательно)",
                    "Capacity in bytes (optional)",
                );
                let note = self.t("Описание / заметка", "Description / note");
                let save = self.t("Сохранить", "Save");
                let cancel = self.t("Отмена", "Cancel");
                egui::Window::new(self.t("Свойства диска", "Disk properties"))
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(true)
                    .default_width(520.0)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                            ui.strong(format!("#{} · {}", volume.id, volume.label));
                            ui.label(self.t("Локальное имя и свойства хранятся отдельно; исходный каталог и его экспортируемое имя не меняются.",
                                "Local name and properties are stored separately; the source catalog and its exported name stay unchanged."));
                            ui.label(local_name);
                            let name_response = ui.add(egui::TextEdit::singleline(&mut self.disk_draft.values.display_name).id(egui::Id::unique(DISK_NAME_INPUT_ID)).desired_width(f32::INFINITY));
                            if self.focus_disk_name { name_response.request_focus(); self.focus_disk_name = false; }
                            ui.label(shelf);
                            ui.add(egui::TextEdit::singleline(&mut self.disk_draft.values.shelf_label).desired_width(f32::INFINITY));
                            if collision { ui.colored_label(Color32::from_rgb(224, 160, 40), self.t("Такая наклейка указана и для другого диска.", "This sticker is also assigned to another disk.")); }
                            ui.label(model);
                            ui.add(egui::TextEdit::singleline(&mut self.disk_draft.values.model).desired_width(f32::INFINITY));
                            ui.label(filesystem);
                            ui.add(egui::TextEdit::singleline(&mut self.disk_draft.values.filesystem).desired_width(f32::INFINITY));
                            ui.label(capacity);
                            ui.add(egui::TextEdit::singleline(&mut self.disk_draft.capacity).desired_width(f32::INFINITY));
                            ui.label(note);
                            ui.add(egui::TextEdit::multiline(&mut self.disk_draft.values.note).desired_rows(3).desired_width(f32::INFINITY));
                            ui.label(format!("{}: {} · {}", self.t("Записано объектов / байт", "Recorded entries / bytes"), volume.entry_count, bytes_compact(volume.total_bytes)));
                            ui.label(format!("{}: {}", self.t("Исторический корень", "Recorded root"), volume.root_path));
                            ui.label(self.t("Наклейка и введённые свойства НЕ подтверждают подключение USB.", "Sticker and entered properties do NOT verify a connected USB device."));
                            if !self.preferences_ready { ui.colored_label(Color32::from_rgb(224, 96, 88), self.t("Локальное хранилище не загружено; сохранение недоступно.", "Local store was not loaded; saving is unavailable.")); }
                            if let Some(error) = &self.error { ui.colored_label(Color32::from_rgb(224, 96, 88), error); }
                            ui.horizontal(|ui| {
                                if ui.add_enabled(self.preferences_ready && self.active_request.is_none(), egui::Button::new(save)).clicked() { self.save_disk_properties(ctx); }
                                if ui.add_enabled(self.active_request.is_none(), egui::Button::new(cancel)).clicked() { self.show_disk_properties = None; }
                            });
                        });
                    });
                if !open && self.active_request.is_none() {
                    self.show_disk_properties = None;
                }
            } else {
                self.show_disk_properties = None;
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let modal_open = self.show_catalog_dialog
            || self.show_export_dialog
            || self.show_scan_dialog
            || self.show_disk_properties.is_some();
        if modal_open {
            return;
        }
        // Use modifiers attached to the key event: the user can release Ctrl
        // before the next frame, leaving the frame's final modifiers empty.
        let (search, properties, all, back) = ctx.input(|i| {
            let has = |key, modifier: fn(egui::Modifiers) -> bool| {
                i.events.iter().any(|event| {
                    matches!(event,
                    egui::Event::Key { key: pressed_key, pressed: true, modifiers, .. }
                        if *pressed_key == key && modifier(*modifiers))
                })
            };
            (
                has(egui::Key::F, |m| m.command),
                has(egui::Key::F2, |_| true) || has(egui::Key::Enter, |m| m.alt),
                has(egui::Key::L, |m| m.command),
                has(egui::Key::ArrowLeft, |m| m.alt),
            )
        });
        if all {
            self.navigate_disk(None, ctx);
        }
        if search {
            ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique(SEARCH_INPUT_ID)));
        }
        if properties && self.active_request.is_none() {
            if let Some(id) = self.selected_volume {
                self.open_disk_properties(id);
            }
        }
        if back {
            self.go_back(ctx);
        }
    }
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        ctx.set_visuals(if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        if self.auto_open {
            self.auto_open = false;
            self.start_catalog(false, &ctx);
        }
        while let Some(reply) = self.bus.poll() {
            self.apply_reply(reply, &ctx);
        }
        self.poll_scan_progress(&ctx);
        self.handle_shortcuts(&ctx);
        self.menu(root, &ctx);
        self.search_toolbar(root, &ctx);
        self.disk_sidebar(root, &ctx);
        self.inspector(root, &ctx);
        self.results_table(root, &ctx);
        self.dialogs(&ctx);
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;

    #[test]
    fn rejected_export_replaces_previous_success_status() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("synthetic.sqlite".to_owned());
        app.export_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("Cargo.toml")
            .to_string_lossy()
            .into_owned();
        app.status = "Export completed".to_owned();
        app.show_export_dialog = true;

        app.start_export(&ctx);

        assert_eq!(app.status, "Операция отклонена");
        assert!(app.error.is_some());
        assert!(app.show_export_dialog);
        assert!(app.active_request.is_none());
    }

    fn key_input(key: egui::Key, modifiers: egui::Modifiers) -> egui::RawInput {
        egui::RawInput {
            focused: true,
            events: vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
                egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: false,
                    repeat: false,
                    modifiers,
                },
                egui::Event::ModifiersChanged(egui::Modifiers::default()),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn ctrl_f_focuses_search_without_discarding_navigation() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.query_input = "pending query".into();
        app.route = Route::Folder {
            volume_id: 7,
            parent_id: Some(13),
        };
        app.offset = 250;
        let route = app.route.clone();
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let mut output = ctx.run_ui(key_input(egui::Key::F, modifiers), |root| {
            let ctx = root.ctx().clone();
            app.handle_shortcuts(&ctx);
            app.search_toolbar(root, &ctx);
        });
        output.textures_delta.clear();
        assert_eq!(app.route, route);
        assert_eq!(app.offset, 250);
        assert_eq!(app.query_input, "pending query");
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(egui::Id::unique(SEARCH_INPUT_ID))
        );
    }

    #[test]
    fn released_modifiers_still_trigger_all_disks_and_back() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.selected_volume = Some(60);
        app.query_input = "video".into();
        let ctrl = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let mut output = ctx.run_pass(key_input(egui::Key::L, ctrl), |ctx| {
            app.handle_shortcuts(ctx)
        });
        output.textures_delta.clear();
        assert_eq!(app.selected_volume, None);
        assert_eq!(
            app.route,
            Route::Search {
                query: "video".into(),
                volume_id: None
            }
        );

        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        let original = Route::Search {
            query: "video".into(),
            volume_id: Some(60),
        };
        app.route = Route::Copies { entry_id: 9 };
        app.history.push((original.clone(), 250));
        let alt = egui::Modifiers {
            alt: true,
            ..Default::default()
        };
        let mut output = ctx.run_pass(key_input(egui::Key::ArrowLeft, alt), |ctx| {
            app.handle_shortcuts(ctx)
        });
        output.textures_delta.clear();
        assert_eq!(app.route, original);
        assert_eq!(app.offset, 250);
    }

    fn selected_volume(app: &mut DesktopApp) {
        app.selected_volume = Some(60);
        app.volumes.push(Volume {
            id: 60,
            label: "Disk 60".to_owned(),
            root_path: "synthetic".to_owned(),
            entry_count: 1,
            total_bytes: 1,
        });
    }

    #[test]
    fn alt_enter_opens_selected_disk_properties() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        selected_volume(&mut app);
        let modifiers = egui::Modifiers {
            alt: true,
            ..Default::default()
        };
        let mut output = ctx.run_pass(key_input(egui::Key::Enter, modifiers), |ctx| {
            app.handle_shortcuts(ctx)
        });
        output.textures_delta.clear();
        assert_eq!(app.show_disk_properties, Some(60));
    }

    #[test]
    fn f2_opens_selected_disk_properties_and_respects_modal_dialogs() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        selected_volume(&mut app);
        let mut output = ctx.run_pass(
            key_input(egui::Key::F2, egui::Modifiers::default()),
            |ctx| app.handle_shortcuts(ctx),
        );
        output.textures_delta.clear();
        assert_eq!(app.show_disk_properties, Some(60));

        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        selected_volume(&mut app);
        app.show_export_dialog = true;
        let mut output = ctx.run_pass(
            key_input(egui::Key::F2, egui::Modifiers::default()),
            |ctx| app.handle_shortcuts(ctx),
        );
        output.textures_delta.clear();
        assert_eq!(app.show_disk_properties, None);
    }
}
