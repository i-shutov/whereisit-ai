//! On-demand read-only CTF mirror. The original file is authoritative.
//! A disposable SQLite v3 generation is keyed by source bytes and parser recipe.
use crate::{ctf_sidecar, legacy, maintenance, optional_id, text, unsigned};
use atlas_core::{Catalog, Error, Result};
use ctf_sidecar::{Recipe, SourceIdentity};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const CTF_LIMIT: u64 = 2_000_000_000;
const DB_LIMIT: u64 = 64_000_000_000;
const MANIFEST_LIMIT: u64 = 65_536;
const FORMAT: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    source: SourceIdentity,
    recipe: Recipe,
    sqlite: String,
    db_sha256: String,
    db_bytes: u64,
    entries: u64,
    volumes: u64,
    readonly: bool,
}
struct Generation {
    root: PathBuf,
    key: String,
    source: SourceIdentity,
    manifest: Manifest,
    built: bool,
}

const PIN_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const MAX_PINNED_GENERATIONS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DbStamp {
    bytes: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    inode: u64,
}
#[derive(Clone)]
struct Pinned {
    source: SourceIdentity,
    stamp: DbStamp,
    checked_at: Instant,
}
type PinKey = (PathBuf, String);
static PINS: OnceLock<Mutex<HashMap<PinKey, Pinned>>> = OnceLock::new();
fn pins() -> &'static Mutex<HashMap<PinKey, Pinned>> {
    PINS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn stamp(path: &Path) -> Result<DbStamp> {
    let meta = fs::symlink_metadata(path).map_err(io_error)?;
    if !meta.file_type().is_file() || meta.len() > DB_LIMIT || !meta.permissions().readonly() {
        return Err(failure(
            "cache_changed",
            "Pinned SQLite artifact is absent, changed or writable",
        ));
    }
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Ok(DbStamp {
        bytes: meta.len(),
        modified: meta.modified().ok(),
        created: meta.created().ok(),
        #[cfg(unix)]
        dev: meta.dev(),
        #[cfg(unix)]
        inode: meta.ino(),
    })
}
fn remember(g: &Generation) -> Result<()> {
    let key = (g.root.clone(), g.key.clone());
    let db = g.root.join(&g.key).join("catalog.sqlite");
    let stamped = stamp(&db)?;
    let mut entries = pins()
        .lock()
        .map_err(|_| failure("internal", "Pinned registry is unavailable"))?;
    if entries.len() >= MAX_PINNED_GENERATIONS && !entries.contains_key(&key) {
        let evict = entries
            .iter()
            .min_by_key(|(_, p)| p.checked_at)
            .map(|(key, _)| key.clone());
        if let Some(evict) = evict {
            entries.remove(&evict);
        }
    }
    entries.insert(
        key,
        Pinned {
            source: g.source.clone(),
            stamp: stamped,
            checked_at: Instant::now(),
        },
    );
    Ok(())
}
fn forget(root: &Path, key: &str) {
    if let Ok(mut registry) = pins().lock() {
        registry.remove(&(root.to_path_buf(), key.to_owned()));
    }
}

