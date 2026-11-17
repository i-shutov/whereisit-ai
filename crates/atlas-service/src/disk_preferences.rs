//! User-authored disk presentation metadata, stored separately from the catalog.
//! These values are annotations, never live device identity or OS measurements.
use atlas_core::{Catalog, Error, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn invalid(message: &str) -> Error {
    Error::new("invalid_disk_properties", message)
}

fn paths(v: &Value) -> Result<(PathBuf, PathBuf, String, &'static str)> {
    let db_text = crate::text(v, "db")?;
    let db = Path::new(db_text);
    let store = Path::new(crate::text(v, "user_store")?);
    if !db.is_absolute()
        || !store.is_absolute()
        || db.as_os_str().len() > 2048
        || store.as_os_str().len() > 4096
    {
        return Err(invalid(
            "Catalog and user store must be bounded absolute paths",
        ));
    }
    let db = db.canonicalize()?;
    let _catalog = Catalog::open_readonly(&db)?;
    if let Ok(meta) = std::fs::symlink_metadata(store) {
        if !meta.file_type().is_file() {
            return Err(invalid("User store must be a regular file, not a link"));
        }
        for source in [
            &db,
            &PathBuf::from(format!("{}-wal", db.display())),
            &PathBuf::from(format!("{}-shm", db.display())),
            &PathBuf::from(format!("{}-journal", db.display())),
        ] {
            if source.exists() && same_file::is_same_file(source, store)? {
                return Err(invalid(
                    "User store cannot be the source catalog or its SQLite sidecars",
                ));
            }
        }
    }
    let os = match crate::text(v, "platform")? {
        "windows" => "windows",
        "macos" => "macos",
        _ => return Err(invalid("Unsupported platform")),
    };
    // Match the existing physical-label contract, which uses the catalog path
    // supplied by the native client as its scope.
    let key = format!("sqlite:{db_text}");
    if key.len() > 2048 {
        return Err(invalid("Catalog identity is too long"));
    }
    Ok((db, store.to_path_buf(), key, os))
}
fn volume_id(v: &Value) -> Result<i64> {
    v.get("volume_id")
        .and_then(Value::as_i64)
        .filter(|x| *x > 0)
        .ok_or_else(|| invalid("A positive volume ID is required"))
}
fn ensure_volume(db: &Path, id: i64) -> Result<()> {
    let catalog = Catalog::open_readonly(db)?;
    let exists: bool = catalog.conn().query_row(
        "SELECT EXISTS(SELECT 1 FROM volumes WHERE id=?1)",
        [id],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(invalid("Volume does not exist in this catalog"));
    }
    Ok(())
}
fn ensure_store(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )?;
    let names = statement
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if names.iter().any(|n| {
        !matches!(
            n.as_str(),
            "physical_bindings" | "physical_shelf_labels" | "disk_preferences"
        )
    }) {
        return Err(invalid("Refusing to modify an unrelated SQLite file"));
    }
    Ok(())
}
fn field(v: &Value, name: &str, max: usize, multiline: bool) -> Result<String> {
    let text = crate::text(v, name)?.trim();
    if text.chars().count() > max
        || text.len() > max * 4
        || text
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')))
    {
        return Err(invalid(
            "A field is too long or contains control characters",
        ));
    }
    Ok(text.to_owned())
}
fn read_one(conn: &Connection, key: &str, os: &str, id: i64) -> Result<Value> {
    let has_properties: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='disk_preferences')",
        [],
        |r| r.get(0),
    )?;
    let has_shelf: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='physical_shelf_labels')", [], |r| r.get(0))?;
    let props = if has_properties {
        conn.query_row("SELECT display_name,model,filesystem,capacity_bytes,note,revision FROM disk_preferences WHERE catalog_key=?1 AND platform=?2 AND volume_id=?3",params![key,os,id],|r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<i64>>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?))).optional()?
    } else {
        None
    };
    let (name, model, fs, capacity, note, revision) = props.unwrap_or_default();
    let shelf = if has_shelf {
        conn.query_row("SELECT shelf_label FROM physical_shelf_labels WHERE catalog_key=?1 AND platform=?2 AND volume_id=?3", params![key,os,id], |r| r.get::<_,String>(0)).optional()?.unwrap_or_default()
    } else {
        String::new()
    };
    Ok(
        json!({"volume_id":id,"display_name":name,"shelf_label":shelf,"model":model,"filesystem":fs,"capacity_bytes":capacity,"note":note,"revision":revision}),
    )
}
pub fn get(v: &Value) -> Result<Value> {
    let (db, store, key, os) = paths(v)?;
    let id = volume_id(v)?;
    ensure_volume(&db, id)?;
    if !store.exists() {
        return Ok(
            json!({"volume_id":id,"display_name":"","shelf_label":"","model":"","filesystem":"","capacity_bytes":null,"note":"","revision":0}),
        );
    }
    let conn = Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    ensure_store(&conn)?;
    read_one(&conn, &key, os, id)
}
pub fn list(v: &Value) -> Result<Value> {
    let (db, store, key, os) = paths(v)?;
    let volumes = Catalog::open_readonly(&db)?.volumes()?;
    if volumes.len() > 10000 {
        return Err(invalid("Too many volumes for disk preferences"));
    }
    let conn = if store.exists() {
        let conn = Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        ensure_store(&conn)?;
        Some(conn)
    } else {
        None
    };
    let mut items = Vec::with_capacity(volumes.len());
    let mut groups: std::collections::BTreeMap<String, Vec<i64>> =
        std::collections::BTreeMap::new();
    for volume in volumes {
        let item = if let Some(conn) = &conn {
            read_one(conn, &key, os, volume.id)?
        } else {
            json!({"volume_id":volume.id,"display_name":"","shelf_label":"","model":"","filesystem":"","capacity_bytes":null,"note":"","revision":0})
        };
        let normalized = item["shelf_label"]
            .as_str()
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if !normalized.is_empty() {
            groups.entry(normalized).or_default().push(volume.id);
        }
        items.push(item);
    }
    let collisions: Vec<_> = groups
        .into_values()
        .filter(|ids| ids.len() > 1)
        .map(|mut ids| {
            ids.sort_unstable();
            json!({"volume_ids":ids})
        })
        .collect();
    Ok(json!({"items":items,"collisions":collisions}))
}

