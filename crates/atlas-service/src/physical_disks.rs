//! User-owned physical-volume binding. Never modifies the catalog or linked CTF.
//! Native frontends supply live OS-enumerated IDs, never historical drive letters.
use crate::text;
use atlas_core::{Error, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Clone)]
struct Binding {
    volume_id: i64,
    physical_id: String,
}

fn invalid(message: &str) -> Error {
    Error::new("invalid_physical_identity", message)
}

fn store_path(request: &Value) -> Result<&Path> {
    let value = text(request, "user_store")?;
    let path = Path::new(value);
    if !path.is_absolute() || value.len() > 4096 {
        return Err(invalid(
            "user_store must be a bounded absolute user-owned path",
        ));
    }
    Ok(path)
}

fn scope(request: &Value) -> Result<&str> {
    let value = text(request, "catalog_key")?;
    if value.len() > 2048
        || !(value.starts_with("ctf:") || value.starts_with("sqlite:"))
        || value.ends_with(':')
    {
        return Err(invalid(
            "catalog_key must identify an exact catalog/source generation",
        ));
    }
    Ok(value)
}

fn platform(request: &Value) -> Result<&str> {
    match text(request, "platform")? {
        "windows" => Ok("windows"),
        "macos" => Ok("macos"),
        _ => Err(invalid("Unsupported physical identity platform")),
    }
}

fn normalized_id(raw: &str, platform: &str) -> Result<String> {
    let id = raw.trim().to_ascii_lowercase();
    let prefix = if platform == "windows" {
        "win:"
    } else {
        "mac:"
    };
    if id.len() < prefix.len() + 8
        || id.len() > 160
        || !id.starts_with(prefix)
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b":{}-".contains(&c))
    {
        return Err(invalid(
            "Physical identity must be a platform-specific volume UUID",
        ));
    }
    Ok(id)
}

