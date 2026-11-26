//! PR58-first native workbench. Every user-visible catalog fact comes from atlas-service;
//! no fictitious USB mount badges, byte-duplicate claims or on-disk delete operations.
use crate::bridge::{
    analytics_cancel_request, analytics_request, exact_category_query, extension_search_query,
    new_scan_job_id, primary_category_search_query, scan_request, CommandBus, ExportKind, Purpose,
    Route,
};
use crate::categories::{self, parse_categories, Category};
use crate::linked::LinkedCtf;
use crate::local_analyzer::LocalAnalyzer;
use crate::model::{bytes_compact, parse_volumes, DiskPreferences, Entry, Page, Volume};
use crate::physical::PhysicalPanel;
use crate::shelf::{valid_label, ShelfContext, ShelfSnapshot};
use eframe::egui::{self, Color32, RichText};
use egui_extras::{Column, TableBuilder};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const PAGE_LIMIT: u64 = 250;
const SEARCH_INPUT_ID: &str = "global_search_input";
const DISK_NAME_INPUT_ID: &str = "disk_local_name_input";
const LANGUAGE_SETTING: &str = "whereisit.language.v1";
const THEME_SETTING: &str = "whereisit.theme.v1";
type FolderKey = (i64, Option<i64>);
type HistoryEntry = (
    Route,
    u64,
    Option<Entry>,
    Option<i64>,
    f32,
    Vec<BrowseCrumb>,
    String,
);

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrowseCrumb {
    id: i64,
    parent_id: Option<i64>,
    name: String,
    kind: String,
}

