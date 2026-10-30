//! Explicit, whole-file snapshot and restore of the PRIVATE annotation sidecar.
//! No CTF or derived mirror bytes are modified. Never overwrite a destination.
use crate::{ctf_annotations, ctf_mirror, text};
use atlas_core::{Error, Result};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

const MAX_SNAPSHOT_BYTES: u64 = 8 * 1024 * 1024 * 1024;

fn fail(code: &'static str, message: &'static str) -> Error {
    Error::new(code, message)
}

fn target(v: &Value, key: &str, suffix: &str) -> Result<PathBuf> {
    let supplied = PathBuf::from(text(v, key)?);
    let filename = supplied
        .file_name()
        .ok_or_else(|| fail("invalid_request", "Snapshot filename is required"))?;
    if !filename.to_string_lossy().ends_with(suffix) {
        return Err(fail(
            "invalid_request",
            "Unexpected snapshot filename extension",
        ));
    }
    let parent = supplied
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let path = parent.join(filename);
    let original = PathBuf::from(text(v, "path")?).canonicalize()?;
    let cache = PathBuf::from(text(v, "cache_root")?).canonicalize()?;
    let sidecar = ctf_annotations::store_path(v)?;
    if path == original || path.starts_with(cache) || path == sidecar {
        return Err(fail(
            "unsafe_annotation_snapshot",
            "Snapshot must be outside CTF, derived cache and selected user store",
        ));
    }
    Ok(path)
}

fn must_be_absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(fail(
            "annotation_snapshot_exists",
            "Refusing to overwrite an existing snapshot or annotation store",
        )),
        Err(e) => Err(e.into()),
    }
}

