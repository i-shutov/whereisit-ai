# CTF opening memory Р Р†Р вЂљРІР‚Сњ 27 November 2026

Development on top of Windows 1.1.1 source `4bb4a403`. Published `v1.1.1` and
its assets stay immutable. This work is a new candidate, not a release:
`stable_ready=false` and publication approval is revoked on this branch.

## Baseline

Three fresh-process runs of the unchanged release-mode service benchmark on a
protected full catalog copy: 44 volumes, 3,046,133 records. Each ran all ten
query classes with three samples, catalog order and the Rust executor. These
short runs establish memory baselines, not a new p95 performance acceptance.
No concurrent build or GUI automation ran. OS cache and physical-memory
pressure were uncontrolled. The benchmark verifies source bytes unchanged and
closes the generation. Actual paths, source hashes and raw logs remain private.

| Run | Peak working set MiB | Peak commit MiB | Private after open MiB | Private after close MiB |
|---|---:|---:|---:|---:|
| 1 | 656.840 | 1080.664 | 732.969 | 8.859 |
| 2 | 800.656 | 1081.043 | 733.359 | 9.254 |
| 3 | 862.449 | 1080.676 | 733.219 | 8.879 |

Opening took 1624 / 1254 / 1247 ms; opening through first search took
1662 / 1281 / 1274 ms. Process-lifetime peaks include opening and the later
small query workload. Working set varies with Windows paging; commit and
retained private bytes are reported alongside it. Return after close is a
bounded observation, not a long-duration leak claim.

## First measured hypothesis

The parser grows each disk's owned entry vector geometrically while captured
source bytes and per-entry raw/decoded fields coexist. Directory ranges already
carry file counts; after validating ranges, counts and physical lower bounds,
reserve once for the remaining entries to avoid repeated large reallocations
and unused capacity. Do not trust an unvalidated count, discard opaque bytes,
weaken source binding or bypass parser limits.

Further layout/index changes depend on measurements from this first change.
Regressions must cover allocation capacity, forged counts, global limits,
Unicode/opaque roundtrip integrity and query/navigation semantics. Follow-up
memory measurements use the same executable harness in fresh processes;
latency and native GUI checks remain required before acceptance.

## Stage 1: checked exact entry reservation

The parser now checks file counts against validated range payload lengths and
the minimum accepted 11-byte file record, then enforces the aggregate entry
limit before a fallible exact reservation. Full opaque data and structural
checks are retained. Twenty parser tests passed, including count forgery,
multiple-disk limits and Unicode/unknown-field byte-identical roundtrip.
An independent review found no P0/P1 in the bounds/reservation change.

Same unchanged benchmark harness and workload, three fresh processes:

| Run | Peak working set MiB | Peak commit MiB | Private after open MiB | Private after close MiB |
|---|---:|---:|---:|---:|
| 1 | 781.699 | 819.203 | 732.691 | 8.504 |
| 2 | 806.492 | 819.027 | 732.523 | 9.074 |
| 3 | 806.297 | 818.656 | 732.375 | 8.020 |

Opening took 1158 / 1133 / 1112 ms; through first search 1187 / 1162 / 1139 ms.
Peak commit dropped from about 1081 to 819 MiB (about 24%). The maximum
observed working set dropped from 862 to 807 MiB, but retained private memory
remained about 733 MiB. Paging makes individual working-set runs incomparable
as a strict allocation measure; commit and retained bytes motivate the next
step. Source bytes stayed unchanged in every run.

Next: a distinct read-only metadata projection that validates the same format
but does not retain raw writer/import opaque buffers. Normal import and export
continue using the full-fidelity parser; metadata types cannot enter the CTF
writer. This is not permission to discard unknown source data or change files.

## Stage 2: typed read-only metadata projection

The common parser now has two storage types. `CtfEntry` retains all opaque
writer/import bytes; `MetadataEntry` retains the same interpreted fields in
96 bytes instead of 120 on the measured Windows x64 toolchain. The preview
parses directly into metadata storage rather than allocating a full catalog
and converting it. Raw file/folder/header/trailer copies are skipped only for
this explicitly read-only type. Structural, encoding, extension, range and
aggregate-description-budget validation are shared with the full parser.
A compile-fail doctest proves metadata cannot enter `write_ctf`.

Normal import full-parses its existing captured/verified bytes; it never reuses
the metadata projection. This removes the old full-parse reuse optimization
for explicit import but introduces no additional source read. Import/export
while preview remains active has a byte-exact opaque/shared-description
regression. Snapshot close is checked with a weak Arc; capacity trim occurs
after dropping the source capture, preserving folder-heavy behavior.

### Memory and opening

Three fresh processes, same unchanged benchmark workload as baseline:

| Run | Peak working set MiB | Peak commit MiB | Private after open MiB | Private after close MiB |
|---|---:|---:|---:|---:|
| 1 | 549.488 | 553.996 | 467.113 | 6.531 |
| 2 | 549.242 | 553.895 | 466.922 | 5.949 |
| 3 | 549.223 | 553.980 | 466.969 | 7.129 |

Peak working set is about 549 MiB versus the baseline maximum 862 MiB (36%
lower); peak commit is about 554 versus 1081 MiB (49% lower). Retained private
memory is about 467 versus 733 MiB (36% lower). Opening through first search
was 970 / 914 / 891 ms, versus baseline 1662 / 1281 / 1274 ms. Cache was
uncontrolled; these are service process observations, not GUI timing or
cold-cache guarantees. Baseline working-set variation remains in the table.

### Semantics and performance

Parser tests and compile-fail doctest passed, as did 98 service unit tests,
the full service integration suite and 103 desktop tests. Standard targets
passed strict all-target Clippy. Source binding and query/cancellation logic
remain unchanged. The optional DuckDB projection is adapted separately;
service feature tests compiled and strict feature Clippy passed locally; the
desktop qa-duckdb tests also compiled. Runtime feature CI is tracked in the PR.

A separate explicit-copy test compared all common fields of full and metadata
parses across all 44 disks / 3,046,133 entries, using length-delimited semantic
digests without publishing them. The full writer roundtrip was byte-identical
to input, and source bytes were unchanged. This parity process is separate
from memory measurement and its larger allocations are not benchmark results.

Two series of 30 samples for each of ten search classes were run. Ordinary
text p95 was 27.033 / 28.319 ms; Unicode 79.754 / 77.259 ms; absent text
114.986 / 111.633 ms. One full-count size/extension series exceeded the
150 ms target: p95 186.376 ms (other series 93.632 ms). This observation is
retained; there is no unqualified all-series SLO PASS.

The newly observed full-count tail triggered three paired fresh-process
repetitions, 30 samples each, baseline followed by candidate:

| Pair | Baseline size/count p50 / p95 ms | Metadata p50 / p95 ms |
|---|---:|---:|
| 1 | 83.115 / 117.686 | 76.606 / 85.090 |
| 2 | 85.200 / 103.291 | 76.762 / 86.080 |
| 3 | 88.055 / 98.885 | 77.150 / 81.795 |

These pairs show no regression for that class in the controlled repeat; they
do not prove the earlier tail was caused by Windows scheduling. There is no
arbitrary-query or hardware latency guarantee. Native GUI and exact-head CI
results will be recorded in the PR; this candidate is not published.
