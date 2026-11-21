//! Explicit replacement of one native filesystem snapshot.
//!
//! The live root is derived from a freshly probed OS UUID already paired in
//! the private sidecar. No caller-supplied or historical path authorizes this
//! operation. CTF-imported volumes remain immutable.

use crate::{os_probe, Error, Result};
use atlas_core::{Catalog, Volume};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MAX_DB_PATH: usize = 2048;
const MAX_STORE_PATH: usize = 4096;
const MAX_JOB_ID: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Candidate {
    physical_id: String,
    mount_root: String,
}

fn invalid(code: &'static str, message: &'static str) -> Error {
    Error::new(code, message)
}

fn native_platform() -> Option<&'static str> {
    #[cfg(windows)]
    {
        Some("windows")
    }
    #[cfg(target_os = "macos")]
    {
        Some("macos")
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

fn requested_platform(request: &Value) -> Result<&'static str> {
    let platform = request
        .get("platform")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("invalid_request", "platform must be a string"))?;
    match platform {
        "windows" => Ok("windows"),
        "macos" => Ok("macos"),
        _ => Err(invalid("invalid_request", "Unsupported physical platform")),
    }
}

fn absolute_path(request: &Value, key: &str, max_len: usize) -> Result<(String, PathBuf)> {
    let text = request
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("invalid_request", "Required path is missing"))?;
    let path = PathBuf::from(text);
    if !path.is_absolute() || text.len() > max_len || text.chars().any(char::is_control) {
        return Err(invalid(
            "invalid_request",
            "Paths must be bounded absolute paths",
        ));
    }
    Ok((text.to_owned(), path))
}

fn request_ids(request: &Value) -> Result<(i64, &str)> {
    let volume_id = request
        .get("volume_id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| invalid("invalid_request", "volume_id must be positive"))?;
    let job_id = request
        .get("job_id")
        .and_then(Value::as_str)
        .filter(|id| {
            !id.is_empty()
                && id.len() <= MAX_JOB_ID
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
        })
        .ok_or_else(|| invalid("invalid_request", "job_id is invalid"))?;
    Ok((volume_id, job_id))
}

fn read_only(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(std::time::Duration::from_secs(3))?;
    Ok(conn)
}

fn preflight_native_volume(db: &Path, volume_id: i64) -> Result<()> {
    if !db.is_file() {
        return Err(invalid("not_found", "Catalog does not exist"));
    }
    let conn = read_only(db)?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM volumes WHERE id=?1)",
        [volume_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(invalid(
            "not_found",
            "Volume does not exist in this catalog",
        ));
    }
    let has_ctf_volumes: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ctf_volumes')",
        [],
        |row| row.get(0),
    )?;
    if has_ctf_volumes {
        let is_ctf_volume: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM ctf_volumes WHERE volume_id=?1)",
            [volume_id],
            |row| row.get(0),
        )?;
        if is_ctf_volume {
            return Err(invalid(
                "ctf_volume_read_only",
                "Imported CTF volumes cannot be rescanned from a filesystem",
            ));
        }
    }
    Ok(())
}

fn validate_private_store(db: &Path, store: &Path) -> Result<()> {
    if !store.is_file() {
        return Err(invalid(
            "unpaired_volume",
            "No saved physical volume pairing exists",
        ));
    }
    let db_real = db.canonicalize()?;
    let store_real = store.canonicalize()?;
    if same_file::is_same_file(&db_real, &store_real)? {
        return Err(invalid(
            "invalid_user_store",
            "Private binding store aliases the catalog",
        ));
    }
    Ok(())
}

