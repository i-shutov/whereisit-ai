//! Read-only, metadata-only local folder analysis.
//!
//! Jobs scan into a private temporary SQLite snapshot so memory use stays bounded and
//! result pages remain stable while the source tree changes. No file contents are opened.
#[path = "local_analyzer_views.rs"]
mod views;
use atlas_core::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock, Weak,
    },
    time::Instant,
};
use unicode_normalization::UnicodeNormalization;

const MAX_JOBS: usize = 16;
const MAX_PAGE: u64 = 500;
const MAX_SELECTED: usize = 10_000;
const PREPARED: &str = "prepared";
const SCANNING: &str = "scanning";
const CHECKING: &str = "checking_catalog";
const COMPLETE: &str = "completed";
const CANCELLED: &str = "cancelled";
const FAILED: &str = "failed";
const MAX_SPENT_OPERATION_IDS: usize = 64;

struct Snapshot {
    _dir: tempfile::TempDir,
    db: PathBuf,
    root: PathBuf,
    root_identity: Arc<same_file::Handle>,
}
struct Job {
    cancel: AtomicBool,
    operation_cancel: AtomicBool,
    operation_active: AtomicBool,
    interrupt: Mutex<Vec<Weak<rusqlite::InterruptHandle>>>,
    inner: Mutex<JobState>,
    began: Instant,
}
struct JobState {
    phase: String,
    root_label: Option<String>,
    files: u64,
    dirs: u64,
    bytes: u64,
    errors: u64,
    skipped_links: u64,
    error_code: Option<String>,
    snapshot: Option<Arc<Snapshot>>,
    catalog_generation: Option<String>,
    catalog_binding: Option<(PathBuf, Option<i64>, String)>,
    operation_reservation: Option<OperationReservation>,
    spent_operation_ids: VecDeque<String>,
}
struct OperationReservation {
    id: String,
    started: bool,
}
impl Default for JobState {
    fn default() -> Self {
        Self {
            phase: PREPARED.into(),
            root_label: None,
            files: 0,
            dirs: 0,
            bytes: 0,
            errors: 0,
            skipped_links: 0,
            error_code: None,
            snapshot: None,
            catalog_generation: None,
            catalog_binding: None,
            operation_reservation: None,
            spent_operation_ids: VecDeque::new(),
        }
    }
}
static JOBS: OnceLock<Mutex<HashMap<String, Arc<Job>>>> = OnceLock::new();
static BASELINES: OnceLock<Mutex<HashMap<String, Arc<Baseline>>>> = OnceLock::new();
struct Baseline {
    root: PathBuf,
    root_identity: Arc<same_file::Handle>,
    _dir: tempfile::TempDir,
    db: PathBuf,
    saved_unix: i64,
}
fn jobs() -> &'static Mutex<HashMap<String, Arc<Job>>> {
    JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn baselines() -> &'static Mutex<HashMap<String, Arc<Baseline>>> {
    BASELINES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn err(code: &'static str, message: impl std::fmt::Display) -> Error {
    Error::new(code, message)
}
fn field<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| err("invalid_request", format!("{k} must be a string")))
}
fn job(v: &Value) -> Result<(String, Arc<Job>)> {
    let id = field(v, "job_id")?;
    if id.is_empty() || id.len() > 128 {
        return Err(err("invalid_request", "job_id must be 1..128 bytes"));
    }
    let j = jobs()
        .lock()
        .map_err(|_| err("internal", "analyzer registry poisoned"))?
        .get(id)
        .cloned()
        .ok_or_else(|| err("not_found", "analyzer job not found"))?;
    Ok((id.to_owned(), j))
}
fn set_phase(j: &Job, p: &str) {
    if let Ok(mut s) = j.inner.lock() {
        s.phase = p.into();
    }
}
fn canceled(j: &Job) -> bool {
    j.cancel.load(Ordering::Acquire) || j.operation_cancel.load(Ordering::Acquire)
}
fn check(j: &Job) -> Result<()> {
    if canceled(j) {
        Err(err("cancelled", "analysis cancelled"))
    } else {
        Ok(())
    }
}
fn operation_id(v: &Value) -> Result<Option<&str>> {
    match v.get("operation_id") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(id)) if !id.is_empty() && id.len() <= 128 => Ok(Some(id)),
        _ => Err(err("invalid_request", "operation_id must be 1..128 bytes")),
    }
}
fn begin_operation(j: &Job, v: &Value) -> Result<()> {
    check(j)?;
    let requested = operation_id(v)?;
    let mut s = j
        .inner
        .lock()
        .map_err(|_| err("internal", "analyzer job poisoned"))?;
    if s.phase != COMPLETE {
        return Err(err("not_ready", "analysis results are not ready"));
    }
    if j.operation_active.load(Ordering::Acquire) {
        return Err(err("busy", "analyzer operation is already active"));
    }
    if let Some(id) = requested {
        if s.spent_operation_ids.iter().any(|x| x == id) {
            return Err(err(
                "cancelled",
                "operation reservation has already been consumed",
            ));
        }
        match s.operation_reservation.as_mut() {
            Some(reservation) if reservation.id == id && !reservation.started => {
                reservation.started = true
            }
            _ => return Err(err("not_found", "operation was not prepared")),
        }
    } else if s.operation_reservation.is_some() {
        return Err(err("conflict", "prepared operation_id is required"));
    }
    j.operation_cancel.store(false, Ordering::Release);
    j.operation_active.store(true, Ordering::Release);
    Ok(())
}
fn finish_operation_locked(j: &Job, s: &mut JobState) -> bool {
    let interrupted = j.operation_cancel.swap(false, Ordering::AcqRel);
    if let Some(reservation) = s.operation_reservation.take() {
        remember_spent(s, reservation.id);
    }
    j.operation_active.store(false, Ordering::Release);
    interrupted
}
fn finish_operation(j: &Job) -> Result<bool> {
    let mut s = j
        .inner
        .lock()
        .map_err(|_| err("internal", "analyzer job poisoned"))?;
    Ok(finish_operation_locked(j, &mut s))
}
fn remember_spent(s: &mut JobState, id: String) {
    if s.spent_operation_ids.iter().any(|old| old == &id) {
        return;
    }
    s.spent_operation_ids.push_back(id);
    while s.spent_operation_ids.len() > MAX_SPENT_OPERATION_IDS {
        s.spent_operation_ids.pop_front();
    }
}
fn register_interrupt(j: &Job, c: &Connection) -> Result<Arc<rusqlite::InterruptHandle>> {
    let handle = Arc::new(c.get_interrupt_handle());
    let mut handles = j
        .interrupt
        .lock()
        .map_err(|_| err("internal", "interrupt lock poisoned"))?;
    handles.retain(|weak| weak.strong_count() > 0);
    handles.push(Arc::downgrade(&handle));
    Ok(handle)
}
fn catalog_stamp(path: &Path, volume: Option<i64>) -> Result<(PathBuf, String)> {
    let canonical = path.canonicalize().map_err(|e| err("invalid_path", e))?;
    fn stamp(path: &Path) -> Result<String> {
        match fs::metadata(path) {
            Ok(m) => {
                if m.len() == 0 {
                    return Ok("0:0".into());
                }
                let ns = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                Ok(format!("{}:{ns}", m.len()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("0:0".into()),
            Err(e) => Err(err("io", e)),
        }
    }
    let mut wal = canonical.as_os_str().to_os_string();
    wal.push("-wal");
    let identity = same_file::Handle::from_path(&canonical).map_err(|e| err("io", e))?;
    use std::hash::{Hash, Hasher};
    let mut handle_hash = std::collections::hash_map::DefaultHasher::new();
    identity.hash(&mut handle_hash);
    let canonical_text = canonical
        .to_str()
        .ok_or_else(|| err("unsupported_path", "catalog path is not Unicode"))?;
    let raw = format!(
        "{}|{}|{}|{}|{:?}",
        canonical_text,
        handle_hash.finish(),
        stamp(&canonical)?,
        stamp(Path::new(&wal))?,
        volume
    );
    let digest = Sha256::digest(raw.as_bytes());
    let generation = digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok((canonical, generation))
}

/// Ensure a catalog-derived view still refers to the same read-only catalog snapshot.
fn validate_catalog_binding(j: &Job) -> Result<()> {
    let (path, volume, expected, snapshot) = {
        let s = j
            .inner
            .lock()
            .map_err(|_| err("internal", "analyzer job poisoned"))?;
        let (path, volume, expected) = s
            .catalog_binding
            .clone()
            .ok_or_else(|| err("not_ready", "catalog comparison has not completed"))?;
        (path, volume, expected, s.snapshot.clone())
    };
    let actual = catalog_stamp(&path, volume)
        .map(|x| x.1)
        .unwrap_or_default();
    if actual == expected {
        return Ok(());
    }
    if let Some(snapshot) = snapshot {
        let c = Connection::open(&snapshot.db)?;
        c.execute("DELETE FROM matches", [])?;
    }
    let mut s = j
        .inner
        .lock()
        .map_err(|_| err("internal", "analyzer job poisoned"))?;
    s.catalog_generation = None;
    s.catalog_binding = None;
    s.error_code = Some("stale_generation".into());
    Err(err(
        "stale_generation",
        "catalog changed since the comparison",
    ))
}

pub(crate) fn prepare(v: &Value) -> Result<Value> {
    let id = field(v, "job_id")?;
    if id.is_empty() || id.len() > 128 {
        return Err(err("invalid_request", "job_id must be 1..128 bytes"));
    }
    let mut m = jobs()
        .lock()
        .map_err(|_| err("internal", "analyzer registry poisoned"))?;
    if m.contains_key(id) {
        return Err(err("conflict", "job_id is already reserved"));
    }
    if m.len() >= MAX_JOBS {
        return Err(err(
            "busy",
            "analyzer job limit reached; close a completed job",
        ));
    }
    m.insert(
        id.into(),
        Arc::new(Job {
            cancel: AtomicBool::new(false),
            operation_cancel: AtomicBool::new(false),
            operation_active: AtomicBool::new(false),
            interrupt: Mutex::new(Vec::new()),
            inner: Mutex::new(JobState::default()),
            began: Instant::now(),
        }),
    );
    Ok(json!({"job_id":id,"phase":PREPARED}))
}

pub(crate) fn prepare_operation(v: &Value) -> Result<Value> {
    let (id, j) = job(v)?;
    let op_id =
        operation_id(v)?.ok_or_else(|| err("invalid_request", "operation_id must be provided"))?;
    let mut s = j
        .inner
        .lock()
        .map_err(|_| err("internal", "analyzer job poisoned"))?;
    if s.phase != COMPLETE {
        return Err(err("not_ready", "analysis results are not ready"));
    }
    if j.operation_active.load(Ordering::Acquire) || s.operation_reservation.is_some() {
        return Err(err(
            "busy",
            "analyzer operation is already active or reserved",
        ));
    }
    if s.spent_operation_ids.iter().any(|x| x == op_id) {
        return Err(err(
            "conflict",
            "operation_id is one-shot and has already been used",
        ));
    }
    s.operation_reservation = Some(OperationReservation {
        id: op_id.to_owned(),
        started: false,
    });
    j.operation_cancel.store(false, Ordering::Release);
    Ok(json!({"job_id":id,"operation_id":op_id,"reserved":true}))
}

pub(crate) fn start(v: &Value) -> Result<Value> {
    let (id, j) = job(v)?;
    let root = PathBuf::from(field(v, "root")?);
    if !root.is_absolute() {
        return Err(err("invalid_path", "root must be an absolute path"));
    }
    {
        let mut s = j
            .inner
            .lock()
            .map_err(|_| err("internal", "analyzer job poisoned"))?;
        if s.phase != PREPARED {
            return Err(err("conflict", "job is not prepared"));
        }
        s.phase = SCANNING.into();
    }
    let clone = j.clone();
    std::thread::Builder::new()
        .name("atlas-local-analyzer".into())
        .spawn(move || {
            if let Err(e) = scan(&clone, root) {
                if canceled(&clone) {
                    set_phase(&clone, CANCELLED);
                } else if let Ok(mut s) = clone.inner.lock() {
                    s.error_code = Some(e.code.to_owned());
                    s.phase = FAILED.into();
                }
            }
        })
        .map_err(|e| {
            set_phase(&j, PREPARED);
            err("worker", e)
        })?;
    Ok(json!({"job_id":id,"phase":SCANNING}))
}

fn validate_root(root: &Path) -> Result<PathBuf> {
    let canonical = root.canonicalize().map_err(|e| err("invalid_path", e))?;
    // Reject a symlink/reparse point in every path component. canonicalize alone would hide it.
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        return Err(err("invalid_path", "root must be absolute"));
    };
    let mut cur = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::Prefix(p) => cur.push(p.as_os_str()),
            Component::RootDir => cur.push(part.as_os_str()),
            Component::Normal(x) => cur.push(x),
            Component::CurDir => {}
            Component::ParentDir => return Err(err("invalid_path", "root may not contain '..'")),
        }
        if let Ok(m) = fs::symlink_metadata(&cur) {
            if is_link_or_reparse(&m) {
                return Err(err(
                    "unsafe_path",
                    "root or one of its ancestors is a symbolic link or reparse point",
                ));
            }
        }
    }
    let m = fs::metadata(&canonical)?;
    if !m.is_dir() {
        return Err(err("invalid_path", "root must be a directory"));
    }
    if canonical.to_str().is_none() {
        return Err(err("unsupported_path", "root path is not Unicode"));
    }
    Ok(canonical)
}
#[cfg(windows)]
fn is_link_or_reparse(m: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    m.file_type().is_symlink() || m.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_link_or_reparse(m: &fs::Metadata) -> bool {
    m.file_type().is_symlink()
}

fn scan(j: &Arc<Job>, root: PathBuf) -> Result<()> {
    check(j)?;
    let root = validate_root(&root)?;
    let root_identity = Arc::new(same_file::Handle::from_path(&root).map_err(|e| err("io", e))?);
    let temp = tempfile::Builder::new()
        .prefix("atlas-analyzer-")
        .tempdir()
        .map_err(|e| err("temp", e))?;
    let db = temp.path().join("snapshot.sqlite");
    let mut c = Connection::open(&db)?;
    c.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA temp_store=FILE; CREATE TABLE folders(id INTEGER PRIMARY KEY,parent_id INTEGER,name TEXT NOT NULL,path TEXT NOT NULL,depth INTEGER NOT NULL,bytes INTEGER NOT NULL DEFAULT 0,files INTEGER NOT NULL DEFAULT 0,dirs INTEGER NOT NULL DEFAULT 0,mtime INTEGER); CREATE TABLE files(id INTEGER PRIMARY KEY,folder_id INTEGER NOT NULL,name TEXT NOT NULL,path TEXT NOT NULL,norm_name TEXT NOT NULL,ext TEXT NOT NULL,size INTEGER NOT NULL,mtime INTEGER); CREATE INDEX files_size ON files(size DESC,id); CREATE INDEX files_norm ON files(norm_name,size); CREATE INDEX folders_parent ON folders(parent_id); CREATE TABLE errors(path TEXT NOT NULL,reason TEXT NOT NULL);")?;
    let name = root
        .file_name()
        .and_then(|x| x.to_str())
        .or_else(|| root.to_str())
        .ok_or_else(|| err("unsupported_path", "root name is not Unicode"))?;
    c.execute(
        "INSERT INTO folders(parent_id,name,path,depth) VALUES(NULL,?1,'',0)",
        [name],
    )?;
    let root_id = c.last_insert_rowid();
    let _interrupt = register_interrupt(j, &c)?;
    let mut files = 0u64;
    let mut dirs = 0u64;
    let mut bytes = 0u64;
    let mut errors = 0u64;
    let mut links = 0u64;
    let mut visited = 0u64;
    c.execute_batch("CREATE TABLE pending(folder_id INTEGER PRIMARY KEY,absolute_path TEXT NOT NULL,relative_path TEXT NOT NULL,depth INTEGER NOT NULL);")?;
    c.execute(
        "INSERT INTO pending VALUES(?1,?2,'',0)",
        params![
            root_id,
            root.to_str()
                .ok_or_else(|| err("unsupported_path", "root path is not Unicode"))?
        ],
    )?;
    let tx = c.transaction()?;
    loop {
        check(j)?;
        let next:Option<(i64,String,String,u32)>=tx.query_row("SELECT folder_id,absolute_path,relative_path,depth FROM pending ORDER BY folder_id LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let Some((folder_id, absolute, rel_dir, depth)) = next else {
            break;
        };
        tx.execute("DELETE FROM pending WHERE folder_id=?1", [folder_id])?;
        let dir = PathBuf::from(absolute);
        let queued_meta = match fs::symlink_metadata(&dir) {
            Ok(m) => m,
            Err(e) => {
                errors += 1;
                tx.execute(
                    "INSERT INTO errors(path,reason) VALUES(?1,?2)",
                    params![rel_dir, format!("queued_metadata:{:?}", e.kind())],
                )?;
                continue;
            }
        };
        if is_link_or_reparse(&queued_meta) {
            links += 1;
            continue;
        }
        if !queued_meta.is_dir() {
            errors += 1;
            tx.execute(
                "INSERT INTO errors(path,reason) VALUES(?1,'queued_path_not_directory')",
                [rel_dir],
            )?;
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(x) => x,
            Err(e) => {
                errors += 1;
                tx.execute(
                    "INSERT INTO errors(path,reason) VALUES(?1,?2)",
                    params![rel_dir, format!("read_dir:{:?}", e.kind())],
                )?;
                continue;
            }
        };
        for item in entries {
            check(j)?;
            visited = visited.saturating_add(1);
            let item = match item {
                Ok(x) => x,
                Err(e) => {
                    errors += 1;
                    tx.execute(
                        "INSERT INTO errors(path,reason) VALUES(?1,?2)",
                        params![rel_dir, format!("entry:{:?}", e.kind())],
                    )?;
                    continue;
                }
            };
            let path = item.path();
            let meta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    errors += 1;
                    tx.execute(
                        "INSERT INTO errors(path,reason) VALUES(?1,?2)",
                        params![rel_dir, format!("metadata:{:?}", e.kind())],
                    )?;
                    continue;
                }
            };
            if is_link_or_reparse(&meta) {
                links += 1;
                continue;
            }
            let file_name = match item.file_name().into_string() {
                Ok(s) => s,
                Err(_) => {
                    errors += 1;
                    tx.execute(
                        "INSERT INTO errors(path,reason) VALUES(?1,'non_unicode_path')",
                        [rel_dir.clone()],
                    )?;
                    continue;
                }
            };
            let relative = if rel_dir.is_empty() {
                file_name.clone()
            } else {
                format!("{rel_dir}/{file_name}")
            };
            if meta.is_dir() {
                let mtime = modified_unix(&meta);
                tx.execute(
                    "INSERT INTO folders(parent_id,name,path,depth,mtime) VALUES(?1,?2,?3,?4,?5)",
                    params![folder_id, file_name, relative, depth + 1, mtime],
                )?;
                let child_id = tx.last_insert_rowid();
                dirs += 1;
                // Non-Unicode absolute paths are rejected at root validation; relative names are strict.
                tx.execute(
                    "INSERT INTO pending VALUES(?1,?2,?3,?4)",
                    params![
                        child_id,
                        path.to_str()
                            .ok_or_else(|| err("unsupported_path", "path is not Unicode"))?,
                        relative,
                        depth + 1
                    ],
                )?;
            } else if meta.is_file() {
                let size = meta.len();
                let mtime = modified_unix(&meta);
                let norm = normalize_name(&file_name);
                let ext = Path::new(&file_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                tx.execute("INSERT INTO files(folder_id,name,path,norm_name,ext,size,mtime) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![folder_id,file_name,relative,norm,ext,size.min(i64::MAX as u64) as i64,mtime])?;
                tx.execute(
                    "UPDATE folders SET bytes=bytes+?1,files=files+1 WHERE id=?2",
                    params![size.min(i64::MAX as u64) as i64, folder_id],
                )?;
                files += 1;
                bytes = bytes.saturating_add(size);
            } else {
                errors += 1;
                tx.execute(
                    "INSERT INTO errors(path,reason) VALUES(?1,'unsupported_file_type')",
                    [relative],
                )?;
            }
            if visited.is_multiple_of(128) {
                if let Ok(mut s) = j.inner.lock() {
                    s.files = files;
                    s.dirs = dirs;
                    s.bytes = bytes;
                    s.errors = errors;
                    s.skipped_links = links;
                }
            }
        }
        if let Ok(mut s) = j.inner.lock() {
            s.files = files;
            s.dirs = dirs;
            s.bytes = bytes;
            s.errors = errors;
            s.skipped_links = links;
        }
    }
    if let Ok(mut s) = j.inner.lock() {
        s.files = files;
        s.dirs = dirs;
        s.bytes = bytes;
        s.errors = errors;
        s.skipped_links = links;
    }
    // Aggregate descendants bottom-up; every file is counted once and empty dirs remain.
    let max_depth: i64 = tx.query_row("SELECT COALESCE(MAX(depth),0) FROM folders", [], |r| {
        r.get(0)
    })?;
    for depth in (1..=max_depth).rev() {
        tx.execute("UPDATE folders AS p SET bytes=bytes+COALESCE((SELECT SUM(c.bytes) FROM folders c WHERE c.parent_id=p.id),0),files=files+COALESCE((SELECT SUM(c.files) FROM folders c WHERE c.parent_id=p.id),0),dirs=dirs+COALESCE((SELECT SUM(1+c.dirs) FROM folders c WHERE c.parent_id=p.id),0) WHERE p.depth=?1-1",[depth])?;
    }
    tx.commit()?;
    check(j)?;
    let current = Arc::new(same_file::Handle::from_path(&root).map_err(|e| err("io", e))?);
    if *root_identity != *current {
        return Err(err("stale_root", "scan root was replaced while scanning"));
    }
    let mut s = j
        .inner
        .lock()
        .map_err(|_| err("internal", "analyzer job poisoned"))?;
    check(j)?;
    s.root_label = root
        .file_name()
        .and_then(|x| x.to_str())
        .or_else(|| root.to_str())
        .map(str::to_owned);
    s.snapshot = Some(Arc::new(Snapshot {
        _dir: temp,
        db,
        root,
        root_identity,
    }));
    s.phase = COMPLETE.into();
    Ok(())
}

fn normalize_name(s: &str) -> String {
    s.trim().nfc().collect::<String>().to_lowercase()
}
fn modified_unix(meta: &fs::Metadata) -> Option<i64> {
    let t = meta.modified().ok()?;
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => Some(d.as_secs().min(i64::MAX as u64) as i64),
        Err(e) => {
            let d = e.duration();
            let seconds = d
                .as_secs()
                .saturating_add(u64::from(d.subsec_nanos() != 0))
                .min(i64::MAX as u64);
            Some(-(seconds as i64))
        }
    }
}

