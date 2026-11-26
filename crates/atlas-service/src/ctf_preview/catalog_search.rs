//! Bounded source-order catalog search. Workers own their query-local ancestry
//! caches; results are joined and merged in catalog order before publication.

use super::{
    matches_metadata_with_cache, matches_row_with_cache, path_cache::AncestorCache, RowRef,
    Snapshot,
};
use crate::ctf_job::Job;
use atlas_core::{query::MetadataQuery, Error, Result};
use std::{cell::RefCell, thread};

const MAX_CHUNK_ROWS: usize = 65_536;
const MAX_WORKERS: usize = 4;
const WARMUP_ROWS: usize = 4_096;

pub(super) struct SearchOptions {
    pub offset: u64,
    pub limit: u64,
    pub full_count: bool,
}

pub(super) struct Settings {
    pub warmup_rows: usize,
    pub chunk_rows: usize,
    pub workers: usize,
}

#[derive(Clone, Copy)]
struct Span {
    volume_index: usize,
    start: usize,
    end: usize,
}

struct Cursor {
    volume_index: usize,
    entry_index: usize,
    last_volume: Option<usize>,
}

impl Cursor {
    fn new(volume_id: Option<i64>) -> Self {
        let selected =
            volume_id.map(|id| usize::try_from(id.saturating_sub(1)).unwrap_or(usize::MAX));
        Self {
            volume_index: selected.unwrap_or(0),
            entry_index: 0,
            last_volume: selected,
        }
    }

    fn next_span(&mut self, snapshot: &Snapshot, chunk_rows: usize) -> Option<Span> {
        while self.volume_index < snapshot.parsed.disks.len()
            && self
                .last_volume
                .is_none_or(|last| self.volume_index <= last)
        {
            let len = snapshot.parsed.disks[self.volume_index].entries.len();
            if self.entry_index >= len {
                self.volume_index += 1;
                self.entry_index = 0;
                continue;
            }
            let start = self.entry_index;
            let end = start.saturating_add(chunk_rows).min(len);
            self.entry_index = end;
            return Some(Span {
                volume_index: self.volume_index,
                start,
                end,
            });
        }
        None
    }
}

fn invalid_settings() -> Error {
    Error::new("invalid_request", "Invalid catalog search settings")
}

fn worker_error() -> Error {
    Error::new("internal", "Catalog search worker failed")
}

fn row(volume_index: usize, entry_index: usize) -> RowRef {
    RowRef {
        volume_index: volume_index as u32,
        entry_index: entry_index as u32,
    }
}

fn record_match(
    snapshot: &Snapshot,
    row: RowRef,
    query: &MetadataQuery,
    needs_extension: bool,
    volume_id: Option<i64>,
    job: &Job,
    cache: Option<&RefCell<AncestorCache>>,
) -> Result<bool> {
    matches_row_with_cache(snapshot, row, query, needs_extension, volume_id, job, cache)
}

fn collect_span(
    snapshot: &Snapshot,
    span: Span,
    query: &MetadataQuery,
    needs_extension: bool,
    job: &Job,
    cache: &RefCell<AncestorCache>,
) -> Result<Vec<RowRef>> {
    let mut matches = Vec::new();
    let mut processed = 0u64;
    for entry_index in span.start..span.end {
        job.check()?;
        if matches_metadata_with_cache(
            snapshot,
            span.volume_index,
            entry_index,
            query,
            needs_extension,
            Some(cache),
        ) {
            matches.push(row(span.volume_index, entry_index));
        }
        processed += 1;
        if processed == 256 {
            job.entries(processed)?;
            processed = 0;
        }
    }
    if processed > 0 {
        job.entries(processed)?;
    }
    Ok(matches)
}

fn absorb(
    matches: impl IntoIterator<Item = RowRef>,
    offset: u64,
    limit: u64,
    count: &mut u64,
    page: &mut Vec<RowRef>,
) {
    let end = offset.saturating_add(limit);
    for matched in matches {
        let ordinal = *count;
        *count = count.saturating_add(1);
        if ordinal >= offset && ordinal < end {
            page.push(matched);
        }
    }
}

