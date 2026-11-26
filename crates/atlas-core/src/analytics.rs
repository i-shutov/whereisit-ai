//! Read-only, bounded catalog analytics for native volume-map clients.
use crate::{Catalog, Error, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Deserialize)]
pub struct AnalyticsRequest {
    #[serde(default)]
    pub volume_id: Option<i64>,
    #[serde(default)]
    pub folder_entry_id: Option<i64>,
    #[serde(default = "default_group")]
    pub group_by: String,
    #[serde(default)]
    pub highlight_query: Option<String>,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default = "default_depth")]
    pub max_depth: usize,
    #[serde(default = "default_nodes")]
    pub max_nodes: usize,
}
fn default_group() -> String {
    "volumes".into()
}
fn default_depth() -> usize {
    4
}
fn default_nodes() -> usize {
    5000
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsNode {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub item_count: u64,
    pub matched_bytes: u64,
    pub matched_files: u64,
    pub kind: String,
    pub volume_id: Option<i64>,
    pub folder_path: Option<String>,
    pub entry_id: Option<i64>,
    pub children: Vec<AnalyticsNode>,
}
#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsSlice {
    pub id: String,
    pub label: String,
    pub bytes: u64,
    pub file_count: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsResponse {
    pub root: AnalyticsNode,
    pub breadcrumbs: Vec<serde_json::Value>,
    pub totals: AnalyticsTotals,
    pub by_extension: Vec<AnalyticsSlice>,
    pub by_category: Vec<AnalyticsSlice>,
    pub by_size_bucket: Vec<AnalyticsSlice>,
    pub by_volume: Vec<AnalyticsSlice>,
    pub truncated: bool,
    pub omitted_node_count: u64,
    pub remainder_bytes: u64,
    pub media_fields: &'static str,
}
#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsTotals {
    pub files: u64,
    pub directories: u64,
    pub indexed_bytes: u64,
}

#[derive(Clone)]
struct Row {
    id: i64,
    volume: i64,
    parent: Option<i64>,
    name: String,
    path: String,
    kind: String,
    size: u64,
}