pub(crate) fn progress(v: &Value) -> Result<Value> {
    let (id, j) = job(v)?;
    let s = j
        .inner
        .lock()
        .map_err(|_| err("internal", "analyzer job poisoned"))?;
    Ok(
        json!({"job_id":id,"phase":s.phase,"root_label":s.root_label,"files_seen":s.files,"dirs_seen":s.dirs,"logical_bytes":s.bytes,"read_errors":s.errors,"symlinks_skipped":s.skipped_links,"elapsed_ms":j.began.elapsed().as_millis(),"cancelled":canceled(&j),"operation_reserved":s.operation_reservation.is_some(),"operation_active":j.operation_active.load(Ordering::Acquire),"results_ready":s.snapshot.is_some(),"error_code":s.error_code}),
    )
}
pub(crate) fn cancel(v: &Value) -> Result<Value> {
    let (id, j) = job(v)?;
    let requested = operation_id(v)?;
    let (scope, pending, handles) = {
        let mut s = j
            .inner
            .lock()
            .map_err(|_| err("internal", "analyzer job poisoned"))?;
        let (scope, pending) = match s.phase.as_str() {
            PREPARED | SCANNING => {
                if requested.is_some() {
                    return Err(err(
                        "conflict",
                        "scan cancellation does not take operation_id",
                    ));
                }
                j.cancel.store(true, Ordering::Release);
                s.phase = CANCELLED.into();
                ("scan", false)
            }
            CHECKING | COMPLETE if j.operation_active.load(Ordering::Acquire) => {
                if let Some(id) = requested {
                    if !s
                        .operation_reservation
                        .as_ref()
                        .is_some_and(|r| r.id == id && r.started)
                    {
                        return Err(err(
                            "conflict",
                            "operation_id does not match the active operation",
                        ));
                    }
                }
                j.operation_cancel.store(true, Ordering::Release);
                ("operation", false)
            }
            COMPLETE if s.operation_reservation.as_ref().is_some_and(|r| !r.started) => {
                if let Some(id) = requested {
                    if !s
                        .operation_reservation
                        .as_ref()
                        .is_some_and(|r| r.id == id && !r.started)
                    {
                        return Err(err(
                            "conflict",
                            "operation_id does not match the reserved operation",
                        ));
                    }
                }
                let reservation = s.operation_reservation.take().expect("checked reservation");
                remember_spent(&mut s, reservation.id);
                j.operation_cancel.store(false, Ordering::Release);
                j.operation_active.store(false, Ordering::Release);
                ("operation", true)
            }
            _ => return Err(err("conflict", "no analyzer operation is active")),
        };
        let handles = if pending {
            Vec::new()
        } else {
            j.interrupt
                .lock()
                .ok()
                .map(|list| list.iter().filter_map(Weak::upgrade).collect())
                .unwrap_or_default()
        };
        (scope, pending, handles)
    };
    for handle in handles {
        handle.interrupt();
    }
    Ok(json!({"job_id":id,"cancelled":true,"scope":scope,"pending":pending}))
}
pub(crate) fn close(v: &Value) -> Result<Value> {
    let (id, j) = job(v)?;
    j.cancel.store(true, Ordering::Release);
    if let Ok(handles) = j.interrupt.lock() {
        for weak in handles.iter() {
            if let Some(h) = weak.upgrade() {
                h.interrupt();
            }
        }
    }
    jobs()
        .lock()
        .map_err(|_| err("internal", "analyzer registry poisoned"))?
        .remove(&id);
    Ok(json!({"job_id":id,"closed":true}))
}