#[derive(Clone, Debug, Default)]
struct TreeChildren {
    items: Vec<Entry>,
    next_offset: u64,
    has_more: bool,
    loading_request: Option<u64>,
    loading_offset: Option<u64>,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    Russian,
    English,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisterOrder {
    Id,
    IndexedBytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspaceTab {
    Search,
    Slices,
    VolumeMap,
    UsbDisks,
    LocalPc,
}

impl WorkspaceTab {
    const ORDERED: [Self; 5] = [
        Self::Search,
        Self::Slices,
        Self::VolumeMap,
        Self::UsbDisks,
        Self::LocalPc,
    ];

    fn label(self, lang: Language) -> &'static str {
        match self {
            Self::Search => lang.text("Поиск", "Search"),
            Self::Slices => lang.text("Разрезы", "Slices"),
            Self::VolumeMap => lang.text("Карта объёма", "Volume Map"),
            Self::UsbDisks => lang.text("Носители", "USB Disks"),
            Self::LocalPc => lang.text("Локальный ПК", "Local PC"),
        }
    }

    fn available(self) -> bool {
        matches!(
            self,
            Self::Search | Self::Slices | Self::VolumeMap | Self::UsbDisks | Self::LocalPc
        )
    }
}

fn sort_register(volumes: &mut [Volume], order: RegisterOrder) {
    match order {
        RegisterOrder::Id => volumes.sort_by_key(|volume| volume.id),
        RegisterOrder::IndexedBytes => volumes.sort_by(|a, b| {
            b.total_bytes
                .cmp(&a.total_bytes)
                .then_with(|| a.id.cmp(&b.id))
        }),
    }
}

fn draw_annular_sector(
    painter: &egui::Painter,
    center: egui::Pos2,
    max_radius: f32,
    radii: (f32, f32),
    start_angle: f64,
    end_angle: f64,
    color: Color32,
) {
    let sweep = (end_angle - start_angle).abs();
    if sweep < 0.0005 {
        return;
    }
    let steps = ((sweep * 24.0).ceil() as usize).max(1);
    let point = |radius: f32, angle: f64| {
        center + egui::vec2(angle.cos() as f32, angle.sin() as f32) * (max_radius * radius)
    };
    let mut mesh = egui::epaint::Mesh::default();
    for i in 0..=steps {
        let angle = start_angle + (end_angle - start_angle) * i as f64 / steps as f64;
        mesh.colored_vertex(point(radii.0, angle), color);
        mesh.colored_vertex(point(radii.1, angle), color);
    }
    for i in 0..steps {
        let base = (i * 2) as u32;
        mesh.add_triangle(base, base + 1, base + 3);
        mesh.add_triangle(base, base + 3, base + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn register_matches(
    volume: &Volume,
    display_name: &str,
    sticker: &str,
    note: &str,
    filter: &str,
) -> bool {
    filter.is_empty()
        || volume.label.to_lowercase().contains(filter)
        || display_name.to_lowercase().contains(filter)
        || sticker.to_lowercase().contains(filter)
        || note.to_lowercase().contains(filter)
        || volume.id.to_string().contains(filter)
        || format!("#{}", volume.id).contains(filter)
}

struct StatusText {
    ru: String,
    en: String,
}

impl StatusText {
    fn new(ru: impl Into<String>, en: impl Into<String>) -> Self {
        Self {
            ru: ru.into(),
            en: en.into(),
        }
    }

    fn text(&self, language: Language) -> &str {
        match language {
            Language::Russian => &self.ru,
            Language::English => &self.en,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct DiskPropertiesDraft {
    values: DiskPreferences,
    original_shelf: String,
    capacity: String,
}

fn user_store() -> Option<std::path::PathBuf> {
    #[cfg(all(feature = "qa-isolated", target_os = "windows"))]
    {
        crate::qa_isolation::user_store_path()
    }
    #[cfg(not(all(feature = "qa-isolated", target_os = "windows")))]
    {
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
        let root = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|p| std::path::PathBuf::from(p).join(".local/share"))
            });
        root.filter(|p| p.is_absolute()).map(|p| {
            p.join("AtlasCatalog")
                .join("physical-volume-bindings.sqlite")
        })
    }
}

fn preference_platform() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        Some("windows")
    } else if cfg!(target_os = "macos") {
        Some("macos")
    } else {
        // Match the pre-existing egui shelf-label adapter on Linux.
        Some("macos")
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CatalogIntegrity {
    Closed,
    Unchecked,
    Checking,
    Verified,
    Failed,
    LinkedReadOnly,
}

impl CatalogIntegrity {
    fn text(self, lang: Language) -> Option<&'static str> {
        Some(match self {
            Self::Closed => return None,
            Self::LinkedReadOnly => lang.text(
                "CTF: связанный источник только для чтения",
                "CTF: linked source is read-only",
            ),
            Self::Unchecked => lang.text("Целостность не проверена", "Integrity not checked"),
            Self::Checking => lang.text("Проверка целостности…", "Checking integrity…"),
            Self::Verified => lang.text(
                "Последняя проверка целостности SQLite пройдена",
                "Last SQLite integrity check passed",
            ),
            Self::Failed => lang.text(
                "Проверка целостности не пройдена. Каталог закрыт.",
                "Integrity check failed. Catalog closed.",
            ),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CtfDialog {
    Import,
    Export,
}

pub struct DesktopApp {
    bus: CommandBus,
    local_analyzer: LocalAnalyzer,
    active_request: Option<u64>,
    db: Option<String>,
    workspace_tab: WorkspaceTab,
    workspace_tab_focus_request: bool,
    analytics_job_id: Option<String>,
    analytics_preview_job: bool,
    analytics_request_id: Option<u64>,
    analytics_generation: u64,
    analytics_db: Option<String>,
    analytics_data: Option<Arc<Value>>,
    analytics_error: Option<String>,
    analytics_cancelled: bool,
    analytics_volume: Option<i64>,
    analytics_folder: Option<i64>,
    analytics_category_id: Option<i64>,
    analytics_group: String,
    analytics_highlight: String,
    analytics_applied_highlight: String,
    slices_dimension: usize,
    analytics_loaded: bool,
    slices_job_id: Option<String>,
    slices_preview_job: bool,
    slices_request_id: Option<u64>,
    slices_generation: u64,
    slices_db: Option<String>,
    slices_data: Option<Arc<Value>>,
    slices_error: Option<String>,
    slices_cancelled: bool,
    slices_volume: Option<i64>,
    slices_category: Option<i64>,
    slices_loaded: bool,
    integrity: CatalogIntegrity,
    linked_ctf: Option<LinkedCtf>,
    ctf_dialog: Option<CtfDialog>,
    ctf_source_input: String,
    ctf_target_input: String,
    ctf_job_id: Option<String>,
    ctf_progress_request: Option<u64>,
    ctf_cancel_request: Option<u64>,
    ctf_cancel_requested: bool,
    ctf_job_registered: bool,
    ctf_phase: String,
    ctf_entries: u64,
    ctf_last_poll: Option<Instant>,
    description_entry_id: Option<i64>,
    description_input: String,
    path_input: String,
    query_input: String,
    volumes: Vec<Volume>,
    shelf_context: Option<ShelfContext>,
    physical: PhysicalPanel,
    shelf: ShelfSnapshot,
    shelf_editor: String,
    legacy_shelf_editable: bool,
    route: Route,
    history: Vec<HistoryEntry>,
    page: Page,
    offset: u64,
    selected: Option<Entry>,
    restore_selection: Option<Entry>,
    reveal_selected: bool,
    results_scroll: f32,
    restore_scroll: Option<f32>,
    selected_volume: Option<i64>,
    folder_crumbs: Vec<BrowseCrumb>,
    catalog_generation: u64,
    tree_children: HashMap<FolderKey, TreeChildren>,
    tree_expanded: HashSet<FolderKey>,
    tree_path_nodes: HashMap<FolderKey, Vec<BrowseCrumb>>,
    tree_reveal_target: Option<FolderKey>,
    tree_ancestors_request_id: Option<u64>,
    tree_ancestors_target: Option<(i64, i64)>,
    disk_filter: String,
    register_order: RegisterOrder,
    disks_filter: String,
    disks_order: RegisterOrder,
    disks_selected_volume: Option<i64>,
    disks_scroll: f32,
    show_catalog_dialog: bool,
    show_categories_dialog: bool,
    categories: Vec<Category>,
    categories_loaded: bool,
    category_assigned: HashSet<i64>,
    category_assignments_ready: bool,
    category_assignments_error: bool,
    category_result_refresh_pending: bool,
    category_name_input: String,
    category_parent_id: Option<i64>,
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
    confirm_rescan: bool,
    focus_disk_name: bool,
    disk_draft: DiskPropertiesDraft,
    disk_preferences: HashMap<i64, DiskPreferences>,
    shelf_collisions: HashSet<i64>,
    preferences_ready: bool,
    status: StatusText,
    error: Option<String>,
    lang: Language,
    dark: bool,
    auto_open: bool,
}

impl DesktopApp {
    fn dialog_open(&self) -> bool {
        self.ctf_dialog.is_some()
            || self.description_entry_id.is_some()
            || self.show_catalog_dialog
            || self.show_categories_dialog
            || self.show_export_dialog
            || self.show_scan_dialog
            || self.show_disk_properties.is_some()
            || self.physical.is_open()
    }

    fn protect_background(&self, ui: &mut egui::Ui) {
        if self.dialog_open() {
            ui.disable();
        }
    }

    pub fn new(cc: &eframe::CreationContext<'_>, startup_catalog: Option<String>) -> Self {
        let mut app = Self::with_context(&cc.egui_ctx, startup_catalog);
        app.restore_appearance(cc.storage);
        app
    }

    fn restore_appearance(&mut self, storage: Option<&dyn eframe::Storage>) {
        if let Some(storage) = storage {
            self.lang = match storage.get_string(LANGUAGE_SETTING).as_deref() {
                Some("en") => Language::English,
                _ => Language::Russian,
            };
            self.dark = !matches!(storage.get_string(THEME_SETTING).as_deref(), Some("light"));
        }
    }

    fn with_context(ctx: &egui::Context, startup_catalog: Option<String>) -> Self {
        ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = egui::vec2(6.0, 5.0);
            style.spacing.button_padding = egui::vec2(7.0, 4.0);
        });
        Self {
            bus: CommandBus::new(),
            local_analyzer: LocalAnalyzer::default(),
            active_request: None,
            db: None,
            workspace_tab: WorkspaceTab::Search,
            workspace_tab_focus_request: false,
            analytics_job_id: None,
            analytics_preview_job: false,
            analytics_request_id: None,
            analytics_generation: 0,
            analytics_db: None,
            analytics_data: None,
            analytics_error: None,
            analytics_cancelled: false,
            analytics_volume: None,
            analytics_folder: None,
            analytics_category_id: None,
            analytics_group: "volumes".to_owned(),
            analytics_highlight: String::new(),
            analytics_applied_highlight: String::new(),
            slices_dimension: 0,
            analytics_loaded: false,
            slices_job_id: None,
            slices_preview_job: false,
            slices_request_id: None,
            slices_generation: 0,
            slices_db: None,
            slices_data: None,
            slices_error: None,
            slices_cancelled: false,
            slices_volume: None,
            slices_category: None,
            slices_loaded: false,
            integrity: CatalogIntegrity::Closed,
            linked_ctf: None,
            ctf_dialog: None,
            ctf_source_input: String::new(),
            ctf_target_input: String::new(),
            ctf_job_id: None,
            ctf_progress_request: None,
            ctf_cancel_request: None,
            ctf_cancel_requested: false,
            ctf_job_registered: false,
            ctf_phase: String::new(),
            ctf_entries: 0,
            ctf_last_poll: None,
            description_entry_id: None,
            description_input: String::new(),
            path_input: startup_catalog.clone().unwrap_or_default(),
            query_input: String::new(),
            volumes: Vec::new(),
            shelf_context: None,
            physical: PhysicalPanel::default(),
            shelf: ShelfSnapshot::default(),
            shelf_editor: String::new(),
            legacy_shelf_editable: false,
            route: Route::Catalog,
            history: Vec::new(),
            page: Page::default(),
            offset: 0,
            selected: None,
            restore_selection: None,
            reveal_selected: false,
            results_scroll: 0.0,
            restore_scroll: None,
            selected_volume: None,
            folder_crumbs: Vec::new(),
            catalog_generation: 0,
            tree_children: HashMap::new(),
            tree_expanded: HashSet::new(),
            tree_path_nodes: HashMap::new(),
            tree_reveal_target: None,
            tree_ancestors_request_id: None,
            tree_ancestors_target: None,
            disk_filter: String::new(),
            register_order: RegisterOrder::Id,
            disks_filter: String::new(),
            disks_order: RegisterOrder::Id,
            disks_selected_volume: None,
            disks_scroll: 0.0,
            show_catalog_dialog: false,
            show_categories_dialog: false,
            categories: Vec::new(),
            categories_loaded: false,
            category_assigned: HashSet::new(),
            category_assignments_ready: false,
            category_assignments_error: false,
            category_result_refresh_pending: false,
            category_name_input: String::new(),
            category_parent_id: None,
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
            confirm_rescan: false,
            focus_disk_name: false,
            disk_draft: DiskPropertiesDraft::default(),
            disk_preferences: HashMap::new(),
            shelf_collisions: HashSet::new(),
            preferences_ready: false,
            status: StatusText::new(
                "Готово · только локальные метаданные",
                "Ready · local metadata only",
            ),
            error: None,
            lang: Language::Russian,
            dark: true,
            auto_open: startup_catalog.is_some(),
        }
    }

    fn t(&self, ru: &'static str, en: &'static str) -> &'static str {
        self.lang.text(ru, en)
    }

    fn switch_workspace(&mut self, tab: WorkspaceTab) -> bool {
        if self.dialog_open() || !tab.available() {
            return false;
        }
        if self.workspace_tab == tab {
            return true;
        }
        self.workspace_tab = tab;
        self.workspace_tab_focus_request = true;
        if tab == WorkspaceTab::Search {
            self.restore_scroll = Some(self.results_scroll);
        }
        true
    }

    fn queue(&mut self, purpose: Purpose, request: Value, ctx: &egui::Context) {
        // Do not supersede an I/O worker: a stale write could still complete
        // even if its reply were discarded. Navigation remains disabled while busy.
        if self.active_request.is_some() {
            self.error = Some("Previous catalog operation is still running.".to_owned());
            return;
        }
        let mut request = request;
        let preview_long_job = if self.linked_ctf.as_ref().is_some_and(|link| {
            link.preview
                && matches!(
                    request["op"].as_str(),
                    Some(
                        "search_fast"
                            | "copy_candidates"
                            | "export_csv"
                            | "export_json"
                            | "export_parquet"
                    )
                )
        }) {
            let job_id = self.begin_ctf_job();
            self.ctf_phase = match request["op"].as_str() {
                Some("copy_candidates") => "preview_copies".to_owned(),
                Some("export_csv" | "export_json" | "export_parquet") => {
                    "preview_export".to_owned()
                }
                _ => "preview_search".to_owned(),
            };
            request["job_id"] = json!(job_id);
            Some(job_id)
        } else {
            None
        };
        let (purpose, request) = if let Some(link) = &self.linked_ctf {
            if matches!(
                &purpose,
                Purpose::Open { .. }
                    | Purpose::Create { .. }
                    | Purpose::ImportCtf { .. }
                    | Purpose::ExportCtf { .. }
                    | Purpose::ShelfLabels { .. }
                    | Purpose::ShelfSave { .. }
            ) {
                (purpose, request)
            } else {
                let mapped = match link.rewrite(request) {
                    Ok(mapped) => mapped,
                    Err(error) => {
                        if preview_long_job.is_some() {
                            self.ctf_job_id = None;
                        }
                        self.error = Some(error);
                        return;
                    }
                };
                (
                    Purpose::LinkedRead {
                        inner: Box::new(purpose),
                        generation: link.generation.clone(),
                        job_id: preview_long_job,
                    },
                    mapped,
                )
            }
        } else {
            (purpose, request)
        };
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
        if path.to_ascii_lowercase().ends_with(".ctf") {
            if create {
                self.error = Some(
                    self.t(
                        "Новый рабочий каталог должен иметь расширение .sqlite.",
                        "A new working catalog must use .sqlite.",
                    )
                    .to_owned(),
                );
            } else {
                self.start_link_ctf(path, ctx);
            }
            return;
        }
        let purpose = if create {
            Purpose::Create { path: path.clone() }
        } else {
            Purpose::Open { path: path.clone() }
        };
        let op = if create { "init" } else { "probe_catalog" };
        self.status = StatusText::new("Открытие каталога…", "Opening catalog…");
        self.queue(purpose, json!({"op":op,"db":path}), ctx);
        self.show_catalog_dialog = false;
    }

    fn begin_ctf_job(&mut self) -> String {
        let id = new_scan_job_id();
        self.ctf_job_id = Some(id.clone());
        self.ctf_progress_request = None;
        self.ctf_cancel_request = None;
        self.ctf_cancel_requested = false;
        self.ctf_job_registered = false;
        self.ctf_phase = "preparing".into();
        self.ctf_entries = 0;
        self.ctf_last_poll = None;
        id
    }

    fn ctf_phase_label(&self) -> &'static str {
        match self.ctf_phase.as_str() {
            "reading_source" | "verifying_source" => {
                self.t("Проверка исходника", "Checking source")
            }
            "parsing" => self.t("Чтение структуры CTF", "Reading CTF structure"),
            "importing" => self.t("Импорт записей", "Importing entries"),
            "indexing" => self.t("Построение индекса", "Building index"),
            "validating" | "verifying_cache" => self.t("Проверка результата", "Checking result"),
            "reading_catalog" => self.t("Чтение каталога", "Reading catalog"),
            "encoding" => self.t("Подготовка CTF", "Preparing CTF"),
            "preview_search" => self.t("Поиск в CTF", "Searching CTF"),
            "preview_copies" => self.t("Поиск кандидатов", "Finding copy candidates"),
            "preview_copy_candidates" => self.t("Поиск кандидатов", "Finding copy candidates"),
            "preview_export" => self.t("Экспорт CTF", "Exporting CTF metadata"),
            "writing" | "publishing" => self.t("Сохранение результата", "Saving result"),
            _ => self.t("Подготовка", "Preparing"),
        }
    }

    fn start_link_ctf(&mut self, path: String, ctx: &egui::Context) {
        // Direct reads have no writable-cache prerequisite. Keep the optional
        // locator only for the retained legacy adapter; opening creates no DB.
        let cache_root = user_store()
            .and_then(|store| store.parent().map(|parent| parent.join("ctf-cache")))
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.show_catalog_dialog = false;
        self.status = StatusText::new("Открытие каталога CTF…", "Opening CTF catalog…");
        self.error = None;
        self.active_request = Some(self.bus.send(
            Purpose::CtfPreviewOpen {
                path: path.clone(),
                cache_root,
            },
            json!({"op":"ctf_preview_open","path":path}),
            ctx,
        ));
    }

    fn start_ctf_transfer(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        let Some(mode) = self.ctf_dialog else {
            return;
        };
        let target = self.ctf_target_input.trim().to_owned();
        let file = Path::new(&target);
        let expected = if mode == CtfDialog::Import {
            "sqlite"
        } else {
            "ctf"
        };
        if target.is_empty()
            || file
                .extension()
                .and_then(|x| x.to_str())
                .is_none_or(|x| !x.eq_ignore_ascii_case(expected))
            || file.exists()
            || file.is_symlink()
            || !file
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .is_dir()
        {
            self.error = Some(self.t("Выберите существующую папку и новое имя с нужным расширением. Перезапись запрещена.", "Choose an existing folder and a new filename with the required extension. Overwrite is forbidden.").into());
            return;
        }
        let target = if file.is_absolute() {
            target
        } else {
            match std::env::current_dir() {
                Ok(p) => p.join(file).to_string_lossy().into_owned(),
                Err(e) => {
                    self.error = Some(e.to_string());
                    return;
                }
            }
        };
        if mode == CtfDialog::Import {
            let source = self.ctf_source_input.trim().to_owned();
            if !Path::new(&source).is_file() || !source.to_ascii_lowercase().ends_with(".ctf") {
                self.error = Some(
                    self.t("Выберите существующий CTF.", "Choose an existing CTF.")
                        .into(),
                );
                return;
            }
            let job = self.begin_ctf_job();
            let mut request = json!({"op":"import_ctf_new","path":source,"db":target,"job_id":job});
            if let Some(link) = &self.linked_ctf {
                if std::fs::canonicalize(&link.path)
                    .ok()
                    .zip(std::fs::canonicalize(&source).ok())
                    .is_some_and(|(a, b)| a == b)
                {
                    request["expected_source_sha256"] = json!(link.source_sha256);
                }
            }
            self.ctf_dialog = None;
            self.status = StatusText::new(
                "Импорт CTF в новый каталог…",
                "Importing CTF into a new catalog…",
            );
            self.queue(Purpose::ImportCtf { output: target }, request, ctx);
        } else {
            if self.linked_ctf.is_some() {
                self.error = Some(
                    self.t(
                        "Сначала импортируйте CTF в новый рабочий каталог.",
                        "Import CTF into a new working catalog first.",
                    )
                    .into(),
                );
                return;
            }
            let Some(db) = self.db.clone() else {
                return;
            };
            let job = self.begin_ctf_job();
            self.ctf_dialog = None;
            self.status = StatusText::new("Экспорт нового CTF…", "Exporting a new CTF…");
            self.queue(Purpose::ExportCtf {output:target.clone()},json!({"op":"export_ctf","db":db,"path":target,"job_id":job,"overwrite":false,"allow_lossy":false}),ctx);
        }
    }

    fn poll_ctf_progress(&mut self, ctx: &egui::Context) {
        let Some(job) = self.ctf_job_id.clone() else {
            return;
        };
        if self.ctf_progress_request.is_none()
            && self
                .ctf_last_poll
                .is_none_or(|t| t.elapsed() >= Duration::from_millis(150))
        {
            self.ctf_last_poll = Some(Instant::now());
            self.ctf_progress_request = Some(self.bus.send(
                Purpose::CtfProgress {
                    job_id: job.clone(),
                },
                json!({"op":"ctf_progress","job_id":job}),
                ctx,
            ));
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn cancel_ctf_job(&mut self, ctx: &egui::Context) {
        let Some(job) = self.ctf_job_id.clone() else {
            return;
        };
        if self.ctf_cancel_request.is_some() || !self.ctf_job_registered {
            return;
        }
        self.ctf_cancel_requested = true;
        self.ctf_cancel_request = Some(self.bus.send(
            Purpose::CtfCancel {
                job_id: job.clone(),
            },
            json!({"op":"ctf_cancel","job_id":job}),
            ctx,
        ));
    }

    fn save_description(&mut self, ctx: &egui::Context) {
        if self.linked_ctf.is_some() || self.active_request.is_some() {
            return;
        }
        let (Some(id), Some(db)) = (self.description_entry_id, self.db.clone()) else {
            return;
        };
        self.queue(Purpose::SaveDescription {entry_id:id,db:db.clone()},json!({"op":"update_ctf_description","db":db,"entry_id":id,"description":self.description_input}),ctx);
    }

    fn check_catalog_integrity(&mut self, ctx: &egui::Context) {
        let Some(path) = self.db.clone().filter(|_| self.active_request.is_none()) else {
            return;
        };
        self.integrity = CatalogIntegrity::Checking;
        self.queue(
            Purpose::CheckCatalogIntegrity { path: path.clone() },
            json!({"op":"validate_catalog","db":path}),
            ctx,
        );
    }

    fn clear_catalog(&mut self) {
        self.ctf_job_id = None;
        self.ctf_progress_request = None;
        self.ctf_cancel_request = None;
        self.ctf_job_registered = false;
        self.ctf_cancel_requested = false;
        if let Some(link) = self.linked_ctf.as_ref().filter(|link| link.preview) {
            let _ = self.bus.send(
                Purpose::CtfPreviewClose,
                json!({"op":"ctf_preview_close","path":link.path,"generation":link.generation}),
                &egui::Context::default(),
            );
        }
        for (job_id, preview) in [
            (self.analytics_job_id.take(), self.analytics_preview_job),
            (self.slices_job_id.take(), self.slices_preview_job),
        ] {
            if let Some(job_id) = job_id {
                if preview {
                    let _ = atlas_service::cancel_ctf_preview_analytics_job(&job_id);
                } else {
                    let _ = atlas_service::cancel_analytics_job(&job_id);
                }
            }
        }
        self.analytics_preview_job = false;
        self.slices_preview_job = false;
        self.analytics_generation = self.analytics_generation.wrapping_add(1);
        self.slices_generation = self.slices_generation.wrapping_add(1);
        self.catalog_generation = self.catalog_generation.wrapping_add(1);
        self.tree_ancestors_request_id = None;
        self.tree_ancestors_target = None;
        self.tree_children.clear();
        self.tree_expanded.clear();
        self.tree_path_nodes.clear();
        self.tree_reveal_target = None;
        self.folder_crumbs.clear();
        self.analytics_request_id = None;
        self.slices_request_id = None;
        self.analytics_db = None;
        self.slices_db = None;
        self.analytics_data = None;
        self.slices_data = None;
        self.analytics_volume = None;
        self.analytics_folder = None;
        self.analytics_category_id = None;
        self.slices_volume = None;
        self.slices_category = None;
        self.analytics_loaded = false;
        self.slices_loaded = false;
        self.physical.close_if_scope_changed(None);
        self.db = None;
        self.workspace_tab = WorkspaceTab::Search;
        self.workspace_tab_focus_request = false;
        self.linked_ctf = None;
        self.ctf_dialog = None;
        self.description_entry_id = None;
        self.integrity = CatalogIntegrity::Closed;
        self.volumes.clear();
        self.shelf_context = None;
        self.shelf = ShelfSnapshot::default();
        self.shelf_editor.clear();
        self.legacy_shelf_editable = false;
        self.disk_preferences.clear();
        self.shelf_collisions.clear();
        self.preferences_ready = false;
        self.show_disk_properties = None;
        self.focus_disk_name = false;
        self.disk_draft = DiskPropertiesDraft::default();
        self.selected_volume = None;
        self.disks_filter.clear();
        self.disks_order = RegisterOrder::Id;
        self.disks_selected_volume = None;
        self.disks_scroll = 0.0;
        self.query_input.clear();
        self.route = Route::Catalog;
        self.history.clear();
        self.page = Page::default();
        self.offset = 0;
        self.selected = None;
        self.restore_selection = None;
        self.reveal_selected = false;
        self.results_scroll = 0.0;
        self.restore_scroll = None;
        self.categories.clear();
        self.categories_loaded = false;
        self.category_assigned.clear();
        self.category_assignments_ready = false;
        self.category_assignments_error = false;
        self.category_result_refresh_pending = false;
        self.category_parent_id = None;
        self.category_name_input.clear();
        self.show_categories_dialog = false;
        self.show_export_dialog = false;
        self.show_scan_dialog = false;
    }

    fn start_export(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() || self.workspace_tab != WorkspaceTab::Search {
            return;
        }
        let Some(db) = self.db.clone() else {
            return;
        };
        self.status = StatusText::new("Операция отклонена", "Operation refused");
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
        self.status = StatusText::new(
            "Экспорт полного набора метаданных…",
            "Exporting full metadata scope…",
        );
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
        self.status = StatusText::new("Операция отклонена", "Operation refused");
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
        self.status = StatusText::new(
            "Сканирование метаданных запущено…",
            "Metadata scan started…",
        );
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

    #[allow(dead_code)] // Deferred action retained for the release-scope guard test.
    fn start_rescan(&mut self, volume_id: i64, ctx: &egui::Context) {
        // Bound-device rescans are outside the Windows 1.0 release scope.
        // Keep ordinary metadata scanning available through start_scan.
        let _ = (volume_id, ctx);
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
        self.status = StatusText::new(
            "Запрос отмены сканирования…",
            "Requesting scan cancellation…",
        );
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
            self.status = StatusText::new("Загрузка реестра дисков…", "Loading disk register…");
            self.queue(Purpose::Volumes, json!({"op":"volumes","db":db}), ctx);
        }
    }

    fn load_disk_preferences(&mut self, ctx: &egui::Context) {
        if self.linked_ctf.is_some() {
            self.preferences_ready = false;
            self.legacy_shelf_editable = false;
            self.load_shelf_labels(ctx, true);
            return;
        }
        self.preferences_ready = false;
        self.legacy_shelf_editable = false;
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
            self.load_shelf_labels(ctx, true);
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

    fn load_shelf_labels(&mut self, ctx: &egui::Context, initial: bool) {
        let Some(scope) = self.shelf_context.clone() else {
            self.shelf = ShelfSnapshot::default();
            if initial {
                self.load_page(ctx);
            }
            return;
        };
        let ids: Vec<i64> = self.volumes.iter().map(|v| v.id).collect();
        let catalog_key = scope.catalog_key.clone();
        self.queue(
            Purpose::ShelfLabels {
                catalog_key,
                initial,
            },
            scope.list_request(&ids),
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
        self.confirm_rescan = false;
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
        self.shelf_editor = self.shelf.label(id).to_owned();
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

    fn save_shelf_label(&mut self, volume_id: i64, ctx: &egui::Context) {
        if !self.legacy_shelf_editable
            || self.active_request.is_some()
            || !self.volumes.iter().any(|v| v.id == volume_id)
        {
            return;
        }
        let Some(db) = self.db.clone() else {
            return;
        };
        let label = self.shelf_editor.trim().to_owned();
        if !valid_label(&label) {
            self.error = Some(
                self.t(
                    "Наклейка должна быть одной строкой, максимум 80 символов.",
                    "A shelf tag must be a single line, maximum 80 characters.",
                )
                .to_owned(),
            );
            return;
        }
        let Some(scope) = self.shelf_context.clone() else {
            self.error = Some(
                self.t(
                    "Личное хранилище наклеек недоступно; данные каталога не изменены.",
                    "Private shelf-label store unavailable; catalog is unchanged.",
                )
                .to_owned(),
            );
            return;
        };
        let catalog_key = scope.catalog_key.clone();
        self.status = StatusText::new(
            "Сохраняем наклейку отдельно от каталога…",
            "Saving the shelf tag outside the catalog…",
        );
        self.queue(
            Purpose::ShelfSave {
                catalog_key,
                volume_id,
            },
            scope.save_request(&db, volume_id, &label, self.shelf.label(volume_id)),
            ctx,
        );
    }

    fn load_page(&mut self, ctx: &egui::Context) {
        if matches!(&self.route, Route::Catalog) {
            self.page = Page::default();
            self.offset = 0;
            self.selected = None;
            return;
        }
        if let Some(db) = self.db.clone() {
            let route = self.route.clone();
            let request = route.request(&db, self.offset, PAGE_LIMIT);
            self.status = StatusText::new("Поиск по каталогу…", "Searching catalog…");
            self.queue(Purpose::Page { route }, request, ctx);
        }
    }

    fn request_tree_children(&mut self, key: FolderKey, offset: u64, ctx: &egui::Context) {
        let Some(db) = self.db.clone() else {
            return;
        };
        let page = self.tree_children.entry(key).or_default();
        if page.loading_request.is_some() || (offset > 0 && page.next_offset != offset) {
            return;
        }
        let mut request = json!({"op":"children","db":db,"volume_id":key.0,
            "parent_id":key.1,"offset":offset,"limit":PAGE_LIMIT});
        let linked_generation = self.linked_ctf.as_ref().map(|link| link.generation.clone());
        if let Some(link) = &self.linked_ctf {
            match link.rewrite(request) {
                Ok(mapped) => request = mapped,
                Err(error) => {
                    self.tree_children.entry(key).or_default().error = Some(error);
                    return;
                }
            }
        }
        let request_id = self.bus.send(
            Purpose::TreeChildren {
                db,
                catalog_generation: self.catalog_generation,
                linked_generation,
                volume_id: key.0,
                parent_id: key.1,
                offset,
            },
            request,
            ctx,
        );
        let page = self.tree_children.entry(key).or_default();
        page.loading_request = Some(request_id);
        page.loading_offset = Some(offset);
        page.error = None;
    }

    fn request_tree_ancestors(&mut self, volume_id: i64, entry_id: i64, ctx: &egui::Context) {
        let Some(db) = self.db.clone() else {
            return;
        };
        let mut request = json!({"op":"entry_ancestors","db":db,
            "volume_id":volume_id,"entry_id":entry_id});
        let linked_generation = self.linked_ctf.as_ref().map(|link| link.generation.clone());
        if let Some(link) = &self.linked_ctf {
            match link.rewrite(request) {
                Ok(mapped) => request = mapped,
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }
        let request_id = self.bus.send(
            Purpose::TreeAncestors {
                db,
                catalog_generation: self.catalog_generation,
                linked_generation,
                volume_id,
                entry_id,
            },
            request,
            ctx,
        );
        self.tree_ancestors_request_id = Some(request_id);
        self.tree_ancestors_target = Some((volume_id, entry_id));
    }

    fn push_history(&mut self) {
        self.history.push((
            self.route.clone(),
            self.offset,
            self.selected.clone(),
            self.selected_volume,
            self.results_scroll,
            self.folder_crumbs.clone(),
            self.query_input.clone(),
        ));
    }

    fn toggle_tree_node(&mut self, key: FolderKey, ctx: &egui::Context) {
        if !self.tree_expanded.remove(&key) {
            self.tree_expanded.insert(key);
            let should_load = self
                .tree_children
                .get(&key)
                .is_none_or(|page| page.items.is_empty() && page.next_offset == 0);
            if should_load {
                self.request_tree_children(key, 0, ctx);
            }
        }
    }

    fn draw_tree_children(
        &mut self,
        ui: &mut egui::Ui,
        key: FolderKey,
        depth: usize,
        ctx: &egui::Context,
    ) {
        if depth > 255 {
            ui.label(self.t(
                "Достигнута максимальная глубина папок.",
                "Maximum folder depth reached.",
            ));
            return;
        }
        let mut rows = self
            .tree_children
            .get(&key)
            .map(|page| page.items.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(Entry::is_container)
            .map(|entry| BrowseCrumb {
                id: entry.id,
                parent_id: entry.parent_id,
                name: entry.name,
                kind: entry.kind,
            })
            .collect::<Vec<_>>();
        if let Some(path_nodes) = self.tree_path_nodes.get(&key) {
            for crumb in path_nodes {
                if !rows.iter().any(|entry| entry.id == crumb.id) {
                    rows.push(crumb.clone());
                }
            }
        }
        for crumb in rows {
            let child_key = (key.0, Some(crumb.id));
            let expanded = self.tree_expanded.contains(&child_key);
            ui.horizontal(|ui| {
                ui.add_space((depth.min(18) * 14) as f32);
                if ui.small_button(if expanded { "▾" } else { "▸" }).clicked() {
                    self.toggle_tree_node(child_key, ctx);
                }
                let response = ui
                    .selectable_label(
                        matches!(&self.route, Route::Folder { volume_id, parent_id: Some(id) }
                            if *volume_id == key.0 && *id == crumb.id),
                        &crumb.name,
                    )
                    .on_hover_text(&crumb.name);
                if self.tree_reveal_target == Some(child_key) {
                    response.scroll_to_me(Some(egui::Align::Center));
                    // Ancestors are pinned before lazy sibling pages arrive.
                    // Those pages can insert rows above this node on a later
                    // frame, so retain the reveal until the layout settles.
                    if self
                        .tree_children
                        .values()
                        .all(|page| page.loading_request.is_none())
                    {
                        self.tree_reveal_target = None;
                    }
                }
                if response.clicked() {
                    self.navigate_tree_folder(key.0, crumb.id, ctx);
                }
            });
            if expanded {
                self.draw_tree_children(ui, child_key, depth.saturating_add(1), ctx);
            }
        }
        if let Some(page) = self.tree_children.get(&key).cloned() {
            if page.loading_request.is_some() {
                ui.horizontal(|ui| {
                    ui.add_space((depth.saturating_add(1).min(18) * 14) as f32);
                    ui.spinner();
                    ui.small(self.t("Загрузка папок…", "Loading folders…"));
                });
            } else if page.has_more
                && ui
                    .small_button(self.t("Ещё папки…", "More folders…"))
                    .clicked()
            {
                self.request_tree_children(key, page.next_offset, ctx);
            } else if let Some(error) = page.error {
                ui.colored_label(Color32::from_rgb(224, 96, 88), error);
            }
        }
    }

    fn navigate_tree_folder(&mut self, volume_id: i64, entry_id: i64, ctx: &egui::Context) {
        if self.active_request.is_some()
            || entry_id <= 0
            || !self.volumes.iter().any(|volume| volume.id == volume_id)
        {
            return;
        }
        self.push_history();
        self.selected_volume = Some(volume_id);
        self.query_input.clear();
        self.folder_crumbs.clear();
        self.route = Route::Folder {
            volume_id,
            parent_id: Some(entry_id),
        };
        self.offset = 0;
        self.page = Page::default();
        self.selected = None;
        self.restore_selection = None;
        self.reveal_selected = false;
        self.request_tree_ancestors(volume_id, entry_id, ctx);
        self.load_page(ctx);
    }

    fn browse_up(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        let Route::Folder {
            volume_id,
            parent_id: Some(current_id),
        } = &self.route
        else {
            return;
        };
        let volume_id = *volume_id;
        let current_id = *current_id;
        if !self
            .folder_crumbs
            .last()
            .is_some_and(|crumb| crumb.id == current_id)
        {
            self.request_tree_ancestors(volume_id, current_id, ctx);
            return;
        }
        self.push_history();
        let current = self.folder_crumbs.pop();
        self.route = Route::Folder {
            volume_id,
            parent_id: current.and_then(|crumb| crumb.parent_id),
        };
        self.offset = 0;
        self.selected = None;
        self.load_page(ctx);
    }

    fn navigate_breadcrumb(&mut self, volume_id: i64, index: Option<usize>, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        self.push_history();
        self.selected_volume = Some(volume_id);
        self.query_input.clear();
        if let Some(index) = index {
            if let Some(crumb) = self.folder_crumbs.get(index).cloned() {
                self.folder_crumbs.truncate(index + 1);
                self.route = Route::Folder {
                    volume_id,
                    parent_id: Some(crumb.id),
                };
            } else {
                return;
            }
        } else {
            self.folder_crumbs.clear();
            self.route = Route::Folder {
                volume_id,
                parent_id: None,
            };
        }
        self.offset = 0;
        self.page = Page::default();
        self.selected = None;
        self.restore_selection = None;
        self.reveal_selected = false;
        self.load_page(ctx);
    }

    fn apply_tree_reply(&mut self, reply: crate::bridge::Reply, ctx: &egui::Context) {
        let (db, catalog_generation, linked_generation) = match &reply.purpose {
            Purpose::TreeChildren {
                db,
                catalog_generation,
                linked_generation,
                ..
            }
            | Purpose::TreeAncestors {
                db,
                catalog_generation,
                linked_generation,
                ..
            } => (db.clone(), *catalog_generation, linked_generation.clone()),
            _ => return,
        };
        let link = linked_generation.as_ref().and_then(|_| {
            self.linked_ctf.clone().filter(|link| {
                link.path == db && Some(&link.generation) == linked_generation.as_ref()
            })
        });
        let scope_current = self.catalog_generation == catalog_generation
            && self.db.as_deref() == Some(db.as_str())
            && (linked_generation.is_none() || link.is_some());
        if !scope_current {
            return;
        }
        let request_current = match &reply.purpose {
            Purpose::TreeChildren {
                volume_id,
                parent_id,
                offset,
                ..
            } => self
                .tree_children
                .get(&(*volume_id, *parent_id))
                .is_some_and(|page| {
                    page.loading_request == Some(reply.id) && page.loading_offset == Some(*offset)
                }),
            Purpose::TreeAncestors {
                volume_id,
                entry_id,
                ..
            } => {
                let route_target_matches = match &self.route {
                    Route::Folder {
                        volume_id: current_volume,
                        parent_id: Some(current_entry),
                    } => *current_volume == *volume_id && *current_entry == *entry_id,
                    Route::BranchSearch {
                        volume_id: current_volume,
                        parent_id,
                        ..
                    } => *current_volume == *volume_id && *parent_id == *entry_id,
                    _ => false,
                };
                self.tree_ancestors_request_id == Some(reply.id)
                    && self.tree_ancestors_target == Some((*volume_id, *entry_id))
                    && route_target_matches
            }
            _ => false,
        };
        if !request_current {
            return;
        }

        if reply.response["ok"] != true {
            let code = reply
                .response
                .pointer("/error/code")
                .and_then(Value::as_str);
            let message = reply
                .response
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Folder navigation request failed")
                .to_owned();
            match &reply.purpose {
                Purpose::TreeChildren {
                    volume_id,
                    parent_id,
                    offset,
                    ..
                } => {
                    let page = self
                        .tree_children
                        .entry((*volume_id, *parent_id))
                        .or_default();
                    if page.loading_request == Some(reply.id)
                        && page.loading_offset == Some(*offset)
                    {
                        page.loading_request = None;
                        page.loading_offset = None;
                        page.error = Some(message.clone());
                    } else {
                        return;
                    }
                }
                Purpose::TreeAncestors { .. } => {
                    if self.tree_ancestors_request_id != Some(reply.id) {
                        return;
                    }
                    self.tree_ancestors_request_id = None;
                    self.tree_ancestors_target = None;
                    self.error = Some(message.clone());
                }
                _ => return,
            }
            if matches!(
                code,
                Some(
                    "source_changed"
                        | "cache_changed"
                        | "cache_corrupt"
                        | "cache_stale"
                        | "not_pinned"
                )
            ) {
                self.clear_catalog();
                self.error = Some(message);
            }
            ctx.request_repaint();
            return;
        }
        let data = if linked_generation.is_some() {
            match link
                .as_ref()
                .and_then(|link| link.unwrap_reply(&reply.response).ok())
            {
                Some(normalized) => normalized["data"].clone(),
                None => {
                    match &reply.purpose {
                        Purpose::TreeChildren {
                            volume_id,
                            parent_id,
                            ..
                        } => {
                            let page = self
                                .tree_children
                                .entry((*volume_id, *parent_id))
                                .or_default();
                            page.loading_request = None;
                            page.loading_offset = None;
                            page.error = Some("Folder response has stale source identity".into());
                        }
                        Purpose::TreeAncestors { .. } => {
                            self.tree_ancestors_request_id = None;
                            self.tree_ancestors_target = None;
                            self.error = Some("Ancestor response has stale source identity".into());
                        }
                        _ => {}
                    }
                    ctx.request_repaint();
                    return;
                }
            }
        } else {
            reply.response["data"].clone()
        };

        match reply.purpose {
            Purpose::TreeChildren {
                volume_id,
                parent_id,
                offset,
                ..
            } => {
                let key = (volume_id, parent_id);
                let page_state = self.tree_children.entry(key).or_default();
                if page_state.loading_request != Some(reply.id)
                    || page_state.loading_offset != Some(offset)
                    || (offset > 0 && page_state.next_offset != offset)
                {
                    return;
                }
                match Page::parse(&data) {
                    Ok(page) => {
                        let next_offset = page.offset.saturating_add(page.items.len() as u64);
                        if offset == 0 {
                            page_state.items = page.items;
                        } else {
                            page_state.items.extend(page.items);
                        }
                        // The server offset counts files as well as folders.
                        page_state.next_offset = next_offset;
                        page_state.has_more = page.has_more;
                        page_state.loading_request = None;
                        page_state.loading_offset = None;
                        page_state.error = None;
                    }
                    Err(error) => {
                        page_state.loading_request = None;
                        page_state.loading_offset = None;
                        page_state.error = Some(error);
                    }
                }
            }
            Purpose::TreeAncestors {
                volume_id,
                entry_id,
                ..
            } => {
                let target_current = match &self.route {
                    Route::Folder {
                        volume_id: current_volume,
                        parent_id: Some(current_entry),
                    } => *current_volume == volume_id && *current_entry == entry_id,
                    Route::BranchSearch {
                        volume_id: current_volume,
                        parent_id,
                        ..
                    } => *current_volume == volume_id && *parent_id == entry_id,
                    _ => false,
                };
                if self.tree_ancestors_request_id != Some(reply.id)
                    || self.tree_ancestors_target != Some((volume_id, entry_id))
                    || !target_current
                {
                    return;
                }
                self.tree_ancestors_request_id = None;
                self.tree_ancestors_target = None;
                let parsed = data
                    .get("items")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "Ancestors response is missing items[]".to_owned())
                    .and_then(|items| {
                        items
                            .iter()
                            .map(|item| {
                                Ok(BrowseCrumb {
                                    id: item["id"]
                                        .as_i64()
                                        .filter(|id| *id > 0)
                                        .ok_or_else(|| "Invalid ancestor id".to_owned())?,
                                    parent_id: item["parent_id"].as_i64(),
                                    name: item["name"]
                                        .as_str()
                                        .ok_or_else(|| "Invalid ancestor name".to_owned())?
                                        .to_owned(),
                                    kind: item["kind"]
                                        .as_str()
                                        .ok_or_else(|| "Invalid ancestor kind".to_owned())?
                                        .to_owned(),
                                })
                            })
                            .collect::<Result<Vec<_>, String>>()
                    });
                match parsed {
                    Ok(crumbs)
                        if data["volume_id"].as_i64() == Some(volume_id)
                            && crumbs.last().is_some_and(|crumb| crumb.id == entry_id)
                            && crumbs
                                .windows(2)
                                .all(|pair| pair[1].parent_id == Some(pair[0].id)) =>
                    {
                        self.folder_crumbs = crumbs.clone();
                        self.tree_reveal_target = Some((volume_id, Some(entry_id)));
                        let mut parent_id = None;
                        let mut load_keys = vec![(volume_id, None)];
                        self.tree_expanded.insert((volume_id, None));
                        for crumb in crumbs {
                            let key = (volume_id, parent_id);
                            let nodes = self.tree_path_nodes.entry(key).or_default();
                            if !nodes.iter().any(|node| node.id == crumb.id) {
                                nodes.push(crumb.clone());
                            }
                            let child_key = (volume_id, Some(crumb.id));
                            self.tree_expanded.insert(child_key);
                            load_keys.push(child_key);
                            parent_id = Some(crumb.id);
                        }
                        for key in load_keys {
                            if self
                                .tree_children
                                .get(&key)
                                .is_none_or(|page| page.next_offset == 0 && page.items.is_empty())
                            {
                                self.request_tree_children(key, 0, ctx);
                            }
                        }
                    }
                    Ok(_) => {
                        self.error =
                            Some("Ancestor response did not match the selected folder".into())
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            _ => {}
        }
        ctx.request_repaint();
    }

    fn load_categories(&mut self, ctx: &egui::Context) {
        self.categories_loaded = false;
        self.categories.clear();
        if let Some(db) = self.db.clone() {
            self.queue(Purpose::Categories, categories::list_request(&db), ctx);
        }
    }

    fn load_assigned_categories(&mut self, ctx: &egui::Context) {
        self.category_assigned.clear();
        self.category_assignments_ready = false;
        self.category_assignments_error = false;
        let (Some(db), Some(entry_id)) = (
            self.db.clone(),
            self.selected.as_ref().map(|entry| entry.id),
        ) else {
            return;
        };
        self.queue(
            Purpose::EntryCategoryIds { entry_id },
            categories::list_assigned_request(&db, entry_id),
            ctx,
        );
    }

    /// Reload results from SQLite after a category write or a stale-entry reply.
    /// An existing selection survives only when the refreshed page contains it.
    fn refresh_category_results(&mut self, ctx: &egui::Context) {
        self.category_assigned.clear();
        self.category_assignments_ready = false;
        self.category_assignments_error = false;
        self.category_result_refresh_pending = true;
        self.restore_selection = self.selected.clone();
        self.page = Page::default();
        self.load_page(ctx);
    }

    fn unassign_category(&mut self, entry_id: i64, category_id: i64, ctx: &egui::Context) {
        if self.active_request.is_some()
            || !self.category_assignments_ready
            || !self.category_assigned.contains(&category_id)
            || !self
                .selected
                .as_ref()
                .is_some_and(|entry| entry.id == entry_id)
        {
            return;
        }
        let Some(db) = self.db.clone() else {
            return;
        };
        self.queue(
            Purpose::UnassignCategory {
                entry_id,
                category_id,
            },
            categories::unassign_request(&db, entry_id, category_id),
            ctx,
        );
    }

    fn create_category(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() || !self.categories_loaded {
            return;
        }
        let name = match categories::new_name(&self.category_name_input) {
            Ok(name) => name.to_owned(),
            Err(_) => {
                self.error = Some(
                    self.t(
                        "Название категории должно содержать 1–80 печатных символов.",
                        "Category name must contain 1–80 printable characters.",
                    )
                    .to_owned(),
                );
                return;
            }
        };
        let Some(db) = self.db.clone() else {
            return;
        };
        self.queue(
            Purpose::CreateCategory { name: name.clone() },
            categories::create_request(&db, &name, self.category_parent_id),
            ctx,
        );
    }

    fn assign_category(&mut self, entry_id: i64, category_id: i64, ctx: &egui::Context) {
        if self.active_request.is_some()
            || entry_id <= 0
            || category_id <= 0
            || !self.category_assignments_ready
            || self.category_assigned.contains(&category_id)
            || !self
                .selected
                .as_ref()
                .is_some_and(|entry| entry.id == entry_id)
        {
            return;
        }
        let Some(db) = self.db.clone() else {
            return;
        };
        if !self
            .categories
            .iter()
            .any(|category| category.id == category_id)
        {
            return;
        }
        self.queue(
            Purpose::AssignCategory {
                entry_id,
                category_id,
            },
            categories::assign_request(&db, entry_id, category_id),
            ctx,
        );
    }

    fn navigate_category(&mut self, category: &Category, ctx: &egui::Context) {
        if self.active_request.is_some()
            || !self.categories_loaded
            || !self
                .categories
                .iter()
                .any(|listed| listed.id == category.id)
        {
            return;
        }
        self.push_history();
        self.folder_crumbs.clear();
        self.query_input = category.search_query();
        self.route = Route::Search {
            query: self.query_input.clone(),
            volume_id: None,
        };
        self.selected_volume = None;
        self.offset = 0;
        self.page = Page::default();
        self.selected = None;
        self.restore_selection = None;
        self.reveal_selected = false;
        self.restore_scroll = None;
        self.load_page(ctx);
    }

    fn navigate_search(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        let query = self.query_input.trim().to_owned();
        let previous = self.route.clone();
        if query.is_empty() {
            match previous {
                Route::Folder { .. } => {}
                Route::BranchSearch {
                    volume_id,
                    parent_id,
                    ..
                } => {
                    self.route = Route::Folder {
                        volume_id,
                        parent_id: Some(parent_id),
                    };
                }
                Route::Catalog => {
                    self.selected_volume = None;
                }
                Route::Search {
                    volume_id: None, ..
                } => {
                    self.route = Route::Catalog;
                    self.selected_volume = None;
                    self.folder_crumbs.clear();
                }
                Route::Search {
                    volume_id: Some(volume_id),
                    ..
                } => {
                    self.selected_volume = Some(volume_id);
                    self.route = Route::Folder {
                        volume_id,
                        parent_id: None,
                    };
                    self.folder_crumbs.clear();
                }
                _ => {
                    self.route = Route::Catalog;
                    self.selected_volume = None;
                    self.folder_crumbs.clear();
                }
            }
        } else {
            self.route = match previous {
                Route::Folder {
                    volume_id,
                    parent_id: Some(parent_id),
                } => Route::BranchSearch {
                    volume_id,
                    parent_id,
                    query: query.clone(),
                    exact_category_id: None,
                },
                Route::BranchSearch {
                    volume_id,
                    parent_id,
                    exact_category_id,
                    ..
                } => Route::BranchSearch {
                    volume_id,
                    parent_id,
                    query: query.clone(),
                    exact_category_id,
                },
                Route::Folder {
                    volume_id,
                    parent_id: None,
                } => Route::Search {
                    query: query.clone(),
                    volume_id: Some(volume_id),
                },
                _ => Route::Search {
                    query: query.clone(),
                    volume_id: self.selected_volume,
                },
            };
        }
        self.restore_selection = None;
        self.reveal_selected = false;
        self.offset = 0;
        self.selected = None;
        self.page = Page::default();
        self.load_page(ctx);
    }

    fn submit_search(&mut self, ctx: &egui::Context) {
        if self.dialog_open() {
            return;
        }
        self.navigate_search(ctx);
    }

    fn navigate_disk(&mut self, volume_id: Option<i64>, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        if let Some(volume_id) = volume_id {
            self.browse_volume_root(volume_id, ctx);
            return;
        }
        self.selected_volume = None;
        let query = self.query_input.trim().to_owned();
        self.folder_crumbs.clear();
        self.offset = 0;
        self.page = Page::default();
        self.selected = None;
        self.restore_selection = None;
        if query.is_empty() {
            self.route = Route::Catalog;
        } else {
            self.route = Route::Search {
                query,
                volume_id: None,
            };
        }
        self.load_page(ctx);
    }

    fn browse_volume_root(&mut self, volume_id: i64, ctx: &egui::Context) {
        if self.active_request.is_some()
            || !self.volumes.iter().any(|volume| volume.id == volume_id)
        {
            return;
        }
        self.push_history();
        self.query_input.clear();
        self.selected_volume = Some(volume_id);
        self.folder_crumbs.clear();
        self.query_input.clear();
        self.offset = 0;
        self.page = Page::default();
        self.selected = None;
        self.restore_selection = None;
        self.reveal_selected = false;
        self.results_scroll = 0.0;
        self.restore_scroll = Some(0.0);
        self.route = Route::Folder {
            volume_id,
            parent_id: None,
        };
        self.load_page(ctx);
    }

    fn jump_to_search_on_disk(&mut self, volume_id: i64, ctx: &egui::Context) {
        if self.dialog_open()
            || self.active_request.is_some()
            || !self.volumes.iter().any(|volume| volume.id == volume_id)
        {
            return;
        }
        self.switch_workspace(WorkspaceTab::Search);
        self.workspace_tab_focus_request = false;
        self.browse_volume_root(volume_id, ctx);
    }

    fn navigate_folder(&mut self, entry: &Entry, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        if !entry.is_container() {
            return;
        }
        self.push_history();
        self.restore_selection = None;
        self.query_input.clear();
        self.offset = 0;
        self.selected = None;
        self.route = Route::Folder {
            volume_id: entry.volume_id,
            parent_id: Some(entry.id),
        };
        self.selected_volume = Some(entry.volume_id);
        self.query_input.clear();
        self.folder_crumbs.clear();
        self.request_tree_ancestors(entry.volume_id, entry.id, ctx);
        self.load_page(ctx);
    }

    fn navigate_copies(&mut self, ctx: &egui::Context) {
        if self.active_request.is_some() {
            return;
        }
        if let Some(entry) = self.selected.clone() {
            if entry.is_container() {
                return;
            }
            self.push_history();
            self.restore_selection = None;
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
        if let Some((
            route,
            offset,
            selection,
            selected_volume,
            scroll,
            folder_crumbs,
            query_input,
        )) = self.history.pop()
        {
            self.reveal_selected = false;
            self.restore_scroll = Some(scroll);
            if let Route::Search { query, .. } | Route::BranchSearch { query, .. } = &route {
                self.query_input = query.clone();
            } else {
                self.query_input = query_input;
            }
            self.route = route;
            self.offset = offset;
            self.selected = None;
            self.restore_selection = selection;
            self.selected_volume = selected_volume;
            self.folder_crumbs = folder_crumbs;
            self.load_page(ctx);
        }
    }

    fn navigate_containing_folder(&mut self, entry: &Entry, ctx: &egui::Context) {
        if self.active_request.is_some() || entry.is_container() {
            return;
        }
        self.push_history();
        self.query_input.clear();
        self.route = Route::Folder {
            volume_id: entry.volume_id,
            parent_id: entry.parent_id,
        };
        self.selected_volume = Some(entry.volume_id);
        self.offset = 0;
        self.selected = None;
        self.restore_selection = Some(entry.clone());
        self.reveal_selected = true;
        self.folder_crumbs.clear();
        if let Some(parent_id) = entry.parent_id {
            self.tree_expanded.insert((entry.volume_id, None));
            self.request_tree_ancestors(entry.volume_id, parent_id, ctx);
        } else {
            self.folder_crumbs.clear();
            self.tree_expanded.insert((entry.volume_id, None));
            self.request_tree_children((entry.volume_id, None), 0, ctx);
        }
        if let Some(db) = self.db.clone() {
            self.queue(
                Purpose::Page {
                    route: self.route.clone(),
                },
                json!({
                    "op":"children_focused", "db":db, "volume_id":entry.volume_id,
                    "parent_id":entry.parent_id, "entry_id":entry.id, "limit":PAGE_LIMIT
                }),
                ctx,
            );
        }
    }

    fn apply_reply(&mut self, mut reply: crate::bridge::Reply, ctx: &egui::Context) {
        if matches!(
            &reply.purpose,
            Purpose::TreeChildren { .. } | Purpose::TreeAncestors { .. }
        ) {
            self.apply_tree_reply(reply, ctx);
            return;
        }
        let linked_operation_job_id = match &reply.purpose {
            Purpose::LinkedRead { job_id, .. } => job_id.clone(),
            _ => None,
        };
        if self
            .local_analyzer
            .apply_reply(&mut reply, &mut self.bus, ctx, self.db.as_deref())
        {
            return;
        }
        if let Purpose::AnalyticsCancel = &reply.purpose {
            return;
        }
        if let Purpose::Analytics {
            job_id,
            generation,
            db,
            slices,
            linked_generation,
        } = &reply.purpose
        {
            let current = if *slices {
                self.slices_request_id == Some(reply.id)
                    && self.slices_generation == *generation
                    && self.slices_job_id.as_deref() == Some(job_id.as_str())
                    && self.slices_db.as_deref() == Some(db.as_str())
            } else {
                self.analytics_request_id == Some(reply.id)
                    && self.analytics_generation == *generation
                    && self.analytics_job_id.as_deref() == Some(job_id.as_str())
                    && self.analytics_db.as_deref() == Some(db.as_str())
            };
            let linked_current = linked_generation.as_ref().is_none_or(|source_generation| {
                self.linked_ctf.as_ref().is_some_and(|link| {
                    link.preview && link.path == *db && link.generation == *source_generation
                })
            });
            if !current || self.db.as_deref() != Some(db.as_str()) || !linked_current {
                return;
            }
            if *slices {
                self.slices_request_id = None;
                self.slices_job_id = None;
                self.slices_preview_job = false;
                self.slices_loaded = true;
            } else {
                self.analytics_request_id = None;
                self.analytics_job_id = None;
                self.analytics_preview_job = false;
                self.analytics_loaded = true;
            }
            if reply.response["ok"] != true {
                let code = reply
                    .response
                    .pointer("/error/code")
                    .and_then(Value::as_str);
                let cancelled = code == Some("cancelled");
                let error = reply
                    .response
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("Analytics request failed")
                    .to_owned();
                if *slices {
                    self.slices_cancelled = cancelled;
                    if !cancelled {
                        self.slices_error = Some(error);
                    }
                } else {
                    self.analytics_cancelled = cancelled;
                    if !cancelled {
                        self.analytics_error = Some(error);
                    }
                }
            } else {
                let data = if linked_generation.is_some() {
                    match self
                        .linked_ctf
                        .as_ref()
                        .and_then(|link| link.unwrap_reply(&reply.response).ok())
                    {
                        Some(response) => response["data"].clone(),
                        None => {
                            if *slices {
                                self.slices_error =
                                    Some("CTF analytics response is stale or invalid".into());
                            } else {
                                self.analytics_error =
                                    Some("CTF analytics response is stale or invalid".into());
                            }
                            return;
                        }
                    }
                } else {
                    reply.response["data"].clone()
                };
                if data["root"].is_object()
                    && data["totals"].is_object()
                    && data["breadcrumbs"].is_array()
                {
                    if *slices {
                        self.slices_data = Some(Arc::new(data.clone()));
                        self.slices_error = None;
                        self.slices_cancelled = false;
                    } else {
                        self.analytics_data = Some(Arc::new(data.clone()));
                        self.analytics_error = None;
                        self.analytics_cancelled = false;
                    }
                } else if *slices {
                    self.slices_data = None;
                    self.slices_error = Some(
                        self.t(
                            "Сервис вернул неполный ответ аналитики.",
                            "Service returned an incomplete analytics response.",
                        )
                        .to_owned(),
                    );
                } else {
                    self.analytics_data = None;
                    self.analytics_error = Some(
                        self.t(
                            "Сервис вернул неполный ответ аналитики.",
                            "Service returned an incomplete analytics response.",
                        )
                        .to_owned(),
                    );
                }
            }
            ctx.request_repaint();
            return;
        }
        if let Purpose::CtfProgress { job_id } = &reply.purpose {
            if self.ctf_progress_request == Some(reply.id)
                && self.ctf_job_id.as_ref() == Some(job_id)
            {
                self.ctf_progress_request = None;
                if reply.response["ok"] == true {
                    self.ctf_job_registered = true;
                    self.ctf_phase = reply.response["data"]["phase"]
                        .as_str()
                        .unwrap_or("working")
                        .to_owned();
                    self.ctf_entries = reply.response["data"]["entries"].as_u64().unwrap_or(0);
                }
            }
            return;
        }
        if let Purpose::CtfCancel { job_id } = &reply.purpose {
            if self.ctf_cancel_request == Some(reply.id) && self.ctf_job_id.as_ref() == Some(job_id)
            {
                self.ctf_cancel_request = None;
                if reply.response["ok"] != true {
                    self.ctf_cancel_requested = false;
                }
            }
            return;
        }
        if matches!(&reply.purpose, Purpose::CtfPreviewClose) {
            return;
        }
        if let Purpose::LinkedRead {
            inner, generation, ..
        } = &reply.purpose
        {
            if self.active_request != Some(reply.id) {
                return;
            }
            let Some(link) = self
                .linked_ctf
                .clone()
                .filter(|x| &x.generation == generation)
            else {
                self.active_request = None;
                return;
            };
            if reply.response["ok"] != true {
                let code = reply
                    .response
                    .pointer("/error/code")
                    .and_then(Value::as_str);
                if matches!(
                    code,
                    Some(
                        "source_changed"
                            | "cache_changed"
                            | "cache_corrupt"
                            | "cache_stale"
                            | "not_pinned"
                    )
                ) {
                    let error = reply
                        .response
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("Linked CTF read failed")
                        .to_owned();
                    self.active_request = None;
                    self.clear_catalog();
                    self.error = Some(error);
                    return;
                }
                reply.purpose = (**inner).clone();
            } else {
                match link.unwrap_reply(&reply.response) {
                    Ok(normalized) => {
                        reply.response = normalized;
                        reply.purpose = (**inner).clone();
                    }
                    Err(error) => {
                        self.active_request = None;
                        self.clear_catalog();
                        self.error = Some(error);
                        return;
                    }
                }
            }
        }
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
                            StatusText::new(
                                "Отмена запрошена; ожидается откат транзакции.",
                                "Cancellation requested; waiting for rollback.",
                            )
                        } else {
                            StatusText::new(
                                "Отмена уже невозможна: операция завершается.",
                                "Cancellation is too late; operation is finishing.",
                            )
                        };
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
        if linked_operation_job_id
            .as_ref()
            .is_some_and(|job_id| self.ctf_job_id.as_ref() == Some(job_id))
        {
            self.ctf_job_id = None;
            self.ctf_progress_request = None;
            self.ctf_cancel_request = None;
            self.ctf_job_registered = false;
            self.ctf_cancel_requested = false;
        }
        let ctf_finished = matches!(
            &reply.purpose,
            Purpose::ImportCtf { .. } | Purpose::ExportCtf { .. }
        );
        if ctf_finished {
            self.ctf_job_id = None;
            self.ctf_progress_request = None;
            self.ctf_cancel_request = None;
            self.ctf_job_registered = false;
        }
        if ctf_finished
            && reply
                .response
                .pointer("/error/code")
                .and_then(Value::as_str)
                == Some("cancelled")
        {
            self.status = StatusText::new(
                "CTF-операция отменена; результат не опубликован.",
                "CTF operation cancelled; no result was published.",
            );
            return;
        }
        if let Purpose::CheckCatalogIntegrity { path } = &reply.purpose {
            if self.db.as_deref() != Some(path.as_str()) {
                return;
            }
            let data = &reply.response["data"];
            if reply.response["ok"] == true
                && matches!(data["schema_version"].as_i64(), Some(1..=3))
                && data["readonly"] == true
                && data["quick_check"] == "ok"
                && data["foreign_key_errors"].as_u64() == Some(0)
            {
                self.integrity = CatalogIntegrity::Verified;
            } else {
                self.clear_catalog();
                self.integrity = CatalogIntegrity::Failed;
                self.status = StatusText::new("Каталог закрыт", "Catalog closed");
                self.error = Some(
                    reply
                        .response
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or(self.t(
                            "Получен неверный результат проверки целостности.",
                            "Invalid integrity check response.",
                        ))
                        .to_owned(),
                );
            }
            return;
        }
        let scan_finished = matches!(
            &reply.purpose,
            Purpose::Scan { .. } | Purpose::Rescan { .. }
        );
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
            self.status = StatusText::new(
                "Сканирование отменено: снимок не сохранён.",
                "Scan cancelled: no snapshot was saved.",
            );
            return;
        }
        if reply.response.get("ok").and_then(Value::as_bool) != Some(true) {
            let code = reply
                .response
                .pointer("/error/code")
                .and_then(Value::as_str);
            let message = reply
                .response
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Unknown service error");
            let error_message = message.to_owned();
            if linked_operation_job_id.is_some() && code == Some("cancelled") {
                self.selected = None;
                self.page = Page::default();
                self.status = match &reply.purpose {
                    Purpose::Export { .. } => {
                        StatusText::new("Экспорт CTF отменён", "CTF export cancelled")
                    }
                    Purpose::Page {
                        route: Route::Copies { .. },
                    } => StatusText::new(
                        "Поиск кандидатов отменён",
                        "Copy-candidate search cancelled",
                    ),
                    _ => StatusText::new("Поиск в CTF отменён", "CTF search cancelled"),
                };
                self.error = None;
                return;
            }
            self.status = StatusText::new("Операция отклонена", "Operation refused");
            if matches!(&reply.purpose, Purpose::Categories) {
                self.categories.clear();
                self.categories_loaded = false;
                self.category_assigned.clear();
                self.category_assignments_ready = false;
            } else if matches!(&reply.purpose, Purpose::EntryCategoryIds { .. }) {
                self.category_assigned.clear();
                self.category_assignments_ready = false;
                self.category_assignments_error = true;
            } else if matches!(
                &reply.purpose,
                Purpose::AssignCategory { .. } | Purpose::UnassignCategory { .. }
            ) && code == Some("not_found")
            {
                self.selected = None;
                self.refresh_category_results(ctx);
            } else if matches!(&reply.purpose, Purpose::SaveDescription { .. })
                && code == Some("not_found")
            {
                self.description_entry_id = None;
                self.selected = None;
                self.load_page(ctx);
            } else if matches!(&reply.purpose, Purpose::Page { .. }) {
                self.category_result_refresh_pending = false;
                self.category_assigned.clear();
                self.category_assignments_ready = false;
                self.restore_selection = None;
                self.restore_scroll = None;
                self.selected = None;
                self.page = Page::default();
            }
            if matches!(&reply.purpose, Purpose::DiskPreferences) {
                self.preferences_ready = false;
                self.legacy_shelf_editable = code == Some("schema_version");
                self.disk_preferences.clear();
                self.shelf_collisions.clear();
                self.load_shelf_labels(ctx, true);
            } else if matches!(&reply.purpose, Purpose::ShelfLabels { initial: true, .. }) {
                self.shelf = ShelfSnapshot::default();
                self.load_page(ctx);
            }
            self.error = Some(error_message);
            return;
        }
        let data = &reply.response["data"];
        let opening_existing = matches!(&reply.purpose, Purpose::Open { .. });
        match reply.purpose {
            Purpose::CtfPreviewOpen { path, cache_root } => {
                match LinkedCtf::parse_preview(path.clone(), cache_root.clone(), data) {
                    Ok(link) => {
                        self.clear_catalog();
                        let source_sha256 = link.source_sha256.clone();
                        self.db = Some(path.clone());
                        self.path_input = path.clone();
                        self.integrity = CatalogIntegrity::LinkedReadOnly;
                        self.shelf_context =
                            ShelfContext::from_sqlite(&path).ok().map(|mut scope| {
                                scope.catalog_key = format!("ctf:{source_sha256}");
                                if let Some(store) = user_store() {
                                    scope.user_store = store.to_string_lossy().into_owned();
                                }
                                scope
                            });
                        self.linked_ctf = Some(link);
                        self.status = StatusText::new("Каталог CTF открыт", "CTF catalog opened");
                        self.load_volumes(ctx);
                    }
                    Err(error) => {
                        self.ctf_job_id = None;
                        self.error = Some(error);
                    }
                }
            }
            Purpose::ImportCtf { output } => {
                let returned = data["db"].as_str().unwrap_or("");
                let expected = std::fs::canonicalize(&output).ok();
                if expected.is_none() || std::fs::canonicalize(returned).ok() != expected {
                    self.error = Some("Import returned an unexpected catalog path".into());
                    return;
                }
                self.path_input = output;
                self.start_catalog(false, ctx);
            }
            Purpose::ExportCtf { output } => {
                self.status = StatusText::new(
                    format!("CTF сохранён: {output}"),
                    format!("CTF saved: {output}"),
                );
            }
            Purpose::SaveDescription { entry_id, db } => {
                if self.db.as_deref() == Some(db.as_str())
                    && data["entry_id"].as_i64() == Some(entry_id)
                {
                    self.description_entry_id = None;
                    self.selected = None;
                    self.load_page(ctx);
                }
            }
            Purpose::CtfPreviewClose => unreachable!("Preview close is handled above"),
            Purpose::LinkedRead { .. }
            | Purpose::CtfProgress { .. }
            | Purpose::CtfCancel { .. }
            | Purpose::Analytics { .. }
            | Purpose::TreeChildren { .. }
            | Purpose::TreeAncestors { .. }
            | Purpose::AnalyticsCancel
            | Purpose::LocalAnalyzer { .. } => unreachable!("Handled above"),
            Purpose::Open { path } | Purpose::Create { path } => {
                if opening_existing
                    && (!matches!(data["schema_version"].as_i64(), Some(1..=3))
                        || data["readonly"] != true
                        || data["full_validation"] != false)
                {
                    self.status = StatusText::new("Открытие отменено", "Open refused");
                    self.error = Some(
                        self.t(
                            "Получен неверный результат проверки схемы.",
                            "Invalid catalog probe response.",
                        )
                        .to_owned(),
                    );
                    return;
                }
                self.clear_catalog();
                self.shelf_context = ShelfContext::from_sqlite(&path).ok();
                if let Some(store) = user_store() {
                    if let Some(scope) = &mut self.shelf_context {
                        scope.user_store = store.to_string_lossy().into_owned();
                    }
                } else {
                    self.shelf_context = None;
                }
                self.db = Some(path);
                self.integrity = CatalogIntegrity::Unchecked;
                self.load_volumes(ctx);
            }
            Purpose::CheckCatalogIntegrity { .. } => unreachable!("Integrity reply handled above"),
            Purpose::Volumes => match parse_volumes(data) {
                Ok(volumes) => {
                    self.volumes = volumes;
                    if self.linked_ctf.as_ref().is_some_and(|link| link.preview) {
                        self.route = Route::Catalog;
                        self.query_input.clear();
                        self.offset = 0;
                        self.status = StatusText::new("Каталог CTF открыт", "CTF catalog opened");
                        self.page = Page::default();
                    } else {
                        self.status = StatusText::new("Каталог открыт", "Catalog opened");
                        self.load_disk_preferences(ctx);
                    }
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
                        let ids: Vec<i64> = self.volumes.iter().map(|v| v.id).collect();
                        match ShelfSnapshot::parse(data, &ids) {
                            Ok(snapshot) => self.shelf = snapshot,
                            Err(e) => {
                                self.preferences_ready = false;
                                self.disk_preferences.clear();
                                self.shelf_collisions.clear();
                                self.shelf = ShelfSnapshot::default();
                                self.load_page(ctx);
                                self.error = Some(e);
                                return;
                            }
                        }
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
                        self.disk_preferences.clear();
                        self.shelf_collisions.clear();
                        self.shelf = ShelfSnapshot::default();
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
                self.status =
                    StatusText::new("Локальные свойства сохранены", "Local properties saved");
                self.load_volumes(ctx);
            }
            Purpose::ShelfLabels {
                catalog_key,
                initial,
            } => {
                if self
                    .shelf_context
                    .as_ref()
                    .is_some_and(|scope| scope.catalog_key == catalog_key)
                {
                    let ids: Vec<i64> = self.volumes.iter().map(|v| v.id).collect();
                    let error = match ShelfSnapshot::parse(data, &ids) {
                        Ok(snapshot) => {
                            self.shelf = snapshot;
                            None
                        }
                        Err(e) => {
                            self.shelf = ShelfSnapshot::default();
                            Some(e)
                        }
                    };
                    if initial {
                        self.load_page(ctx);
                    }
                    if error.is_some() {
                        self.error = error;
                    }
                }
            }
            Purpose::ShelfSave {
                catalog_key,
                volume_id,
            } => {
                if self
                    .shelf_context
                    .as_ref()
                    .is_some_and(|scope| scope.catalog_key == catalog_key)
                    && self.volumes.iter().any(|v| v.id == volume_id)
                {
                    self.status = StatusText::new(
                        "Наклейка сохранена; не является подтверждением подключения USB.",
                        "Shelf label saved; it does not prove USB presence.",
                    );
                    self.load_shelf_labels(ctx, false);
                }
            }
            Purpose::Rescan {
                db,
                volume_id,
                job_id,
            } => {
                let _ = job_id;
                if self.db.as_deref() != Some(db.as_str()) {
                    return;
                }
                if data["id"].as_i64() != Some(volume_id) {
                    self.clear_catalog();
                    self.error = Some(
                        self.t(
                            "Неожиданный результат обновления снимка.",
                            "Unexpected snapshot update result.",
                        )
                        .into(),
                    );
                    return;
                }
                self.selected_volume = Some(volume_id);
                self.query_input.clear();
                self.history.clear();
                self.offset = 0;
                self.selected = None;
                self.restore_selection = None;
                self.results_scroll = 0.0;
                self.route = Route::Search {
                    query: String::new(),
                    volume_id: Some(volume_id),
                };
                self.status = StatusText::new(
                    "Снимок обновлён; локальные заметки сохранены.",
                    "Snapshot updated; local notes preserved.",
                );
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
                self.status = StatusText::new(
                    "Сканирование закончено; обновляем реестр.",
                    "Scan complete; refreshing disk register.",
                );
                self.load_volumes(ctx);
            }
            Purpose::ScanProgress { .. } | Purpose::ScanCancel { .. } => {
                unreachable!("Auxiliary scan reply must be processed before active worker");
            }
            Purpose::Categories => match parse_categories(data) {
                Ok(categories) => {
                    self.categories = categories;
                    self.categories_loaded = true;
                    if !self
                        .category_parent_id
                        .is_none_or(|id| self.categories.iter().any(|c| c.id == id))
                    {
                        self.category_parent_id = None;
                    }
                    if self.show_categories_dialog && self.selected.is_some() {
                        self.load_assigned_categories(ctx);
                    }
                }
                Err(error) => {
                    self.categories.clear();
                    self.categories_loaded = false;
                    self.category_assigned.clear();
                    self.category_assignments_ready = false;
                    self.error = Some(error);
                }
            },
            Purpose::EntryCategoryIds { entry_id } => {
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|entry| entry.id == entry_id)
                {
                    match categories::parse_assignment_ids(data) {
                        Ok(ids) => {
                            self.category_assigned = ids;
                            self.category_assignments_ready = true;
                            self.category_assignments_error = false;
                        }
                        Err(error) => {
                            self.category_assigned.clear();
                            self.category_assignments_ready = false;
                            self.category_assignments_error = true;
                            self.error = Some(error);
                        }
                    }
                }
            }
            Purpose::CreateCategory { name } => {
                self.category_name_input.clear();
                self.category_parent_id = None;
                self.status = StatusText::new(
                    format!("Категория «{name}» создана"),
                    format!("Category “{name}” created"),
                );
                self.load_categories(ctx);
            }
            Purpose::AssignCategory {
                entry_id,
                category_id,
            } => {
                self.status = StatusText::new(
                    format!("Категория #{category_id} назначена записи #{entry_id}"),
                    format!("Category #{category_id} assigned to entry #{entry_id}"),
                );
                self.refresh_category_results(ctx);
            }
            Purpose::UnassignCategory {
                entry_id,
                category_id,
            } => {
                if data["removed"] != true {
                    self.error = Some(
                        self.t(
                            "Назначение уже отсутствует; обновляем список.",
                            "Assignment already absent; refreshing membership.",
                        )
                        .to_owned(),
                    );
                } else {
                    self.status = StatusText::new(
                        format!("Назначение категории #{category_id} снято для записи #{entry_id}"),
                        format!("Category #{category_id} removed from entry #{entry_id}"),
                    );
                }
                self.refresh_category_results(ctx);
            }
            Purpose::Export { path } => {
                let rows = data.get("rows").and_then(Value::as_u64).unwrap_or(0);
                self.status = StatusText::new(
                    format!("Экспорт завершён: {path} ({rows} записей)"),
                    format!("Export complete: {path} ({rows} rows)"),
                );
            }
            Purpose::Page { route } => match Page::parse(data) {
                Ok(page) => {
                    self.route = route;
                    self.offset = page.offset;
                    if page.items.is_empty() && page.offset > 0 {
                        // Other processes can remove the last hit before Back
                        // returns to a formerly populated high-offset page.
                        // The same guard applies after category edits and to
                        // regular navigation, not just a pending mutation.
                        self.offset = self.offset.saturating_sub(PAGE_LIMIT);
                        self.page = Page::default();
                        self.selected = None;
                        self.restore_selection = None;
                        self.restore_scroll = None;
                        self.load_page(ctx);
                        return;
                    }
                    let start =
                        page.offset
                            .saturating_add(if page.items.is_empty() { 0 } else { 1 });
                    let end = page.offset.saturating_add(page.items.len() as u64);
                    self.status = StatusText::new(
                        format!("Показаны строки {start}–{end}"),
                        format!("Showing rows {start}–{end}"),
                    );
                    self.selected = self.restore_selection.take().and_then(|selection| {
                        page.items
                            .iter()
                            .find(|item| item.id == selection.id)
                            .cloned()
                    });
                    self.page = page;
                    if self.category_result_refresh_pending {
                        self.category_result_refresh_pending = false;
                        if self.show_categories_dialog && self.selected.is_some() {
                            self.load_assigned_categories(ctx);
                        } else {
                            self.category_assigned.clear();
                            self.category_assignments_ready = false;
                        }
                    }
                }
                Err(e) => {
                    // A malformed success response must fail closed exactly
                    // like an explicit backend error: no stale inspector row.
                    self.category_result_refresh_pending = false;
                    self.category_assigned.clear();
                    self.category_assignments_ready = false;
                    self.selected = None;
                    self.restore_selection = None;
                    self.restore_scroll = None;
                    self.page = Page::default();
                    self.error = Some(e);
                }
            },
        }
    }

    fn menu(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::top("menu").show(root, |ui| {
            self.protect_background(ui);
            egui::MenuBar::new().ui(ui, |ui| {
                ui.strong("WhereIsIt AI");
                ui.separator();
                ui.menu_button(self.t("Каталог", "Catalog"), |ui| {
                    if ui
                        .add_enabled(
                            self.active_request.is_none(),
                            egui::Button::new(
                                self.t("Открыть SQLite / CTF…", "Open SQLite / CTF…"),
                            ),
                        )
                        .clicked()
                    {
                        self.show_catalog_dialog = true;
                        self.create_catalog = false;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.active_request.is_none(),
                            egui::Button::new(self.t("Новый каталог…", "New catalog…")),
                        )
                        .clicked()
                    {
                        self.show_catalog_dialog = true;
                        self.create_catalog = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.active_request.is_none(),
                            egui::Button::new(self.t(
                                "Импорт CTF в новый каталог…",
                                "Import CTF into a new catalog…",
                            )),
                        )
                        .clicked()
                    {
                        self.ctf_source_input = self
                            .linked_ctf
                            .as_ref()
                            .map(|l| l.path.clone())
                            .unwrap_or_default();
                        self.ctf_target_input.clear();
                        self.ctf_dialog = Some(CtfDialog::Import);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.db.is_some()
                                && self.linked_ctf.is_none()
                                && self.active_request.is_none(),
                            egui::Button::new(self.t("Экспорт нового CTF…", "Export new CTF…")),
                        )
                        .clicked()
                    {
                        self.ctf_target_input.clear();
                        self.ctf_dialog = Some(CtfDialog::Export);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.db.is_some()
                                && self.linked_ctf.is_none()
                                && self.active_request.is_none(),
                            egui::Button::new(self.t("Проверить целостность…", "Check integrity…")),
                        )
                        .clicked()
                    {
                        self.check_catalog_integrity(ctx);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.db.is_some()
                                && self.linked_ctf.is_none()
                                && self.active_request.is_none(),
                            egui::Button::new(
                                self.t("Сканировать диск/папку…", "Scan disk/folder…"),
                            ),
                        )
                        .clicked()
                    {
                        self.show_scan_dialog = true;
                        self.scan_root.clear();
                        self.scan_label.clear();
                        self.error = None;
                        ui.close();
                    }
                    let can_export = self.db.is_some()
                        && self.active_request.is_none()
                        && self.workspace_tab == WorkspaceTab::Search;
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
                if ui
                    .add_enabled(
                        self.db.is_some()
                            && self.linked_ctf.is_none()
                            && self.active_request.is_none()
                            && self.workspace_tab == WorkspaceTab::Search,
                        egui::Button::new(self.t("Категории", "Categories")),
                    )
                    .clicked()
                {
                    self.category_name_input.clear();
                    self.category_parent_id = None;
                    self.category_assigned.clear();
                    self.category_assignments_ready = false;
                    self.error = None;
                    self.show_categories_dialog = true;
                    self.load_categories(ctx);
                }
                ui.menu_button(self.t("Вид", "View"), |ui| {
                    if ui.button(self.t("Все диски", "All disks")).clicked() {
                        self.navigate_disk(None, ctx);
                        ui.close();
                    }
                    let theme_label = self.t("Тёмная тема", "Dark mode");
                    ui.checkbox(&mut self.dark, theme_label);
                });
                ui.menu_button(self.t("Справка", "Help"), |ui| {
                    ui.label(concat!("WhereIsIt AI · ", env!("CARGO_PKG_VERSION")));
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
                    ui.label(RichText::new(env!("CARGO_PKG_VERSION")).small().weak());
                });
            });
        });
    }

    fn workspace_tabs(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::top("workspace_tabs").show(root, |ui| {
            self.protect_background(ui);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let mut tab_responses = Vec::with_capacity(WorkspaceTab::ORDERED.len());
                for tab in WorkspaceTab::ORDERED {
                    let title = if tab == WorkspaceTab::UsbDisks && self.db.is_some() {
                        format!("{} ({})", tab.label(self.lang), self.volumes.len())
                    } else {
                        tab.label(self.lang).to_owned()
                    };
                    let mut response = ui.add_enabled(
                        tab.available(),
                        egui::Button::selectable(self.workspace_tab == tab, title),
                    );
                    if !tab.available() {
                        let explanation = self.t(
                            "Это рабочее пространство готовится для Windows 1.1.",
                            "This workspace is planned for Windows 1.1.",
                        );
                        response = response
                            .on_hover_text(explanation)
                            .on_disabled_hover_text(explanation);
                    } else if response.clicked() && self.switch_workspace(tab) {
                        response.request_focus();
                    }
                    tab_responses.push((tab, response));
                }

                if self.workspace_tab_focus_request {
                    if !self.dialog_open() {
                        if let Some((_, response)) = tab_responses
                            .iter()
                            .find(|(tab, _)| *tab == self.workspace_tab)
                        {
                            response.request_focus();
                        }
                    }
                    self.workspace_tab_focus_request = false;
                }

                let focus_inside_tabs = tab_responses
                    .iter()
                    .any(|(_, response)| response.has_focus());
                if focus_inside_tabs {
                    let (left, right, home, end) = ctx.input(|input| {
                        let pressed = |key| {
                            input.events.iter().any(|event| {
                                matches!(event, egui::Event::Key {
                                    key: pressed_key,
                                    pressed: true,
                                    modifiers,
                                    ..
                                } if *pressed_key == key && *modifiers == egui::Modifiers::NONE)
                            })
                        };
                        (
                            pressed(egui::Key::ArrowLeft),
                            pressed(egui::Key::ArrowRight),
                            pressed(egui::Key::Home),
                            pressed(egui::Key::End),
                        )
                    });
                    if left || right || home || end {
                        let enabled: Vec<_> = WorkspaceTab::ORDERED
                            .into_iter()
                            .filter(|tab| tab.available())
                            .collect();
                        let current = enabled
                            .iter()
                            .position(|tab| *tab == self.workspace_tab)
                            .unwrap_or(0);
                        let next = if home {
                            0
                        } else if end {
                            enabled.len().saturating_sub(1)
                        } else if right {
                            (current + 1) % enabled.len()
                        } else {
                            (current + enabled.len() - 1) % enabled.len()
                        };
                        if self.switch_workspace(enabled[next]) {
                            ctx.request_repaint();
                        }
                    }
                }
            });
        });
    }

    fn disks_workspace(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::CentralPanel::default().show(root, |ui| {
            self.protect_background(ui);
            ui.horizontal(|ui| {
                ui.heading(format!(
                    "{} ({})",
                    self.t("Носители", "USB Disks"),
                    self.volumes.len()
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = match self.disks_order {
                        RegisterOrder::Id => self.t("Объём ↓", "Size ↓"),
                        RegisterOrder::IndexedBytes => "#ID",
                    };
                    if ui.small_button(label).clicked() {
                        self.disks_order = match self.disks_order {
                            RegisterOrder::Id => RegisterOrder::IndexedBytes,
                            RegisterOrder::IndexedBytes => RegisterOrder::Id,
                        };
                    }
                    ui.label(self.t("Сортировка", "Sort"));
                });
            });
            ui.horizontal(|ui| {
                ui.label(self.t("Фильтр", "Filter"));
                let filter_hint = self.t("Имя, метка или #ID", "Name, label, or #ID");
                ui.add_sized(
                    [260.0, 24.0],
                    egui::TextEdit::singleline(&mut self.disks_filter).hint_text(filter_hint),
                );
                if ui.button(self.t("Все диски", "All disks")).clicked() {
                    self.disks_selected_volume = None;
                }
            });
            ui.separator();
            if self.db.is_none() {
                ui.label(self.t(
                    "Откройте каталог, чтобы увидеть записанные диски.",
                    "Open a catalog to view its recorded disks.",
                ));
                return;
            }
            let filter = self.disks_filter.trim().to_lowercase();
            let mut visible: Vec<_> = self
                .volumes
                .iter()
                .filter(|volume| {
                    register_matches(
                        volume,
                        self.display_name(volume),
                        self.shelf.label(volume.id),
                        self.disk_preferences
                            .get(&volume.id)
                            .map_or("", |preferences| preferences.note.as_str()),
                        &filter,
                    )
                })
                .cloned()
                .collect();
            sort_register(&mut visible, self.disks_order);
            let table = TableBuilder::new(ui)
                .id_salt("usb_disks_workspace_table")
                .striped(true)
                .column(Column::initial(58.0).at_least(52.0))
                .column(Column::remainder().at_least(180.0))
                .column(Column::initial(145.0).at_least(110.0))
                .column(Column::initial(115.0).at_least(90.0))
                .column(Column::initial(220.0).at_least(205.0))
                .vertical_scroll_offset(self.disks_scroll);
            let output = table
                .header(26.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("ID");
                    });
                    header.col(|ui| {
                        ui.strong(self.t("Имя", "Name"));
                    });
                    header.col(|ui| {
                        ui.strong(self.t("Записанный объём", "Indexed size"));
                    });
                    header.col(|ui| {
                        ui.strong(self.t("Записей", "Entries"));
                    });
                    header.col(|ui| {
                        ui.strong("");
                    });
                })
                .body(|body| {
                    body.rows(34.0, visible.len(), |mut row| {
                        let volume = &visible[row.index()];
                        row.col(|ui| {
                            ui.label(format!("#{}", volume.id));
                        });
                        row.col(|ui| {
                            let name = self.display_name(volume);
                            let label = self.shelf.label(volume.id);
                            let title = if label.is_empty() {
                                name.to_owned()
                            } else {
                                format!("{name} · 🏷 {label}")
                            };
                            let selected = self.disks_selected_volume == Some(volume.id);
                            if ui.selectable_label(selected, title).clicked() {
                                self.disks_selected_volume = Some(volume.id);
                            }
                        });
                        row.col(|ui| {
                            ui.label(bytes_compact(volume.total_bytes));
                        });
                        row.col(|ui| {
                            ui.label(volume.entry_count.to_string());
                        });
                        row.col(|ui| {
                            ui.horizontal(|ui| {
                                if ui
                                    .small_button(self.t("Поиск на диске", "Search disk"))
                                    .clicked()
                                {
                                    self.jump_to_search_on_disk(volume.id, ctx);
                                }
                                if ui.small_button(self.t("Свойства", "Properties")).clicked()
                                {
                                    self.disks_selected_volume = Some(volume.id);
                                    self.open_disk_properties(volume.id);
                                    self.shelf_editor = self.shelf.label(volume.id).to_owned();
                                }
                            });
                        });
                    })
                });
            self.disks_scroll = output.state.clamped_offset().y;
            if let Some(id) = self.disks_selected_volume {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(self.t(
                        "Выбранный элемент — запись каталога. Физическое наличие не проверено.",
                        "Selected item is a catalog record. Physical presence is unverified.",
                    ));
                    if ui
                        .button(self.t("Открыть свойства", "Open properties"))
                        .clicked()
                    {
                        self.open_disk_properties(id);
                        self.shelf_editor = self.shelf.label(id).to_owned();
                    }
                });
            }
        });
    }

    fn request_analytics(&mut self, ctx: &egui::Context, slices: bool) {
        let Some(db) = self.db.clone() else {
            return;
        };
        if self.linked_ctf.as_ref().is_some_and(|link| !link.preview) {
            let message = self
                .t(
                    "Аналитика для связанного CTF-источника недоступна.",
                    "Analytics is unavailable for a linked CTF source.",
                )
                .to_owned();
            if slices {
                self.slices_loaded = true;
                self.slices_error = Some(message);
            } else {
                self.analytics_loaded = true;
                self.analytics_error = Some(message);
            }
            return;
        }
        if self.active_request.is_some() {
            return;
        }
        let (old, old_preview) = if slices {
            let old = self.slices_job_id.take();
            let preview = self.slices_preview_job;
            self.slices_preview_job = false;
            (old, preview)
        } else {
            let old = self.analytics_job_id.take();
            let preview = self.analytics_preview_job;
            self.analytics_preview_job = false;
            (old, preview)
        };
        if let Some(old) = old {
            if old_preview {
                let _ = atlas_service::cancel_ctf_preview_analytics_job(&old);
            } else {
                let _ = self.bus.send(
                    Purpose::AnalyticsCancel,
                    analytics_cancel_request(&old),
                    ctx,
                );
            }
        }
        let generation = if slices {
            self.slices_generation = self.slices_generation.wrapping_add(1);
            self.slices_generation
        } else {
            self.analytics_generation = self.analytics_generation.wrapping_add(1);
            self.analytics_generation
        };
        let job_id = new_scan_job_id();
        let volume = if slices {
            self.slices_volume
        } else {
            self.analytics_volume
        };
        let folder = if slices { None } else { self.analytics_folder };
        let category = if slices {
            self.slices_category
        } else {
            self.analytics_category_id
        };
        let mut request = analytics_request(
            &db,
            &job_id,
            volume,
            folder,
            if slices {
                "volumes"
            } else {
                &self.analytics_group
            },
            if slices {
                ""
            } else {
                &self.analytics_highlight
            },
            category,
        );
        let linked_generation = self
            .linked_ctf
            .as_ref()
            .filter(|link| link.preview)
            .map(|link| link.generation.clone());
        if let Some(link) = self.linked_ctf.as_ref().filter(|link| link.preview) {
            match link.rewrite(request) {
                Ok(mapped) => request = mapped,
                Err(error) => {
                    if slices {
                        self.slices_loaded = true;
                        self.slices_error = Some(error);
                    } else {
                        self.analytics_loaded = true;
                        self.analytics_error = Some(error);
                    }
                    return;
                }
            }
        }
        if !slices {
            self.analytics_applied_highlight = self.analytics_highlight.trim().to_owned();
        }
        let preview_job = linked_generation.is_some();
        let request_id = self.bus.send(
            Purpose::Analytics {
                job_id: job_id.clone(),
                generation,
                db: db.clone(),
                slices,
                linked_generation,
            },
            request,
            ctx,
        );
        if slices {
            self.slices_request_id = Some(request_id);
            self.slices_job_id = Some(job_id);
            self.slices_preview_job = preview_job;
            self.slices_db = Some(db);
            self.slices_data = None;
            self.slices_error = None;
            self.slices_cancelled = false;
            self.slices_loaded = false;
        } else {
            self.analytics_request_id = Some(request_id);
            self.analytics_job_id = Some(job_id);
            self.analytics_preview_job = preview_job;
            self.analytics_db = Some(db);
            self.analytics_data = None;
            self.analytics_error = None;
            self.analytics_cancelled = false;
            self.analytics_loaded = false;
        }
    }

    fn cancel_analytics(&mut self, ctx: &egui::Context, slices: bool, user_cancelled: bool) {
        let (job_id, preview) = if slices {
            let job_id = self.slices_job_id.take();
            let preview = self.slices_preview_job;
            self.slices_preview_job = false;
            (job_id, preview)
        } else {
            let job_id = self.analytics_job_id.take();
            let preview = self.analytics_preview_job;
            self.analytics_preview_job = false;
            (job_id, preview)
        };
        if let Some(job_id) = job_id {
            if preview {
                let _ = atlas_service::cancel_ctf_preview_analytics_job(&job_id);
            } else {
                let _ = self.bus.send(
                    Purpose::AnalyticsCancel,
                    analytics_cancel_request(&job_id),
                    ctx,
                );
            }
            if slices {
                self.slices_cancelled = user_cancelled;
                self.slices_loaded = user_cancelled;
                self.slices_generation = self.slices_generation.wrapping_add(1);
                self.slices_request_id = None;
            } else {
                self.analytics_cancelled = user_cancelled;
                self.analytics_loaded = user_cancelled;
                self.analytics_generation = self.analytics_generation.wrapping_add(1);
                self.analytics_request_id = None;
            }
        }
    }

    fn jump_analytics_to_search(
        &mut self,
        ctx: &egui::Context,
        volume: Option<i64>,
        folder: Option<i64>,
        query: &str,
    ) {
        if self.active_request.is_some() || self.dialog_open() {
            return;
        }
        let exact_category = if self.workspace_tab == WorkspaceTab::Slices {
            self.slices_category
        } else {
            self.analytics_category_id
        };
        let scoped_query = exact_category_query(query, exact_category);
        self.history.clear();
        self.selected_volume = volume;
        self.query_input = if folder.is_some() {
            query.to_owned()
        } else {
            scoped_query.clone()
        };
        self.offset = 0;
        self.page = Page::default();
        self.selected = None;
        self.restore_selection = None;
        self.reveal_selected = false;
        self.restore_scroll = None;
        self.results_scroll = 0.0;
        self.route = match (volume, folder) {
            (Some(volume_id), Some(parent_id)) if scoped_query.trim().is_empty() => Route::Folder {
                volume_id,
                parent_id: Some(parent_id),
            },
            (Some(volume_id), Some(parent_id)) => Route::BranchSearch {
                volume_id,
                parent_id,
                query: query.trim().to_owned(),
                exact_category_id: exact_category,
            },
            (Some(volume_id), None) if scoped_query.trim().is_empty() => Route::Folder {
                volume_id,
                parent_id: None,
            },
            (None, None) if scoped_query.trim().is_empty() => Route::Catalog,
            (_, _) => Route::Search {
                query: scoped_query,
                volume_id: volume,
            },
        };
        self.folder_crumbs.clear();
        if let (Some(volume_id), Some(parent_id)) = (volume, folder) {
            self.tree_expanded.insert((volume_id, None));
            self.request_tree_ancestors(volume_id, parent_id, ctx);
        }
        self.switch_workspace(WorkspaceTab::Search);
        self.load_page(ctx);
    }

    fn jump_analytics_to_copy_candidates(
        &mut self,
        ctx: &egui::Context,
        entry_id: i64,
        volume: Option<i64>,
    ) {
        if self.active_request.is_some() || self.dialog_open() {
            return;
        }
        self.history.clear();
        self.selected_volume = volume;
        self.query_input.clear();
        self.offset = 0;
        self.page = Page::default();
        self.selected = None;
        self.restore_selection = None;
        self.reveal_selected = false;
        self.restore_scroll = None;
        self.results_scroll = 0.0;
        self.route = Route::Copies { entry_id };
        self.folder_crumbs.clear();
        self.switch_workspace(WorkspaceTab::Search);
        self.load_page(ctx);
    }

    fn analytics_workspace(&mut self, root: &mut egui::Ui, ctx: &egui::Context, slices: bool) {
        let db_now = self.db.clone();
        let old_db = if slices {
            self.slices_db.as_ref()
        } else {
            self.analytics_db.as_ref()
        };
        let has_job = if slices {
            self.slices_job_id.is_some()
        } else {
            self.analytics_job_id.is_some()
        };
        if has_job && old_db != db_now.as_ref() {
            self.cancel_analytics(ctx, slices, false);
        }
        let (loaded, job_id) = if slices {
            (self.slices_loaded, self.slices_job_id.is_some())
        } else {
            (self.analytics_loaded, self.analytics_job_id.is_some())
        };
        if self.db.is_none() {
            self.cancel_analytics(ctx, slices, false);
        } else if !loaded && !job_id {
            self.request_analytics(ctx, slices);
        }
        let mut selected_map_volume = None;
        if !slices {
            egui::Panel::left("analytics_volume_register")
                .resizable(true)
                .default_size(194.0)
                .size_range(150.0..=270.0)
                .show(root, |ui| {
                    self.protect_background(ui);
                    ui.heading(self.t("Диски каталога", "Catalog disks"));
                    ui.weak(self.t("Логические байты индекса", "Logical indexed bytes"));
                    let all = self.analytics_volume.is_none();
                    if ui
                        .selectable_label(all, self.t("Все диски каталога", "All catalog disks"))
                        .clicked()
                        && !all
                    {
                        selected_map_volume = Some(None);
                    }
                    let rows = self.volumes.clone();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for volume in &rows {
                            let selected = self.analytics_volume == Some(volume.id);
                            let mut short_name: String = volume.label.chars().take(14).collect();
                            if volume.label.chars().count() > 14 {
                                short_name.push('…');
                            }
                            let label = format!("#{} · {short_name}", volume.id);
                            ui.horizontal(|ui| {
                                let response =
                                    ui.selectable_label(selected, label).on_hover_text(format!(
                                        "{} · {} entries · {} {}",
                                        volume.label,
                                        volume.entry_count,
                                        bytes_compact(volume.total_bytes),
                                        self.t("логических байт индекса", "logical indexed bytes"),
                                    ));
                                if response.clicked() && !selected {
                                    selected_map_volume = Some(Some(volume.id));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.weak(bytes_compact(volume.total_bytes));
                                    },
                                );
                            });
                        }
                    });
                });
        }
        if let Some(volume) = selected_map_volume {
            self.analytics_volume = volume;
            self.analytics_folder = None;
            self.request_analytics(ctx, false);
        }
        egui::CentralPanel::default().show(root, |ui| {
            self.protect_background(ui);
            let mut changed = false;
            let (is_loading, is_cancelled, error, current_data) = if slices {
                (
                    self.slices_job_id.is_some(),
                    self.slices_cancelled,
                    self.slices_error.clone(),
                    self.slices_data.clone(),
                )
            } else {
                (
                    self.analytics_job_id.is_some(),
                    self.analytics_cancelled,
                    self.analytics_error.clone(),
                    self.analytics_data.clone(),
                )
            };
            let slices_all_label = self.t("Весь архив", "All archive");
            let slices_all_categories_label = self.t("Все категории", "All categories");
            let slices_category_label = self.slices_category.map(|id| {
                self.categories.iter().find(|category| category.id == id).map(|category| category.name.clone())
                    .unwrap_or_else(|| format!("Категория #{id}"))
            }).unwrap_or_else(|| slices_all_categories_label.to_owned());
            let highlight_hint = self.t("Подсветить запрос…", "Highlight query…");
            let slice_volumes = self.volumes.clone();
            ui.horizontal_wrapped(|ui| {
                ui.heading(if slices {
                    self.t("Разрезы архива", "Archive Slices")
                } else {
                    self.t("Карта объёма", "Volume Map")
                });
                ui.separator();
                let volume_label = self.t("Диски", "Volumes");
                let category_label = self.t("Категории", "Categories");
                let extension_label = self.t("Расширения", "Extensions");
                if !slices {
                    for (id, label) in [
                        ("volumes", volume_label),
                        ("categories", category_label),
                        ("extensions", extension_label),
                    ] {
                        if ui
                            .selectable_label(self.analytics_group == id, label)
                            .clicked()
                            && self.analytics_group != id
                        {
                            self.analytics_group = id.to_owned();
                            changed = true;
                        }
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.analytics_highlight)
                            .hint_text(highlight_hint)
                            .desired_width(170.0),
                    );
                    if response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                        changed = true;
                    }
                    if ui.small_button(self.t("Применить", "Apply")).clicked() {
                        changed = true;
                    }
                } else {
                    for (n, label) in [
                        (0, self.t("Обзор", "Overview")),
                        (1, self.t("Кандидаты", "Candidates")),
                        (2, volume_label),
                    ] {
                        if ui
                            .selectable_label(self.slices_dimension == n, label)
                            .clicked()
                        {
                            self.slices_dimension = n;
                        }
                    }
                    egui::ComboBox::from_id_salt("slices_volume_scope")
                        .selected_text(
                            self.slices_volume
                                .map(|id| format!("#{id}"))
                                .unwrap_or_else(|| slices_all_label.to_owned()),
                        )
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(&mut self.slices_volume, None, slices_all_label)
                                .changed()
                            {
                                changed = true;
                            }
                            for volume in &slice_volumes {
                                if ui
                                    .selectable_value(
                                        &mut self.slices_volume,
                                        Some(volume.id),
                                        format!("#{} · {}", volume.id, volume.label),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                        });
                    let categories = current_data
                        .as_ref()
                        .and_then(|d| d["by_category"].as_array().cloned())
                        .unwrap_or_default();
                    egui::ComboBox::from_id_salt("slices_category_scope")
                            .selected_text(slices_category_label)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(
                                    &mut self.slices_category,
                                    None,
                                    slices_all_categories_label,
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            for category in &categories {
                                if let Some(id) = category["id"]
                                    .as_str()
                                    .and_then(|v| v.strip_prefix("category:"))
                                    .and_then(|v| v.parse::<i64>().ok())
                                {
                                    if ui
                                        .selectable_value(
                                            &mut self.slices_category,
                                            Some(id),
                                            category["label"].as_str().unwrap_or("?"),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                }
                            }
                        });
                }
                if ui
                    .small_button(self.t("Открыть в поиске", "Open in Search"))
                    .clicked()
                {
                    let volume = if slices {
                        self.slices_volume
                    } else {
                        self.analytics_volume
                    };
                    let folder = if slices { None } else { self.analytics_folder };
                    let map_query = self.analytics_highlight.clone();
                    let query = if slices { "" } else { map_query.as_str() };
                    self.jump_analytics_to_search(ctx, volume, folder, query);
                }
                if slices && ui.small_button(self.t("Карта", "Map")).clicked() {
                    self.analytics_volume = self.slices_volume;
                    self.analytics_folder = None;
                    self.analytics_category_id = self.slices_category;
                    self.switch_workspace(WorkspaceTab::VolumeMap);
                    self.request_analytics(ctx, false);
                }
                if is_loading && ui.button(self.t("Отмена", "Cancel")).clicked() {
                    self.cancel_analytics(ctx, slices, true);
                }
                if let Some(result) = &current_data {
                    if result["truncated"] == true {
                        ui.colored_label(
                            Color32::YELLOW,
                            format!(
                                "{} · {} {} · {}",
                                self.t("Результат ограничен", "Result limited"),
                                result["omitted_node_count"].as_u64().unwrap_or(0),
                                self.t("узлов пропущено", "nodes omitted"),
                                bytes_compact(result["remainder_bytes"].as_u64().unwrap_or(0))
                            ),
                        );
                    }
                }
            });
            if changed {
                if slices {
                    self.request_analytics(ctx, true);
                } else {
                    self.request_analytics(ctx, false);
                }
            }
            if is_loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(self.t("Чтение локального индекса…", "Reading local index…"));
                });
            }
            if let Some(error) = error {
                ui.colored_label(Color32::LIGHT_RED, error);
                if ui.button(self.t("Повторить", "Retry")).clicked() {
                    self.request_analytics(ctx, slices);
                }
            }
            if is_cancelled && current_data.is_none() && !is_loading {
                ui.label(self.t("Анализ отменён.", "Analysis cancelled."));
                if ui.button(self.t("Запустить снова", "Run again")).clicked() {
                    self.request_analytics(ctx, slices);
                }
            }
            let Some(data) = current_data else {
                return;
            };
            let totals = &data["totals"];
            ui.columns(4, |cols| {
                let cards = [
                    (
                        self.t("Логические байты индекса", "Logical indexed bytes"),
                        bytes_compact(totals["indexed_bytes"].as_u64().unwrap_or(0)),
                    ),
                    (
                        self.t("Файлы", "Files"),
                        totals["files"].as_u64().unwrap_or(0).to_string(),
                    ),
                    (
                        self.t("Папки структуры", "Structural directories"),
                        totals["directories"].as_u64().unwrap_or(0).to_string(),
                    ),
                    (
                        self.t("Диски в срезе", "Volumes in scope"),
                        data["by_volume"]
                            .as_array()
                            .map_or(0, |rows| rows.len() as u64)
                            .to_string(),
                    ),
                ];
                for (index, (col, (label, value))) in cols.iter_mut().zip(cards).enumerate() {
                    let card = col.group(|ui| {
                        ui.weak(label);
                        ui.strong(value);
                    });
                    if index == 2 {
                        card.response.on_hover_text(self.t(
                            "Число папок в выбранном дисковом охвате; категории фильтруют файлы, не структуру папок.",
                            "Folder count in the selected disk scope; category filters apply to files, not the folder structure.",
                        ));
                    }
                }
            });
            if slices {
                self.draw_slice_panels(ui, &data, ctx);
            } else {
                self.draw_sunburst(ui, &data, ctx);
            }
        });
    }

    fn draw_slice_panels(&mut self, ui: &mut egui::Ui, data: &Value, ctx: &egui::Context) {
        if self.slices_dimension == 1 {
            ui.columns(2,|cols|{
                cols[0].group(|ui|{ui.strong(self.t("Кандидаты на сверку","Review candidates"));ui.separator();ui.weak(self.t("Срезы не подтверждают равенство содержимого или наличие резерва.","Archive slices do not verify equal contents or backup status."));ui.label(self.t("Нет данных кандидатов для этой выборки.","No candidate rows are available for this scope."));});
                cols[1].group(|ui|{ui.strong(self.t("Безопасное действие","Safe action"));ui.separator();ui.label(self.t("Откройте результаты поиска и используйте отдельный отчёт кандидатов по выбранному файлу.","Open search results and use the candidate report for a selected file."));});
            });
            return;
        }
        let left_key = if self.slices_dimension == 2 {
            "by_volume"
        } else {
            "by_extension"
        };
        let right_key = if self.slices_dimension == 2 {
            "by_size_bucket"
        } else {
            "by_category"
        };
        let left_title = if self.slices_dimension == 2 {
            self.t("Диски каталога", "Catalog volumes")
        } else {
            self.t("Расширения", "Extensions")
        };
        let right_title = if self.slices_dimension == 2 {
            self.t("Размеры файлов", "File sizes")
        } else {
            self.t("Категории", "Categories")
        };
        ui.columns(2, |cols| {
            for (col, key, title) in [(0, left_key, left_title), (1, right_key, right_title)] {
                let rows = data[key].as_array().cloned().unwrap_or_default();
                cols[col].group(|ui| {
                    ui.strong(title);
                    ui.separator();
                    if rows.is_empty() {
                        ui.weak(self.t("Нет данных для этой выборки.", "No rows for this scope."));
                    }
                    let maximum = rows
                        .iter()
                        .filter_map(|r| r["bytes"].as_u64())
                        .max()
                        .unwrap_or(1)
                        .max(1);
                    egui::ScrollArea::vertical()
                        .max_height(430.0)
                        .show(ui, |ui| {
                            for row in rows.iter().take(100) {
                                let label = row["label"]
                                    .as_str()
                                    .or_else(|| row["id"].as_str())
                                    .unwrap_or("?");
                                let bytes = row["bytes"].as_u64().unwrap_or(0);
                                let count = row["file_count"].as_u64().unwrap_or(0);
                                let id = row["id"].as_str().unwrap_or("");
                                let query = if key == "by_extension" {
                                    let extension = id.strip_prefix("ext:").unwrap_or("");
                                    extension_search_query(extension)
                                } else if key == "by_category" {
                                    primary_category_search_query(id).unwrap_or_default()
                                } else if key == "by_size_bucket" {
                                    match id {
                                        "size:0" => "size:<1MiB".into(),
                                        "size:1" => "size:>=1MiB AND size:<1GiB".into(),
                                        "size:2" => "size:>=1GiB AND size:<10GiB".into(),
                                        "size:3" => "size:>=10GiB".into(),
                                        _ => String::new(),
                                    }
                                } else {
                                    String::new()
                                };
                                if ui
                                    .selectable_label(
                                        false,
                                        format!("{label}    {count} · {}", bytes_compact(bytes)),
                                    )
                                    .clicked()
                                {
                                    if key == "by_volume" {
                                        let volume = id
                                            .strip_prefix("volume:")
                                            .and_then(|v| v.parse::<i64>().ok());
                                        if let Some(volume) = volume {
                                            self.jump_analytics_to_search(
                                                ctx,
                                                Some(volume),
                                                None,
                                                "",
                                            );
                                        }
                                    } else if !query.is_empty() {
                                        self.jump_analytics_to_search(
                                            ctx,
                                            self.slices_volume,
                                            None,
                                            &query,
                                        );
                                    }
                                }
                                ui.add(
                                    egui::ProgressBar::new(bytes as f32 / maximum as f32).text(""),
                                );
                            }
                        });
                });
            }
        });
    }

    fn draw_sunburst(&mut self, ui: &mut egui::Ui, data: &Value, ctx: &egui::Context) {
        let root = &data["root"];
        if !root.is_object() {
            ui.colored_label(
                Color32::LIGHT_RED,
                self.t(
                    "Ответ аналитики неполон.",
                    "Analytics response is incomplete.",
                ),
            );
            return;
        }
        let crumbs = data["breadcrumbs"].as_array().cloned().unwrap_or_default();
        ui.horizontal(|ui| {
            for (i, crumb) in crumbs.iter().enumerate() {
                if i > 0 {
                    ui.label("›");
                }
                let label = crumb["label"].as_str().unwrap_or("?");
                if ui.small_button(label).clicked() {
                    self.analytics_volume = crumb
                        .get("volume_id")
                        .or_else(|| crumb.get("volumeId"))
                        .and_then(Value::as_i64);
                    self.analytics_folder = crumb
                        .get("folder_entry_id")
                        .or_else(|| crumb.get("folderEntryId"))
                        .and_then(Value::as_i64);
                    self.request_analytics(ctx, false);
                }
            }
        });
        let mut clicked: Option<Value> = None;
        let mut hovered: Option<Value> = None;
        let mut parent_clicked = false;
        ui.columns(2, |cols| {
            let chart = &mut cols[0];
            let (rect, response) = chart.allocate_exact_size(
                egui::vec2(chart.available_width().min(500.0), 460.0),
                egui::Sense::click(),
            );
            let painter = chart.painter_at(rect);
            let center = rect.center();
            let max_r = rect.width().min(rect.height()) * 0.47;
            painter.circle_filled(
                center,
                max_r * 0.34,
                if self.dark {
                    Color32::from_gray(28)
                } else {
                    Color32::from_gray(248)
                },
            );
            let colors = [
                [
                    [37, 99, 235],
                    [59, 130, 246],
                    [96, 165, 250],
                    [147, 197, 253],
                ],
                [
                    [13, 148, 136],
                    [20, 184, 166],
                    [45, 212, 191],
                    [94, 234, 212],
                ],
                [
                    [217, 119, 6],
                    [245, 158, 11],
                    [251, 191, 36],
                    [252, 211, 77],
                ],
                [
                    [124, 58, 237],
                    [139, 92, 246],
                    [167, 139, 250],
                    [196, 181, 253],
                ],
                [
                    [225, 29, 72],
                    [244, 63, 94],
                    [251, 113, 133],
                    [253, 164, 175],
                ],
                [
                    [2, 132, 199],
                    [14, 165, 233],
                    [56, 189, 248],
                    [125, 211, 252],
                ],
                [
                    [5, 150, 105],
                    [16, 185, 129],
                    [52, 211, 153],
                    [110, 231, 183],
                ],
                [
                    [79, 70, 229],
                    [99, 102, 241],
                    [129, 140, 248],
                    [165, 180, 252],
                ],
            ];
            let radii = [(0.36, 0.54), (0.56, 0.70), (0.72, 0.84), (0.86, 0.98)];
            let mut ranked = Vec::new();
            let mut hit_regions: Vec<(f64, f64, f32, f32, Value)> = Vec::new();
            let has_truncated_remainder =
                data["truncated"] == true && data["remainder_bytes"].as_u64().unwrap_or(0) > 0;
            let start_angle = -std::f64::consts::FRAC_PI_2;
            let mut parents = vec![(
                root.clone(),
                start_angle,
                start_angle + std::f64::consts::TAU,
                0usize,
                0usize,
            )];
            while let Some((parent, start, end, depth, branch_color)) = parents.pop() {
                if depth >= 4 {
                    continue;
                }
                let container = matches!(
                    parent["kind"].as_str(),
                    Some("root" | "volume" | "directory" | "archive")
                );
                if !container {
                    continue;
                }
                let Some(children) = parent["children"].as_array() else {
                    continue;
                };
                if children.is_empty() {
                    if has_truncated_remainder
                        && parent["size"].as_u64().unwrap_or(0) > 0
                        && parent["item_count"].as_u64().unwrap_or(0) > 0
                    {
                        draw_annular_sector(
                            &painter,
                            center,
                            max_r,
                            radii[depth],
                            start,
                            end,
                            Color32::DARK_GRAY,
                        );
                    }
                    continue;
                }
                let total = parent["size"]
                    .as_f64()
                    .unwrap_or_else(|| children.iter().filter_map(|n| n["size"].as_f64()).sum())
                    .max(1.0);
                let mut angle = start;
                let mut ordered = children.clone();
                ordered.sort_by(|a, b| {
                    b["size"]
                        .as_u64()
                        .unwrap_or(0)
                        .cmp(&a["size"].as_u64().unwrap_or(0))
                });
                for (child_index, node) in ordered.into_iter().enumerate() {
                    let size = node["size"].as_f64().unwrap_or(0.0);
                    let sweep = ((end - start) * size / total).max(0.0).min(end - angle);
                    let stop = (angle + sweep).min(end);
                    if sweep < 0.0005 {
                        angle = stop;
                        continue;
                    }
                    let hue = if depth == 0 {
                        child_index % colors.len()
                    } else {
                        branch_color
                    };
                    let base = colors[hue][depth];
                    let (inner, outer) = radii[depth];
                    if sweep > 0.0 {
                        hit_regions.push((angle, stop, inner * max_r, outer * max_r, node.clone()));
                    }
                    let point = |r: f32, a: f64| {
                        center + egui::vec2(a.cos() as f32, a.sin() as f32) * (max_r * r)
                    };
                    let matched = node["matched_bytes"].as_u64().unwrap_or(0) > 0
                        || node["matched_files"].as_u64().unwrap_or(0) > 0;
                    let alpha = if self.analytics_applied_highlight.is_empty() || matched {
                        255
                    } else {
                        54
                    };
                    let fill = Color32::from_rgba_unmultiplied(base[0], base[1], base[2], alpha);
                    let steps = ((stop - angle).abs() * 24.0).ceil().max(1.0) as usize;
                    draw_annular_sector(&painter, center, max_r, (inner, outer), angle, stop, fill);
                    let boundary = if self.dark {
                        Color32::from_gray(20)
                    } else {
                        Color32::WHITE
                    };
                    painter.line_segment(
                        [point(inner, angle), point(outer, angle)],
                        egui::Stroke::new(1.0, boundary),
                    );
                    painter.line_segment(
                        [point(inner, stop), point(outer, stop)],
                        egui::Stroke::new(1.0, boundary),
                    );
                    let arc_points: Vec<_> = (0..=steps)
                        .map(|s| point(outer, angle + (stop - angle) * s as f64 / steps as f64))
                        .collect();
                    painter.add(egui::Shape::line(
                        arc_points,
                        egui::Stroke::new(1.0, boundary),
                    ));
                    if depth == 0 {
                        ranked.push(node.clone());
                    }
                    if stop - angle > 0.003 {
                        parents.push((node.clone(), angle, stop, depth + 1, hue));
                    }
                    angle = stop;
                }
                if has_truncated_remainder && angle < end - 0.002 {
                    // Render the bounded/capped tail as an unknown remainder, never redistribute it.
                    draw_annular_sector(
                        &painter,
                        center,
                        max_r,
                        radii[depth],
                        angle,
                        end,
                        Color32::DARK_GRAY,
                    );
                }
            }
            if let Some(pointer) = response.hover_pos() {
                let v = pointer - center;
                let radius = v.length();
                let mut angle = (v.y as f64).atan2(v.x as f64);
                while angle < start_angle {
                    angle += std::f64::consts::TAU;
                }
                if let Some((_, _, _, _, node)) =
                    hit_regions.iter().find(|(a, b, inner, outer, _)| {
                        radius >= *inner && radius <= *outer && angle >= *a && angle <= *b
                    })
                {
                    hovered = Some(node.clone());
                    if response.clicked() {
                        clicked = Some(node.clone());
                    }
                } else if radius < max_r * 0.34 && response.clicked() {
                    parent_clicked = true;
                }
            }
            let center_node = hovered.as_ref().unwrap_or(root);
            let center_text = if self.analytics_applied_highlight.is_empty() {
                bytes_compact(center_node["size"].as_u64().unwrap_or(0))
            } else {
                format!(
                    "{} / {}",
                    bytes_compact(center_node["matched_bytes"].as_u64().unwrap_or(0)),
                    center_node["matched_files"].as_u64().unwrap_or(0)
                )
            };
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                center_text,
                egui::FontId::proportional(15.0),
                if self.dark {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                },
            );
            let list = &mut cols[1];
            list.strong(root["name"].as_str().unwrap_or("Archive"));
            list.separator();
            if let Some(node) = &hovered {
                list.group(|ui| {
                    ui.strong(node["name"].as_str().unwrap_or("?"));
                    ui.label(format!(
                        "{} · {}",
                        bytes_compact(node["size"].as_u64().unwrap_or(0)),
                        node["item_count"].as_u64().unwrap_or(0)
                    ));
                    if !self.analytics_applied_highlight.is_empty() {
                        ui.weak(format!(
                            "{} · {}",
                            self.t("Совпало", "Matched"),
                            bytes_compact(node["matched_bytes"].as_u64().unwrap_or(0))
                        ));
                    }
                });
            }
            ranked.sort_by(|a, b| {
                b["size"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&a["size"].as_u64().unwrap_or(0))
            });
            egui::ScrollArea::vertical()
                .max_height(470.0)
                .show(list, |ui| {
                    for node in ranked {
                        let name = node["name"].as_str().unwrap_or("?");
                        let bytes = node["size"].as_u64().unwrap_or(0);
                        let count = node["item_count"].as_u64().unwrap_or(0);
                        if ui
                            .selectable_label(
                                false,
                                format!("{name}    {} · {count}", bytes_compact(bytes)),
                            )
                            .clicked()
                        {
                            clicked = Some(node);
                        }
                    }
                });
        });
        if parent_clicked && crumbs.len() > 1 {
            let crumb = &crumbs[crumbs.len() - 2];
            self.analytics_volume = crumb
                .get("volume_id")
                .or_else(|| crumb.get("volumeId"))
                .and_then(Value::as_i64);
            self.analytics_folder = crumb
                .get("folder_entry_id")
                .or_else(|| crumb.get("folderEntryId"))
                .and_then(Value::as_i64);
            self.request_analytics(ctx, false);
        }
        if let Some(node) = clicked {
            if self.analytics_group == "categories" && node["kind"] == "category" {
                let id = node["id"].as_str().unwrap_or("");
                if let Some(query) = primary_category_search_query(id) {
                    self.jump_analytics_to_search(ctx, self.analytics_volume, None, &query);
                }
            } else if self.analytics_group == "extensions" && node["kind"] == "other_group" {
                let extension = node["id"]
                    .as_str()
                    .and_then(|id| id.strip_prefix("ext:"))
                    .unwrap_or("");
                let query = extension_search_query(extension);
                self.jump_analytics_to_search(ctx, self.analytics_volume, None, &query);
            } else if let Some(entry_id) = node["entry_id"].as_i64() {
                if node["kind"] == "file" {
                    self.jump_analytics_to_copy_candidates(
                        ctx,
                        entry_id,
                        node["volume_id"].as_i64(),
                    );
                } else {
                    self.analytics_volume = node["volume_id"].as_i64().or(self.analytics_volume);
                    self.analytics_folder = Some(entry_id);
                    self.request_analytics(ctx, false);
                }
            } else if let Some(volume) = node["volume_id"].as_i64() {
                self.analytics_volume = Some(volume);
                self.analytics_folder = None;
                self.request_analytics(ctx, false);
            }
        }
    }

    fn search_toolbar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::top("search_toolbar").show(root, |ui| {
            self.protect_background(ui);
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                let back = ui.add_enabled(
                    !self.history.is_empty() && self.active_request.is_none(),
                    egui::Button::new(self.t("← Назад", "← Back")),
                );
                if back.clicked() {
                    self.go_back(ctx);
                }
                let can_go_up = match &self.route {
                    Route::Folder {
                        parent_id: Some(id),
                        ..
                    } => self
                        .folder_crumbs
                        .last()
                        .is_some_and(|crumb| crumb.id == *id),
                    _ => false,
                };
                if ui
                    .add_enabled(
                        can_go_up && self.active_request.is_none(),
                        egui::Button::new(self.t("↑ Вверх", "↑ Up")),
                    )
                    .clicked()
                {
                    self.browse_up(ctx);
                }
                if ui
                    .add_enabled(
                        self.db.is_some()
                            && self.active_request.is_none()
                            && !matches!(&self.route, Route::Catalog),
                        egui::Button::new(self.t("Обзор", "Overview")),
                    )
                    .clicked()
                {
                    self.push_history();
                    self.route = Route::Catalog;
                    self.folder_crumbs.clear();
                    self.query_input.clear();
                    self.selected_volume = None;
                    self.offset = 0;
                    self.page = Page::default();
                    self.selected = None;
                    self.load_page(ctx);
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
                    self.submit_search(ctx);
                }
                if ui
                    .add_enabled(self.db.is_some(), egui::Button::new(self.t("Все", "All")))
                    .clicked()
                {
                    self.navigate_disk(None, ctx);
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
                self.protect_background(ui);
                ui.horizontal(|ui| {
                    ui.strong(self.t("Диски каталога", "Catalog disks"));
                    ui.label(RichText::new(self.volumes.len().to_string()).weak());
                    let order_label = match self.register_order {
                        RegisterOrder::Id => "#ID",
                        RegisterOrder::IndexedBytes => self.t("Объём↓", "Size↓"),
                    };
                    if ui
                        .small_button(order_label)
                        .on_hover_text(self.t(
                            "Сортировать по ID или записанному объёму",
                            "Sort by ID or indexed size",
                        ))
                        .clicked()
                    {
                        self.register_order = match self.register_order {
                            RegisterOrder::Id => RegisterOrder::IndexedBytes,
                            RegisterOrder::IndexedBytes => RegisterOrder::Id,
                        };
                    }
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

                let filter = self.disk_filter.trim().to_lowercase();
                let mut visible: Vec<_> = self
                    .volumes
                    .iter()
                    .filter(|v| {
                        register_matches(
                            v,
                            self.display_name(v),
                            self.shelf.label(v.id),
                            self.disk_preferences
                                .get(&v.id)
                                .map_or("", |p| p.note.as_str()),
                            &filter,
                        )
                    })
                    .cloned()
                    .collect();
                sort_register(&mut visible, self.register_order);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for volume in &visible {
                            let selected = self.selected_volume == Some(volume.id);
                            let root_key = (volume.id, None);
                            let root_expanded = self.tree_expanded.contains(&root_key);
                            ui.horizontal(|ui| {
                                let sticker = self.shelf.label(volume.id);
                                let title = if sticker.is_empty() {
                                    format!("#{} · {}", volume.id, self.display_name(volume))
                                } else {
                                    format!(
                                        "#{} · {} · 🏷 {}{}",
                                        volume.id,
                                        self.display_name(volume),
                                        sticker,
                                        if self.shelf.collision_other_disks(volume.id).is_empty() {
                                            ""
                                        } else {
                                            " ⚠"
                                        }
                                    )
                                };
                                if ui
                                    .small_button(if root_expanded { "▾" } else { "▸" })
                                    .clicked()
                                {
                                    self.toggle_tree_node(root_key, ctx);
                                }
                                let label_width = (ui.available_width() - 88.0).max(24.0);
                                let label = ui
                                    .add_sized(
                                        [label_width, 20.0],
                                        egui::Button::selectable(selected, title.clone())
                                            .truncate(),
                                    )
                                    .on_hover_text(format!(
                                        "{}\n{} · {}",
                                        title,
                                        volume.entry_count,
                                        bytes_compact(volume.total_bytes)
                                    ));
                                if label.clicked() {
                                    self.browse_volume_root(volume.id, ctx);
                                }
                                ui.label(
                                    RichText::new(bytes_compact(volume.total_bytes))
                                        .small()
                                        .weak(),
                                );
                                if let Some(state) = self.physical.state(volume.id) {
                                    ui.small(match state {
                                        "connected" => self.t("Подключён ✓", "Online ✓"),
                                        "offline" => self.t("Отключён", "Offline"),
                                        "conflict" => self.t("Конфликт UUID", "UUID conflict"),
                                        "unbound" => self.t("Не привязан", "Unbound"),
                                        _ => self.t("Не проверен", "Unverified"),
                                    });
                                }
                                if ui
                                    .small_button("⋯")
                                    .on_hover_text(self.t("Свойства диска", "Disk properties"))
                                    .clicked()
                                {
                                    self.open_disk_properties(volume.id);
                                    self.shelf_editor = self.shelf.label(volume.id).to_owned();
                                }
                            });
                            if root_expanded {
                                self.draw_tree_children(ui, root_key, 1, ctx);
                            }
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
                self.protect_background(ui);
                ui.horizontal(|ui| {
                    ui.strong(self.t("Инспектор", "Inspector"));
                    ui.separator();
                    if let Some(integrity) = self.integrity.text(self.lang) {
                        ui.label(RichText::new(integrity).small()).on_hover_text(self.t(
                            "Проверка целостности описывает состояние SQLite на момент проверки.",
                            "An integrity check reports the SQLite state at the time of the check.",
                        ));
                        ui.separator();
                    }
                    ui.label(RichText::new(self.status.text(self.lang)).small().weak());
                    if self.active_request.is_some() { ui.spinner(); }
                });
                if self.ctf_job_id.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(self.ctf_phase_label());
                        ui.label(format!("{}: {}",self.t("Обработано записей","Entries processed"),self.ctf_entries));
                        if ui.add_enabled(self.ctf_job_registered && !self.ctf_cancel_requested,egui::Button::new(self.t("Отменить CTF-операцию (Esc)","Cancel CTF operation (Esc)"))).clicked(){self.cancel_ctf_job(ctx);}
                        if self.ctf_cancel_requested {ui.label(self.t("Отмена запрошена; ожидается откат.","Cancellation requested; waiting for rollback."));}
                    });
                }
                if self.scan_job_id.is_some() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{}: {}",
                            self.t("Обработано записей", "Entries processed"),
                            self.scan_progress_entries,
                        ));
                        if ui.add_enabled(
                            self.scan_registered && !self.scan_cancel_requested,
                            egui::Button::new(self.t("Отменить сканирование (Esc)", "Cancel scan (Esc)")),
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
                        if ui.add_enabled(self.active_request.is_none(),egui::Button::new(self.t("Описание…","Description…")).small()).clicked(){self.description_input=selected.description.clone();self.description_entry_id=Some(selected.id);}
                        let copies = ui.add_enabled(!selected.is_container(),
                            egui::Button::new(self.t("Кандидаты на других дисках", "Copies on other disks")));
                        if copies.clicked() { self.navigate_copies(ctx); }
                        if !selected.is_container() && ui.button(self.t("Открыть папку файла", "Open containing folder")).clicked() {
                            self.navigate_containing_folder(&selected, ctx);
                        }
                        if selected.is_container() && ui.button(self.t("Открыть папку", "Open folder")).clicked() {
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
            self.protect_background(ui);
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
                        .button(self.t("Открыть SQLite или CTF", "Open SQLite or CTF"))
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
            if matches!(&self.route, Route::Catalog) {
                self.catalog_overview_table(ui, ctx);
                return;
            }
            if let Route::Folder { volume_id, .. } | Route::BranchSearch { volume_id, .. } =
                &self.route
            {
                let volume_id = *volume_id;
                let disk_label = self
                    .volumes
                    .iter()
                    .find(|volume| volume.id == volume_id)
                    .map(|volume| format!("#{} · {}", volume.id, self.display_name(volume)))
                    .unwrap_or_else(|| format!("#{}", volume_id));
                let mut clicked = None;
                ui.horizontal_wrapped(|ui| {
                    if ui.link(&disk_label).clicked() {
                        clicked = Some(None);
                    }
                    let crumbs = self.folder_crumbs.clone();
                    for (index, crumb) in crumbs.iter().enumerate() {
                        ui.label("›");
                        if ui.link(&crumb.name).clicked() {
                            clicked = Some(Some(index));
                        }
                    }
                });
                if let Some(index) = clicked {
                    self.navigate_breadcrumb(volume_id, index, ctx);
                }
            }
            ui.horizontal(|ui| {
                let result_heading = match &self.route {
                    Route::Catalog => self.t("Обзор каталога", "Catalog overview").to_owned(),
                    Route::Search { .. } => {
                        self.t("Результаты поиска", "Search results").to_owned()
                    }
                    Route::BranchSearch {
                        parent_id,
                        exact_category_id,
                        ..
                    } => format!(
                        "{} #{parent_id}{} · {}",
                        self.t("Поиск в папке", "Folder search"),
                        exact_category_id
                            .map(|id| format!(" · {} #{id}", self.t("категория", "category")))
                            .unwrap_or_default(),
                        self.query_input
                    ),
                    Route::Folder { .. } => {
                        self.t("Содержимое каталога", "Folder contents").to_owned()
                    }
                    Route::Copies { .. } => self
                        .t(
                            "Кандидаты по имени и размеру · НЕ байтовая проверка",
                            "Name+size candidates · NOT byte verified",
                        )
                        .to_owned(),
                };
                ui.strong(result_heading);
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
            // Scroll the headers and rows together when resized columns cannot fit.
            let output = egui::ScrollArea::horizontal()
                .id_salt("results_horizontal_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut table = TableBuilder::new(ui)
                        .striped(true)
                        .resizable(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::initial(42.0).at_least(35.0))
                        .column(Column::initial(260.0).at_least(180.0).clip(true))
                        .column(Column::initial(86.0).at_least(72.0))
                        .column(Column::initial(92.0).at_least(80.0))
                        .column(Column::initial(235.0).at_least(90.0).clip(true));
                    if self.reveal_selected {
                        if let Some(index) = rows.iter().position(|row| Some(row.id) == selection) {
                            table = table.scroll_to_row(index, Some(egui::Align::Center));
                            self.reveal_selected = false;
                        }
                    }
                    if self.active_request.is_none() {
                        if let Some(scroll) = self.restore_scroll.take() {
                            table = table.vertical_scroll_offset(scroll);
                        }
                    }
                    table
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
                                    let name = if entry.is_container() {
                                        format!("▸ {}", entry.name)
                                    } else {
                                        entry.name.clone()
                                    };
                                    let result = ui
                                        .add(
                                            egui::Button::selectable(
                                                selection == Some(entry.id),
                                                &name,
                                            )
                                            .truncate(),
                                        )
                                        .on_hover_text(name);
                                    if result.clicked() {
                                        chosen = Some(entry.clone());
                                    }
                                    if result.double_clicked() && entry.is_container() {
                                        open_folder = Some(entry.clone());
                                    }
                                });
                                row.col(|ui| {
                                    ui.label(bytes_compact(entry.size));
                                });
                                row.col(|ui| {
                                    let kind = if entry.is_container() {
                                        "Folder"
                                    } else if ["mkv", "mp4", "avi", "m2ts", "mov", "webm"]
                                        .iter()
                                        .any(|ext| {
                                            entry.name.to_lowercase().ends_with(&format!(".{ext}"))
                                        })
                                    {
                                        "Video*"
                                    } else {
                                        "File"
                                    };
                                    ui.label(kind)
                                        .on_hover_text("Video* = filename extension only");
                                });
                                row.col(|ui| {
                                    ui.add(egui::Label::new(&entry.relative_path).truncate())
                                        .on_hover_text(&entry.relative_path);
                                });
                            });
                        })
                })
                .inner;
            self.results_scroll = output.state.clamped_offset().y;
            if let Some(entry) = chosen {
                self.selected = Some(entry);
            }
            if let Some(folder) = open_folder {
                self.navigate_folder(&folder, ctx);
            }
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mode) = self.ctf_dialog {
            let mut open = true;
            egui::Window::new(if mode == CtfDialog::Import {
                self.t("Импорт CTF", "Import CTF")
            } else {
                self.t("Новый CTF", "New CTF")
            })
            .open(&mut open)
            .collapsible(false)
            .default_width(560.0)
            .show(ctx, |ui| {
                if mode == CtfDialog::Import {
                    ui.label(self.t("Исходный CTF — только чтение:", "Source CTF — read-only:"));
                    ui.text_edit_singleline(&mut self.ctf_source_input);
                    ui.label(self.t(
                        "Новый рабочий SQLite (.sqlite):",
                        "New working SQLite (.sqlite):",
                    ));
                } else {
                    ui.label(self.t(
                        "Новый файл CTF (.ctf); исходники не перезаписываются:",
                        "New CTF file (.ctf); sources are never overwritten:",
                    ));
                }
                ui.text_edit_singleline(&mut self.ctf_target_input);
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::from_rgb(224, 96, 88), error);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.active_request.is_none(),
                            egui::Button::new(if mode == CtfDialog::Import {
                                self.t("Импортировать", "Import")
                            } else {
                                self.t("Экспортировать", "Export")
                            }),
                        )
                        .clicked()
                    {
                        self.start_ctf_transfer(ctx);
                    }
                    if ui.button(self.t("Закрыть", "Close")).clicked() {
                        self.ctf_dialog = None;
                    }
                });
            });
            if !open {
                self.ctf_dialog = None;
            }
        }
        if let Some(id) = self.description_entry_id {
            let mut open = true;
            egui::Window::new(self.t("Описание записи","Entry description")).open(&mut open).collapsible(false).default_width(560.0).show(ctx,|ui| {
                if self.linked_ctf.is_some(){ui.label(self.t("Связанный CTF доступен только для чтения. Для изменений импортируйте его в новый каталог.","Linked CTF is read-only. Import it into a new catalog to edit."));}
                ui.add_enabled(self.linked_ctf.is_none() && self.active_request.is_none(),egui::TextEdit::multiline(&mut self.description_input).desired_rows(6).desired_width(f32::INFINITY));
                if let Some(error)=&self.error{ui.colored_label(Color32::from_rgb(224,96,88),error);}
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.linked_ctf.is_none() && self.active_request.is_none(),egui::Button::new(self.t("Сохранить описание","Save description"))).clicked(){self.save_description(ctx);}
                    if ui.button(self.t("Закрыть","Close")).clicked(){self.description_entry_id=None;}
                });
                ui.label(format!("ID: {id}"));
            });
            if !open {
                self.description_entry_id = None;
            }
        }
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
                    if let Some(error) = &self.error {
                        ui.colored_label(Color32::from_rgb(224, 96, 88), error);
                    }
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
        if self.show_categories_dialog {
            let mut open = self.show_categories_dialog;
            egui::Window::new(self.t("Категории каталога", "Catalog categories"))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(560.0)
                .show(ctx, |ui| {
                    ui.label(self.t(
                        "Категории и назначения изменяют открытый SQLite. Исходный CTF не открывается.",
                        "Categories and assignments modify the open SQLite. Original CTF is not opened.",
                    ));
                    let selected = self.selected.clone();
                    let catalog_categories = self.categories.clone();
                    let category_by_id: HashMap<i64, &Category> = catalog_categories
                        .iter()
                        .map(|category| (category.id, category))
                        .collect();
                    if let Some(entry) = &selected {
                        ui.label(format!(
                            "{}: #{} · {}",
                            self.t("Выбрана запись", "Selected entry"),
                            entry.id,
                            entry.name
                        ));
                        ui.label(format!(
                            "{} #{} · {}",
                            self.t("Диск каталога", "Catalog disk"),
                            entry.volume_id,
                            entry.relative_path
                        ));
                        if entry.is_directory() {
                            ui.label(self.t(
                                "Назначение папке также включит её вложенные записи в категорийный поиск.",
                                "Assigning a folder also includes its recorded descendants in category search.",
                            ));
                        }
                        if self.category_assignments_ready {
                            ui.label(format!(
                                "{}: {}",
                                self.t("Прямых назначений", "Direct assignments"),
                                self.category_assigned.len()
                            ));
                            ui.label(self.t(
                                "Наследованные через папки/подкатегории совпадения здесь не отмечены.",
                                "Inherited folder/subcategory matches are not shown as direct assignments.",
                            ));
                        } else if self.category_assignments_error {
                            ui.colored_label(
                                Color32::from_rgb(224, 96, 88),
                                self.t(
                                    "Назначения не загружены. Изменения отключены.",
                                    "Assignments unavailable. Changes are disabled.",
                                ),
                            );
                            if ui
                                .add_enabled(
                                    self.active_request.is_none(),
                                    egui::Button::new(self.t("Повторить загрузку", "Retry loading")),
                                )
                                .clicked()
                            {
                                self.load_assigned_categories(ctx);
                            }
                        } else {
                            ui.label(self.t("Загрузка назначений…", "Loading assignments…"));
                        }
                    } else {
                        ui.label(self.t(
                            "Чтобы назначить категорию, выделите файл в результатах до открытия окна.",
                            "To assign a category, select a result before opening this dialog.",
                        ));
                    }
                    ui.separator();
                    if self.categories_loaded && catalog_categories.is_empty() {
                        ui.label(self.t("Категорий пока нет.", "No categories yet."));
                    }
                    if !self.categories_loaded && self.active_request.is_none() {
                        ui.label(self.t(
                            "Справочник категорий не загружен; переходы и изменения отключены.",
                            "Categories not loaded; navigation and edits are disabled.",
                        ));
                        if ui.button(self.t("Повторить загрузку", "Retry loading")).clicked() {
                            self.load_categories(ctx);
                        }
                    }
                    egui::ScrollArea::vertical().max_height(260.0).show_rows(
                        ui,
                        28.0,
                        catalog_categories.len(),
                        |ui, visible_rows| {
                        for row in visible_rows {
                            let category = &catalog_categories[row];
                            ui.horizontal(|ui| {
                                let title = categories::display_path(category, &category_by_id);
                                if ui
                                    .add_enabled(
                                        self.active_request.is_none() && self.categories_loaded,
                                        egui::Button::new(title),
                                    )
                                    .on_hover_text(self.t(
                                        "Показать записи этой категории на всех дисках",
                                        "Show this category across all catalog disks",
                                    ))
                                    .clicked()
                                {
                                    self.navigate_category(category, ctx);
                                    self.show_categories_dialog = false;
                                }
                                if let Some(entry) = &selected {
                                    let assigned = self.category_assigned.contains(&category.id);
                                    let action_label = if assigned {
                                        self.t("Снять", "Remove")
                                    } else {
                                        self.t("Назначить", "Assign")
                                    };
                                    let action_hint = if assigned {
                                        self.t(
                                            "Удалить только связь категории в SQLite, не файл; назначение можно восстановить",
                                            "Remove only the SQLite category link, never the file; reversible",
                                        )
                                    } else {
                                        self.t(
                                            "Назначить категорию выбранной записи каталога SQLite",
                                            "Assign this category to the selected SQLite entry",
                                        )
                                    };
                                    if assigned {
                                        ui.label("✓");
                                    }
                                    if ui
                                        .add_enabled(
                                            self.active_request.is_none()
                                                && self.categories_loaded
                                                && self.category_assignments_ready,
                                            egui::Button::new(action_label),
                                        )
                                        .on_hover_text(action_hint)
                                        .clicked()
                                    {
                                        if assigned {
                                            self.unassign_category(entry.id, category.id, ctx);
                                        } else {
                                            self.assign_category(entry.id, category.id, ctx);
                                        }
                                    }
                                }
                            });
                        }
                    });
                    ui.separator();
                    ui.strong(self.t("Новая категория", "New category"));
                    let name_hint = self.t("Название", "Name");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.category_name_input)
                            .hint_text(name_hint)
                            .desired_width(f32::INFINITY),
                    );
                    let parent_name = self
                        .category_parent_id
                        .and_then(|id| category_by_id.get(&id).copied())
                        .map(|category| categories::display_path(category, &category_by_id))
                        .unwrap_or_else(|| self.t("Без родителя", "No parent").to_owned());
                    let no_parent = self.t("Без родителя", "No parent");
                    ui.horizontal(|ui| {
                        ui.label(self.t("Родитель", "Parent"));
                        ui.add_enabled_ui(
                            self.categories_loaded && self.active_request.is_none(),
                            |ui| {
                        egui::ComboBox::from_id_salt("new_category_parent")
                            .selected_text(parent_name)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.category_parent_id,
                                    None,
                                    no_parent,
                                );
                                for category in &catalog_categories {
                                    ui.selectable_value(
                                        &mut self.category_parent_id,
                                        Some(category.id),
                                        categories::display_path(category, &category_by_id),
                                    );
                                }
                            });
                        });
                    });
                    let can_create = self.active_request.is_none()
                        && self.categories_loaded
                        && categories::new_name(&self.category_name_input).is_ok();
                    if ui
                        .add_enabled(
                            can_create,
                            egui::Button::new(self.t("Создать категорию", "Create category")),
                        )
                        .clicked()
                    {
                        self.create_category(ctx);
                    }
                    if let Some(error) = &self.error {
                        ui.colored_label(Color32::from_rgb(224, 96, 88), error);
                    }
                    if ui.button(self.t("Закрыть", "Close")).clicked() {
                        self.show_categories_dialog = false;
                    }
                });
            self.show_categories_dialog &= open;
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
                        if ui.add_enabled(self.active_request.is_none()
                            && self.workspace_tab == WorkspaceTab::Search,
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
            egui::Window::new(self.t("Открыть каталог", "Open catalog"))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(510.0)
                .show(ctx, |ui| {
                    ui.label(self.t(
                        "SQLite открывается как рабочий каталог; CTF связывается только для чтения.",
                        "SQLite opens as a working catalog; CTF is linked read-only.",
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
                let mut conflicting: Vec<i64> = self
                    .disk_preferences
                    .iter()
                    .filter_map(|(other, p)| {
                        (*other != id
                            && !normalized.is_empty()
                            && p.shelf_label
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                                .to_lowercase()
                                == normalized)
                            .then_some(*other)
                    })
                    .collect();
                conflicting.sort_unstable();
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
                        if self.preferences_ready {
                        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                            ui.strong(format!("#{} · {}", volume.id, volume.label));
                            ui.label(self.t("Локальное имя и свойства хранятся отдельно; исходный каталог и его экспортируемое имя не меняются.",
                                "Local name and properties are stored separately; the source catalog and its exported name stay unchanged."));
                            ui.label(local_name);
                            let name_response = ui.add(egui::TextEdit::singleline(&mut self.disk_draft.values.display_name).id(egui::Id::unique(DISK_NAME_INPUT_ID)).desired_width(f32::INFINITY));
                            if self.focus_disk_name { name_response.request_focus(); self.focus_disk_name = false; }
                            ui.label(shelf);
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.disk_draft.values.shelf_label).desired_width((ui.available_width() - 90.0).max(80.0)));
                                if ui.button(self.t("Очистить", "Clear")).clicked() { self.disk_draft.values.shelf_label.clear(); }
                            });
                            if !conflicting.is_empty() { ui.colored_label(Color32::from_rgb(224, 160, 40), format!("{}: {}", self.t("Одинаковая наклейка у дисков", "Shared sticker on disks"), conflicting.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(", "))); }
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
                        } else {
                        ui.label(self.t("Расширенные свойства недоступны; наклейку можно изменить только для поддерживаемого старого каталога.", "Extended properties are unavailable; sticker editing is available only for a supported legacy catalog."));
                        ui.strong(format!("#{} · {}", volume.id, volume.label));
                        ui.label(format!(
                            "{}: {}",
                            self.t("Записано файлов/папок", "Recorded entries"),
                            volume.entry_count
                        ));
                        ui.label(format!(
                            "{}: {}",
                            self.t("Логический объём", "Logical total bytes"),
                            bytes_compact(volume.total_bytes)
                        ));
                        ui.label(format!(
                            "{}: {}",
                            self.t("Исторический корень", "Recorded root"),
                            volume.root_path
                        ));
                        ui.separator();
                        ui.label(self.t(
                            "Наклейка на полке (ваша заметка, не UUID и не USB-статус):",
                            "Physical shelf sticker (your hint, not a UUID or USB status):",
                        ));
                        let hint = self.t("Например: BOX-17", "For example: BOX-17");
                        ui.add_enabled(
                            self.active_request.is_none(),
                            egui::TextEdit::singleline(&mut self.shelf_editor).hint_text(hint),
                        );
                        let conflicting = self.shelf.collision_other_disks(volume.id);
                        if !conflicting.is_empty() {
                            ui.colored_label(
                                Color32::from_rgb(230, 175, 55),
                                format!(
                                    "{}: {}",
                                    self.t(
                                        "Одинаковая наклейка у дисков",
                                        "Shared sticker on disks"
                                    ),
                                    conflicting
                                        .iter()
                                        .map(|i| format!("#{i}"))
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                ),
                            );
                        }
                        if ui
                            .add_enabled(
                                self.active_request.is_none() && self.shelf_context.is_some() && self.legacy_shelf_editable,
                                egui::Button::new(
                                    self.t("Сохранить наклейку", "Save shelf sticker"),
                                ),
                            )
                            .clicked()
                        {
                            self.save_shelf_label(volume.id, ctx);
                        }
                        ui.label(
                            RichText::new(self.t(
                                "Это НЕ подтверждённый текущий путь или подключённый USB.",
                                "This is NOT a verified current mount or physical USB connection.",
                            ))
                            .small()
                            .weak(),
                        );
                        if let Some(error) = &self.error {
                            ui.colored_label(Color32::from_rgb(224, 96, 88), error);
                        }
                        ui.add_space(3.0);
                        if ui.button(self.t("Закрыть", "Close")).clicked() {
                            self.show_disk_properties = None;
                        }
                        }
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
        if self.dialog_open() {
            return;
        }
        // Use modifiers attached to the key event: the user can release Ctrl
        // before the next frame, leaving the frame's final modifiers empty.
        let (search, properties, all, back, cancel_scan, activate_folder) = ctx.input(|i| {
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
                has(egui::Key::Escape, |m| {
                    !m.alt && !m.ctrl && !m.shift && !m.command
                }),
                has(egui::Key::Enter, |m| {
                    !m.alt && !m.ctrl && !m.shift && !m.command
                }),
            )
        });
        if search {
            self.switch_workspace(WorkspaceTab::Search);
            self.workspace_tab_focus_request = false;
            ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique(SEARCH_INPUT_ID)));
        }
        if all && self.workspace_tab == WorkspaceTab::Search {
            self.navigate_disk(None, ctx);
        }
        if properties && self.active_request.is_none() {
            let selected = match self.workspace_tab {
                WorkspaceTab::Search => self.selected_volume,
                WorkspaceTab::UsbDisks => self.disks_selected_volume,
                WorkspaceTab::Slices | WorkspaceTab::VolumeMap | WorkspaceTab::LocalPc => None,
            };
            if let Some(id) = selected {
                self.open_disk_properties(id);
            }
        }
        if back && self.workspace_tab == WorkspaceTab::Search {
            self.go_back(ctx);
        }
        if activate_folder
            && self.workspace_tab == WorkspaceTab::Search
            && !ctx.egui_wants_keyboard_input()
            && self.active_request.is_none()
        {
            if let Some(entry) = self.selected.clone().filter(Entry::is_container) {
                self.navigate_folder(&entry, ctx);
            }
        }
        if cancel_scan {
            let canceled_workspace_analytics = match self.workspace_tab {
                WorkspaceTab::VolumeMap if self.analytics_job_id.is_some() => {
                    self.cancel_analytics(ctx, false, true);
                    true
                }
                WorkspaceTab::Slices if self.slices_job_id.is_some() => {
                    self.cancel_analytics(ctx, true, true);
                    true
                }
                _ => false,
            };
            if !canceled_workspace_analytics {
                if self.ctf_job_id.is_some() {
                    self.cancel_ctf_job(ctx);
                } else {
                    self.request_scan_cancel(ctx);
                }
            }
        }
    }
}

