//! Explicit, read-only OS volume verification and user-owned physical pairing.
//!
//! Catalogs and linked CTF sources are never opened here. Only the separately
//! configured private binding sidecar is written by an explicitly confirmed
//! pair/unpair action.

use crate::shelf::ShelfContext;
use eframe::egui;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

const PHYSICAL_STATE_TTL: Duration = Duration::from_secs(12);
const PHYSICAL_REFRESH_INTERVAL: Duration = Duration::from_secs(6);

#[derive(Clone, Debug, PartialEq, Eq)]
struct OsVolume {
    physical_id: String,
    label: String,
    mount_root: String,
    system: bool,
    eligible: bool,
}

#[derive(Clone, Debug)]
struct ProbeSnapshot {
    platform: String,
    observed_ids: Vec<String>,
    items: Vec<OsVolume>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Refresh,
    Pair,
    Unpair,
}

#[derive(Debug)]
struct WorkerReply {
    ticket: u64,
    scope_key: String,
    action: Action,
    snapshot: Option<ProbeSnapshot>,
    states: HashMap<i64, String>,
    success: bool,
}

fn data(response: &Value) -> Option<&Value> {
    if response.get("ok")?.as_bool()? {
        response.get("data")
    } else {
        None
    }
}

fn parse_probe(reply: &Value) -> Option<ProbeSnapshot> {
    let data = data(reply)?;
    if !data.get("probe_ok")?.as_bool()? {
        return None;
    }
    let platform = data.get("platform")?.as_str()?;
    if !matches!(platform, "windows" | "macos") {
        return None;
    }
    let observed_ids: Vec<String> = data
        .get("observed_ids")?
        .as_array()?
        .iter()
        .map(|id| id.as_str().map(str::to_owned))
        .collect::<Option<_>>()?;
    if observed_ids.len() > 256 {
        return None;
    }
    let rows = data.get("items")?.as_array()?;
    if rows.len() != observed_ids.len() || rows.len() > 256 {
        return None;
    }
    let mut items = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let physical_id = row.get("physical_id")?.as_str()?;
        if physical_id != observed_ids[index] || !valid_physical_id(physical_id, platform) {
            return None;
        }
        let label = bounded_metadata(row.get("label")?.as_str()?, 256)?;
        let mount_root = bounded_metadata(row.get("mount_root")?.as_str()?, 4096)?;
        let system = row.get("system")?.as_bool()?;
        let declared_eligible = row.get("eligible")?.as_bool()?;
        items.push(OsVolume {
            physical_id: physical_id.to_owned(),
            label,
            mount_root,
            system,
            // Independently enforce the safety boundary. An OS payload cannot
            // make a duplicate or system UUID pairable by setting a flag.
            eligible: declared_eligible
                && !system
                && observed_ids
                    .iter()
                    .filter(|id| id.as_str() == physical_id)
                    .take(2)
                    .count()
                    < 2
                && !row.get("mount_root")?.as_str()?.is_empty(),
        });
    }
    Some(ProbeSnapshot {
        platform: platform.to_owned(),
        observed_ids,
        items,
    })
}

fn valid_physical_id(value: &str, platform: &str) -> bool {
    let Some(guid) = value.strip_prefix(match platform {
        "windows" => "win:",
        "macos" => "mac:",
        _ => return false,
    }) else {
        return false;
    };
    let uuid = if platform == "windows" {
        let Some((guid, serial)) = guid.split_once(':') else {
            return false;
        };
        let Some(uuid) = guid
            .strip_prefix('{')
            .and_then(|body| body.strip_suffix('}'))
        else {
            return false;
        };
        if serial.len() != 8 || !serial.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return false;
        }
        uuid
    } else {
        guid
    };
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn bounded_metadata(value: &str, limit: usize) -> Option<String> {
    if value.chars().count() > limit || value.chars().any(char::is_control) {
        None
    } else {
        Some(value.to_owned())
    }
}

