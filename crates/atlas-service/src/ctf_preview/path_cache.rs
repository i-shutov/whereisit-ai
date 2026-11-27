//! Bounded, query-local memoization for repeated directory ancestry matches.
//!
//! Only directory components are cached. Callers still test the row's own name
//! and use their exact fallback whenever this cache declines a lookup.

use atlas_core::query::MetadataContainsMatcher;
use ctf_format::{EntryKind, MetadataDisk};

const WARMUP_LOOKUPS: usize = 1024;
const MAX_PATTERNS: usize = 4;
const STATE_BUDGET_BYTES: usize = 16 * 1024 * 1024;

const UNKNOWN: u8 = 0;
const NO_MATCH: u8 = 1;
const MATCH: u8 = 2;

struct PatternStates {
    needle: String,
    states: Vec<u8>,
}

/// Cache reusable only during one caller-owned query.
#[derive(Default)]
pub(super) struct AncestorCache {
    volume_index: Option<usize>,
    lookups: usize,
    state_bytes: usize,
    patterns: Vec<PatternStates>,
    unwind: Vec<usize>,
    #[cfg(test)]
    component_checks: usize,
}

impl AncestorCache {
    /// Return cached ancestry truth when eligible and warmed up, otherwise let
    /// the caller perform its existing exact matcher path.
    pub(super) fn ancestors_contain(
        &mut self,
        volume_index: usize,
        disk: &MetadataDisk,
        parent: Option<usize>,
        matcher: &MetadataContainsMatcher<'_>,
    ) -> Option<bool> {
        self.select_volume(volume_index);
        let needle = matcher.normalized_needle();
        if !needle.is_ascii() || needle.contains('/') {
            return None;
        }
        if parent.is_none() {
            return Some(false);
        }
        self.lookups = self.lookups.saturating_add(1);
        if self.lookups <= WARMUP_LOOKUPS {
            return None;
        }
        let parent = parent?;
        if parent >= disk.entries.len() {
            return None;
        }

        let pattern_index = match self.patterns.iter().position(|p| p.needle == needle) {
            Some(index) => index,
            None => self.add_pattern(needle, disk.entries.len())?,
        };

        self.resolve_parent(disk, parent, pattern_index, |name| matcher.matches(name))
    }

    fn select_volume(&mut self, volume_index: usize) {
        if self.volume_index != Some(volume_index) {
            self.volume_index = Some(volume_index);
            self.lookups = 0;
            self.state_bytes = 0;
            self.patterns.clear();
            self.unwind.clear();
        }
    }

    fn resolve_parent(
        &mut self,
        disk: &MetadataDisk,
        parent: usize,
        pattern_index: usize,
        mut matches: impl FnMut(&str) -> bool,
    ) -> Option<bool> {
        // Build the unresolved ancestor chain iteratively. Parent links in a
        // validated CTF point backwards; the cap also keeps malformed input
        // from growing scratch space without bound.
        self.unwind.clear();
        let mut cursor = Some(parent);
        let mut result = false;
        let mut depth = 0usize;
        while let Some(index) = cursor {
            if index >= disk.entries.len() || depth >= 4096 {
                self.unwind.clear();
                return None;
            }
            let entry = &disk.entries[index];
            if !matches!(entry.kind, EntryKind::Directory | EntryKind::Archive) {
                self.unwind.clear();
                return None;
            }
            let state = self.patterns[pattern_index].states[index];
            if state != UNKNOWN {
                result = state == MATCH;
                break;
            }

            #[cfg(test)]
            {
                self.component_checks += 1;
            }
            if matches(entry.name.as_str()) {
                self.unwind.try_reserve(1).ok()?;
                self.unwind.push(index);
                result = true;
                break;
            }
            self.unwind.try_reserve(1).ok()?;
            self.unwind.push(index);
            cursor = entry.parent;
            depth += 1;
        }

        // Propagate the resolved truth to every visited directory. If scratch
        // growth failed, do not claim a cached answer; the caller can fallback.
        for &index in &self.unwind {
            self.patterns[pattern_index].states[index] = if result { MATCH } else { NO_MATCH };
        }
        self.unwind.clear();
        Some(result)
    }

