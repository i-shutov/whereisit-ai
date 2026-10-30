//! User-owned annotation store for linked, read-only CTF generations.
//! This SQLite file is NOT the derived CTF mirror. Annotations are never
//! written into the CTF, its immutable SQLite index, or an export snapshot.
//! Generation changes remain explicit conflicts; no ID/filename auto-rebinding.
use crate::{ctf_mirror, optional_id, text, unsigned};
use atlas_core::{Error, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const APPLICATION_ID: i64 = 0x5749_4131; // WIA1, distinct from an ordinary catalog.
const SCHEMA_VERSION: i64 = 1;
const MAX_NOTE_BYTES: usize = 16 * 1024;
const MAX_TAG_BYTES: usize = 8 * 1024;

fn fail(code: &'static str, message: &'static str) -> Error {
    Error::new(code, message)
}

#[cfg(unix)]
fn multiply_linked(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    meta.nlink() != 1
}

#[cfg(windows)]
fn multiply_linked(_meta: &fs::Metadata) -> bool {
    // std::os::windows::fs::MetadataExt::number_of_links is unstable on
    // the pinned Rust 1.90 Windows runner (E0658). SQLite NOFOLLOW and the
    // application-id/foreign-schema checks still reject CTF/mirror aliases
    // before any mutation. Do not claim generic Windows hardlink detection.
    false
}

#[cfg(not(any(unix, windows)))]
fn multiply_linked(_meta: &fs::Metadata) -> bool {
    true // Fail closed on platforms whose file identity we cannot verify.
}

// A separate, explicitly selected .wia.sqlite is user data, not a generation
// cache. Canonicalize the parent before checking containment and reject symlink
// destinations so a dialog-selected annotation file cannot be the original CTF
// or a SQLite mirror through a simple alias.
fn store_path(v: &Value) -> Result<PathBuf> {
    let requested = PathBuf::from(text(v, "annotation_db")?);
    let name = requested
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| fail("invalid_request", "Annotation filename is required"))?;
    if !name.ends_with(".wia.sqlite") {
        return Err(fail(
            "invalid_request",
            "User annotation store must end in .wia.sqlite",
        ));
    }
    let parent = requested
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let target = parent.join(name);
    let source = PathBuf::from(text(v, "path")?).canonicalize()?;
    let cache = PathBuf::from(text(v, "cache_root")?).canonicalize()?;
    if target == source || target.starts_with(cache) {
        return Err(fail(
            "unsafe_annotation_store",
            "User annotations must reside outside source and derived cache",
        ));
    }
    match fs::symlink_metadata(&target) {
        Ok(meta) if !meta.file_type().is_file() || multiply_linked(&meta) => {
            return Err(fail(
                "unsafe_annotation_store",
                "Annotation destination must be one regular non-linked file",
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(target)
}

fn check_app(conn: &Connection) -> Result<()> {
    let app: i64 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let schema: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if app != APPLICATION_ID || schema != SCHEMA_VERSION {
        return Err(fail(
            "annotation_store_invalid",
            "Unrecognized user annotation store version or application ID",
        ));
    }
    Ok(())
}

fn open_read(path: &Path) -> Result<Option<Connection>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
        Ok(meta) if !meta.file_type().is_file() => {
            return Err(fail(
                "unsafe_annotation_store",
                "Annotation file was replaced or multiply linked",
            ));
        }
        Ok(_) => {}
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    conn.busy_timeout(Duration::from_secs(5))?;
    check_app(&conn)?;
    Ok(Some(conn))
}

// Only a SET request is allowed to create a user-owned sidecar. Never replace
// a foreign SQLite file; initialise in an IMMEDIATE transaction, keeping the
// generated opaque document identity independent of CTF bytes and filenames.
fn open_write(path: &Path, generation: &str) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    conn.busy_timeout(Duration::from_secs(5))?;
    // Check foreign SQLite before changing its journal mode or any pragma.
    let app: i64 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    if app != 0 && app != APPLICATION_ID {
        return Err(fail(
            "annotation_store_invalid",
            "Refusing to modify a foreign SQLite application database",
        ));
    }
    let tables: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%'",
        [],
        |r| r.get(0),
    )?;
    if app == 0 && tables != 0 {
        return Err(fail(
            "annotation_store_invalid",
            "Refusing to initialize an existing foreign SQLite database",
        ));
    }
    if app == APPLICATION_ID {
        check_app(&conn)?;
    }
    conn.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;")?;
    if app == 0 {
        let doc: String = conn.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
        conn.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE annotation_document(
                document_id TEXT NOT NULL PRIMARY KEY,
                generation TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK(schema_version=1)
             );
             CREATE TABLE annotation_entries(
                entry_id INTEGER PRIMARY KEY,
                volume_label TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                kind TEXT NOT NULL,
                size INTEGER NOT NULL,
                note TEXT NOT NULL,
                tags TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK(revision > 0)
             );",
        )?;
        conn.execute(
            "INSERT INTO annotation_document(document_id,generation,schema_version) VALUES(?1,?2,1)",
            params![doc, generation],
        )?;
        conn.pragma_update(None, "application_id", APPLICATION_ID)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        conn.execute_batch("COMMIT;")?;
    }
    check_app(&conn)?;
    Ok(conn)
}

