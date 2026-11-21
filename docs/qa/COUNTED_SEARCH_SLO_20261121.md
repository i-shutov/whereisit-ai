# Counted-search SLO regression — 2026-11-21

P1 was observed on the installed hosted RC4 candidate `9058728f2bdea8076a9ac31268a30d1d090eab02`, native run 44958630693. On the same 44-volume / 3,046,133-entry derivative catalog, the global full-count workload measured p50/p95 148.57/155.23 ms over 30 warm requests, exceeding the 150 ms p95 target. The other five workload classes passed; all 180 responses were valid. Green CI did not override this measured FAIL.

The count already used a covering extension index, but still evaluated the negated predicate for each index row. For a standalone, unscoped negated extension equality, the fix subtracts the indexed equality count from the exact unfiltered row count. Both scalar counts execute in one SQLite statement snapshot. It never substitutes cached volume totals. Compound, scoped and other predicates keep their existing logic; the parser identifies the eligible expression structurally, including parentheses and normalized Unicode extensions. All supported schemas declare the extension column NOT NULL.

Synthetic regressions cover schemas v1/v2/v3, incorrect cached volume totals, Unicode/case/leading-dot normalization, empty extensions, category hierarchy, compound Boolean expressions, scope, pagination and the b-tree Count/index-seek plan. All 35 atlas-core tests and strict crate Clippy passed. Independent code review found no actionable P0/P1 in the change. The pre-existing separation between the total and page statements under concurrent writes is unchanged.

The rebuilt local standard RC4 CLI completed one new 30-warm-request series per class:

| Workload | p50 ms | p95 ms |
|---|---:|---:|
| Global Boolean | 38.47 | 42.47 |
| Unicode | 17.37 | 22.07 |
| Volume | 12.54 | 15.72 |
| Size/extension | 36.16 | 43.56 |
| Folder | 3.36 | 7.31 |
| Global with count | 54.93 | 62.72 |

All 180 requests passed response checks with stable ordering; the full count remained 3,041,007. Peak CLI working set was 9.02 MiB. SQLite 3.54.1; page limit 250; latency includes CLI JSON round trips and SQLite opens. OS cache was uncontrolled; these are not GUI-rendering or controlled cold-cache measurements. The working SQLite size/write timestamp stayed unchanged; no source CTF was opened by the benchmark. All paths, queries, raw results, screenshots and private source digests remain local.

Status: local fix and measured workload PASS; exact-head hosted CI and installed fixed-artifact acceptance remain required. Stable readiness remains false for the separate signing, platform/physical USB and independent sign-off gates.
