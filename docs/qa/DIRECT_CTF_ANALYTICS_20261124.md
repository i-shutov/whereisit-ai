# Direct CTF analytics checkpoint

24 November 2026. Development branch, not installed-release acceptance.

## Implementation

Read-only CTF volume maps and slices use the existing immutable metadata snapshot.
Numeric per-entry counters and a reverse parent pass compute subtree totals once,
rather than issuing recursive SQLite queries for each displayed node. Only the
selected disks allocate counters; child-page selection uses bounded heaps with
SQLite-compatible ASCII NOCASE/name and stable-ID ordering. Output retains the
existing analytics API's scopes, grouped slices, breadcrumbs, truncation/remainder
accounting and logical-file rules. Archive containers are not counted as files.
Empty highlight text produces zero highlighted counters.

This aggregate engine is Rust, independently of the optional DuckDB search
executor. No SQLite mirror, FTS build, persistent aggregate database, Python or
Parquet input is needed. Explicit native conversion remains separate. Original
sources remain read-only; generation/source checks and cooperative cancellation
guard publication. A single-use prepared job owns the request and cancellation
slot before a worker starts; dropping an abandoned request releases the slot.
Native replies are pinned to source path/generation and request identity.
Cancellation retains the job's originating backend across catalog switches.

## Tests and full-catalog measurements

The core analytics suite passed 3 tests, service unit tests passed 70, and desktop
unit/workflow tests passed 94 on the final catalog-scope source. Coverage includes complete synthetic JSON equality
with imported SQLite for scopes/groups/highlights/truncation, empty-volume
handling, source preservation, pre-dispatch cancellation, slot ownership and
stale native replies. Workspace/all-target release-mode Clippy with warnings
denied passed. Rust and Cargo 1.100.0; the optional search feature links DuckDB
1.4.1. Formatting passed.

After adding workspace-aware Escape cancellation, both Escape regression tests
and the desktop suite passed, including the new direct-CTF case.
Formatting and workspace/all-target release Clippy were repeated successfully
after that change.

A final scope review found that catalog switches retained numeric map/slices
volume, folder and category selections. Those selections now reset with the
catalog, preventing IDs from the previous source selecting unrelated records.
The new regression and complete 94-test desktop suite passed.

A sequential release-mode benchmark used a full working CTF copy with 44 volumes
and 3,046,133 records. Each case compared the complete response with a read-only
SQLite reference, then collected five direct-service samples. All comparisons
and source-byte checks passed. Requests used the native UI limits: depth 4 and
128 tree nodes. Five-sample nearest-rank p95 is the maximum observation.

| Request | p50 ms | p95 ms |
|---|---:|---:|
| Global volume tree | 630.246 | 656.637 |
| Global extensions, extension highlight | 745.615 | 783.614 |
| Global categories | 589.463 | 647.299 |
| Largest volume tree | 318.162 | 339.649 |

Windows process peak working set was 1,032,265,728 bytes; final working set was
768,188,416 bytes. These values include parsed CTF metadata and the reference
connection, not just aggregate counters. Timings include source verification and
service serialization, exclude initial opening and SQLite reference preparation,
and have uncontrolled OS caches. Total harness runtime was 219.86 seconds, much
of it preparing the SQLite reference responses outside the timed samples. These
observations do not certify Windows 10, disk-cold behavior or an installed product.

## Related defect and remaining gates

The new parity test exposed [P1-ANALYTICS-TEXT-HIGHLIGHT](ANALYTICS_HIGHLIGHT_DEFECT_20261124.md)
in the existing SQLite path. Highlight counters now use the same table-selection
logic as normal queries, preserving metadata fields, category correlations and
bound parameters. Cheap non-highlight aggregates retain their compact path.
Synthetic description/tag/default-text, branch/category and group regressions
passed. The published 1.1.0 installer remains affected until a hotfix is accepted.

## Observed native GUI

An isolated Windows development executable opened the full working CTF copy.
Physical interaction verified volume maps, extension highlights and grouping,
slices, preserved workspace context, volume/folder drill-down and opening the
same scope in search. The map's scoped file count matched the search result.
The Cancel button stopped a deliberately long synthetic highlight expression;
a subsequent extension highlight completed. An initial Escape routing defect
was fixed, rebuilt and retested: Escape displayed the cancellation result and
the next calculation completed normally.

Opening a generated SQLite catalog through the native catalog menu verified
description, tag and default-text highlights: respectively 1 file/17 bytes,
1 file/31 bytes and 2 files/48 bytes. No SQL error occurred. Original and working
CTF byte hashes still matched the frozen baseline after closing the application.
Private evidence records the tested executable identities; no raw evidence is
published. These checks do not certify the installed 1.1.0 executable.

Exact candidate CI and installed/offline package acceptance for this follow-up
remain separate gates. Keep `stable_ready=false`.
No private paths, catalog rows, source hashes, licenses or screenshots are in this
report. Published release artifacts are unchanged.

## Independent code review limitations

Two focused reviewers found no new P0/P1 in the direct analytics change. They
identified two P2 robustness issues for follow-up. `max_nodes` bounds the rendered
tree, while the API still returns every distinct extension slice; pathological
extension cardinality can enlarge the response and delay cancellation during
slice materialization/sorting. The final source check observes cancellation,
but read-only response serialization has no atomic publication claim: a late
cancel acknowledgment can overlap delivery of an already completed result.
Neither issue changes source bytes. These limits are not certified as resolved
by the ordinary-catalog measurements or native GUI cancellation above.