    fn add_pattern(&mut self, needle: &str, entry_count: usize) -> Option<usize> {
        if self.patterns.len() >= MAX_PATTERNS
            || entry_count > STATE_BUDGET_BYTES.saturating_sub(self.state_bytes)
        {
            return None;
        }

        let mut owned_needle = String::new();
        owned_needle.try_reserve_exact(needle.len()).ok()?;
        owned_needle.push_str(needle);

        let mut states = Vec::new();
        states.try_reserve_exact(entry_count).ok()?;
        states.resize(entry_count, UNKNOWN);

        self.patterns.try_reserve(1).ok()?;
        let index = self.patterns.len();
        self.patterns.push(PatternStates {
            needle: owned_needle,
            states,
        });
        self.state_bytes += entry_count;
        Some(index)
    }

    #[cfg(test)]
    fn cached_state_bytes(&self) -> usize {
        self.state_bytes
    }

    #[cfg(test)]
    fn cached_pattern_count(&self) -> usize {
        self.patterns.len()
    }

    #[cfg(test)]
    fn warmup_complete(&mut self, volume_index: usize) -> bool {
        self.select_volume(volume_index);
        self.lookups = self.lookups.saturating_add(1);
        self.lookups > WARMUP_LOOKUPS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_core::query::{
        MetadataCategoryPredicate, MetadataQuery, MetadataQueryContext, MetadataRecord,
    };
    use ctf_format::{EntryKind, MetadataDisk, MetadataEntry};
    use std::cell::RefCell;

    fn entry(name: &str, parent: Option<usize>, kind: EntryKind) -> MetadataEntry {
        MetadataEntry {
            name: name.to_owned(),
            description: String::new(),
            parent,
            kind,
            size: 0,
            attributes: 0,
            modified_dos: 0,
            created_dos: 0,
            accessed_dos: 0,
        }
    }

    fn disk(entries: Vec<MetadataEntry>) -> MetadataDisk {
        MetadataDisk {
            name: "synthetic".into(),
            opaque_header: vec![],
            entries,
        }
    }

    struct QueryContext<'a> {
        cache: RefCell<AncestorCache>,
        disk: &'a MetadataDisk,
        volume: usize,
        parent: RefCell<Option<usize>>,
        answers: RefCell<Vec<Option<bool>>>,
    }

