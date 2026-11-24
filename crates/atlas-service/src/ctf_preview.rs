//! Read-only, in-memory CTF preview used before the disposable SQLite index is ready.
//! The source CTF remains authoritative and is never written by this module.
use crate::{ctf_sidecar, optional_id, text, unsigned};
use atlas_core::{Entry, Error, Page, Result, Volume};
use ctf_format::{CtfCatalog, EntryKind, Limits};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

const MAX_SOURCE_BYTES: u64 = 2_000_000_000;

struct Snapshot {
    path: PathBuf,
    open_path: PathBuf,
    generation: String,
    source_sha256: String,
    source_bytes: u64,
    parsed: Arc<CtfCatalog>,
    volumes: Vec<Volume>,
    id_bases: Vec<i64>,
    children: Vec<HashMap<Option<i64>, Vec<usize>>>,
}

static CURRENT: OnceLock<Mutex<Option<Arc<Snapshot>>>> = OnceLock::new();
static GENERATION_COUNTER: AtomicU64 = AtomicU64::new(1);
fn current() -> &'static Mutex<Option<Arc<Snapshot>>> {
    CURRENT.get_or_init(|| Mutex::new(None))
}

fn failure(code: &'static str, message: &'static str) -> Error {
    Error::new(code, message)
}
fn io_failure() -> Error {
    failure("io", "Unable to read the CTF source")
}
fn canonical_source(v: &Value) -> Result<PathBuf> {
    let path = Path::new(text(v, "path")?);
    let metadata = fs::symlink_metadata(path).map_err(|_| io_failure())?;
    if !metadata.file_type().is_file()
        || metadata.len() > MAX_SOURCE_BYTES
        || path
            .extension()
            .and_then(|x| x.to_str())
            .is_none_or(|x| !x.eq_ignore_ascii_case("ctf"))
    {
        return Err(failure(
            "ctf_limit",
            "Expected an unlinked regular .CTF source",
        ));
    }
    path.canonicalize().map_err(|_| io_failure())
}

fn ascii_nocase(a: &str, b: &str) -> std::cmp::Ordering {
    a.bytes()
        .map(|x| x.to_ascii_lowercase())
        .cmp(b.bytes().map(|x| x.to_ascii_lowercase()))
}

fn build_snapshot(
    path: PathBuf,
    open_path: PathBuf,
    parsed: Arc<CtfCatalog>,
    generation: String,
    source_sha256: String,
    source_bytes: u64,
) -> Result<Snapshot> {
    let mut volumes = Vec::with_capacity(parsed.disks.len());
    let mut id_bases = Vec::with_capacity(parsed.disks.len());
    let mut all_children = Vec::with_capacity(parsed.disks.len());
    let mut next_id = 1i64;
    for (volume_index, disk) in parsed.disks.iter().enumerate() {
        let volume_id = i64::try_from(volume_index + 1)
            .map_err(|_| failure("ctf_limit", "CTF has too many disks"))?;
        let id_base = next_id;
        id_bases.push(id_base);
        let mut children: HashMap<Option<i64>, Vec<usize>> = HashMap::new();
        let mut total_bytes = 0u64;
        for (index, source_entry) in disk.entries.iter().enumerate() {
            let parent_id = match source_entry.parent {
                Some(parent)
                    if parent < index
                        && matches!(
                            disk.entries[parent].kind,
                            EntryKind::Directory | EntryKind::Archive
                        ) =>
                {
                    Some(id_base + parent as i64)
                }
                Some(_) => {
                    return Err(failure(
                        "ctf_tree",
                        "CTF parent is invalid or cannot contain children",
                    ));
                }
                None => None,
            };
            let kind = match source_entry.kind {
                EntryKind::File => "file",
                EntryKind::Directory => "directory",
                EntryKind::Archive => "archive",
            };
            if kind == "file" {
                total_bytes = total_bytes
                    .checked_add(source_entry.size)
                    .ok_or_else(|| failure("ctf_limit", "CTF file size total overflow"))?;
            }
            children.entry(parent_id).or_default().push(index);
        }
        for indexes in children.values_mut() {
            indexes.sort_by(|a, b| {
                let left = &disk.entries[*a];
                let right = &disk.entries[*b];
                let left_dir = left.kind == EntryKind::Directory;
                let right_dir = right.kind == EntryKind::Directory;
                right_dir
                    .cmp(&left_dir)
                    .then_with(|| ascii_nocase(&left.name, &right.name))
                    .then_with(|| a.cmp(b))
            });
        }
        volumes.push(Volume {
            id: volume_id,
            label: disk.name.clone(),
            root_path: String::new(),
            scanned_at: 0,
            entry_count: disk.entries.len() as u64,
            total_bytes,
            status: "complete".into(),
        });
        all_children.push(children);
        next_id = next_id
            .checked_add(disk.entries.len() as i64)
            .ok_or_else(|| failure("ctf_limit", "CTF has too many entries"))?;
    }
    // Atlas lists the newest disk first; CTF source order is preserved in IDs.
    volumes.reverse();
    Ok(Snapshot {
        path,
        open_path,
        generation,
        source_sha256,
        source_bytes,
        parsed,
        volumes,
        id_bases,
        children: all_children,
    })
}

