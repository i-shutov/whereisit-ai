# Technical handoff: WhereIsIt AI checkpoint CP-2026-11-13-01

## Resume point

- Canonical integrated `main` commit: `cabdb3d2b139a67729a795ba9c34e4d3144c9c9f`.
- Exact Git tree: `00c1e20ab5779475610275e1292cd2c5e333b618`.
- Published `v1.0.0-rc.3` tag at `5d91ba12adfa46f5ec6f0310f8e08b590b60f3c6` **before** privacy gate merge PR #56. Do not shift tag.
- Current draft PR #57: `93d7e525dc0433b6790a99250030c9c7fe09ca9a`, tree `4f09c2b6175d12899881ad95381ed9d95c92e921`; pending manual UX/USB QA.
- Open release issues: #1 #2 #3 #4 #5 #6 #7 #8 #19 #23; issue status must be re-read, as some issue titles are legacy relative to integrated features.

## Inputs and provenance

1. `source/WhereIsItAI_main_cabdb3d2_tree_00c1e20a_source.zip`: full 580-file source tree from previously security-reviewed GitHub Actions offline artifact ID `13029884531` (run `41169239686`, actual source commit `e87e06f3c6b1921bdcce232ee44f0c3028ca9d95`). Its **tree SHA matches latest main exactly**; this makes it a complete byte-exact source tree snapshot despite a different commit SHA/history. Verify inner sha256 in `STATE.json` and Git tree via GitHub API. No Git history included in archive; GitHub remains the history source.
2. `source/WhereIsItAI_PR57_draft_93d7e525_source.zip`: exact source of PR #57 from offline artifact ID `13220324321`, run `41616027525`, commit 93d7 and tree 4f09. Separate, not approved for main.
3. A companion Drive file `WhereIsItAI_RC3_privacy_gated_offline_kit_13029884531.zip` holds runnable Linux binaries, source.zip, CPython3.13 DuckDB wheel, manifests and licenses. SHA-256 `ba712598b419f98dfc3e7ffb17c268959f15fe1bb6729aecd0d465cc997074b6`. This runner corresponds to source commit e87 with tree identical to current main. It is not a signed Windows/Mac installer.
4. Official Windows/macOS RC3 installers and manifest: https://github.com/i-shutov/whereisit-ai/releases/tag/v1.0.0-rc.3 ; note tag 5d91 **precedes** late privacy gate in main.

## Status and truth policy

- Full genuine 44-disk / 3,046,027-entry CTF verification was run on Linux exact CLI at `fedc833...`; results: byte-identical unchanged roundtrip, all 15-field CSV/Parquet 3M parity, 12,664 selected export rows, 32,757 duplicate candidate export, annotation CAS/restore; source stays read-only. The aggregated redacted JSON in `refs/main/docs/qa/...` is the evidence, not the private input or raw local output.
- 150 ms p95 SLO **not achieved**, three p95 numbers 218.60/209.82/252.83 ms, 10 samples each. No physical Win/Mac user-experience acceptance.
- Historical `e-books.CTF` and `Mp3-60Gb.CTF` were submitted but source provider denied byte retrieval; do not call them absent or invent new measurement.
- Unknown opaque `0x10` extension is preserved, not decoded/spec-guessed. Numeric `1,782` is NOT proven lost descriptions.
- Green CI is necessary but insufficient for stable release. Final authoritative acceptance file declares `stable_ready:false`.

## CI matrix (verified remotely)

- Main exact-head: native `41171910671` 4/4 PASS, installed RC2→RC3 `41171910713` 3/3 PASS, integration regression `41171910729` PASS, candidate publisher `41172238335` success.
- Draft PR57 exact-head: native `41616027549` 4/4 PASS, upgrade `41616027556` 3/3 PASS, integration `41616027540` PASS, offline `41616027525` PASS. PR57 remains draft and has not been merged.
- All these runs are historical: recheck live HEAD, workflows, artifacts, PRs and failures before additional changes.

## Privacy and engineering boundaries

- Do not commit or upload any private original CTF, derived owner SQLite, PII paths/names, private SHA-256, user license/keys, network credentials or signed binaries from private machines.
- Core CTF source read-only, derived caches content-pinned. Annotation data durable in separate user-owned store; CAS, reconciliation, no-clobber and fail-closed conflict identity are required invariants.
- The publicly present checker covers fixed allowlisted redacted evidence and sentinels; it is not a complete historical public Git audit.
- Codex agents, PRs and automations are **not** interchangeable. The current WhereIsIt scheduled watchdog is enabled but **read-only**; it neither deploys releases nor writes commits without its own execution mode/authorization.

## Next execution choice

Finish one high-impact QA/development batch: review PR57 source + 880/1100/1180/1366 screenshot atlas with mouse-click travel measurements, physical Win10/11 and Mac USB identity controls, focus/a11y; then decide integration. Independently resolve modified-CTF original-app and historic CTF source access. Do not make a stable 1.0 release until all hard gates are evidenced.

## External links

- GitHub repo: https://github.com/i-shutov/whereisit-ai
- Main commit: https://github.com/i-shutov/whereisit-ai/commit/cabdb3d2b139a67729a795ba9c34e4d3144c9c9f
- PR57: https://github.com/i-shutov/whereisit-ai/pull/57
- RC3 tag: https://github.com/i-shutov/whereisit-ai/releases/tag/v1.0.0-rc.3
- Release acceptance: https://github.com/i-shutov/whereisit-ai/blob/main/release/ACCEPTANCE.json
- Main handoff: https://github.com/i-shutov/whereisit-ai/blob/main/HANDOFF.md
- Real-data aggregates: https://github.com/i-shutov/whereisit-ai/blob/main/docs/qa/RC3_REAL_DATA_REDACTED_20261031.json
