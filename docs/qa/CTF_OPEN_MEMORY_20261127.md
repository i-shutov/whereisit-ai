# CTF opening memory — 27 November 2026

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