impl Catalog {
    /// Compute metadata-only analytics from one stable read snapshot. The callback is checked
    /// between streamed rows and child queries; cancellation returns `cancelled` without writes.
    pub fn analytics(
        &self,
        req: &AnalyticsRequest,
        cancelled: impl Fn() -> bool,
    ) -> Result<AnalyticsResponse> {
        if !["volumes", "categories", "extensions"].contains(&req.group_by.as_str()) {
            return Err(Error::new(
                "invalid_request",
                "group_by must be volumes, categories, or extensions",
            ));
        }
        if req.volume_id.is_some_and(|v| v <= 0) || req.folder_entry_id.is_some_and(|v| v <= 0) {
            return Err(Error::new("invalid_request", "scope IDs must be positive"));
        }
        if req.category_id.is_some_and(|id| id <= 0) {
            return Err(Error::new(
                "invalid_request",
                "category_id must be positive",
            ));
        }
        let max_depth = req.max_depth.clamp(1, 32);
        let max_nodes = req.max_nodes.clamp(1, 20_000);
        let query_text = req.highlight_query.as_deref().unwrap_or("").trim();
        let (match_sql, match_args) = if query_text.is_empty() {
            ("0".to_string(), vec![])
        } else {
            crate::query::compile(query_text)?
        };
        let match_table = self.query_table(query_text)?;
        let tx = self.conn().unchecked_transaction()?;
        let volume = req.volume_id;
        if let Some(id) = volume {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM volumes WHERE id=?1)",
                [id],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(Error::new("not_found", "volume not found"));
            }
        }
        if let Some(id) = req.category_id {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM categories WHERE id=?1)",
                [id],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(Error::new("not_found", "category not found"));
            }
        }
        let category_id = req.category_id;
        if let Some(id) = req.folder_entry_id {
            let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM entries_core WHERE id=?1 AND (?2 IS NULL OR volume_id=?2) AND kind IN ('directory','archive'))",params![id,volume],|r|r.get(0))?;
            if !valid {
                return Err(Error::new(
                    "not_found",
                    "folder entry not found in requested volume",
                ));
            }
        }
        check(&cancelled)?;
        let (files, dirs, bytes) = scope_totals(&tx, volume, req.folder_entry_id, category_id)?;
        check(&cancelled)?;
        let by_volume = volume_slices(&tx, volume, req.folder_entry_id, category_id)?;
        let volume_totals: HashMap<i64, (u64, u64)> = by_volume
            .iter()
            .filter_map(|x| {
                x.id.strip_prefix("volume:").and_then(|id| {
                    id.parse::<i64>()
                        .ok()
                        .map(|id| (id, (x.file_count, x.bytes)))
                })
            })
            .collect();
        check(&cancelled)?;
        let mut root = if let Some(folder) = req.folder_entry_id {
            let row = entry(&tx, folder)?;
            node(
                format!("entry:{}", row.id),
                row.name,
                row.size,
                row.kind,
                Some(row.volume),
                Some(row.path),
                Some(row.id),
            )
        } else {
            node(
                "root".into(),
                "Catalog".into(),
                bytes,
                "root".into(),
                None,
                None,
                None,
            )
        };
        root.size = bytes;
        root.item_count = files;
        let (root_match_files, root_match_bytes) = matched_totals(
            &tx,
            volume,
            req.folder_entry_id,
            &match_sql,
            &match_args,
            category_id,
            match_table,
        )?;
        check(&cancelled)?;
        root.matched_files = root_match_files;
        root.matched_bytes = root_match_bytes;
        let mut queue = VecDeque::new();
        let mut emitted = 0usize;
        let mut omitted = 0u64;
        let mut remainder = 0u64;
        let mut truncated = false;
        if req.group_by != "volumes" {
            let groups = if req.group_by == "categories" {
                categories(&tx, volume, req.folder_entry_id, category_id)?
            } else {
                extensions(&tx, volume, req.folder_entry_id, category_id)?
            };
            for slice in groups {
                if emitted >= max_nodes {
                    truncated = true;
                    omitted = omitted.saturating_add(1);
                    remainder = remainder.saturating_add(slice.bytes);
                    continue;
                }
                let mut group = node(
                    slice.id,
                    slice.label,
                    slice.bytes,
                    if req.group_by == "categories" {
                        "category"
                    } else {
                        "other_group"
                    }
                    .into(),
                    None,
                    None,
                    None,
                );
                group.item_count = slice.file_count;
                let (mf, mb) = matched_group_totals(
                    &tx,
                    volume,
                    req.folder_entry_id,
                    (&match_sql, &match_args, match_table),
                    &req.group_by,
                    &group.id,
                    category_id,
                )?;
                group.matched_files = mf;
                group.matched_bytes = mb;
                root.children.push(group);
                emitted += 1;
            }
        } else if req.volume_id.is_none() && req.folder_entry_id.is_none() {
            for (id, label) in volumes(&tx)? {
                check(&cancelled)?;
                let (n, b) = volume_totals.get(&id).copied().unwrap_or((0, 0));
                if category_id.is_some() && n == 0 {
                    continue;
                }
                if emitted >= max_nodes {
                    truncated = true;
                    omitted = omitted.saturating_add(volume_node_count(&tx, id)?.saturating_add(1));
                    remainder = remainder.saturating_add(b);
                    continue;
                }
                root.children.push(node(
                    format!("volume:{id}"),
                    label,
                    b,
                    "volume".into(),
                    Some(id),
                    None,
                    None,
                ));
                root.children.last_mut().unwrap().item_count = n;
                let (mf, mb) = matched_totals(
                    &tx,
                    Some(id),
                    None,
                    &match_sql,
                    &match_args,
                    category_id,
                    match_table,
                )?;
                root.children.last_mut().unwrap().matched_files = mf;
                root.children.last_mut().unwrap().matched_bytes = mb;
                queue.push_back((None, 0, vec![root.children.len() - 1]));
                emitted += 1;
            }
        } else {
            queue.push_back((req.folder_entry_id, 0, Vec::new()));
        }
        while let Some((parent, depth, path)) = queue.pop_front() {
            if req.group_by != "volumes" {
                break;
            }
            check(&cancelled)?;
            let vol = if req.volume_id.is_some() {
                req.volume_id
            } else if !path.is_empty() {
                node_ref(&root, &path).volume_id
            } else {
                None
            };
            if depth >= max_depth {
                let (n, b) = direct_remainder(&tx, vol, parent, category_id)?;
                if n > 0 {
                    truncated = true;
                    omitted += n;
                    remainder = remainder.saturating_add(b);
                }
                continue;
            }
            let rows = children(
                &tx,
                vol,
                parent,
                max_nodes.saturating_sub(emitted).saturating_add(1),
                category_id,
            )?;
            for (processed, row) in rows.iter().cloned().enumerate() {
                check(&cancelled)?;
                if emitted >= max_nodes {
                    truncated = true;
                    let (n, b) =
                        direct_remainder_after(&tx, vol, parent, processed as u64, category_id)?;
                    omitted = omitted.saturating_add(n);
                    remainder = remainder.saturating_add(b);
                    break;
                }
                let (nfiles, nbytes) = subtree_totals(&tx, row.id, category_id)?;
                let mut child = node(
                    format!("entry:{}", row.id),
                    row.name,
                    nbytes,
                    row.kind,
                    Some(row.volume),
                    Some(row.path),
                    Some(row.id),
                );
                child.item_count = nfiles;
                let (mf, mb) = matched_totals(
                    &tx,
                    Some(row.volume),
                    Some(row.id),
                    &match_sql,
                    &match_args,
                    category_id,
                    match_table,
                )?;
                child.matched_files = mf;
                child.matched_bytes = mb;
                let parent_node = node_mut(&mut root, &path);
                parent_node.children.push(child);
                let mut child_path = path.clone();
                child_path.push(parent_node.children.len() - 1);
                queue.push_back((Some(row.id), depth + 1, child_path));
                emitted += 1;
            }
        }
        let by_extension = extensions(&tx, volume, req.folder_entry_id, category_id)?;
        let by_category = categories(&tx, volume, req.folder_entry_id, category_id)?;
        let by_size_bucket = size_buckets(&tx, volume, req.folder_entry_id, category_id)?;
        check(&cancelled)?;
        let breadcrumbs = breadcrumbs(&tx, volume, req.folder_entry_id)?;
        tx.commit()?;
        Ok(AnalyticsResponse {
            root,
            breadcrumbs,
            totals: AnalyticsTotals {
                files,
                directories: dirs,
                indexed_bytes: bytes,
            },
            by_extension,
            by_category,
            by_size_bucket,
            by_volume,
            truncated,
            omitted_node_count: omitted,
            remainder_bytes: remainder,
            media_fields: "unknown",
        })
    }
}
fn node(
    id: String,
    name: String,
    size: u64,
    kind: String,
    volume_id: Option<i64>,
    folder_path: Option<String>,
    entry_id: Option<i64>,
) -> AnalyticsNode {
    AnalyticsNode {
        id,
        name,
        size,
        item_count: 0,
        matched_bytes: 0,
        matched_files: 0,
        kind,
        volume_id,
        folder_path,
        entry_id,
        children: vec![],
    }
}
fn node_ref<'a>(root: &'a AnalyticsNode, path: &[usize]) -> &'a AnalyticsNode {
    let mut n = root;
    for i in path {
        n = &n.children[*i];
    }
    n
}
fn node_mut<'a>(root: &'a mut AnalyticsNode, path: &[usize]) -> &'a mut AnalyticsNode {
    let mut n = root;
    for i in path {
        n = &mut n.children[*i];
    }
    n
}
fn check(f: &impl Fn() -> bool) -> Result<()> {
    if f() {
        Err(Error::new("cancelled", "analytics request cancelled"))
    } else {
        Ok(())
    }
}
fn scope_cte(v: Option<i64>, f: Option<i64>) -> &'static str {
    match (v,f) {
        (_,Some(_)) => "WITH RECURSIVE scoped(id,volume_id) AS (SELECT id,volume_id FROM entries_core WHERE id=?2 UNION SELECT e.id,e.volume_id FROM entries_core e JOIN scoped s ON e.parent_id=s.id AND e.volume_id=s.volume_id) ",
        (Some(_),None) => "WITH scoped(id,volume_id) AS NOT MATERIALIZED (SELECT id,volume_id FROM entries_core WHERE volume_id=?1) ",
        (None,None) => "WITH scoped(id,volume_id) AS NOT MATERIALIZED (SELECT id,volume_id FROM entries_core) ",
    }
}
fn scope_totals(
    c: &rusqlite::Connection,
    v: Option<i64>,
    f: Option<i64>,
    category: Option<i64>,
) -> Result<(u64, u64, u64)> {
    if category.is_none() && f.is_none() {
        let result = if let Some(volume_id) = v {
            c.query_row("SELECT SUM(kind='file'),SUM(kind='directory'),SUM(CASE WHEN kind='file' THEN size ELSE 0 END) FROM entries_core WHERE volume_id=?1",[volume_id], |r| {
                Ok((sql_u64(r, 0)?, sql_u64(r, 1)?, sql_u64(r, 2)?))
            })
        } else {
            c.query_row("SELECT SUM(kind='file'),SUM(kind='directory'),SUM(CASE WHEN kind='file' THEN size ELSE 0 END) FROM entries_core",[], |r| {
                Ok((sql_u64(r, 0)?, sql_u64(r, 1)?, sql_u64(r, 2)?))
            })
        };
        return result.map_err(Into::into);
    }
    let sql=format!("{} SELECT SUM(e.kind='file' AND (?3 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=e.id AND ec.category_id=?3))),SUM(e.kind='directory'),SUM(CASE WHEN e.kind='file' AND (?3 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=e.id AND ec.category_id=?3)) THEN e.size ELSE 0 END) FROM entries_core e JOIN scoped s ON s.id=e.id",scope_cte(v,f));
    c.query_row(&sql, params![v, f, category], |r| {
        Ok((sql_u64(r, 0)?, sql_u64(r, 1)?, sql_u64(r, 2)?))
    })
    .map_err(Into::into)
}
fn matched_totals(
    c: &rusqlite::Connection,
    v: Option<i64>,
    f: Option<i64>,
    predicate: &str,
    args: &[rusqlite::types::Value],
    category: Option<i64>,
    table: &str,
) -> Result<(u64, u64)> {
    if predicate == "0" {
        return Ok((0, 0));
    }
    let sql=format!("{} SELECT COUNT(*),COALESCE(SUM(size),0) FROM {table} WHERE kind='file' AND id IN (SELECT id FROM scoped) AND (?3 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id={table}.id AND ec.category_id=?3)) AND ({predicate})",scope_cte(v,f));
    let mut values = vec![v.into(), f.into(), category.into()];
    values.extend_from_slice(args);
    c.query_row(&sql, rusqlite::params_from_iter(values.iter()), |r| {
        Ok((sql_u64(r, 0)?, sql_u64(r, 1)?))
    })
    .map_err(Into::into)
}
fn matched_group_totals(
    c: &rusqlite::Connection,
    v: Option<i64>,
    f: Option<i64>,
    highlight: (&str, &[rusqlite::types::Value], &str),
    group: &str,
    id: &str,
    category: Option<i64>,
) -> Result<(u64, u64)> {
    let (predicate, args, table) = highlight;
    if predicate == "0" {
        return Ok((0, 0));
    }
    let group_param = args.len() + 4;
    let (extra, key) = if group == "extensions" {
        (
            format!("ext=?{group_param}"),
            rusqlite::types::Value::Text(id.strip_prefix("ext:").unwrap_or("").into()),
        )
    } else {
        let raw = id.strip_prefix("category:").unwrap_or("uncategorized");
        (format!("COALESCE((SELECT MIN(category_id) FROM entry_categories WHERE entry_id={table}.id),0)=?{group_param}"),raw.parse::<i64>().ok().map(rusqlite::types::Value::Integer).unwrap_or(rusqlite::types::Value::Integer(0)))
    };
    let sql=format!("{} SELECT COUNT(*),COALESCE(SUM(size),0) FROM {table} WHERE kind='file' AND id IN (SELECT id FROM scoped) AND (?3 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id={table}.id AND ec.category_id=?3)) AND ({predicate}) AND ({extra})",scope_cte(v,f));
    let mut values = vec![v.into(), f.into(), category.into()];
    values.extend_from_slice(args);
    values.push(key);
    c.query_row(&sql, rusqlite::params_from_iter(values.iter()), |r| {
        Ok((sql_u64(r, 0)?, sql_u64(r, 1)?))
    })
    .map_err(Into::into)
}
fn sql_u64(r: &rusqlite::Row<'_>, i: usize) -> rusqlite::Result<u64> {
    let n: Option<i64> = r.get(i)?;
    Ok(n.unwrap_or(0).max(0) as u64)
}
fn children(
    c: &rusqlite::Connection,
    v: Option<i64>,
    p: Option<i64>,
    limit: usize,
    category: Option<i64>,
) -> Result<Vec<Row>> {
    let volume_clause = if v.is_some() {
        "e.volume_id=?1 AND "
    } else {
        ""
    };
    let sql=format!("SELECT e.id,e.volume_id,e.parent_id,e.name,e.relative_path,e.kind,e.size FROM entries_core e WHERE {volume_clause}e.parent_id IS ?2 AND (?4 IS NULL OR (e.kind='file' AND EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=e.id AND ec.category_id=?4)) OR (e.kind IN ('directory','archive') AND EXISTS(WITH RECURSIVE d(id) AS (SELECT ch.id FROM entries_core ch WHERE ch.parent_id=e.id AND ch.volume_id=e.volume_id UNION SELECT ch.id FROM entries_core ch JOIN d ON ch.parent_id=d.id AND ch.volume_id=e.volume_id) SELECT 1 FROM d JOIN entry_categories ec ON ec.entry_id=d.id WHERE ec.category_id=?4))) ORDER BY e.name COLLATE NOCASE,e.id LIMIT ?3");
    let mut s = c.prepare(&sql)?;
    let rows = s.query_map(params![v, p, limit.min(20_001) as i64, category], |r| {
        Ok(Row {
            id: r.get(0)?,
            volume: r.get(1)?,
            parent: r.get(2)?,
            name: r.get(3)?,
            path: r.get(4)?,
            kind: r.get(5)?,
            size: r.get::<_, i64>(6)?.max(0) as u64,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
fn entry(c: &rusqlite::Connection, id: i64) -> Result<Row> {
    c.query_row(
        "SELECT id,volume_id,parent_id,name,relative_path,kind,size FROM entries_core WHERE id=?1",
        [id],
        |r| {
            Ok(Row {
                id: r.get(0)?,
                volume: r.get(1)?,
                parent: r.get(2)?,
                name: r.get(3)?,
                path: r.get(4)?,
                kind: r.get(5)?,
                size: r.get::<_, i64>(6)?.max(0) as u64,
            })
        },
    )
    .map_err(Into::into)
}
fn subtree_totals(c: &rusqlite::Connection, id: i64, category: Option<i64>) -> Result<(u64, u64)> {
    c.query_row("WITH RECURSIVE t(id,volume_id,kind,size) AS (SELECT id,volume_id,kind,size FROM entries_core WHERE id=?1 UNION SELECT e.id,e.volume_id,e.kind,e.size FROM entries_core e JOIN t ON e.parent_id=t.id AND e.volume_id=t.volume_id) SELECT SUM(t.kind='file' AND (?2 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=t.id AND ec.category_id=?2))),SUM(CASE WHEN t.kind='file' AND (?2 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=t.id AND ec.category_id=?2)) THEN t.size ELSE 0 END) FROM t",params![id,category],|r|Ok((sql_u64(r,0)?,sql_u64(r,1)?))).map_err(Into::into)
}
fn direct_remainder(
    c: &rusqlite::Connection,
    v: Option<i64>,
    p: Option<i64>,
    category: Option<i64>,
) -> Result<(u64, u64)> {
    direct_remainder_after(c, v, p, 0, category)
}
fn direct_remainder_after(
    c: &rusqlite::Connection,
    v: Option<i64>,
    p: Option<i64>,
    offset: u64,
    category: Option<i64>,
) -> Result<(u64, u64)> {
    if category.is_none() {
        if offset == 0 && p.is_none() {
            if let Some(volume_id) = v {
                return c.query_row("SELECT COUNT(*),COALESCE(SUM(CASE WHEN kind='file' THEN size ELSE 0 END),0) FROM entries_core WHERE volume_id=?1",[volume_id],|r|Ok((sql_u64(r,0)?,sql_u64(r,1)?))).map_err(Into::into);
            }
        }
        let volume_clause = if v.is_some() {
            "e.volume_id=?1 AND "
        } else {
            ""
        };
        let sql=format!("WITH RECURSIVE rest(id,volume_id) AS (SELECT e.id,e.volume_id FROM entries_core e WHERE {volume_clause}e.parent_id IS ?2 ORDER BY e.name COLLATE NOCASE,e.id LIMIT -1 OFFSET ?3), tree(id,volume_id,kind,size) AS (SELECT e.id,e.volume_id,e.kind,e.size FROM entries_core e JOIN rest r ON r.id=e.id UNION SELECT e.id,e.volume_id,e.kind,e.size FROM entries_core e JOIN tree t ON e.parent_id=t.id AND e.volume_id=t.volume_id) SELECT COUNT(*),COALESCE(SUM(CASE WHEN kind='file' THEN size ELSE 0 END),0) FROM tree");
        return c
            .query_row(&sql, params![v, p, offset as i64], |r| {
                Ok((sql_u64(r, 0)?, sql_u64(r, 1)?))
            })
            .map_err(Into::into);
    }
    let volume_clause = if v.is_some() {
        "e.volume_id=?1 AND "
    } else {
        ""
    };
    let sql=format!("WITH RECURSIVE rest(id,volume_id) AS (SELECT e.id,e.volume_id FROM entries_core e WHERE {volume_clause}e.parent_id IS ?2 AND (?4 IS NULL OR (e.kind='file' AND EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=e.id AND ec.category_id=?4)) OR (e.kind IN ('directory','archive') AND EXISTS(WITH RECURSIVE d(id) AS (SELECT ch.id FROM entries_core ch WHERE ch.parent_id=e.id AND ch.volume_id=e.volume_id UNION SELECT ch.id FROM entries_core ch JOIN d ON ch.parent_id=d.id AND ch.volume_id=e.volume_id) SELECT 1 FROM d JOIN entry_categories ec ON ec.entry_id=d.id WHERE ec.category_id=?4))) ORDER BY e.name COLLATE NOCASE,e.id LIMIT -1 OFFSET ?3), tree(id,volume_id,kind,size) AS (SELECT e.id,e.volume_id,e.kind,e.size FROM entries_core e JOIN rest r ON r.id=e.id UNION SELECT e.id,e.volume_id,e.kind,e.size FROM entries_core e JOIN tree t ON e.parent_id=t.id AND e.volume_id=t.volume_id), eligible(id) AS (SELECT tree.id FROM tree JOIN entries_core e ON e.id=tree.id WHERE (?4 IS NULL OR (tree.kind='file' AND EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=tree.id AND ec.category_id=?4))) UNION SELECT parent.id FROM entries_core child JOIN eligible x ON x.id=child.id JOIN tree parent ON parent.id=child.parent_id AND parent.volume_id=child.volume_id) SELECT (SELECT COUNT(*) FROM eligible),COALESCE(SUM(CASE WHEN kind='file' AND (?4 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=tree.id AND ec.category_id=?4)) THEN size ELSE 0 END),0) FROM tree");
    c.query_row(&sql, params![v, p, offset as i64, category], |r| {
        Ok((sql_u64(r, 0)?, sql_u64(r, 1)?))
    })
    .map_err(Into::into)
}
fn volume_node_count(c: &rusqlite::Connection, id: i64) -> Result<u64> {
    c.query_row(
        "SELECT COUNT(*) FROM entries_core WHERE volume_id=?1",
        [id],
        |r| sql_u64(r, 0),
    )
    .map_err(Into::into)
}
fn volumes(c: &rusqlite::Connection) -> Result<Vec<(i64, String)>> {
    let mut s = c.prepare("SELECT id,label FROM volumes ORDER BY id")?;
    let rows = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
fn breadcrumbs(
    c: &rusqlite::Connection,
    v: Option<i64>,
    f: Option<i64>,
) -> Result<Vec<serde_json::Value>> {
    let mut seen = std::collections::HashSet::new();
    let mut b = vec![serde_json::json!({"label":"Catalog","volumeId":null,"folderEntryId":null})];
    if let Some(id) = v {
        let label: String =
            c.query_row("SELECT label FROM volumes WHERE id=?1", [id], |r| r.get(0))?;
        b.push(serde_json::json!({"label":label,"volumeId":id,"folderEntryId":null}));
    }
    if let Some(id) = f {
        let mut cur = Some(entry(c, id)?);
        let mut chain = vec![];
        while let Some(x) = cur {
            if !seen.insert(x.id) {
                return Err(Error::new("invalid_catalog", "entry parent cycle detected"));
            }
            if v.is_some_and(|volume| volume != x.volume) {
                return Err(Error::new(
                    "invalid_catalog",
                    "folder ancestry crosses volumes",
                ));
            }
            chain.push((x.id, x.volume, x.name));
            cur = if let Some(pid) = x.parent {
                let parent = entry(c, pid)?;
                if parent.volume != x.volume {
                    return Err(Error::new(
                        "invalid_catalog",
                        "folder ancestry crosses volumes",
                    ));
                }
                Some(parent)
            } else {
                None
            };
        }
        for (eid, vid, name) in chain.into_iter().rev() {
            b.push(serde_json::json!({"label":name,"volumeId":vid,"folderEntryId":eid}));
        }
    }
    Ok(b)
}
fn extensions(
    c: &rusqlite::Connection,
    v: Option<i64>,
    f: Option<i64>,
    category: Option<i64>,
) -> Result<Vec<AnalyticsSlice>> {
    let sql=format!("{} SELECT e.ext,COUNT(*),SUM(e.size) FROM entries_core e JOIN scoped s ON s.id=e.id WHERE e.kind='file' AND (?3 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=e.id AND ec.category_id=?3)) GROUP BY e.ext ORDER BY SUM(e.size) DESC,e.ext COLLATE NOCASE",scope_cte(v,f));
    let mut s = c.prepare(&sql)?;
    let rows = s.query_map(params![v, f, category], |r| {
        Ok(AnalyticsSlice {
            id: format!("ext:{}", r.get::<_, String>(0)?),
            label: {
                let x: String = r.get(0)?;
                if x.is_empty() {
                    "(no extension)".into()
                } else {
                    format!(".{x}")
                }
            },
            file_count: sql_u64(r, 1)?,
            bytes: sql_u64(r, 2)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
fn categories(
    c: &rusqlite::Connection,
    v: Option<i64>,
    f: Option<i64>,
    category: Option<i64>,
) -> Result<Vec<AnalyticsSlice>> {
    let sql=format!("{} SELECT c.id,c.name,COUNT(*),SUM(e.size) FROM entries_core e JOIN scoped s ON s.id=e.id LEFT JOIN (SELECT entry_id,MIN(category_id) category_id FROM entry_categories GROUP BY entry_id) ec ON ec.entry_id=e.id LEFT JOIN categories c ON c.id=ec.category_id WHERE e.kind='file' AND (?3 IS NULL OR EXISTS(SELECT 1 FROM entry_categories filter_ec WHERE filter_ec.entry_id=e.id AND filter_ec.category_id=?3)) GROUP BY c.id,c.name ORDER BY SUM(e.size) DESC,c.name COLLATE NOCASE",scope_cte(v,f));
    let mut s = c.prepare(&sql)?;
    let rows = s.query_map(params![v, f, category], |r| {
        let id: Option<i64> = r.get(0)?;
        let label: Option<String> = r.get(1)?;
        Ok(AnalyticsSlice {
            id: id
                .map(|x| format!("category:{x}"))
                .unwrap_or_else(|| "category:uncategorized".into()),
            label: label.unwrap_or_else(|| "(uncategorized)".into()),
            file_count: sql_u64(r, 2)?,
            bytes: sql_u64(r, 3)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
fn size_buckets(
    c: &rusqlite::Connection,
    v: Option<i64>,
    f: Option<i64>,
    category: Option<i64>,
) -> Result<Vec<AnalyticsSlice>> {
    let sql=format!("{} SELECT CASE WHEN e.size<1048576 THEN 0 WHEN e.size<1073741824 THEN 1 WHEN e.size<10737418240 THEN 2 ELSE 3 END b,COUNT(*),SUM(e.size) FROM entries_core e JOIN scoped s ON s.id=e.id WHERE e.kind='file' AND (?3 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=e.id AND ec.category_id=?3)) GROUP BY b ORDER BY b",scope_cte(v,f));
    let mut s = c.prepare(&sql)?;
    let rows = s.query_map(params![v, f, category], |r| {
        let b: i64 = r.get(0)?;
        let l = ["<1 MiB", "1 MiB–1 GiB", "1–10 GiB", ">=10 GiB"][b as usize];
        Ok(AnalyticsSlice {
            id: format!("size:{b}"),
            label: l.into(),
            file_count: sql_u64(r, 1)?,
            bytes: sql_u64(r, 2)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
fn volume_slices(
    c: &rusqlite::Connection,
    v: Option<i64>,
    f: Option<i64>,
    category: Option<i64>,
) -> Result<Vec<AnalyticsSlice>> {
    let sql=format!("{} SELECT v.id,v.label,COUNT(*),SUM(e.size) FROM entries_core e JOIN scoped s ON s.id=e.id JOIN volumes v ON v.id=e.volume_id WHERE e.kind='file' AND (?3 IS NULL OR EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=e.id AND ec.category_id=?3)) GROUP BY v.id,v.label ORDER BY v.id",scope_cte(v,f));
    let mut s = c.prepare(&sql)?;
    let rows = s.query_map(params![v, f, category], |r| {
        Ok(AnalyticsSlice {
            id: format!("volume:{}", r.get::<_, i64>(0)?),
            label: r.get(1)?,
            file_count: sql_u64(r, 2)?,
            bytes: sql_u64(r, 3)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod scope_fastpath_tests {
    use super::*;
    use crate::NewEntry;

    fn entry(parent_id: Option<i64>, name: &str, path: &str, kind: &str, size: u64) -> NewEntry {
        NewEntry {
            parent_id,
            name: name.into(),
            relative_path: path.into(),
            kind: kind.into(),
            size,
            modified_unix: None,
            description: String::new(),
            tags: String::new(),
        }
    }
    #[test]
    fn analytics_scope_and_remainder_fastpaths_keep_global_volume_folder_and_zero_byte_counts() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::open(dir.path().join("fastpath.sqlite")).unwrap();
        let mut folder = 0;
        let first = catalog
            .bulk_import("First", "offline", |w| {
                folder = w.insert(&entry(None, "資料", "資料", "directory", 0))?;
                w.insert(&entry(Some(folder), "a.txt", "資料/a.txt", "file", 3))?;
                w.insert(&entry(
                    Some(folder),
                    "empty.txt",
                    "資料/empty.txt",
                    "file",
                    0,
                ))?;
                w.insert(&entry(None, "root.bin", "root.bin", "file", 4))?;
                Ok(())
            })
            .unwrap()
            .id;
        catalog
            .bulk_import("Second", "offline", |w| {
                w.insert(&entry(None, "other.bin", "other.bin", "file", 5))?;
                Ok(())
            })
            .unwrap();
        let global = scope_totals(catalog.conn(), None, None, None).unwrap();
        assert_eq!(global, (4, 1, 12));
        let volume = scope_totals(catalog.conn(), Some(first), None, None).unwrap();
        assert_eq!(volume, (3, 1, 7));
        let branch = scope_totals(catalog.conn(), Some(first), Some(folder), None).unwrap();
        assert_eq!(branch, (2, 1, 3));
        assert_eq!(
            direct_remainder_after(catalog.conn(), Some(first), None, 0, None).unwrap(),
            (4, 7)
        );
        assert_eq!(
            direct_remainder_after(catalog.conn(), Some(first), Some(folder), 0, None).unwrap(),
            (2, 3)
        );
        assert_eq!(
            direct_remainder_after(catalog.conn(), Some(first), Some(folder), 1, None).unwrap(),
            (1, 0)
        );
        let cat = catalog.create_category("Books", None).unwrap();
        let first_file = catalog.children(first, Some(folder), 0, 10).unwrap().items[0].id;
        catalog.assign_category(first_file, cat).unwrap();
        let filtered = catalog
            .analytics(
                &AnalyticsRequest {
                    volume_id: None,
                    folder_entry_id: None,
                    group_by: "volumes".into(),
                    highlight_query: None,
                    category_id: Some(cat),
                    max_depth: 4,
                    max_nodes: 8,
                },
                || false,
            )
            .unwrap();
        assert_eq!(filtered.root.children.len(), 1);
        assert!(filtered.root.children.iter().all(|n| n.item_count > 0));
    }
}
