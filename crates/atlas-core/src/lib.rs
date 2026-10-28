use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;
pub mod query;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}
impl Error {
    pub fn new(code: &'static str, message: impl ToString) -> Self {
        Self {
            code,
            message: message.to_string(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for Error {}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::new("database", e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new("io", e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub volume_id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub relative_path: String,
    pub kind: String,
    pub size: u64,
    pub modified_unix: Option<i64>,
    pub description: String,
    pub tags: String,
}
#[derive(Debug, Clone)]
pub struct NewEntry {
    pub parent_id: Option<i64>,
    pub name: String,
    pub relative_path: String,
    pub kind: String,
    pub size: u64,
    pub modified_unix: Option<i64>,
    pub description: String,
    pub tags: String,
}
#[derive(Debug, Serialize)]
pub struct Volume {
    pub id: i64,
    pub label: String,
    pub root_path: String,
    pub scanned_at: i64,
    pub entry_count: u64,
    /// Sum of logical file entries; directory and archive aggregate sizes are excluded.
    /// File entries inside archives are included. This is not physical storage used.
    pub total_bytes: u64,
    pub status: String,
}
#[derive(Debug, Serialize)]
pub struct Page {
    pub items: Vec<Entry>,
    pub total: u64,
    pub offset: u64,
    pub limit: u64,
}

#[derive(Debug, Serialize)]
pub struct FastPage {
    pub items: Vec<Entry>,
    pub offset: u64,
    pub limit: u64,
    pub has_more: bool,
}

pub struct Catalog {
    conn: Connection,
}
impl Catalog {
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        conn.create_scalar_function(
            "unicode_lower",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC
                | rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            |ctx| Ok(ctx.get::<String>(0)?.to_lowercase()),
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;",
        )?;
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if v > 3 {
            return Err(Error::new(
                "schema_version",
                "Database was created by a newer version",
            ));
        }
        if v == 0 {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let current: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
            if current == 0 {
                tx.execute_batch(include_str!("../migrations/003.sql"))?;
            } else if current > 3 {
                return Err(Error::new(
                    "schema_version",
                    "Database was created by a newer version",
                ));
            }
            tx.commit()?;
        }
        Ok(Self { conn })
    }
    /// Hot predicates select the indexed core directly. Annotations are joined
    /// only for the final display page, or when requested by the predicate.
    pub fn query_table(&self, query: &str) -> Result<&'static str> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        Ok(if version >= 3 && !query::needs_payload(query)? {
            "entries_core"
        } else {
            "entries"
        })
    }
    pub fn volumes(&self) -> Result<Vec<Volume>> {
        let mut s=self.conn.prepare("SELECT id,label,root_path,scanned_at,entry_count,total_bytes,status FROM volumes ORDER BY id DESC")?;
        let rows = s.query_map([], |r| {
            Ok(Volume {
                id: r.get(0)?,
                label: r.get(1)?,
                root_path: r.get(2)?,
                scanned_at: r.get(3)?,
                entry_count: r.get(4)?,
                total_bytes: r.get(5)?,
                status: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
    pub fn stats(&self) -> Result<Value> {
        self.conn.query_row("SELECT COUNT(*),COALESCE(SUM(entry_count),0),COALESCE(SUM(total_bytes),0) FROM volumes",[],|r|Ok(json!({"volumes":r.get::<_,u64>(0)?,"entries":r.get::<_,u64>(1)?,"total_bytes":r.get::<_,u64>(2)?}))).map_err(Into::into)
    }
    pub fn children(
        &self,
        volume_id: i64,
        parent_id: Option<i64>,
        offset: u64,
        limit: u64,
    ) -> Result<Page> {
        let limit = limit.clamp(1, 1000);
        let off = offset.min(i64::MAX as u64) as i64;
        let table = self.query_table("")?;
        let total = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE volume_id=?1 AND parent_id IS ?2"),
            params![volume_id, parent_id],
            |r| r.get(0),
        )?;
        let mut s=self.conn.prepare(&format!("WITH page AS MATERIALIZED (SELECT id FROM {table} WHERE volume_id=?1 AND parent_id IS ?2 ORDER BY CASE kind WHEN 'directory' THEN 0 ELSE 1 END,name COLLATE NOCASE,id LIMIT ?3 OFFSET ?4) SELECT e.id,e.volume_id,e.parent_id,e.name,e.relative_path,e.kind,e.size,e.modified_unix,e.description,e.tags FROM page p JOIN entries e ON e.id=p.id ORDER BY CASE e.kind WHEN 'directory' THEN 0 ELSE 1 END,e.name COLLATE NOCASE,e.id"))?;
        let items = s
            .query_map(params![volume_id, parent_id, limit as i64, off], entry_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Page {
            items,
            total,
            offset,
            limit,
        })
    }
    /// Search using the Atlas query language; an empty query matches all entries.
    /// Counts all matches before returning a page; use `search_fast` to avoid counting.
    pub fn search(
        &self,
        query: &str,
        volume_id: Option<i64>,
        offset: u64,
        limit: u64,
    ) -> Result<Page> {
        let (filter, mut args) = query::compile(query)?;
        let table = self.query_table(query)?;
        let limit = limit.clamp(1, 1000);
        let off = offset.min(i64::MAX as u64) as i64;
        let mut clauses = vec![filter];
        if let Some(id) = volume_id {
            clauses.push("volume_id = ?".into());
            args.push(id.into());
        }
        let filter = clauses.join(" AND ");
        let total = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE {filter}"),
            rusqlite::params_from_iter(args.iter()),
            |r| r.get(0),
        )?;
        let sql=format!("WITH page AS MATERIALIZED (SELECT id FROM {table} WHERE {filter} ORDER BY name COLLATE NOCASE,id LIMIT ? OFFSET ?) SELECT e.id,e.volume_id,e.parent_id,e.name,e.relative_path,e.kind,e.size,e.modified_unix,e.description,e.tags FROM page p JOIN entries e ON e.id=p.id ORDER BY e.name COLLATE NOCASE,e.id");
        args.push((limit as i64).into());
        args.push(off.into());
        let mut s = self.conn.prepare(&sql)?;
        let items = s
            .query_map(rusqlite::params_from_iter(args.iter()), entry_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Page {
            items,
            total,
            offset,
            limit,
        })
    }
    /// Return a page and a continuation flag without an exact total or a COUNT query.
    /// Large OFFSETs can still require substantial work; ordering follows `search`.
    pub fn search_fast(
        &self,
        query: &str,
        volume_id: Option<i64>,
        offset: u64,
        limit: u64,
    ) -> Result<FastPage> {
        let (filter, mut args) = query::compile(query)?;
        let table = self.query_table(query)?;
        let limit = limit.clamp(1, 1000);
        let mut clauses = vec![filter];
        if let Some(id) = volume_id {
            clauses.push("volume_id = ?".into());
            args.push(id.into());
        }
        let sql=format!("WITH page AS MATERIALIZED (SELECT id FROM {table} WHERE {} ORDER BY name COLLATE NOCASE,id LIMIT ? OFFSET ?) SELECT e.id,e.volume_id,e.parent_id,e.name,e.relative_path,e.kind,e.size,e.modified_unix,e.description,e.tags FROM page p JOIN entries e ON e.id=p.id ORDER BY e.name COLLATE NOCASE,e.id",clauses.join(" AND "));
        args.push(((limit + 1) as i64).into());
        args.push((offset.min(i64::MAX as u64) as i64).into());
        let mut stmt = self.conn.prepare(&sql)?;
        let mut items = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), entry_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = items.len() > limit as usize;
        if has_more {
            items.pop();
        }
        Ok(FastPage {
            items,
            offset,
            limit,
            has_more,
        })
    }
    pub fn update_entry(&self, id: i64, description: &str, tags: &str) -> Result<()> {
        let found: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT name,relative_path FROM entries WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (name, path) = found.ok_or_else(|| Error::new("not_found", "Entry not found"))?;
        self.conn.execute(
            "UPDATE entries SET description=?1,tags=?2,search_text=?3 WHERE id=?4",
            params![
                description,
                tags,
                search_text(&name, &path, description, tags),
                id
            ],
        )?;
        Ok(())
    }
    /// Atomic bulk import. The closure inserts entries one at a time; parent IDs refer to inserted rows.
    /// Failure rolls back the complete volume and all entries.
    pub fn bulk_import<F>(&mut self, label: &str, root_path: &str, mut write: F) -> Result<Volume>
    where
        F: FnMut(&mut BulkWriter<'_>) -> Result<()>,
    {
        let tx = self.conn.transaction()?;
        let now = unix_now();
        tx.execute("INSERT INTO volumes(label,root_path,scanned_at,entry_count,total_bytes,status) VALUES(?1,?2,?3,0,0,'complete')",params![label,root_path,now])?;
        let id = tx.last_insert_rowid();
        let mut w = BulkWriter {
            tx: &tx,
            volume_id: id,
            count: 0,
            bytes: 0,
            sparse: tx.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? >= 3,
        };
        write(&mut w)?;
        let (count, bytes) = (w.count, w.bytes);
        tx.execute(
            "UPDATE volumes SET entry_count=?1,total_bytes=?2 WHERE id=?3",
            params![count, bytes, id],
        )?;
        // Collect bounded planner statistics within the atomic transaction.
        tx.execute_batch("PRAGMA optimize=0x10002")?;
        tx.commit()?;
        Ok(Volume {
            id,
            label: label.into(),
            root_path: root_path.into(),
            scanned_at: now,
            entry_count: count,
            total_bytes: bytes,
            status: "complete".into(),
        })
    }
    /// Import any number of volumes atomically, streaming entries one at a time.
    /// Every started volume must be finished. An error or panic rolls back every volume.
    pub fn bulk_import_many<F, T>(&mut self, write: F) -> Result<T>
    where
        F: FnOnce(&mut ImportSession<'_>) -> Result<T>,
    {
        let tx = self.conn.transaction()?;
        let output = {
            let mut session = ImportSession {
                tx: &tx,
                current: None,
            };
            let output = write(&mut session)?;
            if session.current.is_some() {
                return Err(Error::new(
                    "invalid_import",
                    "Finish the current volume before committing",
                ));
            }
            output
        };
        // Include newly imported volumes in bounded planner analysis.
        tx.execute_batch("PRAGMA optimize=0x10002")?;
        tx.commit()?;
        Ok(output)
    }
    pub fn scan(&mut self, root: &Path, label: &str, job_id: Option<&str>) -> Result<Volume> {
        let root_meta = fs::symlink_metadata(root)
            .map_err(|e| Error::new("invalid_root", format!("{}: {e}", root.display())))?;
        if !root_meta.file_type().is_dir() {
            return Err(Error::new(
                "invalid_root",
                format!(
                    "Not a directory or root is a symbolic link: {}",
                    root.display()
                ),
            ));
        }
        let root = fs::canonicalize(root)?;
        let root_text = root.to_string_lossy().into_owned();
        let job = Arc::new(Job::default());
        let _guard = JobGuard::register(job_id, job.clone())?;
        self.bulk_import(label, &root_text, |writer| {
            let mut parents: Vec<i64> = Vec::new();
            for item in WalkDir::new(&root).follow_links(false).min_depth(1) {
                if job.state.load(Ordering::Acquire) == 1 {
                    return Err(Error::new(
                        "cancelled",
                        "Scan cancelled; no snapshot was saved",
                    ));
                }
                let item = item.map_err(|e| Error::new("scan", e))?;
                let meta = fs::symlink_metadata(item.path())
                    .map_err(|e| Error::new("scan", format!("{}: {e}", item.path().display())))?;
                let depth = item.depth();
                parents.truncate(depth.saturating_sub(1));
                let parent = if depth == 1 {
                    None
                } else {
                    Some(
                        *parents
                            .last()
                            .ok_or_else(|| Error::new("scan", "Invalid directory hierarchy"))?,
                    )
                };
                let ty = meta.file_type();
                let kind = if ty.is_symlink() {
                    "symlink"
                } else if ty.is_dir() {
                    "directory"
                } else if ty.is_file() {
                    "file"
                } else {
                    continue;
                };
                let rel = item
                    .path()
                    .strip_prefix(&root)
                    .map_err(|e| Error::new("scan", e))?
                    .to_string_lossy()
                    .replace('\\', "/");
                let e = NewEntry {
                    parent_id: parent,
                    name: item.file_name().to_string_lossy().into_owned(),
                    relative_path: rel,
                    kind: kind.into(),
                    size: if kind == "file" { meta.len() } else { 0 },
                    modified_unix: meta.modified().ok().and_then(to_unix),
                    description: String::new(),
                    tags: String::new(),
                };
                let id = writer.insert(&e)?;
                if kind == "directory" {
                    parents.push(id)
                }
                job.count.fetch_add(1, Ordering::Relaxed);
            }
            if job
                .state
                .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Err(Error::new(
                    "cancelled",
                    "Scan cancelled; no snapshot was saved",
                ));
            }
            Ok(())
        })
    }
    pub fn create_category(&self, name: &str, parent_id: Option<i64>) -> Result<i64> {
        if name.trim().is_empty() {
            return Err(Error::new(
                "invalid_request",
                "Category name cannot be empty",
            ));
        }
        self.conn.execute(
            "INSERT INTO categories(name,name_norm,parent_id) VALUES(?1,?2,?3)",
            params![name, name.to_lowercase(), parent_id],
        )?;
        Ok(self.conn.last_insert_rowid())
    }
    pub fn assign_category(&self, entry_id: i64, category_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO entry_categories(entry_id,category_id) VALUES(?1,?2)",
            params![entry_id, category_id],
        )?;
        Ok(())
    }
    pub fn categories(&self) -> Result<Value> {
        let mut s = self
            .conn
            .prepare("SELECT id,parent_id,name FROM categories ORDER BY name")?;
        let result=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"parent_id":r.get::<_,Option<i64>>(1)?,"name":r.get::<_,String>(2)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(json!(result))
    }
    pub fn explain_query(&self, q: &str) -> Result<Value> {
        let (filter, args) = query::compile(q)?;
        let table = self.query_table(q)?;
        let sql = format!("SELECT id FROM {table} WHERE {filter}");
        let mut s = self.conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let plan = s
            .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                r.get::<_, String>(3)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(
            json!({"sql":sql,"parameters":args.iter().map(|x| match x {rusqlite::types::Value::Integer(n)=>json!(n),rusqlite::types::Value::Real(n)=>json!(n),rusqlite::types::Value::Text(s)=>json!(s),_=>Value::Null}).collect::<Vec<_>>(),"plan":plan}),
        )
    }
    pub fn export_json(&self, path: &Path) -> Result<()> {
        use std::io::Write;
        let mut f = std::io::BufWriter::new(fs::File::create(path)?);
        write!(f, "{{\"volumes\":")?;
        serde_json::to_writer(&mut f, &self.volumes()?).map_err(|e| Error::new("export", e))?;
        write!(f, ",\"entries\":[")?;
        let mut s=self.conn.prepare("SELECT id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,description,tags FROM entries ORDER BY id")?;
        let mut rows = s.query([])?;
        let mut first = true;
        while let Some(r) = rows.next()? {
            if !first {
                write!(f, ",")?
            }
            first = false;
            serde_json::to_writer(&mut f, &entry_row(r)?).map_err(|e| Error::new("export", e))?;
        }
        write!(f, "]}}")?;
        f.flush()?;
        Ok(())
    }
    pub fn export_csv(&self, path: &Path) -> Result<()> {
        let mut w = csv::Writer::from_path(path).map_err(|e| Error::new("export", e))?;
        let mut s=self.conn.prepare("SELECT id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,description,tags FROM entries ORDER BY id")?;
        let mut rows = s.query([])?;
        while let Some(r) = rows.next()? {
            w.serialize(entry_row(r)?)
                .map_err(|e| Error::new("export", e))?;
        }
        w.flush()?;
        Ok(())
    }
}
/// A single-transaction importer. Metadata can be written through `transaction()`;
/// callers must not issue transaction control statements themselves.
pub struct ImportSession<'a> {
    tx: &'a Transaction<'a>,
    current: Option<(Volume, BulkWriter<'a>)>,
}
impl<'a> ImportSession<'a> {
    pub fn transaction(&self) -> &Transaction<'a> {
        self.tx
    }
    pub fn start_volume(&mut self, label: &str, root_path: &str) -> Result<i64> {
        if self.current.is_some() {
            return Err(Error::new(
                "invalid_import",
                "Finish the current volume first",
            ));
        }
        let now = unix_now();
        self.tx.execute("INSERT INTO volumes(label,root_path,scanned_at,entry_count,total_bytes,status) VALUES(?1,?2,?3,0,0,'complete')", params![label,root_path,now])?;
        let id = self.tx.last_insert_rowid();
        let volume = Volume {
            id,
            label: label.into(),
            root_path: root_path.into(),
            scanned_at: now,
            entry_count: 0,
            total_bytes: 0,
            status: "complete".into(),
        };
        self.current = Some((
            volume,
            BulkWriter {
                tx: self.tx,
                volume_id: id,
                count: 0,
                bytes: 0,
                sparse: self
                    .tx
                    .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))?
                    >= 3,
            },
        ));
        Ok(id)
    }
    pub fn insert(&mut self, entry: &NewEntry) -> Result<i64> {
        self.current
            .as_mut()
            .ok_or_else(|| Error::new("invalid_import", "Start a volume first"))?
            .1
            .insert(entry)
    }
    pub fn finish_volume(&mut self) -> Result<Volume> {
        let (mut volume, writer) = self
            .current
            .take()
            .ok_or_else(|| Error::new("invalid_import", "No current volume"))?;
        volume.entry_count = writer.count;
        volume.total_bytes = writer.bytes;
        self.tx.execute(
            "UPDATE volumes SET entry_count=?1,total_bytes=?2 WHERE id=?3",
            params![volume.entry_count, volume.total_bytes, volume.id],
        )?;
        Ok(volume)
    }
}
pub struct BulkWriter<'a> {
    tx: &'a Transaction<'a>,
    pub volume_id: i64,
    count: u64,
    bytes: u64,
    sparse: bool,
}
impl BulkWriter<'_> {
    pub fn insert(&mut self, e: &NewEntry) -> Result<i64> {
        if !matches!(
            e.kind.as_str(),
            "file" | "directory" | "symlink" | "archive"
        ) {
            return Err(Error::new("invalid_entry", "Invalid entry kind"));
        }
        let size = i64::try_from(e.size)
            .map_err(|_| Error::new("invalid_entry", "Entry size is too large"))?;
        if let Some(parent) = e.parent_id {
            let valid:bool=self.tx.prepare_cached("SELECT EXISTS(SELECT 1 FROM entries WHERE id=?1 AND volume_id=?2 AND kind IN ('directory','archive'))")?.query_row(params![parent,self.volume_id],|r|r.get(0))?;
            if !valid {
                return Err(Error::new(
                    "invalid_entry",
                    "Parent does not belong to this volume or cannot contain children",
                ));
            }
        }
        let entry_id = if self.sparse {
            self.tx.prepare_cached("INSERT INTO entries_core(volume_id,parent_id,name,relative_path,kind,size,modified_unix,name_norm,ext) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)")?.execute(params![self.volume_id,e.parent_id,e.name,e.relative_path,e.kind,size,e.modified_unix,e.name.to_lowercase(),Path::new(&e.name).extension().and_then(|x|x.to_str()).unwrap_or("").to_lowercase()])?;
            let entry_id = self.tx.last_insert_rowid();
            if !e.description.is_empty() || !e.tags.is_empty() {
                self.tx
                    .prepare_cached(
                        "INSERT INTO entry_payload(entry_id,description,tags) VALUES(?1,?2,?3)",
                    )?
                    .execute(params![entry_id, e.description, e.tags])?;
            }
            entry_id
        } else {
            self.tx.prepare_cached("INSERT INTO entries(volume_id,parent_id,name,relative_path,kind,size,modified_unix,description,tags,search_text,name_norm,ext) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)")?.execute(params![self.volume_id,e.parent_id,e.name,e.relative_path,e.kind,size,e.modified_unix,e.description,e.tags,search_text(&e.name,&e.relative_path,&e.description,&e.tags),e.name.to_lowercase(),Path::new(&e.name).extension().and_then(|x|x.to_str()).unwrap_or("").to_lowercase()])?;
            self.tx.last_insert_rowid()
        };
        self.count += 1;
        if e.kind == "file" {
            self.bytes = self
                .bytes
                .checked_add(e.size)
                .ok_or_else(|| Error::new("invalid_entry", "Total size overflow"))?;
        }
        if self.bytes > i64::MAX as u64 {
            return Err(Error::new("invalid_entry", "Total size is too large"));
        }
        Ok(entry_id)
    }
}
fn search_text(n: &str, p: &str, d: &str, t: &str) -> String {
    format!("{n}\n{p}\n{d}\n{t}").to_lowercase()
}
pub fn entry_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: r.get(0)?,
        volume_id: r.get(1)?,
        parent_id: r.get(2)?,
        name: r.get(3)?,
        relative_path: r.get(4)?,
        kind: r.get(5)?,
        size: r.get(6)?,
        modified_unix: r.get(7)?,
        description: r.get(8)?,
        tags: r.get(9)?,
    })
}
fn to_unix(t: SystemTime) -> Option<i64> {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).ok(),
        Err(e) => i64::try_from(e.duration().as_secs()).ok().map(|v| -v),
    }
}
fn unix_now() -> i64 {
    to_unix(SystemTime::now()).unwrap_or(0)
}
#[derive(Default)]
struct Job {
    state: AtomicU8,
    count: AtomicU64,
}
static JOBS: OnceLock<Mutex<HashMap<String, Arc<Job>>>> = OnceLock::new();
fn jobs() -> &'static Mutex<HashMap<String, Arc<Job>>> {
    JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}
