//! Read-only analytics over the validated direct CTF snapshot.
//! The source CTF is authoritative; this module does not create a catalog.

use crate::ctf_preview::{self, Snapshot};
use atlas_core::{
    analytics::{
        AnalyticsNode, AnalyticsRequest, AnalyticsResponse, AnalyticsSlice, AnalyticsTotals,
    },
    query::MetadataQuery,
    Error, Result,
};
use ctf_format::EntryKind;
use serde_json::{json, Value};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, VecDeque},
    path::Path,
};

#[derive(Clone, Copy, Default)]
struct Stats {
    nodes: u32,
    files: u32,
    directories: u32,
    matched_files: u32,
    bytes: u64,
    matched_bytes: u64,
}

impl Stats {
    fn add(&mut self, other: Self) -> Result<()> {
        self.nodes = self.nodes.checked_add(other.nodes).ok_or_else(overflow)?;
        self.files = self.files.checked_add(other.files).ok_or_else(overflow)?;
        self.directories = self
            .directories
            .checked_add(other.directories)
            .ok_or_else(overflow)?;
        self.matched_files = self
            .matched_files
            .checked_add(other.matched_files)
            .ok_or_else(overflow)?;
        self.bytes = self.bytes.checked_add(other.bytes).ok_or_else(overflow)?;
        self.matched_bytes = self
            .matched_bytes
            .checked_add(other.matched_bytes)
            .ok_or_else(overflow)?;
        Ok(())
    }
}

#[derive(Default, Clone, Copy)]
struct SliceStats {
    files: u64,
    bytes: u64,
    matched_files: u64,
    matched_bytes: u64,
}
impl SliceStats {
    fn add(&mut self, other: Self) -> Result<()> {
        self.files = self.files.checked_add(other.files).ok_or_else(overflow)?;
        self.bytes = self.bytes.checked_add(other.bytes).ok_or_else(overflow)?;
        self.matched_files = self
            .matched_files
            .checked_add(other.matched_files)
            .ok_or_else(overflow)?;
        self.matched_bytes = self
            .matched_bytes
            .checked_add(other.matched_bytes)
            .ok_or_else(overflow)?;
        Ok(())
    }
}

struct ChildHit<'a> {
    name: &'a str,
    id: i64,
    index: usize,
}
impl PartialEq for ChildHit<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for ChildHit<'_> {}
impl PartialOrd for ChildHit<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ChildHit<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        ascii_nocase(self.name, other.name).then_with(|| self.id.cmp(&other.id))
    }
}

fn ascii_nocase(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|x| x.to_ascii_lowercase())
        .cmp(b.bytes().map(|x| x.to_ascii_lowercase()))
}

fn failure(code: &'static str, message: &'static str) -> Error {
    Error::new(code, message)
}
fn overflow() -> Error {
    failure(
        "analytics_overflow",
        "CTF analytics total exceeds the supported range",
    )
}

