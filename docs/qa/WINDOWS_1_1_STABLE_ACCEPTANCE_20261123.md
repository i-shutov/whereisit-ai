# Windows 1.1 stable acceptance — 23 November 2026

**Correction, 24 November 2026:** the owner reports that first opening a large CTF on Windows 10 still processes records after 15 minutes. This exposes P1-CTF-COLD-OPEN and invalidates the completeness of the earlier no-remaining-P1 conclusion below. The GUI full-catalog checks used a prepared SQLite derivative; the warm search benchmark did not cover cold CTF opening. Historical import completion and roundtrip integrity remain evidence for those specific operations, not acceptable first-open latency. See [defect and measurements](CTF_COLD_OPEN_DEFECT_20261124.md). Published tags and artifacts remain immutable; working-branch readiness is revoked pending a corrected candidate and actual GUI verification.

Status: **PASS for the tested Windows 1.1 implementation; ready for final artifact verification and explicit stable publication**. Windows x64 only, unsigned, as requested by the owner. No remaining confirmed P0/P1 in the accepted scope. The readiness commit changes release records and the RC-publisher stable-version guard, with synthetic tests; it does not change the tested Rust product code. Its rebuilt exact-main installer must still be checked in the GUI before publication. This record identifies the previously measured artifact precisely and does not retroactively certify another installer.

## Scope and exact artifact

- Target: Windows x64 Rust/egui, version 1.1.0, unsigned. USB pairing and bound-media rescan remain disabled; ordinary metadata scanning is supported. macOS/Linux product releases remain deferred while portable compile/tests continue.
- Exact main source: `9ab97f1662dcc0f5a6490a8d7f721892d6578a38`.
- Native CI run: `45187054366`; product artifact: `14825485729`.
- Installer SHA-256: `3733bc6ef67fdbcd0cd1b4cac478a0de0d9ea880738f64a7634212eecf5ff3ff`.
- Installed GUI SHA-256: `81d6c44ab668849cf20620b8d234df9becb1bd67375526eb66e93aad31dbaa9e`.
- Installed CLI SHA-256: `a3a048ab67a97665fb200d2e0c30f0cd1f389093518730202e4e2a8a864bdb25`.

## Installed Windows upgrade and GUI — PASS

In the Windows 11 Administrator RDP desktop, the published 1.0.0 installation was upgraded to the exact 1.1.0 installer above. Restart and installed-app checks passed. Two generated external SQLite catalogs and a separate SQLite user-store remained byte-identical across upgrade and restart; notes, categories, preferences and existing 60-disk context remained available. The earlier exact RC GUI lifecycle also covered rollback/reinstall; this stable acceptance does not claim a new stable rollback run.

The exact 1.1.0 GUI displayed the full-catalog map with a 128-node bounded tree, correct full totals, explicit truncation/remainder, and branch navigation. Cancellation was observed; retry worked, and search/page 251–500 remained usable with context retained. The source SQLite hash was unchanged. The initial 5,000-node native chart request was a P1 latency defect; the 128-node native request fix and real-service synthetic regression close that defect for this build. The regression checks a 300-file flat folder, the node cap, full file/byte totals, omitted count and exact remainder accounting.

## Full-catalog performance — PASS for this measured Windows 11 workload

The private derived catalog contained 44 volumes and 3,046,133 entries. Six search classes were measured at 30 warm samples each on the exact installed CLI, without concurrent GUI automation. p95 target is 150 ms.

| Search class | p50 (ms) | p95 (ms) |
|---|---:|---:|
| Global Boolean | 34.829 | 38.711 |
| Unicode | 15.859 | 20.546 |
| Volume | 11.179 | 14.673 |
| Size / extension | 32.402 | 38.075 |
| Folder | 3.586 | 6.814 |
| Full count | 50.937 | 58.380 |

Peak working set was 9,433,088 bytes. The measured workloads passed the 150 ms p95 target. Cache state was uncontrolled; this is not a cold-cache guarantee or a claim about arbitrary hardware/load.

Analytics was measured separately from search using seven fresh workers, depth 4 and a 128-node output cap. Full totals were 2,805,176 files, 239,207 ordinary directories and 62,440,113,273,118 logical file bytes. Archive-container sizes were not double-counted. Analytics p50 was 38,312.898 ms; nearest-rank p95 was 49,456.722 ms; peak working set was 29,749,248 bytes. Cancellation and retry were observed in the GUI. Source SQLite was unchanged. These background aggregation measurements are not search-latency measurements.

## CTF import, roundtrip and exports — PASS

A fresh full CTF import processed 44 volumes and 3,046,133 entries. Catalog validation passed. Four fixed query classes agreed between fast and counted/full search pages. Unchanged CTF export was byte-identical to its input; the original source remained read-only.

