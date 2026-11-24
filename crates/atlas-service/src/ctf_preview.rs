//! Read-only, in-memory CTF preview used before the disposable SQLite index is ready.
//! The source CTF remains authoritative and is never written by this module.
use crate::{ctf_sidecar, optional_id, text, unsigned};
use atlas_core::query::{
    MetadataCategoryPredicate, MetadataQuery, MetadataQueryContext, MetadataRecord,
};
use atlas_core::{Entry, Error, FastPage, Page, Result, Volume};
use ctf_format::{CtfCatalog, EntryKind, Limits};
use serde_json::{json, Value};
use std::{
    cell::OnceCell as LocalOnceCell,
    cmp::Ordering as CmpOrdering,
    collections::{BinaryHeap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

const MAX_SOURCE_BYTES: u64 = 2_000_000_000;
const MAX_ANCESTOR_DEPTH: usize = 4096;

pub(super) struct Snapshot {
    path: PathBuf,
    open_path: PathBuf,
    generation: String,
    source_sha256: String,
    source_bytes: u64,
    pub(super) parsed: Arc<CtfCatalog>,
    volumes: Vec<Volume>,
    pub(super) id_bases: Vec<i64>,
    pub(super) children: Vec<HashMap<Option<i64>, Vec<usize>>>,
    name_order: OnceLock<Vec<RowRef>>,
    #[cfg(feature = "duckdb-query")]
    pub(super) duckdb: Mutex<Option<crate::ctf_duckdb::Engine>>,
}

#[derive(Clone, Copy)]
struct RowRef {
    volume_index: u32,
    entry_index: u32,
}

#[derive(Clone, Copy)]
struct SearchHit<'a> {
    name: &'a str,
    id: i64,
    row: RowRef,
}

struct CandidateHit {
    volume_id: i64,
    path: String,
    id: i64,
    row: RowRef,
}

impl PartialEq for CandidateHit {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for CandidateHit {}
impl PartialOrd for CandidateHit {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}
impl Ord for CandidateHit {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        self.volume_id
            .cmp(&other.volume_id)
            .then_with(|| self.path.cmp(&other.path))
            .then_with(|| self.id.cmp(&other.id))
    }
}

impl PartialEq for SearchHit<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for SearchHit<'_> {}
impl PartialOrd for SearchHit<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}
impl Ord for SearchHit<'_> {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        ascii_nocase(self.name, other.name).then_with(|| self.id.cmp(&other.id))
    }
}

static CURRENT: OnceLock<Mutex<Option<Arc<Snapshot>>>> = OnceLock::new();
static GENERATION_COUNTER: AtomicU64 = AtomicU64::new(1);
fn current() -> &'static Mutex<Option<Arc<Snapshot>>> {
    CURRENT.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(super) static TEST_LOCK: Mutex<()> = Mutex::new(());

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
        name_order: OnceLock::new(),
        #[cfg(feature = "duckdb-query")]
        duckdb: Mutex::new(None),
    })
}

fn compact_entry_capacity(parsed: &mut CtfCatalog) {
    for disk in &mut parsed.disks {
        disk.entries.shrink_to_fit();
    }
}

