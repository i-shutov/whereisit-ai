# CTF hotfix progress — 26 November 2026

Status: **in progress; P1-CTF-COLD-OPEN remains OPEN; `stable_ready=false`.**
This checkpoint records exact-head and GUI evidence observed on 26 November. It
does not certify a hotfix installer or close the cold-open defect.

## Exact-head CI and catalog navigation

On exact source commit `aa8b37d`, the required PR CI completed successfully.
The Windows GUI cold-tree retest also passed on that exact build:

- With a generated fixture of 60 disks and 18,886 entries, a deep folder on
  disk 30 beyond 250 siblings became visible after lazy sibling loading settled.
  This verifies the corrected reveal/scroll behavior for that case.
- A real double-click on a selected folder entered it successfully.

These results close the previously pending cold-tree GUI retest for the stated
synthetic cases. They do not establish CTF first-open performance or installed
artifact acceptance.

## Full read-only working-copy GUI observation

The native GUI opened a protected read-only working copy containing 44 volumes
and 3,046,133 records. A bounded synthetic absent-path query of 2,044 characters
and 127 tokens was submitted. The GUI showed the search still active at progress
347,356; pressing Esc then produced the visible status “Поиск в CTF отменён”.
This is observed native GUI cancellation evidence for this run. Retrying the
same query showed fresh progress and then completed normally with zero results;
the cancel control disappeared and the catalog retained its read-only status.
The retry was still active at the 63-second observation and was complete at the
92-second observation. These are coarse GUI observation bounds for a deliberate
64-clause stress query, not a precise latency benchmark or ordinary-query SLA.

After the GUI and benchmark runs, the original, frozen copy and navigation copy
all matched the private source SHA-256 baseline. No private catalog paths,
record contents, hashes or screenshots are included.

## Full-catalog direct Rust measurements

Source code: `aa8b37d` plus documentation-only checkpoint `a2c6368`.
Windows 11 Pro x64 build 22631, four logical processors, Rust/Cargo 1.100.0,
release mode, default Rust executor without the optional DuckDB feature.
The existing ignored `direct_ctf_first_search_and_query_latency` harness ran
serially with GUI closed, 30 repetitions per class, on the protected full copy.
OS cache was uncontrolled; source hashing before open warms file data. These
are service timings, not OS disk-cold measurements or application startup/render
measurements. A fresh SQLite-reference comparison was not configured for this
run; earlier semantic-parity evidence remains separate.

One first-open sample measured **2,392 ms to opened snapshot** and **5,545 ms
through the first ordinary text search**, reporting 44 volumes / 3,046,133
records. Repeated query results use nearest-rank p50/p95, in milliseconds:

| Query class | Samples | p50 ms | p95 ms |
|---|---:|---:|---:|
| Empty global diagnostic (not catalog startup) | 30 | 188.297 | 200.823 |
| Extension | 30 | 197.081 | 204.230 |
| Ordinary text | 30 | 222.952 | 275.485 |
| Boolean | 30 | 182.910 | 196.013 |
| Unicode name | 30 | 1073.935 | 1248.425 |
| Absent name | 30 | 830.808 | 1025.425 |
| Absent ordinary text | 30 | 2999.794 | 3148.377 |
| Absent path | 30 | 2736.702 | 2925.088 |
| Absent description | 30 | 472.821 | 499.628 |
| Extension/size with complete count | 30 | 407.861 | 433.935 |

Process peak working set was **769,720,320 bytes (734.06 MiB)**; working set at
the end was **680,427,520 bytes (648.91 MiB)**. These are process working-set
counters, not allocator-only usage, private bytes or GUI memory. The harness
completed all requests successfully, checked unchanged source bytes and verified
that direct opening created no SQLite cache. The run took 287.99 seconds,
excluding compilation. Raw local logs remain private.

This improves access substantially over the released conversion-dependent path,
but does not establish vendor-equivalent latency or satisfy the historical
150 ms search target. Full-scan text/path latency and installed first-use
acceptance remain open. No global filename-sort optimization was added.

## Remaining release gates

| Gate | Status |
|---|---|
| P1-CTF-COLD-OPEN: acceptable first open and first search on the full catalog | **OPEN** |
| Current direct-snapshot service benchmark with repeated latency and memory measurements | **EXECUTED**; 300 successful requests, timings above; performance gate remains open |
| Same-query retry after the observed GUI cancellation | **PASS** |
| Fresh source identity verification after current GUI/benchmark runs | **PASS**; original and both copies match private baseline |
| Exact hotfix installer verification and installed-app lifecycle | **NOT RUN** |
| Upgrade from installed 1.1 with preservation of external user data | **NOT RUN** |
| `release/ACCEPTANCE.json` stable readiness | **false** |

The published 1.1.0 tag and assets remain unchanged. Windows 10 performance
for a new artifact must be recorded separately; the owner's historical Windows
10 report remains valid and does not require recreating that machine. Do not
infer cold disk-cache timing from uncontrolled-cache service measurements.

## Next actions

1. Address measured full-scan text/path costs while preserving query semantics,
   cancellation and source integrity. Code audit identified repeated path buffer
   reconstruction as a bounded candidate; it is not yet a measured improvement.
   Ordinary ASCII name contains already has a no-allocation path; GLOB allocation
   work is a separate workload and must not be claimed as fixing these results.
2. Complete the hotfix operation audit, offline installer verification and
   installed 1.1 upgrade/data-preservation checks on the exact candidate.
3. Keep stable readiness false until the required evidence and release gates
   are complete.