impl eframe::App for DesktopApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(
            LANGUAGE_SETTING,
            match self.lang {
                Language::Russian => "ru",
                Language::English => "en",
            }
            .to_owned(),
        );
        storage.set_string(
            THEME_SETTING,
            if self.dark { "dark" } else { "light" }.to_owned(),
        );
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        #[cfg(feature = "qa-screenshots")]
        crate::qa_screenshot::capture_on_f12(&ctx);
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
        self.poll_ctf_progress(&ctx);
        self.handle_shortcuts(&ctx);
        self.menu(root, &ctx);
        self.workspace_tabs(root, &ctx);
        match self.workspace_tab {
            WorkspaceTab::Search => {
                self.cancel_analytics(&ctx, false, false);
                self.cancel_analytics(&ctx, true, false);
                self.search_toolbar(root, &ctx);
                self.disk_sidebar(root, &ctx);
                self.inspector(root, &ctx);
                self.results_table(root, &ctx);
            }
            WorkspaceTab::UsbDisks => {
                self.cancel_analytics(&ctx, false, false);
                self.cancel_analytics(&ctx, true, false);
                self.disks_workspace(root, &ctx)
            }
            WorkspaceTab::Slices => {
                self.cancel_analytics(&ctx, false, false);
                self.analytics_workspace(root, &ctx, true)
            }
            WorkspaceTab::VolumeMap => {
                self.cancel_analytics(&ctx, true, false);
                self.analytics_workspace(root, &ctx, false)
            }
            WorkspaceTab::LocalPc => {
                self.cancel_analytics(&ctx, false, false);
                self.cancel_analytics(&ctx, true, false);
                let volumes: Vec<_> = self
                    .volumes
                    .iter()
                    .map(|v| (v.id, v.label.clone()))
                    .collect();
                self.local_analyzer.show(
                    root,
                    &mut self.bus,
                    &ctx,
                    self.db.as_deref(),
                    &volumes,
                    self.lang == Language::Russian,
                )
            }
        }
        self.dialogs(&ctx);
        self.physical
            .close_if_scope_changed(self.shelf_context.as_ref().map(|s| s.catalog_key.as_str()));
        self.physical.show(&ctx, self.lang == Language::Russian);
    }
}