fn parse_states(reply: &Value, expected_ids: &[i64]) -> HashMap<i64, String> {
    let Some(data) = data(reply) else {
        return HashMap::new();
    };
    if data.get("probe_ok").and_then(Value::as_bool) != Some(true) {
        return HashMap::new();
    }
    let Some(rows) = data.get("items").and_then(Value::as_array) else {
        return HashMap::new();
    };
    let mut states = HashMap::new();
    for row in rows {
        let (Some(id), Some(state)) = (
            row.get("volume_id").and_then(Value::as_i64),
            row.get("state").and_then(Value::as_str),
        ) else {
            return HashMap::new();
        };
        if !expected_ids.contains(&id)
            || !matches!(state, "connected" | "offline" | "conflict" | "unverified")
        {
            return HashMap::new();
        }
        states.insert(id, state.to_owned());
    }
    states
}

fn status_request(scope: &ShelfContext, volume_ids: &[i64], snapshot: &ProbeSnapshot) -> Value {
    json!({
        "op":"physical_status",
        "user_store":scope.user_store,
        "catalog_key":scope.catalog_key,
        "platform":snapshot.platform,
        "volume_ids":volume_ids,
        "observed_ids":snapshot.observed_ids,
        "probe_ok":true,
    })
}

fn service(request: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&request.to_string()))
        .unwrap_or_else(|_| json!({"ok":false}))
}

fn run_worker(
    ticket: u64,
    scope: ShelfContext,
    volume_id: i64,
    volume_ids: Vec<i64>,
    action: Action,
    selected_physical_id: Option<String>,
) -> WorkerReply {
    let mut reply = WorkerReply {
        ticket,
        scope_key: scope.catalog_key.clone(),
        action,
        snapshot: None,
        states: HashMap::new(),
        success: false,
    };

    if action == Action::Unpair {
        let result = service(json!({
            "op":"physical_unpair",
            "user_store":scope.user_store,
            "catalog_key":scope.catalog_key,
            "platform":scope.platform,
            "volume_id":volume_id,
            "confirmed":true,
        }));
        if data(&result)
            .and_then(|d| d.get("removed"))
            .and_then(Value::as_bool)
            .is_some()
        {
            reply.success = data(&result)
                .and_then(|d| d.get("removed"))
                .and_then(Value::as_bool)
                == Some(true);
        } else {
            return reply;
        }
    }

    // Pair always performs a fresh complete OS enumeration in this worker
    // immediately before invoking the write-only private-sidecar operation.
    let probe_reply = service(json!({"op":"physical_probe_os"}));
    let Some(snapshot) = parse_probe(&probe_reply) else {
        return reply;
    };
    reply.snapshot = Some(snapshot.clone());

    if action == Action::Pair {
        let Some(wanted) = selected_physical_id.as_deref() else {
            return reply;
        };
        let mut matching = snapshot
            .items
            .iter()
            .filter(|item| item.physical_id == wanted);
        let Some(item) = matching.next() else {
            return reply;
        };
        if matching.next().is_some() || !item.eligible || item.system || item.mount_root.is_empty()
        {
            return reply;
        }
        let result = service(json!({
            "op":"physical_pair",
            "user_store":scope.user_store,
            "catalog_key":scope.catalog_key,
            "platform":snapshot.platform,
            "volume_id":volume_id,
            "physical_id":wanted,
            "observed_ids":snapshot.observed_ids,
            "probe_ok":true,
            "confirmed":true,
        }));
        reply.success = data(&result)
            .and_then(|d| d.get("paired"))
            .and_then(Value::as_bool)
            == Some(true);
        if !reply.success {
            return reply;
        }
    }

    let status = service(status_request(&scope, &volume_ids, &snapshot));
    reply.states = parse_states(&status, &volume_ids);
    reply
}

