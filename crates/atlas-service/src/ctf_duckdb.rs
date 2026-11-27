//! In-memory DuckDB query engine over borrowed CTF metadata.
//!
//! The catalog remains owned by the caller. This module exposes it through a
//! read-only DuckDB table function; it never stages records in SQLite or files.

use atlas_core::{
    query::{MetadataGlobPattern, MetadataQuery},
    Error, Result,
};
use ctf_format::{EntryKind, MetadataCatalog, MetadataDisk};
use duckdb::{
    core::{DataChunkHandle, Inserter, LogicalTypeHandle, LogicalTypeId},
    params_from_iter,
    types::{DuckString, Value as DuckValue},
    vscalar::{ScalarFunctionSignature, VScalar},
    vtab::{BindInfo, InitInfo, TableFunctionInfo, VTab},
    Config, Connection,
};
use std::{
    collections::HashMap,
    io,
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
    sync::{
        atomic::{AtomicI64, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock, RwLock, Weak,
    },
};

type Checkpoint = Arc<dyn Fn(u64) -> Result<()> + Send + Sync>;
const VECTOR_SIZE: usize = 2048;
const MAX_PAGE: u64 = 1000;

#[derive(Debug)]
pub(super) struct DuckQueryResult {
    pub ids: Vec<i64>,
    pub total: Option<u64>,
    pub has_more: bool,
}

pub(super) type Engine = DuckEngine;

struct Source {
    catalog: Arc<MetadataCatalog>,
    id_bases: Vec<i64>,
    disk_ends: Vec<usize>,
    rows: usize,
    checkpoint: Mutex<Option<Checkpoint>>,
    checkpoint_error: Mutex<Option<Error>>,
}

static NEXT_TOKEN: AtomicI64 = AtomicI64::new(1);
static SOURCES: OnceLock<Mutex<HashMap<i64, Weak<Source>>>> = OnceLock::new();

fn sources() -> &'static Mutex<HashMap<i64, Weak<Source>>> {
    SOURCES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn boxed_error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(io::Error::other(message.into()))
}

fn checked<T, F>(f: F) -> std::result::Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce() -> std::result::Result<T, Box<dyn std::error::Error>>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(_) => Err(boxed_error("DuckDB CTF callback failed safely")),
    }
}

struct CtfRows;

struct Cursor {
    source: Arc<Source>,
    projected: Vec<usize>,
    next: AtomicUsize,
    checkpoint: Option<Checkpoint>,
}

impl VTab for CtfRows {
    type InitData = Cursor;
    type BindData = Arc<Source>;

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeId::Bigint.into()])
    }

    fn supports_pushdown() -> bool {
        true
    }

    fn bind(bind: &BindInfo) -> std::result::Result<Self::BindData, Box<dyn std::error::Error>> {
        checked(|| bind_inner(bind))
    }

    fn init(init: &InitInfo) -> std::result::Result<Self::InitData, Box<dyn std::error::Error>> {
        checked(|| {
            // SAFETY: DuckDB's VTab wrapper boxes the Arc returned by bind_inner.
            let source = unsafe { &*init.get_bind_data::<Arc<Source>>() }.clone();
            init.set_max_threads(4);
            let checkpoint = source
                .checkpoint
                .lock()
                .map_err(|_| boxed_error("DuckDB checkpoint lock poisoned"))?
                .clone();
            Ok(Cursor {
                source,
                projected: init
                    .get_column_indices()
                    .into_iter()
                    .map(|column| column as usize)
                    .collect(),
                next: AtomicUsize::new(0),
                checkpoint,
            })
        })
    }

    fn func(
        func: &TableFunctionInfo<Self>,
        output: &mut DataChunkHandle,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        checked(|| fill_chunk(func.get_init_data(), output))
    }
}

