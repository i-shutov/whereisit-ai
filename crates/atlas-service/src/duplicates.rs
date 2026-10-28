//! Persistent, revision-checked metadata duplicate candidates. The index contains IDs,
//! not file contents; equality of bytes is never implied by a candidate group.
use crate::{optional_id, unsigned};
use atlas_core::{Catalog, Error, Result};
use rusqlite::{params, types::Value as SqlValue, TransactionBehavior};
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};

static BUILD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug)]
struct Request {
    config: String,
    filter: String,
    args: Vec<SqlValue>,
    mode: String,
}
fn request(v: &Value, force: bool) -> Result<Request> {
    let query = match v.get("query") {
        None => "",
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(Error::new("invalid_request", "query must be a string")),
    };
    let mode = match v.get("duplicate_mode") {
        Some(Value::String(s)) => s.as_str(),
        None => {
            if force {
                match v.get("mode") {
                    Some(Value::String(s))
                        if matches!(s.as_str(), "name_size" | "size" | "name" | "hash") =>
                    {
                        s.as_str()
                    }
                    _ => "name_size",
                }
            } else {
                "name_size"
            }
        }
        _ => {
            return Err(Error::new(
                "invalid_request",
                "duplicate_mode must be a string",
            ))
        }
    };
    if mode == "hash" {
        return Err(Error::new(
            "unsupported",
            "Content hash comparison is not available",
        ));
    }
    if !matches!(mode, "name_size" | "size" | "name") {
        return Err(Error::new(
            "invalid_request",
            "Duplicate mode must be name_size, size, or name",
        ));
    }
    let volume = optional_id(v, "volume_id")?;
    let (filter, mut args) = atlas_core::query::compile(query)?;
    let filter = if let Some(id) = volume {
        args.push(id.into());
        format!("({filter}) AND volume_id=? AND kind='file'")
    } else {
        format!("({filter}) AND kind='file'")
    };
    let config = json!({"version":2,"query":query,"volume_id":volume,"mode":mode}).to_string();
    Ok(Request {
        config,
        filter,
        args,
        mode: mode.into(),
    })
}
/// Install a revision tracker and persistent single-configuration cache. Insertions
/// from the public bulk API always create a volume in the same transaction; its
/// volume trigger invalidates the cache once instead of once for each entry.
pub fn ensure_schema(c: &Catalog) -> Result<()> {
    // The final trigger is installed last, so its presence marks a complete schema.
    let exists: bool = c.conn().query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='trigger' AND name='atlas_rev_link_update')", [], |r| r.get(0))?;
    if exists {
        return Ok(());
    }
    let mut schema = r#"