pub fn open(v: &Value) -> Result<Value> {
    let open_path = PathBuf::from(text(v, "path")?);
    let source = canonical_source(v)?;
    let captured = ctf_sidecar::capture(&source, MAX_SOURCE_BYTES).map_err(|_| io_failure())?;
    let identity = captured.identity.clone();
    let parsed = Arc::new(
        ctf_format::parse_ctf(captured.bytes(), &Limits::default()).map_err(|e| {
            Error::new(
                "ctf",
                format!("CTF parse failed at byte {}: {}", e.offset, e.message),
            )
        })?,
    );
    drop(captured);
    let serial = GENERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let generation = ctf_sidecar::sha256(format!("{}:{serial}", identity.sha256).as_bytes());
    let snapshot = Arc::new(build_snapshot(
        source.clone(),
        open_path,
        parsed,
        generation,
        identity.sha256.clone(),
        identity.size_bytes,
    )?);
    // Catch edits during parse and compact snapshot construction before publishing.
    if ctf_sidecar::identity(&source, MAX_SOURCE_BYTES).map_err(|_| io_failure())? != identity {
        return Err(failure(
            "source_changed",
            "CTF changed while opening preview",
        ));
    }
    let entries = snapshot
        .parsed
        .disks
        .iter()
        .map(|d| d.entries.len() as u64)
        .sum::<u64>();
    let volume_count = snapshot.volumes.len() as u64;
    *current()
        .lock()
        .map_err(|_| failure("internal", "CTF preview is unavailable"))? = Some(snapshot.clone());
    Ok(json!({
        "state":"browsable", "generation":snapshot.generation,
        "source_sha256":snapshot.source_sha256, "source_bytes":snapshot.source_bytes,
        "entries":entries, "volumes":volume_count,
        "original_ctf_read_only":true, "native_annotation_writes_allowed":false,
        "derived_database_disposable":true
    }))
}

fn selected(v: &Value) -> Result<Arc<Snapshot>> {
    let generation = text(v, "generation")?;
    let requested_path = canonical_source(v)?;
    let found = current()
        .lock()
        .map_err(|_| failure("internal", "CTF preview is unavailable"))?
        .as_ref()
        .filter(|s| s.generation == generation && s.path == requested_path)
        .cloned()
        .ok_or_else(|| failure("stale_generation", "Open this CTF again before browsing"))?;
    let current_identity =
        ctf_sidecar::identity(&found.path, MAX_SOURCE_BYTES).map_err(|_| io_failure())?;
    if current_identity.sha256 != found.source_sha256
        || current_identity.size_bytes != found.source_bytes
    {
        return Err(failure(
            "source_changed",
            "CTF source changed; reopen it to browse",
        ));
    }
    Ok(found)
}

fn checked_result(snapshot: &Snapshot, result: Value) -> Result<Value> {
    let after =
        ctf_sidecar::identity(&snapshot.path, MAX_SOURCE_BYTES).map_err(|_| io_failure())?;
    if after.sha256 != snapshot.source_sha256 || after.size_bytes != snapshot.source_bytes {
        return Err(failure(
            "source_changed",
            "CTF source changed during preview read",
        ));
    }
    Ok(
        json!({"generation":snapshot.generation,"source_sha256":snapshot.source_sha256,
        "read_only":true,"original_ctf_read_only":true,"annotations_supported":false,"result":result}),
    )
}

