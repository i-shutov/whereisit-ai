//! CTF is an interchange boundary; normal queries never open the CTF source.
use crate::{ctf_job::Job, exports, maintenance, text};
use atlas_core::{Catalog, Error, NewEntry, Result};
use chrono::{Datelike, FixedOffset, Local, NaiveDate, TimeZone, Timelike, Utc};
use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{ErrorKind, Read, Write},
    path::Path,
};

const CTF_BLOCK_BYTES: usize = 256 * 1024;

const LEGACY_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS ctf_sources(id INTEGER PRIMARY KEY,sha256 TEXT NOT NULL UNIQUE,source_path TEXT NOT NULL,catalog_name TEXT NOT NULL,trailer BLOB NOT NULL,utc_offset_minutes INTEGER,disk_count INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS ctf_volumes(volume_id INTEGER PRIMARY KEY REFERENCES volumes(id) ON DELETE CASCADE,source_id INTEGER NOT NULL REFERENCES ctf_sources(id),source_order INTEGER NOT NULL,header BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS ctf_entries(entry_id INTEGER PRIMARY KEY REFERENCES entries(id) ON DELETE CASCADE,attributes INTEGER NOT NULL,modified_dos INTEGER NOT NULL,created_dos INTEGER NOT NULL,accessed_dos INTEGER NOT NULL,opaque BLOB NOT NULL);
CREATE INDEX IF NOT EXISTS ctf_volumes_source ON ctf_volumes(source_id,source_order);
"#;

/// Raw CTF records are dense provenance, not sparse optional annotations.
/// Keep v1 and experimental v2 compatible; v3 refers to the real core table.
pub(crate) fn ensure_legacy_schema(c: &Catalog) -> Result<bool> {
    let version: i64 = c
        .conn()
        .pragma_query_value(None, "user_version", |r| r.get(0))?;
    let mut schema = LEGACY_SCHEMA.to_owned();
    if version >= 3 {
        schema = schema.replace("REFERENCES entries(id)", "REFERENCES entries_core(id)");
    }
    if version == 2 {
        schema = schema.replace(",opaque BLOB NOT NULL", "");
        schema.push_str("CREATE TABLE IF NOT EXISTS ctf_entry_payload(entry_id INTEGER PRIMARY KEY REFERENCES ctf_entries(entry_id) ON DELETE CASCADE,opaque BLOB NOT NULL CHECK(length(opaque)>0));");
    }
    c.conn().execute_batch(&schema)?;
    Ok(version != 2)
}
fn fail(e: impl std::fmt::Display) -> Error {
    Error::new("ctf", e)
}
fn read(v: &Value, job: &Job) -> Result<(Vec<u8>, String)> {
    let path = Path::new(text(v, "path")?);
    let mut file = fs::File::open(path)?;
    let meta = file.metadata()?;
    let limit = Limits::default().max_bytes;
    if !meta.is_file() {
        return Err(Error::new(
            "invalid_path",
            "CTF source must be a regular file",
        ));
    }
    if meta.len() > limit as u64 {
        return Err(Error::new(
            "ctf_limit",
            "CTF exceeds the supported 2 GB size limit",
        ));
    }
    let mut bytes = Vec::new();
    let mut block = vec![0u8; CTF_BLOCK_BYTES];
    let mut hash = Sha256::new();
    loop {
        job.check()?;
        let count = file.read(&mut block)?;
        if count == 0 {
            break;
        }
        if count > limit - bytes.len() {
            return Err(Error::new(
                "ctf_limit",
                "CTF grew beyond the supported size limit while reading",
            ));
        }
        hash.update(&block[..count]);
        bytes.extend_from_slice(&block[..count]);
    }
    Ok((bytes, crate::digest_hex(hash.finalize())))
}
pub fn info(v: &Value) -> Result<Value> {
    let (bytes, sha) = read(v, &Job::default())?;
    let catalog = ctf_format::parse_ctf(&bytes, &Limits::default()).map_err(fail)?;
    let entries: usize = catalog.disks.iter().map(|d| d.entries.len()).sum();
    let files: usize = catalog
        .disks
        .iter()
        .map(|d| {
            d.entries
                .iter()
                .filter(|e| e.kind == EntryKind::File)
                .count()
        })
        .sum();
    Ok(
        json!({"name":catalog.name,"sha256":sha,"bytes":bytes.len(),"disks":catalog.disks.len(),"entries":entries,"files":files,"format":"Catalog 3.00 / observed uncompressed variant"}),
    )
}
pub fn import(c: &mut Catalog, v: &Value) -> Result<Value> {
    import_with_job(c, v, &Job::default(), None)
}

pub(crate) fn import_with_job(
    c: &mut Catalog,
    v: &Value,
    job: &Job,
    expected_sha: Option<&str>,
) -> Result<Value> {
    let offset = match v.get("utc_offset_minutes") {
        None | Some(Value::Null) => None,
        Some(x) => Some(
            x.as_i64()
                .filter(|x| (-1439..=1439).contains(x))
                .ok_or_else(|| {
                    Error::new(
                        "invalid_request",
                        "utc_offset_minutes must be within -1439..1439",
                    )
                })? as i32,
        ),
    };
    job.phase("reading_source")?;
    let (bytes, sha) = read(v, job)?;
    if expected_sha.is_some_and(|expected| !sha.eq_ignore_ascii_case(expected)) {
        return Err(Error::new(
            "source_changed",
            "CTF bytes differ from the expected source",
        ));
    }
    let dense_opaque = ensure_legacy_schema(c)?;
    let seen: Option<i64> = c
        .conn()
        .query_row("SELECT id FROM ctf_sources WHERE sha256=?1", [&sha], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(id) = seen {
        return Err(Error::new(
            "already_imported",
            format!("This exact CTF is already imported as source {id}"),
        ));
    }
    job.phase("parsing")?;
    let parsed = ctf_format::parse_ctf(&bytes, &Limits::default()).map_err(fail)?;
    job.check()?;
    let source_path = Path::new(text(v, "path")?)
        .canonicalize()?
        .to_string_lossy()
        .into_owned();
    let source_name = parsed.name.clone();
    let disk_count = parsed.disks.len();
    let diagnostics = ctf_format::diagnostics(&parsed).map_err(fail)?;
    // Parsing completes before any catalog rows are changed. All volumes and legacy
    // metadata then commit together; a parser/SQL/validation error leaves no partial import.
    drop(bytes);
    job.phase("importing")?;
    let result=c.bulk_import_many_fast_fts(|session|{
  session.transaction().execute("INSERT INTO ctf_sources(sha256,source_path,catalog_name,trailer,utc_offset_minutes,disk_count) VALUES(?1,?2,?3,?4,?5,?6)",params![sha,source_path,source_name,parsed.opaque_trailer,offset,disk_count as i64])?;
  let source_id=session.transaction().last_insert_rowid();
  let mut volumes=Vec::new();let mut entries=0u64;
  for (sequence,disk) in parsed.disks.iter().enumerate(){
   job.check()?;
   let volume_id=session.start_volume(&disk.name,"")?;
   session.transaction().execute("INSERT INTO ctf_volumes(volume_id,source_id,source_order,header) VALUES(?1,?2,?3,?4)",params![volume_id,source_id,sequence as i64,disk.opaque_header])?;
   let mut ids=Vec::with_capacity(disk.entries.len());
   // Only directory paths need to remain in memory; paths for files are streamed.
   let mut directory_paths:Vec<Option<String>>=Vec::with_capacity(disk.entries.len());
   for (index,e) in disk.entries.iter().enumerate(){
    job.entry()?;
    let (parent_id,path)=if let Some(parent)=e.parent{
     if parent>=index{return Err(Error::new("ctf_tree","CTF parent must precede its child"));}
     let prefix=directory_paths.get(parent).and_then(Option::as_ref).ok_or_else(||Error::new("ctf_tree","Missing CTF directory parent"))?;
     (Some(ids[parent]),format!("{prefix}/{}",e.name))
    }else{(None,e.name.clone())};
    let kind=match e.kind{EntryKind::File=>"file",EntryKind::Directory=>"directory",EntryKind::Archive=>"archive"};
    let new=NewEntry{parent_id,name:e.name.clone(),relative_path:path.clone(),kind:kind.into(),size:e.size,modified_unix:dos_to_unix(e.modified_dos,offset),description:e.description.clone(),tags:String::new()};
    let entry_id=session.insert(&new)?;
    if dense_opaque {
    session.transaction().prepare_cached("INSERT INTO ctf_entries(entry_id,attributes,modified_dos,created_dos,accessed_dos,opaque) VALUES(?1,?2,?3,?4,?5,?6)")?.execute(params![entry_id,e.attributes,e.modified_dos,e.created_dos,e.accessed_dos,e.opaque])?;
    } else {
      session.transaction().prepare_cached("INSERT INTO ctf_entries(entry_id,attributes,modified_dos,created_dos,accessed_dos) VALUES(?1,?2,?3,?4,?5)")?.execute(params![entry_id,e.attributes,e.modified_dos,e.created_dos,e.accessed_dos])?;
      if !e.opaque.is_empty() {
        session.transaction().prepare_cached("INSERT INTO ctf_entry_payload(entry_id,opaque) VALUES(?1,?2)")?.execute(params![entry_id,e.opaque])?;
      }
    }
    ids.push(entry_id);directory_paths.push(if e.kind==EntryKind::File{None}else{Some(path)});entries+=1;
   }
   volumes.push(session.finish_volume()?);
  }
  job.phase("indexing")?;
  Ok(json!({"source_id":source_id,"sha256":sha,"volumes":volumes,"entries":entries}))
 })?;
    job.check()?;
    let mut result = result;
    result["diagnostics"] = serde_json::to_value(diagnostics).map_err(fail)?;
    result["timestamp_policy"] = json!(if offset.is_some() {
        "DOS local timestamps converted using explicit utc_offset_minutes; original fields retained"
    } else {
        "DOS timestamps have no timezone; raw values retained, Unix time left unknown"
    });
    result["category_policy"]=json!("Legacy opaque metadata is retained; mapping CTF category definitions to native categories is not implemented");
    // Move completed WAL pages into the portable database when no concurrent reader
    // prevents it. SQLite remains consistent if the checkpoint is busy.
    let _ = c.conn().execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
    Ok(result)
}

fn vacant(target: &Path) -> Result<()> {
    match fs::symlink_metadata(target) {
        Ok(_) => Err(Error::new(
            "file_exists",
            "Import needs a new filename with no existing SQLite sidecars",
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn check_import_destination(target: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = target.as_os_str().to_owned();
        name.push(suffix);
        vacant(Path::new(&name))?;
    }
    Ok(())
}

pub(crate) fn import_new(v: &Value) -> Result<Value> {
    let job = Job::start(v)?;
    if v["overwrite"] == true {
        return Err(Error::new(
            "invalid_request",
            "CTF import needs a new database",
        ));
    }
    let source = Path::new(text(v, "path")?);
    let expected_sha = match v.get("expected_source_sha256") {
        None => None,
        Some(_) => {
            let sha = text(v, "expected_source_sha256")?;
            if sha.len() != 64 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(Error::new(
                    "invalid_request",
                    "Expected a 64-character source SHA-256",
                ));
            }
            Some(sha)
        }
    };
    let requested = Path::new(text(v, "db")?);
    let filename = requested
        .file_name()
        .ok_or_else(|| Error::new("invalid_path", "Import needs a database filename"))?;
    let name = filename.to_string_lossy().to_ascii_lowercase();
    if ["-wal", "-shm", "-journal"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
    {
        return Err(Error::new(
            "unsafe_output",
            "Import cannot use a SQLite sidecar filename",
        ));
    }
    let parent = requested
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let target = parent.join(filename);
    check_import_destination(&target)?;
    let stage = tempfile::Builder::new()
        .prefix(".atlas-ctf-import-")
        .tempfile_in(parent)?;
    let mut catalog = Catalog::open(stage.path())?;
    // A standalone staged SQLite file is published without any WAL sidecars.
    catalog
        .conn()
        .execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;")?;
    let mut result = import_with_job(&mut catalog, v, &job, expected_sha)?;
    job.phase("validating")?;
    catalog.conn().execute(
        "INSERT INTO entries_trigram(entries_trigram,rank) VALUES('integrity-check',1)",
        [],
    )?;
    drop(catalog);
    job.check()?;
    let checked = maintenance::validate(&json!({"db":stage.path()}))?;
    if checked["entries"] != result["entries"]
        || checked["volumes"].as_u64() != result["volumes"].as_array().map(|v| v.len() as u64)
    {
        return Err(Error::new(
            "integrity",
            "Imported catalog counts could not be verified",
        ));
    }
    stage.as_file().sync_all()?;
    job.phase("verifying_source")?;
    let found = job.identity(source, Limits::default().max_bytes as u64)?;
    if result["sha256"].as_str() != Some(found.sha256.as_str()) {
        return Err(Error::new("source_changed", "CTF changed during import"));
    }
    check_import_destination(&target)?;
    job.begin_publication()?;
    exports::publish(stage, &target, false)?;
    result["db"] = json!(target);
    Ok(result)
}

pub(crate) fn update_description(v: &Value) -> Result<Value> {
    let db = Path::new(text(v, "db")?);
    let description = text(v, "description")?;
    let entry_id = crate::optional_id(v, "entry_id")?
        .ok_or_else(|| Error::new("invalid_request", "A positive entry_id is required"))?;
    if description.contains('\0') {
        return Err(Error::new(
            "invalid_request",
            "Description cannot contain NUL",
        ));
    }
    let metadata = fs::symlink_metadata(db)?;
    if !metadata.file_type().is_file() {
        return Err(Error::new(
            "invalid_path",
            "A regular editable SQLite catalog is required",
        ));
    }
    if metadata.permissions().readonly() {
        return Err(Error::new("read_only", "The SQLite catalog is read-only"));
    }
    let catalog = Catalog::open(db)?;
    let tx = Transaction::new_unchecked(catalog.conn(), TransactionBehavior::Immediate)?;
    let (kind, current, tags) = tx
        .query_row(
            "SELECT kind,description,tags FROM entries WHERE id=?1",
            [entry_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| Error::new("not_found", "Entry not found"))?;
    if current != description {
        validate_imported_description(&tx, entry_id, &kind, description)?;
        catalog.update_entry(entry_id, description, &tags)?;
    }
    tx.commit()?;
    Ok(json!({"entry_id":entry_id,"description":description}))
}

fn validate_imported_description(
    tx: &Transaction<'_>,
    entry_id: i64,
    kind: &str,
    description: &str,
) -> Result<()> {
    let has_ctf: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='ctf_sources')",
        [],
        |r| r.get(0),
    )?;
    if !has_ctf {
        return Ok(());
    }
    let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let opaque_query = if version == 2 {
        "SELECT l.opaque,p.opaque,v.header,s.source_path,e.parent_id IS NOT NULL
         FROM entries e JOIN ctf_volumes v ON v.volume_id=e.volume_id
         JOIN ctf_sources s ON s.id=v.source_id
         LEFT JOIN ctf_entry_payload l ON l.entry_id=e.id
         LEFT JOIN ctf_entry_payload p ON p.entry_id=e.parent_id
         WHERE e.id=?1"
    } else {
        "SELECT l.opaque,p.opaque,v.header,s.source_path,e.parent_id IS NOT NULL
         FROM entries e JOIN ctf_volumes v ON v.volume_id=e.volume_id
         JOIN ctf_sources s ON s.id=v.source_id
         LEFT JOIN ctf_entries l ON l.entry_id=e.id
         LEFT JOIN ctf_entries p ON p.entry_id=e.parent_id
         WHERE e.id=?1"
    };
    let imported = tx
        .query_row(opaque_query, [entry_id], |r| {
            Ok((
                r.get::<_, Option<Vec<u8>>>(0)?.unwrap_or_default(),
                r.get::<_, Option<Vec<u8>>>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, bool>(4)?,
            ))
        })
        .optional()?;
    if let Some((record, parent, disk, source, has_parent)) = imported {
        if source.is_empty() {
            return Err(Error::new("read_only", "Linked CTF mirrors are read-only"));
        }
        let kind = match kind {
            "file" => EntryKind::File,
            "directory" => EntryKind::Directory,
            "archive" => EntryKind::Archive,
            _ => {
                return Err(Error::new(
                    "ctf_unsupported",
                    "Unsupported imported CTF entry kind",
                ))
            }
        };
        ctf_format::validate_description_edit(
            kind,
            &record,
            if has_parent {
                Some(parent.as_deref().unwrap_or_default())
            } else {
                None
            },
            &disk,
            description,
        )
        .map_err(|e| Error::new("ctf_unsupported", e))?;
    }
    Ok(())
}

pub fn export(v: &Value) -> Result<Value> {
    let job = Job::start(v)?;
    let allow_lossy = v
        .get("allow_lossy")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let db = Path::new(text(v, "db")?);
    if v.get("job_id").is_some() {
        if allow_lossy || v["overwrite"] == true {
            return Err(Error::new(
                "invalid_request",
                "CTF jobs require a new output and preserve all supported metadata",
            ));
        }
        if !db.is_file() {
            return Err(Error::new("not_found", "SQLite catalog does not exist"));
        }
    }
    let c = Catalog::open(db)?;
    let dense_opaque = ensure_legacy_schema(&c)?;
    let snapshot = c.conn().unchecked_transaction()?;
    let has_categories: bool = snapshot.query_row(
        "SELECT EXISTS(SELECT 1 FROM categories) OR EXISTS(SELECT 1 FROM entry_categories)",
        [],
        |r| r.get(0),
    )?;
    let symlinks: i64 = snapshot.query_row(
        "SELECT COUNT(*) FROM entries WHERE kind='symlink'",
        [],
        |r| r.get(0),
    )?;
    if symlinks > 0 {
        return Err(Error::new(
            "ctf_unsupported",
            "CTF export of symbolic links is not yet supported; use CSV or Parquet",
        ));
    }
    if has_categories && !allow_lossy {
        return Err(Error::new("ctf_categories","Native categories cannot yet be encoded in CTF; export CSV/Parquet for category assignments, or explicitly set allow_lossy=true to export without native categories"));
    }
    let has_tags: bool = snapshot.query_row(
        "SELECT EXISTS(SELECT 1 FROM entries WHERE tags <> '')",
        [],
        |r| r.get(0),
    )?;
    if has_tags && !allow_lossy {
        return Err(Error::new("ctf_tags","New native tags cannot yet be encoded in CTF; export CSV/Parquet, or explicitly set allow_lossy=true to export without those tags"));
    }
    let sources: i64 = snapshot.query_row("SELECT COUNT(*) FROM ctf_sources", [], |r| r.get(0))?;
    if sources > 1 {
        return Err(Error::new("ctf_unsupported","Merging several legacy CTF sources for CTF export is not yet supported; CSV/Parquet export remains available"));
    }
    let source: Option<(String, Vec<u8>, String)> = snapshot
        .query_row(
            "SELECT catalog_name,trailer,source_path FROM ctf_sources LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((_, _, ref original)) = source {
        let output = Path::new(text(v, "path")?);
        if output.exists() && output.canonicalize().ok() == Path::new(original).canonicalize().ok()
        {
            return Err(Error::new(
                "unsafe_output",
                "Export must use a separate path from the original CTF",
            ));
        }
    }
    let (tmp, target, overwrite) = exports::output_file(v)?;
    let mut catalog = CtfCatalog {
        name: v
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                source
                    .as_ref()
                    .map(|s| s.0.clone())
                    .unwrap_or_else(|| "WhereIsIt AI".into())
            }),
        disks: Vec::new(),
        opaque_trailer: source.as_ref().map(|s| s.1.clone()).unwrap_or_default(),
    };
    let volumes = {
        let mut s=snapshot.prepare("SELECT v.id,v.label,cv.header FROM volumes v LEFT JOIN ctf_volumes cv ON cv.volume_id=v.id ORDER BY COALESCE(cv.source_order,2147483647),v.id")?;
        let rows = s.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<Vec<u8>>>(2)?.unwrap_or_default(),
            ))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    job.phase("reading_catalog")?;
    let mut entry_count = 0u64;
    for (volume_id, label, header) in volumes {
        job.check()?;
        let mut disk = CtfDisk {
            name: label,
            entries: Vec::new(),
            opaque_header: header,
        };
        let mut ids = HashMap::<i64, usize>::new();
        let sql = if dense_opaque {
            "SELECT e.id,e.parent_id,e.name,e.kind,e.size,e.modified_unix,e.description,l.attributes,l.modified_dos,l.created_dos,l.accessed_dos,l.opaque FROM entries e LEFT JOIN ctf_entries l ON l.entry_id=e.id WHERE e.volume_id=?1 ORDER BY e.id"
        } else {
            "SELECT e.id,e.parent_id,e.name,e.kind,e.size,e.modified_unix,e.description,l.attributes,l.modified_dos,l.created_dos,l.accessed_dos,p.opaque FROM entries e LEFT JOIN ctf_entries l ON l.entry_id=e.id LEFT JOIN ctf_entry_payload p ON p.entry_id=e.id WHERE e.volume_id=?1 ORDER BY e.id"
        };
        let mut stmt = snapshot.prepare(sql)?;
        let mut rows = stmt.query([volume_id])?;
        while let Some(r) = rows.next()? {
            job.entry()?;
            let id: i64 = r.get(0)?;
            let parent: Option<i64> = r.get(1)?;
            let kind: String = r.get(3)?;
            let parent = parent
                .map(|p| {
                    ids.get(&p)
                        .copied()
                        .ok_or_else(|| Error::new("ctf_tree", "Parent ordering is invalid"))
                })
                .transpose()?;
            let kind = match kind.as_str() {
                "file" => EntryKind::File,
                "directory" => EntryKind::Directory,
                "archive" => EntryKind::Archive,
                _ => return Err(Error::new("ctf_unsupported", "Unsupported entry type")),
            };
            let unix: Option<i64> = r.get(5)?;
            let modified = r
                .get::<_, Option<u32>>(8)?
                .unwrap_or_else(|| unix_to_dos(unix));
            let signed_size: i64 = r.get(4)?;
            let size = u64::try_from(signed_size)
                .map_err(|_| Error::new("ctf_corrupt", "Negative SQL size in legacy CTF export"))?;
            let entry = CtfEntry {
                name: r.get(2)?,
                description: r.get(6)?,
                parent,
                kind,
                size,
                attributes: r
                    .get::<_, Option<u32>>(7)?
                    .unwrap_or(if kind == EntryKind::Directory { 16 } else { 32 }),
                modified_dos: modified,
                created_dos: r.get::<_, Option<u32>>(9)?.unwrap_or(modified),
                accessed_dos: r.get::<_, Option<u32>>(10)?.unwrap_or(modified),
                opaque: r.get::<_, Option<Vec<u8>>>(11)?.unwrap_or_default(),
            };
            ids.insert(id, disk.entries.len());
            disk.entries.push(entry);
            entry_count += 1;
        }
        catalog.disks.push(disk);
    }
    snapshot.commit()?;
    job.phase("encoding")?;
    let bytes = ctf_format::write_ctf(&catalog).map_err(fail)?;
    let len = bytes.len();
    let disk_count = catalog.disks.len();
    drop(catalog);
    job.phase("writing")?;
    let mut tmp = tmp;
    let mut digest = Sha256::new();
    for block in bytes.chunks(CTF_BLOCK_BYTES) {
        job.check()?;
        tmp.write_all(block)?;
        digest.update(block);
    }
    let sha = crate::digest_hex(digest.finalize());
    tmp.as_file().sync_all()?;
    job.begin_publication()?;
    exports::publish(tmp, &target, overwrite)?;
    Ok(
        json!({"path":target,"bytes":len,"sha256":sha,"disks":disk_count,"entries":entry_count,"format":"Catalog 3.00","new_categories_omitted":has_categories,"new_tags_omitted":has_tags,"compatibility":"Observed variant; see validation report for original-application checks"}),
    )
}

fn dos_to_unix(raw: u32, offset: Option<i32>) -> Option<i64> {
    let offset = offset?;
    if raw == 0 {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(
        ((raw >> 25) & 127) as i32 + 1980,
        (raw >> 21) & 15,
        (raw >> 16) & 31,
    )?;
    let local = date.and_hms_opt((raw >> 11) & 31, (raw >> 5) & 63, (raw & 31) * 2)?;
    FixedOffset::east_opt(offset * 60)?
        .from_local_datetime(&local)
        .single()
        .map(|x| x.timestamp())
}
fn unix_to_dos(unix: Option<i64>) -> u32 {
    let Some(t) = unix
        .and_then(|t| Utc.timestamp_opt(t, 0).single())
        .map(|t| t.with_timezone(&Local))
    else {
        return 0;
    };
    if !(1980..=2107).contains(&t.year()) {
        return 0;
    }
    ((t.year() - 1980) as u32) << 25
        | t.month() << 21
        | t.day() << 16
        | t.hour() << 11
        | t.minute() << 5
        | (t.second() / 2)
}