CREATE TABLE IF NOT EXISTS atlas_meta(id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL);
INSERT OR IGNORE INTO atlas_meta(id,revision) VALUES(1,0);
CREATE TABLE IF NOT EXISTS duplicate_cache_state(id INTEGER PRIMARY KEY CHECK(id=1),config TEXT NOT NULL,revision INTEGER NOT NULL,total INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS duplicate_cache_groups(id INTEGER PRIMARY KEY,name_key TEXT,size_key INTEGER,duplicate_count INTEGER NOT NULL,duplicate_group TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS duplicate_cache_keys ON duplicate_cache_groups(name_key,size_key);
CREATE INDEX IF NOT EXISTS duplicate_cache_size ON duplicate_cache_groups(size_key);
CREATE TABLE IF NOT EXISTS duplicate_cache_members(position INTEGER PRIMARY KEY,entry_id INTEGER NOT NULL,group_id INTEGER NOT NULL);
CREATE TRIGGER IF NOT EXISTS atlas_rev_volume_insert AFTER INSERT ON volumes BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_volume_update AFTER UPDATE ON volumes BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_volume_delete AFTER DELETE ON volumes BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_entry_update AFTER UPDATE ON entries BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_entry_delete AFTER DELETE ON entries BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_category_insert AFTER INSERT ON categories BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_category_update AFTER UPDATE ON categories BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_category_delete AFTER DELETE ON categories BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_link_insert AFTER INSERT ON entry_categories BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_link_delete AFTER DELETE ON entry_categories BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS atlas_rev_link_update AFTER UPDATE ON entry_categories BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END;
"#.to_owned();
    if c.query_table("")? == "entries_core" {
        schema = schema.replace(" ON entries BEGIN", " ON entries_core BEGIN");
        let payload_triggers = "CREATE TRIGGER IF NOT EXISTS atlas_rev_payload_insert AFTER INSERT ON entry_payload BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END; CREATE TRIGGER IF NOT EXISTS atlas_rev_payload_update AFTER UPDATE ON entry_payload BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END; CREATE TRIGGER IF NOT EXISTS atlas_rev_payload_delete AFTER DELETE ON entry_payload BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END; ";
        schema = schema.replace(
            "CREATE TRIGGER IF NOT EXISTS atlas_rev_link_update",
            &format!("{payload_triggers}CREATE TRIGGER IF NOT EXISTS atlas_rev_link_update"),
        );
    }
    c.conn().execute_batch(&schema)?;
    Ok(())
}
/// Ensure that the cache corresponds to the selected duplicate mode and revision.
/// Callers consuming the cache must hold a read snapshot and verify state, or use
/// `duplicates`, which performs that verification automatically.
pub fn ensure_cache(c: &Catalog, v: &Value, force: bool) -> Result<()> {
    let req = request(v, force)?;
    let table = c.query_table(v.get("query").and_then(Value::as_str).unwrap_or(""))?;
    ensure_schema(c)?;
    let already_current:bool=c.conn().query_row("SELECT EXISTS(SELECT 1 FROM duplicate_cache_state s JOIN atlas_meta a ON a.id=1 AND a.revision=s.revision WHERE s.id=1 AND s.config=?1)", [&req.config], |r|r.get(0))?;
    if already_current {
        return Ok(());
    }
    let _guard = BUILD_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| Error::new("internal", "Duplicate cache lock failed"))?;
    let tx = rusqlite::Transaction::new_unchecked(c.conn(), TransactionBehavior::Immediate)?;
    let revision: i64 = tx.query_row("SELECT revision FROM atlas_meta WHERE id=1", [], |r| {
        r.get(0)
    })?;
    let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM duplicate_cache_state WHERE id=1 AND config=?1 AND revision=?2)",params![req.config,revision],|r|r.get(0))?;
    if !valid {
        tx.execute_batch("DELETE FROM duplicate_cache_state; DELETE FROM duplicate_cache_members; DELETE FROM duplicate_cache_groups;")?;
        let (key_columns, group_by, display, join) = match req.mode.as_str() {
            "name_size" => (
                "name_norm,size",
                "name_norm,size",
                "name_norm || ':' || size",
                "e.name_norm=g.name_key AND e.size=g.size_key",
            ),
            "name" => (
                "name_norm,NULL",
                "name_norm",
                "name_norm",
                "e.name_norm=g.name_key",
            ),
            _ => (
                "NULL,size",
                "size",
                "CAST(size AS TEXT)",
                "e.size=g.size_key",
            ),
        };
        let groups=format!("INSERT INTO duplicate_cache_groups(name_key,size_key,duplicate_count,duplicate_group) SELECT {key_columns},COUNT(*),{display} FROM {table} WHERE {} GROUP BY {group_by} HAVING COUNT(*)>1",req.filter);
        tx.execute(&groups, rusqlite::params_from_iter(req.args.iter()))?;
        // Keep original result order and materialize it once. Page reads then seek by
        // position and join only their bounded set of IDs to live rows.
        let members=format!("INSERT INTO duplicate_cache_members(entry_id,group_id) SELECT e.id,g.group_id FROM {table} e JOIN (SELECT id AS group_id,name_key,size_key,duplicate_group FROM duplicate_cache_groups) g ON {join} WHERE {} ORDER BY g.duplicate_group,e.volume_id,e.relative_path,e.id",req.filter);
        tx.execute(&members, rusqlite::params_from_iter(req.args.iter()))?;
        let total: i64 = tx.query_row("SELECT COUNT(*) FROM duplicate_cache_members", [], |r| {
            r.get(0)
        })?;
        tx.execute(
            "INSERT INTO duplicate_cache_state(id,config,revision,total) VALUES(1,?1,?2,?3)",
            params![req.config, revision, total],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Open a consistent cache snapshot for a duplicate export. The caller must use
/// this transaction for every export query until it has finished reading rows.
pub fn export_snapshot<'a>(c: &'a Catalog, v: &Value) -> Result<rusqlite::Transaction<'a>> {
    let req = request(v, false)?;
    for _ in 0..3 {
        ensure_cache(c, v, false)?;
        let tx = c.conn().unchecked_transaction()?;
        let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM duplicate_cache_state s JOIN atlas_meta a ON a.id=1 AND a.revision=s.revision WHERE s.id=1 AND s.config=?1)",[&req.config],|r|r.get(0))?;
        if valid {
            return Ok(tx);
        }
        drop(tx);
    }
    Err(Error::new(
        "busy",
        "Catalog changed repeatedly while preparing export",
    ))
}