fn read_binding(store: &Path, catalog_key: &str, platform: &str, volume_id: i64) -> Result<String> {
    let conn = read_only(store)?;
    let table: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='physical_bindings')",
        [], |row| row.get(0)
    )?;
    if !table {
        return Err(invalid(
            "unpaired_volume",
            "No saved physical volume pairing exists",
        ));
    }
    let mut statement = conn.prepare(
        "SELECT physical_id FROM physical_bindings WHERE catalog_key=?1 AND platform=?2 AND volume_id=?3"
    )?;
    let values = statement
        .query_map(params![catalog_key, platform, volume_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if values.len() != 1 {
        return Err(invalid(
            "unpaired_volume",
            "Volume has no unique saved physical pairing",
        ));
    }
    let id = values.into_iter().next().expect("one physical binding");
    let owners: i64 = conn.query_row(
        "SELECT COUNT(*) FROM physical_bindings WHERE catalog_key=?1 AND platform=?2 AND physical_id=?3",
        params![catalog_key, platform, id], |row| row.get(0)
    )?;
    if owners != 1 {
        return Err(invalid(
            "identity_conflict",
            "Saved physical pairing is ambiguous",
        ));
    }
    Ok(id)
}

fn valid_id(value: &str, platform: &str) -> bool {
    let Some(uuid) = value.strip_prefix(if platform == "windows" {
        "win:"
    } else {
        "mac:"
    }) else {
        return false;
    };
    let bytes = uuid.as_bytes();
    let body = if platform == "windows" {
        // Keep the historical Windows identity shape: OS volume GUID plus
        // the OS-reported filesystem serial. Both pieces come from the volume
        // API; neither a label nor drive letter participates in identity.
        if bytes.len() != 47
            || bytes[0] != b'{'
            || bytes[37] != b'}'
            || bytes[38] != b':'
            || !bytes[39..].iter().all(u8::is_ascii_hexdigit)
        {
            return false;
        }
        &bytes[1..37]
    } else {
        bytes
    };
    body.len() == 36
        && body.iter().copied().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

fn mounted_candidate(probe: &Value, platform: &str, expected_id: &str) -> Result<Candidate> {
    if probe.get("probe_ok").and_then(Value::as_bool) != Some(true)
        || probe.get("platform").and_then(Value::as_str) != Some(platform)
    {
        return Err(invalid(
            "probe_incomplete",
            "A complete native OS volume probe is required",
        ));
    }
    let ids = probe
        .get("observed_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("probe_malformed", "Native OS probe response is malformed"))?;
    let items = probe
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("probe_malformed", "Native OS probe response is malformed"))?;
    if ids.len() != items.len() || ids.len() > 256 {
        return Err(invalid(
            "probe_malformed",
            "Native OS probe response is incomplete",
        ));
    }
    let mut matches = Vec::new();
    let mut found_count = 0usize;
    for (index, row) in items.iter().enumerate() {
        let physical_id = ids[index]
            .as_str()
            .ok_or_else(|| invalid("probe_malformed", "Native OS probe response is malformed"))?;
        if row.get("physical_id").and_then(Value::as_str) != Some(physical_id)
            || !valid_id(physical_id, platform)
        {
            return Err(invalid(
                "probe_malformed",
                "Native OS probe response is malformed",
            ));
        }
        let system = row
            .get("system")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("probe_malformed", "Native OS probe response is malformed"))?;
        let eligible = row
            .get("eligible")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("probe_malformed", "Native OS probe response is malformed"))?;
        let mount_root = row
            .get("mount_root")
            .and_then(Value::as_str)
            .filter(|root| root.len() <= 4096 && !root.chars().any(char::is_control))
            .ok_or_else(|| invalid("probe_malformed", "Native OS probe response is malformed"))?;
        if physical_id == expected_id {
            found_count += 1;
            if !system && eligible && !mount_root.is_empty() && Path::new(mount_root).is_absolute()
            {
                matches.push(Candidate {
                    physical_id: physical_id.to_owned(),
                    mount_root: mount_root.to_owned(),
                });
            }
        }
    }
    if found_count != 1 || matches.len() != 1 {
        return Err(invalid(
            "identity_conflict",
            "Saved volume UUID is missing, cloned, system, or not pairable",
        ));
    }
    Ok(matches.pop().expect("one eligible volume"))
}

