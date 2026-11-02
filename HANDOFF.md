## UX11 — native whole-volume OS scan, fail-closed (2026-11-02; exact HEAD CI pending)

In the same draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57), resolved a physical disk trust defect: letter-only Win32 enumeration could falsely call an explicitly paired, live letterless/folder-mounted USB volume **offline**. The native Windows shell now enumerates OS volume GUIDs, derives GUID+serial identity, shows available current mount-point hints and checks complete enumeration/close. SwiftUI now includes hidden local mounts; incomplete UUID scans remain unverified, never false offline. A live OS UUID and explicit per-generation pairing prove only OS volume presence, not manufacturer hardware identity, recorded file presence, hash or metadata-copy equality. Windows pure unit + dual-native wiring regression and [UX11 manual runbook](docs/qa/UX11_ALL_VOLUME_PROBE_20261102.md) added. Recheck exact-HEAD Linux/Windows/macOS ARM/Intel, integration/offline kit and RC2→RC3 upgrades before declaring green. Manual hardware/GUI, original modified/historic CTF, signing/notarization and real p95 remain blockers. PR draft/unmerged, RC3 unsigned prerelease only.

---

## UX9 exact-HEAD CI repair (2026-11-02)

Initial UX9 [SHA 6649bb3](https://github.com/i-shutov/whereisit-ai/commit/6649bb33db9cb4ec19da86232735bdafa9647207) failed [offline acceptance](https://github.com/i-shutov/whereisit-ai/actions/runs/41361019201) at **Check integrated Rust contracts → cargo fmt --all -- --check** (Rustfmt-only formatting of the new Windows shortlist helper, test and button layout). Fixed the reported diff **in the same draft PR #57**, no tests disabled. Recheck all new exact-HEAD jobs before green status; earlier pending workflow results cannot certify a later SHA.

---

## UX9 — native offline USB pickup checklist (2026-11-02)

In the SAME draft PR #57, a selected file's cross-disk name+size candidates now
produce a locally copied **all-disks pickup checklist** on Win32 (button, View
menu, Ctrl+Shift+P) and SwiftUI (button, Command-Shift-P). It lists the recorded
file name and all per-disk candidate counts (not just the visible 250-row page),
exact catalog IDs, owner-only sticker/shelf hints, historical labels and last
OS-checked pairing states. Candidate pickers are now ordered by last verified
connected, paired offline, unverified, identity conflict; within each category
by metadata count descending and exact ID. Win32 maps sorted combo rows back
to exact IDs even after live OS refresh; Mac uses the same order in accessible
scrollable candidate buttons. No USB mount is inferred from a historical path
or personal sticker. No actual file bytes or hashes have been compared; no
automatic deletion; clipboard-only with no catalog or sidecar writes.
Rust unit tests exercise duplicate labels, Cyrillic tags, failed/conflicted
states, stable ID ordering and newline-resistant clipboard text; native source
wiring regressions and [UX9 QA](docs/qa/UX9_PHYSICAL_PICKUP_LIST_20261102.md)
cover keyboard and hardware checks. **Verify new exact-HEAD CI before claiming
green.** The earlier real 44-disk CTF acceptance has not been re-run for this
feature. Manual USB hardware, GUI, p95 and signing remain open. PR stays draft,
no merge or stable release.

---



## UX8 — candidate disk shelf/sticker retrieval + stale-response guard (2026-11-02)

Implemented in the same draft PR #57 on Win32 and SwiftUI: every per-disk
name+size candidate chooser identifies the **exact catalog disk ID** before its
historical label and displays the **owner-entered sticker/shelf tag** (or
explicitly says none is assigned), the metadata candidate count and the
independently checked OS presence/conflict/unverified state. This closes an
important physical retrieval gap: a candidate "Archive" label alone could
correspond to several of the ~40 USB disks. It does not infer file content
equality, USB identity from a sticker, current paths, or safe deletion.

Windows updates the chooser immediately after a tag is saved/refreshed and
unit-tests Cyrillic stickers, duplicate labels and unverified/conflict states.
macOS uses VoiceOver-specific descriptive labels and prevents an older
asynchronous shelf response from overwriting a new catalog scope or user tag.
Cross-client source-wiring regression covers both native code paths.
Manual hardware and visual acceptance remains outstanding; instructions:
[UX8 QA](docs/qa/UX8_CANDIDATE_DISK_SHELF_LOCATOR_20261102.md).
The **new exact-HEAD CI results must be checked** before declaring this block
green. PR stays draft/unmerged; no stable 1.0, no private data in git.
## UX7 — selected file to exact catalog disk in register (2026-11-01, new exact-HEAD CI required)

Same draft PR #57 adds a source-backed retrieval shortcut: Win32 Show disk / Ctrl+J and SwiftUI Show disk in register / Command-J isolate the selected current result's exact existing catalog disk ID inside the left register of approximately 40 mostly unplugged disks. The register text filter becomes id:<catalog-volume-ID>, an exact numeric key; label, private shelf tag and partial-number collisions cannot resolve the wrong disk. Current file query/selection/page, cross-disk copy candidates, and independent OS-paired state remain unchanged. Clicking the now-visible disk is a separate intentional browse action, not a live USB filesystem access.

Native Rust row-to-disk and ID collision tests, two-frontend Python source regressions, and docs/qa/UX7_EXACT_DISK_REGISTER_REVEAL_20261101.md cover safety, accessibility and manual steps. Recheck new exact-HEAD Linux fmt/tests/Clippy, Windows and Mac ARM64/Intel native builds, integration/offline kit and installed upgrades before a PASS claim. This does not verify physical hardware, file contents, visual ergonomics, or p95 search. Linked CTF stays generation-pinned read-only. PR stays draft/unmerged; stable 1.0 blocked. No other repositories modified.

---

## UX6 exact-HEAD Rustfmt failure repaired (2026-11-01; recheck new HEAD)

The initial UX6 SHA [27edf20](https://github.com/i-shutov/whereisit-ai/commit/27edf20d783744eeadc36493de9d067f4da9abf9) failed [offline-kit run 41345029231](https://github.com/i-shutov/whereisit-ai/actions/runs/41345029231), step **Check integrated Rust contracts**, at `cargo fmt --all -- --check` in `apps/windows/src/native.rs`. Formatting-only differences (three test assertions, one Win32 SendMessageW invocation and match-arm comma) were applied in the SAME draft PR, without suppressing any check. No conclusion for the revised HEAD or other earlier pending workflows is implied; inspect the NEW SHA's Linux/Win/Mac ARM+Intel native CI, offline, integration and installed upgrade workflows. No stable publication or merge.

---

## UX6 — physical retrieval note and cross-platform shelf-tag search (2026-11-01; exact-HEAD CI required)

The same draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57) now gives the user a **local, explicit Copy locator text** action in Win32 (button/View/Ctrl+Shift+C) and **Copy retrieval note** in SwiftUI (accessible Command-Shift-C). The note names the **catalogued** disk, manually assigned physical sticker/shelf tag, recorded relative path, metadata candidate count and the *last observed* OS-pairing state, with explicit warnings that an OS mount does not prove file contents or current presence after the scan. Neither client traverses a mount nor writes CTF/SQLite/annotations when copying. The Windows native register can now find a disk by its user-entered sticker/shelf tag (case-insensitive Cyrillic), matching the existing Mac register; its filtered-row ID mapping and global search scope remain unchanged. New Windows unit regression, dual-native source-wiring test, and [UX6 runbook](docs/qa/UX6_RETRIEVAL_NOTE_20261101.md) cover the feature.

Validate **new exact-HEAD** Linux Rust fmt/Clippy/tests, Windows installer, macOS ARM64/Intel compiled native apps, integration, offline kit and installed RC2→RC3 upgrades. This is synthetic CI/native build coverage, not owner 1366×768, USB hardware or accessibility acceptance. Physical UUID clones, original modified/historical CTF, real p95 150ms, signing/notarization remain unresolved; PR stays draft, no merge or stable. The previously verified real 44-volume / 3,046,027-entry evidence is separate and not rerun here. Do not publish private source data.

---

## UX5 SwiftUI release compiler repair and register-only tagging (2026-11-01; exact new HEAD CI required)

The exact-head macOS ARM64 native build on [run 41343297951](https://github.com/i-shutov/whereisit-ai/actions/runs/41343297951) failed at SwiftUI compiler type-checking DiskLocatorInspector.body (not a Rust/CTF regression); Linux Rust/Clippy/rustfmt and Windows native jobs passed. In this same draft PR #57, split the complex SwiftUI body into selected-file, shelf editor and OS-mount subviews; use macOS 13-compatible onChange overloads; let operators tag a disk selected in the left register without requiring file selection; clear stale shelf labels eagerly upon CTF scope switches on both native clients. Native source-wiring tests, manual QA and this handoff updated. All new exact-head Linux/Windows/Mac ARM/Intel, offline, integration and RC2→RC3 upgrade CI must run again; no PASS claim on pending runs. CTF remains read-only/generation-pinned, all shelf tags are private and never prove USB presence. No merge or stable.

---

## UX5 — private offline USB sticker/shelf tags (2026-11-01, exact-head CI pending)

In the SAME draft PR #57, a new integrated native retrieval feature allows explicitly saving/clearing a case/sticker/shelf label for one catalog disk (Win32 sidebar, SwiftUI selected-file inspector). Both native disk registers and locator show it; the Mac register filter also matches it. User-owned physical sidecar only, independently keyed to exact source generation/platform/catalog ID; no CTF, original catalog, mirror, pairing, candidate or annotation writes. Rust service rejects unconfirmed/invalid labels, tests Unicode and generation isolation, and shows that manually entered labels never make USB presence verified. New source-wiring regressions and [UX5 QA](docs/qa/UX_SHELF_TAGS_20261101.md).

Recheck final exact SHA on Linux/Windows/macOS ARM64/Intel native CI, offline, integration and installed RC2→RC3 upgrades; repair any failures in the same PR, no test disabling. No human hardware/GUI acceptance, real p95 150ms, original modified or provider-blocked historic CTF checks, signing/notarization. Keep PR draft/unmerged, RC3 prerelease, no stable. No other repository was touched.

---

## UX4 OS-verified candidate disk focus — 2026-11-01 (new CI required)

Draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57) now attaches
fail-closed live OS pairing states to each catalogued copy-candidate disk,
with a fresh-probe, exactly-one-connected shortcut (Win32 Ctrl+G/View; macOS
Command+G/accessibility button). Zero/multiple candidates are never guessed.
Win32 repaints candidate selector after USB changes; SwiftUI protects async
probe response with generation/source/entry-ID checks before focusing. This
only navigates *metadata name+size candidates*, not file existence, content
hashes or hardware manufacturer's unique identity. Synthetic Rust/pure Windows
and dual-native Python wiring regressions, QA runbook and UX spec updated.
The new HEAD requires exact-HASH native Linux/Windows/Mac ARM64/Intel CI,
offline, integration and installed RC2→RC3 checks before a green claim.
Read-only CTF/source SHA pinning, private annotation and pairing stores remain
unchanged. No merge or stable; real device/GUI, modified/historic CTF, p95
150ms, signing and notarization remain blockers. No other repo edits.

---

## UX3 integrated disk-level copy focus — 2026-11-01 (new CI pending)

Existing draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57) now
implements an all-matches, per-disk candidate rollup plus an optional
catalog-volume focus (one selected file -> copies by source catalog disk).
Win32 has a native keyboard-operable per-disk combobox; macOS SwiftUI has
horizontally scrollable, VoiceOver-labelled disk buttons. Counts are global
across exact normalized name+recorded size, independent of the current 250-row
page. Focused pages report filtered `total` separately from `all_total`;
Back navigation and source entry ID are preserved. The service takes an
optional strictly positive `candidate_volume_id`, returns `disk_groups`
and `source_volume_id`, and checks linked CTF generation SHA anew for every
page. Synthetic Rust SQLite and pinned two-disk CTF tests cover rollup,
focus, freshness rejection and source/mirror byte invariance; native source
wiring tests and [UX3 manual runbook](docs/qa/UX_GLOBAL_SEARCH_AND_PAIRING_PICKER_20261101.md)
are updated. No content hashes or file equality claims: **candidates are
metadata-only; disk labels do not prove USB is currently connected**.

