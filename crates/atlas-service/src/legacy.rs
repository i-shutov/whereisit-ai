//! CTF is an interchange boundary; normal queries never open the CTF source.
use crate::{exports, text};
use atlas_core::{Catalog, Error, NewEntry, Result};
use chrono::{Datelike, FixedOffset, Local, NaiveDate, TimeZone, Timelike, Utc};
use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::Path,
};

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
fn read(v: &Value) -> Result<(Vec<u8>, String)> {
    let path = Path::new(text(v, "path")?);
    let meta = fs::metadata(path)?;
    if meta.len() > Limits::default().max_bytes as u64 {
        return Err(Error::new(
            "ctf_limit",
            "CTF exceeds the supported 2 GB size limit",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(Limits::default().max_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > Limits::default().max_bytes {
        return Err(Error::new(
            "ctf_limit",
            "CTF grew beyond the supported size limit while reading",
        ));
    }
    let hash = format!("{:x}", Sha256::digest(&bytes));
    Ok((bytes, hash))
}
pub fn info(v: &Value) -> Result<Value> {
    let (bytes, sha) = read(v)?;
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
    let (bytes, sha) = read(v)?;
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
    let parsed = ctf_format::parse_ctf(&bytes, &Limits::default()).map_err(fail)?;
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
    let result=c.bulk_import_many(|session|{
  session.transaction().execute("INSERT INTO ctf_sources(sha256,source_path,catalog_name,trailer,utc_offset_minutes,disk_count) VALUES(?1,?2,?3,?4,?5,?6)",params![sha,source_path,source_name,parsed.opaque_trailer,offset,disk_count as i64])?;
  let source_id=session.transaction().last_insert_rowid();
  let mut volumes=Vec::new();let mut entries=0u64;
  for (sequence,disk) in parsed.disks.iter().enumerate(){
   let volume_id=session.start_volume(&disk.name,"")?;
   session.transaction().execute("INSERT INTO ctf_volumes(volume_id,source_id,source_order,header) VALUES(?1,?2,?3,?4)",params![volume_id,source_id,sequence as i64,disk.opaque_header])?;
   let mut ids=Vec::with_capacity(disk.entries.len());
   // Only directory paths need to remain in memory; paths for files are streamed.
   let mut directory_paths:Vec<Option<String>>=Vec::with_capacity(disk.entries.len());
   for (index,e) in disk.entries.iter().enumerate(){
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
  Ok(json!({"source_id":source_id,"sha256":sha,"volumes":volumes,"entries":entries}))
 })?;
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

pub fn export(c: &Catalog, v: &Value) -> Result<Value> {
    let dense_opaque = ensure_legacy_schema(c)?;
    let snapshot = c.conn().unchecked_transaction()?;
    let links: i64 =
        snapshot.query_row("SELECT COUNT(*) FROM entry_categories", [], |r| r.get(0))?;
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
    let allow_lossy = v
        .get("allow_lossy")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if links > 0 && !allow_lossy {
        return Err(Error::new("ctf_categories","New category assignments cannot yet be encoded in CTF; export CSV/Parquet, or explicitly set allow_lossy=true to export without those assignments"));
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
    let mut entry_count = 0u64;
    for (volume_id, label, header) in volumes {
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
    let bytes = ctf_format::write_ctf(&catalog).map_err(fail)?;
    let sha = format!("{:x}", Sha256::digest(&bytes));
    let len = bytes.len();
    let disk_count = catalog.disks.len();
    drop(catalog);
    let mut tmp = tmp;
    tmp.write_all(&bytes)?;
    exports::finish(tmp, &target, overwrite)?;
    Ok(
        json!({"path":target,"bytes":len,"sha256":sha,"disks":disk_count,"entries":entry_count,"format":"Catalog 3.00","new_categories_omitted":links>0,"new_tags_omitted":has_tags,"compatibility":"Observed variant; see validation report for original-application checks"}),
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