fn failure(code: &'static str, msg: &'static str) -> Error {
    Error::new(code, msg)
}
fn io_error(e: std::io::Error) -> Error {
    Error::new("io", format!("CTF mirror I/O error: {:?}", e.kind()))
}
fn path(v: &Value, field: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(text(v, field)?))
}
fn identity(path: &Path) -> Result<SourceIdentity> {
    ctf_sidecar::identity(path, CTF_LIMIT).map_err(io_error)
}
fn recipe() -> Recipe {
    Recipe {
        parser_version: format!("ctf-format-{}", env!("CARGO_PKG_VERSION")),
        index_format_version: 3,
        normalization_version: "rust-unicode-lower-v1".into(),
        sort_version: "sqlite-nocase-id-v1".into(),
        index_kind: "read-only-ctf-sqlite-v3-r1".into(),
        options_sha256: ctf_sidecar::sha256(b"no-plugins;no-native-annotations"),
        overlay_sha256: None,
    }
}
fn hash_file(path: &Path) -> Result<SourceIdentity> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.file_type().is_file() || metadata.len() > DB_LIMIT {
        return Err(failure(
            "cache_corrupt",
            "Missing, linked or oversized SQLite mirror",
        ));
    }
    let mut stream = File::open(path).map_err(io_error)?;
    let mut h = Sha256::new();
    let mut bytes = 0u64;
    let mut block = [0u8; 262_144];
    loop {
        let n = stream.read(&mut block).map_err(io_error)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .ok_or_else(|| failure("cache_corrupt", "SQLite mirror size overflow"))?;
        if bytes > DB_LIMIT {
            return Err(failure("cache_corrupt", "SQLite mirror exceeds limit"));
        }
        h.update(&block[..n]);
    }
    Ok(SourceIdentity {
        sha256: format!("{:x}", h.finalize()),
        size_bytes: bytes,
    })
}
fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    File::open(path)
        .map_err(io_error)?
        .sync_all()
        .map_err(io_error)?;
    #[cfg(not(target_os = "linux"))]
    let _ = path;
    Ok(())
}
fn verify(target: &Path, source: &SourceIdentity, recipe: &Recipe) -> Result<Manifest> {
    if !fs::symlink_metadata(target)
        .map_err(io_error)?
        .file_type()
        .is_dir()
    {
        return Err(failure(
            "cache_corrupt",
            "Invalid mirror generation directory",
        ));
    }
    let manifest_file = target.join("manifest.json");
    let metadata = fs::symlink_metadata(&manifest_file).map_err(io_error)?;
    if !metadata.file_type().is_file() || metadata.len() > MANIFEST_LIMIT {
        return Err(failure("cache_corrupt", "Incomplete mirror manifest"));
    }
    let raw = fs::read(manifest_file).map_err(io_error)?;
    let manifest: Manifest = serde_json::from_slice(&raw)
        .map_err(|_| failure("cache_corrupt", "Malformed mirror manifest"))?;
    if manifest.version != FORMAT
        || manifest.source != *source
        || manifest.recipe != *recipe
        || manifest.sqlite != rusqlite::version()
        || !manifest.readonly
    {
        return Err(failure("cache_stale", "Mirror recipe or source mismatch"));
    }
    let actual = hash_file(&target.join("catalog.sqlite"))?;
    if actual.sha256 != manifest.db_sha256 || actual.size_bytes != manifest.db_bytes {
        return Err(failure(
            "cache_corrupt",
            "SQLite mirror changed after publication",
        ));
    }
    Ok(manifest)
}
fn ensure(v: &Value) -> Result<Generation> {
    let source_path = path(v, "path")?;
    let metadata = fs::symlink_metadata(&source_path).map_err(io_error)?;
    if !metadata.file_type().is_file()
        || metadata.len() > CTF_LIMIT
        || source_path
            .extension()
            .and_then(|x| x.to_str())
            .is_none_or(|x| !x.eq_ignore_ascii_case("ctf"))
    {
        return Err(failure(
            "ctf_limit",
            "Expected an unlinked regular .CTF file",
        ));
    }
    let source = identity(&source_path)?;
    let recipe = recipe();
    let key = ctf_sidecar::cache_key(&source, &recipe).map_err(io_error)?;
    let root = path(v, "cache_root")?;
    if root.exists() {
        if !fs::symlink_metadata(&root)
            .map_err(io_error)?
            .file_type()
            .is_dir()
        {
            return Err(failure("cache_root", "Cache root must be a real directory"));
        }
    } else {
        fs::create_dir_all(&root).map_err(io_error)?;
    }
    let target = root.join(&key);
    if target.exists() {
        let manifest = verify(&target, &source, &recipe)?;
        if identity(&source_path)? != source {
            return Err(failure(
                "source_changed",
                "CTF changed while opening cached generation",
            ));
        }
        return Ok(Generation {
            root,
            key,
            source,
            manifest,
            built: false,
        });
    }
    let temp = tempfile::Builder::new()
        .prefix("ctf-building-")
        .tempdir_in(&root)
        .map_err(io_error)?;
    let db_path = temp.path().join("catalog.sqlite");
    let mut db = Catalog::open(&db_path)?;
    let imported = legacy::import(
        &mut db,
        &json!({"path":source_path.to_string_lossy().to_string()}),
    )?;
    let entries = imported["entries"]
        .as_u64()
        .ok_or_else(|| failure("ctf", "Importer did not report entries"))?;
    let volumes = imported["volumes"]
        .as_array()
        .map(|a| a.len() as u64)
        .ok_or_else(|| failure("ctf", "Importer did not report volumes"))?;
    // Remove local locator from a content-addressed derivative so byte-identical
    // renamed source copies can safely share the immutable mirror.
    db.conn().execute(
        "UPDATE ctf_sources SET source_path='' WHERE source_path<>''",
        [],
    )?;
    // Capture time belongs to a scan, not to the source CTF. Ensure two
    // independently built mirrors of identical CTF bytes have the same
    // non-user metadata, regardless of their build timestamp.
    db.conn()
        .execute("UPDATE volumes SET scanned_at=0 WHERE scanned_at<>0", [])?;
    db.conn().execute(
        "INSERT INTO entries_trigram(entries_trigram,rank) VALUES('integrity-check',1)",
        [],
    )?;
    db.conn().execute_batch(
        "PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;",
    )?;
    drop(db);
    let checked = maintenance::validate(&json!({"db":db_path.to_string_lossy()}))?;
    if checked["schema_version"] != 3
        || checked["entries"] != entries
        || checked["volumes"] != volumes
        || checked["quick_check"] != "ok"
        || checked["foreign_key_errors"] != 0
    {
        return Err(failure("integrity", "Mirror validation failed"));
    }
    if identity(&source_path)? != source {
        return Err(failure("source_changed", "CTF changed during mirror build"));
    }
    let mut perms = fs::metadata(&db_path).map_err(io_error)?.permissions();
    perms.set_readonly(true);
    fs::set_permissions(&db_path, perms).map_err(io_error)?;
    let image = hash_file(&db_path)?;
    let manifest = Manifest {
        version: FORMAT,
        source: source.clone(),
        recipe: recipe.clone(),
        sqlite: rusqlite::version().to_owned(),
        db_sha256: image.sha256,
        db_bytes: image.size_bytes,
        entries,
        volumes,
        readonly: true,
    };
    let encoded = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| failure("cache_corrupt", "Cannot encode mirror manifest"))?;
    if encoded.len() as u64 > MANIFEST_LIMIT {
        return Err(failure("cache_corrupt", "Mirror manifest exceeds limit"));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp.path().join("manifest.json"))
        .map_err(io_error)?;
    file.write_all(&encoded).map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    drop(file);
    sync_dir(temp.path())?;
    if let Err(error) = fs::rename(temp.path(), &target) {
        if !target.exists() {
            return Err(io_error(error));
        }
        // A concurrently built generation is usable only if its manifest and
        // derived bytes are identical. Never delete a conflicting generation.
        let other = verify(&target, &source, &recipe)?;
        if other.db_sha256 != manifest.db_sha256 {
            return Err(failure(
                "cache_conflict",
                "Different concurrent mirror generation",
            ));
        }
        if identity(&source_path)? != source {
            return Err(failure(
                "source_changed",
                "CTF changed during concurrent publication",
            ));
        }
        return Ok(Generation {
            root,
            key,
            source,
            manifest: other,
            built: false,
        });
    }
    sync_dir(&root)?;
    if identity(&source_path)? != source {
        return Err(failure(
            "source_changed",
            "CTF changed after mirror publication",
        ));
    }
    Ok(Generation {
        root,
        key,
        source,
        manifest,
        built: true,
    })
}
pub fn sync(v: &Value) -> Result<Value> {
    let generation = ensure(v)?;
    remember(&generation)?;
    Ok(json!({
        "state":"ready",
        "generation":generation.key,
        "source_sha256":generation.source.sha256,
        "source_bytes":generation.source.size_bytes,
        "entries":generation.manifest.entries,
        "volumes":generation.manifest.volumes,
        "rebuilt":generation.built,
        "original_ctf_read_only":true,
        "native_annotation_writes_allowed":false,
        "derived_database_disposable":true
    }))
}
pub fn search(v: &Value) -> Result<Value> {
    let query = text(v, "query")?;
    // Reject malformed queries before potentially multi-minute imports.
    atlas_core::query::compile(query)?;
    let offset = unsigned(v, "offset", 0)?;
    let limit = unsigned(v, "limit", 100)?;
    let volume_id = optional_id(v, "volume_id")?;
    let full_count = match v.get("full_count") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err(failure("invalid_request", "full_count must be boolean")),
    };
    let generation = ensure(v)?;
    remember(&generation)?;
    let db_path = generation.root.join(&generation.key).join("catalog.sqlite");
    let catalog = Catalog::open_readonly(&db_path)?;
    let page = if full_count {
        serde_json::to_value(catalog.search(query, volume_id, offset, limit)?)
    } else {
        serde_json::to_value(catalog.search_fast(query, volume_id, offset, limit)?)
    }
    .map_err(|_| failure("internal", "Search page encoding failed"))?;
    drop(catalog);
    if identity(&path(v, "path")?)? != generation.source {
        return Err(failure(
            "source_changed",
            "CTF changed during mirror search",
        ));
    }
    Ok(json!({
        "generation":generation.key,
        "source_sha256":generation.source.sha256,
        "read_only":true,
        "annotations_supported":false,
        "rebuilt":generation.built,
        "result":page
    }))
}