pub(crate) fn page(v: &Value) -> Result<Value> {
    let (_, j) = job(v)?;
    begin_operation(&j, v)?;
    let result = views::page(v);
    let interrupted = finish_operation(&j)?;
    if interrupted {
        Err(err("cancelled", "analyzer page cancelled"))
    } else {
        result
    }
}

fn parse_folder_id(s: &str) -> Result<i64> {
    s.strip_prefix("folder:")
        .and_then(|n| n.parse().ok())
        .filter(|n| *n > 0)
        .ok_or_else(|| {
            err(
                "invalid_request",
                "folder_id must be root or folder:<positive id>",
            )
        })
}

pub(crate) fn check_catalog(v: &Value) -> Result<Value> {
    let (id, j) = job(v)?;
    let db = field(v, "db")?.to_owned();
    let volume = match v.get("volume_id") {
        None | Some(Value::Null) => None,
        Some(x) => Some(x.as_i64().filter(|n| *n > 0).ok_or_else(|| {
            err(
                "invalid_request",
                "volume_id must be a positive integer or null",
            )
        })?),
    };
    let expected = match v.get("expected_generation") {
        None | Some(Value::Null) => None,
        Some(Value::String(x)) => Some(x.clone()),
        _ => {
            return Err(err(
                "invalid_request",
                "expected_generation must be a string or null",
            ));
        }
    };
    begin_operation(&j, v)?;
    let snap = {
        let mut s = j
            .inner
            .lock()
            .map_err(|_| err("internal", "analyzer job poisoned"))?;
        if s.phase != COMPLETE {
            return Err(err("not_ready", "scan must complete before catalog check"));
        }
        let snapshot = s
            .snapshot
            .clone()
            .ok_or_else(|| err("not_ready", "snapshot unavailable"))?;
        s.phase = CHECKING.into();
        s.error_code = None;
        s.catalog_generation = None;
        s.catalog_binding = None;
        snapshot
    };
    let clone = j.clone();
    std::thread::Builder::new().name("atlas-analyzer-catalog".into()).spawn(move||{
        let result=(||->Result<(PathBuf,String)>{
            let local=Connection::open(&snap.db)?;
            let _local_interrupt=register_interrupt(&clone,&local)?;
            local.execute_batch("CREATE TABLE IF NOT EXISTS matches(file_id INTEGER NOT NULL,volume_id INTEGER NOT NULL,entry_id INTEGER NOT NULL,PRIMARY KEY(file_id,volume_id,entry_id)); DELETE FROM matches;")?;
            let (canonical,generation)=catalog_stamp(Path::new(&db),volume)?;
            if expected.as_ref().is_some_and(|x|x!=&generation) { return Err(err("stale_generation","catalog changed since the comparison")); }
            let remote=atlas_core::Catalog::open_readonly(&db)?;
            remote.conn().execute_batch("BEGIN DEFERRED")?;
            let _remote_interrupt=register_interrupt(&clone,remote.conn())?;
            check(&clone)?;
            if let Some(wanted)=volume { if !remote.volumes()?.iter().any(|x|x.id==wanted) { return Err(err("not_found","catalog volume was not found")); } }
            check(&clone)?;
            let mut stmt=remote.conn().prepare("SELECT id,volume_id,name,size FROM entries_core WHERE kind='file' AND (?1 IS NULL OR volume_id=?1) ORDER BY id")?;
            let rows=stmt.query_map([volume],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?)))?;
            let tx=local.unchecked_transaction()?;
            let mut select=tx.prepare_cached("SELECT id FROM files WHERE norm_name=?1 AND size=?2")?;
            let mut insert=tx.prepare_cached("INSERT OR IGNORE INTO matches(file_id,volume_id,entry_id) VALUES(?1,?2,?3)")?;
            for row in rows { check(&clone)?; let(entry,vol,name,size)=row?; let matches=select.query_map(params![normalize_name(&name),size],|r|r.get::<_,i64>(0))?; for file in matches { check(&clone)?; insert.execute(params![file?,vol,entry])?; } }
            drop(insert);drop(select);drop(stmt);
            check(&clone)?;
            let post_generation=catalog_stamp(&canonical,volume)?.1;
            if post_generation!=generation {return Err(err("stale_generation","catalog changed during comparison"));}
            remote.conn().execute_batch("COMMIT")?; tx.commit()?; Ok((canonical,generation))
        })();
        if let Ok(mut s)=clone.inner.lock(){let operation_cancelled=finish_operation_locked(&clone,&mut s);match result{Ok((path,g)) if !operation_cancelled&&!clone.cancel.load(Ordering::Acquire)=>{s.catalog_generation=Some(g.clone());s.catalog_binding=Some((path,volume,g));s.phase=COMPLETE.into();},Ok(_)=>{s.error_code=Some("cancelled".into());s.phase=if clone.cancel.load(Ordering::Acquire){CANCELLED.into()}else{COMPLETE.into()};s.catalog_generation=None;s.catalog_binding=None;},Err(e)=>{let was_cancelled=operation_cancelled||clone.cancel.load(Ordering::Acquire);s.error_code=Some(if was_cancelled{"cancelled".into()}else{e.code.to_owned()});s.phase=if clone.cancel.load(Ordering::Acquire){CANCELLED.into()}else{COMPLETE.into()};s.catalog_generation=None;s.catalog_binding=None;}}}
    }).map_err(|e|{let _=finish_operation(&j);set_phase(&j,COMPLETE);err("worker",e)})?;
    Ok(json!({"job_id":id,"phase":CHECKING}))
}
pub(crate) fn save_baseline(v: &Value) -> Result<Value> {
    let (_, j) = job(v)?;
    begin_operation(&j, v)?;
    let result = views::save_baseline(v);
    let interrupted = finish_operation(&j)?;
    if interrupted {
        Err(err("cancelled", "baseline save cancelled"))
    } else {
        result
    }
}
pub(crate) fn clear_baseline(v: &Value) -> Result<Value> {
    let key = field(v, "baseline_id")?;
    let removed = baselines()
        .lock()
        .map_err(|_| err("internal", "baseline registry poisoned"))?
        .remove(key);
    let existed = removed.is_some();
    drop(removed);
    Ok(json!({"baseline_id":key,"cleared":existed}))
}
pub(crate) fn export_csv(v: &Value) -> Result<Value> {
    let (_, j) = job(v)?;
    begin_operation(&j, v)?;
    let result = views::export_csv(v);
    let interrupted = finish_operation(&j)?;
    if interrupted {
        Err(err("cancelled", "CSV export cancelled"))
    } else {
        result
    }
}

#[cfg(test)]
#[path = "local_analyzer_tests.rs"]
mod tests;
