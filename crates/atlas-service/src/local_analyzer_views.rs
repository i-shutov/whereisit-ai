//! Bounded views over an immutable, private local-analysis snapshot.
use super::*;
use rusqlite::OpenFlags;
use std::collections::VecDeque;

fn snapshot_state(j: &Job) -> Result<(Arc<Snapshot>, Value, Option<String>)> {
    check(j)?;
    let s = j
        .inner
        .lock()
        .map_err(|_| err("internal", "analyzer job poisoned"))?;
    if s.phase != COMPLETE {
        return Err(err("not_ready", format!("analysis phase is {}", s.phase)));
    }
    let snapshot = s
        .snapshot
        .clone()
        .ok_or_else(|| err("not_ready", "snapshot unavailable"))?;
    Ok((
        snapshot,
        json!({"files":s.files,"dirs":s.dirs,"logical_bytes":s.bytes,
        "read_errors":s.errors,"symlinks_skipped":s.skipped_links,"complete":s.errors==0}),
        s.catalog_generation.clone(),
    ))
}

struct ActiveConnection {
    connection: Connection,
    _interrupt: Arc<rusqlite::InterruptHandle>,
}
impl std::ops::Deref for ActiveConnection {
    type Target = Connection;
    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}
fn connect(snapshot: &Snapshot, j: &Job) -> Result<ActiveConnection> {
    let c = Connection::open_with_flags(&snapshot.db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    c.busy_timeout(std::time::Duration::from_secs(2))?;
    c.execute_batch("PRAGMA temp_store=FILE;")?;
    let interrupt = register_interrupt(j, &c)?;
    check(j)?;
    c.create_scalar_function(
        "analyzer_lower",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| Ok(ctx.get::<String>(0)?.to_lowercase()),
    )?;
    c.create_scalar_function(
        "analyzer_class",
        3,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            Ok(classification(
                &ctx.get::<String>(0)?,
                &ctx.get::<String>(1)?,
                ctx.get(2)?,
            ))
        },
    )?;
    Ok(ActiveConnection {
        connection: c,
        _interrupt: interrupt,
    })
}

fn classification(path: &str, ext: &str, size: i64) -> &'static str {
    let parts: Vec<_> = path.split('/').map(str::to_lowercase).collect();
    if parts.iter().any(|p| {
        matches!(
            p.as_str(),
            "node_modules" | ".venv" | "venv" | "dist" | "target" | "build"
        )
    }) {
        "dev"
    } else if parts
        .iter()
        .any(|p| p.contains("cache") || matches!(p.as_str(), "temp" | "tmp"))
        || matches!(ext, "tmp" | "temp" | "log" | "dmp")
    {
        "cache"
    } else if matches!(ext, "dmg" | "pkg" | "exe" | "msi" | "iso" | "ova") {
        "installers"
    } else if size >= 50 * 1024 * 1024 && matches!(ext, "zip" | "tar" | "gz" | "tgz" | "7z" | "rar")
    {
        "archives"
    } else {
        "other"
    }
}

fn number(v: &Value, key: &str, default: u64, max: u64) -> Result<u64> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(x) => x
            .as_u64()
            .filter(|n| *n <= max)
            .ok_or_else(|| err("invalid_request", format!("invalid {key}"))),
    }
}

fn folder(v: &Value) -> Result<i64> {
    match v.get("folder_id") {
        None | Some(Value::Null) => Ok(1),
        Some(Value::String(s)) if s == "root" => Ok(1),
        Some(Value::String(s)) => parse_folder_id(s),
        _ => Err(err(
            "invalid_request",
            "folder_id must be root or folder:<id>",
        )),
    }
}