fn bind_inner(bind: &BindInfo) -> std::result::Result<Arc<Source>, Box<dyn std::error::Error>> {
    let token = bind.get_parameter(0).to_int64();
    let source = sources()
        .lock()
        .map_err(|_| boxed_error("DuckDB CTF registry lock poisoned"))?
        .get(&token)
        .and_then(Weak::upgrade)
        .ok_or_else(|| boxed_error("CTF metadata snapshot expired"))?;

    for (name, ty) in [
        ("id", LogicalTypeId::Bigint),
        ("volume_id", LogicalTypeId::Bigint),
        ("parent_id", LogicalTypeId::Bigint),
        ("name_norm", LogicalTypeId::Varchar),
        ("path_norm", LogicalTypeId::Varchar),
        ("description_norm", LogicalTypeId::Varchar),
        ("tags_norm", LogicalTypeId::Varchar),
        ("ext", LogicalTypeId::Varchar),
        ("kind", LogicalTypeId::Varchar),
        ("size", LogicalTypeId::UBigint),
        ("sort_name", LogicalTypeId::Varchar),
    ] {
        bind.add_result_column(name, ty.into());
    }
    bind.set_cardinality(source.rows as u64, true);
    Ok(source)
}

fn fill_chunk(
    cursor: &Cursor,
    output: &mut DataChunkHandle,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let start = cursor.next.fetch_add(VECTOR_SIZE, Ordering::AcqRel);
    if start >= cursor.source.rows {
        output.set_len(0);
        return Ok(());
    }
    let end = start.saturating_add(VECTOR_SIZE).min(cursor.source.rows);
    report_checkpoint(cursor, 0)?;

    let count = end - start;
    for (output_column, source_column) in cursor.projected.iter().copied().enumerate() {
        let mut vector = output.flat_vector(output_column);
        match source_column {
            0 => {
                let values = vector.as_mut_slice::<i64>();
                for (row, target) in values.iter_mut().take(count).enumerate() {
                    let (disk, index) = source_row(&cursor.source, start + row)?;
                    *target = cursor.source.id_bases[disk] + index as i64;
                }
            }
            1 => {
                let values = vector.as_mut_slice::<i64>();
                for (row, target) in values.iter_mut().take(count).enumerate() {
                    let (disk, _) = source_row(&cursor.source, start + row)?;
                    *target = disk as i64 + 1;
                }
            }
            2 => {
                let mut null_rows = Vec::new();
                {
                    let values = vector.as_mut_slice::<i64>();
                    for (row, target) in values.iter_mut().take(count).enumerate() {
                        let (disk_index, entry_index) = source_row(&cursor.source, start + row)?;
                        let disk = &cursor.source.catalog.disks[disk_index];
                        match disk.entries[entry_index].parent {
                            Some(parent) if parent < disk.entries.len() => {
                                *target = cursor.source.id_bases[disk_index] + parent as i64;
                            }
                            Some(_) => return Err(boxed_error("CTF parent reference is invalid")),
                            None => null_rows.push(row),
                        }
                    }
                }
                for row in null_rows {
                    vector.set_null(row);
                }
            }
            3 | 4 | 5 | 6 | 7 | 8 | 10 => {
                for row in 0..count {
                    let (disk_index, entry_index) = source_row(&cursor.source, start + row)?;
                    let disk = &cursor.source.catalog.disks[disk_index];
                    let entry = &disk.entries[entry_index];
                    let text = match source_column {
                        3 => entry.name.to_lowercase(),
                        4 => relative_path(disk, entry_index)?.to_lowercase(),
                        5 => entry.description.to_lowercase(),
                        6 => String::new(),
                        7 => Path::new(&entry.name)
                            .extension()
                            .and_then(|extension| extension.to_str())
                            .unwrap_or("")
                            .to_lowercase(),
                        8 => match entry.kind {
                            EntryKind::File => "file".into(),
                            EntryKind::Directory => "directory".into(),
                            EntryKind::Archive => "archive".into(),
                        },
                        10 => entry.name.to_ascii_lowercase(),
                        _ => unreachable!(),
                    };
                    vector.insert(row, text.as_bytes());
                }
            }
            9 => {
                let values = vector.as_mut_slice::<u64>();
                for (row, target) in values.iter_mut().take(count).enumerate() {
                    let (disk, index) = source_row(&cursor.source, start + row)?;
                    *target = cursor.source.catalog.disks[disk].entries[index].size;
                }
            }
            usize::MAX => {
                let values = vector.as_mut_slice::<i64>();
                for (row, target) in values.iter_mut().take(count).enumerate() {
                    let (disk, index) = source_row(&cursor.source, start + row)?;
                    *target = cursor.source.id_bases[disk] + index as i64;
                }
            }
            _ => return Err(boxed_error("DuckDB requested an unknown CTF column")),
        }
    }
    output.set_len(count);
    report_checkpoint(cursor, count as u64)?;
    Ok(())
}