fn observed(request: &Value, platform: &str) -> Result<HashMap<String, usize>> {
    let values = request
        .get("observed_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("observed_ids must be a live OS-probe array"))?;
    if values.len() > 256 {
        return Err(invalid("Too many mounted volumes"));
    }
    let mut result = HashMap::new();
    for value in values {
        let id = normalized_id(
            value
                .as_str()
                .ok_or_else(|| invalid("Observed volume ID must be a string"))?,
            platform,
        )?;
        *result.entry(id).or_insert(0) += 1;
    }
    Ok(result)
}

fn volume_id(request: &Value) -> Result<i64> {
    request
        .get("volume_id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| invalid("volume_id must be a positive catalog identifier"))
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS physical_bindings (
           catalog_key TEXT NOT NULL,
           platform TEXT NOT NULL,
           volume_id INTEGER NOT NULL CHECK(volume_id > 0),
           physical_id TEXT NOT NULL,
           PRIMARY KEY (catalog_key, platform, volume_id),
           UNIQUE (catalog_key, platform, physical_id)
         );
         CREATE TABLE IF NOT EXISTS physical_shelf_labels (
           catalog_key TEXT NOT NULL,
           platform TEXT NOT NULL,
           volume_id INTEGER NOT NULL CHECK(volume_id > 0),
           shelf_label TEXT NOT NULL,
           PRIMARY KEY (catalog_key, platform, volume_id)
         );",
    )?;
    Ok(())
}

fn bindings(path: &Path, key: &str, os: &str) -> Result<Vec<Binding>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = conn.prepare(
        "SELECT volume_id, physical_id FROM physical_bindings
         WHERE catalog_key=?1 AND platform=?2",
    )?;
    let rows = statement
        .query_map(params![key, os], |row| {
            Ok(Binding {
                volume_id: row.get(0)?,
                physical_id: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn classify(
    volume: i64,
    known: &[Binding],
    seen: &HashMap<String, usize>,
    probe_ok: bool,
) -> &'static str {
    let Some(binding) = known.iter().find(|binding| binding.volume_id == volume) else {
        return "unverified";
    };
    if !probe_ok {
        return "unverified";
    }
    if known
        .iter()
        .filter(|other| other.physical_id == binding.physical_id)
        .count()
        != 1
    {
        return "conflict";
    }
    match seen.get(&binding.physical_id).copied().unwrap_or(0) {
        0 => "offline",
        1 => "connected",
        _ => "conflict",
    }
}

/// No pairing => unverified, even when the last recorded root path is mounted.
/// Offline means a previously *explicitly paired* UUID is absent from a successful probe.
pub fn status(request: &Value) -> Result<Value> {
    let os = platform(request)?;
    let key = scope(request)?;
    let path = store_path(request)?;
    let probe_ok = request
        .get("probe_ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("probe_ok must explicitly record OS probe success"))?;
    let ids = request
        .get("volume_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("volume_ids must be an array"))?;
    if ids.len() > 512 {
        return Err(invalid("Too many catalog volume IDs"));
    }
    let mut unique = HashSet::new();
    let mut volumes = Vec::new();
    for value in ids {
        let id = value
            .as_i64()
            .filter(|id| *id > 0)
            .ok_or_else(|| invalid("Invalid catalog volume ID"))?;
        if !unique.insert(id) {
            return Err(invalid("Duplicate catalog volume ID"));
        }
        volumes.push(id);
    }
    let seen = if probe_ok {
        observed(request, os)?
    } else {
        HashMap::new()
    };
    let known = bindings(path, key, os)?;
    let items: Vec<_> = volumes
        .into_iter()
        .map(|id| json!({"volume_id":id,"state":classify(id, &known, &seen, probe_ok)}))
        .collect();
    Ok(json!({"items":items,"probe_ok":probe_ok}))
}

/// A UI must obtain this UUID from a fresh native OS probe and explicitly ask
/// the operator to bind the selected catalog disk to that *physical* volume.
pub fn pair(request: &Value) -> Result<Value> {
    let os = platform(request)?;
    let key = scope(request)?;
    let path = store_path(request)?;
    let id = volume_id(request)?;
    if request.get("confirmed") != Some(&Value::Bool(true)) {
        return Err(invalid("Explicit operator confirmation is required"));
    }
    if request.get("probe_ok") != Some(&Value::Bool(true)) {
        return Err(invalid(
            "A successful live OS probe is required for pairing",
        ));
    }
    let physical_id = normalized_id(text(request, "physical_id")?, os)?;
    if observed(request, os)?.get(&physical_id) != Some(&1) {
        return Err(Error::new(
            "identity_conflict",
            "Cannot pair absent or ambiguous OS volume identity",
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(path)?;
    create_schema(&conn)?;
    let transaction = conn.transaction()?;
    let previous: Option<String> = transaction
        .query_row(
            "SELECT physical_id FROM physical_bindings
             WHERE catalog_key=?1 AND platform=?2 AND volume_id=?3",
            params![key, os, id],
            |row| row.get(0),
        )
        .optional()?;
    if previous.as_ref().is_some_and(|old| old != &physical_id) {
        return Err(Error::new(
            "identity_conflict",
            "Catalog disk already paired; unpair explicitly before reassigning",
        ));
    }
    let other: Option<i64> = transaction
        .query_row(
            "SELECT volume_id FROM physical_bindings
             WHERE catalog_key=?1 AND platform=?2 AND physical_id=?3",
            params![key, os, physical_id],
            |row| row.get(0),
        )
        .optional()?;
    if other.is_some_and(|other_id| other_id != id) {
        return Err(Error::new(
            "identity_conflict",
            "This physical volume is already paired to another catalog disk",
        ));
    }
    transaction.execute(
        "INSERT OR IGNORE INTO physical_bindings
         (catalog_key,platform,volume_id,physical_id) VALUES (?1,?2,?3,?4)",
        params![key, os, id, physical_id],
    )?;
    transaction.commit()?;
    Ok(json!({"paired":true,"volume_id":id}))
}

pub fn unpair(request: &Value) -> Result<Value> {
    let os = platform(request)?;
    let key = scope(request)?;
    let path = store_path(request)?;
    let id = volume_id(request)?;
    if request.get("confirmed") != Some(&Value::Bool(true)) {
        return Err(invalid("Explicit operator confirmation is required"));
    }
    if !path.exists() {
        return Ok(json!({"removed":false}));
    }
    let conn = Connection::open(path)?;
    let count = conn.execute(
        "DELETE FROM physical_bindings
         WHERE catalog_key=?1 AND platform=?2 AND volume_id=?3",
        params![key, os, id],
    )?;
    Ok(json!({"removed":count > 0}))
}

// Equal human shelf tags may be intentional, but they make disk retrieval ambiguous.
// This is an ID-only, generation/platform-scoped warning, not OS mount evidence.
fn shelf_collisions(stored: &[(i64, String)]) -> Vec<Value> {
    let mut tags: HashMap<String, Vec<i64>> = HashMap::new();
    for (disk, tag) in stored {
        let normalized = tag
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if !normalized.is_empty() {
            tags.entry(normalized).or_default().push(*disk);
        }
    }
    let mut collisions: Vec<Vec<i64>> = tags.into_values().filter(|ids| ids.len() > 1).collect();
    for ids in &mut collisions {
        ids.sort_unstable();
    }
    collisions.sort();
    collisions
        .into_iter()
        .map(|volume_ids| json!({"volume_ids":volume_ids}))
        .collect()
}

/// Private, operator-authored shelf hints: never evidence of mounted USB media.
/// No original CTF or catalog is opened by these operations.
pub fn shelf_labels(request: &Value) -> Result<Value> {
    let os = platform(request)?;
    let key = scope(request)?;
    let path = store_path(request)?;
    let ids = request
        .get("volume_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("volume_ids must be an array"))?;
    if ids.len() > 512 {
        return Err(invalid("Too many catalog volumes"));
    }
    let selected: HashSet<i64> = ids
        .iter()
        .map(|v| {
            v.as_i64()
                .filter(|id| *id > 0)
                .ok_or_else(|| invalid("Invalid catalog volume ID"))
        })
        .collect::<Result<_>>()?;
    if !path.exists() {
        return Ok(json!({"items":[],"collisions":[]}));
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let exists: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='physical_shelf_labels'",
        [],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Ok(json!({"items":[],"collisions":[]}));
    }
    let mut statement = conn.prepare(
        "SELECT volume_id,shelf_label FROM physical_shelf_labels
         WHERE catalog_key=?1 AND platform=?2 ORDER BY volume_id",
    )?;
    let stored = statement
        .query_map(params![key, os], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let visible: Vec<_> = stored
        .into_iter()
        .filter(|(id, _)| selected.contains(id))
        .collect();
    let collisions = shelf_collisions(&visible);
    let items: Vec<_> = visible
        .into_iter()
        .map(|(volume_id, shelf_label)| json!({"volume_id":volume_id,"shelf_label":shelf_label}))
        .collect();
    Ok(json!({"items":items,"collisions":collisions}))
}

/// Explicit Save alone updates the user-side SQLite; empty Save removes tag.
/// The physical_status and pairing tables are unaffected.
pub fn set_shelf_label(request: &Value) -> Result<Value> {
    let os = platform(request)?;
    let key = scope(request)?;
    let path = store_path(request)?;
    let volume = volume_id(request)?;
    if request.get("confirmed") != Some(&Value::Bool(true)) {
        return Err(invalid(
            "Explicit operator confirmation required for shelf labels",
        ));
    }
    let label = text(request, "shelf_label")?.trim();
    if label.len() > 320 || label.chars().count() > 80 || label.chars().any(|ch| ch.is_control()) {
        return Err(invalid(
            "Shelf label must be one line of at most 80 characters",
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    create_schema(&conn)?;
    if label.is_empty() {
        conn.execute(
            "DELETE FROM physical_shelf_labels WHERE catalog_key=?1 AND platform=?2 AND volume_id=?3",
            params![key, os, volume],
        )?;
    } else {
        conn.execute(
            "INSERT INTO physical_shelf_labels (catalog_key,platform,volume_id,shelf_label)
             VALUES (?1,?2,?3,?4)
             ON CONFLICT(catalog_key,platform,volume_id)
             DO UPDATE SET shelf_label=excluded.shelf_label",
            params![key, os, volume, label],
        )?;
    }
    Ok(json!({"saved":true,"volume_id":volume,"shelf_label":label}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_json;

    fn request(value: Value) -> Value {
        serde_json::from_str(&dispatch_json(&value.to_string())).unwrap()
    }

    #[test]
    fn shelf_tags_are_private_explicit_generation_scoped_and_not_mount_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let sidecar = temp
            .path()
            .join("user-tags.sqlite")
            .to_string_lossy()
            .into_owned();
        let base = json!({"user_store":sidecar,"catalog_key":"ctf:synthetic-a",
            "platform":"windows","volume_ids":[1,2],"observed_ids":[],"probe_ok":true});
        let mut list = base.clone();
        list["op"] = json!("physical_shelf_labels");
        assert_eq!(request(list.clone())["data"]["items"], json!([]));
        let mut save = base.clone();
        save["op"] = json!("physical_shelf_label_set");
        save["volume_id"] = json!(2);
        save["shelf_label"] = json!("Стеллаж Б / USB-02");
        assert_eq!(request(save.clone())["ok"], false);
        save["confirmed"] = json!(true);
        assert_eq!(request(save.clone())["data"]["saved"], true);
        assert_eq!(
            request(list.clone())["data"]["items"][0]["shelf_label"],
            "Стеллаж Б / USB-02"
        );

        let mut state = base.clone();
        state["op"] = json!("physical_status");
        assert_eq!(request(state)["data"]["items"][1]["state"], "unverified");
        list["catalog_key"] = json!("ctf:synthetic-b");
        assert_eq!(request(list.clone())["data"]["items"], json!([]));
        list["catalog_key"] = json!("ctf:synthetic-a");
        list["platform"] = json!("macos");
        assert_eq!(request(list.clone())["data"]["items"], json!([]));
        list["platform"] = json!("windows");
        list["volume_ids"] = json!([1]);
        assert_eq!(request(list.clone())["data"]["items"], json!([]));

        save["shelf_label"] = json!("line\nbreak");
        assert_eq!(request(save.clone())["ok"], false);
        save["shelf_label"] = json!("X".repeat(81));
        assert_eq!(request(save.clone())["ok"], false);
        save["shelf_label"] = json!("");
        assert_eq!(request(save)["data"]["saved"], true);
        list["volume_ids"] = json!([1, 2]);
        assert_eq!(request(list)["data"]["items"], json!([]));
    }

    #[test]
    fn repeated_stickers_are_scoped_warnings_not_mount_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp
            .path()
            .join("user.sqlite")
            .to_string_lossy()
            .into_owned();
        let base = json!({"op":"physical_shelf_labels","user_store":path,
            "catalog_key":"ctf:synthetic-a","platform":"windows","volume_ids":[1,2,3,4],
            "observed_ids":[],"probe_ok":true});
        for (disk, tag) in [
            (1, "Стеллаж   Б"),
            (2, "стеллаж б"),
            (3, "USB-04"),
            (4, "usb-04"),
        ] {
            let mut save = base.clone();
            save["op"] = json!("physical_shelf_label_set");
            save["volume_id"] = json!(disk);
            save["shelf_label"] = json!(tag);
            save["confirmed"] = json!(true);
            assert_eq!(request(save)["data"]["saved"], true);
        }
        assert_eq!(
            request(base.clone())["data"]["collisions"],
            json!([{"volume_ids":[1,2]},{"volume_ids":[3,4]}])
        );
        let mut subset = base.clone();
        subset["volume_ids"] = json!([1, 3]);
        assert_eq!(request(subset)["data"]["collisions"], json!([]));
        let mut other = base.clone();
        other["catalog_key"] = json!("ctf:synthetic-b");
        assert_eq!(request(other.clone())["data"]["collisions"], json!([]));
        other["catalog_key"] = json!("ctf:synthetic-a");
        other["platform"] = json!("macos");
        assert_eq!(request(other)["data"]["collisions"], json!([]));
        let mut state = base.clone();
        state["op"] = json!("physical_status");
        assert_eq!(request(state)["data"]["items"][0]["state"], "unverified");
        let mut clear = base.clone();
        clear["op"] = json!("physical_shelf_label_set");
        clear["volume_id"] = json!(2);
        clear["shelf_label"] = json!("");
        clear["confirmed"] = json!(true);
        assert_eq!(request(clear)["data"]["saved"], true);
        assert_eq!(
            request(base)["data"]["collisions"],
            json!([{"volume_ids":[3,4]}])
        );
    }

    #[test]
    fn unknown_offline_online_and_generation_scope_are_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("identity.sqlite")
            .to_string_lossy()
            .into_owned();
        let base = json!({"user_store":path,"catalog_key":"ctf:synthetic-v1",
            "platform":"macos","volume_ids":[1,2],"observed_ids":["mac:aaaa-1111"],"probe_ok":true});
        let mut read = base.clone();
        read["op"] = json!("physical_status");
        assert_eq!(
            request(read.clone())["data"]["items"][0]["state"],
            "unverified"
        );
        let mut pair = base.clone();
        pair["op"] = json!("physical_pair");
        pair["volume_id"] = json!(1);
        pair["physical_id"] = json!("mac:AAAA-1111");
        pair["confirmed"] = json!(true);
        assert_eq!(request(pair)["data"]["paired"], true);
        assert_eq!(
            request(read.clone())["data"]["items"][0]["state"],
            "connected"
        );
        read["observed_ids"] = json!([]);
        assert_eq!(
            request(read.clone())["data"]["items"][0]["state"],
            "offline"
        );
        read["probe_ok"] = json!(false);
        assert_eq!(
            request(read.clone())["data"]["items"][0]["state"],
            "unverified"
        );
        read["catalog_key"] = json!("ctf:synthetic-v2");
        read["probe_ok"] = json!(true);
        assert_eq!(request(read)["data"]["items"][0]["state"], "unverified");
    }

    #[test]
    fn pairing_is_explicit_and_conflicts_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("identity.sqlite")
            .to_string_lossy()
            .into_owned();
        let mut pair = json!({"op":"physical_pair","user_store":path,
            "catalog_key":"sqlite:synthetic","platform":"windows","volume_id":1,
            "physical_id":"win:{aaaa-bbbb}:12345678","observed_ids":["win:{aaaa-bbbb}:12345678"],"probe_ok":true});
        assert_eq!(request(pair.clone())["ok"], false);
        pair["confirmed"] = json!(true);
        pair["probe_ok"] = json!(false);
        assert_eq!(request(pair.clone())["ok"], false);
        pair["probe_ok"] = json!(true);
        assert_eq!(request(pair.clone())["ok"], true);
        pair["volume_id"] = json!(2);
        assert_eq!(request(pair.clone())["error"]["code"], "identity_conflict");
        pair["volume_id"] = json!(1);
        pair["physical_id"] = json!("win:{cccc-dddd}:12345678");
        pair["observed_ids"] = json!(["win:{cccc-dddd}:12345678"]);
        assert_eq!(request(pair.clone())["error"]["code"], "identity_conflict");
        pair["observed_ids"] = json!(["win:{cccc-dddd}:12345678", "win:{cccc-dddd}:12345678"]);
        assert_eq!(request(pair)["error"]["code"], "identity_conflict");
    }

    #[test]
    fn incomplete_os_scan_never_claims_connected_or_offline() {
        let bindings = [Binding {
            volume_id: 11,
            physical_id: "mac:demo-1111".into(),
        }];
        let seen = HashMap::from([("mac:demo-1111".into(), 1)]);
        assert_eq!(classify(11, &bindings, &seen, true), "connected");
        assert_eq!(classify(11, &bindings, &seen, false), "unverified");
        assert_eq!(classify(11, &bindings, &HashMap::new(), true), "offline");
        assert_eq!(
            classify(11, &bindings, &HashMap::new(), false),
            "unverified"
        );
    }

    #[test]
    fn duplicate_probe_id_cannot_claim_connection() {
        let binding = Binding {
            volume_id: 4,
            physical_id: "mac:duplicate-uuid".into(),
        };
        let seen = HashMap::from([("mac:duplicate-uuid".into(), 2)]);
        assert_eq!(classify(4, &[binding], &seen, true), "conflict");
        assert_eq!(classify(5, &[], &seen, true), "unverified");
    }
}