fn metadata(conn: &Connection) -> Result<(String, String)> {
    let (doc, generation, schema): (String, String, i64) = conn.query_row(
        "SELECT document_id,generation,schema_version FROM annotation_document LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if schema != SCHEMA_VERSION || doc.len() != 32 || generation.len() != 64 {
        return Err(fail(
            "annotation_store_invalid",
            "Invalid annotation document identity or generation",
        ));
    }
    Ok((doc, generation))
}

fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM annotation_entries", [], |r| r.get(0))?)
}

/// List and inspect are read-only. If an external CTF save causes a new
/// generation, return an explicit conflict WITHOUT projecting old entry IDs
/// onto new ones. The user-owned store is retained untouched.
pub fn list(v: &Value) -> Result<Value> {
    let path = store_path(v)?;
    let generation = text(v, "generation")?;
    let offset = unsigned(v, "offset", 0)?.min(i64::MAX as u64);
    let limit = unsigned(v, "limit", 100)?.clamp(1, 250);
    let mut output = ctf_mirror::read_pinned(v, |_db| {
        let Some(conn) = open_read(&path)? else {
            return Ok(json!({"state":"uninitialized","items":[],"total":0,
                             "document_id":null,"read_only_source":true}));
        };
        let (document_id, stored_generation) = metadata(&conn)?;
        let total = count(&conn)?;
        if stored_generation != generation {
            return Ok(json!({
                "state":"generation_conflict","document_id":document_id,
                "items":[],"total":total,"requires_explicit_reconciliation":true,
                "read_only_source":true
            }));
        }
        let mut stmt = conn.prepare(
            "SELECT entry_id,volume_label,relative_path,kind,size,note,tags,revision
             FROM annotation_entries ORDER BY entry_id LIMIT ?1 OFFSET ?2",
        )?;
        let items: Vec<Value> = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                Ok(json!({
                    "entry_id":row.get::<_,i64>(0)?,
                    "volume_label":row.get::<_,String>(1)?,
                    "relative_path":row.get::<_,String>(2)?,
                    "kind":row.get::<_,String>(3)?,
                    "size":row.get::<_,i64>(4)?,
                    "note":row.get::<_,String>(5)?,
                    "tags":row.get::<_,String>(6)?,
                    "revision":row.get::<_,i64>(7)?
                }))
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(json!({
            "state":"ready","document_id":document_id,"items":items,
            "total":total,"limit":limit,"offset":offset,
            "read_only_source":true,"user_owned_separate_store":true
        }))
    })?;
    output["annotations_supported"] = json!(true);
    Ok(output)
}