/// Compact physical identity review dialog. It never opens a mounted root.
pub struct PhysicalPanel {
    scope: Option<ShelfContext>,
    volume_id: i64,
    volume_ids: Vec<i64>,
    is_open: bool,
    refresh_requested: bool,
    ticket: u64,
    pending: bool,
    last_checked: Option<Instant>,
    next_refresh_at: Option<Instant>,
    tx: Sender<WorkerReply>,
    rx: Receiver<WorkerReply>,
    snapshot: Option<ProbeSnapshot>,
    states: HashMap<i64, String>,
    selected_physical_id: Option<String>,
    confirm_pair: bool,
    confirm_unpair: bool,
    notice: Notice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Notice {
    None,
    ProbeFailed,
    PairFailed,
    PairSucceeded,
    UnpairFailed,
    UnpairSucceeded,
}

impl Default for PhysicalPanel {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            scope: None,
            volume_id: 0,
            volume_ids: Vec::new(),
            is_open: false,
            refresh_requested: false,
            ticket: 0,
            pending: false,
            last_checked: None,
            next_refresh_at: None,
            tx,
            rx,
            snapshot: None,
            states: HashMap::new(),
            selected_physical_id: None,
            confirm_pair: false,
            confirm_unpair: false,
            notice: Notice::None,
        }
    }
}

impl PhysicalPanel {
    // Kept for later physical-identity UI scope; the v1.0 panel stays inert.
    #[allow(dead_code)]
    pub fn open(&mut self, scope: ShelfContext, volume_id: i64, volume_ids: Vec<i64>) {
        self.ticket = self.ticket.wrapping_add(1);
        self.scope = Some(scope);
        self.volume_id = volume_id;
        self.volume_ids = unique_positive_ids(volume_ids);
        self.is_open = true;
        // Physical USB identity is not part of the Windows 1.0 UI surface.
        // Keep this panel entry point inert so no OS probe is started.
        self.refresh_requested = false;
        self.pending = false;
        self.last_checked = None;
        self.next_refresh_at = None;
        self.snapshot = None;
        self.states.clear();
        self.selected_physical_id = None;
        self.confirm_pair = false;
        self.confirm_unpair = false;
        self.notice = Notice::None;
    }

    pub fn close_if_scope_changed(&mut self, current_key: Option<&str>) {
        let matches = self
            .scope
            .as_ref()
            .is_some_and(|scope| Some(scope.catalog_key.as_str()) == current_key);
        if !matches {
            self.ticket = self.ticket.wrapping_add(1);
            self.scope = None;
            self.is_open = false;
            self.refresh_requested = false;
            self.pending = false;
            self.last_checked = None;
            self.next_refresh_at = None;
            self.snapshot = None;
            self.states.clear();
            self.selected_physical_id = None;
            self.confirm_pair = false;
            self.confirm_unpair = false;
        }
    }

