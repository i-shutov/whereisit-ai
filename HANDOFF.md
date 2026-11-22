## Resume checkpoint — 2026-11-22 — Windows 1.0 stable released

Windows x64 [v1.0.0 stable](https://github.com/i-shutov/whereisit-ai/releases/tag/v1.0.0) is published/latest, unsigned with explicit owner approval. Source `938fece2d74793c2eda7a1b84082ab5db3f8a092` (PR #85); [native CI](https://github.com/i-shutov/whereisit-ai/actions/runs/44999900277), installed Windows GUI acceptance, data-preserving RC4 upgrade/rollback and [publication](https://github.com/i-shutov/whereisit-ai/actions/runs/45012028164) passed. All four public assets were downloaded and verified; protected original/external controls remained unchanged. `stable_ready=true`; release tags/assets stay immutable. USB pairing/bound-device rescan is disabled; ordinary metadata scanning remains supported.

**Current plan: [docs/ROADMAP.md](docs/ROADMAP.md).** Next functional release is planned Windows 1.1: PR58 tabs/workspaces, sunburst/archive slices and local analyzer, prototype-first then Rust/egui. macOS and Linux are separate release tracks without assigned version/date, while portable code and cross-platform checks continue. Do not reopen Windows 1.0 gates for these deferred features/platforms. Windows 10 evidence remains owner-confirmed historical testing, not a new exact-1.0 result. Private catalogs, paths, licenses and raw GUI evidence stay outside public Git.

All checkpoints below are historical; their NO-GO/pending statements do not override the released Windows 1.0 or the current roadmap.

---

## Historical checkpoint — 2026-11-21 — Win11 native QA release plan

Current live baseline: `main ddd9215b0926c5e74482a3cbf7f10c229a66fb88`, PR #68 merged; postmerge native CI #459 and RC2→RC3 upgrade #250 are PASS. PR #69 (PR58 Rust/egui parity) and PR #70 (trusted manual Windows 11 runner smoke) are separate drafts. New Windows 11 Pro hardware is provisioned but actual runner Online/Idle, trusted workflow smoke and interactive native GUI tests are **not** equivalent and have not been accepted. Full stable v1.0 remains NO-GO.

**Authoritative next action order:** [Win11/Windows 10 + macOS 1.0 action plan](docs/RELEASE_1_0_WIN11_ACTION_PLAN_2026-11-21.md). Keep the IP, credentials and private CTF evidence outside the public repository. Do not run untrusted PR code on the administrator-owned self-hosted runner, touch other projects' services, or mark USB physical QA PASS (owner deferred it for the current Windows test).

---

## Resume checkpoint — 2026-11-17 — R2/R3 merged, R4/R5 QA pending

The owner-selected [PR #58](https://github.com/i-shutov/whereisit-ai/pull/58)
remains the **canonical interactive UX design**. Main now includes the real
Rust/egui 0.37, Rust 1.100 desktop preview (R2 PR #62, merge `81f04912`)
and full-scope SQLite CSV/JSON/Parquet no-clobber export
(R3 PR #63, merge `405b4ccb`). **This is not a stable 1.0 release.**

Source evidence: R3 exact-head native Linux/Windows/macOS ARM+Intel CI
[44035800572](https://github.com/i-shutov/whereisit-ai/actions/runs/44035800572),
installed RC2→RC3 upgrades [44035800600](https://github.com/i-shutov/whereisit-ai/actions/runs/44035800600),
and offline QA [44035800563](https://github.com/i-shutov/whereisit-ai/actions/runs/44035800563)
all PASS. Source commit `405b4ccbc534db73cc6f986b77c8ba6e1b5a5e9e`.
R4 scan/progress/cancel [PR #64](https://github.com/i-shutov/whereisit-ai/pull/64)
and R5 side-by-side Win/Mac egui QA preview ZIP packaging
[PR #65](https://github.com/i-shutov/whereisit-ai/pull/65)
remain draft while full exact-head CI completes. Manual physical
Windows 10/11 and macOS ARM/Intel GUI tests are [issue #66](https://github.com/i-shutov/whereisit-ai/issues/66).

For current release truth use [gate matrix](release/RELEASE_1_0_GATE_MATRIX_20261116.json),
[master plan](docs/RELEASE_1_0_MASTER_PLAN_2026-11-16.md)
and [R3/R4/R5 status checkpoint](docs/checkpoint/2026-11-17/RELEASE_STATUS.md).
Private historical CTF original-app compatibility is deferred until complete
application assembly, not marked PASS. No source CTF/SQLite bytes in public CI,
no silent data removal, no trust in metadata-only duplicate equivalence.
`release/ACCEPTANCE.json: stable_ready=false`, immutable RC3 release tag.

---

## Resume 2026-11-16 — UX consolidation and egui architecture

Before continuing, read [integration audit](docs/RELEASE_1_0_INTEGRATION_AUDIT_2026-11-16.md),
[release plan](docs/RELEASE_1_0_EXECUTION_PLAN_2026-11-16.md), and
[ADR-0004](docs/decisions/ADR-0004-egui-desktop.md). PR #58 React/Express in-memory
reference prototype is merged; it is *not* the Rust/egui native app. PR #57 native
Windows/macOS locator and physical identity implementation is brought into the
integration code tree preserving newer 2026-11-13 handoff/checkpoint. The latest
main merge and native exact-SHA CI must be verified independently after integration.
No manual real USB/UI acceptance, CTF edited-original-app proof, signing/notarization
or 150ms real SLO waiver is asserted. Keep `stable_ready=false`, RC3 tag immutable.

---

# WhereIsIt AI — архивный handoff (checkpoint CP-2026-11-13)

## Historical resume point (2026-11-13; superseded above)

Historical `main` at that checkpoint: `cabdb3d2b139a67729a795ba9c34e4d3144c9c9f`
(tree `00c1e20ab5779475610275e1292cd2c5e333b618`). Identity `1.0.0-rc.3`,
`release/ACCEPTANCE.json` keeps `stable_ready: false`.

Integral checkpoint materials (reports, evidence, gates, verification):
[`docs/checkpoint/2026-11-13/`](docs/checkpoint/2026-11-13/00_START_HERE.md).
Start with `reports/NEXT_STEPS_AND_GATES_RU.md` and `VERIFY.md` in that folder.

Published tag `v1.0.0-rc.3` points at `5d91ba12…` and **precedes** the later
privacy-publication gate merged via PR #56 into current `main`. **Do not move
that tag.** Newer installers must be built from an exact later SHA.

Draft PR #57 (search-first / physical USB locator), HEAD
`93d7e525dc0433b6790a99250030c9c7fe09ca9a` on branch
`ux/search-first-disk-locator-20261101`, has green CI but **is not merged** and
has no manual Win/Mac USB GUI acceptance. Work that UX on its branch; do not
substitute it for `main`.

Local Linux offline kit (binaries, DuckDB wheel): sibling directory documented in
[`docs/checkpoint/2026-11-13/OFFLINE_KIT.md`](docs/checkpoint/2026-11-13/OFFLINE_KIT.md).
Not a git artifact.

---

## Historical: integrated RC3 handoff (2026-10-31)

## Continue the integrated product, not the old micro-PR plan

Integration PR: https://github.com/i-shutov/whereisit-ai/pull/54
Branch: `release/finalization-20261031`, targets `main`.
The entire #37–#53 ancestry is incorporated without rewriting those branches.
Read the live refs and PR54 discussion before doing any work; do NOT restart at
PR34, at the old `e8a16abd...` main, or reopen a separate stack of tiny draft PRs.
The owner requested project finalization. Merge only verified exact source;
release only an explicitly unsigned prerelease while the hard gates below remain.

## Actual executed evidence (not a planned checklist)

`docs/qa/RC3_REAL_DATA_REDACTED_20261031.json` is an allowlisted report from a
**genuine owner CTF**: 44 volumes, 3,046,027 entries, unmodified external source.
Runtime tested: `fedc833b19c69428ad3f22d4fa2e2c0a9582a07a`, RC3 CLI.
Full import/integrity, counted-vs-fast first and second pages, full export and
CTF→SQLite→CTF were executed. Independent DuckDB comparison matched all
3,046,027 CSV/Parquet rows and all 15 fields. Persistent linked-service testing
matched 12,664 selected CSV/Parquet/JSON rows and independently verified all
32,757 metadata-duplicate candidates. CAS conflicts, snapshot restore, stale
source refusal, no-clobber, source and derivative immutability passed.

Real-data raw reports and sources are private and are NOT repository artifacts.
Two historic CTF attachments **were located** in the owner's Library; their raw
materialization is denied by the file provider. Do not report them as never
provided, do not fabricate execution, and do not bypass provider authorization.
The large source was retrieved from the owner's connected Drive for local-only
execution. No CTF, SQLite, source hashes, filenames or private paths are public.

## Finalization fixes

- Distinct RC3 identity across six crates/lockfile/notices, installer and macOS
  bundle build 10003; do not relabel RC2 binaries as RC3.
- Exact PR-head checkout and `git rev-parse HEAD` provenance in native builds.
- Actual installed RC2→RC3 upgrade acceptance on Windows x64/Mac ARM64/Intel.
  Run https://github.com/i-shutov/whereisit-ai/actions/runs/41166647760 was **3/3
  PASS** at `818a0693a7f03707e675bff934d059d3a9af048b`; native CI
  https://github.com/i-shutov/whereisit-ai/actions/runs/41166647758 was **4/4 PASS**.
  Final exact-head reruns are required after subsequent native wiring changes.
- Initial upgrade failure was the QA helper hardcoding RC1; fixed via explicit
  source/target release pairs and checkpoint validation, four regression tests.
- Final UI audit found stale duplicate-export guards and missing JSON options
  despite correct service APIs. Fixed Windows/macOS controls, kept linked CTF
  writes forbidden, pinned macOS export scope before its save dialog. Added
  compiled Windows policy tests and cross-platform source-wiring regressions.
- One-shot preparation/repair workflows are removed from the deliverable tree.
  Concurrent offline-wheel packaging and documentation commits were preserved.

## Reproduce privately

Download the exact source/binary offline acceptance kit from PR54/main Actions.
Verify `SOURCE_COMMIT.txt` and `SHA256SUMS.txt` before execution. Install its
pinned DuckDB wheel offline as described in the offline QA documentation. Use
an owner-selected local `.CTF` path; keep it read-only. Run these on Linux:

```sh
python tools/qa/offline_ctf_v3.py --cli /local/bin/atlas \
  --output /private/new-full-report --full-export --verify-export-parity \
  /private/source.CTF
python tools/qa/linked_candidate_acceptance.py --cli /local/bin/atlas \
  --output /private/new-linked-report.json /private/source.CTF
```

Paths above are placeholders, not real owner paths. Never upload raw output of
`offline_ctf_v3.py`: it contains private verification hashes. Only manually
allowlisted aggregates may enter public reports. The linked harness reports
aggregates and removes its temporary derivative catalogs/annotations. These
CLI runs do not prove physical GUI correctness or original-app compatibility.

## Acceptance boundary — keep it truthful

RC3 is an unsigned usable integration candidate, **not accepted stable 1.0**.
Full-source-checked search p95 measured **209.82–252.83 ms**, ten warm samples
per query on the isolated Linux environment; the 150 ms target is not met.
Do not remove per-use generation checks to make the latency number look good.
Remaining gates: original WhereIsIt reading genuinely modified CTF, the two
historic raw inputs, human Win10/11 and Mac ARM/Intel GUI/accessibility and
cancellation checks, real hardware SLO, Authenticode and Apple notarization.
Opaque flag 0x10 is preserved; its semantics are not proven lost descriptions.

## Release/integration completion

Require final exact-head native **4/4**, installed upgrade **3/3**, integration
regressions and offline kit checks. Recheck main and concurrent refs. Merge PR54
with ancestry preserved, then verify main CI and published RC3 assets/checksums.
The existing validated-candidate publisher reacts to successful native main CI
and publishes an explicitly **prerelease** RC tag; it must not publish stable.
Record actual merge, main CI and release URLs in PR54 after they exist. Do not
claim a planned release already exists or equate CI installer smoke with human
GUI acceptance. Do not touch FlowBridge.