pub fn volumes(v: &Value) -> Result<Value> {
    let snapshot = selected(v)?;
    let result = serde_json::to_value(&snapshot.volumes)
        .map_err(|_| failure("internal", "Cannot encode CTF volume list"))?;
    checked_result(&snapshot, result)
}

pub fn children(v: &Value) -> Result<Value> {
    let volume_id = optional_id(v, "volume_id")?
        .ok_or_else(|| failure("invalid_request", "A positive volume_id is required"))?;
    let parent_id = optional_id(v, "parent_id")?;
    let offset = unsigned(v, "offset", 0)?;
    let requested_limit = unsigned(v, "limit", 250)?;
    let limit = requested_limit.clamp(1, 1000);
    let snapshot = selected(v)?;
    let volume_index =
        usize::try_from(volume_id - 1).map_err(|_| failure("not_found", "CTF volume not found"))?;
    let children_map = snapshot
        .children
        .get(volume_index)
        .ok_or_else(|| failure("not_found", "CTF volume not found"))?;
    let indexes = children_map
        .get(&parent_id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let start = usize::try_from(offset.min(usize::MAX as u64))
        .unwrap_or(usize::MAX)
        .min(indexes.len());
    let end = start.saturating_add(limit as usize).min(indexes.len());
    let disk = &snapshot.parsed.disks[volume_index];
    let id_base = snapshot.id_bases[volume_index];
    let items = indexes[start..end]
        .iter()
        .map(|index| {
            let source = &disk.entries[*index];
            let parent_id = source.parent.map(|parent| id_base + parent as i64);
            let mut components = Vec::new();
            let mut cursor = Some(*index);
            while let Some(current) = cursor {
                let item = &disk.entries[current];
                components.push(item.name.as_str());
                cursor = item.parent;
            }
            components.reverse();
            let relative_path = components.join("/");
            let kind = match source.kind {
                EntryKind::File => "file",
                EntryKind::Directory => "directory",
                EntryKind::Archive => "archive",
            };
            Entry {
                id: id_base + *index as i64,
                volume_id,
                parent_id,
                name: source.name.clone(),
                relative_path,
                kind: kind.into(),
                size: source.size,
                modified_unix: None,
                description: source.description.clone(),
                tags: String::new(),
            }
        })
        .collect();
    let page = Page {
        items,
        total: indexes.len() as u64,
        offset,
        limit,
    };
    let result = serde_json::to_value(page)
        .map_err(|_| failure("internal", "Cannot encode CTF directory page"))?;
    checked_result(&snapshot, result)
}

pub fn close(v: &Value) -> Result<Value> {
    let generation = text(v, "generation")?;
    let path = Path::new(text(v, "path")?);
    let requested_path = path.canonicalize().unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    });
    let mut guard = current()
        .lock()
        .map_err(|_| failure("internal", "CTF preview is unavailable"))?;
    if guard.as_ref().is_some_and(|s| {
        s.generation == generation && (s.path == requested_path || s.open_path == path)
    }) {
        *guard = None;
        Ok(json!({"closed":true,"generation":generation}))
    } else {
        Ok(json!({"closed":false,"generation":generation}))
    }
}