fn report_checkpoint(
    cursor: &Cursor,
    delta: u64,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    if let Some(checkpoint) = &cursor.checkpoint {
        if let Err(error) = checkpoint(delta) {
            *cursor
                .source
                .checkpoint_error
                .lock()
                .map_err(|_| boxed_error("DuckDB checkpoint error lock poisoned"))? =
                Some(Error::new(error.code, error.message.clone()));
            return Err(boxed_error(error.message));
        }
    }
    Ok(())
}

fn source_row(
    source: &Source,
    row: usize,
) -> std::result::Result<(usize, usize), Box<dyn std::error::Error>> {
    if row >= source.rows {
        return Err(boxed_error("DuckDB CTF row is out of range"));
    }
    let disk = source.disk_ends.partition_point(|end| *end <= row);
    let start = if disk == 0 {
        0
    } else {
        source.disk_ends[disk - 1]
    };
    Ok((disk, row - start))
}

fn relative_path(
    disk: &MetadataDisk,
    entry_index: usize,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let mut components = Vec::new();
    let mut cursor = Some(entry_index);
    while let Some(index) = cursor {
        let entry = disk
            .entries
            .get(index)
            .ok_or_else(|| boxed_error("CTF parent reference is invalid"))?;
        components.push(entry.name.as_str());
        cursor = entry.parent;
        if components.len() > disk.entries.len() {
            return Err(boxed_error("CTF parent cycle detected"));
        }
    }
    components.reverse();
    Ok(components.join("/"))
}

#[derive(Default)]
struct GlobCache(RwLock<HashMap<String, Arc<MetadataGlobPattern>>>);

struct AtlasGlob;

impl VScalar for AtlasGlob {
    type State = Arc<GlobCache>;

    unsafe fn invoke(
        state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn duckdb::vtab::arrow::WritableVector,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        checked(|| {
            let value_vector = input.flat_vector(0);
            let pattern_vector = input.flat_vector(1);
            let values =
                value_vector.as_slice_with_len::<duckdb::ffi::duckdb_string_t>(input.len());
            let patterns =
                pattern_vector.as_slice_with_len::<duckdb::ffi::duckdb_string_t>(input.len());
            let mut output_vector = output.flat_vector();
            let mut null_rows = Vec::new();
            {
                let results = output_vector.as_mut_slice::<bool>();
                for row in 0..input.len() {
                    if value_vector.row_is_null(row as u64)
                        || pattern_vector.row_is_null(row as u64)
                    {
                        null_rows.push(row);
                        continue;
                    }
                    let mut value_bytes = values[row];
                    let mut pattern_bytes = patterns[row];
                    let value = DuckString::new(&mut value_bytes).as_str();
                    let pattern = DuckString::new(&mut pattern_bytes).as_str();
                    let compiled = {
                        let cache = state
                            .0
                            .read()
                            .map_err(|_| boxed_error("GLOB cache lock poisoned"))?;
                        cache.get(pattern.as_ref()).cloned()
                    }
                    .unwrap_or_else(|| {
                        let mut cache = state.0.write().unwrap_or_else(|p| p.into_inner());
                        cache
                            .entry(pattern.to_string())
                            .or_insert_with(|| Arc::new(MetadataGlobPattern::new(&pattern)))
                            .clone()
                    });
                    results[row] = compiled.is_match(&value);
                }
            }
            for row in null_rows {
                output_vector.set_null(row);
            }
            Ok(())
        })
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![LogicalTypeId::Varchar.into(), LogicalTypeId::Varchar.into()],
            LogicalTypeId::Boolean.into(),
        )]
    }
}

pub(super) struct DuckEngine {
    conn: Connection,
    token: i64,
    source: Arc<Source>,
    _glob_cache: Arc<GlobCache>,
}

