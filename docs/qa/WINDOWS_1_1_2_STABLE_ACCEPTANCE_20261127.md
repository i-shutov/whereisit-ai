# Windows 1.1.2 memory-fix readiness — 2026-11-27

Windows x64 unsigned publication is explicitly owner-authorized. Prior releases,
tags and assets remain immutable. macOS/Linux product releases are deferred;
portable compile/tests remain useful. Windows 10 coverage is historical and
owner-confirmed, not an exact 1.1.2 artifact PASS.

## Accepted code and versioned candidate

PR #103 was reviewed without actionable findings and merged as `c936124`.
Its 15 reported PR checks and 13 main workflow jobs passed. The exact-main
local standard installer passed actual Windows 11 Administrator RDP setup,
payload hash checks, 60 synthetic disks, full protected-copy opening/search
with 44 disks / 3,046,133 records, paging, observed running-query cancellation,
retry, close/restart and external SQLite opening. GUI uninstall preserved the
generated external SQLite byte-for-byte. Original, frozen master and working
copy remained equal to the frozen baseline. This is observed evidence for
that code checkpoint, not retroactive certification of later installers.

Version preparation `c836ec3` updates release identity to 1.1.2 without changing
Rust product behavior. Its local standard installer passed actual Install and
Finish, installed GUI/CLI hash and version checks, 60-disk opening, search
pages 1–250 and 251–500, and normal close. The temporary candidate was removed
and prior stable registration restored. Release-identity checks, four identity
regressions and fourteen Windows stable-publication contracts passed.

Readiness records functional/data-safety acceptance of the tested candidate.
The final rebuilt main artifact still requires successful source-bound test
and Windows jobs, archive/provenance/checksum verification, and actual installed
GUI acceptance before the publisher is triggered. Later binaries are not
certified by the candidate evidence alone. Final results belong to the release
PR and publisher provenance; raw local evidence is not published.

## Memory, fidelity and limits

See [the sanitized memory report](CTF_OPEN_MEMORY_20261127.md). Three fresh
service processes reduced maximum observed working set from 862 to 549 MiB,
peak commit from 1081 to 554 MiB and retained private memory from 733 to 467 MiB.
Opening through first search was 0.891–0.970 seconds. Cache and host pressure
were uncontrolled; these are service measurements, not final-installer timing
or an arbitrary-query guarantee. One size/count series retained p95 186.376 ms;
three paired repeats showed no regression. The target miss is not discarded.

Full and metadata parser common-field parity passed across the complete copy;
the full writer roundtrip was byte-identical. Synthetic parser bounds,
Unicode/description budgets, metadata writer exclusion, preview-active full
import/export and snapshot release are covered. Optional native DuckDB runtime
tests and strict feature Clippy passed in PR #103 CI.

Actual independent original-WhereIsIt compatibility in both directions is
historical 1.1.1 evidence in `release/history/v1.1.1.json`. No new vendor GUI run
is claimed for 1.1.2. The full writer/import route retains opaque data; metadata
storage is only used for read-only browsing/search. The memory change adds no
content indexing or source writes. USB pairing/bound-media rescan stays disabled.

No private catalogs, source paths/digests, proprietary licenses, raw logs or
screenshots are included in source distribution or release assets.
