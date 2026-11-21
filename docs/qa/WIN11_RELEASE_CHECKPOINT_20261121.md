# Win11 release checkpoint — 2026-11-21

Only aggregate evidence is recorded here. No private catalogs, real filenames/paths/queries, source hashes, raw screenshots or licenses are included.

Current integration: PR #79 merged as `e7ab4d5f6a1df6ea820b8b867dfe5255004fb4cd`, plan #80 as `db6c1aeb1b426280175b583265e5e26c3812b643`, and functional PR #81 as `0708f0444f035fc44946c224afb18e847efca1de`. The PR81 section records actual installed product acceptance and exact-head CI; post-merge main CI is a separate check. Earlier measurements below retain their original artifact identity. RC4 release identity is being prepared separately; stable_ready remains false.

## Main accepted scope

Main `8cbb3b4f5e9f07537cbd0d42deeeee01ec4e35ef`, after ordered merges #73 and #78. Exact-main hosted [native platforms/installer](https://github.com/i-shutov/whereisit-ai/actions/runs/44929080959), [upgrade](https://github.com/i-shutov/whereisit-ai/actions/runs/44929080983), [integration](https://github.com/i-shutov/whereisit-ai/actions/runs/44929081025): PASS.

- Windows 11 Pro x64 build 22631, interactive Administrator. Native installed egui GUI was actually used. Generated 60-volume catalogs, categories by ID, query/page/inspector/Back and narrow Path checks passed within their recorded scope. Install/restart/uninstall preserved external SQLite. Wider DPI/platform/physical-USB QA remains separate.
- WhereIsIt 2014 build 220 accepted corrected synthetic output and an exported description change on an authorized private copy. A vendor-saved description change was imported natively; the tested re-export matched the vendor save byte-for-byte. Protected source/control files remained unchanged. [Sanitized CTF results](WIN11_CTF_PATH_ACCEPTANCE_REDACTED_20261121.md). This does not establish arbitrary-format compatibility or a direct egui CTF workflow.
- Full-catalog search: 44 volumes, 3,046,133 entries, page limit 250, one CLI serve process, six fixed workload classes, 30 warm samples each. Latency includes request, SQLite opens, SQL, annotation and JSON transport; OS cache uncontrolled. No cold-cache or GUI-rendering claim.

| Workload | p50 ms | p95 ms |
|---|---:|---:|
| Global Boolean | 31.41 | 34.64 |
| Unicode | 15.42 | 18.87 |
| Volume | 10.41 | 11.97 |
| Size/extension | 30.31 | 32.62 |
| Folder | 2.98 | 3.81 |
| Global with count | 126.92 | 130.21 |

Peak CLI working set 9.14 MiB; sampled private bytes 5.52 MiB. First responses reached 574.65 ms and are excluded from warm quantiles. SQLite size 1,675,427,840 bytes; snapshot content/generation unchanged. Windows process I/O counters were collected but are not physical disk traffic. PASS applies to these warm workloads, not every query/platform.

## PR79 artifact accepted scope, subsequently integrated