pub fn open(v: &Value) -> Result<Value> {
    let open_path = PathBuf::from(text(v, "path")?);
    let source = canonical_source(v)?;
    let captured = ctf_sidecar::capture(&source, MAX_SOURCE_BYTES).map_err(|_| io_failure())?;
    let identity = captured.identity.clone();
    let mut parsed = ctf_format::parse_ctf(captured.bytes(), &Limits::default()).map_err(|e| {
        Error::new(
            "ctf",
            format!("CTF parse failed at byte {}: {}", e.offset, e.message),
        )
    })?;
    drop(captured);
    compact_entry_capacity(&mut parsed);
    let parsed = Arc::new(parsed);
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

pub(super) fn selected(v: &Value, job: Option<&crate::ctf_job::Job>) -> Result<Arc<Snapshot>> {
    let generation = text(v, "generation")?;
    let requested_path = canonical_source(v)?;
    let found = current()
        .lock()
        .map_err(|_| failure("internal", "CTF preview is unavailable"))?
        .as_ref()
        .filter(|s| s.generation == generation && s.path == requested_path)
        .cloned()
        .ok_or_else(|| failure("stale_generation", "Open this CTF again before browsing"))?;
    let current_identity = if let Some(job) = job {
        job.identity(&found.path, MAX_SOURCE_BYTES)?
    } else {
        ctf_sidecar::identity(&found.path, MAX_SOURCE_BYTES).map_err(|_| io_failure())?
    };
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

pub(super) fn checked_result(
    snapshot: &Snapshot,
    result: Value,
    job: Option<&crate::ctf_job::Job>,
) -> Result<Value> {
    let after = if let Some(job) = job {
        job.identity(&snapshot.path, MAX_SOURCE_BYTES)?
    } else {
        ctf_sidecar::identity(&snapshot.path, MAX_SOURCE_BYTES).map_err(|_| io_failure())?
    };
    if let Some(job) = job {
        job.check()?;
    }
    if after.sha256 != snapshot.source_sha256 || after.size_bytes != snapshot.source_bytes {
        return Err(failure(
            "source_changed",
            "CTF source changed during preview read",
        ));
    }
    Ok(
        json!({"generation":snapshot.generation,"source_sha256":snapshot.source_sha256,
        "read_only":true,"original_ctf_read_only":true,"annotations_supported":false,
        "source_sha_checked":true,"verification_mode":"direct_ctf_preview","result":result}),
    )
}

pub fn volumes(v: &Value) -> Result<Value> {
    let snapshot = selected(v, None)?;
    let result = serde_json::to_value(&snapshot.volumes)
        .map_err(|_| failure("internal", "Cannot encode CTF volume list"))?;
    checked_result(&snapshot, result, None)
}

pub fn children(v: &Value) -> Result<Value> {
    let volume_id = optional_id(v, "volume_id")?
        .ok_or_else(|| failure("invalid_request", "A positive volume_id is required"))?;
    let parent_id = optional_id(v, "parent_id")?;
    let offset = unsigned(v, "offset", 0)?;
    let requested_limit = unsigned(v, "limit", 250)?;
    let limit = requested_limit.clamp(1, 1000);
    let snapshot = selected(v, None)?;
    let volume_index =
        usize::try_from(volume_id - 1).map_err(|_| failure("not_found", "CTF volume not found"))?;
    if snapshot.children.get(volume_index).is_none() {
        return Err(failure("not_found", "CTF volume not found"));
    }
    let page = children_page(&snapshot, volume_index, parent_id, offset, limit)?;
    let result = serde_json::to_value(page)
        .map_err(|_| failure("internal", "Cannot encode CTF directory page"))?;
    checked_result(&snapshot, result, None)
}

pub fn children_focused(v: &Value) -> Result<Value> {
    let volume_id = optional_id(v, "volume_id")?
        .ok_or_else(|| failure("invalid_request", "Select a volume"))?;
    let entry_id =
        optional_id(v, "entry_id")?.ok_or_else(|| failure("invalid_request", "Select an entry"))?;
    let parent_id = optional_id(v, "parent_id")?;
    let limit = unsigned(v, "limit", 250)?.clamp(1, 1000);
    let snapshot = selected(v, None)?;
    let volume_index =
        usize::try_from(volume_id - 1).map_err(|_| failure("not_found", "CTF volume not found"))?;
    let disk = snapshot
        .parsed
        .disks
        .get(volume_index)
        .ok_or_else(|| failure("not_found", "CTF volume not found"))?;
    let entry_index = usize::try_from(entry_id - snapshot.id_bases[volume_index])
        .map_err(|_| failure("not_found", "Entry is no longer in this folder"))?;
    let entry = disk
        .entries
        .get(entry_index)
        .ok_or_else(|| failure("not_found", "Entry is no longer in this folder"))?;
    let actual_parent = entry
        .parent
        .map(|parent| snapshot.id_bases[volume_index] + parent as i64);
    if actual_parent != parent_id {
        return Err(failure("not_found", "Entry is no longer in this folder"));
    }
    let siblings = snapshot.children[volume_index]
        .get(&parent_id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let position = siblings
        .iter()
        .position(|index| *index == entry_index)
        .ok_or_else(|| failure("not_found", "Entry is no longer in this folder"))?;
    let offset = (position as u64 / limit) * limit;
    let page = children_page(&snapshot, volume_index, parent_id, offset, limit)?;
    let result = serde_json::to_value(page)
        .map_err(|_| failure("internal", "Cannot encode focused CTF directory page"))?;
    checked_result(&snapshot, result, None)
}

pub fn ancestors(v: &Value) -> Result<Value> {
    let volume_id = optional_id(v, "volume_id")?
        .ok_or_else(|| failure("invalid_request", "Select a volume"))?;
    let entry_id =
        optional_id(v, "entry_id")?.ok_or_else(|| failure("invalid_request", "Select an entry"))?;
    let snapshot = selected(v, None)?;
    let volume_index =
        usize::try_from(volume_id - 1).map_err(|_| failure("not_found", "CTF volume not found"))?;
    let disk = snapshot
        .parsed
        .disks
        .get(volume_index)
        .ok_or_else(|| failure("not_found", "CTF volume not found"))?;
    let result = ancestor_items(
        volume_id,
        snapshot.id_bases[volume_index],
        &disk.entries,
        entry_id,
    )?;
    checked_result(&snapshot, result, None)
}

fn ancestor_items(
    volume_id: i64,
    id_base: i64,
    entries: &[ctf_format::CtfEntry],
    entry_id: i64,
) -> Result<Value> {
    let first = usize::try_from(
        entry_id
            .checked_sub(id_base)
            .ok_or_else(|| failure("not_found", "Entry not found in requested volume"))?,
    )
    .map_err(|_| failure("not_found", "Entry not found in requested volume"))?;
    if first >= entries.len() {
        return Err(failure("not_found", "Entry not found in requested volume"));
    }
    let mut current = Some(first);
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    while let Some(index) = current {
        if items.len() >= MAX_ANCESTOR_DEPTH || !seen.insert(index) {
            return Err(failure("ctf_tree", "Entry ancestry is cyclic or too deep"));
        }
        let entry = entries
            .get(index)
            .ok_or_else(|| failure("ctf_tree", "Entry ancestry references a missing parent"))?;
        let id = id_base
            .checked_add(index as i64)
            .ok_or_else(|| failure("ctf_limit", "CTF entry ID overflow"))?;
        let parent_id = entry
            .parent
            .map(|parent| {
                if parent >= index
                    || !matches!(
                        entries.get(parent).map(|parent| parent.kind),
                        Some(EntryKind::Directory | EntryKind::Archive)
                    )
                {
                    return Err(failure("ctf_tree", "Entry ancestry parent is invalid"));
                }
                id_base
                    .checked_add(parent as i64)
                    .ok_or_else(|| failure("ctf_limit", "CTF parent ID overflow"))
            })
            .transpose()?;
        let kind = match entry.kind {
            EntryKind::File => "file",
            EntryKind::Directory => "directory",
            EntryKind::Archive => "archive",
        };
        items.push(json!({
            "id":id,
            "parent_id":parent_id,
            "name":entry.name,
            "kind":kind
        }));
        current = entry.parent;
    }
    items.reverse();
    Ok(json!({"volume_id":volume_id,"items":items}))
}

struct DirectContext<'a> {
    disk: &'a ctf_format::CtfDisk,
    entry_index: usize,
    id_base: i64,
    relative_path: LocalOnceCell<String>,
}

impl MetadataQueryContext for DirectContext<'_> {
    fn relative_path(&self) -> &str {
        self.relative_path
            .get_or_init(|| relative_path(self.disk, self.entry_index))
    }

    fn is_branch_descendant(&self, anchor_id: i64) -> bool {
        let mut parent = self.disk.entries[self.entry_index].parent;
        while let Some(index) = parent {
            if self.id_base + index as i64 == anchor_id {
                return true;
            }
            parent = self.disk.entries[index].parent;
        }
        false
    }

    fn category_matches(&self, predicate: &MetadataCategoryPredicate) -> bool {
        let entry = &self.disk.entries[self.entry_index];
        matches!(predicate, MetadataCategoryPredicate::PrimaryId(0))
            && entry.kind == EntryKind::File
    }
}

pub(super) fn relative_path(disk: &ctf_format::CtfDisk, index: usize) -> String {
    let mut components = Vec::new();
    let mut cursor = Some(index);
    while let Some(current) = cursor {
        let entry = &disk.entries[current];
        components.push(entry.name.as_str());
        cursor = entry.parent;
    }
    components.reverse();
    components.join("/")
}

fn matches_row(
    snapshot: &Snapshot,
    row: RowRef,
    query: &MetadataQuery,
    needs_extension: bool,
    volume_id: Option<i64>,
    job: &crate::ctf_job::Job,
) -> Result<bool> {
    job.entry()?;
    let volume_index = row.volume_index as usize;
    let entry_index = row.entry_index as usize;
    let actual_volume_id = volume_index as i64 + 1;
    if volume_id.is_some_and(|filter| filter != actual_volume_id) {
        return Ok(false);
    }
    Ok(matches_metadata(
        snapshot,
        volume_index,
        entry_index,
        query,
        needs_extension,
    ))
}

pub(super) fn matches_metadata(
    snapshot: &Snapshot,
    volume_index: usize,
    entry_index: usize,
    query: &MetadataQuery,
    needs_extension: bool,
) -> bool {
    let disk = &snapshot.parsed.disks[volume_index];
    let entry = &disk.entries[entry_index];
    let kind = match entry.kind {
        EntryKind::File => "file",
        EntryKind::Directory => "directory",
        EntryKind::Archive => "archive",
    };
    let extension = if needs_extension {
        Path::new(&entry.name)
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
    } else {
        ""
    };
    let id = snapshot.id_bases[volume_index] + entry_index as i64;
    let context = DirectContext {
        disk,
        entry_index,
        id_base: snapshot.id_bases[volume_index],
        relative_path: LocalOnceCell::new(),
    };
    let record = MetadataRecord {
        id,
        name: &entry.name,
        kind,
        size: entry.size,
        extension,
        description: &entry.description,
        tags: "",
    };
    query.matches(&record, &context)
}

fn name_order(snapshot: &Snapshot) -> &[RowRef] {
    snapshot.name_order.get_or_init(|| {
        let total = snapshot.parsed.disks.iter().map(|d| d.entries.len()).sum();
        let mut rows = Vec::with_capacity(total);
        for (volume_index, disk) in snapshot.parsed.disks.iter().enumerate() {
            for entry_index in 0..disk.entries.len() {
                rows.push(RowRef {
                    volume_index: volume_index as u32,
                    entry_index: entry_index as u32,
                });
            }
        }
        rows.sort_unstable_by(|a, b| {
            let left =
                &snapshot.parsed.disks[a.volume_index as usize].entries[a.entry_index as usize];
            let right =
                &snapshot.parsed.disks[b.volume_index as usize].entries[b.entry_index as usize];
            ascii_nocase(&left.name, &right.name).then_with(|| {
                (snapshot.id_bases[a.volume_index as usize] + a.entry_index as i64)
                    .cmp(&(snapshot.id_bases[b.volume_index as usize] + b.entry_index as i64))
            })
        });
        rows
    })
}

pub(super) fn entry_at(snapshot: &Snapshot, volume_index: usize, entry_index: usize) -> Entry {
    let disk = &snapshot.parsed.disks[volume_index];
    let source = &disk.entries[entry_index];
    let id_base = snapshot.id_bases[volume_index];
    let parent_id = source.parent.map(|parent| id_base + parent as i64);
    let kind = match source.kind {
        EntryKind::File => "file",
        EntryKind::Directory => "directory",
        EntryKind::Archive => "archive",
    };
    Entry {
        id: id_base + entry_index as i64,
        volume_id: volume_index as i64 + 1,
        parent_id,
        name: source.name.clone(),
        relative_path: relative_path(disk, entry_index),
        kind: kind.into(),
        size: source.size,
        modified_unix: None,
        description: source.description.clone(),
        tags: String::new(),
    }
}

fn find_id(snapshot: &Snapshot, id: i64) -> Option<(usize, usize, &ctf_format::CtfEntry)> {
    snapshot
        .id_bases
        .iter()
        .enumerate()
        .find_map(|(volume_index, base)| {
            let entry_index = usize::try_from(id.checked_sub(*base)?).ok()?;
            let entry = snapshot
                .parsed
                .disks
                .get(volume_index)?
                .entries
                .get(entry_index)?;
            Some((volume_index, entry_index, entry))
        })
}

fn copy_path_order(snapshot: &Snapshot, a: RowRef, b: RowRef) -> CmpOrdering {
    let av = a.volume_index as usize;
    let ai = a.entry_index as usize;
    let bv = b.volume_index as usize;
    let bi = b.entry_index as usize;
    (av as i64 + 1)
        .cmp(&(bv as i64 + 1))
        .then_with(|| {
            relative_path(&snapshot.parsed.disks[av], ai)
                .cmp(&relative_path(&snapshot.parsed.disks[bv], bi))
        })
        .then_with(|| (snapshot.id_bases[av] + ai as i64).cmp(&(snapshot.id_bases[bv] + bi as i64)))
}

fn children_page(
    snapshot: &Snapshot,
    volume_index: usize,
    parent_id: Option<i64>,
    offset: u64,
    limit: u64,
) -> Result<Page> {
    let indexes = snapshot.children[volume_index]
        .get(&parent_id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let start = usize::try_from(offset.min(usize::MAX as u64))
        .unwrap_or(usize::MAX)
        .min(indexes.len());
    let end = start.saturating_add(limit as usize).min(indexes.len());
    let items = indexes[start..end]
        .iter()
        .map(|index| entry_at(snapshot, volume_index, *index))
        .collect();
    Ok(Page {
        items,
        total: indexes.len() as u64,
        offset,
        limit,
    })
}

pub fn search(v: &Value) -> Result<Value> {
    search_with_job(v, crate::ctf_job::Job::start(v)?)
}

pub fn copy_candidates(v: &Value) -> Result<Value> {
    let job = crate::ctf_job::Job::start(v)?;
    job.phase("preview_copy_candidates")?;
    let snapshot = selected(v, Some(&job))?;
    let source_id = optional_id(v, "entry_id")?
        .ok_or_else(|| failure("invalid_request", "Select a positive file entry_id"))?;
    let offset = unsigned(v, "offset", 0)?.min(i64::MAX as u64);
    let limit = unsigned(v, "limit", 200)?.clamp(1, 1000);
    let candidate_volume_id = optional_id(v, "candidate_volume_id")?;
    let other_disks_only = match v.get("other_disks_only") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => {
            return Err(failure(
                "invalid_request",
                "other_disks_only must be boolean",
            ));
        }
    };
    let source = find_id(&snapshot, source_id)
        .filter(|(_, _, entry)| entry.kind == EntryKind::File)
        .ok_or_else(|| failure("not_found", "Selected file is unavailable in this CTF"))?;
    let source_volume_id = source.0 as i64 + 1;
    let name_key = source.2.name.to_lowercase();
    let size = source.2.size;
    let mut groups = Vec::new();
    const MAX_HEAP_OFFSET: u64 = 50_000;
    let heap_page = offset <= MAX_HEAP_OFFSET;
    let retain = offset.saturating_add(limit) as usize;
    let mut best = BinaryHeap::with_capacity(if heap_page { retain } else { 0 });
    let mut ordered_rows = if heap_page { None } else { Some(Vec::new()) };
    for (volume_index, disk) in snapshot.parsed.disks.iter().enumerate() {
        let volume_id = volume_index as i64 + 1;
        let mut group_count = 0i64;
        for (entry_index, entry) in disk.entries.iter().enumerate() {
            job.entry()?;
            if entry.kind == EntryKind::File
                && entry.size == size
                && entry.name.to_lowercase() == name_key
            {
                group_count += 1;
                if (!other_disks_only || volume_id != source_volume_id)
                    && candidate_volume_id.is_none_or(|filter| filter == volume_id)
                {
                    let row = RowRef {
                        volume_index: volume_index as u32,
                        entry_index: entry_index as u32,
                    };
                    if let Some(rows) = &mut ordered_rows {
                        rows.push(row);
                    } else {
                        let hit = CandidateHit {
                            volume_id,
                            path: relative_path(disk, entry_index),
                            id: snapshot.id_bases[volume_index] + entry_index as i64,
                            row,
                        };
                        if best.len() < retain {
                            best.push(hit);
                        } else if best.peek().is_some_and(|largest| hit < *largest) {
                            best.pop();
                            best.push(hit);
                        }
                    }
                }
            }
        }
        if group_count > 0 {
            groups.push(
                json!({"volume_id":volume_id,"label":disk.name.as_str(),"count":group_count}),
            );
        }
    }
    let all_total: i64 = groups.iter().filter_map(|g| g["count"].as_i64()).sum();
    let other_total: i64 = groups
        .iter()
        .filter(|g| g["volume_id"].as_i64() != Some(source_volume_id))
        .filter_map(|g| g["count"].as_i64())
        .sum();
    if other_disks_only {
        groups.retain(|g| g["volume_id"].as_i64() != Some(source_volume_id));
    }
    let distinct_disks = groups.len();
    let scoped_total = if other_disks_only {
        other_total
    } else {
        all_total
    };
    let total = candidate_volume_id.map_or(scoped_total, |focused| {
        groups
            .iter()
            .find(|g| g["volume_id"] == focused)
            .and_then(|g| g["count"].as_i64())
            .unwrap_or(0)
    });
    let page_rows = if let Some(mut rows) = ordered_rows {
        rows.sort_unstable_by(|a, b| copy_path_order(&snapshot, *a, *b));
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(rows.len());
        let end = start.saturating_add(limit as usize).min(rows.len());
        rows[start..end].to_vec()
    } else {
        let mut hits = best.into_vec();
        hits.sort_unstable();
        hits.into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|hit| hit.row)
            .collect()
    };
    let items: Vec<Value> = page_rows
        .iter()
        .map(|row| {
            let mut item = serde_json::to_value(entry_at(
                &snapshot,
                row.volume_index as usize,
                row.entry_index as usize,
            ))
            .expect("Entry serializes");
            item["duplicate_count"] = json!(all_total);
            item
        })
        .collect();
    let result = json!({
        "items":items,"total":total,"all_total":all_total,"other_total":other_total,
        "other_disks_only":other_disks_only,"offset":offset,"limit":limit,
        "has_more":offset.saturating_add(limit)<total as u64,
        "source_entry_id":source_id,"source_volume_id":source_volume_id,
        "candidate_volume_id":candidate_volume_id,"distinct_disks":distinct_disks,
        "disk_groups":groups,"basis":"exact_normalized_name_and_size",
        "verified":false,"read_only":true
    });
    checked_result(&snapshot, result, Some(&job))
}