/// Reuse the already validated in-memory parse when normal import opens the same bytes.
pub(crate) fn parsed_for_import(path: &Path, sha256: &str) -> Option<Arc<CtfCatalog>> {
    let canonical = path.canonicalize().ok()?;
    current()
        .lock()
        .ok()?
        .as_ref()
        .filter(|s| s.path == canonical && s.source_sha256.eq_ignore_ascii_case(sha256))
        .map(|s| s.parsed.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry};
    use std::sync::Mutex;
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn entry(name: &str, parent: Option<usize>, kind: EntryKind) -> CtfEntry {
        CtfEntry {
            name: name.into(),
            description: String::new(),
            parent,
            kind,
            size: 17,
            attributes: 0,
            modified_dos: 0,
            created_dos: 0,
            accessed_dos: 0,
            opaque: vec![],
        }
    }
    fn write_fixture(path: &Path, disk_name: &str) {
        let bytes = write_ctf(&CtfCatalog {
            name: "preview test".into(),
            opaque_trailer: vec![37, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, b'G', 0],
            disks: vec![
                CtfDisk {
                    name: disk_name.into(),
                    opaque_header: vec![],
                    entries: vec![
                        entry("root", None, EntryKind::Directory),
                        entry("z.txt", Some(0), EntryKind::File),
                        entry("東京", Some(0), EntryKind::Directory),
                        entry("é.txt", Some(2), EntryKind::File),
                    ],
                },
                CtfDisk {
                    name: "second".into(),
                    opaque_header: vec![],
                    entries: vec![entry("β", None, EntryKind::File)],
                },
            ],
        })
        .unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn req(path: &Path, generation: &str) -> Value {
        json!({"path":path,"generation":generation})
    }

    #[test]
    fn browses_source_order_ids_unicode_and_readonly() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.CTF");
        write_fixture(&path, "disc 東京");
        let before = fs::read(&path).unwrap();
        let opened = open(&json!({"path":path})).unwrap();
        assert_eq!(opened["state"], "browsable");
        assert_eq!(opened["entries"], 5);
        assert_eq!(opened["volumes"], 2);
        assert_eq!(opened["original_ctf_read_only"], true);
        let generation = opened["generation"].as_str().unwrap();
        let vols = volumes(&req(&path, generation)).unwrap();
        assert_eq!(vols["result"][0]["label"], "second");
        assert_eq!(vols["result"][1]["label"], "disc 東京");
        let page = children(
            &json!({"path":path,"generation":generation,"volume_id":1,"parent_id":1,"limit":10}),
        )
        .unwrap();
        assert_eq!(page["result"]["total"], 2);
        assert_eq!(page["result"]["items"][0]["name"], "東京");
        assert_eq!(page["result"]["items"][0]["id"], 2);
        let second =
            children(&json!({"path":path,"generation":generation,"volume_id":2,"limit":10}))
                .unwrap();
        assert_eq!(second["result"]["items"][0]["id"], 5);
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn source_change_fails_closed_and_stale_close_cannot_clear_new_snapshot() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.ctf");
        let b = dir.path().join("b.ctf");
        write_fixture(&a, "a");
        write_fixture(&b, "b");
        let one = open(&json!({"path":a})).unwrap();
        let old = one["generation"].as_str().unwrap().to_owned();
        let two = open(&json!({"path":b})).unwrap();
        let new = two["generation"].as_str().unwrap();
        assert_eq!(close(&req(&a, &old)).unwrap()["closed"], false);
        let three = open(&json!({"path":b})).unwrap();
        let newest = three["generation"].as_str().unwrap();
        assert_ne!(new, newest);
        assert_eq!(close(&req(&b, new)).unwrap()["closed"], false);
        assert!(volumes(&req(&b, newest)).is_ok());
        let mut changed = fs::read(&b).unwrap();
        changed.push(7);
        fs::write(&b, changed).unwrap();
        assert_eq!(
            volumes(&req(&b, newest)).unwrap_err().code,
            "source_changed"
        );
    }

    #[test]
    fn invalid_open_does_not_publish_partial_snapshot() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.ctf");
        let bad = dir.path().join("bad.ctf");
        write_fixture(&good, "good");
        fs::write(&bad, b"not ctf").unwrap();
        let opened = open(&json!({"path":good})).unwrap();
        assert!(open(&json!({"path":bad})).is_err());
        assert!(volumes(&req(&good, opened["generation"].as_str().unwrap())).is_ok());
    }

    #[test]
    fn preview_pages_match_imported_catalog_order_ids_and_paths() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("parity.ctf");
        write_fixture(&path, "parity");
        let opened = open(&json!({"path":path})).unwrap();
        let generation = opened["generation"].as_str().unwrap();
        let preview = children(
            &json!({"path":path,"generation":generation,"volume_id":1,"parent_id":1,"limit":10}),
        )
        .unwrap();

        let mut catalog = atlas_core::Catalog::open(dir.path().join("import.sqlite")).unwrap();
        crate::legacy::import(&mut catalog, &json!({"path":path})).unwrap();
        let imported = serde_json::to_value(catalog.children(1, Some(1), 0, 10).unwrap()).unwrap();
        assert_eq!(preview["result"], imported);
        let nested = children(
            &json!({"path":path,"generation":generation,"volume_id":1,"parent_id":3,"limit":10}),
        )
        .unwrap();
        let imported_nested =
            serde_json::to_value(catalog.children(1, Some(3), 0, 10).unwrap()).unwrap();
        assert_eq!(nested["result"], imported_nested);
    }
}