fn eligible(c: &Connection, v: &Value, root: i64, j: &Job) -> Result<()> {
    check(j)?;
    let filter = match v.get("path_contains") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) if s.len() <= 4096 => s.to_lowercase(),
        _ => {
            return Err(err(
                "invalid_request",
                "path_contains must be a bounded string",
            ));
        }
    };
    let min = number(v, "min_size", 0, i64::MAX as u64)? as i64;
    let age = number(v, "min_age_days", 0, 365_000)? as i64;
    let cutoff = if age == 0 {
        None
    } else {
        Some(now_unix().saturating_sub(age * 86400))
    };
    let exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM folders WHERE id=?1)",
        [root],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(err("not_found", "folder does not belong to this snapshot"));
    }
    c.execute("CREATE TEMP TABLE eligible AS WITH RECURSIVE branch(id) AS (SELECT ?1 UNION ALL SELECT d.id FROM folders d JOIN branch b ON d.parent_id=b.id) SELECT f.* FROM files f JOIN branch b ON b.id=f.folder_id WHERE f.size>=?2 AND (?3='' OR instr(analyzer_lower(f.path),?3)>0) AND (?4 IS NULL OR f.mtime<=?4)", params![root,min,filter,cutoff])?;
    check(j)?;
    c.execute_batch("CREATE INDEX temp.eligible_folder ON eligible(folder_id); CREATE INDEX temp.eligible_name ON eligible(norm_name,size);
        CREATE TEMP TABLE rollup AS WITH RECURSIVE chain(folder_id,bytes) AS (
            SELECT folder_id,size FROM eligible UNION ALL
            SELECT d.parent_id,c.bytes FROM folders d JOIN chain c ON d.id=c.folder_id WHERE d.parent_id IS NOT NULL
        ) SELECT folder_id,SUM(bytes) AS bytes,COUNT(*) AS files FROM chain GROUP BY folder_id;
        CREATE UNIQUE INDEX temp.rollup_id ON rollup(folder_id);")?;
    Ok(())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

fn children(
    c: &Connection,
    parent: i64,
    offset: u64,
    limit: u64,
    filtered: bool,
) -> Result<Vec<Value>> {
    let mut q = c.prepare("SELECT * FROM (
        SELECT 'folder:'||d.id AS id,'folder' AS kind,d.name,d.path,'folder:'||d.parent_id AS parent,
            COALESCE(t.bytes,0) AS bytes,COALESCE(t.files,0) AS files,d.dirs AS dirs,d.mtime AS mtime,'' AS ext,'structure' AS class
        FROM folders d LEFT JOIN rollup t ON t.folder_id=d.id
        WHERE d.parent_id=?1 AND (?4=0 OR COALESCE(t.files,0)>0)
        UNION ALL
        SELECT 'file:'||f.id,'file',f.name,f.path,'folder:'||f.folder_id,f.size,1,0,f.mtime,f.ext,analyzer_class(f.path,f.ext,f.size)
        FROM eligible f WHERE f.folder_id=?1
    ) ORDER BY bytes DESC,kind,name,id LIMIT ?2 OFFSET ?3")?;
    let rows = q.query_map(params![parent,limit as i64,offset as i64,filtered], |r| Ok(json!({
        "id":r.get::<_,String>(0)?,"kind":r.get::<_,String>(1)?,"name":r.get::<_,String>(2)?,
        "relative_path":r.get::<_,String>(3)?,"parent_id":r.get::<_,String>(4)?,"bytes":r.get::<_,i64>(5)?,
        "file_count":r.get::<_,i64>(6)?,"dir_count":r.get::<_,i64>(7)?,"modified_unix":r.get::<_,Option<i64>>(8)?,
        "ext":r.get::<_,String>(9)?,"classification":r.get::<_,String>(10)?
    })))?;
    rows.map(|x| x.map_err(Into::into)).collect()
}

pub(super) fn page(v: &Value) -> Result<Value> {
    let (id, j) = job(v)?;
    let (snapshot, totals, mut generation) = snapshot_state(&j)?;
    let view = field(v, "view")?;
    if generation.is_some() {
        if let Err(e) = validate_catalog_binding(&j) {
            if view == "catalog_check" {
                return Err(e);
            }
            generation = None;
        }
    }
    let offset = number(v, "offset", 0, i64::MAX as u64)?;
    let limit = number(v, "limit", 100, MAX_PAGE)?.max(1);
    let parent = folder(v)?;
    let c = connect(&snapshot, &j)?;
    eligible(&c, v, parent, &j)?;
    check(&j)?;
    let filtered = v
        .get("path_contains")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
        || v.get("min_size").and_then(Value::as_u64).unwrap_or(0) > 0
        || v.get("min_age_days").and_then(Value::as_u64).unwrap_or(0) > 0;
    let (selected_bytes, selected_files): (i64, i64) = c.query_row(
        "SELECT COALESCE(SUM(size),0),COUNT(*) FROM eligible",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut map = Vec::new();
    let mut breadcrumbs = Vec::new();
    let mut groups = Vec::new();
    let mut omitted = 0i64;
    let mut remainder = 0i64;
    let total: i64;
    let items: Vec<Value>;
    match view {
        "tree_map" => {
            total = c.query_row("SELECT (SELECT COUNT(*) FROM folders d LEFT JOIN rollup t ON t.folder_id=d.id WHERE d.parent_id=?1 AND (?2=0 OR COALESCE(t.files,0)>0))+(SELECT COUNT(*) FROM eligible WHERE folder_id=?1)",params![parent,filtered],|r|r.get(0))?;
            items = children(&c, parent, offset, limit, filtered)?;
            let depth = number(v, "max_depth", 4, 4)?.max(1);
            let max_nodes = number(v, "max_nodes", 512, 20_000)?.max(1) as usize;
            let mut queue = VecDeque::from([(parent, 0u64)]);
            while let Some((dir, level)) = queue.pop_front() {
                check(&j)?;
                if level >= depth || map.len() >= max_nodes {
                    continue;
                }
                for mut item in children(&c, dir, 0, (max_nodes - map.len()) as u64, filtered)? {
                    item["depth"] = json!(level + 1);
                    if item["kind"] == "folder" {
                        queue.push_back((
                            parse_folder_id(item["id"].as_str().unwrap_or(""))?,
                            level + 1,
                        ));
                    }
                    map.push(item);
                }
            }
            let descendants:i64 = c.query_row("WITH RECURSIVE d(id) AS (SELECT ?1 UNION ALL SELECT f.id FROM folders f JOIN d ON f.parent_id=d.id) SELECT COUNT(*) FROM d JOIN folders f ON f.id=d.id LEFT JOIN rollup t ON t.folder_id=f.id WHERE f.id<>?1 AND (?2=0 OR COALESCE(t.files,0)>0)",params![parent,filtered],|r|r.get(0))?;
            omitted = (descendants + selected_files - map.len() as i64).max(0);
            let shown_bytes: i64 = map
                .iter()
                .filter(|n| n["kind"] == "file")
                .filter_map(|n| n["bytes"].as_i64())
                .sum();
            remainder = selected_bytes.saturating_sub(shown_bytes);
            let mut current = Some(parent);
            while let Some(folder_id) = current {
                let (name, path, next): (String, String, Option<i64>) = c.query_row(
                    "SELECT name,path,parent_id FROM folders WHERE id=?1",
                    [folder_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?;
                breadcrumbs.push(
                    json!({"id":format!("folder:{folder_id}"),"name":name,"relative_path":path}),
                );
                current = next;
            }
            breadcrumbs.reverse();
        }
        "largest" | "bloat" | "metadata_candidates" | "catalog_check" => {
            let predicate = match view {
                "bloat" => "analyzer_class(f.path,f.ext,f.size)<>'other'",
                "metadata_candidates" => match v
                    .get("criterion")
                    .and_then(Value::as_str)
                    .unwrap_or("name_size")
                {
                    "name_size" => {
                        "EXISTS(SELECT 1 FROM eligible d WHERE d.id<>f.id AND d.norm_name=f.norm_name AND d.size=f.size)"
                    }
                    "size" => {
                        "f.size>=1048576 AND EXISTS(SELECT 1 FROM eligible d WHERE d.id<>f.id AND d.size=f.size AND d.norm_name<>f.norm_name)"
                    }
                    "name" => {
                        "EXISTS(SELECT 1 FROM eligible d WHERE d.id<>f.id AND d.norm_name=f.norm_name AND d.size<>f.size)"
                    }
                    _ => return Err(err("invalid_request", "unknown metadata criterion")),
                },
                "catalog_check" => {
                    if generation.is_none() {
                        return Err(err("not_ready", "catalog comparison has not completed"));
                    }
                    if let Some(expected) = v.get("catalog_generation").and_then(Value::as_str) {
                        if Some(expected) != generation.as_deref() {
                            return Err(err(
                                "stale_catalog",
                                "catalog comparison generation changed",
                            ));
                        }
                    }
                    match v
                        .get("match_filter")
                        .and_then(Value::as_str)
                        .unwrap_or("all")
                    {
                        "all" => "1",
                        "matched" => "EXISTS(SELECT 1 FROM matches m WHERE m.file_id=f.id)",
                        "unmatched" => "NOT EXISTS(SELECT 1 FROM matches m WHERE m.file_id=f.id)",
                        _ => return Err(err("invalid_request", "unknown match filter")),
                    }
                }
                _ => "1",
            };
            let count_sql = format!("SELECT COUNT(*) FROM eligible f WHERE {predicate}");
            total = c.query_row(&count_sql, [], |r| r.get(0))?;
            let match_sql = if generation.is_some() {
                "(SELECT COUNT(DISTINCT volume_id) FROM matches m WHERE m.file_id=f.id)"
            } else {
                "NULL"
            };
            let group_predicate = match v
                .get("criterion")
                .and_then(Value::as_str)
                .unwrap_or("name_size")
            {
                "size" => "d.size=f.size",
                "name" => "d.norm_name=f.norm_name",
                _ => "d.norm_name=f.norm_name AND d.size=f.size",
            };
            let sql = format!(
                "SELECT f.id,f.name,f.path,f.folder_id,f.size,f.ext,f.mtime,analyzer_class(f.path,f.ext,f.size),{match_sql},(SELECT COUNT(*) FROM eligible d WHERE {group_predicate}),f.norm_name FROM eligible f WHERE {predicate} ORDER BY f.size DESC,f.path,f.id LIMIT ?1 OFFSET ?2"
            );
            let mut q = c.prepare(&sql)?;
            items=q.query_map(params![limit as i64,offset as i64],|r|Ok(json!({
                "id":format!("file:{}",r.get::<_,i64>(0)?),"kind":"file","name":r.get::<_,String>(1)?,
                "relative_path":r.get::<_,String>(2)?,"parent_id":format!("folder:{}",r.get::<_,i64>(3)?),
                "bytes":r.get::<_,i64>(4)?,"ext":r.get::<_,String>(5)?,"modified_unix":r.get::<_,Option<i64>>(6)?,
                "classification":r.get::<_,String>(7)?,"candidate_volumes":r.get::<_,Option<i64>>(8)?,
                "metadata_group_count":r.get::<_,i64>(9)?,"metadata_group_key":match v.get("criterion").and_then(Value::as_str).unwrap_or("name_size") {
                    "size"=>json!(["size",r.get::<_,i64>(4)?]),
                    "name"=>json!(["name",r.get::<_,String>(10)?]),
                    _=>json!(["name_size",r.get::<_,String>(10)?,r.get::<_,i64>(4)?])
                },"file_count":1,"verified":false
            })))?.map(|x|x.map_err(Into::into)).collect::<Result<Vec<_>>>()?;
            let mut q=c.prepare("SELECT analyzer_class(path,ext,size) AS class,COUNT(*),SUM(size) FROM eligible GROUP BY class ORDER BY SUM(size) DESC")?;
            groups=q.query_map([],|r|Ok(json!({"classification":r.get::<_,String>(0)?,"files":r.get::<_,i64>(1)?,"bytes":r.get::<_,i64>(2)?})))?.map(|x|x.map_err(Into::into)).collect::<Result<Vec<_>>>()?;
        }
        "growth_diff" => {
            let baseline_id = field(v, "baseline_id")?;
            let base = baselines()
                .lock()
                .map_err(|_| err("internal", "baseline registry poisoned"))?
                .get(baseline_id)
                .cloned()
                .ok_or_else(|| err("not_found", "baseline unavailable"))?;
            if !same_path(&base.root, &snapshot.root)
                || base.root_identity != snapshot.root_identity
            {
                return Err(err(
                    "baseline_scope",
                    "baseline belongs to a different source object",
                ));
            }
            if totals["read_errors"].as_u64().unwrap_or(1) > 0 {
                return Err(err("incomplete", "growth comparison needs a complete scan"));
            }
            c.execute(
                "ATTACH DATABASE ?1 AS baseline",
                [base
                    .db
                    .to_str()
                    .ok_or_else(|| err("unsupported_path", "baseline path is not Unicode"))?],
            )?;
            c.execute_batch("CREATE TEMP TABLE diff AS SELECT paths.path,COALESCE(n.bytes,0) AS bytes,COALESCE(o.bytes,0) AS previous_bytes,COALESCE(n.bytes,0)-COALESCE(o.bytes,0) AS delta_bytes FROM (SELECT path FROM folders UNION SELECT path FROM baseline.folders) paths LEFT JOIN folders n ON n.path=paths.path LEFT JOIN baseline.folders o ON o.path=paths.path;")?;
            let filter = v
                .get("path_contains")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            total = c.query_row(
                "SELECT COUNT(*) FROM diff WHERE instr(analyzer_lower(path),?1)>0",
                [&filter],
                |r| r.get(0),
            )?;
            let mut q=c.prepare("SELECT path,bytes,previous_bytes,delta_bytes FROM diff WHERE instr(analyzer_lower(path),?1)>0 ORDER BY abs(delta_bytes) DESC,path LIMIT ?2 OFFSET ?3")?;
            items=q.query_map(params![filter,limit as i64,offset as i64],|r|Ok(json!({"relative_path":r.get::<_,String>(0)?,"bytes":r.get::<_,i64>(1)?,"previous_bytes":r.get::<_,i64>(2)?,"delta_bytes":r.get::<_,i64>(3)?,"baseline_saved_unix":base.saved_unix})))?.map(|x|x.map_err(Into::into)).collect::<Result<Vec<_>>>()?;
        }
        _ => return Err(err("invalid_request", "unsupported analyzer view")),
    }
    check(&j)?;
    let mut issues_query = c.prepare("SELECT path,reason FROM errors ORDER BY rowid LIMIT 32")?;
    let issues = issues_query
        .query_map([], |r| {
            Ok(json!({"relative_path":r.get::<_,String>(0)?,"reason":r.get::<_,String>(1)?}))
        })?
        .map(|x| x.map_err(Into::into))
        .collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"job_id":id,"view":view,"items":items,"total":total,"offset":offset,"limit":limit,
        "totals":totals,"selected_totals":{"files":selected_files,"logical_bytes":selected_bytes},
        "nodes":map,"breadcrumbs":breadcrumbs,"groups":groups,"truncated":omitted>0,
        "omitted_node_count":omitted,"remainder_bytes":remainder,"has_more":offset.saturating_add(items.len() as u64)<total as u64,
        "catalog_generation":generation,"issues":issues,"issues_truncated":totals["read_errors"].as_u64().unwrap_or(0)>32,"verified":false}),
    )
}

pub(super) fn save_baseline(v: &Value) -> Result<Value> {
    let (id, j) = job(v)?;
    let (snapshot, totals, _) = snapshot_state(&j)?;
    if totals["read_errors"].as_u64().unwrap_or(1) > 0 {
        return Err(err("incomplete", "baseline needs a complete scan"));
    }
    let key = format!("baseline:{id}");
    let dir = tempfile::Builder::new()
        .prefix("atlas-baseline-")
        .tempdir()?;
    let db = dir.path().join("baseline.sqlite");
    let mut out = Connection::open(&db)?;
    out.execute_batch("CREATE TABLE folders(path TEXT PRIMARY KEY,bytes INTEGER NOT NULL,files INTEGER NOT NULL);")?;
    let source = connect(&snapshot, &j)?;
    let tx = out.transaction()?;
    let mut q = source.prepare("SELECT path,bytes,files FROM folders ORDER BY id")?;
    let rows = q.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        check(&j)?;
        let (path, bytes, files) = row?;
        tx.execute(
            "INSERT INTO folders VALUES(?1,?2,?3)",
            params![path, bytes, files],
        )?;
    }
    tx.commit()?;
    let saved_unix = now_unix();
    let mut registry = baselines()
        .lock()
        .map_err(|_| err("internal", "baseline registry poisoned"))?;
    if registry.len() >= 16 && !registry.contains_key(&key) {
        return Err(err("busy", "baseline limit reached; clear an old baseline"));
    }
    check(&j)?;
    registry.insert(
        key.clone(),
        Arc::new(Baseline {
            root: snapshot.root.clone(),
            root_identity: snapshot.root_identity.clone(),
            _dir: dir,
            db,
            saved_unix,
        }),
    );
    Ok(
        json!({"job_id":id,"baseline_id":key,"saved_unix":saved_unix,"files":totals["files"],"logical_bytes":totals["logical_bytes"]}),
    )
}