    pub fn state(&self, volume_id: i64) -> Option<&str> {
        if !self.is_open
            || self
                .last_checked
                .is_none_or(|at| at.elapsed() > PHYSICAL_STATE_TTL)
        {
            return None;
        }
        self.states.get(&volume_id).map(String::as_str)
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    fn start(&mut self, ctx: &egui::Context, action: Action) {
        let Some(scope) = self.scope.clone() else {
            return;
        };
        self.ticket = self.ticket.wrapping_add(1);
        let ticket = self.ticket;
        let volume_id = self.volume_id;
        let volume_ids = self.volume_ids.clone();
        let selected = (action == Action::Pair)
            .then(|| self.selected_physical_id.clone())
            .flatten();
        let tx = self.tx.clone();
        let repaint = ctx.clone();
        self.pending = true;
        self.refresh_requested = false;
        self.snapshot = None;
        self.states.clear();
        self.last_checked = None;
        self.confirm_pair = false;
        self.notice = Notice::None;
        thread::spawn(move || {
            let result = run_worker(ticket, scope, volume_id, volume_ids, action, selected);
            let _ = tx.send(result);
            repaint.request_repaint();
        });
    }

    fn poll(&mut self) {
        while let Ok(reply) = self.rx.try_recv() {
            let scope_key = self.scope.as_ref().map(|s| s.catalog_key.as_str());
            if reply.ticket != self.ticket || Some(reply.scope_key.as_str()) != scope_key {
                continue;
            }
            self.pending = false;
            if let Some(snapshot) = reply.snapshot {
                let checked = Instant::now();
                self.last_checked = Some(checked);
                self.next_refresh_at = Some(checked + PHYSICAL_REFRESH_INTERVAL);
                self.states = reply.states;
                self.snapshot = Some(snapshot);
                match reply.action {
                    Action::Pair => {
                        self.notice = if reply.success {
                            Notice::PairSucceeded
                        } else {
                            Notice::PairFailed
                        }
                    }
                    Action::Unpair => {
                        self.notice = if reply.success {
                            Notice::UnpairSucceeded
                        } else {
                            Notice::UnpairFailed
                        }
                    }
                    Action::Refresh => self.notice = Notice::None,
                }
            } else {
                self.last_checked = None;
                self.next_refresh_at = Some(Instant::now() + PHYSICAL_REFRESH_INTERVAL);
                self.snapshot = None;
                self.states.clear();
                self.notice = if reply.action == Action::Pair {
                    Notice::PairFailed
                } else {
                    Notice::ProbeFailed
                };
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, russian: bool) {
        if !self.is_open {
            return;
        }
        let tr = |ru: &'static str, en: &'static str| if russian { ru } else { en };
        let mut open = self.is_open;
        egui::Window::new(tr("Проверка физического диска", "Physical disk check"))
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(tr(
                    "USB-привязка и отвязка недоступны в версии 1.0.",
                    "USB pairing and unpairing are unavailable in version 1.0.",
                ));
            });
        self.is_open = open;
        if !self.is_open {
            self.scope = None;
            self.snapshot = None;
            self.states.clear();
            self.last_checked = None;
            self.next_refresh_at = None;
            self.selected_physical_id = None;
        }
    }

    #[allow(dead_code)]
    fn show_enabled(&mut self, ctx: &egui::Context, russian: bool) {
        if !self.is_open {
            return;
        }
        self.poll();
        if self.next_refresh_at.is_some_and(|at| Instant::now() >= at) && !self.pending {
            self.refresh_requested = true;
        }
        if self.refresh_requested && !self.pending {
            self.start(ctx, Action::Refresh);
        }
        ctx.request_repaint_after(Duration::from_secs(1));
        let tr = |ru: &'static str, en: &'static str| if russian { ru } else { en };
        let mut open = self.is_open;
        egui::Window::new(tr("Проверка физического диска", "Physical disk check"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(510.0)
            .show(ctx, |ui| {
                ui.label(if russian {
                    format!("Выбранный диск каталога: #{}", self.volume_id)
                } else {
                    format!("Selected catalog disk: #{}", self.volume_id)
                });
                ui.small(tr(
                    "Проверяется только локальная метаинформация ОС. Файлы не открываются.",
                    "Only local OS metadata is checked. Files are never opened.",
                ));
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.pending, egui::Button::new(tr("Обновить", "Refresh"))).clicked() {
                        self.refresh_requested = true;
                    }
                    if self.pending { ui.spinner(); ui.label(tr("Проверка…", "Checking…")); }
                    let state = self.state(self.volume_id).unwrap_or("unverified");
                    ui.label(format!("{}: {}", tr("Статус", "Status"), state_label(state, russian)));
                });
                if let Some(checked) = self.last_checked {
                    let age = checked.elapsed().as_secs();
                    ui.small(if russian {
                        format!("Состояние по последней проверке: {age} сек. назад; обновление каждые 6 сек.")
                    } else {
                        format!("State as of last check: {age}s ago; refreshes every 6s.")
                    });
                }

                if let Some(snapshot) = self.snapshot.clone() {
                    let eligible: Vec<_> = snapshot.items.iter().filter(|item| item.eligible).collect();
                    let selected_is_eligible = self.selected_physical_id.as_deref().is_some_and(|id|
                        eligible.iter().any(|item| item.physical_id == id));
                    if eligible.is_empty() {
                        ui.label(tr("Нет однозначных доступных томов для выбора.", "No unambiguous mounted volumes are available."));
                    }
                    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        for item in eligible {
                            let title = if item.label.trim().is_empty() { tr("(без имени)", "(unnamed)").to_owned() } else { item.label.clone() };
                            let row = format!("{}  ·  {}", title, item.mount_root);
                            let selected = self.selected_physical_id.as_deref() == Some(item.physical_id.as_str());
                            if ui.selectable_label(selected, row).clicked() {
                                self.selected_physical_id = Some(item.physical_id.clone());
                                self.confirm_pair = false;
                            }
                        }
                    });
                    if let Some(selected) = self.selected_physical_id.as_deref()
                        .and_then(|id| snapshot.items.iter().find(|item| item.physical_id == id && item.eligible)) {
                        ui.label(format!("{}: {}", tr("Том", "Volume"), if selected.label.is_empty() { tr("(без имени)", "(unnamed)") } else { &selected.label }));
                        ui.label(format!("{}: {}", tr("Текущий корень", "Current mount root"), selected.mount_root));
                    }
                    ui.add_enabled_ui(selected_is_eligible, |ui| {
                        ui.checkbox(&mut self.confirm_pair, tr(
                            "Подтверждаю связь этого диска каталога с выбранным томом ОС.",
                            "I confirm pairing this catalog disk with the selected OS volume.",
                        ));
                    });
                    if ui.add_enabled(!self.pending && self.confirm_pair && selected_is_eligible,
                        egui::Button::new(tr("Связать с томом", "Pair with volume"))).clicked() {
                        self.confirm_pair = false;
                        self.start(ctx, Action::Pair);
                    }
                } else if self.pending {
                    ui.label(tr("Ожидается полный снимок ОС…", "Waiting for a complete OS snapshot…"));
                } else {
                    ui.colored_label(egui::Color32::YELLOW, tr(
                        "Проверка не завершена. Состояние остаётся непроверенным.",
                        "The probe did not complete. State remains unverified.",
                    ));
                }

                ui.separator();
                ui.checkbox(&mut self.confirm_unpair, tr(
                    "Подтверждаю удаление сохранённой связи.",
                    "I confirm removing the saved pairing.",
                ));
                if ui.add_enabled(!self.pending && self.confirm_unpair,
                    egui::Button::new(tr("Отвязать", "Unpair"))).clicked() {
                    self.confirm_unpair = false;
                    self.start(ctx, Action::Unpair);
                }
                match self.notice {
                    Notice::None => {},
                    Notice::ProbeFailed => { ui.colored_label(egui::Color32::YELLOW, tr("OS-проверка не удалась; состояние не подтверждено.", "OS probe failed; presence is unverified.")); },
                    Notice::PairFailed => { ui.colored_label(egui::Color32::YELLOW, tr("Связь не сохранена: повторная проверка не подтвердила уникальный том.", "Pairing was not saved: the fresh probe did not confirm a unique eligible volume.")); },
                    Notice::PairSucceeded => { ui.label(tr("Связь сохранена в личных настройках.", "Pairing saved in private user settings.")); },
                    Notice::UnpairFailed => { ui.label(tr("Сохранённая связь не найдена или не изменена.", "No saved pairing was found or changed.")); },
                    Notice::UnpairSucceeded => { ui.label(tr("Связь удалена из личных настроек.", "Pairing removed from private user settings.")); },
                }
            });
        self.is_open = open;
        if !self.is_open {
            self.ticket = self.ticket.wrapping_add(1);
            self.scope = None;
            self.snapshot = None;
            self.states.clear();
            self.last_checked = None;
            self.next_refresh_at = None;
            self.selected_physical_id = None;
        }
    }
}