After final documentation commit, inspect exact-HEAD native Linux/Windows
10/11 installer candidate and macOS ARM64/Intel CI, offline kit, native
integration and installed RC2->RC3 upgrade. Fix failed jobs in this draft PR;
no merge or stable release until all existing hardware/manual GUI, original
modified/historical CTF access, real p95 150ms and signing/notarization gates.
User data and CTF remain private/read-only with SHA pinning. No other repo.

---

## UX2 integrated OS snapshot completeness — 2026-11-01 (exact-HEAD CI pending)

Draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57) adds a
fail-closed native USB identity recheck across Windows Win32 and macOS SwiftUI.
Windows now marks unknown/disappearing GetDriveTypeW roots as incomplete (not
offline), and Mac ignores remote mounts but treats any unreadable local UUID
as an incomplete scan (not a false "offline"). Ctrl+R / Command+R explicitly
refresh current OS observations, without changing global search; Mac displays
an accessible failure indicator. Rust classification and Win32 classifier tests,
dual-native source wiring checks and the physical QA runbook are updated.
Do not assert CI success until **new exact-HEAD** actions pass 4-platform native,
offline kit, integration and installed RC2→RC3 upgrade. Real hardware,
Win10/11/Mac ARM/Intel GUI/a11y, historical/original CTF, p95 and signatures
remain release blockers. CTF read-only/source SHA pinning and user-owned
binding database are unchanged; no private data published. No merge/stable.