impl DuckEngine {
    pub(super) fn new(catalog: Arc<MetadataCatalog>) -> Result<Self> {
        let mut id_bases = Vec::with_capacity(catalog.disks.len());
        let mut disk_ends = Vec::with_capacity(catalog.disks.len());
        let mut next_id = 1i64;
        let mut rows = 0usize;
        for disk in &catalog.disks {
            id_bases.push(next_id);
            rows = rows
                .checked_add(disk.entries.len())
                .ok_or_else(|| Error::new("ctf_limit", "CTF row count is too large"))?;
            next_id = next_id
                .checked_add(disk.entries.len() as i64)
                .ok_or_else(|| Error::new("ctf_limit", "CTF row IDs exceed the supported range"))?;
            disk_ends.push(rows);
        }
        let source = Arc::new(Source {
            catalog,
            id_bases,
            disk_ends,
            rows,
            checkpoint: Mutex::new(None),
            checkpoint_error: Mutex::new(None),
        });
        let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
        sources()
            .lock()
            .map_err(|_| Error::new("duckdb", "CTF registry lock poisoned"))?
            .insert(token, Arc::downgrade(&source));
        let initialized =
            (|| -> std::result::Result<(Connection, Arc<GlobCache>), duckdb::Error> {
                let config = Config::default()
                    .threads(4)?
                    .max_memory("512MB")?
                    .with("temp_directory", "")?
                    .enable_external_access(false)?
                    .enable_autoload_extension(false)?;
                let conn = Connection::open_in_memory_with_flags(config)?;
                let (temp_directory, external_access, autoload, autoinstall): (
                    String,
                    bool,
                    bool,
                    bool,
                ) = conn.query_row(
                    "SELECT current_setting('temp_directory'), current_setting('enable_external_access'), current_setting('autoload_known_extensions'), current_setting('autoinstall_known_extensions')",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?;
                if !temp_directory.is_empty() || external_access || autoload || autoinstall {
                    return Err(duckdb::Error::InvalidQuery);
                }
                conn.register_table_function::<CtfRows>("atlas_ctf_rows")?;
                let glob_cache = Arc::new(GlobCache::default());
                conn.register_scalar_function_with_state::<AtlasGlob>("atlas_glob", &glob_cache)?;
                conn.execute_batch(&format!(
                    "CREATE VIEW ctf_rows AS SELECT * FROM atlas_ctf_rows({token})"
                ))?;
                Ok((conn, glob_cache))
            })();
        match initialized {
            Ok((conn, glob_cache)) => Ok(Self {
                conn,
                token,
                source,
                _glob_cache: glob_cache,
            }),
            Err(error) => {
                sources()
                    .lock()
                    .ok()
                    .map(|mut registry| registry.remove(&token));
                Err(Error::new("duckdb", error.to_string()))
            }
        }
    }

    pub(super) fn query(
        &mut self,
        query: &MetadataQuery,
        volume_id: Option<i64>,
        offset: u64,
        limit: u64,
        full_count: bool,
        checkpoint: Checkpoint,
    ) -> Result<DuckQueryResult> {
        *self
            ._glob_cache
            .0
            .write()
            .map_err(|_| Error::new("duckdb", "GLOB cache lock poisoned"))? = HashMap::new();
        checkpoint(0)?;
        *self
            .source
            .checkpoint_error
            .lock()
            .map_err(|_| Error::new("duckdb", "Checkpoint error lock poisoned"))? = None;
        *self
            .source
            .checkpoint
            .lock()
            .map_err(|_| Error::new("duckdb", "Checkpoint lock poisoned"))? =
            Some(checkpoint.clone());

        let outcome = self.query_inner(query, volume_id, offset, limit, full_count);
        *self
            .source
            .checkpoint
            .lock()
            .map_err(|_| Error::new("duckdb", "Checkpoint lock poisoned"))? = None;
        match outcome {
            Ok(result) => {
                checkpoint(0)?;
                Ok(result)
            }
            Err(error) => {
                if let Some(checkpoint_error) = self
                    .source
                    .checkpoint_error
                    .lock()
                    .map_err(|_| Error::new("duckdb", "Checkpoint error lock poisoned"))?
                    .take()
                {
                    Err(checkpoint_error)
                } else {
                    Err(error)
                }
            }
        }
    }

    fn query_inner(
        &mut self,
        query: &MetadataQuery,
        volume_id: Option<i64>,
        offset: u64,
        limit: u64,
        full_count: bool,
    ) -> Result<DuckQueryResult> {
        let emitted = query.duckdb_predicate();
        let mut where_sql = format!("({})", emitted.sql);
        let mut args = emitted.args.into_iter().map(duck_value).collect::<Vec<_>>();
        if let Some(volume_id) = volume_id {
            where_sql.push_str(" AND volume_id = ?");
            args.push(DuckValue::BigInt(volume_id));
        }

        let total = if full_count {
            let sql = format!("SELECT COUNT(*) FROM ctf_rows WHERE {where_sql}");
            let count: i64 = self
                .conn
                .query_row(&sql, params_from_iter(args.iter()), |row| row.get(0))
                .map_err(|error| self.duck_error(error))?;
            Some(u64::try_from(count).map_err(|_| Error::new("duckdb", "Count is out of range"))?)
        } else {
            None
        };

        let page_limit = limit.clamp(1, MAX_PAGE);
        let offset = offset.min(i64::MAX as u64);
        let fetch_limit = if full_count {
            page_limit
        } else {
            page_limit + 1
        };
        let sql = format!(
            "SELECT id FROM ctf_rows WHERE {where_sql} ORDER BY sort_name, id LIMIT ? OFFSET ?"
        );
        let mut page_args = args;
        page_args.push(DuckValue::UBigInt(fetch_limit));
        page_args.push(DuckValue::UBigInt(offset));
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|error| self.duck_error(error))?;
        let ids = stmt
            .query_map(params_from_iter(page_args.iter()), |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| self.duck_error(error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| self.duck_error(error))?;
        let mut ids = ids;
        let has_more = if full_count {
            offset.saturating_add(ids.len() as u64) < total.unwrap_or(0)
        } else {
            let more = ids.len() > page_limit as usize;
            if more {
                ids.pop();
            }
            more
        };
        Ok(DuckQueryResult {
            ids,
            total,
            has_more,
        })
    }

    fn duck_error(&self, error: duckdb::Error) -> Error {
        self.source
            .checkpoint_error
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
            .unwrap_or_else(|| Error::new("duckdb", error.to_string()))
    }
}

impl Drop for DuckEngine {
    fn drop(&mut self) {
        if let Ok(mut registry) = sources().lock() {
            registry.remove(&self.token);
        }
    }
}

fn duck_value(value: rusqlite::types::Value) -> DuckValue {
    match value {
        rusqlite::types::Value::Null => DuckValue::Null,
        rusqlite::types::Value::Integer(value) => DuckValue::BigInt(value),
        rusqlite::types::Value::Real(value) => DuckValue::Double(value),
        rusqlite::types::Value::Text(value) => DuckValue::Text(value),
        rusqlite::types::Value::Blob(value) => DuckValue::Blob(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_core::query::MetadataQuery;
    use ctf_format::{MetadataCatalog, MetadataDisk, MetadataEntry};

    fn entry(name: String, parent: Option<usize>, kind: EntryKind) -> MetadataEntry {
        MetadataEntry {
            name,
            description: String::new(),
            parent,
            kind,
            size: 1,
            attributes: 0,
            modified_dos: 0,
            created_dos: 0,
            accessed_dos: 0,
        }
    }

    fn checkpoint() -> Checkpoint {
        Arc::new(|_| Ok(()))
    }

    #[test]
    fn native_engine_handles_parent_nulls_across_batches_and_branch_queries() {
        let mut entries = Vec::with_capacity(2050);
        for pair in 0..1023 {
            let root_index = entries.len();
            entries.push(entry(format!("root-{pair:04}"), None, EntryKind::Directory));
            entries.push(entry(
                format!("child-{pair:04}.txt"),
                Some(root_index),
                EntryKind::File,
            ));
        }
        let last_root = entries.len();
        entries.push(entry("root-1023".into(), None, EntryKind::Directory));
        entries.push(entry(
            "child-1023a.txt".into(),
            Some(last_root),
            EntryKind::File,
        ));
        entries.push(entry(
            "child-1023b.txt".into(),
            Some(last_root),
            EntryKind::File,
        ));
        entries.push(entry("root-1024".into(), None, EntryKind::Directory));
        let catalog = Arc::new(MetadataCatalog {
            name: "synthetic".into(),
            opaque_trailer: vec![],
            disks: vec![MetadataDisk {
                name: "disk".into(),
                opaque_header: vec![],
                entries,
            }],
        });
        let mut engine = DuckEngine::new(catalog).unwrap();

        let files = engine
            .query(
                &MetadataQuery::compile_metadata("kind:file").unwrap(),
                None,
                0,
                5,
                true,
                checkpoint(),
            )
            .unwrap();
        assert_eq!(files.total, Some(1025));
        assert_eq!(files.ids, vec![2, 4, 6, 8, 10]);
        assert!(files.has_more);

        let branch = engine
            .query(
                &MetadataQuery::compile_metadata("branch_id:1").unwrap(),
                None,
                0,
                10,
                true,
                checkpoint(),
            )
            .unwrap();
        assert_eq!(branch.total, Some(1));
        assert_eq!(branch.ids, vec![2]);
        assert!(!branch.has_more);
    }

    #[test]
    fn native_engine_counts_an_empty_catalog() {
        let catalog = Arc::new(MetadataCatalog {
            name: "empty".into(),
            opaque_trailer: vec![],
            disks: Vec::new(),
        });
        let mut engine = DuckEngine::new(catalog).unwrap();
        let result = engine
            .query(
                &MetadataQuery::compile_metadata("").unwrap(),
                None,
                0,
                10,
                true,
                checkpoint(),
            )
            .unwrap();
        assert_eq!(result.total, Some(0));
        assert!(result.ids.is_empty());
        assert!(!result.has_more);
    }

    #[test]
    fn native_glob_and_unicode_match_the_shared_sqlite_semantics() {
        let entries = [
            entry("ΟΣ".into(), None, EntryKind::File),
            entry("İstanbul.TXT".into(), None, EntryKind::File),
            entry("ёжик.txt".into(), None, EntryKind::File),
            entry("]bracket.txt".into(), None, EntryKind::File),
            entry("a\0b.txt".into(), None, EntryKind::File),
        ]
        .into();
        let catalog = Arc::new(MetadataCatalog {
            name: "unicode-glob".into(),
            opaque_trailer: vec![],
            disks: vec![MetadataDisk {
                name: "disk".into(),
                opaque_header: vec![],
                entries,
            }],
        });
        let mut engine = DuckEngine::new(catalog).unwrap();

        for (query, expected) in [
            ("name:ος", vec![1]),
            ("name_exact:ΟΣ", vec![1]),
            ("name_exact:İSTANBUL.TXT", vec![2]),
            ("name:ё", vec![3]),
            ("name:[]]*", vec![4]),
            ("name:[^z]*", vec![4, 5, 2, 1, 3]),
            ("name:[unclosed", vec![]),
            ("name:a*", vec![5]),
        ] {
            let got = engine
                .query(
                    &MetadataQuery::compile_metadata(query).unwrap(),
                    None,
                    0,
                    100,
                    true,
                    checkpoint(),
                )
                .unwrap();
            assert_eq!(got.ids, expected, "query={query}");
            assert_eq!(got.total, Some(expected.len() as u64), "query={query}");
        }
    }

    #[test]
    fn native_engine_propagates_cancellation_and_reuses_engine() {
        let entries = (0..5000)
            .map(|index| entry(format!("entry-{index:05}.txt"), None, EntryKind::File))
            .collect();
        let catalog = Arc::new(MetadataCatalog {
            name: "cancel".into(),
            opaque_trailer: vec![],
            disks: vec![MetadataDisk {
                name: "disk".into(),
                opaque_header: vec![],
                entries,
            }],
        });
        let mut engine = DuckEngine::new(catalog).unwrap();
        let processed = Arc::new(AtomicUsize::new(0));
        let progress = processed.clone();
        let cancel = Arc::new(move |delta: u64| {
            let total = progress.fetch_add(delta as usize, Ordering::AcqRel) + delta as usize;
            if total >= VECTOR_SIZE {
                Err(Error::new("cancelled", "synthetic cancellation"))
            } else {
                Ok(())
            }
        });
        let failed = engine.query(
            &MetadataQuery::compile_metadata("kind:file").unwrap(),
            None,
            0,
            10,
            true,
            cancel,
        );
        assert!(failed.is_err());
        assert_eq!(failed.unwrap_err().code, "cancelled");

        let scanned = Arc::new(AtomicUsize::new(0));
        let observer = scanned.clone();
        let result = engine
            .query(
                &MetadataQuery::compile_metadata("kind:file").unwrap(),
                None,
                0,
                10,
                false,
                Arc::new(move |delta| {
                    observer.fetch_add(delta as usize, Ordering::AcqRel);
                    Ok(())
                }),
            )
            .unwrap();
        assert_eq!(result.ids.len(), 10);
        assert!(result.has_more);
        assert_eq!(scanned.load(Ordering::Acquire), 5000);
    }
}