pub(super) fn run(
    snapshot: &Snapshot,
    query: &MetadataQuery,
    needs_extension: bool,
    volume_id: Option<i64>,
    job: &Job,
    options: SearchOptions,
) -> Result<(u64, Vec<RowRef>)> {
    let workers = thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .clamp(1, MAX_WORKERS);
    run_configured(
        snapshot,
        query,
        needs_extension,
        volume_id,
        job,
        options,
        Settings {
            warmup_rows: WARMUP_ROWS,
            chunk_rows: MAX_CHUNK_ROWS,
            workers,
        },
    )
}

#[cfg(test)]
pub(super) fn run_with_settings(
    snapshot: &Snapshot,
    query: &MetadataQuery,
    needs_extension: bool,
    volume_id: Option<i64>,
    job: &Job,
    options: SearchOptions,
    settings: Settings,
) -> Result<(u64, Vec<RowRef>)> {
    run_configured(
        snapshot,
        query,
        needs_extension,
        volume_id,
        job,
        options,
        settings,
    )
}

fn run_configured(
    snapshot: &Snapshot,
    query: &MetadataQuery,
    needs_extension: bool,
    volume_id: Option<i64>,
    job: &Job,
    options: SearchOptions,
    settings: Settings,
) -> Result<(u64, Vec<RowRef>)> {
    let Settings {
        warmup_rows,
        chunk_rows,
        workers,
    } = settings;
    if options.limit == 0
        || warmup_rows == 0
        || chunk_rows == 0
        || chunk_rows > MAX_CHUNK_ROWS
        || !(1..=MAX_WORKERS).contains(&workers)
        || volume_id.is_some_and(|id| id <= 0)
    {
        return Err(invalid_settings());
    }
    job.check()?;
    let offset = options.offset.min(i64::MAX as u64);
    let limit = options.limit;
    let mut page = Vec::with_capacity(usize::try_from(limit.min(1000)).unwrap_or(1000));
    let mut count = 0u64;
    let mut cursor = Cursor::new(volume_id);
    let mut warmed = 0usize;
    let warm_cache = RefCell::new(AncestorCache::default());

    // Preserve source order while warming one cache over the query's first rows.
    while warmed < warmup_rows {
        let Some(span) = cursor.next_span(snapshot, chunk_rows.min(warmup_rows - warmed)) else {
            break;
        };
        for entry_index in span.start..span.end {
            job.check()?;
            if record_match(
                snapshot,
                row(span.volume_index, entry_index),
                query,
                needs_extension,
                volume_id,
                job,
                Some(&warm_cache),
            )? {
                absorb(
                    [row(span.volume_index, entry_index)],
                    offset,
                    limit,
                    &mut count,
                    &mut page,
                );
            }
            warmed += 1;
            if !options.full_count && count > offset.saturating_add(limit) {
                return Ok((count, page));
            }
            if warmed == warmup_rows {
                break;
            }
        }
    }
    if workers == 1 {
        let cache = warm_cache;
        while let Some(span) = cursor.next_span(snapshot, chunk_rows) {
            let matches = collect_span(snapshot, span, query, needs_extension, job, &cache)?;
            absorb(matches, offset, limit, &mut count, &mut page);
            if !options.full_count && count > offset.saturating_add(limit) {
                break;
            }
        }
        job.check()?;
        return Ok((count, page));
    }

    let mut worker_caches = Vec::with_capacity(workers);
    worker_caches.push(warm_cache);
    worker_caches.extend((1..workers).map(|_| RefCell::new(AncestorCache::default())));
    thread::scope(|scope| {
        loop {
            job.check()?;
            let mut batch = Vec::with_capacity(workers);
            let mut spawn_failed = false;
            for (lane, lane_cache) in worker_caches.iter_mut().enumerate() {
                let Some(span) = cursor.next_span(snapshot, chunk_rows) else {
                    break;
                };
                let cache = std::mem::replace(lane_cache, RefCell::new(AncestorCache::default()));
                let handle = thread::Builder::new()
                    .name("ctf-catalog-search".into())
                    .spawn_scoped(scope, move || {
                        let result =
                            collect_span(snapshot, span, query, needs_extension, job, &cache);
                        (result, cache)
                    });
                let handle = match handle {
                    Ok(handle) => handle,
                    Err(_) => {
                        spawn_failed = true;
                        break;
                    }
                };
                batch.push((lane, handle));
            }
            if batch.is_empty() {
                if spawn_failed {
                    return Err(worker_error());
                }
                break;
            }
            let mut stop = false;
            let mut first_error = None;
            for (lane, handle) in batch {
                match handle.join() {
                    Ok((result, cache)) => {
                        worker_caches[lane] = cache;
                        match result {
                            Ok(matches) => {
                                absorb(matches, offset, limit, &mut count, &mut page);
                                if !options.full_count && count > offset.saturating_add(limit) {
                                    stop = true;
                                }
                            }
                            Err(error) => {
                                if first_error.is_none() {
                                    first_error = Some(error);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        if first_error.is_none() {
                            first_error = Some(worker_error());
                        }
                    }
                }
            }
            if let Some(error) = first_error {
                return Err(error);
            }
            if spawn_failed {
                return Err(worker_error());
            }
            if stop {
                break;
            }
        }
        job.check()?;
        Ok((count, page))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind};
    use std::{path::PathBuf, sync::Arc};

    fn snapshot(lengths: &[usize]) -> Snapshot {
        let disks = lengths
            .iter()
            .enumerate()
            .map(|(disk_index, len)| CtfDisk {
                name: format!("disk-{disk_index}"),
                opaque_header: Vec::new(),
                entries: (0..*len)
                    .map(|entry_index| CtfEntry {
                        name: format!("item-{entry_index}"),
                        description: String::new(),
                        parent: None,
                        kind: EntryKind::File,
                        size: 0,
                        attributes: 0,
                        modified_dos: 0,
                        created_dos: 0,
                        accessed_dos: 0,
                        opaque: Vec::new(),
                    })
                    .collect(),
            })
            .collect();
        super::super::build_snapshot(
            PathBuf::new(),
            PathBuf::new(),
            Arc::new(CtfCatalog {
                name: "synthetic".into(),
                disks,
                opaque_trailer: Vec::new(),
            }),
            "generation".into(),
            "sha256".into(),
            0,
        )
        .unwrap()
    }

    #[test]
    fn cursor_keeps_partial_warmup_and_chunk_boundaries_in_source_order() {
        let snapshot = snapshot(&[9, 0, 3]);
        let mut cursor = Cursor::new(None);
        assert_eq!(
            cursor
                .next_span(&snapshot, 3)
                .map(|s| (s.volume_index, s.start, s.end)),
            Some((0, 0, 3))
        );
        assert_eq!(
            cursor
                .next_span(&snapshot, 4)
                .map(|s| (s.volume_index, s.start, s.end)),
            Some((0, 3, 7))
        );
        assert_eq!(
            cursor
                .next_span(&snapshot, 4)
                .map(|s| (s.volume_index, s.start, s.end)),
            Some((0, 7, 9))
        );
        assert_eq!(
            cursor
                .next_span(&snapshot, 4)
                .map(|s| (s.volume_index, s.start, s.end)),
            Some((2, 0, 3))
        );
        assert!(cursor.next_span(&snapshot, 4).is_none());
    }

    #[test]
    fn cursor_scopes_a_volume_and_empty_or_unknown_volumes_yield_no_rows() {
        let snapshot = snapshot(&[2, 0, 3]);
        let mut selected = Cursor::new(Some(3));
        assert_eq!(
            selected
                .next_span(&snapshot, 2)
                .map(|s| (s.volume_index, s.start, s.end)),
            Some((2, 0, 2))
        );
        assert_eq!(
            selected
                .next_span(&snapshot, 2)
                .map(|s| (s.volume_index, s.start, s.end)),
            Some((2, 2, 3))
        );
        assert!(selected.next_span(&snapshot, 2).is_none());

        assert!(Cursor::new(Some(2)).next_span(&snapshot, 2).is_none());
        assert!(Cursor::new(Some(99)).next_span(&snapshot, 2).is_none());
    }
}