fn path_is_within(root: &Path, candidate: &Path) -> bool {
    #[cfg(windows)]
    {
        if candidate.starts_with(root) {
            return true;
        }
        let root_text = root.to_string_lossy().to_lowercase();
        let candidate_text = candidate.to_string_lossy().to_lowercase();
        let prefix = if root_text.ends_with(['\\', '/']) {
            root_text
        } else {
            format!("{root_text}\\")
        };
        candidate_text.starts_with(&prefix)
    }
    #[cfg(not(windows))]
    {
        candidate.starts_with(root)
    }
}

fn ensure_targets_outside_mount(db: &Path, store: &Path, mount_root: &str) -> Result<()> {
    let root = Path::new(mount_root)
        .canonicalize()
        .map_err(|_| invalid("invalid_mount_root", "Verified mount root is unavailable"))?;
    for target in [db, store] {
        let canonical = target.canonicalize()?;
        if path_is_within(&root, target) || path_is_within(&root, &canonical) {
            return Err(invalid(
                "rescan_self_reference",
                "Catalog and private binding store must be outside the rescanned volume",
            ));
        }
    }
    Ok(())
}

fn request_scope(request: &Value) -> Result<(String, PathBuf, PathBuf, &'static str, i64, String)> {
    let (db_text, db) = absolute_path(request, "db", MAX_DB_PATH)?;
    let (_store_text, store) = absolute_path(request, "user_store", MAX_STORE_PATH)?;
    let platform = requested_platform(request)?;
    if let Some(actual) = native_platform() {
        if platform != actual {
            return Err(invalid(
                "invalid_request",
                "Platform does not match this application",
            ));
        }
    } else if !cfg!(test) {
        return Err(invalid(
            "unsupported_platform",
            "Physical rescan is unsupported on this platform",
        ));
    }
    let catalog_key = request
        .get("catalog_key")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("invalid_request", "catalog_key must be a string"))?
        .to_owned();
    if catalog_key != format!("sqlite:{db_text}") || catalog_key.len() > MAX_DB_PATH + 7 {
        return Err(invalid(
            "invalid_scope",
            "Catalog scope does not match the selected database",
        ));
    }
    if request.get("confirmed") != Some(&Value::Bool(true)) {
        return Err(invalid(
            "confirmation_required",
            "Explicit rescan confirmation is required",
        ));
    }
    let (volume_id, job_id) = request_ids(request)?;
    Ok((db_text, db, store, platform, volume_id, job_id.to_owned()))
}

/// Production operation. Reads no caller-provided root; mount_root is taken
/// from this process's native OS snapshot after matching the private pairing.
pub fn run(request: &Value) -> Result<Value> {
    run_with_probe(request, os_probe::probe)
}