/// Called by the prepared native CTF analytics worker. The phase check comes
/// before deserialization, source identity reads, or any metadata access.
pub(crate) fn analytics(v: &Value, job: crate::ctf_job::Job) -> Result<Value> {
    job.phase("preview_analytics")?;
    let request: AnalyticsRequest =
        serde_json::from_value(v.clone()).map_err(|error| Error::new("invalid_request", error))?;
    if !["volumes", "categories", "extensions"].contains(&request.group_by.as_str()) {
        return Err(failure(
            "invalid_request",
            "group_by must be volumes, categories, or extensions",
        ));
    }
    if request.volume_id.is_some_and(|id| id <= 0)
        || request.folder_entry_id.is_some_and(|id| id <= 0)
        || request.category_id.is_some_and(|id| id <= 0)
    {
        return Err(failure(
            "invalid_request",
            "scope IDs and category_id must be positive",
        ));
    }
    // Native CTF preview has no category assignments or category definitions.
    if request.category_id.is_some() {
        return Err(failure("not_found", "category not found"));
    }
    let max_depth = request.max_depth.clamp(1, 32);
    let max_nodes = request.max_nodes.clamp(1, 20_000);
    let highlight = request.highlight_query.as_deref().unwrap_or("").trim();
    let query = if highlight.is_empty() {
        None
    } else {
        Some(MetadataQuery::compile_metadata(highlight)?)
    };
    let snapshot = ctf_preview::selected(v, Some(&job))?;
    let needs_extension = query.as_ref().is_some_and(MetadataQuery::needs_extension);
    job.check()?;

    let volume_count = snapshot.parsed.disks.len();
    let volume_index = if let Some(id) = request.volume_id {
        let index = usize::try_from(id - 1).unwrap_or(usize::MAX);
        if index >= volume_count {
            return Err(failure("not_found", "volume not found"));
        }
        Some(index)
    } else {
        None
    };
    let folder = if let Some(id) = request.folder_entry_id {
        let found = find_id(&snapshot, id)
            .filter(|(vi, _, entry)| {
                volume_index.is_none_or(|volume| volume == *vi)
                    && matches!(entry.kind, EntryKind::Directory | EntryKind::Archive)
            })
            .ok_or_else(|| failure("not_found", "folder entry not found in requested volume"))?;
        Some((found.0, found.1, id))
    } else {
        None
    };
    let scoped_volume = volume_index.or_else(|| folder.map(|(vi, _, _)| vi));

    // Per-entry counters are compact and only hold integers. Parent-before-child
    // validation lets one reverse pass accumulate subtree totals safely.
    let mut stats_by_disk = Vec::with_capacity(volume_count);
    let mut disk_stats = vec![Stats::default(); volume_count];
    let mut extension_stats: HashMap<String, SliceStats> = HashMap::new();
    let mut size_stats = [SliceStats::default(); 4];
    for (vi, disk) in snapshot.parsed.disks.iter().enumerate() {
        let process_disk = scoped_volume.is_none_or(|selected| selected == vi);
        if !process_disk {
            stats_by_disk.push(Vec::new());
            continue;
        }
        let mut stats = vec![Stats::default(); disk.entries.len()];
        let mut included = vec![false; disk.entries.len()];
        for (index, entry) in disk.entries.iter().enumerate() {
            job.entry()?;
            included[index] = if let Some((folder_volume, folder_index, _)) = folder {
                if vi != folder_volume || index < folder_index {
                    false
                } else if index == folder_index {
                    true
                } else if let Some(parent) = entry.parent {
                    if parent >= index {
                        return Err(failure("ctf_tree", "CTF parent is not before its child"));
                    }
                    included[parent]
                } else {
                    false
                }
            } else {
                true
            };
            if !included[index] {
                continue;
            }
            let mut row = Stats {
                nodes: 1,
                ..Stats::default()
            };
            if entry.kind == EntryKind::Directory {
                row.directories = 1;
            }
            if entry.kind == EntryKind::File {
                row.files = 1;
                row.bytes = entry.size;
                if query.as_ref().is_some_and(|query| {
                    ctf_preview::matches_metadata(&snapshot, vi, index, query, needs_extension)
                }) {
                    row.matched_files = 1;
                    row.matched_bytes = entry.size;
                }
                let extension = Path::new(&entry.name)
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                extension_stats
                    .entry(extension)
                    .or_default()
                    .add(SliceStats {
                        files: 1,
                        bytes: entry.size,
                        matched_files: row.matched_files as u64,
                        matched_bytes: row.matched_bytes,
                    })?;
                size_stats[size_bucket(entry.size)].add(SliceStats {
                    files: 1,
                    bytes: entry.size,
                    matched_files: row.matched_files as u64,
                    matched_bytes: row.matched_bytes,
                })?;
            }
            stats[index] = row;
        }
        for index in (0..disk.entries.len()).rev() {
            if index % 2048 == 0 {
                job.check()?;
            }
            if !included[index] {
                continue;
            }
            if let Some(parent) = disk.entries[index].parent {
                if parent >= index {
                    return Err(failure("ctf_tree", "CTF parent is not before its child"));
                }
                let scope_root = folder.is_some_and(|(folder_volume, folder_index, _)| {
                    vi == folder_volume && index == folder_index
                });
                if !included[parent] && !scope_root {
                    return Err(failure("ctf_tree", "CTF parent scope is invalid"));
                }
                if included[parent] {
                    let child = stats[index];
                    stats[parent].add(child)?;
                }
            }
        }
        if let Some((folder_volume, folder_index, _)) = folder {
            if vi == folder_volume {
                disk_stats[vi] = stats[folder_index];
            }
        } else {
            for (index, entry) in disk.entries.iter().enumerate() {
                if entry.parent.is_none() {
                    disk_stats[vi].add(stats[index])?;
                }
            }
        }
        stats_by_disk.push(stats);
    }
    let mut scope_stats = Stats::default();
    if let Some(vi) = scoped_volume {
        scope_stats = disk_stats[vi];
    } else {
        for summary in &disk_stats {
            scope_stats.add(*summary)?;
        }
    }

    let by_extension = extension_slices(&extension_stats);
    let by_category = if scope_stats.files == 0 {
        Vec::new()
    } else {
        vec![AnalyticsSlice {
            id: "category:uncategorized".into(),
            label: "(uncategorized)".into(),
            bytes: scope_stats.bytes,
            file_count: scope_stats.files as u64,
        }]
    };
    let by_size_bucket = size_slices(&size_stats);
    let by_volume = disk_stats
        .iter()
        .enumerate()
        .filter(|(_, stats)| stats.files > 0)
        .map(|(vi, stats)| AnalyticsSlice {
            id: format!("volume:{}", vi + 1),
            label: snapshot.parsed.disks[vi].name.clone(),
            bytes: stats.bytes,
            file_count: stats.files as u64,
        })
        .collect::<Vec<_>>();

    let mut root = if let Some((vi, index, id)) = folder {
        let entry = &snapshot.parsed.disks[vi].entries[index];
        make_node(
            format!("entry:{id}"),
            entry.name.clone(),
            scope_stats.bytes,
            kind_name(entry.kind).into(),
            Some(vi as i64 + 1),
            Some(ctf_preview::relative_path(
                &snapshot.parsed.disks[vi],
                index,
            )),
            Some(id),
        )
    } else {
        make_node(
            "root".into(),
            "Catalog".into(),
            scope_stats.bytes,
            "root".into(),
            None,
            None,
            None,
        )
    };
    root.item_count = scope_stats.files as u64;
    root.matched_files = scope_stats.matched_files as u64;
    root.matched_bytes = scope_stats.matched_bytes;

    let breadcrumbs = breadcrumbs(&snapshot, volume_index, folder);
    let mut truncated = false;
    let mut omitted = 0u64;
    let mut remainder = 0u64;
    let mut emitted = 0usize;
    let mut queue = VecDeque::new();

    if request.group_by != "volumes" {
        let groups = if request.group_by == "extensions" {
            &by_extension
        } else {
            &by_category
        };
        for slice in groups {
            if emitted >= max_nodes {
                truncated = true;
                omitted = omitted.saturating_add(1);
                remainder = remainder.saturating_add(slice.bytes);
                continue;
            }
            let kind = if request.group_by == "categories" {
                "category"
            } else {
                "other_group"
            };
            let mut group = make_node(
                slice.id.clone(),
                slice.label.clone(),
                slice.bytes,
                kind.into(),
                None,
                None,
                None,
            );
            group.item_count = slice.file_count;
            if request.group_by == "extensions" {
                if let Some(stats) =
                    extension_stats.get(slice.id.strip_prefix("ext:").unwrap_or(""))
                {
                    group.matched_files = stats.matched_files;
                    group.matched_bytes = stats.matched_bytes;
                }
            } else {
                group.matched_files = scope_stats.matched_files as u64;
                group.matched_bytes = scope_stats.matched_bytes;
            }
            root.children.push(group);
            emitted += 1;
        }
    } else if volume_index.is_none() && folder.is_none() {
        for vi in 0..volume_count {
            job.check()?;
            let stats = disk_stats[vi];
            if emitted >= max_nodes {
                truncated = true;
                omitted =
                    omitted.saturating_add(snapshot.parsed.disks[vi].entries.len() as u64 + 1);
                remainder = remainder.saturating_add(stats.bytes);
                continue;
            }
            let id = vi as i64 + 1;
            let mut volume = make_node(
                format!("volume:{id}"),
                snapshot.parsed.disks[vi].name.clone(),
                stats.bytes,
                "volume".into(),
                Some(id),
                None,
                None,
            );
            volume.item_count = stats.files as u64;
            volume.matched_files = stats.matched_files as u64;
            volume.matched_bytes = stats.matched_bytes;
            root.children.push(volume);
            queue.push_back((vi, None, 0usize, vec![root.children.len() - 1]));
            emitted += 1;
        }
    } else if let Some(vi) = scoped_volume {
        queue.push_back((vi, folder.map(|(_, _, id)| id), 0usize, Vec::new()));
    }

    while let Some((vi, parent_id, depth, node_path)) = queue.pop_front() {
        job.check()?;
        let child_indexes = snapshot.children[vi]
            .get(&parent_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut direct = Stats::default();
        let mut best = BinaryHeap::new();
        let remaining = max_nodes.saturating_sub(emitted);
        let keep = if depth >= max_depth {
            0
        } else {
            remaining.saturating_add(1)
        };
        for (seen, &index) in child_indexes.iter().enumerate() {
            if seen % 2048 == 0 {
                job.check()?;
            }
            let stat = stats_by_disk[vi][index];
            direct.add(stat)?;
            if keep == 0 {
                continue;
            }
            let entry = &snapshot.parsed.disks[vi].entries[index];
            let hit = ChildHit {
                name: &entry.name,
                id: snapshot.id_bases[vi] + index as i64,
                index,
            };
            if best.len() < keep {
                best.push(hit);
            } else if best.peek().is_some_and(|largest| hit < *largest) {
                best.pop();
                best.push(hit);
            }
        }
        if direct.nodes == 0 {
            continue;
        }
        if depth >= max_depth {
            truncated = true;
            omitted = omitted.saturating_add(direct.nodes as u64);
            remainder = remainder.saturating_add(direct.bytes);
            continue;
        }
        let mut ordered = best.into_vec();
        ordered.sort_unstable();
        let child_count = child_indexes.len();
        let emit_count = remaining.min(child_count);
        let mut emitted_subtree = Stats::default();
        for hit in ordered.into_iter().take(emit_count) {
            let entry = &snapshot.parsed.disks[vi].entries[hit.index];
            let id = hit.id;
            let stat = stats_by_disk[vi][hit.index];
            let mut child = make_node(
                format!("entry:{id}"),
                entry.name.clone(),
                stat.bytes,
                kind_name(entry.kind).into(),
                Some(vi as i64 + 1),
                Some(ctf_preview::relative_path(
                    &snapshot.parsed.disks[vi],
                    hit.index,
                )),
                Some(id),
            );
            child.item_count = stat.files as u64;
            child.matched_files = stat.matched_files as u64;
            child.matched_bytes = stat.matched_bytes;
            emitted_subtree.add(stat)?;
            let child_index = if node_path.is_empty() {
                root.children.push(child);
                root.children.len() - 1
            } else {
                let parent = node_mut(&mut root, &node_path);
                parent.children.push(child);
                parent.children.len() - 1
            };
            let mut next_path = node_path.clone();
            next_path.push(child_index);
            queue.push_back((vi, Some(id), depth + 1, next_path));
            emitted += 1;
        }
        if child_count > emit_count {
            truncated = true;
            omitted =
                omitted.saturating_add(direct.nodes.saturating_sub(emitted_subtree.nodes) as u64);
            remainder =
                remainder.saturating_add(direct.bytes.saturating_sub(emitted_subtree.bytes));
        }
    }

    job.check()?;
    let response = AnalyticsResponse {
        root,
        breadcrumbs,
        totals: AnalyticsTotals {
            files: scope_stats.files as u64,
            directories: scope_stats.directories as u64,
            indexed_bytes: scope_stats.bytes,
        },
        by_extension,
        by_category,
        by_size_bucket,
        by_volume,
        truncated,
        omitted_node_count: omitted,
        remainder_bytes: remainder,
        media_fields: "unknown",
    };
    let value = serde_json::to_value(response).map_err(|error| Error::new("internal", error))?;
    ctf_preview::checked_result(&snapshot, value, Some(&job))
}

fn find_id(snapshot: &Snapshot, id: i64) -> Option<(usize, usize, &ctf_format::MetadataEntry)> {
    snapshot.id_bases.iter().enumerate().find_map(|(vi, base)| {
        let index = usize::try_from(id.checked_sub(*base)?).ok()?;
        Some((
            vi,
            index,
            snapshot.parsed.disks.get(vi)?.entries.get(index)?,
        ))
    })
}

fn kind_name(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::File => "file",
        EntryKind::Directory => "directory",
        EntryKind::Archive => "archive",
    }
}