// Used by the retained later-release physical identity panel entry point.
#[allow(dead_code)]
fn unique_positive_ids(ids: Vec<i64>) -> Vec<i64> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter()
        .filter(|id| *id > 0 && seen.insert(*id))
        .take(512)
        .collect()
}

fn state_label(state: &str, russian: bool) -> &'static str {
    match (state, russian) {
        ("connected", true) => "подключён (том смонтирован)",
        ("offline", true) => "не наблюдается",
        ("conflict", true) => "неоднозначная UUID",
        (_, true) => "не проверен",
        ("connected", false) => "connected (volume observed)",
        ("offline", false) => "not observed",
        ("conflict", false) => "UUID conflict",
        (_, false) => "unverified",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn uuid(hex: &str) -> String {
        format!("mac:{hex}")
    }

    #[test]
    fn generated_os_metadata_rejects_duplicate_system_and_malformed_candidates() {
        let duplicate = uuid("aabbccdd-eeff-0011-2233-445566778899");
        let system = uuid("00112233-4455-6677-8899-aabbccddeeff");
        let unique = uuid("ffeeddcc-bbaa-9988-7766-554433221100");
        let payload = json!({"ok":true,"data":{
            "platform":"macos","probe_ok":true,
            "observed_ids":[duplicate,duplicate,system,unique],
            "items":[
                {"physical_id":duplicate,"label":"USB A","mount_root":"/Volumes/A","system":false,"eligible":false},
                {"physical_id":duplicate,"label":"USB clone","mount_root":"/Volumes/B","system":false,"eligible":false},
                {"physical_id":system,"label":"System","mount_root":"/","system":true,"eligible":false},
                {"physical_id":unique,"label":"USB unique","mount_root":"/Volumes/C","system":false,"eligible":true}
            ]
        }});
        let snapshot = parse_probe(&payload).unwrap();
        assert_eq!(snapshot.items.len(), 4);
        assert_eq!(snapshot.observed_ids.len(), 4);
        assert!(snapshot.items[..3].iter().all(|item| !item.eligible));
        assert!(snapshot.items[3].eligible);

        let malformed = json!({"ok":true,"data":{
            "platform":"macos","probe_ok":true,"observed_ids":["mac:bad"],
            "items":[{"physical_id":"mac:bad","label":"x","mount_root":"/Volumes/x","system":false,"eligible":true}]
        }});
        assert!(parse_probe(&malformed).is_none());
    }

    #[test]
    fn win32_guid_and_serial_are_validated_as_one_identity() {
        const FIRST: &str = "win:{12345678-1234-abcd-abcd-1234567890ab}:00000001";
        const SECOND: &str = "win:{12345678-1234-abcd-abcd-1234567890ab}:1234abcd";
        fn response(ids: &[&str]) -> Value {
            let rows = ids
                .iter()
                .map(|id| {
                    json!({"physical_id":id,"label":"Generated volume",
                    "mount_root":"X:\\","system":false,"eligible":true})
                })
                .collect::<Vec<_>>();
            json!({"ok":true,"data":{
                "platform":"windows","probe_ok":true,"observed_ids":ids,"items":rows}})
        }

        let snapshot = parse_probe(&response(&[FIRST, SECOND])).unwrap();
        assert_eq!(snapshot.observed_ids, [FIRST, SECOND]);
        assert!(snapshot.items.iter().all(|volume| volume.eligible));
        for invalid in [
            "win:{12345678-1234-abcd-abcd-1234567890ab}",
            "win:{12345678-1234-abcd-abcd-1234567890ab}:1234abc",
            "win:{12345678-1234-abcd-abcd-1234567890ab}:1234abcd1",
            "win:{12345678-1234-abcd-abcd-1234567890ab}:1234abcz",
            "win:{12345678-1234-abcdabcd-1234567890ab}:1234abcd",
            "win:12345678-1234-abcd-abcd-1234567890ab:1234abcd",
            "win:X:\\:1234abcd",
        ] {
            assert!(
                parse_probe(&response(&[FIRST, invalid])).is_none(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn failed_probe_never_becomes_an_empty_success_or_offline_state() {
        let failed = json!({"ok":true,"data":{"platform":"macos","probe_ok":false,
            "observed_ids":[],"items":[],"error":"generic"}});
        assert!(parse_probe(&failed).is_none());
        let status = json!({"ok":true,"data":{"probe_ok":false,"items":[{"volume_id":7,"state":"unverified"}]}});
        assert!(parse_states(&status, &[7]).is_empty());
    }

    #[test]
    fn request_contains_only_private_sidecar_scope_and_confirmed_selection() {
        let scope = ShelfContext {
            user_store: "private-user-store.sqlite".into(),
            catalog_key: "sqlite:synthetic-catalog".into(),
            platform: "macos",
        };
        let snapshot = ProbeSnapshot {
            platform: "macos".into(),
            observed_ids: vec![uuid("aabbccdd-eeff-0011-2233-445566778899")],
            items: vec![],
        };
        let request = status_request(&scope, &[7], &snapshot);
        assert_eq!(request["op"], "physical_status");
        assert_eq!(request["user_store"], "private-user-store.sqlite");
        assert!(request.get("db").is_none());
        assert_eq!(request["probe_ok"], true);
        assert_eq!(request["volume_ids"], json!([7]));
    }
}