fn regular_existing(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() || meta.len() > MAX_SNAPSHOT_BYTES {
        return Err(fail(
            "annotation_snapshot_invalid",
            "Snapshot must be a regular file within the size limit",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() != 1 {
            return Err(fail(
                "annotation_snapshot_invalid",
                "Multiply linked snapshot is not accepted",
            ));
        }
    }
    Ok(())
}

fn private_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    File::open(path)?.sync_all()?;
    #[cfg(not(target_os = "linux"))]
    let _ = path;
    Ok(())
}

fn digest_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 262_144];
    let mut total = 0_u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n as u64)
            .ok_or_else(|| fail("annotation_snapshot_invalid", "Snapshot size overflow"))?;
        if total > MAX_SNAPSHOT_BYTES {
            return Err(fail(
                "annotation_snapshot_invalid",
                "Annotation snapshot exceeds size limit",
            ));
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn open_snapshot(path: &Path) -> Result<Connection> {
    regular_existing(path)?;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    ctf_annotations::check_app(&conn)?;
    Ok(conn)
}

type TableInventory = Vec<(String, i64)>;
type SnapshotInfo = (String, String, TableInventory);

fn validate(conn: &Connection) -> Result<SnapshotInfo> {
    ctf_annotations::check_app(conn)?;
    let (document, generation) = ctf_annotations::metadata(conn)?;
    let check: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if check != "ok" {
        return Err(fail(
            "annotation_snapshot_invalid",
            "SQLite integrity check failed",
        ));
    }
    if conn.prepare("PRAGMA foreign_key_check")?.exists([])? {
        return Err(fail(
            "annotation_snapshot_invalid",
            "SQLite foreign key check failed",
        ));
    }
    // Verify every application table, not only notes. Additive confirmation,
    // activation and later audit tables must survive a complete snapshot.
    let mut statement = conn.prepare(
        "SELECT name FROM sqlite_master
         WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut inventory = Vec::with_capacity(names.len());
    for name in names {
        let quoted = name.replace('"', "\"\"");
        let sql = format!("SELECT COUNT(*) FROM \"{quoted}\"");
        let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
        inventory.push((name, count));
    }
    Ok((document, generation, inventory))
}

fn publish(staged: &Path, destination: &Path) -> Result<()> {
    // Stage and destination share one parent/filesystem. Hard-link publication
    // is atomic and fails with AlreadyExists instead of silently replacing.
    fs::hard_link(staged, destination).map_err(|e| {
        if e.kind() == ErrorKind::AlreadyExists {
            fail(
                "annotation_snapshot_exists",
                "Concurrent destination creation; no overwrite allowed",
            )
        } else {
            e.into()
        }
    })?;
    sync_directory(
        destination
            .parent()
            .ok_or_else(|| fail("invalid_request", "Snapshot parent missing"))?,
    )?;
    Ok(())
}

/// SQLite VACUUM INTO captures one consistent source transaction, even if a
/// different application connection is writing. The original remains intact.
/// A CTF generation is checked both BEFORE and AFTER making the staged copy;
/// only then is the validated private copy published.
pub fn backup(v: &Value) -> Result<Value> {
    if text(v, "backup")? != "user_approved_new_snapshot" {
        return Err(fail(
            "annotation_backup_consent",
            "Explicit backup consent required",
        ));
    }
    let expected = text(v, "expected_document_id")?;
    let store = ctf_annotations::store_path(v)?;
    let destination = target(v, "backup_path", ".wia.backup.sqlite")?;
    must_be_absent(&destination)?;
    let temp = tempfile::Builder::new()
        .prefix(".wia-snapshot-")
        .tempdir_in(
            destination
                .parent()
                .ok_or_else(|| fail("invalid_request", "Snapshot parent missing"))?,
        )?;
    let staged = temp.path().join("snapshot.sqlite");
    let mut out = ctf_mirror::read_pinned(v, |_catalog| {
        let source = ctf_annotations::open_read(&store)?.ok_or_else(|| {
            fail(
                "annotation_store_missing",
                "No existing user annotation store to back up",
            )
        })?;
        let (document, _generation, original_inventory) = validate(&source)?;
        if document != expected {
            return Err(fail(
                "annotation_document_conflict",
                "Annotation document changed; reload before backup",
            ));
        }
        let stage_string = staged.to_str().ok_or_else(|| {
            fail(
                "invalid_request",
                "Non-Unicode snapshot path is unsupported",
            )
        })?;
        source.execute("VACUUM INTO ?1", params![stage_string])?;
        private_permissions(&staged)?;
        File::open(&staged)?.sync_all()?;
        let cloned = open_snapshot(&staged)?;
        let (cloned_document, cloned_generation, cloned_inventory) = validate(&cloned)?;
        if cloned_document != document || cloned_inventory != original_inventory {
            return Err(fail(
                "annotation_snapshot_invalid",
                "Snapshot identity or complete table inventory mismatch",
            ));
        }
        let digest = digest_file(&staged)?;
        Ok(json!({
            "state":"snapshot_verified",
            "document_id":cloned_document,
            "original_annotation_generation":cloned_generation,
            "backup_sha256":digest,
            "table_count":cloned_inventory.len(),
            "annotation_count":cloned_inventory.iter()
                .find(|(name, _)| name == "annotation_entries")
                .map_or(0, |(_, n)| *n),
            "ctf_unchanged":true,
            "derived_cache_unchanged":true,
            "user_store_unchanged":true,
            "backup_only":true
        }))
    })?;
    publish(&staged, &destination)?;
    out["result"]["state"] = json!("snapshot_published");
    out["annotations_supported"] = json!(true);
    Ok(out)
}

/// Restore ONLY into a new user-owned .wia.sqlite path. Never touch or
/// replace a living sidecar; a full SHA-256 CAS verifies exactly the snapshot
/// the owner selected. Restoring does not bind notes to the current CTF.
pub fn restore(v: &Value) -> Result<Value> {
    if text(v, "restore")? != "user_approved_new_sidecar" {
        return Err(fail(
            "annotation_restore_consent",
            "Explicit restore consent required",
        ));
    }
    let expected_document = text(v, "expected_document_id")?;
    let expected_hash = text(v, "expected_backup_sha256")?;
    if expected_hash.len() != 64 || !expected_hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(fail("invalid_request", "Expected exact backup SHA-256"));
    }
    let destination = ctf_annotations::store_path(v)?;
    let backup = target(v, "backup_path", ".wia.backup.sqlite")?;
    must_be_absent(&destination)?;
    regular_existing(&backup)?;
    let temp = tempfile::Builder::new()
        .prefix(".wia-restore-")
        .tempdir_in(
            destination
                .parent()
                .ok_or_else(|| fail("invalid_request", "Annotation parent missing"))?,
        )?;
    let staged = temp.path().join("restored.sqlite");
    let mut out = ctf_mirror::read_pinned(v, |_catalog| {
        let selected = open_snapshot(&backup)?;
        let (document, original_generation, inventory) = validate(&selected)?;
        if document != expected_document {
            return Err(fail(
                "annotation_document_conflict",
                "Snapshot document does not match explicit selection",
            ));
        }
        // Copy a stable SQLite snapshot. The staged hash is checked AFTER
        // copying to prevent source replacement/races from changing bytes.
        fs::copy(&backup, &staged)?;
        private_permissions(&staged)?;
        File::open(&staged)?.sync_all()?;
        if !digest_file(&staged)?.eq_ignore_ascii_case(expected_hash) {
            return Err(fail(
                "annotation_backup_changed",
                "Snapshot bytes differ from expected SHA-256; nothing restored",
            ));
        }
        let restored = open_snapshot(&staged)?;
        let (new_document, new_generation, new_inventory) = validate(&restored)?;
        if document != new_document
            || original_generation != new_generation
            || inventory != new_inventory
        {
            return Err(fail(
                "annotation_snapshot_invalid",
                "Restored sidecar does not match validated snapshot",
            ));
        }
        Ok(json!({
            "state":"restore_verified",
            "document_id":new_document,
            "original_annotation_generation":new_generation,
            "table_count":new_inventory.len(),
            "annotation_count":new_inventory.iter()
                .find(|(name, _)| name == "annotation_entries")
                .map_or(0, |(_, n)| *n),
            "requires_explicit_reconciliation":new_generation != text(v, "generation")?,
            "source_ctf_untouched":true,
            "cache_untouched":true,
            "no_existing_sidecar_replaced":true
        }))
    })?;
    publish(&staged, &destination)?;
    out["result"]["state"] = json!("restored_to_new_sidecar");
    out["annotations_supported"] = json!(true);
    Ok(out)
}
