//! Catalog maintenance never upgrades or overwrites the source database.
use crate::{legacy, text};
use atlas_core::{Catalog, Error, Result};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn ro_uri(path: &Path) -> Result<String> {
    let canonical = path.canonicalize()?;
    let value = canonical
        .to_str()
        .ok_or_else(|| Error::new("path_encoding", "Catalog path is not Unicode"))?;
    #[cfg(windows)]
    let value = value
        .strip_prefix("\\\\?\\")
        .unwrap_or(value)
        .replace('\\', "/");
    #[cfg(not(windows))]
    let value = value.to_owned();
    if value.starts_with("//") || value.starts_with("UNC/") {
        return Err(Error::new(
            "unsupported_path",
            "Use a local catalog copy, not a UNC path",
        ));
    }
    let mut uri = String::from("file:");
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~:".contains(&byte) {
            uri.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(&mut uri, "%{byte:02X}").expect("String write cannot fail");
        }
    }
    uri.push_str("?mode=ro");
    Ok(uri)
}

fn readonly(path: &Path) -> Result<Connection> {
    if !path.is_file() {
        return Err(Error::new(
            "not_found",
            "Source catalog is not a regular file",
        ));
    }
    let conn = Connection::open_with_flags(
        ro_uri(path)?,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.create_scalar_function(
        "unicode_lower",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC
            | rusqlite::functions::FunctionFlags::SQLITE_UTF8,
        |ctx| Ok(ctx.get::<String>(0)?.to_lowercase()),
    )?;
    conn.execute_batch("PRAGMA query_only=ON; PRAGMA foreign_keys=ON;")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

fn count(conn: &Connection, sql: &str) -> Result<i64> {
    Ok(conn.query_row(sql, [], |r| r.get(0))?)
}

fn has_table(conn: &Connection, schema: &str, name: &str) -> Result<bool> {
    if !matches!(schema, "main" | "src") {
        return Err(Error::new("internal", "Invalid schema"));
    }
    Ok(conn.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {schema}.sqlite_master WHERE type='table' AND name=?1)"
        ),
        [name],
        |r| r.get(0),
    )?)
}

fn supported_version(conn: &Connection, schema: &str) -> Result<i64> {
    let version: i64 = conn.pragma_query_value(
        Some(rusqlite::DatabaseName::Attached(schema)),
        "user_version",
        |r| r.get(0),
    )?;
    if !(1..=3).contains(&version) {
        return Err(Error::new(
            "schema_version",
            "Only catalog versions 1, 2 and 3 are supported",
        ));
    }
    Ok(version)
}

fn same_existing_file(a: &Path, b: &Path) -> bool {
    let (Ok(a), Ok(b)) = (a.canonicalize(), b.canonicalize()) else {
        return false;
    };
    if a == b || a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase() {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(a), Ok(b)) = (fs::metadata(a), fs::metadata(b)) {
            return a.dev() == b.dev() && a.ino() == b.ino();
        }
    }
    false
}

fn destination(source: &Path, target: &Path) -> Result<PathBuf> {
    if fs::symlink_metadata(target).is_ok() {
        return Err(Error::new(
            "file_exists",
            "Output must be a new file; existing paths are never replaced",
        ));
    }
    let name = target
        .file_name()
        .ok_or_else(|| Error::new("invalid_path", "Output requires a filename"))?;
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let result = parent.join(name);
    let source_real = source.canonicalize()?;
    let source_parent = source
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let source_alias = source_parent.join(
        source
            .file_name()
            .ok_or_else(|| Error::new("invalid_path", "Source requires a filename"))?,
    );
    // A case-insensitive or Unicode-normalizing filesystem may resolve an
    // alternate base spelling to the same catalog even when its sidecar is absent.
    let target_name = name.to_string_lossy();
    for suffix in ["-wal", "-shm", "-journal"] {
        if target_name.to_ascii_lowercase().ends_with(suffix) {
            let stem = &target_name[..target_name.len() - suffix.len()];
            if same_existing_file(source, &parent.join(stem)) {
                return Err(Error::new(
                    "unsafe_output",
                    "Output aliases a source SQLite sidecar",
                ));
            }
        }
    }
    for base in [source_real, source_alias] {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut protected = base.as_os_str().to_os_string();
            protected.push(suffix);
            if result == protected
                || result.to_string_lossy().to_lowercase()
                    == protected.to_string_lossy().to_lowercase()
            {
                return Err(Error::new(
                    "unsafe_output",
                    "Output cannot be a source catalog or SQLite sidecar",
                ));
            }
        }
    }
    Ok(result)
}

fn integrity(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA quick_check")?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if rows != ["ok"] {
        return Err(Error::new("integrity", "SQLite quick_check failed"));
    }
    let invalid = conn
        .prepare("PRAGMA foreign_key_check")?
        .query([])?
        .next()?
        .is_some();
    if invalid {
        return Err(Error::new("integrity", "Catalog foreign-key check failed"));
    }
    Ok(())
}

