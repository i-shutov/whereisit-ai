# WhereIsIt AI — Windows 11 Pro AMD64 egui interactive acceptance

**Prepared 2026-11-21; execution status: NOT_RUN. Owner-approved QA account: Administrator. A separate standard user is not required.** This checklist applies to native Rust/egui Win64 **installed QA** artifact, not legacy Win32 RC3 or service-session runner. Required physical GUI observations cannot be inferred from CI.

## W1: provenance before running any unsigned software

1. On the public GitHub repository, check the trusted source commit, hosted workflow **WhereIsIt AI — native CI and installer candidates** and exact run ID; only use the artifact `WhereIsItAI-egui-Win64-installed-QA-unsigned` from that run. Do not use untrusted forks/mirrors or administrator-owned self-hosted runner for untrusted code.
2. Extract artifact into fresh non-admin scratch directory. Run read-only `tools/qa/windows11-egui/Verify-Win64EguiCandidate.ps1` with expected 40-hex commit SHA and run ID from trusted workflow. Capture PASS, source SHA and hash; if mismatched, **STOP**. This verifies internal integrity; unsigned provenance **does not prove an authenticated publisher**.
3. Record a redacted OS edition/build, Win64 architecture, RDP interactive session vs runner service, display size and local time. Do not publish host IP, real usernames, CTF, files, private paths or license tokens.

## W2/W4: install and physical GUI, separate from headless CI

| ID | Action on Windows 11 Pro **interactive desktop** | Evidence / acceptance | Initial state |
|---|---|---|---|
| WIN64-01 | Per-user install under the owner's Administrator account to separate egui QA AppId; do not upgrade/replace RC3 | Inno setup success, distinct app name and Start Menu, no unexpected UAC | NOT_RUN |
| WIN64-02 | Launch installed `whereisit-egui-win64.exe` and close/reopen | Actual GUI screenshot, crash-free startup; compare installed SHA with provenance `egui_sha256` | NOT_RUN |
| WIN64-03 | Create and reopen synthetic SQLite, 60+ generated volumes, Unicode notes/stickers | Correct catalog, no real input/paths published | NOT_RUN |
| WIN64-04 | Ctrl+F global and disk-scoped Boolean/Unicode queries; 300+ results, page 251+, inspector, containing folder and Back | Correct results and restored scope/selection/scroll vs PR58 reference | NOT_RUN |
| WIN64-05 | CSV/JSON/Parquet full-scope export to fresh destination, then repeat to same destination | Independently checked counts; no-clobber, original bytes unchanged | NOT_RUN |
| WIN64-06 | Generate isolated NTFS tree 20–30k files and separate 272k tree, scan metadata only, cancel with Esc | Genuine progress; rolled-back volume/entry counts; SQLite `quick_check=ok`; successful retry | NOT_RUN |
| WIN64-07 | Missing scan root, symlink root, SQLite inside source, modal noninteraction | Refusal with actionable errors, no partial catalog/snapshot | NOT_RUN |
| WIN64-08 | 1024×768, 1366×768, 1320×760 window view; light/dark and RU/EN persisted; keyboard/Back/focus | Paired screenshots and measured clicks/distance/timing only when directly observed; DPI 100/125/200% only when safe | NOT_RUN |
| WIN64-09 | Uninstall side-by-side QA app as Administrator, leave external SQLite and local notes in place | App EXE removed, generated external data byte-identical and RC3 unaffected | NOT_RUN |
| WIN64-10 | Category UI, physical USB, original-app edited CTF, 3M real-data p95, signing | Remain MISSING/BLOCKED until independent acceptance; no scope downgrading | BLOCKED |

## Isolation and reporting

Use generated fixtures in an independent dedicated directory and separate SQLite; do not scan entire system drive, change Windows DPI for other sessions, run binaries from the CI runner service session, alter other project runners, move real USB, copy private CTF into public logs or disable Windows Defender/SmartScreen. Prefer renderer-only QA screenshots and manually verify each image for sensitive text. 

Record separately `SOURCE_COMPILED`, `INSTALLER_PACKAGED`, `WIN11_GUI_LAUNCHED`, `WIN11_SCAN_CANCEL_ROLLBACK`, `WIN11_SEARCH_EXPORT`, `WIN11_INTERACTIVE_UX_PARITY` with statuses `PASS/FAIL/PARTIAL/BLOCKED/NOT_RUN`, exact source SHA, run ID and test binary SHA. No stable `v1.0.0` without all mandatory gates, including signing and independent platform tests. USB hardware tests are deferred for this immediate Win11 block, **not** automatically accepted for stable.