fn run_with_probe<F>(request: &Value, mut probe: F) -> Result<Value>
where
    F: FnMut() -> Value,
{
    let (_db_text, db, store, platform, volume_id, job_id) = request_scope(request)?;
    preflight_native_volume(&db, volume_id)?;
    validate_private_store(&db, &store)?;
    let catalog_key = request
        .get("catalog_key")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let physical_id = read_binding(&store, catalog_key, platform, volume_id)?;
    let initial = mounted_candidate(&probe(), platform, &physical_id)?;
    ensure_targets_outside_mount(&db, &store, &initial.mount_root)?;

    let mut catalog = Catalog::open(&db)?;
    let result: Volume = catalog.rescan_volume(
        volume_id,
        Path::new(&initial.mount_root),
        Some(&job_id),
        || {
            if read_binding(&store, catalog_key, platform, volume_id)? != physical_id {
                return Err(invalid(
                    "identity_changed",
                    "Saved physical pairing changed during rescan; the previous snapshot was kept",
                ));
            }
            let current = mounted_candidate(&probe(), platform, &physical_id)?;
            // The probe can take long enough for an operator to change the
            // pairing concurrently, so re-read the private binding after it.
            if read_binding(&store, catalog_key, platform, volume_id)? != physical_id {
                return Err(invalid(
                    "identity_changed",
                    "Saved physical pairing changed during rescan; the previous snapshot was kept",
                ));
            }
            if current != initial {
                return Err(invalid(
                    "identity_changed",
                    "Physical volume changed during rescan; the previous snapshot was kept",
                ));
            }
            Ok(())
        },
    )?;
    Ok(serde_json::to_value(result).unwrap_or_else(|_| json!({})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use serde_json::json;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

    const MAC_ID: &str = "mac:aabbccdd-eeff-0011-2233-445566778899";
    const WIN_ID: &str = "win:{aabbccdd-eeff-0011-2233-445566778899}:1234abcd";

    fn platform_id() -> (&'static str, &'static str) {
        if cfg!(windows) {
            ("windows", WIN_ID)
        } else {
            ("macos", MAC_ID)
        }
    }

    fn probe(platform: &str, id: &str, root: &str) -> Value {
        json!({"platform":platform,"probe_ok":true,"observed_ids":[id],"items":[{
            "physical_id":id,"label":"Generated fixture","mount_root":root,"system":false,"eligible":true
        }]})
    }

    fn setup() -> (
        tempfile::TempDir,
        PathBuf,
        PathBuf,
        PathBuf,
        i64,
        String,
        &'static str,
        &'static str,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("synthetic-drive");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("keep.txt"), b"one").unwrap();
        fs::write(root.join("gone.txt"), b"two").unwrap();
        let db = temp.path().join("catalog.sqlite");
        let mut catalog = Catalog::open(&db).unwrap();
        let volume = catalog.scan(&root, "fixture", None).unwrap();
        let page = catalog.children(volume.id, None, 0, 10).unwrap();
        let keep = page.items.iter().find(|e| e.name == "keep.txt").unwrap();
        catalog
            .update_entry(keep.id, "retained note", "retain-tag")
            .unwrap();
        let category = catalog.create_category("Review", None).unwrap();
        catalog.assign_category(keep.id, category).unwrap();
        drop(catalog);

        let store = temp.path().join("private-store.sqlite");
        let conn = Connection::open(&store).unwrap();
        crate::physical_disks::create_schema(&conn).unwrap();
        let (platform, physical_id) = platform_id();
        let catalog_key = format!("sqlite:{}", db.to_string_lossy());
        conn.execute(
            "INSERT INTO physical_bindings(catalog_key,platform,volume_id,physical_id) VALUES(?1,?2,?3,?4)",
            params![catalog_key, platform, volume.id, physical_id],
        ).unwrap();
        drop(conn);
        (
            temp,
            db,
            store,
            root,
            volume.id,
            catalog_key,
            platform,
            physical_id,
        )
    }

    fn request(db: &Path, store: &Path, volume_id: i64, key: &str, platform: &str) -> Value {
        json!({"op":"rescan_volume","db":db,"user_store":store,"catalog_key":key,
            "platform":platform,"volume_id":volume_id,
            "job_id":format!("rescan-test-{}", NEXT_JOB.fetch_add(1, Ordering::Relaxed)),
            "confirmed":true})
    }

    fn call(request: &Value, responses: Vec<Value>) -> Result<Value> {
        let mut responses = responses.into_iter();
        run_with_probe(request, || {
            responses
                .next()
                .unwrap_or_else(|| json!({"probe_ok":false}))
        })
    }

    fn entry_id(catalog: &Catalog, volume_id: i64, name: &str) -> Option<i64> {
        catalog
            .children(volume_id, None, 0, 100)
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.name == name)
            .map(|item| item.id)
    }

    #[test]
    fn rescan_keeps_volume_entry_ids_annotations_and_categories_for_matching_paths() {
        let (_temp, db, store, root, volume_id, key, platform, physical_id) = setup();
        let catalog = Catalog::open(&db).unwrap();
        let keep_id = entry_id(&catalog, volume_id, "keep.txt").unwrap();
        fs::remove_file(root.join("gone.txt")).unwrap();
        fs::write(root.join("new.txt"), b"new data").unwrap();
        let response = call(
            &request(&db, &store, volume_id, &key, platform),
            vec![
                probe(platform, physical_id, root.to_str().unwrap()),
                probe(platform, physical_id, root.to_str().unwrap()),
            ],
        )
        .unwrap();
        assert_eq!(response["id"], volume_id);
        assert_eq!(entry_id(&catalog, volume_id, "keep.txt"), Some(keep_id));
        assert!(entry_id(&catalog, volume_id, "gone.txt").is_none());
        let new_id = entry_id(&catalog, volume_id, "new.txt").unwrap();
        assert_ne!(new_id, keep_id);
        let found = catalog
            .search(
                "description:retained AND tag:retain-tag",
                Some(volume_id),
                0,
                10,
            )
            .unwrap();
        assert_eq!(found.total, 1);
        assert_eq!(found.items[0].id, keep_id);
        let assigned = catalog
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM entry_categories WHERE entry_id=?1",
                [keep_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(assigned, 1);
        let check: String = catalog
            .conn()
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(check, "ok");
    }

    #[test]
    fn missing_annotated_path_refuses_and_rolls_back_every_change() {
        let (_temp, db, store, root, volume_id, key, platform, physical_id) = setup();
        let catalog = Catalog::open(&db).unwrap();
        let before = catalog.volumes().unwrap();
        let keep_id = entry_id(&catalog, volume_id, "keep.txt").unwrap();
        fs::remove_file(root.join("keep.txt")).unwrap();
        fs::write(root.join("new.txt"), b"new data").unwrap();
        let error = call(
            &request(&db, &store, volume_id, &key, platform),
            vec![
                probe(platform, physical_id, root.to_str().unwrap()),
                probe(platform, physical_id, root.to_str().unwrap()),
            ],
        )
        .unwrap_err();
        assert_eq!(error.code, "rescan_missing_annotations");
        assert_eq!(catalog.volumes().unwrap()[0].id, before[0].id);
        assert_eq!(entry_id(&catalog, volume_id, "keep.txt"), Some(keep_id));
        assert!(entry_id(&catalog, volume_id, "new.txt").is_none());
        let note = catalog
            .search("description:retained", Some(volume_id), 0, 10)
            .unwrap();
        assert_eq!(note.total, 1);
    }

    #[test]
    fn second_probe_must_keep_same_uuid_and_mount_or_transaction_rolls_back() {
        let (_temp, db, store, root, volume_id, key, platform, physical_id) = setup();
        let catalog = Catalog::open(&db).unwrap();
        let old = entry_id(&catalog, volume_id, "keep.txt").unwrap();
        fs::write(root.join("extra.txt"), b"new").unwrap();
        let alternate = if platform == "windows" {
            "win:{00112233-4455-6677-8899-aabbccddeeff}:98765432"
        } else {
            "mac:00112233-4455-6677-8899-aabbccddeeff"
        };
        let error = call(
            &request(&db, &store, volume_id, &key, platform),
            vec![
                probe(platform, physical_id, root.to_str().unwrap()),
                probe(platform, alternate, root.to_str().unwrap()),
            ],
        )
        .unwrap_err();
        assert_eq!(error.code, "identity_conflict");
        assert!(entry_id(&catalog, volume_id, "extra.txt").is_none());
        assert_eq!(entry_id(&catalog, volume_id, "keep.txt"), Some(old));
    }

    #[test]
    fn changed_private_binding_before_commit_rolls_back() {
        let (_temp, db, store, root, volume_id, key, platform, physical_id) = setup();
        let catalog = Catalog::open(&db).unwrap();
        let old = entry_id(&catalog, volume_id, "keep.txt").unwrap();
        fs::write(root.join("extra.txt"), b"new").unwrap();
        let other_id = if platform == "windows" {
            "win:{00112233-4455-6677-8899-aabbccddeeff}:98765432"
        } else {
            "mac:00112233-4455-6677-8899-aabbccddeeff"
        };
        let req = request(&db, &store, volume_id, &key, platform);
        let mut call_count = 0;
        let error = run_with_probe(&req, || {
            call_count += 1;
            if call_count == 2 {
                let conn = Connection::open(&store).unwrap();
                conn.execute(
                    "UPDATE physical_bindings SET physical_id=?1 WHERE catalog_key=?2 AND platform=?3 AND volume_id=?4",
                    params![other_id, key, platform, volume_id],
                ).unwrap();
            }
            probe(platform, physical_id, root.to_str().unwrap())
        }).unwrap_err();
        assert_eq!(error.code, "identity_changed");
        assert!(entry_id(&catalog, volume_id, "extra.txt").is_none());
        assert_eq!(entry_id(&catalog, volume_id, "keep.txt"), Some(old));
    }

    #[test]
    fn cancellation_at_final_identity_checkpoint_keeps_previous_snapshot() {
        let (_temp, db, store, root, volume_id, key, platform, physical_id) = setup();
        let catalog = Catalog::open(&db).unwrap();
        let old = entry_id(&catalog, volume_id, "keep.txt").unwrap();
        fs::write(root.join("extra.txt"), b"new").unwrap();
        let req = request(&db, &store, volume_id, &key, platform);
        let mut call_count = 0;
        let error = run_with_probe(&req, || {
            call_count += 1;
            if call_count == 2 {
                atlas_core::cancel_job(req["job_id"].as_str().unwrap()).unwrap();
            }
            probe(platform, physical_id, root.to_str().unwrap())
        })
        .unwrap_err();
        assert_eq!(error.code, "cancelled");
        assert!(entry_id(&catalog, volume_id, "extra.txt").is_none());
        assert_eq!(entry_id(&catalog, volume_id, "keep.txt"), Some(old));
    }

    #[test]
    fn type_change_does_not_transfer_notes_to_a_different_entry_kind() {
        let (_temp, db, store, root, volume_id, key, platform, physical_id) = setup();
        let catalog = Catalog::open(&db).unwrap();
        let keep_id = entry_id(&catalog, volume_id, "keep.txt").unwrap();
        fs::remove_file(root.join("keep.txt")).unwrap();
        fs::create_dir(root.join("keep.txt")).unwrap();
        let error = call(
            &request(&db, &store, volume_id, &key, platform),
            vec![
                probe(platform, physical_id, root.to_str().unwrap()),
                probe(platform, physical_id, root.to_str().unwrap()),
            ],
        )
        .unwrap_err();
        assert_eq!(error.code, "rescan_missing_annotations");
        assert_eq!(entry_id(&catalog, volume_id, "keep.txt"), Some(keep_id));
        assert_eq!(
            catalog
                .children(volume_id, None, 0, 10)
                .unwrap()
                .items
                .iter()
                .find(|item| item.name == "keep.txt")
                .unwrap()
                .kind,
            "file"
        );
    }

    #[test]
    fn ctf_volume_is_refused_before_native_replacement() {
        let (_temp, db, store, root, volume_id, key, platform, physical_id) = setup();
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE ctf_volumes(volume_id INTEGER PRIMARY KEY);")
            .unwrap();
        conn.execute("INSERT INTO ctf_volumes(volume_id) VALUES(?1)", [volume_id])
            .unwrap();
        drop(conn);
        let result = call(
            &request(&db, &store, volume_id, &key, platform),
            vec![probe(platform, physical_id, root.to_str().unwrap())],
        );
        assert_eq!(result.unwrap_err().code, "ctf_volume_read_only");
    }

    #[test]
    fn unbound_system_duplicate_and_incomplete_probes_fail_closed() {
        let (_temp, db, store, root, volume_id, key, platform, physical_id) = setup();
        let req = request(&db, &store, volume_id, &key, platform);
        let good = probe(platform, physical_id, root.to_str().unwrap());
        let mut system = good.clone();
        system["items"][0]["system"] = json!(true);
        assert_eq!(
            call(&req, vec![system]).unwrap_err().code,
            "identity_conflict"
        );
        let mut duplicate = good.clone();
        duplicate["observed_ids"] = json!([physical_id, physical_id]);
        duplicate["items"] = json!([duplicate["items"][0].clone(), duplicate["items"][0].clone()]);
        assert_eq!(
            call(&req, vec![duplicate]).unwrap_err().code,
            "identity_conflict"
        );
        let mut failed = good;
        failed["probe_ok"] = json!(false);
        assert_eq!(
            call(&req, vec![failed]).unwrap_err().code,
            "probe_incomplete"
        );
    }

    #[test]
    fn unrelated_unmounted_volume_with_empty_root_does_not_invalidate_target_probe() {
        let (_temp, db, store, root, volume_id, key, platform, physical_id) = setup();
        let mut snapshot = probe(platform, physical_id, root.to_str().unwrap());
        let other_id = if platform == "windows" {
            "win:{00112233-4455-6677-8899-aabbccddeeff}:98765432"
        } else {
            "mac:00112233-4455-6677-8899-aabbccddeeff"
        };
        snapshot["observed_ids"] = json!([physical_id, other_id]);
        snapshot["items"].as_array_mut().unwrap().push(json!({
            "physical_id":other_id,"label":"unmounted","mount_root":"",
            "system":false,"eligible":false
        }));
        let result = call(
            &request(&db, &store, volume_id, &key, platform),
            vec![snapshot.clone(), snapshot],
        )
        .unwrap();
        assert_eq!(result["id"], volume_id);
    }

    #[test]
    fn legacy_v1_catalog_rescans_through_entries_table() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("legacy-drive");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("keep.txt"), b"old").unwrap();
        let db = temp.path().join("legacy.sqlite");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(include_str!("../../atlas-core/migrations/001.sql"))
            .unwrap();
        drop(conn);

        let mut catalog = Catalog::open(&db).unwrap();
        let volume = catalog.scan(&root, "legacy fixture", None).unwrap();
        let entry = catalog
            .children(volume.id, None, 0, 10)
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.name == "keep.txt")
            .unwrap();
        drop(catalog);

        let store = temp.path().join("legacy-private.sqlite");
        let conn = Connection::open(&store).unwrap();
        crate::physical_disks::create_schema(&conn).unwrap();
        let (platform, physical_id) = platform_id();
        let key = format!("sqlite:{}", db.to_string_lossy());
        conn.execute(
            "INSERT INTO physical_bindings(catalog_key,platform,volume_id,physical_id) VALUES(?1,?2,?3,?4)",
            params![key, platform, volume.id, physical_id],
        )
        .unwrap();
        drop(conn);

        fs::write(root.join("keep.txt"), b"updated legacy file").unwrap();
        let result = call(
            &request(&db, &store, volume.id, &key, platform),
            vec![
                probe(platform, physical_id, root.to_str().unwrap()),
                probe(platform, physical_id, root.to_str().unwrap()),
            ],
        )
        .unwrap();
        assert_eq!(result["id"], volume.id);

        let catalog = Catalog::open(&db).unwrap();
        let updated = catalog
            .children(volume.id, None, 0, 10)
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.name == "keep.txt")
            .unwrap();
        assert_eq!(updated.id, entry.id);
        assert_eq!(updated.size, b"updated legacy file".len() as u64);
        assert_eq!(
            catalog
                .conn()
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