pub fn duplicates(c: &Catalog, v: &Value) -> Result<Value> {
    let req = request(v, true)?;
    let offset = unsigned(v, "offset", 0)?.min(i64::MAX as u64);
    let limit = unsigned(v, "limit", 200)?.clamp(1, 1000);
    for _ in 0..3 {
        ensure_cache(c, v, true)?;
        // A single read snapshot closes the race with writes or other cache configs.
        let tx = c.conn().unchecked_transaction()?;
        let state:Option<i64>=rusqlite::OptionalExtension::optional(tx.query_row("SELECT total FROM duplicate_cache_state s JOIN atlas_meta a ON a.id=1 AND a.revision=s.revision WHERE s.id=1 AND s.config=?1",[&req.config],|r|r.get(0)))?;
        if let Some(total) = state {
            let sql="SELECT e.id,e.volume_id,e.parent_id,e.name,e.relative_path,e.kind,e.size,e.modified_unix,e.description,e.tags,g.duplicate_count,g.duplicate_group FROM duplicate_cache_members m JOIN entries e ON e.id=m.entry_id JOIN duplicate_cache_groups g ON g.id=m.group_id WHERE m.position>?1 ORDER BY m.position LIMIT ?2";
            let mut stmt = tx.prepare(sql)?;
            let items = stmt
                .query_map(params![offset as i64, limit as i64], |r| {
                    let e = atlas_core::entry_row(r)?;
                    let mut v = serde_json::to_value(e).expect("Entry serializes");
                    v["duplicate_count"] = json!(r.get::<_, i64>(10)?);
                    v["duplicate_group"] = json!(r.get::<_, String>(11)?);
                    Ok(v)
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            drop(stmt);
            tx.commit()?;
            return Ok(
                json!({"items":items,"total":total,"offset":offset,"limit":limit,"verified":false,"basis":"metadata_candidates"}),
            );
        }
        drop(tx);
    }
    Err(Error::new(
        "busy",
        "Catalog changed repeatedly while loading duplicate candidates",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_core::NewEntry;
    fn entry(name: &str, size: u64) -> NewEntry {
        NewEntry {
            parent_id: None,
            name: name.into(),
            relative_path: name.into(),
            kind: "file".into(),
            size,
            modified_unix: None,
            description: String::new(),
            tags: String::new(),
        }
    }
    #[test]
    fn duplicate_export_snapshot_uses_requested_scope() {
        let d = tempfile::tempdir().unwrap();
        let mut c = Catalog::open(d.path().join("db")).unwrap();
        c.bulk_import("v", "offline", |w| {
            for name in ["x.mkv", "x.mkv", "y.mp4", "y.mp4"] {
                w.insert(&entry(name, 1))?;
            }
            Ok(())
        })
        .unwrap();
        let req = json!({"mode":"duplicates","duplicate_mode":"name","query":"ext:mp4"});
        let tx = export_snapshot(&c, &req).unwrap();
        let selection = crate::selection::select(&req, false).unwrap();
        let count: i64 = tx
            .query_row(
                &format!("{} SELECT COUNT(*) FROM matches", selection.cte),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
        drop(tx);
        let other = json!({"mode":"duplicates","duplicate_mode":"name","query":"ext:mkv"});
        let tx = export_snapshot(&c, &other).unwrap();
        let result:String=tx.query_row("SELECT e.name FROM duplicate_cache_members m JOIN entries e ON e.id=m.entry_id LIMIT 1",[],|r|r.get(0)).unwrap();
        assert_eq!(result, "x.mkv");
    }
    #[test]
    fn cache_reused_and_invalidated_by_volumes_edits_and_categories() {
        let d = tempfile::tempdir().unwrap();
        let mut c = Catalog::open(d.path().join("db")).unwrap();
        let mut ids = Vec::new();
        c.bulk_import("one", "disk", |w| {
            ids.push(w.insert(&entry("Film.mkv", 100))?);
            ids.push(w.insert(&entry("FILM.mkv", 100))?);
            w.insert(&entry("Other.mkv", 20))?;
            Ok(())
        })
        .unwrap();
        let v = json!({"mode":"name_size","offset":0,"limit":1});
        let r = duplicates(&c, &v).unwrap();
        assert_eq!(r["total"], 2);
        assert_eq!(r["items"].as_array().unwrap().len(), 1);
        assert_eq!(r["verified"], false);
        let revision: i64 = c
            .conn()
            .query_row("SELECT revision FROM atlas_meta", [], |r| r.get(0))
            .unwrap();
        duplicates(&c, &v).unwrap();
        let revision2: i64 = c
            .conn()
            .query_row("SELECT revision FROM duplicate_cache_state", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(revision, revision2);
        c.bulk_import("two", "disk", |w| {
            w.insert(&entry("Film.mkv", 100))?;
            Ok(())
        })
        .unwrap();
        assert_eq!(duplicates(&c, &v).unwrap()["total"], 3);
        c.update_entry(ids[0], "important", "").unwrap();
        assert_eq!(
            duplicates(
                &c,
                &json!({"mode":"name_size","query":"description:important"})
            )
            .unwrap()["total"],
            0
        );
        let category = c.create_category("Books", None).unwrap();
        c.assign_category(ids[0], category).unwrap();
        assert_eq!(
            duplicates(&c, &json!({"mode":"name_size","query":"category:Books"})).unwrap()["total"],
            0
        );
        c.assign_category(ids[1], category).unwrap();
        assert_eq!(
            duplicates(&c, &json!({"mode":"name_size","query":"category:Books"})).unwrap()["total"],
            2
        );
        assert_eq!(
            duplicates(&c, &json!({"mode":"hash"})).unwrap_err().code,
            "unsupported"
        );
        assert!(duplicates(&c, &json!({"query":42})).is_err());
    }
}