fn same_path(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}
fn within(path: &Path, root: &Path) -> bool {
    path.ancestors().any(|p| same_path(p, root))
}
fn csv_text(value: String) -> String {
    if value
        .trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .starts_with(['=', '+', '-', '@'])
        || value.starts_with(['\t', '\r'])
    {
        format!("'{value}")
    } else {
        value
    }
}

pub(super) fn export_csv(v: &Value) -> Result<Value> {
    let (id, j) = job(v)?;
    let (snapshot, _, _) = snapshot_state(&j)?;
    let path = PathBuf::from(field(v, "path")?);
    let ids = v
        .get("selected_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| err("invalid_request", "selected_ids must be an array"))?;
    if ids.is_empty() || ids.len() > MAX_SELECTED {
        return Err(err("invalid_request", "select 1..10000 file/folder IDs"));
    }
    if !path.is_absolute() || path.components().any(|p| matches!(p, Component::ParentDir)) {
        return Err(err(
            "invalid_path",
            "export requires an absolute path without '..'",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| err("invalid_path", "export needs a parent"))?;
    let parent = validate_root(parent)?;
    let target = parent.join(
        path.file_name()
            .ok_or_else(|| err("invalid_path", "export needs a filename"))?,
    );
    if within(&target, &snapshot.root) {
        return Err(err("unsafe_output", "CSV must be outside the scanned root"));
    }
    let bound = j
        .inner
        .lock()
        .map_err(|_| err("internal", "analyzer job poisoned"))?
        .catalog_binding
        .clone();
    let mut catalogs = Vec::new();
    if let Some((path, _, _)) = bound {
        catalogs.push(path);
    }
    if let Some(db) = v.get("db").and_then(Value::as_str) {
        catalogs.push(Path::new(db).canonicalize()?);
    }
    for catalog in catalogs {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut protected = catalog.as_os_str().to_os_string();
            protected.push(suffix);
            if same_path(&target, Path::new(&protected)) {
                return Err(err(
                    "unsafe_output",
                    "CSV cannot replace a catalog or its reserved sidecars",
                ));
            }
        }
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err(err(
            "file_exists",
            "CSV never overwrites existing files or links",
        ));
    }
    let c = connect(&snapshot, &j)?;
    c.execute_batch("CREATE TEMP TABLE chosen_files(id INTEGER PRIMARY KEY); CREATE TEMP TABLE chosen_folders(id INTEGER PRIMARY KEY);")?;
    for value in ids {
        check(&j)?;
        let id = value
            .as_str()
            .ok_or_else(|| err("invalid_request", "selection IDs must be strings"))?;
        let (kind, n) = id
            .split_once(':')
            .ok_or_else(|| err("invalid_request", "invalid selection ID"))?;
        let n = n
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| err("invalid_request", "invalid selection ID"))?;
        let (table, selected) = match kind {
            "file" => ("files", "chosen_files"),
            "folder" => ("folders", "chosen_folders"),
            _ => return Err(err("invalid_request", "invalid selection kind")),
        };
        let exists: bool = c.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),
            [n],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(err(
                "not_found",
                "selected ID does not belong to this snapshot",
            ));
        }
        c.execute(&format!("INSERT OR IGNORE INTO {selected} VALUES(?1)"), [n])?;
    }
    let mut tmp = tempfile::Builder::new()
        .prefix(".atlas-review-")
        .tempfile_in(&parent)?;
    let mut written = 0u64;
    {
        let mut writer = csv::Writer::from_writer(tmp.as_file_mut());
        writer
            .write_record([
                "kind",
                "relative_path",
                "size_bytes",
                "modified_unix",
                "classification",
            ])
            .map_err(|e| err("csv", e))?;
        let mut q=c.prepare("WITH RECURSIVE branches(id) AS (SELECT id FROM chosen_folders UNION SELECT d.id FROM folders d JOIN branches b ON d.parent_id=b.id) SELECT path,size,mtime,analyzer_class(path,ext,size) FROM files WHERE id IN (SELECT id FROM chosen_files) OR folder_id IN (SELECT id FROM branches) ORDER BY id")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            check(&j)?;
            let (path, size, mtime, class) = row?;
            writer
                .write_record([
                    "file".into(),
                    csv_text(path),
                    size.to_string(),
                    mtime.map(|n| n.to_string()).unwrap_or_default(),
                    class,
                ])
                .map_err(|e| err("csv", e))?;
            written += 1;
        }
        writer.flush()?;
    }
    check(&j)?;
    tmp.as_file().sync_all()?;
    tmp.persist_noclobber(&target)
        .map_err(|e| err("export", e))?;
    Ok(
        json!({"job_id":id,"written":true,"rows":written,"folder_selection_expands_files":true,"verified":false}),
    )
}
