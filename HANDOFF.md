## UX1 native loaded-page sorting + private locator preference — 2026-11-01

Same draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57): native Windows column headers and macOS accessible picker/direction toggle sort only each currently loaded result page (max 250). Selection is retained by catalog entry ID; missing numeric metadata appears last. The label **current page only / PAGE ONLY** prevents a false global-sort claim. Windows saves bounded splitter width and visibility in a versioned, private UI-layout JSON; macOS uses UserDefaults for locator visibility. No CTF, source SHA, private search term, catalog DB or physical pairing is stored in these preferences. Synthetic pure Rust and cross-platform source-contract tests added. Exact new SHA CI must pass before green claims; human Win10/11/Mac Intel/ARM GUI/accessibility and unplug/replug QA is still outstanding. Never merge unverified draft or publish stable. [Manual QA](docs/qa/UX_NATIVE_PANE_REVIEW_20261101.md).

---

## UX dense native layout and collapsible disk locator — 2026-11-01

Draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57) continues on branch `ux/search-first-disk-locator-20261101` from the previously exact-HEAD green `7b78af89f88df64dc4bbd79eb01eed0b2ba2e760`. This integrated iteration adds an actual Win32 mouse-resizable disk locator with min-width/file-table guards, a visible native splitter grip and Ctrl+I/View collapse; macOS SwiftUI adds an accessible Command-Shift-I inspector toggle to its already-resizable HSplitView. The compact Windows result columns now preserve the recorded Path when the inspector is hidden even on 880px windows. Neither toggle changes search scope, physical verification, catalogue selection or CTF bytes. See [manual layout runbook](docs/qa/UX_NATIVE_PANE_REVIEW_20261101.md).

**Validation state at commit preparation:** new pure Win32 geometry/hit tests, native UI source-wiring regression, Linux/Windows/macOS native builds and upgrade gates must all be confirmed on the resulting exact commit SHA before marking this block green. No owner screenshots or physical USB acceptance claimed. PR remains draft/unmerged. Stable remains blocked by the known modified/historical CTF, device, platform GUI, signing/notarization and real p95 requirements.

---

## UX1 integrated disk-register filtering — 2026-11-01 (exact-HEAD CI pending)

Continued the same draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57)
with a cross-platform native label/catalog-ID quick filter for ~40 mostly
unplugged USB disks. Windows Ctrl+L and macOS Command+L focus the register
filter, independently of Ctrl/Command+F global file search. Windows maps
filtered list rows back to stable original catalog IDs; hidden selection does
not silently switch search scope or physical pairing. Synthetic Windows Rust
mapping tests and Python dual-native wiring regression added. Runbook:
[register filter / search and pairing](docs/qa/UX_GLOBAL_SEARCH_AND_PAIRING_PICKER_20261101.md).
Check **new exact-HEAD** Windows, macOS ARM64/Intel, Linux, offline kit and
upgrade workflows before claiming PASS; manual real 1366×768/HiDPI/a11y and
USB plug/unplug remain unaccepted. CTF stays read-only and SHA pinned; no
owner raw CTF/SQLite or path is published. PR remains draft, no merge or stable.

---

## UX1 + UX2 integrated scope and pairing picker — 2026-11-01 (CI pending)

Draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57) now makes Search/Enter and broad Duplicates global by default on both Win32 and SwiftUI, even after disk browsing. This disk is an explicit narrow search; native result context displays the applied scope. The Windows picker and macOS SwiftUI picker exclude simultaneously duplicate live OS UUIDs from pairing choices, **without filtering the observations sent to Rust**, so physical_status still fails closed on conflicts. This does not prove physical hardware, content identity or visual UX acceptance. [Runbook](docs/qa/UX_GLOBAL_SEARCH_AND_PAIRING_PICKER_20261101.md). Recheck final exact-HEAD native CI, upgrades, integration and offline kit; no PR merge, stable release, private artifacts or other-repository changes.

---

## UX2 physical OS-volume pairing candidate — 2026-11-01 (CI pending)

PR #57 implements a shared fail-closed user SQLite binding service plus real
Win32 Mount Manager GUID/serial enumeration and macOS mounted filesystem UUID
lookup, opt-in per-catalog-disk pairing/unpairing, six-second live reprobes and
registered-disk/locator states. Explicit user pairing is required; old drive
letters cannot verify presence. Absent ID after successful scan = offline;
unsuccessful scan, missing binding, generation change or collision never show
connected. USB cloning/host-specific ID caveats are documented at
[UX2 runbook](docs/qa/PHYSICAL_USB_IDENTITY_WP_20261101.md). This is source
integration, NOT manual hardware/GUI acceptance. Inspect **final exact-HEAD**
Windows, macOS ARM/Intel and Linux CI, upgrade and offline kit before asserting
success. No stable release or PR merge is authorized.