---

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


## 2026-11-02 — UX10 native async page freshness / page-path clipboard candidate

Branch: draft PR #57, current exact parent 3f31060a96375e3f1f50491c05d1c87a5f15a981; no main merge or stable release.
Native Win32 and SwiftUI clear old table rows and invalidate selection at each new async search/candidate-page request; latest-page-only readiness gates copy/verified-focus actions. Copy page paths (Ctrl+Shift+M / Command-Shift-M) produces only a bounded current-page manifest with exact catalog disk IDs, historical paths, total/page range and source-verification disclaimers. Matching metadata never means byte equality or permission to delete. Original CTF is read-only, per-page linked SHA pinning preserved. Tests and UX10 runbook added. Final SHA and verified Actions URLs must be recorded in PR #57 after exact-HEAD CI; this commit is not a claim of hardware GUI acceptance. PR stays draft; RC3 remains unsigned prerelease and stable 1.0 is BLOCKED by original modified/historic CTFs, manual GUI/hardware/signing and real p95 target.


## 2026-11-02 — UX12 same-sticker ambiguity integrated in draft PR #57

The user-owned shelf/sticker DB now reports ID-only label collisions scoped to
the current catalog generation, platform and requested disk IDs. Native Win32
and macOS SwiftUI surface explicit SHARED STICKER warnings throughout the disk
register, candidate disk selection, locator and local pickup note; macOS
includes screen-reader labels and scoped post-save recheck. Duplicate labels
remain legal and are NEVER evidence of a connected USB volume or equal bytes.
CTF remains read-only and SHA-generation pinned, annotations/caches separate;
no private tags or sources enter CI. Synthetic service/unit and native wiring
checks plus manual QA runbook: docs/qa/UX12_SHELF_COLLISION_AUDIT_20261102.md.
Check final exact-HEAD workflows before PASS claims. No merge or stable 1.0;
original/historic CTF, real Win10/11/Mac GUI/USB, notarization/signing and
p95<=150ms remain open.
