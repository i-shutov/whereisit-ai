# CTF catalog-order first pages — 26 November 2026

This checkpoint follows the owner's disk → folder → file navigation direction.
It is part of the Windows 1.1.1 hotfix work, not an installed stable release.
Published 1.1.0 assets are immutable and `stable_ready` remains false.

## Change and compatibility

The old fast-search path built and sorted a name index for the entire snapshot
before returning its first page. The PR58 prototype now requests catalog order
and preserves that order in the displayed page. After an actual prototype
interaction, the same request was added to the native CTF preview adapter.
The service traverses disks and their source records, retains only the requested
page, and stops after one additional matching record. Disk IDs, recorded paths,
folder reveal and Back retain their existing meaning. No new global sort UI was
introduced. API callers omitting `order`, or explicitly using `name`, retain
legacy ordering. An explicit DuckDB/catalog-order combination is rejected;
catalog-order requests select Rust by default even in feature builds.

ASCII substring terms without a slash can match borrowed path components.
They no longer require joining a complete path for every visited record.
Separator-containing and non-ASCII needles use the existing complete-path
matcher. Unicode lowercase semantics and newline text queries are preserved.
The source protection and portable full-hash fallback from #98 are unchanged.

## Synthetic validation

- Service unit tests: **91 PASS**. New tests cover catalog pages, full counts,
  volume scope, recorded context, invalid options, cancellation and no name index.
- A test-only progress observer proves that a one-row page visits two records,
  offset one visits three, and a full count visits all five fixture records.
- Core and service integration suites: **PASS**, including source protection,
  cancellation, editing, export and query semantics.
- Desktop tests: **103 PASS**, including explicit catalog-order routing.
- Strict Clippy for core/service/desktop all targets and Rust formatting: **PASS**.
- Prototype: lint, **22 tests**, production build: **PASS**.

## Actual interactive observations

Prototype: searched synthetic metadata and used **В папку**. The resulting
breadcrumbs identify the selected disk and exact parent folder and show the
selected file. The prototype received no real CTF or SQLite data.

Native release QA binary built from `dbff8f8`: opened the generated 60-disk,
18,886-record fixture; searched `ext:mkv`; checked pages 1–250 and 251–500;
selected a file on page two; used **Открыть папку файла**; returned with Back to
the same result page. Disk and Unicode recorded Path were present. An Enter key
after accessibility selection did not reveal the folder in this run; the explicit
button did. This is not a fresh physical double-click or Enter acceptance claim.
The QA binary is isolated from the installed user's profile. These observations
do not certify an installer, upgrade lifecycle or Windows 10 performance.

On the protected full-catalog copy, the same binary opened all 44 disks and
3,046,133 records, returned pages 1–250 and 251–500, and cancelled an actively
running 64-clause synthetic path query with Esc. Repeating the identical query
completed with zero results, observed by 66.196 seconds; this is a cancellation
recovery check, not a query-latency measurement. Normal window close completed.
The read-only working copy rejected a write-open probe (access denied; no bytes
written). This is not separate evidence of the sharing-conflict error tested by
the source-pin synthetic suite. All four source/fixture baseline checks passed
again after the full GUI block.

## Measurement status

Controlled sequential service measurements of legacy name order and catalog
order use one optimized binary, the same protected full-catalog copy, Rust,
30 warm samples per query class, and no concurrent build or GUI interaction.
The harness verifies source bytes, creates no persistent search cache, and
records working set and private bytes at open, first search, end and close.
OS cache is uncontrolled; this is not a cold-disk-cache benchmark.

Both runs completed successfully, with 3,046,133 entries across 44 volumes.
Open was 1,505 ms with name order and 1,452 ms with catalog order. Open through
first search was **4,309 ms → 1,645 ms** (single observations, not percentiles).

| Query class | Name p50 / p95 ms | Catalog p50 / p95 ms |
|---|---:|---:|
| Empty | 1.001 / 1.532 | 0.863 / 1.492 |
| Extension | 12.570 / 15.466 | 37.143 / 43.799 |
| Ordinary text | 33.766 / 38.492 | 184.300 / 234.360 |
| Boolean | 1.046 / 2.287 | 11.765 / 14.297 |
| Unicode name | 927.711 / 1120.028 | 309.489 / 328.929 |
| Absent name | 660.626 / 797.785 | 224.137 / 303.202 |
| Absent ordinary text | 2204.860 / 2536.747 | 1057.236 / 1248.765 |
| Absent path | 1727.828 / 2002.082 | 812.687 / 898.913 |
| Absent description | 338.843 / 509.289 | 90.569 / 95.913 |
| Extension/size with full count | 234.148 / 251.810 | 236.554 / 270.232 |

Changing order changes which records a fast query visits before filling its
page. Several common classes were slower in catalog order even though the
first-use sort disappeared. These sequential measurements also have uncontrolled
cache and CPU effects; they are observations, not universal speedup factors.
**150 ms p95 SLO: FAIL** for ordinary text, Unicode, absent name/text/path and
full-count queries. P1-CTF-COLD-OPEN remains open.

| Process memory phase | Name private bytes | Catalog private bytes |
|---|---:|---:|
| Open | 769,220,608 | 768,815,104 |
| First search | 794,120,192 | 769,597,440 |
| End of query series | 794,509,312 | 769,814,528 |
| After close | 10,027,008 | 9,113,600 |

Peak working set was effectively unchanged: **904,450,048 → 904,482,816 bytes**.
Peak commit was **1,133,531,136 → 1,133,318,144 bytes**. Avoiding the name index
accounts for roughly 24.5 MB less current private memory after first search;
it does not reduce the earlier parsing peak. Closing released the snapshot:
working set returned to 11,685,888 / 10,948,608 bytes. This run does not show a
snapshot retained after close; it does not constitute a long-duration leak test.

Original, frozen master, full working copy and generated GUI fixture matched
their private baselines after the benchmark/synthetic-GUI block. Final exact-SHA
CI is tracked in [PR #99](https://github.com/i-shutov/whereisit-ai/pull/99).
No private names, source paths, source hashes, query contents or screenshots
are included in this report.
