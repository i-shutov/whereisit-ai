# CTF opening memory вЂ” 27 November 2026

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