pub fn export(v: &Value) -> Result<Value> {
    let job = crate::ctf_job::Job::start(v)?;
    export_with_job(v, job)
}

fn export_with_job(v: &Value, job: crate::ctf_job::Job) -> Result<Value> {
    let format_name = text(v, "format")?;
    let (format, extension) = match format_name {
        "csv" => ("export_csv", "csv"),
        "json" => ("export_json", "json"),
        "parquet" => ("export_parquet", "parquet"),
        _ => {
            return Err(failure(
                "invalid_request",
                "CTF preview export supports csv, json or parquet",
            ));
        }
    };
    let output = PathBuf::from(text(v, "output")?);
    if output
        .extension()
        .and_then(|x| x.to_str())
        .is_none_or(|x| !x.eq_ignore_ascii_case(extension))
    {
        return Err(failure(
            "invalid_request",
            "Output filename must match the requested format",
        ));
    }
    let mode = match v.get("mode") {
        None => "search",
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(failure("invalid_request", "mode must be a string")),
    };
    if !matches!(mode, "search" | "children" | "all" | "copy_candidates") {
        return Err(failure(
            "invalid_request",
            "Unsupported direct CTF export scope",
        ));
    }
    let query_text = match v.get("query") {
        None => "",
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(failure("invalid_request", "query must be a string")),
    };
    if mode == "copy_candidates" && !query_text.is_empty() {
        return Err(failure(
            "invalid_request",
            "copy_candidates does not accept query text",
        ));
    }
    let query = MetadataQuery::compile_metadata(query_text)?;
    let volume_id = optional_id(v, "volume_id")?;
    let parent_id = optional_id(v, "parent_id")?;
    if mode == "children" && volume_id.is_none() {
        return Err(failure(
            "invalid_request",
            "children export requires volume_id",
        ));
    }
    if mode == "copy_candidates" && (volume_id.is_some() || parent_id.is_some()) {
        return Err(failure(
            "invalid_request",
            "copy_candidates uses candidate_volume_id scope",
        ));
    }
    let candidate_volume_id = optional_id(v, "candidate_volume_id")?;
    let other_disks_only = match v.get("other_disks_only") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => {
            return Err(failure(
                "invalid_request",
                "other_disks_only must be boolean",
            ));
        }
    };
    job.phase("preview_export")?;
    let snapshot = selected(v, Some(&job))?;
    let selected_key = if mode == "copy_candidates" {
        let id = optional_id(v, "entry_id")?
            .ok_or_else(|| failure("invalid_request", "Select a positive file entry_id"))?;
        let (vi, ei, entry) = find_id(&snapshot, id)
            .filter(|(_, _, entry)| entry.kind == EntryKind::File)
            .ok_or_else(|| failure("not_found", "Selected file is unavailable in this CTF"))?;
        Some((id, vi, ei, entry.name.to_lowercase(), entry.size))
    } else {
        None
    };
    let parent_scope = if let Some(id) = parent_id {
        let (vi, ei, entry) = find_id(&snapshot, id)
            .ok_or_else(|| failure("not_found", "Export parent is unavailable"))?;
        Some((vi, ei, entry.kind))
    } else {
        None
    };
    if let Some((parent_volume, _, _)) = parent_scope {
        if volume_id != Some(parent_volume as i64 + 1) {
            return Err(failure(
                "invalid_request",
                "Export parent must belong to volume_id",
            ));
        }
    }

    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = parent
        .canonicalize()
        .map_err(|_| failure("invalid_path", "Output directory is unavailable"))?;
    let name = output
        .file_name()
        .ok_or_else(|| failure("invalid_path", "Output needs a filename"))?;
    let target = parent.join(name);
    if target == snapshot.path || target.exists() || target.is_symlink() {
        return Err(failure(
            "file_exists",
            "Output must be a new file; existing files are never replaced",
        ));
    }
    let mut stage = tempfile::Builder::new()
        .prefix(".whereisit-preview-export-")
        .tempfile_in(&parent)?;
    let mut volume_index = 0usize;
    let mut entry_index = 0usize;
    let rows = std::iter::from_fn(|| loop {
        if volume_index >= snapshot.parsed.disks.len() {
            return None;
        }
        let disk = &snapshot.parsed.disks[volume_index];
        if entry_index >= disk.entries.len() {
            volume_index += 1;
            entry_index = 0;
            continue;
        }
        let vi = volume_index;
        let ei = entry_index;
        entry_index += 1;
        let checkpoint = if mode == "copy_candidates" {
            job.entry()
        } else {
            job.check()
        };
        if let Err(error) = checkpoint {
            return Some(Err(error));
        }
        let source = &disk.entries[ei];
        let row = RowRef {
            volume_index: vi as u32,
            entry_index: ei as u32,
        };
        let in_scope = match mode {
            "all" => true,
            "children" => {
                volume_id == Some(vi as i64 + 1)
                    && parent_scope
                        .is_none_or(|(pvi, pei, _)| pvi == vi && is_descendant(disk, ei, pei))
            }
            "copy_candidates" => {
                selected_key
                    .as_ref()
                    .is_some_and(|(_, source_vi, _, norm, size)| {
                        source.kind == EntryKind::File
                            && source.size == *size
                            && source.name.to_lowercase() == *norm
                            && (!other_disks_only || *source_vi != vi)
                            && candidate_volume_id.is_none_or(|id| id == vi as i64 + 1)
                    })
            }
            _ => true,
        };
        if !in_scope {
            continue;
        }
        if mode != "copy_candidates" {
            match matches_row(
                &snapshot,
                row,
                &query,
                query.needs_extension(),
                volume_id,
                &job,
            ) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => return Some(Err(error)),
            }
        }
        let entry = entry_at(&snapshot, vi, ei);
        let export_size = match i64::try_from(entry.size) {
            Ok(size) => size,
            Err(_) => {
                return Some(Err(failure(
                    "ctf_limit",
                    "CTF entry size exceeds export schema",
                )));
            }
        };
        let (duplicate_count, duplicate_group) = ("0".to_owned(), String::new());
        let record = crate::exports::ExportRow::from_parts(
            [
                Some(entry.id),
                Some(entry.volume_id),
                entry.parent_id,
                Some(export_size),
                None,
            ],
            [
                entry.name,
                entry.relative_path,
                entry.kind,
                entry.description,
                entry.tags,
                disk.name.clone(),
                String::new(),
                duplicate_count,
                duplicate_group,
            ],
            crate::dates::dos_local(source.modified_dos),
        );
        return Some(Ok(record));
    });
    let rows_written = crate::exports::write_rows(
        stage.as_file_mut(),
        format,
        v.get("bom").and_then(Value::as_bool).unwrap_or(false),
        rows,
    )?;
    job.check()?;
    let bytes = stage.as_file().metadata()?.len();
    let result = json!({"path":target,"rows":rows_written,"bytes":bytes,"format":extension,
        "scope":mode,"duplicate_candidates":mode == "copy_candidates"});
    let verified = checked_result(&snapshot, result, Some(&job))?;
    job.begin_publication()?;
    stage.persist_noclobber(&target).map_err(|error| {
        let code = if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            "file_exists"
        } else {
            "export"
        };
        Error::new(code, "CTF preview export publication failed")
    })?;
    let mut verified = verified;
    verified["result"]["path"] = json!(target);
    verified["result"]["source_snapshot_verified_before_publication"] = json!(true);
    verified["result"]["output_no_clobber"] = json!(true);
    Ok(verified)
}

