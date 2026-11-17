//! PR58-first native workbench. Every user-visible catalog fact comes from atlas-service;
//! no fictitious USB mount badges, byte-duplicate claims or on-disk delete operations.
use crate::bridge::{CommandBus, ExportKind, Purpose, Route};
use crate::model::{bytes_compact, parse_volumes, Entry, Page, Volume};
use eframe::egui::{self, Color32, RichText};
use egui_extras::{Column, TableBuilder};
use serde_json::{json, Value};
use std::path::Path;

const PAGE_LIMIT: u64 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    Russian,
    English,
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
    export_path: String,
    export_kind: ExportKind,
    show_disk_properties: Option<i64>,
    status: String,
    error: Option<String>,
    lang: Language,
    dark: bool,
    auto_open: bool,
}

impl DesktopApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup_catalog: Option<String>) -> Self {
        cc.egui_ctx.all_styles_mut(|style| {
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
            export_path: String::new(),
            export_kind: ExportKind::Csv,
            show_disk_properties: None,
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

    fn load_volumes(&mut self, ctx: &egui::Context) {
        if let Some(db) = self.db.clone() {
            self.status = self
                .t("Загрузка реестра дисков…", "Loading disk register…")
                .to_owned();
            self.queue(Purpose::Volumes, json!({"op":"volumes","db":db}), ctx);
        }
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
        // Monotonic request ids reject stale worker replies when a later
        // search, navigation or source change superseded the operation.
        if self.active_request != Some(reply.id) {
            return;
        }
        self.active_request = None;
        if reply.response.get("ok").and_then(Value::as_bool) != Some(true) {
            let message = reply
                .response
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Unknown service error");
            self.error = Some(message.to_owned());
            self.status = self.t("Операция отклонена", "Operation refused").to_owned();
            return;
        }
        let data = &reply.response["data"];
        match reply.purpose {
            Purpose::Open { path } | Purpose::Create { path } => {
                self.db = Some(path);
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
                    self.load_page(ctx);
                }
                Err(e) => self.error = Some(e),
            },
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
                    .map(|v| format!("#{} · {}", v.id, v.label))
                    .unwrap_or_else(|| self.t("Все диски", "All disks").to_owned());
                ui.label(RichText::new(scope).strong());
                ui.separator();
                let query_hint = self.t(
                    "Поиск: имя, размер, ext, AND/OR/NOT…",
                    "Search: name, size, ext, AND/OR/NOT…",
                );
                let response = ui.add_sized(
                    [ui.available_width().max(220.0) - 185.0, 25.0],
                    egui::TextEdit::singleline(&mut self.query_input).hint_text(query_hint),
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
                            || v.label.to_lowercase().contains(&filter)
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
                                let title = format!("#{} · {}", volume.id, volume.label);
                                let label = ui.selectable_label(selected, title);
                                if label.clicked() {
                                    self.navigate_disk(Some(volume.id), ctx);
                                }
                                if label.double_clicked() {
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
                                    self.show_disk_properties = Some(volume.id);
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
                egui::Window::new(self.t("Свойства диска", "Disk properties"))
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
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
                        ui.label(
                            RichText::new(self.t(
                                "Это НЕ подтверждённый текущий путь или подключённый USB.",
                                "This is NOT a verified current mount or physical USB connection.",
                            ))
                            .small()
                            .weak(),
                        );
                        ui.add_space(3.0);
                        if ui.button(self.t("Закрыть", "Close")).clicked() {
                            self.show_disk_properties = None;
                        }
                    });
                if !open {
                    self.show_disk_properties = None;
                }
            } else {
                self.show_disk_properties = None;
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let search = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F));
        let all = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::L));
        let back = ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft));
        if all {
            self.navigate_disk(None, ctx);
        }
        if search {
            self.navigate_search(ctx);
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
        self.handle_shortcuts(&ctx);
        self.menu(root, &ctx);
        self.search_toolbar(root, &ctx);
        self.disk_sidebar(root, &ctx);
        self.inspector(root, &ctx);
        self.results_table(root, &ctx);
        self.dialogs(&ctx);
    }
}