[PR #79](https://github.com/i-shutov/whereisit-ai/pull/79), tested head `43b324644433b026a438b3abf4516119f19b8ac5`, is now merged. Local source-equivalent tree `2f3b947495a82b58873842baab46a4dadf813b38`: 179 tests passed. Hosted [native](https://github.com/i-shutov/whereisit-ai/actions/runs/44944080020), [offline](https://github.com/i-shutov/whereisit-ai/actions/runs/44944079977), [upgrade](https://github.com/i-shutov/whereisit-ai/actions/runs/44944080038), [integration](https://github.com/i-shutov/whereisit-ai/actions/runs/44944079944): PASS.

The packaged Win64 candidate passed actual GUI installation, opening the full catalog before validation, explicit full-check success, catalog-switch status reset, failure cleanup on a generated count-invalid catalog, restart and uninstall with data unchanged. Schema probe + first unfiltered 250 rows: p50/p95 9.15/14.15 ms over 30 warm CLI iterations; first response 34.22 ms. This excludes GUI/sidebar and full validation. Full validation remains an explicit command; opening is not a data-integrity PASS. The CTF malformed-input refusal changes have generated red/green regressions.

## PR81 product acceptance, integrated as 0708f04

PR [#81](https://github.com/i-shutov/whereisit-ai/pull/81) merged as `0708f0444f035fc44946c224afb18e847efca1de`. Its tree `a2fe1d785122c6a7a2967333e23c6f6bba280371` equals the tested head `bd3c235ba5aabb48932da0b630e2a0b845d2545f`. Hosted native [44953342592](https://github.com/i-shutov/whereisit-ai/actions/runs/44953342592), offline 44953342595, upgrade 44953342584 and integration 44953342626 passed all ten jobs. Local Windows workspace tests: 301 PASS; strict all-features Clippy and seven product-packaging regressions PASS. Focused independent review found no actionable P0/P1 in staged import, guarded description edits, pinned reads or pairing safety.

The standard Windows egui product installer was downloaded from that run; archive, setup and installed GUI/CLI hashes matched provenance. Actual interactive GUI evidence:

- 60 generated volumes: read-only CTF link, Boolean/Unicode search, pagination, syntax-error recovery without source loss, disabled linked-description save, import to new SQLite, Unicode description edit and new-file CTF export PASS. Original WhereIsIt 2014 build 220 independently displayed the new description.
- Full private-copy GUI import published 44 volumes / 3,046,133 entries into a new 1,675,427,840-byte SQLite after FTS integrity and SQLite checks. Observed elapsed upper bound 451.47 seconds. Sampled process peaks: working set 803.81 MiB, private bytes 1,421.11 MiB; monitoring included setup/observation delay and is not phase-specific attribution.
- Description edit on the derivative SQLite and new-file CTF export PASS. Original WhereIsIt opened the full exported catalog and displayed the exact changed Unicode description. Four protected source/control hashes remained unchanged. No original was a write target.
- Real-import cancellation observed at 223,604 entries left no destination or staging files; existing catalog and inputs unchanged.
- Candidate CSV/JSON/Parquet exports each contained the same 59 generated entries and 15 fields, excluding the source disk. CSV overwrite refused; existing file unchanged. Back restored the source query and selection.
- Imported-CTF rescan refused. Physical verification offered no eligible non-system volume and disabled pairing. This tests safe refusal; real USB pairing/unplug/replug remains BLOCKED.
- Uninstall removed both installed EXEs and preserved four external databases/controls byte-for-byte. Reinstall verified both EXE hashes and actual GUI reopened the 60-volume external SQLite. This is unsigned product lifecycle evidence, not RC3-to-RC4 or signed acceptance.

Exact installed CLI: SQLite 3.54.1, six classes with 30 warm requests each, zero failures, stable ordered results. Peak working set 8.93 MiB; database size/write timestamp and zero-length WAL unchanged. Timings include CLI JSON round trips/SQLite opens, exclude GUI rendering, and use uncontrolled OS cache.

| Workload | p50 ms | p95 ms |
|---|---:|---:|
| Global Boolean | 31.62 | 35.06 |
| Unicode | 14.92 | 18.27 |
| Volume | 10.15 | 11.23 |
| Size/extension | 29.79 | 34.03 |
| Folder | 3.07 | 3.96 |
| Global with count | 124.66 | 147.08 |

These six warm workloads meet p95 <=150 ms. No controlled cold-cache claim. Full evidence and screenshots remain private and are not release contents.

## RC4 candidate and counted-search closure

PR82 prepares RC4; PR83 was integrated into its branch as `ff5af28758fff55817b80b8173e0a13e1ccd2abc`. Tested hosted runtime head `b0efdb94b1f22727a5205f1510a0c9c0d3849b8d`: native [44960976668](https://github.com/i-shutov/whereisit-ai/actions/runs/44960976668), installed upgrade/rollback 44960976635, frozen historical upgrade 44960976588, offline 44960976630 and integration 44960976570 all PASS. Windows x64 and macOS ARM64/Intel installed product upgrade/rollback passed on hosted runners; these are not interactive Mac GUI claims. All 300 local default-feature workspace tests and all-target/all-feature Clippy passed.

Installed exact-source Win11 GUI/CLI hashes matched provenance. It opened the full 44-volume SQLite and displayed files larger than 4 GiB. The catalog contains 2,805,176 files, 239,207 directories and 1,750 archives; 6,324 entries exceed 4 GiB. Five external controls, including a generated private sidecar note, retained their hashes through the fixed installer upgrade. Earlier local RC4-to-RC3 rollback also retained that note, visibly confirmed in GUI.

The earlier RC4 hosted head 9058728 measured a real full-count p95 FAIL of 155.23 ms. PR83 fixes it using exact counts in one SQLite snapshot, without cached volume totals or changed predicates/pages. The installed fixed artifact passed all six x30 warm workloads, p95 3.60–48.91 ms, peak CLI working set 8.93 MiB; no source CTF was accessed by the benchmark. [Before/after and synthetic regressions](COUNTED_SEARCH_SLO_20261121.md). Original failed measurements remain in private evidence.

The installed fixed GUI also exported the full edited derivative to another new CTF. Its bytes matched the earlier modified export independently opened and checked in original WhereIsIt; this comparison is against that accepted changed output, not an unchanged source roundtrip.

Full CSV/Parquet field parity was rerun with the exact installed fixed CLI: 3,046,133 rows x15 fields, all values equal under the independent DuckDB verifier (384 MiB / two threads). CSV 599,828,444 bytes in15.37s; Parquet123,361,897 bytes in18.76s. The working SQLite hash, size and write timestamp were unchanged; no additional CTF import was performed and the source CTF was never accessed by this export check.

Actual native Scan GUI: a small generated folder completed with 1 volume / 8 entries. A subsequent 100,000-file scan showed 10,436 processed entries; Esc cancelled it with the no-snapshot-saved message. The complete original 1-volume/8-entry logical snapshot was identical afterwards, quick_check OK and foreign-key errors zero. This is separate from the earlier CTF import-cancellation test.

Exact main 0708 product was also installed and actually exercised before RC4: full-catalog GUI search, mouse-wheel → candidates → Back, and containing-folder → child-folder → two Back actions restored query/selection/scroll. Its independent CLI repeat passed six x30 warm workloads, p95 6.73–147.46 ms, peak working set 9.01 MiB. These historical measurements retain their own source identity.

## Release decision

Win11 scoped GUI/product lifecycle, direct egui CTF workflows, tested original-app modified CTF compatibility and measured warm search are accepted evidence. They must not remain NOT_RUN/BLOCKED merely because old documents predate them. Remaining platform/UX parity acceptance, real hardware/wider-platform acceptance, signed RC4 target-platform acceptance and authenticated signatures remain open. Stable 1.0 remains NO-GO, stable_ready=false. Current sequence: [release action plan](../RELEASE_1_0_WIN11_ACTION_PLAN_2026-11-21.md).
