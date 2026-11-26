# Windows CTF source read pin — 26 November 2026

Status: source-pin implementation and service benchmark validated; installed hotfix acceptance remains open; `stable_ready=false`.

## Integrity and scope

Direct CTF preview previously streamed the entire source for SHA-256 before and
after every folder page, search, analytic result and export. On eligible Windows sources the new
path opens a read-only handle with only read sharing, captures and hashes bytes
from that same file object once, and keeps the handle in the snapshot. Other
readers remain allowed; write/delete sharing is denied while the snapshot or an
in-flight reader retains the pin.

Each selection and result publication compares the source pathname against the
held file's full 128-bit Windows file ID and volume serial, plus bounded regular
file/size checks. It does not trust mtime or file length alone. Capture checks the
binding before and after reading, and publication checks it again after parsing.
The implementation uses `GetFileInformationByHandleEx(FileIdInfo)` rather than
the truncated Windows identity in the existing same-file dependency.

The fast path is limited to local sources on volumes reported as fixed NTFS or
ReFS. UNC/device/relative syntax is rejected by the pin helper before opening;
the actual held handle must expose a volume GUID, fixed-drive classification and
supported filesystem. This checks the opened volume rather than trusting an
input drive letter or junction. Ineligible sources retain the full-hash path;
this does not remove catalog-reading support for other storage.

If pin acquisition or any eligibility/identity API is unavailable, including an
existing writer, opening retains the previous capture and full per-request SHA
checks.
Once a pin is established, a failed binding check returns `source_changed`;
it never silently switches to weaker verification. Other platforms retain the
full-hash path. Mirror, native editing and their publication checks are unchanged.

The response identifies `source_verification` as `windows_read_pin` or
`full_sha256`. `source_integrity_verified=true` describes successful validation;
`source_rehashed` and `source_sha_checked` are false on pinned responses and true
on the full-hash fallback. The initial captured source hash is returned in both
modes; this does not claim that every pinned response rereads source bytes.

Closing only clears the matching generation. Existing requests retain their Arc
and source pin through result construction and publication; write access returns
after the last reader drops. External editors may be unable to modify a pinned
source until it is closed. No source writes are introduced.

## Synthetic regression evidence

Windows default-feature service tests: **168 passed**, four manual benchmarks
ignored. The isolated release desktop suite passed all **105 tests**. Strict
all-target service Clippy and formatting passed. Coverage includes:

- Readers allowed; write/truncate/rename/delete and hardlink writers denied.
- An existing writer prevents pin acquisition and selects the hash fallback.
- A pre-existing writable mapped view, with both original file and mapping
  handles closed, prevents pin acquisition; acquisition succeeds after unmapping.
  A section handle without any view also prevents acquisition.
- Read-only attributes, full-width identity mismatch and bounded capture.
- Page, search and export responses under a pin perform no streamed source hash
  reads, verified by a test-only counter at the actual identity-read function.
- Hash fallback rejects same-size changes with restored source mtime.
- Close during an in-flight read retains protection until that reader releases it.
- Existing query/SQLite parity, exports, cancellation and stale-generation tests.

## Measurements and GUI

The following table records the bounded-buffer checkpoint `caec14f`, before the
subsequent eligibility hardening. Exact later-head measurements are recorded in
PR #98 rather than inferred from these numbers.

The default Rust executor was measured on the same protected full working copy
(44 volumes, 3,046,133 records), 30 requests per class, 300 repeated requests.
No concurrent GUI or local builds ran during the benchmark. Source byte identity
passed, and direct opening created no SQLite derivative. OS cache is uncontrolled
and the harness hashes the input before timing; these are service measurements,
not cold physical-disk or installed-UI timing. No fresh SQLite reference was
configured for this run; synthetic query/SQLite parity tests remain separate.

| Metric | Previous `aa8b37d` | Source pin `caec14f` |
|---|---:|---:|
| Open, single observation | 2392 ms | 1366 ms |
| Open through first search, single observation | 5545 ms | 3601 ms |
| Full 300-request run, excluding compilation | 287.99 s | 202.61 s |
| Peak process working set | 734.06 MiB | 862.79 MiB |
| End process working set | 648.91 MiB | 744.57 MiB |

| Query class | New p50 ms | New p95 ms | Previous p95 ms |
|---|---:|---:|---:|
| Empty global diagnostic (not catalog startup) | 1.043 | 1.784 | 200.823 |
| Extension | 10.488 | 11.989 | 204.230 |
| Ordinary text | 35.555 | 41.592 | 275.485 |
| Boolean | 0.977 | 2.017 | 196.013 |
| Unicode name | 771.023 | 816.653 | 1248.425 |
| Absent name | 548.220 | 601.236 | 1025.425 |
| Absent ordinary text | 2495.330 | 2686.393 | 3148.377 |
| Absent path | 2247.090 | 2353.349 | 2925.088 |
| Absent description | 259.467 | 279.862 | 499.628 |
| Extension/size with full count | 208.201 | 216.147 | 433.935 |

The measured executor reported `windows_read_pin`. The first implementation
(`2e1967d`) completed another 300 successful requests but peaked at 964.30 MiB.
Reserving the bounded capture buffer to the protected length plus an EOF byte
reduced that peak; the final full-run peak was **904,704,000 bytes**, end working
set **780,742,656 bytes**. It is still **128.73 MiB above** the earlier baseline
peak. A source-owner audit found no retained raw capture buffer after parsing,
but the working-set increase is unresolved and must not be described as a memory
improvement. These counters are not private bytes or allocator-only usage.

Common query latency improved substantially. First-use delay and expensive full
scans remain; **P1-CTF-COLD-OPEN stays OPEN**. No global-sort optimization was added.
Windows 10, ReFS, network/removable source behavior and installed hotfix lifecycle
are not claimed from this local Windows 11/NTFS run. Existing published assets
and release readiness are unchanged.

Actual current GUI observations and exact final CI status are recorded in the
validation section of [PR #98](https://github.com/i-shutov/whereisit-ai/pull/98).
Service timing, automated installed-upgrade CI and physical GUI evidence are
separate gates. No private catalogs, paths, source hashes, labels or raw
screenshots are included here.

## API references

Microsoft documents write-mapping conflicts when write sharing is omitted in
[CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).
Full-width handle identity follows
[FILE_ID_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info).
The synthetic tests verify the relevant sharing behavior on the actual host;
these references do not substitute for platform acceptance. Local-volume
eligibility also uses the documented
[volume GUID result](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew)
and [drive classification](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew).