    impl MetadataQueryContext for QueryContext<'_> {
        fn relative_path(&self) -> &str {
            ""
        }
        fn relative_path_contains(&self, matcher: &MetadataContainsMatcher<'_>) -> bool {
            let answer = self.cache.borrow_mut().ancestors_contain(
                self.volume,
                self.disk,
                *self.parent.borrow(),
                matcher,
            );
            self.answers.borrow_mut().push(answer);
            answer.unwrap_or(false)
        }
        fn is_branch_descendant(&self, _anchor_id: i64) -> bool {
            false
        }
        fn category_matches(&self, _predicate: &MetadataCategoryPredicate) -> bool {
            false
        }
    }

    fn record() -> MetadataRecord<'static> {
        MetadataRecord {
            id: 1,
            name: "leaf.bin",
            kind: "file",
            size: 1,
            extension: "bin",
            description: "",
            tags: "",
        }
    }

    fn run_path_query(query_text: &str, context: &QueryContext<'_>) -> bool {
        MetadataQuery::compile_metadata(query_text)
            .unwrap()
            .matches(&record(), context)
    }

    #[test]
    fn sibling_entries_reuse_directory_results_and_match_uncached_semantics() {
        let source = disk(vec![
            entry("RootNeedle", None, EntryKind::Directory),
            entry("child-a", Some(0), EntryKind::Directory),
            entry("a.txt", Some(1), EntryKind::File),
            entry("child-b", Some(0), EntryKind::Directory),
            entry("b.txt", Some(3), EntryKind::File),
        ]);
        let context = QueryContext {
            cache: RefCell::new(AncestorCache::default()),
            disk: &source,
            volume: 0,
            parent: RefCell::new(Some(1)),
            answers: RefCell::new(Vec::new()),
        };
        for _ in 0..WARMUP_LOOKUPS {
            assert!(!run_path_query("path:needle", &context));
        }
        assert_eq!(context.answers.borrow().last(), Some(&None));
        assert_eq!(context.cache.borrow().cached_pattern_count(), 0);
        assert_eq!(context.cache.borrow().unwind.capacity(), 0);

        assert!(run_path_query("path:needle", &context));
        assert_eq!(context.answers.borrow().last(), Some(&Some(true)));
        context.parent.replace(Some(3));
        assert!(run_path_query("path:needle", &context));
        assert_eq!(context.answers.borrow().last(), Some(&Some(true)));

        let cache = context.cache.borrow();
        assert_eq!(cache.patterns[0].states[0], MATCH);
        assert_eq!(cache.patterns[0].states[1], MATCH);
        assert_eq!(cache.patterns[0].states[3], MATCH);
        assert_eq!(cache.component_checks, 3);
    }

    #[test]
    fn query_hook_preserves_false_results_and_declines_unsupported_needles() {
        let source = disk(vec![
            entry("root", None, EntryKind::Directory),
            entry("child", Some(0), EntryKind::Directory),
        ]);
        let context = QueryContext {
            cache: RefCell::new(AncestorCache::default()),
            disk: &source,
            volume: 0,
            parent: RefCell::new(Some(1)),
            answers: RefCell::new(Vec::new()),
        };
        for _ in 0..WARMUP_LOOKUPS {
            let _ = run_path_query("path:absent", &context);
        }
        assert!(!run_path_query("path:absent", &context));
        assert_eq!(context.answers.borrow().last(), Some(&Some(false)));
        assert_eq!(context.cache.borrow().patterns[0].states[0], NO_MATCH);
        assert_eq!(context.cache.borrow().patterns[0].states[1], NO_MATCH);

        assert!(!run_path_query("path:root/child", &context));
        assert_eq!(context.answers.borrow().last(), Some(&None));
        assert!(!run_path_query("path:Σ", &context));
        assert_eq!(context.answers.borrow().last(), Some(&None));
    }

    #[test]
    fn cyclic_ancestry_declines_after_bounded_iterative_walk() {
        let source = disk(vec![
            entry("one", Some(1), EntryKind::Directory),
            entry("two", Some(0), EntryKind::Directory),
        ]);
        let context = QueryContext {
            cache: RefCell::new(AncestorCache::default()),
            disk: &source,
            volume: 0,
            parent: RefCell::new(Some(0)),
            answers: RefCell::new(Vec::new()),
        };
        for _ in 0..WARMUP_LOOKUPS {
            let _ = run_path_query("path:absent", &context);
        }
        assert!(!run_path_query("path:absent", &context));
        assert_eq!(context.answers.borrow().last(), Some(&None));
        assert!(context.cache.borrow().component_checks <= 4096);
    }

    #[test]
    fn cache_caps_patterns_budget_and_resets_on_volume_change() {
        let tiny = disk(vec![entry("root", None, EntryKind::Directory)]);
        let mut cache = AncestorCache::default();
        for pattern in 0..64 {
            let result = cache.add_pattern(&format!("term{pattern}"), tiny.entries.len());
            assert_eq!(result.is_some(), pattern < MAX_PATTERNS);
        }
        assert_eq!(cache.cached_pattern_count(), MAX_PATTERNS);
        assert_eq!(cache.cached_state_bytes(), MAX_PATTERNS);
        assert!(!cache.warmup_complete(1));
        assert_eq!(cache.cached_state_bytes(), 0);
        assert_eq!(cache.cached_pattern_count(), 0);
        assert!(cache
            .add_pattern("over-budget", STATE_BUDGET_BYTES + 1)
            .is_none());
        assert!(!cache.warmup_complete(2));
        assert_eq!(cache.cached_state_bytes(), 0);
        assert_eq!(cache.cached_pattern_count(), 0);
    }
}