/// Optimistic revision check prevents silent clobber across native processes.
/// expected_revision=0 inserts a new record; a later update must provide the
/// revision returned by LIST or SET. Clearing both fields deletes the record.
/// Never infer a mapping from name/size for a changed CTF generation.
pub fn set(v: &Value) -> Result<Value> {
    let path = store_path(v)?;
    let generation = text(v, "generation")?;
    let entry_id = optional_id(v, "entry_id")?
        .ok_or_else(|| fail("invalid_request", "Positive entry_id required"))?;
    let expected = unsigned(v, "expected_revision", u64::MAX)?;
    if expected == u64::MAX || expected > i64::MAX as u64 {
        return Err(fail(
            "invalid_request",
            "Non-negative expected_revision required",
        ));
    }
    let note = text(v, "note")?;
    let tags = text(v, "tags")?;
    if note.len() > MAX_NOTE_BYTES || tags.len() > MAX_TAG_BYTES {
        return Err(fail(
            "invalid_request",
            "Annotation text length limit exceeded",
        ));
    }

    let mut output = ctf_mirror::read_pinned(v, |catalog| {
        let entry: Option<(String, String, String, i64)> = catalog
            .conn()
            .query_row(
                "SELECT v.label,e.relative_path,e.kind,e.size
                 FROM entries e JOIN volumes v ON v.id=e.volume_id WHERE e.id=?1",
                [entry_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let (volume_label, relative_path, kind, size) =
            entry.ok_or_else(|| fail("not_found", "Entry not present in pinned CTF"))?;
        let mut conn = open_write(&path, generation)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (document_id, store_generation) = metadata(&tx)?;
        if store_generation != generation {
            return Err(fail(
                "annotation_generation_conflict",
                "External CTF generation changed; explicit reconciliation required",
            ));
        }
        let previous: Option<i64> = tx
            .query_row(
                "SELECT revision FROM annotation_entries WHERE entry_id=?1",
                [entry_id],
                |row| row.get(0),
            )
            .optional()?;
        if previous.unwrap_or(0) as u64 != expected {
            return Err(fail(
                "annotation_revision_conflict",
                "Annotation was changed by another writer; reload before editing",
            ));
        }
        let revision = if note.is_empty() && tags.is_empty() {
            tx.execute(
                "DELETE FROM annotation_entries WHERE entry_id=?1",
                [entry_id],
            )?;
            0
        } else {
            let next = previous
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| fail("annotation_revision_conflict", "Revision overflow"))?;
            tx.execute(
                "INSERT INTO annotation_entries
                 (entry_id,volume_label,relative_path,kind,size,note,tags,revision)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
                 ON CONFLICT(entry_id) DO UPDATE SET
                 volume_label=excluded.volume_label,
                 relative_path=excluded.relative_path,
                 kind=excluded.kind,size=excluded.size,
                 note=excluded.note,tags=excluded.tags,revision=excluded.revision",
                params![
                    entry_id,
                    volume_label,
                    relative_path,
                    kind,
                    size,
                    note,
                    tags,
                    next
                ],
            )?;
            next
        };
        tx.commit()?;
        Ok(json!({
            "state":"saved","document_id":document_id,"entry_id":entry_id,
            "revision":revision,"user_owned_separate_store":true,
            "read_only_source":true
        }))
    })?;
    output["annotations_supported"] = json!(true);
    Ok(output)
}

const MAX_PREVIEW_CANDIDATES: usize = 8;

/// Read-only, bounded reconciliation hints after an external CTF save.
/// Exact metadata matches are NOT content identity. Even one candidate is
/// unconfirmed. This operation never writes a mapping or migrates a note.
pub fn preview(v: &Value) -> Result<Value> {
    let path = store_path(v)?;
    let generation = text(v, "generation")?;
    let offset = unsigned(v, "offset", 0)?.min(i64::MAX as u64);
    let limit = unsigned(v, "limit", 50)?.clamp(1, 50);
    let mut output = ctf_mirror::read_pinned(v, |catalog| {
        let Some(conn) = open_read(&path)? else {
            return Ok(json!({
                "state":"uninitialized","document_id":null,"items":[],"total":0,
                "preview_only":true,"read_only_source":true
            }));
        };
        let (document_id, stored_generation) = metadata(&conn)?;
        let total = count(&conn)?;
        if stored_generation == generation {
            return Ok(json!({
                "state":"current_generation","document_id":document_id,
                "items":[],"total":total,"preview_only":true,"read_only_source":true
            }));
        }
        let mut old = conn.prepare(
            "SELECT entry_id,volume_label,relative_path,kind,size,note,tags,revision
             FROM annotation_entries ORDER BY entry_id LIMIT ?1 OFFSET ?2",
        )?;
        // The immutable v3 core has an indexed kind+size predicate. Other
        // fields match byte-for-byte; no fuzzy or case-folded rebinding.
        let mut matches = catalog.conn().prepare(
            "SELECT e.id,e.volume_id,e.parent_id,e.modified_unix
             FROM entries_core e JOIN volumes v ON v.id=e.volume_id
             WHERE e.kind=?1 AND e.size=?2
               AND e.relative_path=?3 COLLATE BINARY
               AND v.label=?4 COLLATE BINARY
             ORDER BY e.id LIMIT ?5",
        )?;
        let items: Vec<Value> = old
            .query_map(params![limit as i64, offset as i64], |row| {
                let old_entry_id: i64 = row.get(0)?;
                let volume_label: String = row.get(1)?;
                let relative_path: String = row.get(2)?;
                let kind: String = row.get(3)?;
                let size: i64 = row.get(4)?;
                let note: String = row.get(5)?;
                let tags: String = row.get(6)?;
                let revision: i64 = row.get(7)?;
                let mut candidates: Vec<Value> = matches
                    .query_map(
                        params![
                            kind,
                            size,
                            relative_path,
                            volume_label,
                            (MAX_PREVIEW_CANDIDATES + 1) as i64
                        ],
                        |candidate| {
                            Ok(json!({
                                "entry_id":candidate.get::<_, i64>(0)?,
                                "volume_id":candidate.get::<_, i64>(1)?,
                                "parent_id":candidate.get::<_, Option<i64>>(2)?,
                                "modified_unix":candidate.get::<_, Option<i64>>(3)?
                            }))
                        },
                    )?
                    .collect::<std::result::Result<_, _>>()?;
                let count_lower_bound = candidates.len();
                let truncated = count_lower_bound > MAX_PREVIEW_CANDIDATES;
                candidates.truncate(MAX_PREVIEW_CANDIDATES);
                let candidate_state = match count_lower_bound {
                    0 => "no_match",
                    1 => "unique_unconfirmed",
                    _ => "ambiguous",
                };
                Ok(json!({
                    "old_entry_id":old_entry_id,
                    "fingerprint":{
                        "volume_label":volume_label,"relative_path":relative_path,
                        "kind":kind,"size":size
                    },
                    "note":note,"tags":tags,"revision":revision,
                    "candidate_state":candidate_state,
                    "candidates":candidates,
                    "candidate_count_lower_bound":count_lower_bound,
                    "candidate_count_exact":!truncated,
                    "candidates_truncated":truncated,
                    "requires_confirmation":true
                }))
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(json!({
            "state":"generation_conflict","document_id":document_id,
            "items":items,"total":total,"offset":offset,"limit":limit,
            "preview_only":true,"automatic_rebinding":false,
            "requires_explicit_confirmation":true,"read_only_source":true,
            "user_owned_separate_store":true
        }))
    })?;
    output["annotations_supported"] = json!(true);
    Ok(output)
}