pub fn set(v: &Value) -> Result<Value> {
    let (db, store, key, os) = paths(v)?;
    let id = volume_id(v)?;
    ensure_volume(&db, id)?;
    if v.get("confirmed") != Some(&Value::Bool(true)) {
        return Err(invalid("Explicit Save is required"));
    }
    let name = field(v, "display_name", 160, false)?;
    if name.is_empty() {
        return Err(invalid("Display name must not be empty"));
    }
    let shelf = field(v, "shelf_label", 80, false)?;
    let model = field(v, "model", 160, false)?;
    let fs = field(v, "filesystem", 40, false)?;
    let note = field(v, "note", 4000, true)?;
    let capacity = match v.get("capacity_bytes") {
        None | Some(Value::Null) => None,
        Some(x) => Some(
            x.as_i64()
                .filter(|n| *n > 0)
                .ok_or_else(|| invalid("Capacity must be a positive integer or null"))?,
        ),
    };
    let expected = v
        .get("expected_revision")
        .and_then(Value::as_i64)
        .filter(|n| *n >= 0)
        .ok_or_else(|| invalid("Expected revision is required"))?;
    let expected_shelf = field(v, "expected_shelf_label", 80, false)?;
    if let Some(parent) = store.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(&store)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    ensure_store(&conn)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    crate::physical_disks::create_schema(&tx)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS disk_preferences (catalog_key TEXT NOT NULL,platform TEXT NOT NULL,volume_id INTEGER NOT NULL CHECK(volume_id>0),display_name TEXT NOT NULL,model TEXT NOT NULL,filesystem TEXT NOT NULL,capacity_bytes INTEGER,note TEXT NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),PRIMARY KEY(catalog_key,platform,volume_id));")?;
    let current = read_one(&tx, &key, os, id)?;
    if current["revision"].as_i64() != Some(expected)
        || current["shelf_label"].as_str() != Some(expected_shelf.as_str())
    {
        return Err(Error::new(
            "conflict",
            "Properties changed since opening; reload before saving",
        ));
    }
    let next = expected
        .checked_add(1)
        .ok_or_else(|| invalid("Revision overflow"))?;
    tx.execute("INSERT INTO disk_preferences(catalog_key,platform,volume_id,display_name,model,filesystem,capacity_bytes,note,revision) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(catalog_key,platform,volume_id) DO UPDATE SET display_name=excluded.display_name,model=excluded.model,filesystem=excluded.filesystem,capacity_bytes=excluded.capacity_bytes,note=excluded.note,revision=excluded.revision",params![key,os,id,name,model,fs,capacity,note,next])?;
    if shelf.is_empty() {
        tx.execute("DELETE FROM physical_shelf_labels WHERE catalog_key=?1 AND platform=?2 AND volume_id=?3",params![key,os,id])?;
    } else {
        tx.execute("INSERT INTO physical_shelf_labels(catalog_key,platform,volume_id,shelf_label) VALUES(?1,?2,?3,?4) ON CONFLICT(catalog_key,platform,volume_id) DO UPDATE SET shelf_label=excluded.shelf_label",params![key,os,id,shelf])?;
    }
    tx.commit()?;
    Ok(json!({"saved":true,"volume_id":id,"revision":next}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_json;
    fn call(v: &Value) -> Value {
        serde_json::from_str(&dispatch_json(&v.to_string())).unwrap()
    }
    #[test]
    fn save_round_trips_unicode_atomically_and_preserves_source() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("каталог.sqlite");
        let store = tmp.path().join("user store.sqlite");
        let media = tmp.path().join("media");
        std::fs::create_dir(&media).unwrap();
        std::fs::write(media.join("clip.txt"), b"synthetic").unwrap();
        let scan =
            call(&json!({"op":"scan","db":db,"root":media,"label":"Исходное имя","hash":false}));
        assert_eq!(scan["ok"], true, "{scan}");
        let id = scan["data"]["id"].as_i64().unwrap();
        let before = std::fs::read(&db).unwrap();
        let mut request = json!({"op":"disk_preferences_set","db":db,"user_store":store,"platform":"windows","volume_id":id,"display_name":"Кино 新","shelf_label":"BOX-01","model":"Тест","filesystem":"NTFS","capacity_bytes":1024,"note":"строка 1\nстрока 2","expected_revision":0,"expected_shelf_label":"","confirmed":true});
        assert_eq!(call(&request)["ok"], true);
        assert_eq!(std::fs::read(&db).unwrap(), before);
        let get = json!({"op":"disk_preferences_get","db":db,"user_store":store,"platform":"windows","volume_id":id});
        let result = call(&get);
        assert_eq!(result["data"]["display_name"], "Кино 新");
        assert_eq!(result["data"]["shelf_label"], "BOX-01");
        assert_eq!(result["data"]["note"], "строка 1\nстрока 2");
        let key = format!("sqlite:{}", db.to_string_lossy());
        let shelves = call(
            &json!({"op":"physical_shelf_labels","user_store":store,"platform":"windows","catalog_key":key,"volume_ids":[id]}),
        );
        assert_eq!(shelves["data"]["items"][0]["shelf_label"], "BOX-01");
        let physical = call(
            &json!({"op":"physical_status","user_store":store,"platform":"windows","catalog_key":key,"volume_ids":[id],"observed_ids":[],"probe_ok":true}),
        );
        assert_eq!(physical["data"]["items"][0]["state"], "unverified");
        request["display_name"] = json!("Старое изменение");
        request["shelf_label"] = json!("BOX-02");
        let conflict = call(&request);
        assert_eq!(conflict["error"]["code"], "conflict");
        assert_eq!(call(&get)["data"]["shelf_label"], "BOX-01");
        assert_eq!(std::fs::read(&db).unwrap(), before);
        request["expected_revision"] = json!(1);
        request["expected_shelf_label"] = json!("BOX-01");
        request["shelf_label"] = json!("");
        assert_eq!(call(&request)["ok"], true);
        assert_eq!(call(&get)["data"]["shelf_label"], "");
    }
    #[test]
    fn duplicate_stickers_and_external_edits_are_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("catalog.sqlite");
        let store = tmp.path().join("sidecar.sqlite");
        for n in 1..=2 {
            let root = tmp.path().join(format!("disk-{n}"));
            std::fs::create_dir(&root).unwrap();
            let r = call(
                &json!({"op":"scan","db":db,"root":root,"label":format!("disk {n}"),"hash":false}),
            );
            assert_eq!(r["ok"], true, "{r}");
        }
        let before = std::fs::read(&db).unwrap();
        let mut set = json!({"op":"disk_preferences_set","db":db,"user_store":store,"platform":"windows","volume_id":1,"display_name":"Name 1","shelf_label":"Shelf  A","model":"","filesystem":"","capacity_bytes":null,"note":"","expected_revision":0,"expected_shelf_label":"","confirmed":true});
        assert_eq!(call(&set)["ok"], true);
        set["volume_id"] = json!(2);
        set["display_name"] = json!("Name 2");
        set["shelf_label"] = json!(" shelf a ");
        assert_eq!(call(&set)["ok"], true);
        let list = call(
            &json!({"op":"disk_preferences_list","db":db,"user_store":store,"platform":"windows"}),
        );
        assert_eq!(list["data"]["items"].as_array().unwrap().len(), 2);
        assert_eq!(list["data"]["collisions"][0]["volume_ids"], json!([1, 2]));
        let key = format!("sqlite:{}", db.to_string_lossy());
        let external = call(
            &json!({"op":"physical_shelf_label_set","catalog_key":key,"user_store":store,"platform":"windows","volume_id":1,"shelf_label":"OTHER","confirmed":true}),
        );
        assert_eq!(external["ok"], true);
        set["volume_id"] = json!(1);
        set["expected_revision"] = json!(1);
        set["expected_shelf_label"] = json!("Shelf  A");
        assert_eq!(call(&set)["error"]["code"], "conflict");
        let get = call(
            &json!({"op":"disk_preferences_get","db":db,"user_store":store,"platform":"windows","volume_id":1}),
        );
        assert_eq!(get["data"]["display_name"], "Name 1");
        assert_eq!(get["data"]["shelf_label"], "OTHER");
        assert_eq!(std::fs::read(&db).unwrap(), before);
    }

    #[test]
    fn refuse_catalog_alias_missing_volume_invalid_fields_and_unrelated_store() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("catalog.sqlite");
        let media = tmp.path().join("media");
        std::fs::create_dir(&media).unwrap();
        assert_eq!(
            call(&json!({"op":"scan","db":db,"root":media,"label":"one","hash":false}))["ok"],
            true
        );
        let alias = tmp.path().join("alias.sqlite");
        std::fs::hard_link(&db, &alias).unwrap();
        let before = std::fs::read(&db).unwrap();
        let mut req = json!({"op":"disk_preferences_set","db":db,"user_store":alias,"platform":"windows","volume_id":1,"display_name":"Name","shelf_label":"S","model":"","filesystem":"","capacity_bytes":null,"note":"","expected_revision":0,"expected_shelf_label":"","confirmed":true});
        assert_eq!(call(&req)["ok"], false);
        assert_eq!(std::fs::read(&db).unwrap(), before);
        let other = tmp.path().join("unrelated.sqlite");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE precious(x)")
            .unwrap();
        req["user_store"] = json!(other);
        assert_eq!(call(&req)["ok"], false);
        req["user_store"] = json!(tmp.path().join("side.sqlite"));
        req["volume_id"] = json!(999);
        assert_eq!(call(&req)["ok"], false);
        req["volume_id"] = json!(1);
        req["display_name"] = json!("");
        assert_eq!(call(&req)["ok"], false);
        assert_eq!(std::fs::read(&db).unwrap(), before);
    }
}
