# CTF snapshot entry-capacity checkpoint

24 November 2026. Development prototype; published releases are unchanged.

## Change

The direct opener now drops its captured raw buffer and asks each parsed disk's
entry vector to release unused capacity before sharing the immutable snapshot.
It retains every record, decoded string, timestamp and opaque payload. IDs,
query semantics, the parser and the explicit native-import path are unchanged.
This is allocator compaction, not another index or a second RAM table.

## Capacity and allocator measurements

A generated fixture with 370,000 records, Unicode names and descriptions reduced
entry-slot capacity from 62,914,560 to 44,400,000 bytes. Complete serialized CTF
bytes before and after compaction were identical, and roundtrip record counts
were preserved.

An isolated parse-only experiment read the full frozen working copy, dropped the
raw input buffer and compared aggregate capacities and process counters. There
were 44 disks and 3,046,133 records; `size_of::<CtfEntry>()` was 120 bytes.

| Counter | Before | After `shrink_to_fit()` |
|---|---:|---:|
| Entry-slot capacity, bytes | 578,751,840 | 365,535,960 |
| Name capacity, bytes | 63,911,399 | 63,911,399 |
| Description capacity, bytes | 13,452,920 | 13,452,920 |
| Opaque-payload capacity, bytes | 145,728,242 | 145,728,242 |
| Process private usage, bytes | 925,114,368 | 715,329,536 |
| Process working set, bytes | 699,142,144 | 698,724,352 |
| Process peak working set, bytes | 903,430,144 | 903,430,144 |

Slot capacity fell by 213,215,880 bytes (203.34 MiB); private usage fell by
209,784,832 bytes (200.07 MiB). Full metadata/opaque fingerprints matched, without
printing digests. This fingerprint is a consistency check, not byte-exact CTF
compatibility evidence. Parse took 869.98 ms and shrinking 1.122 ms in that run.
The Windows `PrivateUsage` counter describes private committed memory; working
set is a separate counter. [Microsoft's definitions](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex).

A separate explicit reallocation experiment did not lower working set further,
added about 93 ms and more page faults. That variant is not implemented.

## Sequential native service comparison

Both release-mode runs used native DuckDB 1.4.1, the same working copy and the
same read-only SQLite reference. Five samples of each of ten query classes
matched complete response pages/counts/continuation state. Source-byte checks
passed. Timings include source verification and service serialization; OS caches
were uncontrolled. The comparison is sequential and is not a latency improvement
claim.

| Counter | Baseline | Compacted |
|---|---:|---:|
| Open, ms | 1,787 | 1,803 |
| Open through first search, ms | 2,739 | 2,777 |
| Final working set, bytes | 785,309,696 | 788,754,432 |
| Peak working set, bytes | 1,025,277,952 | 1,032,159,232 |

There was no measured working-set improvement in the full service run. Query
p50/p95 observations varied in both directions; the absent-path p95 increased
from 682.347 to 957.938 ms while its p50 increased from 622.278 to 649.090 ms.
Five observations with uncontrolled scheduling/caches do not isolate a cause.
The intended benefit is less unused vector capacity and private commit, not a
claim of lower resident/peak memory or faster searches.

## Validation and remaining gates

Two isolated native GUI builds were opened sequentially with the same working
copy and fresh QA settings. After each initial full-catalog result page completed,
Windows process counters were sampled before further navigation:

| GUI counter | Baseline | Compacted |
|---|---:|---:|
| Private bytes (`PrivateMemorySize64`) | 1,281,699,840 | 1,066,823,680 |
| Working set, bytes | 835,772,416 | 813,703,168 |
| Peak working set, bytes | 1,085,607,936 | 1,023,254,528 |

Private bytes decreased by 214,876,160 bytes (204.92 MiB). These are one-process
observations, not controlled resident-memory or peak-memory guarantees. The
compacted build was physically exercised through the volume map, extension
highlight and opening that highlight in search. Source/working-copy hashes still
matched their frozen baseline after closing the application. Executable hashes
and raw evidence remain private.

The feature-enabled service suite passed 71 tests, including the new synthetic
regression for complete writer bytes, Unicode descriptions, browse/search IDs
and unchanged source bytes. Workspace/all-target release Clippy with warnings
denied passed. The standard Rust allocator may retain extra capacity or resident
pages; behavior on other allocators/platforms is not certified by this run.

Exact candidate CI is recorded in the PR as it completes. Installed/offline
hotfix acceptance and current-artifact Windows 10
coverage remain open. `stable_ready=false`; private catalogs, paths, source
hashes, software licenses, raw rows and screenshots are excluded.