/// All pinned reads share the same full original CTF SHA checks and periodic
/// SQLite cache integrity verification. This returns an explicitly tagged
/// snapshot response, not a user-editable catalog.
pub(crate) fn read_pinned<F>(v: &Value, work: F) -> Result<Value>
where
    F: FnOnce(&Catalog) -> Result<Value>,
{
    let requested = text(v, "generation")?;
    if requested.len() != 64 || !requested.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(failure(
            "invalid_request",
            "Expected 64-character generation digest",
        ));
    }
    let source_path = path(v, "path")?;
    let source_meta = fs::symlink_metadata(&source_path).map_err(io_error)?;
    if !source_meta.file_type().is_file()
        || source_meta.len() > CTF_LIMIT
        || source_path
            .extension()
            .and_then(|x| x.to_str())
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("ctf"))
    {
        return Err(failure(
            "ctf_limit",
            "Expected an unlinked regular .CTF source",
        ));
    }
    let original = identity(&source_path)?;
    let used_recipe = recipe();
    let current_key = ctf_sidecar::cache_key(&original, &used_recipe).map_err(io_error)?;
    if requested != current_key {
        return Err(failure(
            "source_changed",
            "Original CTF changed; synchronize the new generation",
        ));
    }
    let root = path(v, "cache_root")?;
    if !fs::symlink_metadata(&root)
        .map_err(io_error)?
        .file_type()
        .is_dir()
    {
        return Err(failure(
            "cache_root",
            "Pinned mirror root must be a real directory",
        ));
    }
    let key: PinKey = (root.clone(), current_key.clone());
    let pinned = pins()
        .lock()
        .map_err(|_| failure("internal", "Pinned registry unavailable"))?
        .get(&key)
        .cloned()
        .ok_or_else(|| failure("not_pinned", "Call ctf_sync in this process first"))?;
    if pinned.source != original {
        forget(&root, &current_key);
        return Err(failure(
            "source_changed",
            "Pinned source generation does not match",
        ));
    }
    let cache_dir = root.join(&current_key);
    let db_path = cache_dir.join("catalog.sqlite");
    let first_stamp = stamp(&db_path)?;
    if first_stamp != pinned.stamp {
        forget(&root, &current_key);
        return Err(failure(
            "cache_changed",
            "Pinned SQLite artifact changed; strict ctf_sync required",
        ));
    }
    let mut full_cache_recheck = false;
    if pinned.checked_at.elapsed() >= PIN_REFRESH_INTERVAL {
        verify(&cache_dir, &original, &used_recipe)?;
        let after_verify = stamp(&db_path)?;
        if after_verify != first_stamp {
            forget(&root, &current_key);
            return Err(failure(
                "cache_changed",
                "SQLite artifact changed during recheck",
            ));
        }
        let mut registry = pins()
            .lock()
            .map_err(|_| failure("internal", "Pinned registry unavailable"))?;
        if let Some(entry) = registry.get_mut(&key) {
            entry.checked_at = Instant::now();
        }
        full_cache_recheck = true;
    }
    let db = Catalog::open_readonly(&db_path)?;
    let data = work(&db)?;
    drop(db);
    if stamp(&db_path)? != first_stamp {
        forget(&root, &current_key);
        return Err(failure(
            "cache_changed",
            "SQLite artifact changed during query",
        ));
    }
    if identity(&source_path)? != original {
        forget(&root, &current_key);
        return Err(failure(
            "source_changed",
            "Original CTF changed during pinned query",
        ));
    }
    Ok(json!({
        "generation":current_key,
        "source_sha256":original.sha256,
        "read_only":true,
        "annotations_supported":false,
        "source_sha_checked":true,
        "verification_mode":"pinned_cache",
        "cache_fully_rehashed_this_query":full_cache_recheck,
        "cache_full_revalidation_interval_seconds":PIN_REFRESH_INTERVAL.as_secs(),
        "result":data
    }))
}