struct JobGuard(Option<String>);
impl JobGuard {
    fn register(id: Option<&str>, job: Arc<Job>) -> Result<Self> {
        if let Some(id) = id {
            if id.is_empty() {
                return Err(Error::new("invalid_request", "job_id cannot be empty"));
            }
            let mut m = jobs()
                .lock()
                .map_err(|_| Error::new("internal", "Job lock failed"))?;
            if m.contains_key(id) {
                return Err(Error::new("job_exists", "Job already running"));
            }
            m.insert(id.to_string(), job);
            Ok(Self(Some(id.into())))
        } else {
            Ok(Self(None))
        }
    }
}
impl Drop for JobGuard {
    fn drop(&mut self) {
        if let Some(id) = &self.0 {
            if let Ok(mut m) = jobs().lock() {
                m.remove(id);
            }
        }
    }
}
pub fn cancel_job(id: &str) -> Result<()> {
    let m = jobs()
        .lock()
        .map_err(|_| Error::new("internal", "Job lock failed"))?;
    let j = m
        .get(id)
        .ok_or_else(|| Error::new("not_found", "Job not running"))?;
    match j
        .state
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
    {
        Ok(_) | Err(1) => Ok(()),
        _ => Err(Error::new("too_late", "Scan is already committing")),
    }
}
pub fn progress_job(id: &str) -> Result<Value> {
    let m = jobs()
        .lock()
        .map_err(|_| Error::new("internal", "Job lock failed"))?;
    let j = m
        .get(id)
        .ok_or_else(|| Error::new("not_found", "Job not running"))?;
    Ok(
        json!({"job_id":id,"entries":j.count.load(Ordering::Relaxed),"cancelled":j.state.load(Ordering::Acquire) == 1}),
    )
}