# WhereIsIt AI — UX implementation handoff (2026-11-01)

**UX3 implementation checkpoint — 2026-11-01 (exact-head CI required):** On [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57), the native Win32 locator offers **Find copies**/Ctrl+D, and SwiftUI offers **Find copies**/Command+D. Both navigate from the current selected file to paginated candidate locations across **all catalogued disks**, with back-to-results. Service operations `copy_candidates` and SHA-pinned `ctf_copy_candidates_pinned` derive normalized name + exact byte size from selected **entry ID**, count catalog entries and distinct volumes, and explicitly return `verified:false`. Linked CTF SHA is verified afresh on every page, without writing source/mirror. Synthetic SQLite unit and two-disk pinned CTF source-change regression + native Python wiring regression were added. Results are **metadata candidates, not content verification or physical connection evidence**. Stable physical USB presence remains NOT VERIFIED and is the next gate. First exact-HEAD CI at `6cdf57c97d56c8396b57f6fe2ef29f07fbfcb625` failed ONLY Rustfmt step in [native run 41305803121](https://github.com/i-shutov/whereisit-ai/actions/runs/41305803121) and [offline run 41305803164](https://github.com/i-shutov/whereisit-ai/actions/runs/41305803164); their exact rustfmt patch was applied in the same PR, not tests disabled. Recheck the latest branch HEAD before reporting green, and keep PR draft/unmerged until full acceptance. Prior real 44-volume CTF acceptance is not synthetic and is not re-claimed for this new view. No private raw inputs were uploaded.


**OWNER DECISION:** Replace legacy 2000s-style UI with a search-first, dense and ergonomic native tool for approximately 40 mostly offline USB disks. Main jobs: find a movie/file, identify exactly which labelled physical disk to retrieve, and compare metadata duplicate candidates across disks. [Current UX plan](docs/UX_OFFLINE_USB_SEARCH_2026-11-01.md) is authoritative for the upcoming UI work. The concept screenshots are **design references, not deployed functionality**.

**Live baseline:** at branch creation `main=cabdb3d2b139a67729a795ba9c34e4d3144c9c9f` (after PR #54 and #56); always recheck main/PRs/CI, do not use the old e8a16... base. One integrated UX branch `ux/search-first-disk-locator-20261101`, [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57). Do not create a competing micro-PR stack or modify frozen release/finalization branches.

**Implemented in PR #57:** selected-result disk locator in Win32 (native read-only/copyable inspector, redraw on selection, collapse below 1120px; unit tests) and SwiftUI (resizable inspector, copy label/path, browse disk root, search all disks); both clearly state **physical connection not verified**. Disc identity, copied paths and candidate counts are catalog evidence, not physical mount verification/content hashes. Search table made compact by moving secondary fields into inspector. New Python source-wiring regression is included in existing native CI. Plan + native UI docs updated. **No user CTF/SQLite uploaded.**

**Exact-HEAD validation:** inspect current GitHub Actions on PR #57, especially Linux fmt/test/Clippy, Windows installer, macOS ARM64 and Intel native builds, installed upgrade and offline kit. First native CI run #41298743486 failed ONLY `cargo fmt --check`; corrected formatting in same PR, then advanced work on the compact file table. Report only actual final exact-HEAD run results; do not call queued/cancelled jobs PASS. Manual visual/keyboard 1366×768/HiDPI GUI QA remains pending.

**Next integrated development:** make global search the primary focus; verify stable physical media identity (not historical drive letter), explicitly report verified online/offline/unknown states; integrate cross-disk duplicate navigation and safe metadata-only copy comparison, followed by native accessibility/keyboard QA and SLO performance improvements. Preserve the 44-disk real CTF acceptance evidence already in the RC3 handoff below; do not mislabel it synthetic. Public CI stays synthetic. No faux poster thumbnails, media decoding, hash-verified matches or deletion features merely because they appear in a visual mockup.

**Watchdogs:** existing WhereIsIt engineering task re-enabled for UX implementation; separate hourly CI QA task is **read-only** to avoid concurrent branch writes. Both MUST only touch/check `i-shutov/whereisit-ai`, not FlowBridge. Keep PR regression/handoff current and avoid changing stable release without full acceptance.

**Stable 1.0 remains blocked** by original-WhereIsIt reading a genuinely modified CTF, two historical CTFs raw-byte availability, hands-on Windows 10/11 and macOS ARM64/Intel GUI+upgrade, Authenticode/Apple notarization and measured p95 150ms (last real p95 209.82–252.83ms). RC3 is an unsigned prerelease. Linked CTF read-only, per-use generation SHA, caches derived, independent user annotation DB, ambiguous matches conflicts, opaque 0x10 not presumed lost descriptions.

---

# WhereIsIt AI — integrated RC3 handoff (2026-10-31)

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
