# Win11 release checkpoint — 2026-11-21

Only aggregate evidence is recorded here. No private catalogs, real filenames/paths/queries, source hashes, raw screenshots or licenses are included.

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

## PR-only accepted scope

Draft [PR #79](https://github.com/i-shutov/whereisit-ai/pull/79), exact head `43b324644433b026a438b3abf4516119f19b8ac5`, is **not merged**. Local source-equivalent tree `2f3b947495a82b58873842baab46a4dadf813b38`: 179 tests passed. Hosted [native](https://github.com/i-shutov/whereisit-ai/actions/runs/44944080020), [offline](https://github.com/i-shutov/whereisit-ai/actions/runs/44944079977), [upgrade](https://github.com/i-shutov/whereisit-ai/actions/runs/44944080038), [integration](https://github.com/i-shutov/whereisit-ai/actions/runs/44944079944): PASS.

The packaged Win64 candidate passed actual GUI installation, opening the full catalog before validation, explicit full-check success, catalog-switch status reset, failure cleanup on a generated count-invalid catalog, restart and uninstall with data unchanged. Schema probe + first unfiltered 250 rows: p50/p95 9.15/14.15 ms over 30 warm CLI iterations; first response 34.22 ms. This excludes GUI/sidebar and full validation. Full validation remains an explicit command; opening is not a data-integrity PASS. The CTF malformed-input refusal changes have generated red/green regressions.

## Release decision

Win11 scoped GUI/lifecycle, tested original-app CTF compatibility and measured warm search are accepted evidence. They must not remain NOT_RUN/BLOCKED merely because old documents predate them. Direct CTF in egui, remaining P0 parity, real hardware/wider-platform acceptance, product RC4 upgrade and authenticated signatures remain open. Stable 1.0 remains NO-GO, stable_ready=false. Current sequence: [release action plan](../RELEASE_1_0_WIN11_ACTION_PLAN_2026-11-21.md).