fn is_descendant(disk: &ctf_format::CtfDisk, mut index: usize, ancestor: usize) -> bool {
    loop {
        if index == ancestor {
            return true;
        }
        let Some(parent) = disk.entries.get(index).and_then(|entry| entry.parent) else {
            return false;
        };
        index = parent;
    }
}

fn search_with_job(v: &Value, job: crate::ctf_job::Job) -> Result<Value> {
    let query_text = text(v, "query")?;
    let query = MetadataQuery::compile_metadata(query_text)?;
    let needs_extension = query.needs_extension();
    let volume_id = optional_id(v, "volume_id")?;
    let offset = unsigned(v, "offset", 0)?;
    let requested_limit = unsigned(v, "limit", 100)?;
    let limit = requested_limit.clamp(1, 1000);
    let full_count = match v.get("full_count") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err(failure("invalid_request", "full_count must be boolean")),
    };
    let use_duckdb = match v.get("executor") {
        None | Some(Value::Null) => cfg!(feature = "duckdb-query"),
        Some(Value::String(name)) if name == "rust" => false,
        Some(Value::String(name)) if name == "duckdb" => true,
        _ => return Err(failure("invalid_request", "Unknown CTF query executor")),
    };
    #[cfg(not(feature = "duckdb-query"))]
    if use_duckdb {
        return Err(failure(
            "unsupported",
            "DuckDB query prototype is not enabled in this build",
        ));
    }
    job.phase("preview_search")?;
    let snapshot = selected(v, Some(&job))?;
    job.check()?;
    #[cfg(feature = "duckdb-query")]
    if use_duckdb {
        let page = {
            let mut engine = snapshot
                .duckdb
                .lock()
                .map_err(|_| failure("internal", "CTF query executor is unavailable"))?;
            if engine.is_none() {
                *engine = Some(crate::ctf_duckdb::Engine::new(snapshot.parsed.clone())?);
            }
            let result = engine.as_mut().expect("engine initialized").query(
                &query,
                volume_id,
                offset.min(i64::MAX as u64),
                limit,
                full_count,
                job.observer(),
            );
            // Prefer the cooperative cancellation result to an engine wrapper
            // error, and never publish rows after a cancellation request.
            job.check()?;
            result?
        };
        let items = page
            .ids
            .iter()
            .map(|id| {
                let (volume_index, entry_index, _) = find_id(&snapshot, *id)
                    .ok_or_else(|| failure("integrity", "Query returned an unknown CTF entry"))?;
                Ok(entry_at(&snapshot, volume_index, entry_index))
            })
            .collect::<Result<Vec<_>>>()?;
        let result = if full_count {
            let total = page
                .total
                .ok_or_else(|| failure("integrity", "Query did not return a complete count"))?;
            serde_json::to_value(Page {
                items,
                total,
                offset,
                limit,
            })
        } else {
            serde_json::to_value(FastPage {
                items,
                offset,
                limit,
                has_more: page.has_more,
            })
        }
        .map_err(|_| failure("internal", "Cannot encode CTF query result"))?;
        let mut response = checked_result(&snapshot, result, Some(&job))?;
        response["query_executor"] = json!("duckdb");
        return Ok(response);
    }
    let effective_offset = offset.min(i64::MAX as u64);
    let page_end = effective_offset.saturating_add(limit);
    let mut total = 0u64;
    let mut page_rows = Vec::with_capacity(limit as usize + usize::from(!full_count));
    const MAX_PHYSICAL_HEAP_OFFSET: u64 = 100_000;
    if full_count && effective_offset <= MAX_PHYSICAL_HEAP_OFFSET {
        // Full counts need every row, but paging still requires only the first
        // offset+limit rows in name order. Scan source-order metadata (cache
        // friendly), keep that bounded prefix in a max heap, and sort only it.
        let retain = page_end as usize;
        let mut best = BinaryHeap::with_capacity(retain);
        for (volume_index, disk) in snapshot.parsed.disks.iter().enumerate() {
            if volume_id.is_some_and(|filter| filter != volume_index as i64 + 1) {
                continue;
            }
            for (entry_index, entry) in disk.entries.iter().enumerate() {
                let row = RowRef {
                    volume_index: volume_index as u32,
                    entry_index: entry_index as u32,
                };
                if !matches_row(&snapshot, row, &query, needs_extension, volume_id, &job)? {
                    continue;
                }
                total = total.saturating_add(1);
                let hit = SearchHit {
                    name: &entry.name,
                    id: snapshot.id_bases[volume_index] + entry_index as i64,
                    row,
                };
                if best.len() < retain {
                    best.push(hit);
                } else if best.peek().is_some_and(|largest| hit < *largest) {
                    best.pop();
                    best.push(hit);
                }
            }
        }
        let mut best = best.into_vec();
        best.sort_unstable();
        page_rows.extend(
            best.into_iter()
                .skip(effective_offset as usize)
                .take(limit as usize)
                .map(|hit| hit.row),
        );
    } else {
        // Fast pages and very large offsets walk the persistent global order.
        let rows = name_order(&snapshot);
        for &row in rows {
            if !matches_row(&snapshot, row, &query, needs_extension, volume_id, &job)? {
                continue;
            }
            let ordinal = total;
            total = total.saturating_add(1);
            if ordinal >= effective_offset && ordinal < page_end {
                page_rows.push(row);
            }
            if !full_count && total >= page_end.saturating_add(1) {
                break;
            }
        }
    }
    job.check()?;
    let items = page_rows
        .into_iter()
        .map(|row| {
            entry_at(
                &snapshot,
                row.volume_index as usize,
                row.entry_index as usize,
            )
        })
        .collect::<Vec<_>>();
    let result = if full_count {
        serde_json::to_value(Page {
            items,
            total,
            offset,
            limit,
        })
    } else {
        let has_more = total > page_end;
        serde_json::to_value(FastPage {
            items,
            offset,
            limit,
            has_more,
        })
    }
    .map_err(|_| failure("internal", "Cannot encode CTF search result"))?;
    let mut response = checked_result(&snapshot, result, Some(&job))?;
    response["query_executor"] = json!("rust");
    Ok(response)
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
pub(super) mod tests {
    use super::*;
    use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry};

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
    fn entry_capacity_compaction_preserves_ctf_bytes_and_preview_metadata() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("compaction.ctf");
        let source = CtfCatalog {
            name: "synthetic compaction 東京".into(),
            opaque_trailer: vec![37, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, b'G', 0],
            disks: vec![
                CtfDisk {
                    name: "Données".into(),
                    opaque_header: vec![],
                    entries: vec![entry("資料", None, EntryKind::Directory), {
                        let mut file = entry("résumé.txt", Some(0), EntryKind::File);
                        file.description = "description保留".into();
                        file.attributes = 0x20;
                        file.modified_dos = 0x1234;
                        file.created_dos = 0x5678;
                        file.accessed_dos = 0x9abc;
                        file
                    }],
                },
                CtfDisk {
                    name: "second".into(),
                    opaque_header: vec![],
                    entries: vec![entry("β.mkv", None, EntryKind::File)],
                },
            ],
        };
        let source_bytes = write_ctf(&source).unwrap();
        fs::write(&path, &source_bytes).unwrap();

        let mut parsed = ctf_format::parse_ctf(&source_bytes, &Limits::default()).unwrap();
        let before_compaction = write_ctf(&parsed).unwrap();
        assert_eq!(before_compaction, source_bytes);
        let old_capacities = parsed
            .disks
            .iter()
            .map(|disk| disk.entries.capacity())
            .collect::<Vec<_>>();
        compact_entry_capacity(&mut parsed);
        assert_eq!(write_ctf(&parsed).unwrap(), before_compaction);
        for (index, disk) in parsed.disks.iter().enumerate() {
            assert!(disk.entries.capacity() <= old_capacities[index]);
        }

        let opened = open(&json!({"path":path})).unwrap();
        let generation = opened["generation"].as_str().unwrap();
        let page = children(
            &json!({"path":path,"generation":generation,"volume_id":1,"parent_id":1,"limit":10}),
        )
        .unwrap();
        assert_eq!(page["result"]["items"][0]["name"], "résumé.txt");
        assert_eq!(page["result"]["items"][0]["id"], 2);
        let found = search(&json!({"path":path,"generation":generation,
            "query":"description:保留","full_count":true,"limit":10}))
        .unwrap();
        assert_eq!(found["result"]["total"], 1);
        assert_eq!(
            found["result"]["items"][0]["description"],
            "description保留"
        );
        assert_eq!(found["result"]["items"][0]["id"], 2);
        assert_eq!(fs::read(&path).unwrap(), source_bytes);
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
        let focused = children_focused(&json!({
            "path":path,"generation":generation,"volume_id":1,
            "parent_id":1,"entry_id":3,"limit":1
        }))
        .unwrap();
        let focused_imported = crate::focused_children::page(
            &catalog,
            &json!({
                "volume_id":1,"parent_id":1,"entry_id":3,"limit":1
            }),
        )
        .unwrap();
        assert_eq!(focused["result"], focused_imported);
        assert_eq!(focused["result"]["offset"], 1);
        let invalid_focus = children_focused(&json!({
            "path":path,"generation":generation,"volume_id":1,
            "parent_id":3,"entry_id":3,"limit":1
        }));
        assert_eq!(invalid_focus.unwrap_err().code, "not_found");
    }

    #[test]
    fn preview_ancestors_match_database_and_pinned_mirror_root_to_self() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ancestors.ctf");
        let mut nested = entry("nested", Some(1), EntryKind::Directory);
        nested.size = 0;
        let file = entry("film.mkv", Some(2), EntryKind::File);
        let source = CtfCatalog {
            name: "synthetic ancestor fixture".into(),
            opaque_trailer: vec![37, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, b'G', 0],
            disks: vec![
                CtfDisk {
                    name: "first".into(),
                    opaque_header: vec![],
                    entries: vec![
                        entry("root", None, EntryKind::Directory),
                        entry("inside.zip", Some(0), EntryKind::Archive),
                        nested,
                        file,
                    ],
                },
                CtfDisk {
                    name: "second".into(),
                    opaque_header: vec![],
                    entries: vec![entry("other", None, EntryKind::Directory)],
                },
            ],
        };
        let bytes = write_ctf(&source).unwrap();
        fs::write(&path, &bytes).unwrap();
        let before = fs::read(&path).unwrap();
        let opened = open(&json!({"path":path})).unwrap();
        let generation = opened["generation"].as_str().unwrap();
        let request = json!({"path":path,"generation":generation,"volume_id":1,"entry_id":4});
        let preview = ancestors(&request).unwrap();
        let expected = json!({"volume_id":1,"items":[
            {"id":1,"parent_id":null,"name":"root","kind":"directory"},
            {"id":2,"parent_id":1,"name":"inside.zip","kind":"archive"},
            {"id":3,"parent_id":2,"name":"nested","kind":"directory"},
            {"id":4,"parent_id":3,"name":"film.mkv","kind":"file"}
        ]});
        assert_eq!(preview["result"], expected);

        let database = dir.path().join("ancestors.sqlite");
        let mut catalog = atlas_core::Catalog::open(&database).unwrap();
        crate::legacy::import(&mut catalog, &json!({"path":path})).unwrap();
        assert_eq!(
            crate::focused_children::ancestors(&catalog, &json!({"volume_id":1,"entry_id":4}))
                .unwrap(),
            expected
        );
        drop(catalog);
        let dispatched = crate::dispatch_json(
            &json!({"op":"entry_ancestors","db":database,"volume_id":1,"entry_id":4}).to_string(),
        );
        let dispatched: Value = serde_json::from_str(&dispatched).unwrap();
        assert_eq!(dispatched["ok"], true);
        assert_eq!(dispatched["data"], expected);
        assert_eq!(
            ancestors(&json!({"path":path,"generation":generation,"volume_id":2,"entry_id":4}))
                .unwrap_err()
                .code,
            "not_found"
        );
        assert_eq!(
            ancestors(&json!({"path":path,"generation":generation,"volume_id":1,"entry_id":999}))
                .unwrap_err()
                .code,
            "not_found"
        );

        let cache_root = dir.path().join("pinned-cache");
        let pinned =
            crate::ctf_mirror::sync(&json!({"path":path,"cache_root":cache_root})).unwrap();
        let pinned_result = crate::ctf_mirror::ancestors_pinned(&json!({
            "path":path,"cache_root":cache_root,"generation":pinned["generation"],
            "volume_id":1,"entry_id":4
        }))
        .unwrap();
        assert_eq!(pinned_result["result"], expected);

        let malformed = vec![entry("self-cycle", Some(0), EntryKind::Directory)];
        assert_eq!(
            ancestor_items(1, 1, &malformed, 1).unwrap_err().code,
            "ctf_tree"
        );
        let mut deep = Vec::with_capacity(MAX_ANCESTOR_DEPTH + 1);
        for index in 0..=MAX_ANCESTOR_DEPTH {
            deep.push(entry("deep", index.checked_sub(1), EntryKind::Directory));
        }
        assert_eq!(
            ancestor_items(1, 1, &deep, deep.len() as i64)
                .unwrap_err()
                .code,
            "ctf_tree"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn preview_copy_candidates_use_sqlite_path_order_for_pages() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reverse-copy-paths.ctf");
        let matching = |parent| {
            let mut item = entry("Film.MKV", Some(parent), EntryKind::File);
            item.size = 4096;
            item
        };
        let bytes = write_ctf(&CtfCatalog {
            name: "reverse candidate path ordering".into(),
            opaque_trailer: vec![37, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, b'G', 0],
            disks: vec![
                CtfDisk {
                    name: "disk".into(),
                    opaque_header: vec![],
                    entries: vec![
                        entry("Z-folder", None, EntryKind::Directory),
                        matching(0),
                        entry("A-folder", None, EntryKind::Directory),
                        matching(2),
                        entry("M-folder", None, EntryKind::Directory),
                        matching(4),
                    ],
                },
                CtfDisk {
                    name: "second disk".into(),
                    opaque_header: vec![],
                    entries: vec![entry("0-folder", None, EntryKind::Directory), matching(0)],
                },
            ],
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();
        let opened = open(&json!({"path":path})).unwrap();
        let generation = opened["generation"].as_str().unwrap();
        let snapshot = selected(&req(&path, generation), None).unwrap();
        let selected_id = snapshot
            .parsed
            .disks
            .iter()
            .enumerate()
            .find_map(|(volume_index, disk)| {
                disk.entries
                    .iter()
                    .enumerate()
                    .find_map(|(entry_index, item)| {
                        (item.name == "Film.MKV"
                            && item.size == 4096
                            && item.kind == EntryKind::File)
                            .then_some(snapshot.id_bases[volume_index] + entry_index as i64)
                    })
            })
            .unwrap();
        let mut catalog = atlas_core::Catalog::open(dir.path().join("copy-order.sqlite")).unwrap();
        crate::legacy::import(&mut catalog, &json!({"path":path})).unwrap();
        // Volume ID is the primary key even though the second disk has the
        // lexically first path; paths and IDs order rows within each disk.
        for offset in 0..4 {
            let request = json!({"path":path,"generation":generation,
                "entry_id":selected_id,"offset":offset,"limit":1});
            let actual = copy_candidates(&request).unwrap();
            let expected = crate::copy_candidates::page(&catalog, &request).unwrap();
            assert_eq!(actual["result"], expected, "offset={offset}");
        }
    }

    #[test]
    fn direct_search_matches_sqlite_queries_pagination_and_shapes() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("search-parity.ctf");
        write_fixture(&path, "search parity");
        let opened = open(&json!({"path":path})).unwrap();
        let generation = opened["generation"].as_str().unwrap();
        let mut catalog = atlas_core::Catalog::open(dir.path().join("search.sqlite")).unwrap();
        crate::legacy::import(&mut catalog, &json!({"path":path})).unwrap();

        for query_text in [
            "",
            "name:東京",
            "ext:txt",
            "size:>=17",
            "kind:file",
            "path:root/東京",
            "branch_id:2",
            "primary_category_id:0",
            "category_exact_id:1",
            "category_id:1",
            "name:no-such-entry",
            "text:\"root/東京\"",
        ] {
            for volume_id in [None, Some(1), Some(2)] {
                let mut request = json!({
                    "path":path, "generation":generation, "query":query_text,
                    "offset":1, "limit":2, "full_count":true,
                });
                if let Some(volume_id) = volume_id {
                    request["volume_id"] = json!(volume_id);
                }
                let actual = search(&request).unwrap();
                let expected =
                    serde_json::to_value(catalog.search(query_text, volume_id, 1, 2).unwrap())
                        .unwrap();
                assert_eq!(
                    actual["result"], expected,
                    "query={query_text}, volume={volume_id:?}"
                );
                request["full_count"] = json!(false);
                let actual = search(&request).unwrap();
                let expected =
                    serde_json::to_value(catalog.search_fast(query_text, volume_id, 1, 2).unwrap())
                        .unwrap();
                assert_eq!(
                    actual["result"], expected,
                    "fast query={query_text}, volume={volume_id:?}"
                );
            }
        }
    }

    #[test]
    fn direct_search_heap_pages_match_sqlite_across_physical_volumes() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("heap-pages.ctf");
        let make_disk = |disk_name: &str, disk_index: usize| {
            let mut entries = Vec::with_capacity(321);
            // Reverse physical order so the bounded heap must order by the
            // SQLite name collation and stable global ID, not source order.
            for n in (0..321).rev() {
                let name = match n % 17 {
                    0 => "Twin.mkv".to_owned(),
                    1 => "twin.MKV".to_owned(),
                    2 => format!("東京-{:03}.mp3", n),
                    _ => format!(
                        "item-{:03}-disk-{disk_index}.{}",
                        n,
                        if n % 2 == 0 { "mkv" } else { "mp3" }
                    ),
                };
                entries.push(entry(&name, None, EntryKind::File));
            }
            CtfDisk {
                name: disk_name.into(),
                opaque_header: vec![],
                entries,
            }
        };
        let bytes = write_ctf(&CtfCatalog {
            name: "heap pagination synthetic".into(),
            opaque_trailer: vec![37, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, b'G', 0],
            disks: vec![make_disk("first", 0), make_disk("second", 1)],
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();
        let opened = open(&json!({"path":path})).unwrap();
        let generation = opened["generation"].as_str().unwrap();
        let mut catalog = atlas_core::Catalog::open(dir.path().join("heap.sqlite")).unwrap();
        crate::legacy::import(&mut catalog, &json!({"path":path})).unwrap();

        // The full-count path uses the bounded physical-order heap through
        // offset 100,000. Fast mode and larger offsets use global name order.
        for &(offset, limit, volume_id) in &[
            (0u64, 250u64, None),
            (250, 250, None),
            (500, 250, None),
            (0, 0, None),    // limit clamps to one
            (0, 5000, None), // limit clamps to one thousand
            (0, 250, Some(1i64)),
            (250, 250, Some(2i64)),
            (100_001, 250, None), // heap threshold fallback
        ] {
            for full_count in [true, false] {
                let mut request = json!({
                    "path":path, "generation":generation, "query":"", "offset":offset,
                    "limit":limit, "full_count":full_count,
                });
                if let Some(volume_id) = volume_id {
                    request["volume_id"] = json!(volume_id);
                }
                let actual = search(&request).unwrap()["result"].clone();
                let effective_limit = limit.clamp(1, 1000);
                let expected = if full_count {
                    serde_json::to_value(
                        catalog
                            .search("", volume_id, offset, effective_limit)
                            .unwrap(),
                    )
                    .unwrap()
                } else {
                    serde_json::to_value(
                        catalog
                            .search_fast("", volume_id, offset, effective_limit)
                            .unwrap(),
                    )
                    .unwrap()
                };
                assert_eq!(
                    actual, expected,
                    "offset={offset}, limit={limit}, volume={volume_id:?}, full_count={full_count}"
                );
            }
        }
    }

    #[test]
    fn direct_search_honors_pre_cancelled_job() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cancel-search.ctf");
        write_fixture(&path, "cancel search");
        let opened = open(&json!({"path":path})).unwrap();
        let request = json!({
            "path":path, "generation":opened["generation"], "query":"", "job_id":"preview-search-cancel-test"
        });
        let job = crate::ctf_job::Job::start(&request).unwrap();
        crate::ctf_job::cancel(&json!({"job_id":"preview-search-cancel-test"})).unwrap();
        assert_eq!(
            search_with_job(&request, job).unwrap_err().code,
            "cancelled"
        );
    }

    #[test]
    fn preview_copy_candidates_and_exports_preserve_contracts() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidate-export.ctf");
        let mut first = entry("Shelf", None, EntryKind::Directory);
        first.modified_dos = (((2024 - 1980) << 9 | 3 << 5 | 5) << 16) | (9 << 11 | 12 << 5 | 4);
        let mut selected = entry("映画 🎬.mkv", Some(0), EntryKind::File);
        selected.size = 4096;
        selected.description = "候補".into();
        selected.modified_dos = first.modified_dos;
        let mut same_volume = entry("映画 🎬.mkv", Some(0), EntryKind::File);
        same_volume.size = 4096;
        same_volume.modified_dos = first.modified_dos;
        let mut second_root = entry("Shelf B", None, EntryKind::Directory);
        second_root.modified_dos = first.modified_dos;
        let mut other_volume = entry("映画 🎬.MKV", Some(0), EntryKind::File);
        other_volume.size = 4096;
        other_volume.description = "別候補".into();
        other_volume.modified_dos = first.modified_dos;
        let bytes = write_ctf(&CtfCatalog {
            name: "candidate export synthetic".into(),
            opaque_trailer: vec![37, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, b'G', 0],
            disks: vec![
                CtfDisk {
                    name: "Disk A".into(),
                    opaque_header: vec![],
                    entries: vec![first, selected, same_volume],
                },
                CtfDisk {
                    name: "Disk B".into(),
                    opaque_header: vec![],
                    entries: vec![second_root, other_volume],
                },
            ],
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();
        let source_before = fs::read(&path).unwrap();
        let opened = open(&json!({"path":path})).unwrap();
        let generation = opened["generation"].as_str().unwrap();
        let mut catalog = atlas_core::Catalog::open(dir.path().join("candidate.sqlite")).unwrap();
        crate::legacy::import(&mut catalog, &json!({"path":path})).unwrap();

        let request = json!({"path":path,"generation":generation,"entry_id":2,"limit":20});
        let candidates = copy_candidates(&request).unwrap();
        let imported_candidates = crate::copy_candidates::page(&catalog, &request).unwrap();
        assert_eq!(candidates["result"], imported_candidates);
        let other_scope = json!({"path":path,"generation":generation,"entry_id":2,
            "other_disks_only":true,"candidate_volume_id":2,"offset":0,"limit":1});
        let scoped = copy_candidates(&other_scope).unwrap();
        let imported_scoped = crate::copy_candidates::page(&catalog, &other_scope).unwrap();
        assert_eq!(scoped["result"], imported_scoped);

        for format in ["csv", "json", "parquet"] {
            let expected_path = dir.path().join(format!("sqlite.{format}"));
            let preview_path = dir.path().join(format!("preview.{format}"));
            let expected = crate::exports::export(
                &catalog,
                &json!({"path":expected_path,
                "db":dir.path().join("candidate.sqlite"),"mode":"all"}),
                &format!("export_{format}"),
            )
            .unwrap();
            let actual = export(&json!({"path":path,"generation":generation,
                "output":preview_path,"format":format,"mode":"all"}))
            .unwrap();
            assert_eq!(actual["result"]["rows"], expected["rows"]);
            assert_eq!(actual["result"]["format"], format);
            assert_eq!(actual["result"]["output_no_clobber"], true);
            if format == "json" {
                let expected_json: Value =
                    serde_json::from_slice(&fs::read(&expected_path).unwrap()).unwrap();
                let actual_json: Value =
                    serde_json::from_slice(&fs::read(&preview_path).unwrap()).unwrap();
                assert_eq!(actual_json, expected_json);
                assert_eq!(actual_json["entries"][1]["volume_label"], "Disk A");
                assert_eq!(actual_json["entries"][1]["description"], "候補");
                assert_eq!(
                    actual_json["entries"][1]["modified_local"],
                    "2024-03-05 09:12:08"
                );
            }
            if format == "csv" {
                assert_eq!(
                    fs::read(&expected_path).unwrap(),
                    fs::read(&preview_path).unwrap()
                );
            }
            if format == "parquet" {
                use parquet::file::reader::{FileReader, SerializedFileReader};
                let decode = |path: &Path| {
                    let reader = SerializedFileReader::new(fs::File::open(path).unwrap()).unwrap();
                    assert_eq!(
                        reader
                            .metadata()
                            .file_metadata()
                            .schema_descr()
                            .num_columns(),
                        15
                    );
                    reader
                        .get_row_iter(None)
                        .unwrap()
                        .map(|row| {
                            let row = row.unwrap();
                            assert_eq!(row.len(), 15);
                            row.into_columns()
                                .into_iter()
                                .map(|(name, field)| (name, format!("{field:?}")))
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(decode(&preview_path), decode(&expected_path));
            }
        }
        assert_eq!(fs::read(&path).unwrap(), source_before);

        let existing = dir.path().join("existing.csv");
        fs::write(&existing, b"keep").unwrap();
        let no_clobber = export(&json!({"path":path,"generation":generation,
            "output":existing,"format":"csv","mode":"all","overwrite":true}));
        assert_eq!(no_clobber.unwrap_err().code, "file_exists");
        assert_eq!(fs::read(existing).unwrap(), b"keep");
        let invalid = export(&json!({"path":path,"generation":generation,
            "output":dir.path().join("bad.csv"),"format":"csv","mode":"unknown"}));
        assert_eq!(invalid.unwrap_err().code, "invalid_request");
        let mut changed = fs::read(&path).unwrap();
        changed.push(0);
        fs::write(&path, changed).unwrap();
        let changed_result = export(&json!({"path":path,"generation":generation,
            "output":dir.path().join("changed.csv"),"format":"csv","mode":"all"}));
        assert_eq!(changed_result.unwrap_err().code, "source_changed");
        assert!(!dir.path().join("changed.csv").exists());
    }

    #[test]
    fn pre_cancelled_preview_export_does_not_publish_output() {
        let _serial = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("cancel-export.ctf");
        write_fixture(&source, "cancel");
        let opened = open(&json!({"path":source})).unwrap();
        let output = dir.path().join("cancelled.json");
        let request = json!({"path":source,"generation":opened["generation"],
            "output":output,"format":"json","mode":"all","job_id":"cancel-preview-export-test"});
        let job = json!({"job_id":"cancel-preview-export-test"});
        let running_job = crate::ctf_job::Job::start(&request).unwrap();
        crate::ctf_job::cancel(&job).unwrap();
        assert_eq!(
            export_with_job(&request, running_job).unwrap_err().code,
            "cancelled"
        );
        assert!(!output.exists());
    }
}
