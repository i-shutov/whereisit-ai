//! PR58 local analyzer workspace. Local filesystem snapshots are explicit, private,
//! metadata-only jobs; this view never reads file contents or claims backup equality.
#[path = "local_analyzer_chart.rs"]
mod chart;
use crate::bridge::{new_scan_job_id, CommandBus, Purpose};
use eframe::egui::{self, Color32, RichText};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const MODES: [(&str, &str); 6] = [
    ("tree_map", "Дерево + Карта / Tree + Map"),
    ("bloat", "Кэши и сборки / Caches and builds"),
    ("largest", "Крупные и старые / Large and old"),
    (
        "metadata_candidates",
        "Кандидаты по метаданным / Metadata candidates",
    ),
    ("catalog_check", "Кандидаты в каталоге / Catalog candidates"),
    ("growth_diff", "Рост папок / Folder growth"),
];

// Keep room for the page status and navigation controls below result content.
const PAGE_FOOTER_RESERVE: f32 = 88.0;

fn scroll_content_height(ui: &egui::Ui) -> f32 {
    (ui.available_height() - PAGE_FOOTER_RESERVE).max(96.0)
}

fn draw_bloat_cards(
    ui: &mut egui::Ui,
    rows: &[Value],
    groups: &[Value],
    staged: &mut HashSet<String>,
    lang_ru: bool,
) -> (Vec<egui::Rect>, Vec<egui::Rect>) {
    let classes = ["cache", "dev", "installers", "archives"];
    let card_width = ((ui.available_width() - 8.0) / 2.0).max(180.0);
    let mut card_rects = Vec::with_capacity(4);
    let mut action_rects = Vec::new();
    egui::Grid::new("bloat_category_cards")
        .num_columns(2)
        .spacing([8.0, 8.0])
        .show(ui, |ui| {
            for (index, class) in classes.iter().enumerate() {
                ui.vertical(|ui| {
                    let card = egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(card_width - 20.0);
                        ui.vertical(|ui| {
                            ui.strong(match (*class, lang_ru) {
                                ("cache", true) => "Кэши",
                                ("dev", true) => "Сборки и зависимости",
                                ("installers", true) => "Установщики",
                                ("archives", true) => "Архивы",
                                ("cache", false) => "Caches",
                                ("dev", false) => "Builds and dependencies",
                                ("installers", false) => "Installers",
                                (_, false) => "Archives",
                                (_, true) => "Другие файлы",
                            });
                            let summary = groups.iter().find(|row| row["classification"] == *class);
                            let files = summary.and_then(|r| r["files"].as_u64()).unwrap_or(0);
                            let bytes = summary.and_then(|r| r["bytes"].as_u64()).unwrap_or(0);
                            ui.label(format!("{files} · {}", format_bytes(bytes)));
                            let previews: Vec<_> = rows
                                .iter()
                                .filter(|row| row["classification"] == *class)
                                .take(3)
                                .collect();
                            if previews.is_empty() {
                                ui.label(if lang_ru {
                                    "Нет элементов в показанной странице."
                                } else {
                                    "No rows in the current page."
                                });
                            }
                            for item in previews {
                                let id = item["id"].as_str().unwrap_or("");
                                let path = item["relative_path"].as_str().unwrap_or("?");
                                ui.horizontal(|ui| {
                                    let path_width =
                                        (ui.available_width() - 104.0 - 76.0 - 16.0).max(40.0);
                                    ui.add_sized(
                                        [path_width, 20.0],
                                        egui::Label::new(path).truncate(),
                                    )
                                    .on_hover_text(path);
                                    ui.add_sized(
                                        [76.0, 20.0],
                                        egui::Label::new(format_bytes(
                                            item["bytes"].as_u64().unwrap_or(0),
                                        )),
                                    );
                                    let selected = staged.contains(id);
                                    let label = if selected {
                                        if lang_ru {
                                            "В очереди"
                                        } else {
                                            "In CSV queue"
                                        }
                                    } else if lang_ru {
                                        "Добавить"
                                    } else {
                                        "Add to CSV"
                                    };
                                    let enabled = selected || staged.len() < 10_000;
                                    let response = ui.add_enabled(
                                        enabled,
                                        egui::Button::new(label).min_size(egui::vec2(104.0, 22.0)),
                                    );
                                    action_rects.push(response.rect);
                                    if response.clicked() && !staged.insert(id.to_owned()) {
                                        staged.remove(id);
                                    }
                                });
                            }
                            ui.small(if lang_ru {
                                "Предпросмотр первых строк; карточка не выбирает всю группу."
                            } else {
                                "Preview rows only; this card does not select the whole group."
                            });
                        });
                    });
                    card_rects.push(card.response.rect);
                });
                if index % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    (card_rects, action_rects)
}

fn draw_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    r_outer: f32,
    r_inner: f32,
    start: f64,
    end: f64,
    color: Color32,
) {
    let sweep = (end - start).abs();
    if sweep < 0.0005 {
        return;
    }
    let steps = ((sweep * 26.0).ceil() as usize).max(1);
    let mut mesh = egui::epaint::Mesh::default();
    for i in 0..=steps {
        let a = start + (end - start) * (i as f64 / steps as f64);
        let (s, c) = a.sin_cos();
        mesh.colored_vertex(
            center + egui::vec2(c as f32 * r_outer, s as f32 * r_outer),
            color,
        );
        mesh.colored_vertex(
            center + egui::vec2(c as f32 * r_inner, s as f32 * r_inner),
            color,
        );
    }
    for i in 0..steps {
        let b = (i * 2) as u32;
        mesh.add_triangle(b, b + 1, b + 3);
        mesh.add_triangle(b, b + 3, b + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn format_bytes(bytes: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut n = bytes as f64;
    let mut unit = 0;
    while n >= 1024.0 && unit + 1 < units.len() {
        n /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{n:.2} {}", units[unit])
    }
}
fn parse_size_mb(input: &str) -> Option<u64> {
    if input.trim().is_empty() {
        return Some(0);
    }
    let mb = input.trim().parse::<f64>().ok()?;
    if !mb.is_finite() || mb < 0.0 || mb > (i64::MAX as f64) / 1_000_000.0 {
        return None;
    }
    Some((mb * 1_048_576.0).round() as u64)
}
fn parse_age_days(input: &str) -> Option<u64> {
    if input.trim().is_empty() {
        Some(0)
    } else {
        input.trim().parse::<u64>().ok().filter(|x| *x <= 365_000)
    }
}
fn phase_label(phase: &str, ru: bool) -> &'static str {
    match (phase, ru) {
        ("preparing", true) => "Подготовка",
        ("starting", true) => "Запуск",
        ("scanning", true) => "Сканирование",
        ("checking_catalog", true) => "Сверка с каталогом",
        ("cancelling", true) | ("cancel_requested", true) => "Остановка",
        ("completed", true) => "Готово",
        ("cancelled", true) => "Отменено",
        ("failed", true) => "Ошибка",
        ("preparing", false) => "Preparing",
        ("starting", false) => "Starting",
        ("scanning", false) => "Scanning",
        ("checking_catalog", false) => "Checking catalog",
        ("cancelling", false) | ("cancel_requested", false) => "Cancelling",
        ("completed", false) => "Complete",
        ("cancelled", false) => "Cancelled",
        ("failed", false) => "Failed",
        (_, true) => "Не запущено",
        (_, false) => "Not started",
    }
}

fn same_root(a: &str, b: &str) -> bool {
    #[cfg(windows)]
    {
        a.trim_end_matches(['\\', '/'])
            .eq_ignore_ascii_case(b.trim_end_matches(['\\', '/']))
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

fn result_rows(
    ui: &mut egui::Ui,
    rows: &[Value],
    mode: &str,
    staged: &mut HashSet<String>,
    lang_ru: bool,
) -> Option<String> {
    let mut open_folder = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(scroll_content_height(ui))
        .show(ui, |ui| {
            egui::Grid::new(("local_rows", mode))
                .striped(true)
                .show(ui, |ui| {
                    ui.strong(if lang_ru {
                        "Путь / элемент"
                    } else {
                        "Path / item"
                    });
                    ui.strong(if lang_ru { "Размер" } else { "Size" });
                    ui.strong(if mode == "growth_diff" {
                        if lang_ru {
                            "Было / стало / разница"
                        } else {
                            "Previous / current / change"
                        }
                    } else {
                        if lang_ru {
                            "Сведения"
                        } else {
                            "Details"
                        }
                    });
                    ui.strong(if lang_ru {
                        "Очередь CSV"
                    } else {
                        "CSV review"
                    });
                    ui.end_row();
                    for row in rows {
                        let id = row["id"].as_str().unwrap_or("");
                        let kind = row["kind"].as_str().unwrap_or("file");
                        let path = row["relative_path"]
                            .as_str()
                            .or_else(|| row["name"].as_str())
                            .unwrap_or("(unknown path)");
                        let path_response = if mode == "tree_map" && kind == "folder" {
                            ui.link(format!("{path}/"))
                        } else {
                            ui.label(format!(
                                "{}{path}",
                                if kind == "folder" { "Folder: " } else { "" }
                            ))
                        };
                        if path_response.clicked() && kind == "folder" {
                            open_folder = Some(id.to_owned());
                        }
                        ui.label(format_bytes(row["bytes"].as_u64().unwrap_or(0)));
                        let detail = match mode {
                            "growth_diff" => format!(
                                "{} → {}  ({:+})",
                                format_bytes(row["previous_bytes"].as_u64().unwrap_or(0)),
                                format_bytes(row["bytes"].as_u64().unwrap_or(0)),
                                row["delta_bytes"].as_i64().unwrap_or(0)
                            ),
                            "metadata_candidates" => format!(
                                "{} files in metadata group",
                                row["metadata_group_count"].as_u64().unwrap_or(1)
                            ),
                            "catalog_check" => match row["candidate_volumes"].as_u64() {
                                None => if lang_ru {
                                    "Не проверено / устарело"
                                } else {
                                    "Not checked / stale"
                                }
                                .to_owned(),
                                Some(0) => if lang_ru {
                                    "Не найдено"
                                } else {
                                    "No catalog match"
                                }
                                .to_owned(),
                                Some(n) => format!(
                                    "{}: {n}",
                                    if lang_ru {
                                        "Кандидаты в дисках"
                                    } else {
                                        "Candidate volumes"
                                    }
                                ),
                            },
                            "bloat" => format!(
                                "{} · {} files",
                                row["classification"].as_str().unwrap_or("other"),
                                row["file_count"].as_u64().unwrap_or(1)
                            ),
                            _ => format!(
                                "{} · {} files",
                                row["classification"].as_str().unwrap_or("file"),
                                row["file_count"].as_u64().unwrap_or(1)
                            ),
                        };
                        ui.label(detail);
                        if mode != "growth_diff" && !kind.is_empty() {
                            let selected = staged.contains(id);
                            let label = if selected {
                                if lang_ru {
                                    "В очереди"
                                } else {
                                    "In CSV queue"
                                }
                            } else if lang_ru {
                                "Добавить"
                            } else {
                                "Add to CSV"
                            };
                            if ui
                                .add_enabled(
                                    selected || staged.len() < 10_000,
                                    egui::Button::new(label),
                                )
                                .clicked()
                                && !staged.insert(id.to_owned())
                            {
                                staged.remove(id);
                            }
                        } else {
                            ui.label("—");
                        }
                        ui.end_row();
                    }
                });
        });
    open_folder
}

#[derive(Default)]
pub(crate) struct LocalAnalyzer {
    root_input: String,
    requested_root: String,
    job_id: Option<String>,
    generation: u64,
    phase: String,
    progress: Value,
    error: Option<String>,
    mode: String,
    folder_id: String,
    page: Option<Value>,
    page_offset: u64,
    scope_generation: u64,
    page_request_id: Option<u64>,
    progress_request_id: Option<u64>,
    page_pending: bool,
    page_requested_scope: Option<u64>,
    progress_pending: bool,
    export_pending: bool,
    operation_cancel_requested: bool,
    active_operation_id: Option<String>,
    baseline_pending: bool,
    last_poll: Option<Instant>,
    filter_path: String,
    min_size: String,
    min_age: String,
    criterion: String,
    match_filter: String,
    catalog_generation: Option<String>,
    catalog_ready: bool,
    catalog_check_generation: u64,
    baseline_root: Option<String>,
    checked_db: Option<String>,
    baseline_id: Option<String>,
    staged: HashSet<String>,
    csv_path: String,
    message: Option<String>,
    category_volume: Option<i64>,
    folder_picker_rx: Option<Receiver<Option<Result<String, String>>>>,
}

impl Drop for LocalAnalyzer {
    fn drop(&mut self) {
        if let Some(job_id) = self.job_id.as_deref() {
            let mut cancel = json!({"op":"local_analyzer_cancel","job_id":job_id});
            if let Some(operation_id) = &self.active_operation_id {
                cancel["operation_id"] = json!(operation_id);
            }
            let _ = atlas_service::dispatch_json(&cancel.to_string());
            let _ = atlas_service::dispatch_json(
                &json!({"op":"local_analyzer_close","job_id":job_id}).to_string(),
            );
        }
        if let Some(baseline_id) = self.baseline_id.as_deref() {
            let _ = atlas_service::dispatch_json(
                &json!({"op":"local_analyzer_clear_baseline","baseline_id":baseline_id})
                    .to_string(),
            );
        }
    }
}

impl LocalAnalyzer {
    fn open_folder_picker(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        {
            if self.folder_picker_rx.is_some() {
                return;
            }
            let (tx, rx) = mpsc::channel();
            self.folder_picker_rx = Some(rx);
            let repaint = (*ctx).clone();
            std::thread::spawn(move || {
                let result = windows_folder_dialog();
                let _ = tx.send(result);
                repaint.request_repaint();
            });
        }
        #[cfg(not(windows))]
        {
            let _ = ctx;
            self.error = Some("Use the absolute path field on this platform.".into());
        }
    }

    pub(crate) fn apply_reply(
        &mut self,
        reply: &mut crate::bridge::Reply,
        bus: &mut CommandBus,
        ctx: &egui::Context,
        db: Option<&str>,
    ) -> bool {
        let Purpose::LocalAnalyzer {
            job_id,
            generation,
            scope_generation,
            op,
        } = &reply.purpose
        else {
            return false;
        };
        if self.generation != *generation || self.job_id.as_deref() != Some(job_id.as_str()) {
            if op == "cancel" && reply.response["ok"] == true {
                let _ = bus.send(
                    Purpose::LocalAnalyzer {
                        job_id: job_id.clone(),
                        generation: *generation,
                        scope_generation: *scope_generation,
                        op: "close".into(),
                    },
                    json!({"op":"local_analyzer_close","job_id":job_id}),
                    ctx,
                );
            }
            return true;
        }
        if op == "progress"
            && self.phase == "checking_catalog"
            && *scope_generation != self.scope_generation
        {
            if self.progress_request_id == Some(reply.id) {
                self.progress_pending = false;
                self.progress_request_id = None;
            }
            return true;
        }
        if op == "page"
            && (*scope_generation != self.scope_generation
                || self.page_request_id != Some(reply.id))
        {
            if self.page_request_id == Some(reply.id) {
                self.page_pending = false;
                self.page_request_id = None;
                self.active_operation_id = None;
            }
            return true;
        }
        if op == "progress" && self.progress_request_id != Some(reply.id) {
            return true;
        }
        if op == "prepare" {
            if reply.response["ok"] == true {
                self.phase = "starting".into();
                self.send(bus, ctx, "start", json!({"root":self.requested_root}));
            } else {
                self.fail(&reply.response);
            }
            return true;
        }
        match op.as_str() {
            "start" => {
                if reply.response["ok"] == true {
                    self.phase = "scanning".into();
                    self.last_poll = None;
                    self.operation_cancel_requested = false;
                } else {
                    self.fail(&reply.response);
                }
            }
            "progress" => {
                self.progress_pending = false;
                self.progress_request_id = None;
                if reply.response["ok"] == true {
                    let data = reply.response["data"].clone();
                    self.phase = data["phase"].as_str().unwrap_or("failed").to_owned();
                    if data["operation_active"] != true && data["operation_reserved"] != true {
                        self.active_operation_id = None;
                    }
                    if let Some(error) = data["error_code"].as_str() {
                        if error == "cancelled" {
                            self.error=Some("The comparison was cancelled. The local snapshot is still available.".into());
                        } else {
                            self.error = Some(error.to_owned());
                        }
                    }
                    self.progress = data;
                    if self.phase == "completed" {
                        if self.operation_cancel_requested {
                            self.catalog_ready = false;
                            if self.progress["operation_active"] != true
                                && self.progress["operation_reserved"] != true
                            {
                                self.operation_cancel_requested = false;
                                self.active_operation_id = None;
                                self.page_requested_scope = Some(self.scope_generation);
                            }
                        } else if self.mode == "catalog_check" {
                            if self.catalog_check_generation == *scope_generation
                                && self.checked_db.as_deref() == db
                            {
                                self.catalog_ready = true;
                                self.request_page(bus, ctx, db);
                            }
                        } else if self.mode != "growth_diff" || self.baseline_id.is_some() {
                            self.request_page(bus, ctx, db);
                        }
                    }
                } else {
                    self.fail(&reply.response);
                }
            }
            "page" => {
                self.page_pending = false;
                self.page_request_id = None;
                if reply.response["ok"] == true {
                    let data = reply.response["data"].clone();
                    self.catalog_generation =
                        data["catalog_generation"].as_str().map(str::to_owned);
                    self.catalog_ready = self.catalog_generation.is_some();
                    self.page = Some(data);
                    self.error = None;
                    self.active_operation_id = None;
                } else if reply
                    .response
                    .pointer("/error/code")
                    .and_then(Value::as_str)
                    == Some("cancelled")
                {
                    self.active_operation_id = None;
                    self.phase = "completed".into();
                    self.error = Some(
                        "The page query was cancelled. The local snapshot remains available."
                            .into(),
                    );
                } else {
                    self.fail(&reply.response);
                }
            }
            "cancel" => {
                if reply.response["ok"] != true {
                    self.error = Some(
                        reply
                            .response
                            .pointer("/error/message")
                            .and_then(Value::as_str)
                            .unwrap_or("Cancellation was not accepted")
                            .to_owned(),
                    );
                    if self.phase == "cancel_requested" {
                        self.phase = if self.progress["results_ready"] == true {
                            "completed"
                        } else {
                            "scanning"
                        }
                        .into();
                    }
                    self.page_requested_scope = Some(self.scope_generation);
                    return true;
                }
                let scope = reply
                    .response
                    .pointer("/data/scope")
                    .and_then(Value::as_str)
                    .unwrap_or("scan");
                self.phase = if scope == "operation" {
                    "completed"
                } else {
                    "cancelled"
                }
                .into();
                self.operation_cancel_requested = scope == "operation";
                self.page_pending = false;
                self.page_request_id = None;
                self.page_requested_scope = None;
                if scope == "operation" {
                    self.page_requested_scope = Some(self.scope_generation);
                }
                if scope != "operation" {
                    self.active_operation_id = None;
                    let _ = bus.send(
                        Purpose::LocalAnalyzer {
                            job_id: job_id.clone(),
                            generation: *generation,
                            scope_generation: *scope_generation,
                            op: "close".into(),
                        },
                        json!({"op":"local_analyzer_close","job_id":job_id}),
                        ctx,
                    );
                }
            }
            "catalog" => {
                if reply.response["ok"] == true {
                    self.page_requested_scope = None;
                    self.error = None;
                    self.phase = reply.response["data"]["phase"]
                        .as_str()
                        .unwrap_or("checking_catalog")
                        .to_owned();
                    self.catalog_generation = None;
                    self.catalog_ready = false;
                    self.catalog_check_generation = *scope_generation;
                    self.last_poll = None;
                } else {
                    self.fail(&reply.response);
                }
            }
            "baseline" => {
                self.baseline_pending = false;
                if reply.response["ok"] == true {
                    let old = self.baseline_id.take();
                    self.baseline_id = reply.response["data"]["baseline_id"]
                        .as_str()
                        .map(str::to_owned);
                    self.baseline_root = Some(self.requested_root.clone());
                    if let Some(old) = old.filter(|old| Some(old) != self.baseline_id.as_ref()) {
                        let _ = bus.send(
                            Purpose::LocalAnalyzer {
                                job_id: job_id.clone(),
                                generation: self.generation,
                                scope_generation: self.scope_generation,
                                op: "clear_baseline".into(),
                            },
                            json!({"op":"local_analyzer_clear_baseline","baseline_id":old}),
                            ctx,
                        );
                    }
                    self.active_operation_id = None;
                    self.message = Some(
                        "Baseline saved for this exact folder. No file contents were read.".into(),
                    );
                } else {
                    self.fail(&reply.response);
                }
            }
            "export_review_csv" => {
                self.export_pending = false;
                self.active_operation_id = None;
                if reply.response["ok"] == true {
                    self.message = Some(format!(
                        "Review CSV created ({} rows).",
                        reply.response["data"]["rows"].as_u64().unwrap_or(0)
                    ));
                } else {
                    self.fail(&reply.response);
                }
            }
            "close" | "clear_baseline" => {}
            _ => {}
        }
        true
    }

    fn fail(&mut self, response: &Value) {
        self.error = Some(
            response
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Local analysis request failed")
                .to_owned(),
        );
        if response.pointer("/error/code").and_then(Value::as_str) == Some("cancelled") {
            self.phase = if self.progress["results_ready"] == true {
                "completed"
            } else {
                "cancelled"
            }
            .into();
        } else if self.phase != "completed" {
            self.phase = "failed".into();
        }
        self.page_pending = false;
        self.progress_pending = false;
        self.export_pending = false;
        self.page_request_id = None;
        self.progress_request_id = None;
        self.active_operation_id = None;
    }

    fn invalidate_scope(&mut self) {
        self.scope_generation = self.scope_generation.wrapping_add(1);
        self.page_requested_scope = None;
        self.page = None;
    }

    fn invalidate_cancel_scope(&mut self) {
        // Canceling a save/export does not invalidate the completed local view.
        // Keep it regardless of whether its reply or the cancel acknowledgment wins.
        let keep_page = self.export_pending || self.baseline_pending;
        let page = if keep_page { self.page.take() } else { None };
        self.invalidate_scope();
        self.page = page;
    }

    fn sync_catalog_db(&mut self, db: Option<&str>) {
        if self.checked_db.as_deref() != db {
            self.checked_db = db.map(str::to_owned);
            self.catalog_generation = None;
            self.catalog_ready = false;
            if self.mode == "catalog_check" {
                self.invalidate_scope();
            }
        }
    }

    fn baseline_matches_root(&self, root: &str) -> bool {
        self.baseline_id.is_some()
            && self
                .baseline_root
                .as_deref()
                .is_some_and(|old| same_root(old, root))
    }

    fn send(
        &mut self,
        bus: &mut CommandBus,
        ctx: &egui::Context,
        op: &str,
        fields: Value,
    ) -> Option<u64> {
        let Some(job_id) = &self.job_id else {
            return None;
        };
        let service_op = match op {
            "catalog" => "local_analyzer_check_catalog",
            "baseline" => "local_analyzer_save_baseline",
            "export_review_csv" => "local_analyzer_export_review_csv",
            "prepare" => "local_analyzer_prepare",
            "start" => "local_analyzer_start",
            "progress" => "local_analyzer_progress",
            "cancel" => "local_analyzer_cancel",
            "page" => "local_analyzer_page",
            _ => return None,
        };
        let mut request = json!({"op":service_op,"job_id":job_id});
        if let (Some(dst), Some(src)) = (request.as_object_mut(), fields.as_object()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
        if matches!(op, "page" | "catalog" | "baseline" | "export_review_csv") {
            if self.active_operation_id.is_some() {
                self.error = Some(
                    "Другая операция ещё выполняется. / Another operation is still running.".into(),
                );
                return None;
            }
            let operation_id = new_scan_job_id();
            if let Err(error) =
                atlas_service::prepare_local_analyzer_operation(job_id, &operation_id)
            {
                self.error = Some(error.to_string());
                return None;
            }
            request["operation_id"] = json!(operation_id);
            self.active_operation_id = Some(operation_id);
        } else if op == "cancel" {
            if let Some(operation_id) = &self.active_operation_id {
                request["operation_id"] = json!(operation_id);
            }
        }
        Some(bus.send(
            Purpose::LocalAnalyzer {
                job_id: job_id.clone(),
                generation: self.generation,
                scope_generation: self.scope_generation,
                op: op.to_owned(),
            },
            request,
            ctx,
        ))
    }

    fn begin(&mut self, bus: &mut CommandBus, ctx: &egui::Context) {
        let root = self.root_input.trim();
        if root.is_empty() {
            self.error = Some("Choose a local folder by entering its absolute path.".into());
            return;
        }
        if !std::path::Path::new(root).is_absolute() {
            self.error = Some("The folder path must be absolute.".into());
            return;
        }
        self.requested_root = root.to_owned();
        if let Some(old) = self.job_id.take() {
            if self
                .baseline_root
                .as_deref()
                .is_some_and(|old_root| !same_root(old_root, root))
            {
                if let Some(baseline_id) = self.baseline_id.take() {
                    let _ = bus.send(
                        Purpose::LocalAnalyzer {
                            job_id: old.clone(),
                            generation: self.generation,
                            scope_generation: self.scope_generation,
                            op: "clear_baseline".into(),
                        },
                        json!({"op":"local_analyzer_clear_baseline","baseline_id":baseline_id}),
                        ctx,
                    );
                }
                self.baseline_root = None;
            }
            let mut cancel = json!({"op":"local_analyzer_cancel","job_id":old});
            if let Some(operation_id) = &self.active_operation_id {
                cancel["operation_id"] = json!(operation_id);
            }
            let _ = atlas_service::dispatch_json(&cancel.to_string());
            let _ = atlas_service::dispatch_json(
                &json!({"op":"local_analyzer_close","job_id":old}).to_string(),
            );
        }
        self.generation = self.generation.wrapping_add(1);
        self.scope_generation = self.scope_generation.wrapping_add(1);
        let id = new_scan_job_id();
        self.job_id = Some(id.clone());
        self.page_pending = false;
        self.page_request_id = None;
        self.page_requested_scope = None;
        self.progress_pending = false;
        self.progress_request_id = None;
        self.phase = "preparing".into();
        self.error = None;
        self.page = None;
        self.progress = json!({});
        self.active_operation_id = None;
        self.baseline_pending = false;
        self.export_pending = false;
        self.operation_cancel_requested = false;
        self.folder_id = "root".into();
        self.page_offset = 0;
        if self.mode.is_empty() {
            self.mode = "tree_map".into();
        }
        if self.criterion.is_empty() {
            self.criterion = "name_size".into();
        }
        if self.match_filter.is_empty() {
            self.match_filter = "all".into();
        }
        self.staged.clear();
        self.catalog_generation = None;
        self.catalog_ready = false;
        self.checked_db = None;
        if !self.baseline_matches_root(root) {
            self.baseline_id = None;
            self.baseline_root = None;
        }
        self.message = None;
        // Reserve the cancellation token synchronously before worker start.
        match atlas_service::prepare_local_analyzer_job(&id) {
            Ok(()) => {
                self.phase = "starting".into();
                self.send(bus, ctx, "start", json!({"root":self.requested_root}));
            }
            Err(error) => {
                self.phase = "failed".into();
                self.error = Some(error.to_string());
                self.job_id = None;
            }
        }
    }

    fn request_page(&mut self, bus: &mut CommandBus, ctx: &egui::Context, db: Option<&str>) {
        // The comparison is dispatched before its asynchronous reply changes phase.
        // Do not consume the page scope while that operation still owns the job.
        if self.phase != "completed" || self.active_operation_id.is_some() {
            return;
        }
        let Some(id) = self.job_id.clone() else {
            return;
        };
        if self.page_pending {
            return;
        }
        if self.operation_cancel_requested {
            return;
        }
        if self.page_requested_scope == Some(self.scope_generation) {
            return;
        }
        if self.mode == "catalog_check" && !self.catalog_ready {
            self.error = Some("Run the catalog comparison first.".into());
            self.page_requested_scope = Some(self.scope_generation);
            return;
        }
        if self.mode == "growth_diff" && self.baseline_id.is_none() {
            self.error = Some("Save a baseline, then rescan this same folder.".into());
            self.page_requested_scope = Some(self.scope_generation);
            return;
        }
        let parsed_min_size = if self.mode == "largest" {
            parse_size_mb(&self.min_size)
        } else {
            Some(0)
        };
        let parsed_min_age = if self.mode == "largest" {
            parse_age_days(&self.min_age)
        } else {
            Some(0)
        };
        let (Some(min_size), Some(min_age)) = (parsed_min_size, parsed_min_age) else {
            self.error=Some("Укажите корректный размер ≥ 0 МиБ и возраст 0–365000 дней. / Enter a valid size ≥ 0 MiB and age 0–365000 days.".into());
            self.page_requested_scope = Some(self.scope_generation);
            return;
        };
        let folder_id = if self.mode == "tree_map" {
            self.folder_id.as_str()
        } else {
            "root"
        };
        let mut req = json!({"op":"local_analyzer_page","job_id":id,"view":self.mode,"offset":self.page_offset,"limit":100,"folder_id":folder_id,"max_depth":4,"max_nodes":512,"path_contains":self.filter_path,"min_size":min_size,"min_age_days":min_age,"criterion":self.criterion,"match_filter":self.match_filter});
        if let Some(g) = &self.catalog_generation {
            req["catalog_generation"] = json!(g);
        }
        if let Some(b) = &self.baseline_id {
            req["baseline_id"] = json!(b);
        }
        if let Some(db) = db {
            req["db"] = json!(db);
        }
        self.page_requested_scope = Some(self.scope_generation);
        self.page_request_id = self.send(bus, ctx, "page", req);
        self.page_pending = self.page_request_id.is_some();
    }

    pub(crate) fn show(
        &mut self,
        root: &mut egui::Ui,
        bus: &mut CommandBus,
        ctx: &egui::Context,
        db: Option<&str>,
        catalog_volumes: &[(i64, String)],
        lang_ru: bool,
    ) {
        let t = |ru: &'static str, en: &'static str| if lang_ru { ru } else { en };
        if let Some(rx) = &self.folder_picker_rx {
            if let Ok(result) = rx.try_recv() {
                self.folder_picker_rx = None;
                match result {
                    Some(Ok(path)) => {
                        self.root_input = path;
                        self.error = None;
                    }
                    Some(Err(error)) => self.error = Some(error),
                    None => {}
                }
            }
        }
        self.sync_catalog_db(db);
        if (matches!(
            self.phase.as_str(),
            "scanning"
                | "checking_catalog"
                | "preparing"
                | "starting"
                | "cancelling"
                | "cancel_requested"
        ) || self.operation_cancel_requested)
            && !self.progress_pending
            && self
                .last_poll
                .is_none_or(|at| at.elapsed() >= Duration::from_millis(400))
        {
            self.progress_request_id = self.send(bus, ctx, "progress", json!({}));
            self.progress_pending = true;
            self.last_poll = Some(Instant::now());
        }
        let work_in_flight = matches!(
            self.phase.as_str(),
            "scanning"
                | "checking_catalog"
                | "preparing"
                | "starting"
                | "cancelling"
                | "cancel_requested"
        ) || self.progress_pending
            || self.page_pending
            || self.export_pending
            || self.baseline_pending
            || self.active_operation_id.is_some()
            || self.operation_cancel_requested;
        if work_in_flight {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        egui::CentralPanel::default().show(root,|ui| {
            ui.heading(t("Анализ локальной папки","Local folder analysis"));
            ui.label(t("Только метаданные файловой системы. Содержимое файлов не читается.","Filesystem metadata only. File contents are never read."));
            ui.horizontal(|ui| {
                ui.label(t("Папка", "Folder"));
                ui.add(egui::TextEdit::singleline(&mut self.root_input).desired_width(410.0).hint_text(t("Абсолютный путь к папке", "Absolute folder path")));
                if ui.button(t("Открыть папку / диск ПК…", "Open PC folder / drive…")).clicked() { self.open_folder_picker(ctx); }
                if ui.button(t("Анализировать папку", "Analyze folder")).clicked() { self.begin(bus,ctx); }
                if (matches!(self.phase.as_str(),"scanning"|"checking_catalog"|"starting"|"preparing")||self.page_pending||self.export_pending||self.baseline_pending||self.active_operation_id.is_some()) && ui.button(t("Отмена", "Cancel")).clicked() { self.send(bus,ctx,"cancel",json!({})); self.operation_cancel_requested=true; self.progress_pending=false; self.progress_request_id=None; self.invalidate_cancel_scope(); self.page_pending=false;self.page_request_id=None;self.phase="cancel_requested".into(); self.error=None; }
            });
            if !self.phase.is_empty() { let p=&self.progress; ui.label(format!("{} · {} {} · {} {} · {} · {} {} · {} {}",phase_label(&self.phase,lang_ru),p["files_seen"].as_u64().unwrap_or(0),t("файлов","files"),p["dirs_seen"].as_u64().unwrap_or(0),t("папок","folders"),format_bytes(p["logical_bytes"].as_u64().unwrap_or(0)),p["read_errors"].as_u64().unwrap_or(0),t("ошибок чтения","read errors"),p["symlinks_skipped"].as_u64().unwrap_or(0),t("ссылок пропущено","links skipped"))); if p["read_errors"].as_u64().unwrap_or(0)>0 {ui.colored_label(Color32::YELLOW,t("Результат неполный: есть недоступные пути.","Results are incomplete: some paths could not be read."));} }
            if let Some(e)=&self.error { ui.colored_label(Color32::LIGHT_RED,e); }
            if let Some(m)=&self.message { ui.label(m); }
            if self.phase=="cancel_requested" { ui.label(t("Останавливаем текущую задачу…", "Stopping the current job…")); }
            if self.phase!="completed" { ui.separator(); ui.label(t("Выберите папку и запустите анализ. Он может продолжаться при переключении вкладок.","Enter a folder and start analysis. It may continue while you switch tabs.")); return; }
            ui.horizontal_wrapped(|ui| { for (key,label) in MODES { let name=if lang_ru {label.split(" / ").next().unwrap_or(label)}else{label.split(" / ").last().unwrap_or(label)}; if ui.selectable_label(self.mode==key,name).clicked() { self.mode=key.into(); self.page_offset=0; self.invalidate_scope(); self.request_page(bus,ctx,db); } } });
            ui.horizontal(|ui| {
                ui.label(t("Путь","Path")); ui.add(egui::TextEdit::singleline(&mut self.filter_path).desired_width(150.0));
                if self.mode=="largest" {ui.label(t("Мин. размер, МиБ","Min size, MiB"));ui.add(egui::TextEdit::singleline(&mut self.min_size).desired_width(70.0));ui.label(t("Возраст, дни","Age, days"));ui.add(egui::TextEdit::singleline(&mut self.min_age).desired_width(60.0));}
                if self.mode=="metadata_candidates" { egui::ComboBox::from_id_salt("local_candidate_criterion").selected_text(match (self.criterion.as_str(),lang_ru){("size",true)=>"Размер ≥ 1 МБ, разные имена",("name",true)=>"Имя, разный размер",(_,true)=>"Имя и размер",("size",false)=>"Size ≥ 1 MB, different names",("name",false)=>"Name, different size",(_,false)=>"Name + size"}).show_ui(ui,|ui| {for (v,l) in [("name_size",t("Имя + размер","Name + size")),("size",t("Размер ≥ 1 МБ, разные имена","Size ≥ 1 MB, different names")),("name",t("Имя, разный размер","Name, different size"))] {ui.selectable_value(&mut self.criterion,v.into(),l);}}); }
                if self.mode=="catalog_check" { egui::ComboBox::from_id_salt("local_match_filter").selected_text(match (self.match_filter.as_str(),lang_ru){("matched",true)=>"Кандидаты",("unmatched",true)=>"Не найдено",(_,true)=>"Все",("matched",false)=>"Candidates",("unmatched",false)=>"No match",(_,false)=>"All"}).show_ui(ui,|ui|{for (v,l) in [("all",t("Все","All")),("matched",t("Кандидаты","Candidates")),("unmatched",t("Не найдено","No match"))] {ui.selectable_value(&mut self.match_filter,v.into(),l);}}); }
                if ui.button(t("Применить","Apply")).clicked(){self.page_offset=0;self.invalidate_scope();self.request_page(bus,ctx,db);}
            });
            ui.horizontal(|ui| {
                if self.mode=="catalog_check" {
                    let mut selected=self.category_volume;
                    egui::ComboBox::from_id_salt("local_catalog_volume").selected_text(selected.and_then(|id|catalog_volumes.iter().find(|x|x.0==id).map(|x|x.1.clone())).unwrap_or_else(||t("Все диски каталога","All catalog volumes").into())).show_ui(ui,|ui|{ui.selectable_value(&mut selected,None,t("Все диски каталога","All catalog volumes"));for (id,label) in catalog_volumes{ui.selectable_value(&mut selected,Some(*id),label);}});
                    if selected!=self.category_volume {self.category_volume=selected;self.catalog_ready=false;self.catalog_generation=None;self.invalidate_scope();}
                    if let (Some(db),true)= (db,!catalog_volumes.is_empty()) { if ui.button(t("Сверить метаданные","Compare metadata")).clicked(){self.invalidate_scope();self.catalog_ready=false;self.checked_db=Some(db.to_owned());self.send(bus,ctx,"catalog",json!({"db":db,"volume_id":self.category_volume,"expected_generation":Value::Null}));} }
                    else {ui.add_enabled(false,egui::Button::new(t("Каталог недоступен","Catalog unavailable"))).on_hover_text(t("Откройте SQLite-каталог для ручной проверки.","Open a SQLite catalog to run this on-demand check."));}
                    if self.phase=="checking_catalog" {ui.label(t("Сверка…","Comparing…"));}
                }
                if ui.add_enabled(!self.baseline_pending,egui::Button::new(t("Сохранить baseline","Save baseline"))).clicked(){self.baseline_pending=true;if self.send(bus,ctx,"baseline",json!({})).is_none(){self.baseline_pending=false;}}
                if ui.button(t("Обновить папку","Rescan same folder")).clicked(){let baseline=self.baseline_id.clone();self.begin(bus,ctx);self.baseline_id=baseline;}
                ui.add(egui::TextEdit::singleline(&mut self.csv_path).desired_width(220.0).hint_text(t("Новый путь CSV вне папки","New CSV path outside analyzed folder")));
                if ui.button(t("Экспорт обзора CSV","Export review CSV")).clicked(){if self.csv_path.trim().is_empty(){self.error=Some(t("Укажите новый путь CSV.","Enter a new CSV path.").into());}else if self.staged.is_empty(){self.error=Some(t("Добавьте строки в очередь экспорта.","Stage rows for export first.").into());}else{let mut fields=json!({"path":self.csv_path,"selected_ids":self.staged.iter().cloned().collect::<Vec<_>>()});if let Some(db)=db{fields["db"]=json!(db);}self.export_pending=true;if self.send(bus,ctx,"export_review_csv",fields).is_none(){self.export_pending=false;}}}
            });
            let page=self.page.clone();
            if let Some(data)=page.as_ref() {
                ui.horizontal(|ui|{ui.label(format!("{} files · {} bytes selected",data.pointer("/selected_totals/files").and_then(Value::as_i64).unwrap_or(0),data.pointer("/selected_totals/logical_bytes").and_then(Value::as_i64).unwrap_or(0))); if data["verified"]==false {ui.label(RichText::new(t("Кандидаты по метаданным — не подтверждение копии.","Metadata candidates are not proof of a copy.")).italics());}});
                if !data.pointer("/totals/complete").and_then(Value::as_bool).unwrap_or(true){ui.colored_label(Color32::YELLOW,t("Сканирование неполное. Некоторые пути недоступны.","Scan is incomplete. Some paths were inaccessible."));}
                if let Some(issues)=data["issues"].as_array(){for issue in issues.iter().take(32){ui.colored_label(Color32::YELLOW,format!("{} · {}",issue["relative_path"].as_str().unwrap_or("?"),issue["reason"].as_str().unwrap_or("read error")));}}
                if self.mode=="tree_map" {
                    ui.columns(2,|cols|{
                        cols[0].horizontal(|ui|{for crumb in data["breadcrumbs"].as_array().into_iter().flatten(){let id=crumb["id"].as_str().unwrap_or("root");if ui.link(crumb["name"].as_str().unwrap_or("Folder")).clicked(){self.folder_id=id.into();self.page_offset=0;self.invalidate_scope();self.request_page(bus,ctx,db);}ui.label("›");}});
                        cols[0].strong(t("Карта папок · 4 уровня","Folder map · 4 rings"));
                        let breadcrumbs=data["breadcrumbs"].as_array().cloned().unwrap_or_default();
                        let chart_height=(cols[0].available_height()-PAGE_FOOTER_RESERVE).clamp(192.0,460.0);
                        let clicked=chart::draw_tree_ring(&mut cols[0],data["nodes"].as_array().map(Vec::as_slice).unwrap_or(&[]),&breadcrumbs,chart_height,&data["selected_totals"],lang_ru);
                        if let Some((id,kind))=clicked {if kind=="folder"{self.folder_id=id;self.page_offset=0;self.invalidate_scope();self.request_page(bus,ctx,db);}else if !self.staged.insert(id.clone()){self.staged.remove(&id);}}
                        cols[1].strong(t("Папки и крупнейшие файлы","Folders and largest files"));
                        if let Some(rows)=data["items"].as_array(){if let Some(folder)=result_rows(&mut cols[1],rows,"tree_map",&mut self.staged,lang_ru){self.folder_id=folder;self.page_offset=0;self.invalidate_scope();self.request_page(bus,ctx,db);}}
                    });
                } else if self.mode=="bloat" {
                    ui.strong(t("Группы по метаданным","Metadata groups"));
                    if let Some(rows)=data["items"].as_array(){
                        draw_bloat_cards(
                            ui,
                            rows,
                            data["groups"].as_array().map(Vec::as_slice).unwrap_or(&[]),
                            &mut self.staged,
                            lang_ru,
                        );
                    }
                } else if self.mode=="largest" {
                    ui.label(format!("{} · {}",t("Отобрано файлов","Files in scope"),data["total"].as_u64().unwrap_or(0)));
                    if let Some(rows)=data["items"].as_array(){if let Some(folder)=result_rows(ui,rows,"largest",&mut self.staged,lang_ru){self.folder_id=folder;}}
                } else if self.mode=="metadata_candidates" {
                    ui.label(t("Группы показаны по выбранному критерию. Сравнение содержимого не выполнялось.","Groups use the selected metadata criterion. File contents were not compared."));
                    if let Some(rows)=data["items"].as_array(){
                        let mut groups:std::collections::BTreeMap<String,Vec<&Value>>=std::collections::BTreeMap::new();
                        for row in rows {let key=row["metadata_group_key"].to_string();groups.entry(key).or_default().push(row);}
                        egui::ScrollArea::vertical().max_height(scroll_content_height(ui)).show(ui,|ui|{for (_key,items) in groups {egui::Frame::group(ui.style()).show(ui,|ui|{let first=items.first();let name=first.and_then(|row|row["name"].as_str()).unwrap_or("");let size=first.and_then(|row|row["bytes"].as_u64()).unwrap_or(0);let hint=match self.criterion.as_str(){"size"=>format!("{}: {}",t("Размер","Size"),format_bytes(size)),"name"=>format!("{}: {name}",t("Имя","Name")),_=>format!("{name} · {}",format_bytes(size))};ui.strong(format!("{} · {}",if lang_ru{"Группа кандидатов"}else{"Candidate group"},items.len()));ui.small(hint);for item in items{let id=item["id"].as_str().unwrap_or("");ui.horizontal(|ui|{ui.label(format!("{} · {}",item["relative_path"].as_str().unwrap_or("?"),format_bytes(item["bytes"].as_u64().unwrap_or(0))));let enabled=self.staged.contains(id)||self.staged.len()<10_000;if ui.add_enabled(enabled,egui::Button::new(if self.staged.contains(id){t("В очереди","In CSV queue")}else{t("Добавить","Add to CSV")})).clicked()&& !self.staged.insert(id.to_owned()){self.staged.remove(id);}});}});}});
                    }
                } else if self.mode=="catalog_check" {
                    ui.columns(3,|cols|{for (i,(label,value)) in [(t("Файлов просканировано","Files scanned"),data.pointer("/totals/files").and_then(Value::as_u64).unwrap_or(0)),(t("Файлов в локальной области","Files in local scope"),data.pointer("/selected_totals/files").and_then(Value::as_u64).unwrap_or(0)),(t("Строк по фильтру","Rows matching filter"),data["total"].as_u64().unwrap_or(0))].iter().enumerate(){egui::Frame::group(cols[i].style()).show(&mut cols[i],|ui|{ui.strong(*label);ui.heading(value.to_string());});}});
                    ui.label(t("Совпадения по имени и размеру — только метаданные, не подтверждение копии.","Name and size matches are metadata candidates, not proof of a copy."));
                    if let Some(rows)=data["items"].as_array(){let _=result_rows(ui,rows,"catalog_check",&mut self.staged,lang_ru);}
                } else {
                    ui.label(t("Сравнение роста папок с сохранённой точкой этого же корня.","Folder changes compared with a saved baseline for this same root."));
                    if let Some(rows)=data["items"].as_array(){let _=result_rows(ui,rows,"growth_diff",&mut self.staged,lang_ru);}
                }
                    if data["truncated"]==true {ui.label(format!("{} omitted nodes · {} bytes remainder",data["omitted_node_count"].as_i64().unwrap_or(0),data["remainder_bytes"].as_i64().unwrap_or(0)));}
                    ui.horizontal(|ui| {
                        if self.page_offset>0 && ui.button(t("Назад","Previous")).clicked(){self.page_offset=self.page_offset.saturating_sub(100);self.invalidate_scope();self.request_page(bus,ctx,db);}
                        ui.label(format!("{} / {}",self.page_offset+1,data["total"].as_u64().unwrap_or(0)));
                        if data["has_more"]==true && ui.button(t("Далее","Next")).clicked(){self.page_offset=self.page_offset.saturating_add(100);self.invalidate_scope();self.request_page(bus,ctx,db);}
                    });
                    if self.mode=="tree_map"&&data["items"].as_array().is_some_and(|x|x.is_empty()) {ui.label(t("В этой папке нет подходящих файлов.","No matching files in this folder."));}
                    if self.mode=="catalog_check"&&self.catalog_generation.is_none(){ui.label(t("Нет актуального результата сверки.","No current catalog comparison."));}
                    if self.mode=="growth_diff"&&self.baseline_id.is_none(){ui.label(t("Для сравнения нужна сохранённая базовая точка.","A saved baseline is required for comparison."));}
            } else if self.page_requested_scope!=Some(self.scope_generation) { self.request_page(bus,ctx,db); }
        });
    }
}

#[cfg(windows)]
fn windows_folder_dialog() -> Option<Result<String, String>> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::System::Com::{
        CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows_sys::Win32::UI::Shell::{
        SHBrowseForFolderW, SHGetPathFromIDListEx, BIF_EDITBOX, BIF_NEWDIALOGSTYLE,
        BIF_RETURNONLYFSDIRS, BROWSEINFOW,
    };
    unsafe {
        let hr = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        if hr < 0 {
            return Some(Err(format!(
                "Could not initialize the Windows folder dialog (HRESULT {hr:#x})."
            )));
        }
        let title: Vec<u16> = "Choose a local folder\0".encode_utf16().collect();
        let mut info: BROWSEINFOW = std::mem::zeroed();
        info.hwndOwner = null_mut();
        info.pidlRoot = null_mut();
        info.pszDisplayName = null_mut();
        info.lpszTitle = title.as_ptr();
        info.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX;
        info.lpfn = None;
        info.lParam = 0;
        info.iImage = 0;
        let pidl = SHBrowseForFolderW(&info);
        let result =
            if pidl.is_null() {
                None
            } else {
                let mut buffer = vec![0u16; 32768];
                let ok = SHGetPathFromIDListEx(pidl, buffer.as_mut_ptr(), buffer.len() as u32, 0);
                CoTaskMemFree(pidl as _);
                if ok == 0 {
                    Some(Err("The selected item is not a filesystem folder.".into()))
                } else {
                    let end = buffer.iter().position(|v| *v == 0).unwrap_or(buffer.len());
                    Some(String::from_utf16(&buffer[..end]).map_err(|_| {
                        "The selected folder path contains invalid UTF-16.".to_owned()
                    }))
                }
            };
        CoUninitialize();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::Reply;

    #[test]
    fn tree_map_chart_and_hundred_rows_keep_pagination_visible() {
        let rows: Vec<Value> = (0..100)
            .map(|index| {
                json!({
                    "id": format!("file:{index}"),
                    "kind": "file",
                    "name": format!("file-{index}.mkv"),
                    "relative_path": format!("Folder/file-{index}.mkv"),
                    "bytes": 1024,
                })
            })
            .collect();
        let nodes: Vec<Value> = (0..100)
            .map(|index| {
                json!({
                    "id": format!("file:{index}"),
                    "parent_id": "root",
                    "kind": "file",
                    "name": format!("file-{index}.mkv"),
                    "bytes": 1024,
                })
            })
            .collect();
        let breadcrumbs = vec![json!({"id":"root", "name":"All files"})];
        let totals = json!({"logical_bytes":102_400, "files":100});

        for width in [1366.0, 1024.0] {
            let ctx = egui::Context::default();
            let mut staged = HashSet::new();
            let mut next_rect = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 768.0),
                    )),
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        ui.heading("Local analysis");
                        // Approximate the folder, progress, mode and filter controls above the page.
                        ui.add_space(220.0);
                        ui.label("Page status · selected totals · scan issues");
                        ui.columns(2, |columns| {
                            columns[0].strong("Folder map · 4 rings");
                            let chart_height = (columns[0].available_height()
                                - PAGE_FOOTER_RESERVE)
                                .clamp(192.0, 460.0);
                            chart::draw_tree_ring(
                                &mut columns[0],
                                &nodes,
                                &breadcrumbs,
                                chart_height,
                                &totals,
                                false,
                            );
                            columns[1].strong("Folders and largest files");
                            result_rows(&mut columns[1], &rows, "tree_map", &mut staged, false);
                        });
                        ui.label("Omitted nodes · remainder bytes");
                        ui.horizontal(|ui| {
                            let _ = ui.button("Previous");
                            ui.label("1 / 100");
                            next_rect = Some(ui.button("Next").rect);
                        });
                    });
                },
            );
            output.textures_delta.clear();
            let rect = next_rect.expect("pagination control should be rendered");
            assert!(
                rect.min.y >= 0.0,
                "pagination starts on screen at {width}px"
            );
            assert!(
                rect.max.y <= 768.0,
                "pagination ends on screen at {width}px"
            );
        }
    }

    #[test]
    fn bloat_cards_and_all_stage_actions_fit_two_columns() {
        let classes = ["cache", "cache", "dev", "installers", "archives"];
        let rows: Vec<Value> = classes
            .iter()
            .enumerate()
            .map(|(index, class)| {
                json!({
                    "id": format!("file:{index}"),
                    "classification": class,
                    "relative_path": format!("a/very/long/path/that/must/be/truncated/file-{index}.bin"),
                    "bytes": 1024,
                })
            })
            .collect();
        let groups: Vec<Value> = ["cache", "dev", "installers", "archives"]
            .iter()
            .map(|class| json!({"classification":class,"files":1,"bytes":1024}))
            .collect();

        for width in [1366.0, 1024.0] {
            let ctx = egui::Context::default();
            let mut staged = HashSet::new();
            let mut cards = Vec::new();
            let mut actions = Vec::new();
            for _ in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 768.0),
                        )),
                        ..Default::default()
                    },
                    |root| {
                        egui::CentralPanel::default().show(root, |ui| {
                            ui.heading("Metadata groups");
                            let (card_rects, action_rects) =
                                draw_bloat_cards(ui, &rows, &groups, &mut staged, false);
                            cards = card_rects;
                            actions = action_rects;
                        });
                    },
                );
                output.textures_delta.clear();
            }
            assert_eq!(cards.len(), 4, "all four category cards are laid out");
            assert_eq!(actions.len(), 5, "all five staged rows have buttons");
            for rect in cards.iter().chain(&actions) {
                assert!(rect.min.x >= 0.0 && rect.max.x <= width);
                assert!(rect.min.y >= 0.0 && rect.max.y <= 768.0);
            }
            assert!(
                cards[1].min.x > cards[0].min.x,
                "first two cards share a row"
            );
            assert!(
                cards[2].min.y > cards[0].min.y,
                "remaining cards use the next row"
            );
        }
    }

    #[test]
    fn local_work_requests_repaint_after_progress_frames_settle() {
        let ctx = egui::Context::default();
        let mut bus = CommandBus::new();
        let mut view = LocalAnalyzer::default();
        view.phase = "scanning".into();
        view.last_poll = Some(Instant::now());
        let mut repaint_delay = Duration::ZERO;
        for frame in 0..8 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(f64::from(frame) * 0.2),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1024.0, 768.0),
                    )),
                    ..Default::default()
                },
                |root| view.show(root, &mut bus, &ctx, None, &[], false),
            );
            repaint_delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
            output.textures_delta.clear();
        }
        assert!(
            !repaint_delay.is_zero() && repaint_delay <= Duration::from_millis(100),
            "busy analyzer should schedule another frame, got {repaint_delay:?}"
        );
    }

    #[test]
    fn late_cancelled_save_and_export_keep_the_local_snapshot_completed() {
        for op in ["baseline", "export_review_csv"] {
            for acknowledgment_first in [true, false] {
                let ctx = egui::Context::default();
                let mut bus = CommandBus::new();
                let mut view = LocalAnalyzer::default();
                view.job_id = Some("completed-job".into());
                view.generation = 2;
                view.scope_generation = 4;
                view.phase = "cancel_requested".into();
                view.progress = json!({"results_ready":true});
                view.baseline_pending = op == "baseline";
                view.export_pending = op == "export_review_csv";
                view.active_operation_id = Some("operation-1".into());
                view.page = Some(json!({"items":[{"id":"file:1","name":"kept"}]}));
                view.invalidate_cancel_scope();
                let mut acknowledgment = Reply {
                    id: 9,
                    purpose: Purpose::LocalAnalyzer {
                        job_id: "completed-job".into(),
                        generation: 2,
                        scope_generation: 4,
                        op: "cancel".into(),
                    },
                    response: json!({"ok":true,"data":{"scope":"operation","pending":false}}),
                };
                if acknowledgment_first {
                    view.apply_reply(&mut acknowledgment, &mut bus, &ctx, None);
                }
                let mut reply = Reply {
                    id: 8,
                    purpose: Purpose::LocalAnalyzer {
                        job_id: "completed-job".into(),
                        generation: 2,
                        scope_generation: 4,
                        op: op.into(),
                    },
                    response: json!({"ok":false,"error":{"code":"cancelled","message":"cancelled"}}),
                };
                assert!(view.apply_reply(&mut reply, &mut bus, &ctx, None));
                if !acknowledgment_first {
                    view.apply_reply(&mut acknowledgment, &mut bus, &ctx, None);
                }
                assert_eq!(view.phase, "completed");
                assert_eq!(view.page.as_ref().unwrap()["items"][0]["name"], "kept");
                assert_eq!(view.job_id.as_deref(), Some("completed-job"));
                assert!(!view.baseline_pending && !view.export_pending);
                assert!(view.active_operation_id.is_none());
                assert!(
                    bus.poll().is_none(),
                    "operation cancellation must not close the snapshot"
                );
            }
        }

        let mut view = LocalAnalyzer::default();
        view.phase = "scanning".into();
        view.fail(&json!({"ok":false,"error":{"code":"cancelled"}}));
        assert_eq!(view.phase, "cancelled");
    }

    #[test]
    fn catalog_dispatch_does_not_consume_the_page_scope_before_completion() {
        let ctx = egui::Context::default();
        let mut bus = CommandBus::new();
        let mut view = LocalAnalyzer::default();
        view.job_id = Some("catalog-job".into());
        view.phase = "completed".into();
        view.mode = "catalog_check".into();
        view.invalidate_scope();
        view.active_operation_id = Some("catalog-operation".into());
        // Same-frame automatic page request after the Compare button dispatches.
        view.request_page(&mut bus, &ctx, Some("synthetic.sqlite"));
        assert!(view.page_requested_scope.is_none());
        assert!(view.error.is_none());
        let mut reply = Reply {
            id: 1,
            purpose: Purpose::LocalAnalyzer {
                job_id: "catalog-job".into(),
                generation: view.generation,
                scope_generation: view.scope_generation,
                op: "catalog".into(),
            },
            response: json!({"ok":true,"data":{"phase":"checking_catalog"}}),
        };
        // Also recover an earlier not-ready scope, rather than suppressing results.
        view.page_requested_scope = Some(view.scope_generation);
        view.apply_reply(&mut reply, &mut bus, &ctx, Some("synthetic.sqlite"));
        assert!(view.page_requested_scope.is_none());
        assert_eq!(view.phase, "checking_catalog");
        view.request_page(&mut bus, &ctx, Some("synthetic.sqlite"));
        assert!(view.page_requested_scope.is_none());
        assert!(view.error.is_none());
    }

    #[test]
    fn review_csv_reply_releases_the_operation_on_success_and_failure() {
        for response in [
            json!({"ok":true,"data":{"rows":1}}),
            json!({"ok":false,"error":{"code":"file_exists","message":"exists"}}),
        ] {
            let ctx = egui::Context::default();
            let mut bus = CommandBus::new();
            let mut view = LocalAnalyzer::default();
            view.job_id = Some("csv-job".into());
            view.generation = 2;
            view.scope_generation = 3;
            view.phase = "completed".into();
            view.export_pending = true;
            view.active_operation_id = Some("csv-op".into());
            let mut reply = Reply {
                id: 7,
                purpose: Purpose::LocalAnalyzer {
                    job_id: "csv-job".into(),
                    generation: 2,
                    scope_generation: 3,
                    op: "export_review_csv".into(),
                },
                response,
            };
            let success = reply.response["ok"] == true;
            assert!(view.apply_reply(&mut reply, &mut bus, &ctx, None));
            assert!(!view.export_pending);
            assert!(view.active_operation_id.is_none());
            assert_eq!(view.phase, "completed");
            assert_eq!(view.message.is_some(), success);
            assert_eq!(view.error.is_some(), !success);
        }
    }

    #[test]
    fn stale_local_worker_reply_cannot_replace_the_active_job() {
        let ctx = egui::Context::default();
        let mut bus = CommandBus::new();
        let mut view = LocalAnalyzer::default();
        view.job_id = Some("current".into());
        view.generation = 8;
        view.phase = "scanning".into();
        let mut reply = Reply {
            id: 1,
            purpose: Purpose::LocalAnalyzer {
                job_id: "old".into(),
                generation: 7,
                scope_generation: 0,
                op: "start".into(),
            },
            response: json!({"ok":true,"data":{"phase":"scanning"}}),
        };
        assert!(view.apply_reply(&mut reply, &mut bus, &ctx, None));
        assert_eq!(view.job_id.as_deref(), Some("current"));
        assert_eq!(view.generation, 8);
        assert_eq!(view.phase, "scanning");
    }

    #[test]
    fn stale_page_from_previous_scope_is_dropped_and_unblocks_current_scope() {
        let ctx = egui::Context::default();
        let mut bus = CommandBus::new();
        let mut view = LocalAnalyzer::default();
        view.job_id = Some("same-job".into());
        view.generation = 3;
        view.scope_generation = 9;
        view.page_pending = true;
        view.page_request_id = Some(17);
        view.active_operation_id = Some("old-op".into());
        view.page = Some(json!({"items":[{"id":"kept"}]}));
        let mut reply = Reply {
            id: 17,
            purpose: Purpose::LocalAnalyzer {
                job_id: "same-job".into(),
                generation: 3,
                scope_generation: 8,
                op: "page".into(),
            },
            response: json!({"ok":true,"data":{"items":[{"id":"stale"}]}}),
        };
        view.apply_reply(&mut reply, &mut bus, &ctx, None);
        assert_eq!(view.page.as_ref().unwrap()["items"][0]["id"], "kept");
        assert!(!view.page_pending);
        assert_eq!(view.active_operation_id, None);
    }

    #[test]
    fn cancelling_a_page_operation_keeps_the_completed_local_snapshot() {
        let ctx = egui::Context::default();
        let mut bus = CommandBus::new();
        let mut view = LocalAnalyzer::default();
        view.job_id = Some("complete-job".into());
        view.generation = 4;
        view.scope_generation = 12;
        view.phase = "cancel_requested".into();
        view.progress = json!({"results_ready":true});
        view.active_operation_id = Some("page-op".into());
        let mut reply = Reply {
            id: 8,
            purpose: Purpose::LocalAnalyzer {
                job_id: "complete-job".into(),
                generation: 4,
                scope_generation: 12,
                op: "cancel".into(),
            },
            response: json!({"ok":true,"data":{"scope":"operation","pending":false}}),
        };
        view.apply_reply(&mut reply, &mut bus, &ctx, None);
        assert_eq!(view.phase, "completed");
        assert_eq!(view.job_id.as_deref(), Some("complete-job"));
        assert!(view.operation_cancel_requested);
        assert!(
            bus.poll().is_none(),
            "operation cancellation must not close the completed job snapshot"
        );
    }

    #[test]
    fn catalog_switch_invalidates_comparison_without_discarding_local_job() {
        let mut view = LocalAnalyzer::default();
        view.job_id = Some("local-scan".into());
        view.mode = "catalog_check".into();
        view.checked_db = Some("old.sqlite".into());
        view.catalog_generation = Some("generation-1".into());
        view.catalog_ready = true;
        view.page = Some(json!({"items":[{"id":"old-match"}]}));
        view.sync_catalog_db(Some("new.sqlite"));
        assert_eq!(view.job_id.as_deref(), Some("local-scan"));
        assert_eq!(view.catalog_generation, None);
        assert!(!view.catalog_ready);
        assert_eq!(view.page, None);
    }

    #[test]
    fn baseline_is_reused_only_for_same_root_and_mib_filter_is_exact() {
        let mut view = LocalAnalyzer::default();
        view.baseline_id = Some("baseline-1".into());
        view.baseline_root = Some(r"C:\QA\Media".into());
        assert!(view.baseline_matches_root(r"C:\QA\Media"));
        #[cfg(windows)]
        assert!(view.baseline_matches_root(r"c:\qa\media\"));
        #[cfg(not(windows))]
        assert!(!view.baseline_matches_root(r"c:\qa\media\"));
        assert!(!view.baseline_matches_root(r"C:\QA\Other"));
        assert_eq!(parse_size_mb("100"), Some(104_857_600));
        assert_eq!(parse_size_mb("NaN"), None);
    }

    #[test]
    fn relative_root_is_rejected_before_any_job_is_prepared() {
        let ctx = egui::Context::default();
        let mut bus = CommandBus::new();
        let mut view = LocalAnalyzer::default();
        view.root_input = "relative-folder".into();
        view.begin(&mut bus, &ctx);
        assert!(view.job_id.is_none());
        assert!(view
            .error
            .as_deref()
            .is_some_and(|s| s.contains("absolute")));
    }
}