fn size_bucket(size: u64) -> usize {
    if size < 1_048_576 {
        0
    } else if size < 1_073_741_824 {
        1
    } else if size < 10_737_418_240 {
        2
    } else {
        3
    }
}

fn extension_slices(groups: &HashMap<String, SliceStats>) -> Vec<AnalyticsSlice> {
    let mut slices = groups
        .iter()
        .map(|(extension, stats)| AnalyticsSlice {
            id: format!("ext:{extension}"),
            label: if extension.is_empty() {
                "(no extension)".into()
            } else {
                format!(".{extension}")
            },
            bytes: stats.bytes,
            file_count: stats.files,
        })
        .collect::<Vec<_>>();
    slices.sort_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then_with(|| {
                ascii_nocase(
                    a.id.strip_prefix("ext:").unwrap_or(""),
                    b.id.strip_prefix("ext:").unwrap_or(""),
                )
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    slices
}

fn size_slices(stats: &[SliceStats; 4]) -> Vec<AnalyticsSlice> {
    let labels = ["<1 MiB", "1 MiB–1 GiB", "1–10 GiB", ">=10 GiB"];
    stats
        .iter()
        .enumerate()
        .filter(|(_, s)| s.files > 0)
        .map(|(index, s)| AnalyticsSlice {
            id: format!("size:{index}"),
            label: labels[index].into(),
            bytes: s.bytes,
            file_count: s.files,
        })
        .collect()
}

fn make_node(
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
        children: Vec::new(),
    }
}

fn node_mut<'a>(root: &'a mut AnalyticsNode, path: &[usize]) -> &'a mut AnalyticsNode {
    let mut node = root;
    for index in path {
        node = &mut node.children[*index];
    }
    node
}