pub mod api {
    use super::*;
    fn string<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
        v.get(k)
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("invalid_request", format!("{k} must be a string")))
    }
    fn integer(v: &Value, k: &str) -> Result<i64> {
        v.get(k)
            .and_then(Value::as_i64)
            .ok_or_else(|| Error::new("invalid_request", format!("{k} must be an integer")))
    }
    fn opt_integer(v: &Value, k: &str) -> Result<Option<i64>> {
        match v.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(x) => x.as_i64().map(Some).ok_or_else(|| {
                Error::new("invalid_request", format!("{k} must be an integer or null"))
            }),
        }
    }
    fn page(v: &Value, k: &str, default: u64) -> Result<u64> {
        match v.get(k) {
            None => Ok(default),
            Some(x) => x.as_u64().ok_or_else(|| {
                Error::new(
                    "invalid_request",
                    format!("{k} must be a non-negative integer"),
                )
            }),
        }
    }
    pub fn dispatch_json(input: &str) -> String {
        let result = std::panic::catch_unwind(|| dispatch(input));
        match result {
            Ok(Ok(data)) => json!({"ok":true,"data":data}).to_string(),
            Ok(Err(e)) => {
                json!({"ok":false,"error":{"code":e.code,"message":e.message}}).to_string()
            }
            Err(_) => json!({"ok":false,"error":{"code":"internal","message":"Unexpected error"}})
                .to_string(),
        }
    }
    fn dispatch(input: &str) -> Result<Value> {
        let v: Value = serde_json::from_str(input).map_err(|e| Error::new("invalid_json", e))?;
        let cmd = string(&v, "op")
            .or_else(|_| string(&v, "command"))
            .or_else(|_| string(&v, "cmd"))?;
        if cmd == "cancel" {
            cancel_job(string(&v, "job_id")?)?;
            return Ok(json!({}));
        }
        if cmd == "progress" {
            return progress_job(string(&v, "job_id")?);
        }
        let db = string(&v, "db")?;
        let mut c = Catalog::open(db)?;
        match cmd {
            "categories" => c.categories(),
            "create_category" => Ok(
                json!({"id":c.create_category(string(&v,"name")?,opt_integer(&v,"parent_id")?)?}),
            ),
            "assign_category" => {
                c.assign_category(integer(&v, "entry_id")?, integer(&v, "category_id")?)?;
                Ok(json!({}))
            }
            "explain_query" => c.explain_query(string(&v, "query")?),
            "init" => Ok(
                json!({"schema_version":c.conn().pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0))?}),
            ),
            "stats" => c.stats(),
            "volumes" => Ok(json!(c.volumes()?)),
            "children" => Ok(json!(c.children(
                integer(&v, "volume_id")?,
                opt_integer(&v, "parent_id")?,
                page(&v, "offset", 0)?,
                page(&v, "limit", 100)?
            )?)),
            "search" => Ok(json!(c.search(
                string(&v, "query")?,
                opt_integer(&v, "volume_id")?,
                page(&v, "offset", 0)?,
                page(&v, "limit", 100)?
            )?)),
            "search_fast" => Ok(json!(c.search_fast(
                string(&v, "query")?,
                opt_integer(&v, "volume_id")?,
                page(&v, "offset", 0)?,
                page(&v, "limit", 100)?
            )?)),
            "scan" => {
                let root = PathBuf::from(string(&v, "root")?);
                let label = string(&v, "label")?;
                let job =
                    match v.get("job_id") {
                        None | Some(Value::Null) => None,
                        Some(x) => Some(x.as_str().ok_or_else(|| {
                            Error::new("invalid_request", "job_id must be a string")
                        })?),
                    };
                if v.get("hash").and_then(Value::as_bool) == Some(true) {
                    return Err(Error::new(
                        "unsupported",
                        "Content hashing is not implemented",
                    ));
                }
                Ok(json!(c.scan(&root, label, job)?))
            }
            "export_json" => {
                c.export_json(Path::new(string(&v, "path")?))?;
                Ok(json!({}))
            }
            "export_csv" => {
                c.export_csv(Path::new(string(&v, "path")?))?;
                Ok(json!({}))
            }
            "update_entry" => {
                let tags = match v.get("tags") {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Array(a)) => a
                        .iter()
                        .map(|x| {
                            x.as_str().ok_or_else(|| {
                                Error::new("invalid_request", "tags must contain strings")
                            })
                        })
                        .collect::<Result<Vec<_>>>()?
                        .join(", "),
                    None => String::new(),
                    _ => {
                        return Err(Error::new(
                            "invalid_request",
                            "tags must be a string or string array",
                        ))
                    }
                };
                c.update_entry(integer(&v, "entry_id")?, string(&v, "description")?, &tags)?;
                Ok(json!({}))
            }
            _ => Err(Error::new(
                "unknown_command",
                format!("Unknown command: {cmd}"),
            )),
        }
    }
}