/// Read-only structural validation, not a claim about original-app CTF acceptance.
pub fn validate(v: &Value) -> Result<Value> {
    let conn = readonly(Path::new(text(v, "db")?))?;
    conn.execute_batch("BEGIN")?;
    let version = supported_version(&conn, "main")?;
    integrity(&conn)?;
    let records = count(&conn, "SELECT COUNT(*) FROM entries")?;
    let volumes = count(&conn, "SELECT COUNT(*) FROM volumes")?;
    let expected = count(&conn, "SELECT COALESCE(SUM(entry_count),0) FROM volumes")?;
    if records != expected {
        return Err(Error::new(
            "integrity",
            "Volume counters do not match catalog records",
        ));
    }
    let payload_rows = if version >= 3 {
        Some(count(&conn, "SELECT COUNT(*) FROM entry_payload")?)
    } else {
        None
    };
    conn.execute_batch("ROLLBACK")?;
    Ok(
        json!({"schema_version":version,"entries":records,"volumes":volumes,"payload_rows":payload_rows,"quick_check":"ok","foreign_key_errors":0,"readonly":true,"fts_integrity_checked":false}),
    )
}

/// Compact to a new runnable v3 file, with a consistent read-only source snapshot.
/// Work is set-based SQLite I/O, not a materialized in-memory catalog.
pub fn compact_copy(v: &Value) -> Result<Value> {
    let source = Path::new(text(v, "db")?);
    let target = destination(source, Path::new(text(v, "path")?))?;
    let source_uri = ro_uri(source)?;
    let probe = readonly(source)?;
    supported_version(&probe, "main")?;
    drop(probe);
    let temp = tempfile::NamedTempFile::new_in(target.parent().expect("canonical parent"))?;
    let c = Catalog::open(temp.path())?;
    let new_version: i64 = c
        .conn()
        .pragma_query_value(None, "user_version", |r| r.get(0))?;
    if new_version != 3 {
        return Err(Error::new(
            "schema_version",
            "Compact schema v3 is not active in this binary",
        ));
    }
    c.conn().execute_batch(
        "PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;",
    )?;
    legacy::ensure_legacy_schema(&c)?;
    c.conn()
        .execute("ATTACH DATABASE ?1 AS src", [source_uri])?;
    let tx = c.conn().unchecked_transaction()?;
    tx.execute_batch("PRAGMA defer_foreign_keys=ON")?;
    let source_version = supported_version(&tx, "src")?;
    let names = tx
        .prepare("SELECT name FROM src.sqlite_master WHERE type='table'")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for name in &names {
        let known = matches!(
            name.as_str(),
            "volumes"
                | "entries"
                | "entries_core"
                | "entry_payload"
                | "categories"
                | "entry_categories"
                | "ctf_sources"
                | "ctf_volumes"
                | "ctf_entries"
                | "ctf_entry_payload"
                | "atlas_meta"
                | "duplicate_cache_state"
                | "duplicate_cache_groups"
                | "duplicate_cache_members"
                | "sqlite_stat1"
                | "sqlite_stat4"
                | "sqlite_sequence"
                | "entries_trigram"
                | "entries_trigram_data"
                | "entries_trigram_idx"
                | "entries_trigram_docsize"
                | "entries_trigram_config"
        );
        if !known {
            return Err(Error::new(
                "unsupported_schema",
                "Unknown catalog table: refusing to discard unrecognized data",
            ));
        }
    }
    tx.execute_batch("INSERT INTO volumes SELECT id,label,root_path,scanned_at,entry_count,total_bytes,status FROM src.volumes;
INSERT INTO entries_core(id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,name_norm,ext) SELECT id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,name_norm,ext FROM src.entries ORDER BY id;
INSERT INTO entry_payload(entry_id,description,tags) SELECT id,description,tags FROM src.entries WHERE description<>'' OR tags<>'';
INSERT INTO categories SELECT id,parent_id,name,name_norm FROM src.categories;
INSERT INTO entry_categories SELECT entry_id,category_id FROM src.entry_categories;")?;
    let legacy_present = has_table(&tx, "src", "ctf_sources")?;
    if legacy_present {
        if !has_table(&tx, "src", "ctf_volumes")? || !has_table(&tx, "src", "ctf_entries")? {
            return Err(Error::new(
                "unsupported_schema",
                "Incomplete CTF metadata tables",
            ));
        }
        tx.execute_batch("INSERT INTO ctf_sources SELECT id,sha256,source_path,catalog_name,trailer,utc_offset_minutes,disk_count FROM src.ctf_sources;
INSERT INTO ctf_volumes SELECT volume_id,source_id,source_order,header FROM src.ctf_volumes;")?;
        let legacy_select = if source_version == 2 {
            "SELECT e.entry_id,e.attributes,e.modified_dos,e.created_dos,e.accessed_dos,COALESCE(p.opaque,x'') FROM src.ctf_entries e LEFT JOIN src.ctf_entry_payload p ON p.entry_id=e.entry_id"
        } else {
            "SELECT entry_id,attributes,modified_dos,created_dos,accessed_dos,opaque FROM src.ctf_entries"
        };
        tx.execute(&format!("INSERT INTO ctf_entries {legacy_select}"), [])?;
        let mismatch = count(&tx, &format!("SELECT COUNT(*) FROM (SELECT entry_id,attributes,modified_dos,created_dos,accessed_dos,opaque FROM ctf_entries EXCEPT {legacy_select})"))?;
        let expected = count(&tx, "SELECT COUNT(*) FROM src.ctf_entries")?;
        if mismatch != 0 || count(&tx, "SELECT COUNT(*) FROM ctf_entries")? != expected {
            return Err(Error::new("integrity", "CTF metadata parity failed"));
        }
    } else if has_table(&tx, "src", "ctf_volumes")? || has_table(&tx, "src", "ctf_entries")? {
        return Err(Error::new(
            "unsupported_schema",
            "Orphan CTF metadata tables",
        ));
    }
    let records = count(&tx, "SELECT COUNT(*) FROM entries_core")?;
    if records != count(&tx, "SELECT COUNT(*) FROM src.entries")? {
        return Err(Error::new("integrity", "Entry count parity failed"));
    }
    let mismatch = count(&tx, "SELECT COUNT(*) FROM src.entries s LEFT JOIN entries d ON d.id=s.id WHERE d.id IS NULL OR s.volume_id IS NOT d.volume_id OR s.parent_id IS NOT d.parent_id OR s.name IS NOT d.name OR s.relative_path IS NOT d.relative_path OR s.kind IS NOT d.kind OR s.size IS NOT d.size OR s.modified_unix IS NOT d.modified_unix OR s.description IS NOT d.description OR s.tags IS NOT d.tags OR s.name_norm IS NOT d.name_norm OR s.ext IS NOT d.ext OR s.search_text IS NOT d.search_text")?;
    if mismatch != 0 {
        return Err(Error::new(
            "integrity",
            "Entry value parity failed; output not published",
        ));
    }
    let volumes = count(&tx, "SELECT COUNT(*) FROM volumes")?;
    for table in ["volumes", "categories", "entry_categories"] {
        if count(&tx, &format!("SELECT COUNT(*) FROM (SELECT * FROM src.{table} EXCEPT SELECT * FROM main.{table})"))? != 0 || count(&tx, &format!("SELECT COUNT(*) FROM src.{table}"))? != count(&tx, &format!("SELECT COUNT(*) FROM main.{table}"))? {
            return Err(Error::new("integrity", "Metadata parity failed"));
        }
    }
    if legacy_present {
        for table in ["ctf_sources", "ctf_volumes"] {
            if count(&tx, &format!("SELECT COUNT(*) FROM (SELECT * FROM src.{table} EXCEPT SELECT * FROM main.{table})"))? != 0 || count(&tx, &format!("SELECT COUNT(*) FROM src.{table}"))? != count(&tx, &format!("SELECT COUNT(*) FROM main.{table}"))? {
                return Err(Error::new("integrity", "CTF source/header parity failed"));
            }
        }
    }
    if records != count(&tx, "SELECT COALESCE(SUM(entry_count),0) FROM volumes")? {
        return Err(Error::new(
            "integrity",
            "Source volume counters are inconsistent",
        ));
    }
    tx.execute_batch("INSERT INTO entries_trigram(entries_trigram,rank) VALUES('integrity-check',1); PRAGMA main.optimize=0x10002;")?;
    integrity(&tx)?;
    let payload_rows = count(&tx, "SELECT COUNT(*) FROM entry_payload")?;
    tx.commit()?;
    c.conn().execute_batch("DETACH DATABASE src")?;
    drop(c);
    temp.as_file().sync_all()?;
    let bytes = temp.as_file().metadata()?.len();
    let published = temp.persist_noclobber(&target).map_err(|e| {
        Error::new(
            "file_exists",
            format!(
                "Output could not be published without replacing an existing file: {}",
                e.error.kind()
            ),
        )
    })?;
    published.sync_all()?;
    Ok(
        json!({"source_version":source_version,"schema_version":3,"entries":records,"volumes":volumes,"payload_rows":payload_rows,"output_bytes":bytes,"entry_mismatches":0,"ctf_metadata_mismatches":0,"quick_check":"ok","foreign_key_errors":0,"fts_integrity_checked":true,"source_readonly":true,"source_replaced":false}),
    )
}