fn breadcrumbs(
    snapshot: &Snapshot,
    volume_index: Option<usize>,
    folder: Option<(usize, usize, i64)>,
) -> Vec<Value> {
    let mut result = vec![json!({"label":"Catalog","volumeId":null,"folderEntryId":null})];
    if let Some(vi) = volume_index {
        result.push(json!({"label":snapshot.parsed.disks[vi].name,"volumeId":vi as i64 + 1,"folderEntryId":null}));
    }
    if let Some((vi, index, _)) = folder {
        let disk = &snapshot.parsed.disks[vi];
        let mut chain = Vec::new();
        let mut cursor = Some(index);
        while let Some(i) = cursor {
            chain.push(i);
            cursor = disk.entries[i].parent;
        }
        for i in chain.into_iter().rev() {
            result.push(
                json!({"label":disk.entries[i].name,"volumeId":vi as i64 + 1,
                "folderEntryId":snapshot.id_bases[vi] + i as i64}),
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry};
    use std::fs;

    fn entry(
        name: &str,
        parent: Option<usize>,
        kind: EntryKind,
        size: u64,
        description: &str,
    ) -> CtfEntry {
        CtfEntry {
            name: name.into(),
            description: description.into(),
            parent,
            kind,
            size,
            attributes: 0,
            modified_dos: 0,
            created_dos: 0,
            accessed_dos: 0,
            opaque: vec![],
        }
    }

    fn fixture(path: &Path) {
        let bytes = write_ctf(&CtfCatalog {
            name: "synthetic analytics parity".into(),
            opaque_trailer: vec![37, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, b'G', 0],
            disks: vec![
                CtfDisk {
                    name: "Disk A".into(),
                    opaque_header: vec![],
                    entries: vec![
                        entry("Z", None, EntryKind::Directory, 0, ""),
                        entry(
                            "Film.MKV",
                            Some(0),
                            EntryKind::File,
                            1_048_575,
                            "needle movie",
                        ),
                        entry("Container", Some(0), EntryKind::Archive, 0, ""),
                        entry(
                            "huge.MP4",
                            Some(2),
                            EntryKind::File,
                            10_737_418_240,
                            "needle",
                        ),
                        entry("zero", Some(0), EntryKind::File, 0, "empty"),
                        entry("A", None, EntryKind::Directory, 0, ""),
                        entry("small.TXT", Some(5), EntryKind::File, 1_048_576, ""),
                        entry("mid.bin", Some(5), EntryKind::File, 1_073_741_824, ""),
                        entry("root.avi", None, EntryKind::File, 5, "root needle"),
                        entry("éclair.unknown", None, EntryKind::File, 2, "unicode"),
                    ],
                },
                CtfDisk {
                    name: "Disk B".into(),
                    opaque_header: vec![],
                    entries: vec![
                        entry("Folder", None, EntryKind::Directory, 0, ""),
                        entry("film.mkv", Some(0), EntryKind::File, 4, "needle"),
                    ],
                },
                CtfDisk {
                    name: "Empty Disk".into(),
                    opaque_header: vec![],
                    entries: vec![entry("Empty", None, EntryKind::Directory, 0, "")],
                },
            ],
        })
        .unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn run_preview(path: &Path, generation: &str, request: Value) -> Result<Value> {
        let mut request = request;
        request["path"] = json!(path);
        request["generation"] = json!(generation);
        request["job_id"] = json!("ctf-preview-analytics-parity");
        let job = crate::ctf_job::Job::start(&request)?;
        analytics(&request, job)
    }

    #[test]
    fn direct_snapshot_analytics_matches_imported_sqlite_contracts() {
        let _serial = crate::ctf_preview::TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("analytics.ctf");
        fixture(&path);
        let before = fs::read(&path).unwrap();
        let opened = ctf_preview::open(&json!({"path":path})).unwrap();
        let generation = opened["generation"].as_str().unwrap();
        let mut catalog = atlas_core::Catalog::open(dir.path().join("analytics.sqlite")).unwrap();
        crate::legacy::import(&mut catalog, &json!({"path":path})).unwrap();

        let cases = [
            json!({"group_by":"volumes"}),
            json!({"group_by":"volumes","volume_id":2}),
            json!({"group_by":"volumes","folder_entry_id":3}),
            json!({"group_by":"extensions","highlight_query":"description:needle OR name:root"}),
            json!({"group_by":"categories","highlight_query":"path:Z AND type:video"}),
            json!({"group_by":"volumes","max_depth":1,"max_nodes":32}),
            json!({"group_by":"volumes","max_depth":32,"max_nodes":1}),
            json!({"group_by":"extensions","max_nodes":1}),
        ];
        for args in cases {
            let actual = run_preview(&path, generation, args.clone()).unwrap();
            let request: AnalyticsRequest = serde_json::from_value(args).unwrap();
            let expected =
                serde_json::to_value(catalog.analytics(&request, || false).unwrap()).unwrap();
            assert_eq!(actual["result"], expected, "request={request:?}");
        }

        let global = run_preview(&path, generation, json!({"group_by":"volumes"})).unwrap();
        assert_eq!(
            global["result"]["root"]["children"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(global["result"]["root"]["children"][2]["item_count"], 0);
        assert_eq!(global["result"]["by_volume"].as_array().unwrap().len(), 2);
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn direct_analytics_rejects_missing_category_and_obeys_precancel_before_source_access() {
        let _serial = crate::ctf_preview::TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("analytics-error.ctf");
        fixture(&path);
        let opened = ctf_preview::open(&json!({"path":path})).unwrap();
        let generation = opened["generation"].as_str().unwrap();
        let category = run_preview(
            &path,
            generation,
            json!({"group_by":"categories","category_id":9}),
        );
        assert_eq!(category.unwrap_err().code, "not_found");

        let request = json!({"path":dir.path().join("does-not-exist.ctf"),
            "generation":"stale","job_id":"ctf-preview-analytics-cancelled"});
        let job = crate::ctf_job::Job::start(&request).unwrap();
        crate::ctf_job::cancel(&json!({"job_id":"ctf-preview-analytics-cancelled"})).unwrap();
        assert_eq!(analytics(&request, job).unwrap_err().code, "cancelled");
    }
}