Full CSV and Parquet exports were independently compared using DuckDB 1.4.1: all rows and all 15 fields matched. No-clobber refusal passed for CSV, Parquet and CTF export targets. No existing target was overwritten.

Original WhereIsIt 2014.0.0.220 opened the full modified CTF export in the actual GUI. The Unicode Rust note was visible; a changed Unicode note was saved as a new CTF file. The vendor-saved full CTF was imported by the exact installed native 1.1 CLI and validated: 44 volumes and 3,046,133 entries. An independent SQLite comparison covered all 3,046,133 rows and ten fields, allowing exactly the one intended Unicode-description change; all other semantic entry fields and volume metadata matched. Parent identity was resolved by path: vendor Save As reordered some records, so import-local row IDs and import timestamps were excluded from semantic equality. The original source hash remained unchanged. This accepts the exercised supported catalog/layout, not every possible CTF variant.

## Release state and privacy

Exact artifact identity, installed Windows upgrade/GUI, measured search, bounded analytics, full supported CTF import/export and original-vendor compatibility in both directions passed. The 60-disk and analyzer GUI coverage is traced through the earlier implementation/RC records; it is not misrepresented as a newly repeated full stable lifecycle. Exact-main stable CI also passed the published-1.0 upgrade/uninstall/rollback synthetic harness; service/runner acceptance is distinct from interactive GUI. The v1.0.0 tag/assets remain immutable. Publication uses a dedicated explicit control branch only after the final rebuilt artifact's checksum, installed GUI and trusted CI are checked; its identity and outcome are recorded in the release-readiness PR and published provenance.

Only aggregate results and public artifact identities are recorded here. Private CTF/catalog names, paths, source digests, raw rows and screenshots are omitted. Original catalogs are not modified.

## Remaining limitations and defects

Confirmed remaining P0: none. Confirmed remaining P1: none in the accepted Windows scope. Full analytics still takes tens of seconds on this large catalog; it runs outside the UI thread with cancellation and visible truncation. The initial 5,000-node latency defect is closed by the tested 128-node request. Windows 10 coverage is historical owner confirmation, not exact-artifact physical QA. macOS/Linux have compile/test coverage only and no product release claim. The installer is unsigned.

## Post-readiness exact-main checkpoint

Main `a68d51893d5db6c3639edf56fbcf9f7d04101f90`, native run `45198272061`, product artifact `14832205823` was also installed through the actual Windows GUI over the published 1.0.0. Installer SHA-256: `7c4b9fe8cc784c9d5cb2e29b8668d7984782a739cee12fcda9a56f0735a80b5d`; GUI: `ab164518083d0b0d83b6059db68b5b49a4852c8f634a8e379a1a707c8905c0e6`; CLI: `a58cb146b35f5f0028ce65fba5e719c42bd174d2ea45d08b7f52bb8fe73b68dd`.

Observed again: 60 generated disks, 466 Boolean-search matches and page 251–466; Map/Slices → Search retained the query/page. Generated analytics showed 540 files/60 directories/60 volumes. The explicit local analyzer processed two generated files/61 bytes and created a new one-row review CSV. Restart preserved the Unicode entry note, category assignment and all disk-preference fields. The complete catalog's bounded chart rendered correct totals/truncation; cancellation and retry were explicit, and branch navigation showed the independently checked 3,875-file branch. Its dedicated source SQLite was unchanged. Full fresh CTF import/validation, byte-identical unchanged CTF export and independent all-row/all-15-field CSV/Parquet equality passed on this exact installed CLI. Repeated six-class × 30-warm-sample search had maximum p95 58.5324 ms and peak working set 9,641,984 bytes. Actual GUI uninstall and published-1.0 restoration also preserved both external SQLite catalogs and the separate user-store byte-for-byte.

A publication P1 was exposed: the stable path required the *entire* native workflow to succeed even though the declared Windows required jobs are `test` and `windows`. On this source those jobs and macOS Intel passed; the unshipped ARM job was cancelled before a hosted runner was assigned. The dedicated stable publisher/preparer now validate trusted exact-source run identity and unique completed-success `test`/`windows` jobs, while permitting queued/failed/cancelled unshipped jobs. Synthetic cases exercise these states and reject failed, cancelled, missing, duplicated or unfinished required jobs, spoofed identity, wrong artifacts and failed readiness. Source, checksum, scope and tag/release immutability gates stay required. Cross-platform CI continues, with its actual status reported separately; this is not a claim that all native CI jobs passed.

The publication-only repair changes no Rust product code or installer definition. Its new exact-main artifact must still pass checksum/installed-GUI verification before the explicit stable trigger; the release-readiness PR and published provenance record that final identity.