#[cfg(test)]
#[path = "ctf_workflow_tests.rs"]
mod ctf_workflow_tests;

#[cfg(test)]
#[path = "catalog_navigation_tests.rs"]
mod catalog_navigation_tests;

#[cfg(test)]
mod shortcut_tests {
    use super::*;

    #[test]
    fn analytics_stale_generation_cannot_replace_current_view() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("catalog.sqlite".into());
        app.analytics_job_id = Some("new-job".into());
        app.analytics_request_id = Some(2);
        app.analytics_generation = 4;
        app.analytics_data = Some(Arc::new(json!({"root":{"id":"kept"}})));
        app.apply_reply(
            crate::bridge::Reply {
                id: 1,
                purpose: Purpose::Analytics {
                    job_id: "old-job".into(),
                    generation: 3,
                    db: "catalog.sqlite".into(),
                    slices: false,
                    linked_generation: None,
                },
                response: json!({"ok":true,"data":{"root":{"id":"stale"}}}),
            },
            &ctx,
        );
        assert_eq!(app.analytics_data.as_ref().unwrap()["root"]["id"], "kept");
        assert_eq!(app.analytics_job_id.as_deref(), Some("new-job"));
    }

    #[test]
    fn catalog_clear_resets_numeric_analytics_scopes_for_map_and_slices() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("old-catalog.sqlite".into());
        app.analytics_volume = Some(7);
        app.analytics_folder = Some(91);
        app.analytics_category_id = Some(12);
        app.slices_volume = Some(7);
        app.slices_category = Some(12);

        app.clear_catalog();

        assert!(app.analytics_volume.is_none());
        assert!(app.analytics_folder.is_none());
        assert!(app.analytics_category_id.is_none());
        assert!(app.slices_volume.is_none());
        assert!(app.slices_category.is_none());
    }

    #[test]
    fn explicit_analytics_cancel_does_not_restart_until_retry() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.slices_job_id = Some("slice-job".into());
        app.slices_request_id = Some(8);
        app.slices_generation = 11;
        app.cancel_analytics(&ctx, true, true);
        assert!(app.slices_job_id.is_none());
        assert!(app.slices_loaded);
        assert!(app.slices_cancelled);
        assert!(app.slices_generation > 11);
        assert!(app.slices_data.is_none());
    }

    #[test]
    fn map_filter_refresh_keeps_folder_in_captured_analytics_request() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        let temp = tempfile::tempdir().unwrap();
        app.db = Some(
            temp.path()
                .join("catalog.sqlite")
                .to_string_lossy()
                .into_owned(),
        );
        app.analytics_volume = Some(1);
        app.analytics_folder = Some(91);
        app.analytics_group = "extensions".into();
        app.analytics_highlight = "ext:mkv".into();
        let captured = Arc::new(std::sync::Mutex::new(None::<Value>));
        let captured_worker = captured.clone();
        let missing_db = temp
            .path()
            .join("missing-readonly.sqlite")
            .to_string_lossy()
            .into_owned();
        app.bus.set_handler(move |request| {
            *captured_worker.lock().unwrap() = Some(request.clone());
            let mut request = request.clone();
            request["db"] = Value::String(missing_db.clone());
            let raw = atlas_service::dispatch_json(&request.to_string());
            serde_json::from_str(&raw).unwrap()
        });

        // This is the same refresh path used by Apply/group controls in the map.
        app.request_analytics(&ctx, false);
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && captured.lock().unwrap().is_none() {
            std::thread::sleep(Duration::from_millis(1));
        }
        while Instant::now() < deadline && app.bus.poll().is_none() {
            std::thread::sleep(Duration::from_millis(1));
        }

        let request = captured
            .lock()
            .unwrap()
            .clone()
            .expect("analytics request captured");
        assert_eq!(request["volume_id"], 1);
        assert_eq!(request["folder_entry_id"], 91);
        assert_eq!(request["group_by"], "extensions");
        assert_eq!(request["highlight_query"], "ext:mkv");
    }

    #[test]
    fn analytics_navigation_does_not_dispatch_behind_a_modal() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        let route = app.route.clone();
        app.show_export_dialog = true;
        app.jump_analytics_to_search(&ctx, Some(2), Some(9), "ext:mkv");
        assert_eq!(app.route, route);
        assert!(app.history.is_empty());
    }

    #[test]
    fn submitting_a_new_query_keeps_branch_and_exact_category_scope() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("synthetic.sqlite".into());
        app.bus.set_handler(
            |_| json!({"ok":true,"data":{"items":[],"offset":0,"limit":250,"has_more":false}}),
        );
        app.route = Route::BranchSearch {
            volume_id: 3,
            parent_id: 44,
            query: "ext:mkv".into(),
            exact_category_id: Some(12),
        };
        app.query_input = "size:>1GiB".into();

        app.submit_search(&ctx);

        assert_eq!(
            app.route,
            Route::BranchSearch {
                volume_id: 3,
                parent_id: 44,
                query: "size:>1GiB".into(),
                exact_category_id: Some(12),
            }
        );
        assert_eq!(
            app.route.request("synthetic.sqlite", 0, 250)["query"],
            "branch_id:44 AND (category_exact_id:12 AND (size:>1GiB))"
        );
    }

    #[test]
    fn bound_rescan_is_disabled_in_the_windows_1_0_ui() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir
            .path()
            .join("generated.sqlite")
            .to_string_lossy()
            .into_owned();
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some(db.clone());
        app.volumes.push(Volume {
            id: 7,
            label: "Synthetic".into(),
            root_path: "DO_NOT_SCAN_HISTORICAL_ROOT".into(),
            entry_count: 0,
            total_bytes: 0,
        });
        app.shelf_context = Some(ShelfContext {
            user_store: dir
                .path()
                .join("private.sqlite")
                .to_string_lossy()
                .into_owned(),
            catalog_key: format!("sqlite:{db}"),
            platform: "windows",
        });
        let (sent, received) = std::sync::mpsc::channel();
        app.bus.set_handler(move |request| {
            sent.send(request.clone()).unwrap();
            json!({"ok":false,"error":{"code":"unpaired_volume","message":"No saved pairing"}})
        });
        app.confirm_rescan = true;
        app.start_rescan(7, &ctx);
        assert!(app.active_request.is_none());
        assert!(app.scan_job_id.is_none());
        assert_eq!(app.volumes.len(), 1);
        assert_eq!(app.db.as_deref(), Some(db.as_str()));
        assert!(received.try_recv().is_err());
        app.linked_ctf = Some(LinkedCtf {
            path: "source.ctf".into(),
            cache_root: "cache".into(),
            generation: "a".repeat(64),
            source_sha256: "b".repeat(64),
            preview: false,
        });
        app.confirm_rescan = true;
        app.start_rescan(7, &ctx);
        assert!(app.active_request.is_none());
        assert!(received.try_recv().is_err());
    }

    fn wait_for_catalog_reply(app: &DesktopApp) -> crate::bridge::Reply {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(reply) = app.bus.poll() {
                return reply;
            }
            assert!(Instant::now() < deadline, "catalog operation did not reply");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn existing_catalog_open_dispatches_probe_without_full_validation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic.sqlite");
        let init = atlas_service::dispatch_json(&json!({"op":"init","db":path}).to_string());
        assert_eq!(serde_json::from_str::<Value>(&init).unwrap()["ok"], true);
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, Some(path.to_string_lossy().into_owned()));
        let (requests, observed) = std::sync::mpsc::channel();
        app.bus.set_handler(move |request| {
            requests.send(request["op"].clone()).unwrap();
            match request["op"].as_str() {
                Some("validate_catalog") => {
                    json!({"ok":false,"error":{"message":"Full validation trap"}})
                }
                // Do not read an existing user settings sidecar in this test.
                Some("disk_preferences_list") => {
                    json!({"ok":true,"data":{"items":[],"collisions":[]}})
                }
                _ => serde_json::from_str(&atlas_service::dispatch_json(&request.to_string()))
                    .unwrap(),
            }
        });
        app.start_catalog(false, &ctx);
        let reply = wait_for_catalog_reply(&app);
        assert_eq!(observed.try_recv().unwrap(), "probe_catalog");
        app.apply_reply(reply, &ctx);
        assert_eq!(app.db.as_deref(), path.to_str());
        assert_eq!(app.integrity, CatalogIntegrity::Unchecked);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.active_request.is_some() {
            assert!(Instant::now() < deadline, "catalog did not finish loading");
            let reply = wait_for_catalog_reply(&app);
            app.apply_reply(reply, &ctx);
        }
        let followups: Vec<_> = observed.try_iter().collect();
        assert!(followups
            .iter()
            .all(|op| op != "search_fast" && op != "children"));
        assert_eq!(app.route, Route::Catalog);
        assert!(app.page.items.is_empty());
        assert!(followups.iter().all(|op| op != "validate_catalog"));
        assert_eq!(app.integrity, CatalogIntegrity::Unchecked);
        assert!(app.error.is_none(), "{:?}", app.error);
    }

    #[test]
    fn invalid_probe_does_not_replace_current_catalog() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("old.sqlite".into());
        app.integrity = CatalogIntegrity::Verified;
        for invalid in [
            json!({"schema_version":9,"readonly":true,"full_validation":false}),
            json!({"schema_version":3,"readonly":false,"full_validation":false}),
            json!({"schema_version":3,"readonly":true,"full_validation":true}),
        ] {
            app.active_request = Some(9);
            app.apply_reply(
                crate::bridge::Reply {
                    id: 9,
                    purpose: Purpose::Open {
                        path: "new.sqlite".into(),
                    },
                    response: json!({"ok":true,"data":invalid}),
                },
                &ctx,
            );
            assert_eq!(app.db.as_deref(), Some("old.sqlite"));
            assert_eq!(app.integrity, CatalogIntegrity::Verified);
            assert!(app.error.is_some());
            assert!(app.active_request.is_none());
        }
    }

    #[test]
    fn explicit_integrity_check_dispatches_and_failure_clears_catalog_state() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("synthetic.sqlite".into());
        app.preferences_ready = true;
        app.categories_loaded = true;
        app.category_assignments_ready = true;
        app.selected_volume = Some(1);
        app.offset = 250;
        let entry = Entry::parse(&json!({
            "id":42,"volume_id":1,"name":"entry.txt","relative_path":"entry.txt","kind":"file","size":1,
        })).unwrap();
        app.page.items.push(entry.clone());
        app.selected = Some(entry);
        let (requests, observed) = std::sync::mpsc::channel();
        app.bus.set_handler(move |request| {
            requests.send(request.clone()).unwrap();
            json!({"ok":false,"error":{"code":"integrity","message":"Synthetic failure"}})
        });
        app.check_catalog_integrity(&ctx);
        assert_eq!(app.integrity, CatalogIntegrity::Checking);
        let reply = wait_for_catalog_reply(&app);
        assert_eq!(
            observed.try_recv().unwrap(),
            json!({"op":"validate_catalog","db":"synthetic.sqlite"})
        );
        app.apply_reply(reply, &ctx);
        assert_eq!(app.integrity, CatalogIntegrity::Failed);
        assert_eq!(app.error.as_deref(), Some("Synthetic failure"));
        assert!(app.db.is_none());
        assert!(app.selected_volume.is_none());
        assert!(app.selected.is_none());
        assert_eq!(app.offset, 0);
        assert!(app.page.items.is_empty());
        assert!(
            !app.preferences_ready && !app.categories_loaded && !app.category_assignments_ready
        );
    }

    #[test]
    fn integrity_reply_is_bound_to_the_current_catalog() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("current.sqlite".into());
        app.integrity = CatalogIntegrity::Unchecked;
        let response = json!({"ok":true,"data":{
            "schema_version":3,"quick_check":"ok","readonly":true,"foreign_key_errors":0
        }});
        for (id, path, expected) in [
            (7, "prior.sqlite", CatalogIntegrity::Unchecked),
            (8, "current.sqlite", CatalogIntegrity::Verified),
        ] {
            app.active_request = Some(id);
            app.apply_reply(
                crate::bridge::Reply {
                    id,
                    purpose: Purpose::CheckCatalogIntegrity { path: path.into() },
                    response: response.clone(),
                },
                &ctx,
            );
            assert_eq!(app.integrity, expected);
        }
    }

    #[test]
    fn register_order_matches_reference_and_breaks_size_ties_by_id() {
        let volume = |id, total_bytes| Volume {
            id,
            label: format!("Fixture {id}"),
            root_path: String::new(),
            entry_count: 0,
            total_bytes,
        };
        let mut volumes = vec![volume(9, 20), volume(3, 20), volume(1, 5)];
        sort_register(&mut volumes, RegisterOrder::Id);
        assert_eq!(volumes.iter().map(|v| v.id).collect::<Vec<_>>(), [1, 3, 9]);
        sort_register(&mut volumes, RegisterOrder::IndexedBytes);
        assert_eq!(volumes.iter().map(|v| v.id).collect::<Vec<_>>(), [3, 9, 1]);
    }

    #[test]
    fn results_path_stays_visible_when_window_narrows() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("synthetic.sqlite".into());
        app.lang = Language::English;
        app.route = Route::Search {
            query: "synthetic".into(),
            volume_id: None,
        };
        app.page.items.push(
            Entry::parse(&json!({
                "id": 1, "volume_id": 1, "kind": "file", "size": 1,
                "name": format!("{}.mkv", "ДлинноеИмя".repeat(24)),
                "relative_path": format!("Папка/{}", "Вложенная/".repeat(24)),
            }))
            .unwrap(),
        );
        let first = app.page.items[0].clone();
        app.page.items.extend((2..=250).map(|id| Entry {
            id,
            name: format!("Файл-{id}.mkv"),
            relative_path: format!("Папка/Файл-{id}.mkv"),
            ..first.clone()
        }));

        let mut paint = |width, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 768.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| {
                    app.disk_sidebar(root, &ctx);
                    app.results_table(root, &ctx);
                },
            );
            output.textures_delta.clear();
            output.shapes
        };
        let fully_visible = |shapes: &[egui::epaint::ClippedShape], text: &str| {
            shapes.iter().any(|clipped| match &clipped.shape {
                egui::epaint::Shape::Text(shape) if shape.galley.text().starts_with(text) => {
                    shape.pos.x >= clipped.clip_rect.min.x
                        && shape.pos.x + shape.galley.size().x <= clipped.clip_rect.max.x
                }
                _ => false,
            })
        };
        for width in [1366.0, 1366.0, 1024.0, 1024.0] {
            let shapes = paint(width, Vec::new());
            if width == 1024.0 {
                for text in ["Path", "Папка/"] {
                    assert!(
                        fully_visible(&shapes, text),
                        "{text} is clipped at width {width}"
                    );
                }
            }
        }
        let wheel = |delta| {
            vec![
                egui::Event::PointerMoved(egui::pos2(550.0, 170.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta,
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let width = 880.0;
        let _ = paint(width, Vec::new());
        let shapes = paint(width, Vec::new());
        assert!(!fully_visible(&shapes, "Папка/Вложенная/"));
        let _ = paint(width, wheel(egui::vec2(-300.0, 0.0)));
        let shapes = (0..30).map(|_| paint(width, Vec::new())).last().unwrap();
        assert!(fully_visible(&shapes, "Path"));
        assert!(fully_visible(&shapes, "Папка/Вложенная/"));

        let _ = paint(width, wheel(egui::vec2(0.0, -500.0)));
        for _ in 0..30 {
            let _ = paint(width, Vec::new());
        }
        assert!(app.results_scroll > 0.0);
    }

    #[test]
    fn register_filter_matches_id_sticker_and_unicode_note() {
        let volume = Volume {
            id: 61,
            label: "Original".into(),
            root_path: String::new(),
            entry_count: 0,
            total_bytes: 0,
        };
        assert!(register_matches(
            &volume,
            "Локальный",
            "BOX-17",
            "Семейное Кино",
            "#61"
        ));
        assert!(register_matches(
            &volume,
            "Локальный",
            "BOX-17",
            "Семейное Кино",
            "box-17"
        ));
        assert!(register_matches(
            &volume,
            "Локальный",
            "BOX-17",
            "Семейное Кино",
            "кино"
        ));
        assert!(!register_matches(
            &volume,
            "Локальный",
            "BOX-17",
            "Семейное Кино",
            "#62"
        ));
    }

    #[test]
    fn category_membership_buttons_fail_closed_until_service_confirms() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.categories.push(Category {
            id: 7,
            parent_id: None,
            name: "Synthetic".into(),
        });
        app.assign_category(12, 7, &ctx);
        app.unassign_category(12, 7, &ctx);
        assert!(app.active_request.is_none());
        assert!(!app.category_assignments_ready);
        assert!(app.category_assigned.is_empty());
    }

    #[test]
    fn category_mutation_missing_entry_discards_selection_and_refreshes_page() {
        let ctx = egui::Context::default();
        let candidate = json!({
            "id": 19, "volume_id": 2, "kind": "file", "size": 42,
            "name": "Кино.mkv", "relative_path": "Кино.mkv",
        });
        let entry = Entry::parse(&candidate).unwrap();
        let route = Route::Search {
            query: "category_id:7".into(),
            volume_id: None,
        };
        for purpose in [
            Purpose::AssignCategory {
                entry_id: 19,
                category_id: 7,
            },
            Purpose::UnassignCategory {
                entry_id: 19,
                category_id: 7,
            },
        ] {
            let mut app = DesktopApp::with_context(&ctx, None);
            app.route = route.clone();
            app.offset = PAGE_LIMIT;
            app.history
                .push((route.clone(), 0, None, None, 0.0, Vec::new(), String::new()));
            app.show_categories_dialog = true;
            app.selected = Some(entry.clone());
            app.page.items = vec![entry.clone()];
            app.category_assigned.insert(7);
            app.category_assignments_ready = true;

            for (id, code) in [(33, "database_busy"), (34, "not_found")] {
                app.active_request = Some(id);
                app.apply_reply(
                    crate::bridge::Reply {
                        id,
                        purpose: purpose.clone(),
                        response: json!({"ok":false, "error":{"code":code,"message":"Failed"}}),
                    },
                    &ctx,
                );
                assert_eq!(app.error.as_deref(), Some("Failed"));
                if code != "not_found" {
                    assert!(app.category_assignments_ready);
                    assert_eq!(app.selected.as_ref().map(|entry| entry.id), Some(19));
                    assert_eq!(app.page.items.len(), 1);
                }
            }

            assert!(!app.category_assignments_ready);
            assert!(app.category_assigned.is_empty());
            assert!(app.selected.is_none());
            assert!(app.restore_selection.is_none());
            assert!(app.page.items.is_empty());
            assert!(app.category_result_refresh_pending);
            assert_eq!(app.offset, PAGE_LIMIT);
            assert_eq!(app.history.len(), 1);

            let mut fresh = candidate.clone();
            fresh["id"] = json!(20);
            app.active_request = Some(35);
            app.apply_reply(
                crate::bridge::Reply {
                    id: 35,
                    purpose: Purpose::Page {
                        route: route.clone(),
                    },
                    response: json!({"ok":true, "data":{"items":[fresh],
                        "offset":PAGE_LIMIT, "limit":PAGE_LIMIT, "has_more":false}}),
                },
                &ctx,
            );
            assert!(!app.category_result_refresh_pending);
            assert!(app.selected.is_none());
            assert_eq!(app.page.items[0].id, 20);
            assert_eq!(app.offset, PAGE_LIMIT);
            assert_eq!(app.history.len(), 1);
        }
    }

    #[test]
    fn membership_write_invalidates_stale_results_and_selected_entry() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        let entry = Entry {
            id: 19,
            volume_id: 2,
            parent_id: None,
            name: "Кино.mkv".into(),
            relative_path: "Кино.mkv".into(),
            kind: "file".into(),
            size: 42,
            modified_unix: None,
            description: String::new(),
            tags: String::new(),
        };
        app.route = Route::Search {
            query: "category_id:7".into(),
            volume_id: None,
        };
        app.selected = Some(entry.clone());
        app.page.items = vec![entry];
        app.category_assigned.insert(7);
        app.category_assignments_ready = true;
        app.refresh_category_results(&ctx);
        assert!(
            app.page.items.is_empty(),
            "stale category results must disappear"
        );
        assert!(app.category_result_refresh_pending);
        assert!(!app.category_assignments_ready);
        assert!(app.category_assigned.is_empty());
        assert_eq!(app.restore_selection.as_ref().map(|e| e.id), Some(19));

        // SQLite no longer returns the unassigned item. UI selection must
        // disappear, and reloading the result must not claim an old category.
        app.active_request = Some(33);
        app.apply_reply(
            crate::bridge::Reply {
                id: 33,
                purpose: Purpose::Page {
                    route: app.route.clone(),
                },
                response: json!({"ok":true, "data":{"items":[],
                    "offset":0, "limit":250, "has_more":false}}),
            },
            &ctx,
        );
        assert!(!app.category_result_refresh_pending);
        assert!(app.selected.is_none());
        assert!(app.page.items.is_empty());
        assert!(!app.category_assignments_ready);
    }

    #[test]
    fn membership_refresh_steps_back_from_empty_high_offset_page() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.route = Route::Search {
            query: "category_id:7".into(),
            volume_id: None,
        };
        app.offset = 250;
        app.category_result_refresh_pending = true;
        app.active_request = Some(34);
        app.apply_reply(
            crate::bridge::Reply {
                id: 34,
                purpose: Purpose::Page {
                    route: app.route.clone(),
                },
                response: json!({"ok":true, "data":{"items":[],
                    "offset":250, "limit":250, "has_more":false}}),
            },
            &ctx,
        );
        assert_eq!(app.offset, 0);
        assert!(app.category_result_refresh_pending);
    }

    #[test]
    fn back_from_removed_category_last_page_recovers_previous_page() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        let category_route = Route::Search {
            query: "category_id:7".into(),
            volume_id: None,
        };
        app.route = Route::Folder {
            volume_id: 2,
            parent_id: None,
        };
        app.history.push((
            category_route.clone(),
            250,
            None,
            None,
            0.0,
            Vec::new(),
            String::new(),
        ));
        app.go_back(&ctx);
        assert_eq!(app.offset, 250);
        assert!(!app.category_result_refresh_pending);

        app.active_request = Some(40);
        app.apply_reply(
            crate::bridge::Reply {
                id: 40,
                purpose: Purpose::Page {
                    route: category_route.clone(),
                },
                response: json!({"ok":true,"data":{"items":[],
                    "offset":250,"limit":250,"has_more":false}}),
            },
            &ctx,
        );
        assert_eq!(app.offset, 0);
        assert!(app.page.items.is_empty());
        assert!(app.selected.is_none());
        assert!(app.restore_selection.is_none());

        // The fallback page is then loaded from the service: no stale
        // high-offset view and no permanent category refresh state.
        app.active_request = Some(41);
        app.apply_reply(
            crate::bridge::Reply {
                id: 41,
                purpose: Purpose::Page {
                    route: category_route,
                },
                response: json!({"ok":true,"data":{"items":[{
                    "id":11,"volume_id":2,"parent_id":null,
                    "name":"Кино.mkv","relative_path":"Кино.mkv",
                    "kind":"file","size":42,"modified_unix":null,
                    "description":"","tags":""}],
                    "offset":0,"limit":250,"has_more":false}}),
            },
            &ctx,
        );
        assert_eq!(app.offset, 0);
        assert_eq!(app.page.items.len(), 1);
        assert_eq!(app.page.items[0].id, 11);
        assert!(!app.category_result_refresh_pending);
    }

    #[test]
    fn malformed_page_reply_drops_old_inspector_selection_fail_closed() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        let selected = Entry {
            id: 19,
            volume_id: 2,
            parent_id: None,
            name: "Кино.mkv".into(),
            relative_path: "Кино.mkv".into(),
            kind: "file".into(),
            size: 42,
            modified_unix: None,
            description: String::new(),
            tags: String::new(),
        };
        app.selected = Some(selected.clone());
        app.restore_selection = Some(selected.clone());
        app.page.items = vec![selected];
        app.category_result_refresh_pending = true;
        app.active_request = Some(42);
        app.apply_reply(
            crate::bridge::Reply {
                id: 42,
                purpose: Purpose::Page {
                    route: app.route.clone(),
                },
                response: json!({"ok":true,"data":{
                    "items":"bad-type","offset":0,"limit":250}}),
            },
            &ctx,
        );
        assert!(app.error.is_some());
        assert!(app.selected.is_none());
        assert!(app.restore_selection.is_none());
        assert!(app.page.items.is_empty());
        assert!(!app.category_result_refresh_pending);
        assert!(!app.category_assignments_ready);
    }

    #[test]
    fn failed_membership_load_is_retryable_not_permanent_loading() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.selected = Some(Entry {
            id: 19,
            volume_id: 2,
            parent_id: None,
            name: "Кино.mkv".into(),
            relative_path: "Кино.mkv".into(),
            kind: "file".into(),
            size: 42,
            modified_unix: None,
            description: String::new(),
            tags: String::new(),
        });
        app.active_request = Some(43);
        app.apply_reply(
            crate::bridge::Reply {
                id: 43,
                purpose: Purpose::EntryCategoryIds { entry_id: 19 },
                response: json!({"ok":false,"error":{
                    "code":"not_found","message":"Entry removed"}}),
            },
            &ctx,
        );
        assert!(app.category_assignments_error);
        assert!(!app.category_assignments_ready);
        assert!(app.category_assigned.is_empty());

        app.active_request = Some(44);
        app.apply_reply(
            crate::bridge::Reply {
                id: 44,
                purpose: Purpose::EntryCategoryIds { entry_id: 19 },
                response: json!({"ok":true,"data":[7]}),
            },
            &ctx,
        );
        assert!(!app.category_assignments_error);
        assert!(app.category_assignments_ready);
        assert!(app.category_assigned.contains(&7));
    }

    #[test]
    fn category_navigation_is_all_disk_scoped_and_back_restores_query() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.query_input = "clip-001".to_owned();
        app.route = Route::Search {
            query: app.query_input.clone(),
            volume_id: Some(61),
        };
        app.selected_volume = Some(61);
        let category = Category {
            id: 3,
            parent_id: None,
            name: "Семейное Кино".into(),
        };
        app.page.offset = 250;
        app.page.has_more = true;
        app.categories.push(category.clone());
        app.categories_loaded = true;
        app.navigate_category(&category, &ctx);
        assert_eq!(app.page.offset, 0);
        assert!(!app.page.has_more);
        assert_eq!(app.query_input, "category_id:3");
        assert_eq!(app.selected_volume, None);
        assert_eq!(app.history.len(), 1);
        assert_eq!(
            app.route,
            Route::Search {
                query: "category_id:3".into(),
                volume_id: None,
            }
        );
        app.go_back(&ctx);
        assert_eq!(app.query_input, "clip-001");
        assert_eq!(app.selected_volume, Some(61));
    }

    #[test]
    fn dialogs_disable_background_controls_until_closed() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        for dialog in 0..5 {
            app.show_catalog_dialog = dialog == 0;
            app.show_export_dialog = dialog == 1;
            app.show_scan_dialog = dialog == 2;
            app.show_disk_properties = (dialog == 3).then_some(1);
            app.show_categories_dialog = dialog == 4;
            let mut output = ctx.run_pass(egui::RawInput::default(), |ctx| {
                egui::Area::new(egui::Id::unique("background-test")).show(ctx, |ui| {
                    app.protect_background(ui);
                    assert!(!ui.button("Open another dialog").enabled());
                });
            });
            output.textures_delta.clear();
        }
        app.show_catalog_dialog = false;
        app.show_export_dialog = false;
        app.show_scan_dialog = false;
        app.show_disk_properties = None;
        app.show_categories_dialog = false;
        let mut output = ctx.run_pass(egui::RawInput::default(), |ctx| {
            egui::Area::new(egui::Id::unique("background-test")).show(ctx, |ui| {
                app.protect_background(ui);
                assert!(ui.button("Open another dialog").enabled());
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn only_legacy_schema_failure_enables_shelf_fallback() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.active_request = Some(1);
        app.apply_reply(crate::bridge::Reply { id:1, purpose:Purpose::DiskPreferences, response:json!({"ok":false,"error":{"code":"invalid_disk_properties","message":"unsafe store"}}) }, &ctx);
        assert!(!app.legacy_shelf_editable);
        app.active_request = Some(2);
        app.apply_reply(
            crate::bridge::Reply {
                id: 2,
                purpose: Purpose::DiskPreferences,
                response: json!({"ok":false,"error":{"code":"schema_version","message":"legacy"}}),
            },
            &ctx,
        );
        assert!(app.legacy_shelf_editable);
    }

    #[test]
    fn preferences_reply_keeps_shelf_labels_and_collision_ids_in_sync() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        for id in [1, 2] {
            app.volumes.push(Volume {
                id,
                label: format!("Original {id}"),
                root_path: String::new(),
                entry_count: 0,
                total_bytes: 0,
            });
        }
        app.active_request = Some(7);
        let item = |id, name| json!({"volume_id":id, "display_name":name, "shelf_label":"BOX-17", "model":"", "filesystem":"", "capacity_bytes":null, "note":"", "revision":1});
        app.apply_reply(crate::bridge::Reply { id:7, purpose:Purpose::DiskPreferences, response:json!({"ok":true, "data":{"items":[item(1,"Local name"),item(2,"")],"collisions":[{"volume_ids":[1,2]}]}}) }, &ctx);
        assert!(app.preferences_ready);
        assert_eq!(app.display_name(&app.volumes[0]), "Local name");
        assert_eq!(app.shelf.label(1), "BOX-17");
        assert_eq!(app.shelf.collision_other_disks(1), vec![2]);
        app.open_disk_properties(1);
        assert_eq!(app.disk_draft.original_shelf, "BOX-17");
    }

    #[test]
    fn rejected_scan_replaces_previous_status_and_keeps_dialog_open() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("synthetic.sqlite".to_owned());
        app.scan_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("missing-synthetic-scan-root-test")
            .to_string_lossy()
            .into_owned();
        assert!(!Path::new(&app.scan_root).exists());
        app.scan_label = "Synthetic".to_owned();
        app.status = StatusText::new(
            "Предыдущая операция успешна",
            "Previous operation succeeded",
        );
        app.show_scan_dialog = true;

        app.start_scan(&ctx);

        assert_eq!(app.status.text(app.lang), "Операция отклонена");
        assert!(app.error.is_some());
        assert!(app.show_scan_dialog);
        assert!(app.active_request.is_none());
    }

    #[test]
    fn escape_cancels_only_a_registered_scan_and_respects_modals() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.scan_job_id = Some("synthetic-test-job".to_owned());
        let input = || key_input(egui::Key::Escape, egui::Modifiers::default());
        let mut output = ctx.run_pass(input(), |ctx| app.handle_shortcuts(ctx));
        output.textures_delta.clear();
        assert!(!app.scan_cancel_requested);
        app.scan_registered = true;
        app.show_export_dialog = true;
        let mut output = ctx.run_pass(input(), |ctx| app.handle_shortcuts(ctx));
        output.textures_delta.clear();
        assert!(!app.scan_cancel_requested);
        app.show_export_dialog = false;
        let mut output = ctx.run_pass(input(), |ctx| app.handle_shortcuts(ctx));
        output.textures_delta.clear();
        assert!(app.scan_cancel_requested);
        assert!(app.scan_cancel_request.is_some());
    }

    #[test]
    fn escape_cancels_active_preview_analytics_before_other_job_kinds() {
        for (index, workspace) in [WorkspaceTab::VolumeMap, WorkspaceTab::Slices]
            .into_iter()
            .enumerate()
        {
            let ctx = egui::Context::default();
            let mut app = DesktopApp::with_context(&ctx, None);
            app.workspace_tab = workspace;
            let job_id = format!("escape-preview-analytics-{}-{index}", std::process::id());
            let prepared = atlas_service::prepare_ctf_preview_analytics_job(
                &json!({"op":"ctf_preview_analytics","job_id":job_id,
                    "path":"synthetic-no-open.ctf","generation":"synthetic"})
                .to_string(),
            )
            .unwrap();
            if workspace == WorkspaceTab::VolumeMap {
                app.analytics_job_id = Some(job_id.clone());
                app.analytics_preview_job = true;
                app.slices_job_id = Some("out-of-scope-slices-job".into());
            } else {
                app.slices_job_id = Some(job_id.clone());
                app.slices_preview_job = true;
                app.analytics_job_id = Some("out-of-scope-map-job".into());
            }
            app.ctf_job_id = Some("out-of-scope-ctf-job".into());
            app.ctf_job_registered = true;
            app.scan_job_id = Some("out-of-scope-scan-job".into());
            app.scan_registered = true;

            let input = key_input(egui::Key::Escape, egui::Modifiers::default());
            let mut output = ctx.run_pass(input, |ctx| app.handle_shortcuts(ctx));
            output.textures_delta.clear();

            if workspace == WorkspaceTab::VolumeMap {
                assert!(app.analytics_job_id.is_none());
                assert!(app.analytics_cancelled);
                assert_eq!(
                    app.slices_job_id.as_deref(),
                    Some("out-of-scope-slices-job")
                );
            } else {
                assert!(app.slices_job_id.is_none());
                assert!(app.slices_cancelled);
                assert_eq!(
                    app.analytics_job_id.as_deref(),
                    Some("out-of-scope-map-job")
                );
            }
            assert_eq!(app.ctf_job_id.as_deref(), Some("out-of-scope-ctf-job"));
            assert_eq!(app.scan_job_id.as_deref(), Some("out-of-scope-scan-job"));
            assert!(app.ctf_cancel_request.is_none());
            assert!(app.scan_cancel_request.is_none());
            assert!(!app.scan_cancel_requested);
            atlas_service::cancel_ctf_preview_analytics_job(&job_id).unwrap();
            drop(prepared);
            assert_eq!(
                atlas_service::cancel_ctf_preview_analytics_job(&job_id)
                    .unwrap_err()
                    .code,
                "not_found"
            );
        }
    }

    #[test]
    fn rejected_export_replaces_previous_success_status() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("synthetic.sqlite".to_owned());
        app.export_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("Cargo.toml")
            .to_string_lossy()
            .into_owned();
        app.status = StatusText::new("Экспорт завершён", "Export completed");
        app.show_export_dialog = true;

        app.start_export(&ctx);

        assert_eq!(app.status.text(app.lang), "Операция отклонена");
        assert!(app.error.is_some());
        assert!(app.show_export_dialog);
        assert!(app.active_request.is_none());
    }

    #[test]
    fn status_retranslates_existing_page_and_export_results() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.active_request = Some(7);
        app.apply_reply(
            crate::bridge::Reply {
                id: 7,
                purpose: Purpose::Page {
                    route: app.route.clone(),
                },
                response: json!({"ok": true, "data": {"items": [{
                    "id":11,"volume_id":2,"parent_id":null,
                    "name":"Кино.mkv","relative_path":"Кино.mkv","kind":"file",
                    "size":42,"modified_unix":null,"description":"","tags":""
                }], "offset": 250, "limit": 250}}),
            },
            &ctx,
        );
        assert_eq!(app.status.text(app.lang), "Показаны строки 251–251");
        app.lang = Language::English;
        assert_eq!(app.status.text(app.lang), "Showing rows 251–251");
        app.active_request = Some(8);
        app.apply_reply(
            crate::bridge::Reply {
                id: 8,
                purpose: Purpose::Export {
                    path: "synthetic.csv".to_owned(),
                },
                response: json!({"ok": true, "data": {"rows": 42}}),
            },
            &ctx,
        );
        assert_eq!(
            app.status.text(app.lang),
            "Export complete: synthetic.csv (42 rows)"
        );
        app.lang = Language::Russian;
        assert_eq!(
            app.status.text(app.lang),
            "Экспорт завершён: synthetic.csv (42 записей)"
        );
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
        app.history.push((
            original.clone(),
            250,
            None,
            Some(60),
            0.0,
            Vec::new(),
            String::new(),
        ));
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

    #[test]
    fn containing_folder_and_back_restore_selected_search_result() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        let entry = Entry::parse(&json!({"id":42,"volume_id":7,"parent_id":13,
            "name":"film.mkv","relative_path":"Movies/film.mkv","kind":"file","size":42}))
        .unwrap();
        let original = Route::Search {
            query: "film".into(),
            volume_id: None,
        };
        app.route = original.clone();
        app.offset = 250;
        app.results_scroll = 320.0;
        app.selected = Some(entry.clone());
        app.navigate_containing_folder(&entry, &ctx);
        assert_eq!(
            app.route,
            Route::Folder {
                volume_id: 7,
                parent_id: Some(13)
            }
        );
        assert_eq!(app.selected_volume, Some(7));
        app.go_back(&ctx);
        assert_eq!(app.route, original);
        assert_eq!(app.offset, 250);
        assert_eq!(app.selected_volume, None);
        assert_eq!(app.restore_scroll, Some(320.0));
        app.active_request = Some(1);
        app.apply_reply(
            crate::bridge::Reply {
                id: 1,
                purpose: Purpose::Page { route: original },
                response: json!({"ok":true,"data":{"items":[{"id":42,"volume_id":7,"parent_id":13,
                "name":"film.mkv","relative_path":"Movies/film.mkv","kind":"file","size":42}],
                "offset":250,"limit":250,"has_more":false}}),
            },
            &ctx,
        );
        assert_eq!(app.selected, Some(entry));
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
    fn enter_opens_selected_container_but_never_a_file() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("synthetic.sqlite".into());
        selected_volume(&mut app);
        let folder = Entry {
            id: 70,
            volume_id: 60,
            parent_id: None,
            name: "Movies".into(),
            relative_path: "Movies".into(),
            kind: "directory".into(),
            size: 0,
            modified_unix: None,
            description: String::new(),
            tags: String::new(),
        };
        app.selected = Some(folder.clone());
        app.bus.set_handler(
            |_| json!({"ok":true,"data":{"items":[],"offset":0,"limit":250,"has_more":false}}),
        );

        let mut output = ctx.run_pass(key_input(egui::Key::Enter, egui::Modifiers::NONE), |ctx| {
            app.handle_shortcuts(ctx)
        });
        output.textures_delta.clear();
        assert_eq!(
            app.route,
            Route::Folder {
                volume_id: 60,
                parent_id: Some(70)
            }
        );
        assert!(app.active_request.is_some());

        app.active_request = None;
        let file = Entry {
            kind: "file".into(),
            id: 71,
            name: "movie.mkv".into(),
            relative_path: "Movies/movie.mkv".into(),
            parent_id: Some(70),
            ..folder
        };
        app.selected = Some(file);
        let mut output = ctx.run_pass(key_input(egui::Key::Enter, egui::Modifiers::NONE), |ctx| {
            app.handle_shortcuts(ctx)
        });
        output.textures_delta.clear();
        assert_eq!(
            app.route,
            Route::Folder {
                volume_id: 60,
                parent_id: Some(70)
            }
        );
        assert!(app.active_request.is_none());
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

    #[test]
    fn workspace_tabs_keep_pr58_order_and_enable_analytics_workspaces() {
        assert_eq!(
            WorkspaceTab::ORDERED.map(|tab| tab.label(Language::English)),
            ["Search", "Slices", "Volume Map", "USB Disks", "Local PC"]
        );
        assert!(WorkspaceTab::Search.available());
        assert!(WorkspaceTab::UsbDisks.available());
        assert!(WorkspaceTab::Slices.available());
        assert!(WorkspaceTab::VolumeMap.available());
        assert!(WorkspaceTab::LocalPc.available());
    }

    #[test]
    fn changing_workspace_preserves_search_route_page_selection_and_scroll() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        let entry = Entry {
            id: 41,
            volume_id: 5,
            parent_id: Some(8),
            name: "фильм.mkv".into(),
            relative_path: "Кино/фильм.mkv".into(),
            kind: "file".into(),
            size: 12,
            modified_unix: None,
            description: String::new(),
            tags: String::new(),
        };
        app.query_input = "ext:mkv AND name:фильм".into();
        app.route = Route::Folder {
            volume_id: 5,
            parent_id: Some(8),
        };
        app.selected_volume = Some(5);
        app.offset = 250;
        app.page.items = vec![entry.clone()];
        app.selected = Some(entry);
        app.results_scroll = 177.0;
        app.history.push((
            Route::Search {
                query: "ext:mkv".into(),
                volume_id: Some(5),
            },
            0,
            None,
            Some(5),
            64.0,
            Vec::new(),
            "ext:mkv".into(),
        ));

        assert!(app.switch_workspace(WorkspaceTab::UsbDisks));
        app.disks_filter = "#5".into();
        app.disks_selected_volume = Some(5);
        assert!(app.switch_workspace(WorkspaceTab::Search));

        assert_eq!(app.query_input, "ext:mkv AND name:фильм");
        assert_eq!(
            app.route,
            Route::Folder {
                volume_id: 5,
                parent_id: Some(8)
            }
        );
        assert_eq!(app.selected_volume, Some(5));
        assert_eq!(app.offset, 250);
        assert_eq!(app.page.items[0].id, 41);
        assert_eq!(app.selected.as_ref().map(|row| row.id), Some(41));
        assert_eq!(app.results_scroll, 177.0);
        assert_eq!(app.history.len(), 1);
        assert_eq!(app.disks_filter, "#5");
        assert_eq!(app.disks_selected_volume, Some(5));
        assert_eq!(app.restore_scroll, Some(177.0));
        assert!(app.switch_workspace(WorkspaceTab::Slices));
        assert_eq!(app.workspace_tab, WorkspaceTab::Slices);
        app.show_export_dialog = true;
        assert!(!app.switch_workspace(WorkspaceTab::UsbDisks));
        assert_eq!(app.workspace_tab, WorkspaceTab::Slices);
    }

    #[test]
    fn page_reply_while_disks_workspace_is_active_updates_search_session_only() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.active_request = Some(12);
        app.workspace_tab = WorkspaceTab::UsbDisks;
        app.disks_filter = "#5".into();
        app.disks_selected_volume = Some(5);
        let route = Route::Search {
            query: "ext:mkv".into(),
            volume_id: Some(5),
        };
        app.apply_reply(
            crate::bridge::Reply {
                id: 12,
                purpose: Purpose::Page {
                    route: route.clone(),
                },
                response: json!({"ok":true,"data":{"items":[{
                    "id":41,"volume_id":5,"parent_id":null,"name":"фильм.mkv",
                    "relative_path":"Кино/фильм.mkv","kind":"file","size":12
                }],"offset":0,"limit":250,"has_more":false}}),
            },
            &ctx,
        );
        assert_eq!(app.workspace_tab, WorkspaceTab::UsbDisks);
        assert_eq!(app.route, route);
        assert_eq!(app.page.items[0].id, 41);
        assert_eq!(app.disks_filter, "#5");
        assert_eq!(app.disks_selected_volume, Some(5));
        assert_eq!(app.active_request, None);

        app.active_request = Some(14);
        app.page.items.clear();
        app.apply_reply(
            crate::bridge::Reply {
                id: 13,
                purpose: Purpose::Page {
                    route: Route::Search { query: "stale".into(), volume_id: None },
                },
                response: json!({"ok":true,"data":{"items":[],"offset":0,"limit":250,"has_more":false}}),
            },
            &ctx,
        );
        assert_eq!(app.active_request, Some(14));
        assert!(app.page.items.is_empty());
        assert_eq!(app.route, route);
    }

    #[test]
    fn disk_workspace_action_browses_root_without_empty_global_search() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.db = Some("synthetic.sqlite".into());
        app.volumes.push(Volume {
            id: 5,
            label: "Disk Five".into(),
            root_path: String::new(),
            entry_count: 1,
            total_bytes: 42,
        });
        app.workspace_tab = WorkspaceTab::UsbDisks;
        app.disks_filter = "#5".into();
        app.disks_selected_volume = Some(5);
        app.route = Route::Search {
            query: "ext:mkv AND name:фильм".into(),
            volume_id: None,
        };
        app.query_input = "ext:mkv AND name:фильм".into();
        app.results_scroll = 120.0;
        app.history.push((
            app.route.clone(),
            250,
            None,
            Some(5),
            120.0,
            Vec::new(),
            String::new(),
        ));
        let (sent, received) = std::sync::mpsc::channel();
        app.bus.set_handler(move |request| {
            sent.send(request.clone()).unwrap();
            json!({"ok":true,"data":{"items":[],"offset":0,"limit":250,"has_more":false}})
        });

        app.jump_to_search_on_disk(5, &ctx);
        let request = received
            .recv_timeout(Duration::from_secs(2))
            .expect("search request was not sent");
        assert_eq!(app.workspace_tab, WorkspaceTab::Search);
        assert!(app.query_input.is_empty());
        assert_eq!(app.selected_volume, Some(5));
        assert_eq!(request["op"], "children");
        assert_eq!(request["volume_id"], 5);
        assert!(request["parent_id"].is_null());
        assert_eq!(app.disks_filter, "#5");
        assert_eq!(app.disks_selected_volume, Some(5));
        assert_eq!(app.history.len(), 2);
        let previous = app.history.last().unwrap();
        assert_eq!(
            previous.0,
            Route::Search {
                query: "ext:mkv AND name:фильм".into(),
                volume_id: None
            }
        );
        assert_eq!(previous.3, None);
        assert_eq!(previous.4, 120.0);
        assert_eq!(app.results_scroll, 0.0);
        assert_eq!(app.restore_scroll, Some(0.0));
    }

    #[test]
    fn clear_catalog_resets_disk_workspace_state() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        app.workspace_tab = WorkspaceTab::UsbDisks;
        app.disks_filter = "#5".into();
        app.disks_selected_volume = Some(5);
        app.disks_scroll = 900.0;
        app.clear_catalog();
        assert_eq!(app.workspace_tab, WorkspaceTab::Search);
        assert!(app.disks_filter.is_empty());
        assert_eq!(app.disks_selected_volume, None);
        assert_eq!(app.disks_scroll, 0.0);
    }

    #[test]
    fn properties_shortcut_uses_only_active_workspace_selection() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::with_context(&ctx, None);
        selected_volume(&mut app);
        app.selected_volume = Some(60);
        app.disks_selected_volume = None;
        app.workspace_tab = WorkspaceTab::UsbDisks;
        let mut output = ctx.run_pass(
            key_input(egui::Key::F2, egui::Modifiers::default()),
            |ctx| app.handle_shortcuts(ctx),
        );
        output.textures_delta.clear();
        assert_eq!(app.show_disk_properties, None);

        app.disks_selected_volume = Some(60);
        app.workspace_tab = WorkspaceTab::Slices;
        let mut output = ctx.run_pass(
            key_input(egui::Key::F2, egui::Modifiers::default()),
            |ctx| app.handle_shortcuts(ctx),
        );
        output.textures_delta.clear();
        assert_eq!(app.show_disk_properties, None);

        app.workspace_tab = WorkspaceTab::UsbDisks;
        let mut output = ctx.run_pass(
            key_input(egui::Key::F2, egui::Modifiers::default()),
            |ctx| app.handle_shortcuts(ctx),
        );
        output.textures_delta.clear();
        assert_eq!(app.show_disk_properties, Some(60));
        assert_eq!(app.selected_volume, Some(60));
    }
}
#[cfg(test)]
#[derive(Default)]
struct MemoryStorage(std::collections::HashMap<String, String>);
#[cfg(test)]
impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }
    fn flush(&mut self) {}
}

