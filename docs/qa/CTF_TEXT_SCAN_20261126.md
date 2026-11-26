# Direct CTF text scans — 26 November 2026

Windows 1.1.1 hotfix work, stacked in PR #100 after #99. This report contains
synthetic test descriptions and aggregate measurements only. Published 1.1.0
artifacts remain immutable and `stable_ready=false`.

## Implementation and safety

Unqualified text queries already tested the basename before checking the path.
For an ASCII substring without a slash, the CTF context now skips that redundant
leaf test. Other contexts retain their default full-path behavior. Explicit
Path, slash/non-ASCII needles and newline field concatenation preserve the
existing semantics. All-ASCII inputs can reject a non-ASCII normalized needle
without Unicode decoding. A compiled required ASCII digit/punctuation byte can
also reject a candidate before case conversion. Newline cannot be that byte:
the evaluator inserts it between fields. An exhaustive Unicode scalar test
checks that lowercase cannot introduce such a byte; contextual sigma and
cross-field newline cases are covered separately.

A query-local ancestor cache reuses directory/archive matching results for
siblings. It starts after 1,024 ancestry lookups, keeps at most four normalized
needles, and limits state-vector payload to 16 MiB per worker. Volume changes
reset it; no cache survives a search. Scratch vectors and needle strings are
additional bounded allocations. Unsupported conditions or allocation failure
fall back to the existing exact matcher.

Catalog-order search evaluates its first 4,096 rows sequentially, preserving
early return for short pages. Longer scans use up to four scoped workers and
65,536-row portions. Workers own separate caches. The caller joins every worker
before merging in source order or publishing a result; errors and panics are
converted to errors after all launched workers are joined. No global name sort
or persistent search database is built. Read-ahead is limited to one batch:
four portions and approximately 2 MiB of matching-row payload, plus up to 64 MiB
of cache state. Cancellation is checked per row; workers publish progress every
256 rows and flush completed portions. During cancellation the visible counter
can lag by at most 255 rows per worker.

Source selection and final integrity verification remain outside the worker
scope. Workers borrow the immutable snapshot, and source pin/fallback rules are
unchanged. Export, analytics and legacy name-order matching retain their prior
execution paths.

## Validation

Core/service suites, 16 core unit tests, 98 service unit tests, 103 desktop tests,
strict all-target Clippy and formatting passed for the code progression through
`24d7e77`. The final matcher revision also passed the complete scalar invariant.
Regressions cover newline/Unicode matching, short-circuit dispatch, cache
warmup/limits/reset/fallback, bounded cyclic-input handling, partial warmup
boundaries, empty/unknown volumes, and paging/full counts across 1/2/4 workers.
The worker matrix compares actual parsed synthetic records with uncached
evaluation across two volumes, including exact completed progress counts.

## Controlled full-catalog measurements

Same read-only copy: 44 volumes, 3,046,133 records. Windows 11 x64, four logical
processors, about 4 GiB host memory, optimized Rust executor, catalog order, 30 samples
per class. Runs were sequential, without concurrent build or GUI automation.
OS cache was uncontrolled; these are not cold-disk-cache results. The harness
checks source bytes before/after each run. Per-request physical disk I/O was not
captured; process memory counters are reported separately.

| Query class | Basename/rejection p95 ms (`97525f8`) | Cache p95 ms (`6f61434`) | Parallel p50 / p95 / max ms (`b141986`) |
|---|---:|---:|---:|
| Empty | 1.130 | 1.478 | 0.898 / 1.758 / 2.315 |
| Extension | 35.695 | 36.400 | 13.764 / 21.703 / 25.373 |
| Ordinary text | 154.841 | 65.526 | 26.897 / 30.442 / 31.342 |
| Boolean | 12.181 | 12.674 | 10.484 / 14.984 / 16.919 |
| Unicode name | 192.873 | 218.359 | 85.769 / 103.607 / 103.855 |
| Absent name | 234.285 | 215.982 | 96.197 / 110.029 / 110.227 |
| Absent ordinary text | 793.783 | 407.532 | 166.232 / 177.469 / 179.939 |
| Absent path | 735.810 | 311.480 | 130.582 / 152.162 / 183.522 |
| Absent description | 81.017 | 86.242 | 45.505 / 62.014 / 69.994 |
| Extension/size full count | 214.275 | 216.623 | 86.587 / 92.846 / 96.504 |

At `b141986`, eight classes met 150 ms p95; absent text/path still failed.
The final required-byte refinement at `24d7e77` was then measured twice:

| Query class | Run 1 p50 / p95 / max ms | Run 2 p50 / p95 / max ms |
|---|---:|---:|
| Empty | 0.995 / 1.363 / 2.125 | 0.887 / 1.784 / 1.882 |
| Extension | 14.095 / 16.420 / 18.343 | 13.249 / 15.607 / 16.567 |
| Ordinary text | 26.773 / 29.581 / 31.047 | 26.763 / 28.485 / 29.087 |
| Boolean | 8.626 / 10.018 / 11.800 | 8.622 / 12.828 / 12.880 |
| Unicode name | 78.984 / 94.762 / 98.281 | 78.728 / 84.953 / 85.228 |
| Absent name | 71.047 / 79.254 / 84.468 | 70.188 / 81.387 / 81.492 |
| Absent ordinary text | 113.380 / 122.108 / 126.938 | 111.012 / 122.112 / 124.524 |
| Absent path | 101.448 / 106.045 / 109.646 | 99.115 / 104.909 / 108.912 |
| Absent description | 24.168 / 28.068 / 28.304 | 23.086 / 24.830 / 25.525 |
| Extension/size full count | 84.070 / 90.458 / 90.635 | 84.880 / 93.112 / 94.303 |

**150 ms p95 for these ten sampled classes: PASS in both runs** (600 successful
requests total). The largest observed p95 was 122.112 ms; largest single request
126.938 ms. This is not a universal bound for arbitrary query complexity or
other hardware. Open through first search was 1,273 / 1,217 ms; opening itself
1,244 / 1,188 ms. These are single-run observations, not cold-start percentiles.

Parallel-run peak working set was 904,404,992 bytes; peak commit was
1,133,473,792 bytes. First-search private bytes were 771,268,608, end private
bytes 774,733,824, and after-close private bytes 9,785,344. The opening peak
has not been attributed to individual parsing/compaction/index-building phases;
this change does not establish a reduction of that peak or a long-duration
leak result. In the final two runs, peak working set was 904,663,040 /
904,081,408 bytes and peak commit 1,133,379,584 / 1,133,051,904 bytes. End private
bytes were 773,771,264 / 774,918,144; after close 9,404,416 / 10,088,448.
Original, master, full working copy and synthetic fixture matched
their private baselines after these runs.

## Remaining release gates

Exact-head CI and actual native GUI evidence are tracked in
[PR #100](https://github.com/i-shutov/whereisit-ai/pull/100). A disconnected RDP
session prevented the first final-GUI capture; text-tree reads alone are not
claimed as interactive GUI acceptance. Installed 1.1.1 lifecycle/upgrade and
original WhereIsIt compatibility remain separate release checks. Service timings and
synthetic CI do not replace those checks or certify a new Windows 10 artifact.