/// Fast explicit mode: query a strictly verified read-only CTF generation
/// within the process that created the pin. Does not allow source edits.
pub fn search_pinned(v: &Value) -> Result<Value> {
    let query = text(v, "query")?;
    atlas_core::query::compile(query)?;
    let offset = unsigned(v, "offset", 0)?;
    let limit = unsigned(v, "limit", 100)?;
    let volume_id = optional_id(v, "volume_id")?;
    let full_count = match v.get("full_count") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err(failure("invalid_request", "full_count must be boolean")),
    };
    read_pinned(v, |db| {
        if full_count {
            serde_json::to_value(db.search(query, volume_id, offset, limit)?)
        } else {
            serde_json::to_value(db.search_fast(query, volume_id, offset, limit)?)
        }
        .map_err(|_| failure("internal", "Cannot encode pinned search result"))
    })
}

/// List disks from the exact pinned CTF generation without exposing an
/// editable SQLite path to the GUI. Source SHA is rechecked at both ends.
pub fn volumes_pinned(v: &Value) -> Result<Value> {
    read_pinned(v, |db| {
        serde_json::to_value(db.volumes()?)
            .map_err(|_| failure("internal", "Cannot encode CTF volume list"))
    })
}

/// Hierarchical directory browsing in a pinned read-only CTF generation.
pub fn children_pinned(v: &Value) -> Result<Value> {
    let volume_id = optional_id(v, "volume_id")?
        .ok_or_else(|| failure("invalid_request", "A positive volume_id is required"))?;
    let parent_id = optional_id(v, "parent_id")?;
    let offset = unsigned(v, "offset", 0)?;
    let limit = unsigned(v, "limit", 250)?;
    read_pinned(v, |db| {
        serde_json::to_value(db.children(volume_id, parent_id, offset, limit)?)
            .map_err(|_| failure("internal", "Cannot encode CTF directory page"))
    })
}