#[test]
fn appearance_round_trip_saves_only_language_and_theme() {
    let ctx = egui::Context::default();
    let mut app = DesktopApp::with_context(&ctx, None);
    app.lang = Language::English;
    app.dark = false;
    app.query_input = "private search".into();
    app.path_input = "private path".into();
    let mut storage = MemoryStorage::default();
    eframe::App::save(&mut app, &mut storage);
    assert_eq!(storage.0.len(), 2);
    assert!(!eframe::App::persist_egui_memory(&app));
    let mut restarted = DesktopApp::with_context(&ctx, None);
    restarted.restore_appearance(Some(&storage));
    assert!(matches!(restarted.lang, Language::English));
    assert!(!restarted.dark);
    assert!(restarted.query_input.is_empty());
    assert!(restarted.path_input.is_empty());
    eframe::Storage::set_string(&mut storage, LANGUAGE_SETTING, "unknown".into());
    eframe::Storage::set_string(&mut storage, THEME_SETTING, "unknown".into());
    restarted.restore_appearance(Some(&storage));
    assert!(matches!(restarted.lang, Language::Russian));
    assert!(restarted.dark);
}

#[path = "catalog_overview.rs"]
mod catalog_overview;

#[cfg(test)]
#[path = "catalog_double_click_tests.rs"]
mod catalog_double_click_tests;
