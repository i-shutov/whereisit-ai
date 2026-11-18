//! User-owned shelf labels, independent from catalog/CTF and physical OS identity.
//! Same on-disk sidecar convention as existing Win32 and SwiftUI clients.
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ShelfContext {
    pub user_store: String,
    pub catalog_key: String,
    pub platform: &'static str,
}

impl ShelfContext {
    pub fn from_sqlite(db: &str) -> Result<Self, String> {
        let db_path =
            std::fs::canonicalize(db).map_err(|e| format!("Cannot resolve catalog path: {e}"))?;
        // Existing Win32/SwiftUI native clients scope labels to the original
        // absolute catalog path string, not OS-canonicalized \\?\ variants.
        let identity = if Path::new(db).is_absolute() {
            db.to_owned()
        } else {
            db_path.to_string_lossy().into_owned()
        };
        let catalog_key = format!("sqlite:{identity}");
        if catalog_key.len() > 2048 {
            return Err("Catalog identity exceeds the service limit".to_owned());
        }
        let (base, platform) = if cfg!(target_os = "windows") {
            (
                std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
                "windows",
            )
        } else if cfg!(target_os = "macos") {
            (
                std::env::var_os("HOME")
                    .map(|h| PathBuf::from(h).join("Library/Application Support")),
                "macos",
            )
        } else {
            (
                std::env::var_os("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .or_else(|| {
                        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))
                    }),
                "macos",
            )
        };
        let base = base.ok_or("Unable to locate the private user configuration directory")?;
        if !base.is_absolute() {
            return Err("User configuration directory is not absolute".to_owned());
        }
        let store = base
            .join("AtlasCatalog")
            .join("physical-volume-bindings.sqlite");
        let user_store = store.to_string_lossy().into_owned();
        if user_store.len() > 4096 {
            return Err("Private sidecar path exceeds the service limit".to_owned());
        }
        Ok(Self {
            user_store,
            catalog_key,
            platform,
        })
    }

    pub fn list_request(&self, volume_ids: &[i64]) -> Value {
        json!({
            "op":"physical_shelf_labels",
            "user_store":self.user_store,
            "catalog_key":self.catalog_key,
            "platform":self.platform,
            "volume_ids":volume_ids,
            "observed_ids":[],
            "probe_ok":false,
        })
    }

    pub fn save_request(&self, db: &str, volume_id: i64, label: &str, expected: &str) -> Value {
        json!({
            "op":"disk_preferences_legacy_shelf_set", "db":db,
            "user_store":self.user_store,
            "catalog_key":self.catalog_key,
            "platform":self.platform,
            "volume_id":volume_id,
            "shelf_label":label,
            "expected_shelf_label":expected,
            "confirmed":true,
            "observed_ids":[],
            "probe_ok":false,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct ShelfSnapshot {
    pub labels: HashMap<i64, String>,
    pub collisions: HashMap<i64, Vec<i64>>,
}

impl ShelfSnapshot {
    pub fn parse(data: &Value, volume_ids: &[i64]) -> Result<Self, String> {
        let selected: HashSet<_> = volume_ids.iter().copied().collect();
        let mut out = Self::default();
        let rows = data
            .get("items")
            .and_then(Value::as_array)
            .ok_or("Missing shelf label items[]")?;
        for row in rows {
            let id = row
                .get("volume_id")
                .and_then(Value::as_i64)
                .filter(|id| *id > 0)
                .ok_or("Invalid shelf volume ID")?;
            let label = row
                .get("shelf_label")
                .and_then(Value::as_str)
                .ok_or("Invalid shelf label")?;
            if !selected.contains(&id) {
                continue;
            }
            out.labels.insert(id, label.to_owned());
        }
        let collision_rows = data
            .get("collisions")
            .and_then(Value::as_array)
            .ok_or("Missing shelf label collisions[]")?;
        for row in collision_rows {
            let ids = row
                .get("volume_ids")
                .and_then(Value::as_array)
                .ok_or("Invalid collision volume ids")?;
            let members: Vec<i64> = ids
                .iter()
                .filter_map(Value::as_i64)
                .filter(|id| selected.contains(id))
                .collect();
            if members.len() > 1 {
                for id in &members {
                    out.collisions.insert(*id, members.clone());
                }
            }
        }
        Ok(out)
    }

    pub fn label(&self, id: i64) -> &str {
        self.labels.get(&id).map(String::as_str).unwrap_or("")
    }

    pub fn collision_other_disks(&self, id: i64) -> Vec<i64> {
        self.collisions
            .get(&id)
            .map(|members| {
                members
                    .iter()
                    .copied()
                    .filter(|other| *other != id)
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub fn valid_label(text: &str) -> bool {
    text.len() <= 320 && text.chars().count() <= 80 && !text.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(request: Value) -> Value {
        serde_json::from_str(&atlas_service::dispatch_json(&request.to_string())).unwrap()
    }

    #[test]
    fn saving_a_sticker_never_claims_usb_presence_or_modifies_catalog_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let catalog = tmp.path().join("catalog.sqlite");
        let init = call(json!({"op":"init","db":catalog}));
        assert_eq!(init["ok"], true, "{init}");
        let root = tmp.path().join("synthetic");
        std::fs::create_dir(&root).unwrap();
        for label in ["one", "two"] {
            let scan = call(json!({"op":"scan", "db":catalog, "root":root, "label":label}));
            assert_eq!(scan["ok"], true, "{scan}");
        }
        let sqlite_before = std::fs::read(&catalog).unwrap();

        let ctx = ShelfContext {
            user_store: tmp
                .path()
                .join("private/physical-volume-bindings.sqlite")
                .to_string_lossy()
                .into_owned(),
            catalog_key: format!("sqlite:{}", catalog.display()),
            platform: "windows",
        };
        let no_sticker = call(ctx.list_request(&[1, 2]));
        assert_eq!(no_sticker["ok"], true, "{no_sticker}");
        assert!(no_sticker["data"]["items"].as_array().unwrap().is_empty());
        let db = catalog.to_str().unwrap();
        let save = call(ctx.save_request(db, 1, "BOX-17", ""));
        assert_eq!(save["ok"], true, "{save}");
        assert_eq!(save["data"]["saved"], true);
        let other = call(ctx.save_request(db, 2, "BOX-17", ""));
        assert_eq!(other["ok"], true, "{other}");
        let response = call(ctx.list_request(&[1, 2]));
        assert_eq!(response["ok"], true, "{response}");
        let state = ShelfSnapshot::parse(&response["data"], &[1, 2]).unwrap();
        assert_eq!(state.label(1), "BOX-17");
        assert_eq!(state.collision_other_disks(1), vec![2]);
        let status = call(json!({
            "op":"physical_status",
            "user_store":ctx.user_store,
            "catalog_key":ctx.catalog_key,
            "platform":ctx.platform,
            "volume_ids":[1,2], "observed_ids":[], "probe_ok":true
        }));
        assert_eq!(status["ok"], true, "{status}");
        assert_eq!(status["data"]["items"][0]["state"], "unverified");
        assert_eq!(status["data"]["items"][1]["state"], "unverified");

        let clear = call(ctx.save_request(db, 1, "", "BOX-17"));
        assert_eq!(clear["ok"], true);
        assert_eq!(
            ShelfSnapshot::parse(&call(ctx.list_request(&[1, 2]))["data"], &[1, 2])
                .unwrap()
                .label(1),
            ""
        );
        let sqlite_after = std::fs::read(&catalog).unwrap();
        assert_eq!(sqlite_before, sqlite_after);
    }

    #[test]
    fn invalid_shelf_labels_and_malformed_data_fail_closed() {
        assert!(valid_label(""));
        assert!(valid_label("Носитель A"));
        assert!(!valid_label("invalid\nmultiline"));
        assert!(!valid_label(&"a".repeat(81)));
        assert!(ShelfSnapshot::parse(&json!({"items":[]}), &[1]).is_err());
    }
}
