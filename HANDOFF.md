## UX34 exact-HEAD CI repair — Rustfmt-only gate (2026-11-02)

At original integrated UX34 source SHA
`5a33ad3b17e362fb2b44eadce0b72bb03e1af12b`, the
[offline acceptance job](https://github.com/i-shutov/whereisit-ai/actions/runs/41523878738)
failed specifically at **Check integrated Rust contracts** on six reported
formatting-only hunks in the new Windows synthetic tests and two safe-state
call sites. The native integration workflow
[41523878647](https://github.com/i-shutov/whereisit-ai/actions/runs/41523878647)
passed. Applied precisely the Rustfmt diff in the **same draft PR #57**
without disabling or weakening tests. **New exact-HEAD CI must finish** before
any all-green assertion. macOS/Windows human USB and GUI, private CTF,
signing and real search p95 remain release blockers.

---

## UX34 — consistent fail-closed OS status across all retrieval surfaces (2026-11-02)

Draft PR #57, integrated native Windows Win32 + macOS SwiftUI fix. Source
review after UX33 revealed that the dense result row verified the presence
of a non-empty OS-matched ID, while other retrieval UI reused only raw
`connected` service state. Candidate picker, disk register, context inspector,
pickup clipboard, OS status filter and "sole connected copy" could thus present
an unverified volume as OS-verified. Windows now uses one audited ID-and-TTL
state snapshot for all physical retrieval views. Mac central
`physicalState` additionally requires exactly one fresh OS observation
of the verified nonempty filesystem UUID. The shared result and candidate
surfaces must agree; no current file or content-hash assertion is added.
Repaint when verified identity changes even if the status string is identical.

Synthetic Windows compiled regression and dual-native Python integration
contracts added; see [UX34 runbook](docs/qa/UX34_CONSISTENT_OS_STATUS_20261102.md).
**Exact new HEAD CI unverified at source integration:** check native
Linux/Windows/macOS ARM/Intel, offline, integration and installed RC2→RC3.
An immediately previous UX33 installed ARM64 upgrade job was cancelled
before steps and rerun as a transient runner issue; this is NOT evidence
of a product test failure. PR draft/unmerged, RC3 unsigned prerelease;
original modified/historical CTF, real USB and GUI, signing/notarization,
real p95 <=150ms and stable 1.0 remain blocked. No private CTF, cache,
user annotation, paths, hashes or other repositories touched.

---

## UX33 exact-HEAD CI diagnosis and Rustfmt correction (2026-11-02)

At first UX33 complete source+docs SHA
9d21156e0bac1bfd2c67905e41646cd5d4dc4de7,
[offline-kit 41511960752](https://github.com/i-shutov/whereisit-ai/actions/runs/41511960752)
failed specifically in the "Check integrated Rust contracts" step:
cargo fmt --all -- --check reported only four assert_eq! wrapping
differences in the NEW Windows synthetic result badge regression tests
(apps/windows/src/native.rs, around line 317). Native Linux
[41511960636](https://github.com/i-shutov/whereisit-ai/actions/runs/41511960636)
passed Python dual-native contract tests and the workspace Rust tests,
then failed its cargo fmt step for the same exact diff; Windows and macOS
native jobs were skipped due to that gate. Integration wiring
[41511960747](https://github.com/i-shutov/whereisit-ai/actions/runs/41511960747)
passed.

Applied only the four reported Rustfmt changes in the SAME draft
PR #57; no tests disabled or weakened. Entire new exact-HEAD
native Linux/Windows/macOS ARM+Intel, offline, integration and installed
RC2-to-RC3 upgrade CI must be re-evaluated. Real USB/GUI/manual acceptance
and stable 1.0 blockers remain unchanged.

---

## UX33 — OS-paired physical status in ID-first native search results (2026-11-02)

Same draft PR #57: the dense Windows Win32 ListView and macOS SwiftUI Table
now display a compact current-OS badge directly after the exact catalog
disk ID, before historical labels and private shelf/sticker tags.

[OS+] means a unique explicitly paired filesystem identity was matched by
a fresh COMPLETE native OS scan and the physical-status service returned a
verified identity. [OFF] means the explicitly paired identity was absent
from a complete scan. [ID!] is identity conflict. [?] means unverified,
expired, missing identity, unpaired, or incomplete/failed probe.
NO marker proves that a catalogued movie exists on the current mount,
matches bytes or hashes, or can be deleted safely.

Windows rechecks monotonic freshness, repaints ONLY the Disk subitem on
periodic success/failure, USB invalidation and TTL expiry (no ListView
rebuild, no focus or selection loss). A service state string without a
verified physical identity cannot produce [OS+]. macOS computes the same
markers from the existing published TTL-guarded physicalState and checked
verifiedPhysicalIDs, and gives full VoiceOver/Help descriptions.

Synthetic compiled Windows tests cover exact IDs, shared Cyrillic shelf
stickers, failed/stale probes, missing physical identity and four states;
dual-native Python source integration tests cover reactivity, event
invalidation and accessible captions. No per-row filesystem access, no
CTF/derived-cache/private-note mutation, no historical mount inference.
Runbook: docs/qa/UX33_LIVE_OS_BADGES_IN_RESULTS_20261102.md.

**Exact HEAD CI must be independently verified after this commit.**
Required: native Linux/Windows/macOS ARM64+Intel, offline acceptance,
integration regressions and installed RC2-to-RC3 upgrades. PR is draft;
NO merge or stable 1.0. The real 44-disk/3,046,027-record CTF previously
passed documented private acceptance; this is separate from synthetic CI.
Original modified/historical CTF bytes, manual Win10/11/Mac Intel/ARM USB
GUI and accessibility, real p95 <=150 ms, signing/notarization remain
release blockers. FlowBridge/TikZarium and user files untouched.

---

## UX32 Windows live shelf-refresh integration correction (2026-11-02)

After adding exact ID/sticker to the result Disk cell, source review found
that saving a private sticker refreshed the register and inspector but not
the existing Win32 ListView cells. The same draft PR #57 now refreshes only
the Disk subitem after shelf-tag reload, preserving selected entry, keyboard
focus and current page. Shared-sticker conflict captions update immediately.
The Python native wiring regression verifies this behavior. macOS already
uses published shelf values to trigger SwiftUI Table updates. This changes
no CTF or user data storage and makes no physical OS/content claim.
Final exact-HEAD CI must be re-evaluated on the new SHA.

---

## UX32 — exact catalog disk ID + private shelf in native result rows (2026-11-02)

Same draft PR #57: both native main result tables show the exact catalog
disk ID first, followed by a separate user shelf sticker and recorded label.
Duplicate stickers are explicitly flagged and unknown IDs never guessed.
Win32 pure Rust unit tests cover Cyrillic, conflicts, invalid IDs and line
breaks; SwiftUI has VoiceOver full text and Help. Python native-wiring checks
cover both platforms. This changes no CTF, cached data, user annotations,
physical OS state, hashes or file bytes.

Runbook: docs/qa/UX32_RESULT_DISK_ID_SHELF_20261102.md.
Exact-HEAD CI for this commit is PENDING. Older successful CI is not proof
for the new SHA. Manual 880/1366px GUI/HiDPI, OS USB checks, private historic
CTF access, signing and p95 remain open. PR draft, stable 1.0 blocked.

---

## UX31 Swift self-scope correction (2026-11-02)

Source review of the shared SwiftUI context/inspector retrieval-note helper found
an invalid `model` reference after moving the function into `Catalog`.
Replaced it with explicit `self` lookups for the private shelf label,
catalog disk title and last OS verification caption, in the SAME draft PR #57.
Added cross-client source regression to prevent repeating the scope error.
Rustfmt-only failed exact-HEAD run at 22ddff9 was already corrected within
the same PR. The new exact HEAD requires native Mac Intel/ARM builds,
Windows, Linux, offline, integrated and installed upgrade checks before any
PASS claim. No CTF/user data changes, no merge or stable release.

---

## UX31 CI fix — exact initial job/step (2026-11-02)

Initial UX31 SHA \`22ddff9c703e96e854636ec68a27d8c738b5a049\`
failed the native [Linux job](https://github.com/i-shutov/whereisit-ai/actions/runs/41491419458)
at \`cargo fmt --all -- --check\`, and
[offline kit](https://github.com/i-shutov/whereisit-ai/actions/runs/41491419397)
at \`Check integrated Rust contracts\` for the SAME single multiline
\`SendMessageW\` formatting diff in \`apps/windows/src/native.rs\`.
The workspace Rust unit suite and dual-native Python contracts passed.
Fixed that exact hunk in this same draft PR #57; no test disabled.
Also unified the macOS context-menu and inspector retrieval-note renderer,
so both copy the same full disk ID/sticker/path/recorded-size/last-OS-state
disclaimer, with SHA revalidation. New exact-HEAD CI REQUIRED; old SHA
Windows/macOS jobs do not establish results for this repair.

---

## UX31 — native selected-file context actions + compact macOS toolbar (2026-11-02)

In existing draft PR #57, added Win32 right-click / Shift+F10 exact-row
result context menu with guarded catalog parent browse, cross-disk
name+size metadata candidates, exact name and size searches, disk register
reveal, SHA-rechecked locator/disk pickup copies. Empty space, pending
page, missing volume and invalid row fail closed. Mac SwiftUI selected-row
context actions use existing generation-pinned service paths, while a compact
visible File actions menu retains Folder/Disk card/OS root/Finder shortcuts
when third pane is hidden; fixed conflicting OS-root VoiceOver labels.
No historical letter/mount inference, live-file or byte-equality claims,
CTF/cache/annotation mutation or new PR. Windows compiled pure tests and
both-client source contracts included. [Manual UX31 runbook](docs/qa/UX31_NATIVE_RESULT_CONTEXT_ACTIONS_20261102.md).

**At source integration: exact-HEAD CI unverified.** Check native
Linux/Win/macOS ARM+Intel, offline, integration, and installed RC2→RC3
jobs for new SHA. Source real 44-disk CTF acceptance is earlier independent
evidence; missing historical raw inputs, original modified CTF, manual
hardware/GUI, signatures and actual p95 <=150ms remain release blockers.
Draft PR #57 must not merge, stable 1.0 must not publish.

---

## UX30 — linked-source incomplete generation fails closed; selected clipboard pinned (2026-11-02)

Draft PR #57: audited a real native source-safety gap on BOTH Windows Win32
and macOS SwiftUI. When an external CTF path existed but its generation
token was unavailable, local SQLite search/candidates could be selected
instead of refusing, and native mutation controls could treat the session
as writable. Also selected locator / path clipboard actions reused an
already loaded linked page without a fresh source SHA check. This integrated
repair makes external-source presence sufficient for read-only mode, refuses
un-pinned volume/search/candidate fallback, clears stale register/page state,
and independently revalidates external generation and current selection
before Win32 locator and SwiftUI label/path/retrieval clipboard operations.
Mac disk pickup card also refuses missing generation and verifies its
current generation. No source/cache/user SQLite writes; no historical
drive-letter or content hash claims.

Compiled Windows pure regression + dual-native Python source contracts,
manual [UX30 runbook](docs/qa/UX30_LINKED_SOURCE_CLIPBOARD_FAIL_CLOSED_20261102.md).
**Exact-HEAD CI pending until ALL four native builds, offline acceptance,
integration regressions and three installed upgrades complete.**
PR stays draft, no merge; RC3 unsigned prerelease. Real 44-disk CTF results
are earlier separate evidence. Historical raw inputs, original modified CTF,
Win10/11/Mac ARM/Intel manual hardware+GUI, real p95 150 ms and
signing/notarization remain blocked. No other repositories modified.

---

## UX29 — native Explorer/Finder handoff for OS-verified disk root (2026-11-02)

The existing draft PR #57 adds Windows Ctrl+Shift+B **Open OS root** and
macOS Command-Shift-B **Finder**, also available through native buttons
and Windows View menu. Only the explicitly paired, currently OS-observed
filesystem UUID with an unchanged unique mount root across TWO native
probes can open; CTF generation SHA is rechecked each time. Fixed a
source-safety gap shared by both root actions: a partially linked CTF with
missing generation must fail, never bypass the pin as unlinked SQLite.
No catalogued file path, historical drive letter or hash is inferred.
Windows adds a compiled remount/clone regression and both native client
wiring is checked in Python; see
[UX29 runbook](docs/qa/UX29_OS_VERIFIED_ROOT_EXPLORER_FINDER_20261102.md).

**CI at commit preparation: NOT YET VERIFIED**; confirm four-platform
native, integration, offline kit and RC2→RC3 upgrade on exact new SHA.
PR remains draft/unmerged. Hardware GUI, modified original CTF, two
provider-blocked historical CTFs, signing and real p95 remain release
gates; RC3 unsigned prerelease, stable 1.0 blocked. No other repos or
private data changed.

---

## UX28 exact-HEAD Rustfmt diagnosis and repair (2026-11-02)

At the first UX28 HEAD `2241c3be121dc41f91dd0af874a73dea5df10190`,
the [native Linux test job](https://github.com/i-shutov/whereisit-ai/actions/runs/41469634689)
passed workspace Rust unit tests and the Python dual-native source-contract
regression but **failed only `cargo fmt --all -- --check`**. The
[offline-kit workflow](https://github.com/i-shutov/whereisit-ai/actions/runs/41469634681)
failed at its first `Check integrated Rust contracts` step for the same
two Rustfmt hunks in `apps/windows/src/native.rs`: assertion wrapping
and the new folder-navigation status call. Those exact formatting changes
have been applied in the **same draft PR #57**, without suppressing tests.
The [UX28 integration wiring](https://github.com/i-shutov/whereisit-ai/actions/runs/41469634859)
passed. New final-HEAD native/offline/integration/installed-upgrade checks
are required; earlier green workflows cannot certify the repair.
No real USB GUI, original/historical CTF or stable-release acceptance.

---

## UX28 — two-level candidate → folder → source-search navigation (2026-11-02)

Draft PR #57: both native clients let a selected **other-disk candidate** file
open its *recorded catalog parent folder* via Ctrl/Command+Shift+F. First
Back rebuilds the exact name+recorded-size candidate query with original
entry ID, disk focus, page offset and optional `copies:` register filter;
second Back returns to the original global search. Candidate and folder
pages are independently read through the existing generation-SHA-pinned CTF
service (or bounded local SQLite). Old rows are never reused. Fresh search
and disk navigation clear candidate history. No current filesystem, content
hash or physical-USB claim. Windows unit and dual-native contract tests plus
[runbook](docs/qa/UX28_CANDIDATE_FOLDER_RETURN_20261102.md) added.

**Status at commit preparation: CI PENDING** for the new exact HEAD; verify
native 4-platform, offline, integration and installed upgrades before claiming
PASS. PR remains draft, no merge or stable 1.0. Physical Win10/11 and Mac
ARM/Intel GUI/hardware, historical CTF inputs, signing and actual search p95
remain blocked. No private CTF/SQLite, paths or hashes published.

---

## UX27 Win32 shortcut shadowing repair (2026-11-02; new exact-HEAD CI required)

First UX27 exact HEAD 238ad56be5fe83196aa59cf3506c219aa7c747e8 failed
Linux native and offline-kit **cargo fmt --all -- --check** on four
Windows-only formatting hunks; corrected without suppressing tests at
418048a79e2435edcc7af26dc752ee847f7eb519. Additional pre-acceptance
source inspection found Win32's preexisting Ctrl+F handler also consumed
Ctrl+Shift+F (no Shift guard), so the new recorded-folder command could not
fire from the keyboard. Added a negative-Shift guard to ordinary Ctrl+F and a
dual-native source contract regression, in the SAME draft PR #57. Exact-HEAD
Linux/Windows/macOS ARM+Intel and all upgrade/offline jobs must be rerun
before describing UX27 as fully green; real USB/visual QA remains outstanding.

---

## UX27 — selected file to containing CATALOG folder (2026-11-02; exact-HEAD CI required)

In existing draft PR #57, add a direct context navigation path from a verified
global/duplicate results row to the file's recorded *parent_id* on the same
exact catalog disk. Win32: native locator button / View menu / Ctrl+Shift+F;
macOS SwiftUI: inspector button plus always-visible Folder action /
Command-Shift-F even when the locator is collapsed. Back restores the search,
offset and previous scope. Root files browse the exact disk's catalog root;
unknown disk, stale page, non-file, invalid/self parent, copy-mode or missing
recorded path refuse navigation. A recorded breadcrumb is **not** a live mount.
SQLite uses bounded \`children\`, linked CTF \`ctf_children_pinned\` revalidates
original generation SHA per page. A source-change failure is never bypassed.

Synthetic Windows compiled exact-ID/parent/path cases, dual-native contract,
and [UX27 manual runbook](docs/qa/UX27_RECORDED_PARENT_FOLDER_20261102.md)
cover 4/42/142 disk ID collision, page/Back, root and Mac collapsed view.
New exact-HEAD Linux+Windows+Mac ARM/Intel, offline and RC2→RC3 upgrade CI
MUST be verified; human screen-reader, 880/1366px, hardware, historical CTF,
signing and real p95 gates remain open. No user data or source SHA released.
PR draft/unmerged; stable 1.0 blocked.

---

## UX22 exact-HEAD SwiftUI compiler repair (2026-11-02; rerun required)

The initial UUID-clone race repair [f1ba21c](https://github.com/i-shutov/whereisit-ai/commit/f1ba21ce8e5b96612e419d29c1cef4db36ee9fa3) passed [integration](https://github.com/i-shutov/whereisit-ai/actions/runs/41425403204) and [offline kit](https://github.com/i-shutov/whereisit-ai/actions/runs/41425403218), but [macOS ARM64 installed upgrade job](https://github.com/i-shutov/whereisit-ai/actions/runs/41425403188) failed Swift compiler conditional binding at AtlasCatalog.swift lines 856 and 932: `let ... = filter(...)` binds a non-optional array inside `guard`. Replaced these with genuine optional `first(where:)` bindings plus an independent `filter(...).count == 1` UUID uniqueness check. The safety predicate and clone-race regression are preserved; no tests suppressed. Recheck ALL new exact-head workflows, including native Mac ARM/Intel and installed upgrades; do not call earlier green jobs evidence for the new SHA.

---

## UX22 verified defect repair — macOS clone race (2026-11-02; exact-HEAD CI required)

During source review of UX22, found a fail-open macOS second-probe predicate: it counted occurrences of **UUID + original mount path**, so a newly connected cloned filesystem UUID on a *different* mount could leave exactly one old UUID/path match and still copy the OS root. The native confirmation preflight for physical pairing used the same weak predicate (the Rust service already rejects duplicate IDs, but the preflight should fail closed too). Both now count **all observations of the filesystem UUID first** and independently require the sole path to equal the previously inspected mount. Added a synthetic dual-native source regression and explicit clone-between-probes QA scenario. Windows already counts duplicate identities independently of mount root. No CTF, SQLite, source generation, contents or pairing data changed. This is a source/CI repair only; human clone/USB hardware and native GUI acceptance remains outstanding. Verify exact new PR #57 HEAD native Linux/Win/macOS ARM+Intel, offline, integration and RC2→RC3 upgrades; do not merge or issue stable 1.0.

---

## UX22 Windows exact-HEAD Rust test compiler repair (2026-11-02; recheck required)

Latest UX22 [SHA 69cce5a](https://github.com/i-shutov/whereisit-ai/commit/69cce5ab5dcbf8c059d75c0a2aa60d7f72f8d42b) passed [offline acceptance 41422595264](https://github.com/i-shutov/whereisit-ai/actions/runs/41422595264), but Windows native [job 125202564611](https://github.com/i-shutov/whereisit-ai/actions/runs/41422595309) and installed upgrade [job 125201466691](https://github.com/i-shutov/whereisit-ai/actions/runs/41422595165) failed **E0425** in three new synthetic-only test fixtures: interpolation referenced a missing local variable `description`. Corrected fixtures to an explicit synthetic OS mount path without changing the runtime copy logic or suppressing tests. Reverify the NEW exact-HEAD Linux/Win/Mac Intel+ARM, offline, integration and upgrade results; no green claim until every workflow completes. Owner USB/GUI, historical CTF and release blockers remain.

---

## UX22 exact-HEAD Rustfmt repair (2026-11-02; rerun mandatory)

Initial UX22 SHA [13a371f](https://github.com/i-shutov/whereisit-ai/commit/13a371f30e36fe11abd62dff75200faf84e91984) failed [offline-kit run 41421963012](https://github.com/i-shutov/whereisit-ai/actions/runs/41421963012) at **Check integrated Rust contracts → cargo fmt --all -- --check**. The job reported only Rustfmt differences in the new Windows root-copy helper, its regression assertions, the new status command, and the service UUID formatter. Applied exactly those changes in the SAME draft PR #57 without suppressing tests. **New exact-HEAD checks must pass** before any green claim. Physical/visual owner hardware, private historical CTF and stable 1.0 gates remain unchanged.

---

## UX22 — copy the CURRENT OS root of an explicitly paired catalog disk (2026-11-02; exact-HEAD CI PENDING)

Draft PR #57 adds a genuinely live, **native-only** retrieval step on Win32 and SwiftUI: a selected catalog file/disk may request **Copy OS root** (Ctrl+Shift+H / Command+Shift+H). The macOS keyboard handler remains in the visible toolbar even when its locator pane is hidden at 880px; an incomplete linked-CTF session also refuses to copy on Windows instead of bypassing SHA verification. The shared local physical-status service now emits `verified_physical_id` **only** when the binding is explicit, the OS probe is complete, that UUID has exactly one current OS observation, and neither disk binding nor physical ID conflicts. Missing pairing, unplugged disks, scan failures and clones yield no disclosed match. Win32 resolves a mount point solely from GetVolumePathNamesForVolumeNameW and repeats its full OS scan; SwiftUI resolves solely from mountedVolumeURLs and repeats its UUID/mount-path scan before copying. Both reject changed or ambiguous identities and reject non-current file/disk selection. If linked CTF is active both first issue a `ctf_volumes_pinned` read to recheck generation SHA. The clipboard contains **only the current OS volume root** (not a file path, not historical drive letters, not source CTF/SQLite content). The app never claims the selected file actually exists on that mount or has matching bytes and does not delete anything.

Synthetic Rust service + Win32 unit tests, dual-native source-wiring checks and [UX22 manual QA](docs/qa/UX22_FRESH_OS_ROOT_COPY_20261102.md) added. **Recheck exact-HEAD Linux, Windows, macOS ARM64/Intel, offline/integration/upgrade CI before claiming green.** Human physical USB, Windows 10/11, Mac Intel/ARM GUI/a11y and real p95 still unverified; prerelease RC3 unsigned; PR stays draft, no merge or stable 1.0. No user paths, CTF, SQLite or SHA uploaded.

---

## UX21 exact-HEAD CI formatting repair (2026-11-02; rerun required)

Initial UX21 [SHA bd67f2d](https://github.com/i-shutov/whereisit-ai/commit/bd67f2d4bfdc702ccc69b3efa0fdfd50757b36de) passed Rust workspace tests and native source-wiring regression but failed **cargo fmt --all -- --check** in [native CI run 41416845087](https://github.com/i-shutov/whereisit-ai/actions/runs/41416845087) and [offline-kit run 41416845100](https://github.com/i-shutov/whereisit-ai/actions/runs/41416845100). Exact diff: match-arm wrapping in `copy_candidates.rs`, one pinned CTF assertion, and one service-integration JSON call. Applied precisely the reported formatting in the SAME draft PR #57, without disabling checks. New exact-HEAD native four-platform, offline-kit, integration and upgrade jobs must be verified before a green claim. Historic real-owner CTF acceptance is separate; no real USB/visual or stable 1.0 claim.

---

## UX21 — true other-media copy scope, native Win32 + SwiftUI (2026-11-02; CI pending)

Draft PR #57 fixes a user-visible mismatch: **Find copies on other disks** formerly returned metadata candidates from the source catalog disk itself. The optional read-only service flag `other_disks_only: true` now removes **all** records on the source `volume_id` before SQL LIMIT/OFFSET, grouping and per-disk focus. `all_total` still denotes the complete name+recorded-size class, `other_total` counts external candidates only, and the unflagged API remains backward compatible. Both native clients explicitly request this scope for ordinary SQLite and SHA-pinned linked CTF; status/scope captions now say **OTHER catalog disks**. Zero matches on other media is a legitimate zero-result answer, not evidence of missing file bytes. Physical OS verification, pairing, sidecar tags and personal annotations remain independent and unchanged.

Synthetic Rust unit/service integration tests and pinned CTF regression verify other-volume paging, source-volume focus returning zero, malformed flag rejection, and source-changed rejection after external CTF generation change. Dual-native wiring regressions and [UX21 runbook](docs/qa/UX21_ONLY_OTHER_DISKS_20261102.md) included. New exact-HEAD Linux/Windows/Mac ARM64+Intel CI, offline acceptance and installed RC2→RC3 upgrades must pass; manual Win10/11 Mac ARM/Intel GUI/real USB checks remain outstanding. No user CTF/SQLite or private paths published; no content-hash, live-file or auto-delete claims. PR draft/unmerged, RC3 unsigned prerelease, stable 1.0 still blocked.

---

## UX20 — dismiss/retry slow asynchronous search pages, both native clients (2026-11-02; CI pending)

Draft PR #57 adds **Stop waiting** (Win Ctrl+Shift+X, Mac Command+Shift+X) and **Retry page** (Ctrl/Command+Shift+Y) to the native search status strip. This is UI-level dismissal, **not filesystem/CTF work cancellation**: a running Rust service reader can finish, and its pending count remains accurate until completion. Windows invalidates the active page generation and ListView rows, disk-copy picker and candidate-register view; macOS invalidates the main-actor generation and clears stale entries, current selection and candidate groups. Both reject late replies and disable stale file actions. Retry preserves the applied query, exact disk/copy scope and requested offset, starts a new request and performs the existing generation-SHA check again for linked CTF. A failed current-page response also offers Retry; a previous successful page is never silently substituted.

No source CTF or SQLite/cache/user-note mutation, no hash or live-file inference, and no promise to abort physical I/O. Synthetic Rust generation guard tests, dual-native contract regressions and [UX20 runbook](docs/qa/UX20_ASYNC_PAGE_STOP_RETRY_20261102.md) added. Full exact-HEAD Linux/Windows/macOS ARM64/Intel, offline and installed upgrade CI must complete before claiming green. Manual 880/1366px, a11y/keyboard and slow-read/canceled-source hardware testing still required. PR stays draft/unmerged; stable 1.0 blocked.

---

## UX19 — physical disk register health audit and safe status filters (2026-11-02; new exact-HEAD CI required)

Draft PR #57 adds a cross-platform, native **USB retrieval audit** for approximately 40 mostly disconnected media: the left register shows counts of independently OS-verified connected / explicitly paired offline / identity conflict / unverified catalog disks, plus missing and shared owner shelf tags. Windows Physical disks menu and **Ctrl+Shift+O/U**, macOS SwiftUI native accessible icons and **Command+Shift+O/U**, trigger a **new native OS UUID probe** before filtering the register to respectively connected or unverified catalog IDs. Both also accept typed \`state:connected|offline|conflict|unverified\` and \`tag:missing|shared\` filters (exact, reserved syntax; an unknown value matches nothing); \`id:\` and \`copies:\` remain unchanged. Filtering is view-only: current all-disk query, result page, selected file, source generation, physical pairing and private annotations are not changed. Failed or pending probes downgrade all positive states to unverified, never to offline. Historical drive letters, stickers and repeated labels cannot assert physical connection. An OS filesystem UUID is not a hardware-serial guarantee and does not prove file presence or hash equality. Source-linked CTF remains SHA-pinned read-only for every page.

Synthetic compiled Windows test covers ID collision 4/42/142, all audit categories, missing/shared tags, failed probe and stable row mapping; dual-client source-wiring contract and [UX19 manual QA runbook](docs/qa/UX19_PHYSICAL_REGISTER_AUDIT_20261102.md) cover keyboard, VoiceOver, plug/unplug and 1366×768. Verify final exact-HEAD Linux+Win+Mac ARM/Intel CI, offline and installed upgrades before any PASS claim. This is not real owner hardware/GUI acceptance. PR draft/unmerged, RC3 unsigned prerelease only. Stable 1.0 blockers remain unchanged.

---

## UX17 — literal filename search across offline disks, any size (2026-11-02; CI pending)

Draft PR #57 extends the native inspector with a second, distinct retrieval path: Win32 **Ctrl+Shift+N / View → Find same filename across all disks**, macOS SwiftUI **Command+Shift+N / Find same filename (any size)**. It produces a Unicode case-folded `name_exact:"..."` **bound** query, preserving filename punctuation and spaces, searches all catalogued disks regardless of recorded size, and **Back** restores the prior search/offset/scope. Unlike **Find copies** (same normalized name + recorded size), this may surface alternate versions or entirely unrelated bytes. It does not claim a hash match, actual USB file presence or permission to delete. Pending/stale result selection and malformed/oversized filenames are rejected. Linked CTF reads still check source generation SHA each page. Rust core and Win32 pure tests, dual-native integration contract and [UX17 QA](docs/qa/UX17_LITERAL_FILENAME_ALL_DISKS_20261102.md) use only synthetic fixtures. Full exact-HEAD CI and manual Win10/11 Mac ARM/Intel GUI/USB/VoiceOver remain required. PR stays draft, stable 1.0 blocked.

---

## UX16 — native video-extension search preset (2026-11-02; exact-HEAD CI required)

Draft PR #57 integrates indexed `type:video` catalog metadata predicate with Win32 Ctrl+Shift+V / SwiftUI Command+Shift+V and native toggle buttons. Global search, grouped OR query semantics, reversible editable text and linked-CTF source generation SHA verification remain intact. Synthetic two-disk Rust and dual-native source regressions plus [UX16 QA runbook](docs/qa/UX16_VIDEO_EXTENSION_SEARCH_20261102.md) added. This is **not** movie recognition, live USB file existence or content/hash equality. Human native UI/hardware acceptance and p95 150ms are unverified; no merge or stable 1.0.

---

## UX15 — keyboard-first dense result table (2026-11-02; CI must verify exact HEAD)

Draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57) now adds explicit catalogue **Kind** to the compact native result tables and to loaded-page-only sorting in Win32 and SwiftUI. Windows Ctrl+E and macOS Command+E move from search/register to the verified current results and select the first row only if none is selected; no stale page focus when a new query is loading. The associated Rust guards/tests, dual-native source regression and [UX15 manual QA runbook](docs/qa/UX15_DENSE_RESULTS_AND_FOCUS_20261102.md) cover page freshness, correct column indices, 880–1366px layout and keyboard/a11y review. These are catalogue metadata, not OS file presence or hash evidence. All CTF/physical pairings and private annotations remain unchanged. Rerun full exact-HEAD CI and installed upgrades; human USB/GUI tests, real SLO, historic CTF, signatures and stable 1.0 acceptance remain blocked. PR must stay draft/unmerged.

---

## UX13 — exact candidate disks in physical register (2026-11-02; new exact-HEAD CI required)

In draft [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57), the left disk register can now display ONLY the actual catalog IDs present in the selected file's cross-disk name+size candidate groups. Windows: View → Show candidate disks in register / Ctrl+Shift+K. macOS: accessible Candidates button / Command-Shift-K. The temporary `copies:` filter is view-only; the file query, loaded candidate page, selected entry, physical pairing and connected/unverified truth all remain unchanged. The filter clears automatically upon leaving copy mode. Windows maps filtered visible rows back to exact original volume indices, with collision tests for 4/42/142, duplicate groups and missing IDs. Candidate-only rows are invalidated immediately when a new file starts loading and rebuilt when its verified candidate groups arrive, so stale previous-file disks cannot remain visible. Dual-native source regression and [UX13 runbook](docs/qa/UX13_CANDIDATE_DISK_REGISTER_20261102.md) cover keyboard, state transitions and scope safety. No claim of content hash equality, live file existence or OS connection follows from a candidate group.

Recheck exact-HEAD Linux test/fmt/Clippy, Windows + macOS ARM64/Intel native builds, integration/offline acceptance and RC2→RC3 upgrades. This feature has not been visually or physically accepted on owner equipment. Original modified/historical CTF, real p95 150ms, signing/notarization and stable 1.0 remain blocked. PR remains draft/unmerged.

---

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


## UX14 — ranked candidate register, exact-HEAD check pending (2026-11-02)

New changes remain in draft PR #57, targeting existing main. Candidate-mode
native disk registers display exact disk ID, per-disk metadata count, private
sticker, last independent OS pairing status and shared-sticker warnings, ordered
by the physical pickup priority (verified, offline, unverified, conflict;
count desc; exact ID). Does not alter global search or evidence of file bytes.
Rust tests, both-client source wiring checks and manual runbook
docs/qa/UX14_CANDIDATE_REGISTER_PRIORITY_20261102.md accompany the code.
Require final exact-head 4-platform native / offline / integration / upgrade CI.
No stable release or merge without physical/GUI and remaining gates.


## 2026-11-02 — UX18 size-only renamed-file candidate search (draft PR #57)

Added an explicit global **same positive recorded byte size, any filename**
action on the selected current file row in native Win32 (Ctrl+Shift+S) and
SwiftUI (Command-Shift-S). The bounded, read-only \`size:<bytes> AND kind:file\`
query finds potential renames across offline catalogued disks; Back preserves
the prior query/scope/page. Zero/missing/overflow size, folders and stale-page
actions are refused. It does **not** verify equal bytes, physical presence or
safe deletion. Existing linked CTF source SHA-generation pinning applies on
every search page. Synthetic Rust/SQLite multi-disk and both-native wiring
regressions plus operator QA: docs/qa/UX18_SAME_SIZE_RENAMED_CANDIDATES_20261102.md.
This is a source integration checkpoint, **not** hardware/visual acceptance.
Record final exact-HEAD Actions URLs in draft PR #57 after completion; no
merge or stable release without the original/historical CTF, GUI/hardware,
signing/notarization and real p95 <=150ms gates.


## 2026-11-02 — UX23 native sleep freshness + Windows same-UUID clone guard

Draft PR #57: both native clients expire connected states after 12 seconds of
monotonic OS scan age, measured from before enumeration. Windows invalidates
old checks on foreground and before commands/timer, pairing checks uniqueness
of the full UUID among fresh mounts rather than UUID+description counts.
SwiftUI rejects delayed replies and re-probes at wake/foreground. Original
physical service collision checks are unchanged; CTF stays read-only and
generation-SHA pinned; user bindings/notes are separate. Compiled Windows
tests and dual UI source regression + manual docs/qa/UX23_RESUME_FRESHNESS_AND_CLONE_PAIRING_20261102.md
included. No private bytes or IDs published. Final exact-head Actions must
be inspected before PASS; GUI/USB/historical CTF/signing/p95 manual acceptance
still BLOCK stable 1.0; draft only, no merge.


---

## UX24 — disk-only physical pickup card (2026-11-02; exact-HEAD CI pending)

The same **draft PR #57** adds a retrieval aid for the case where the operator
has selected one of ~40 offline catalog disks **without any file result**.
Windows native View menu/Ctrl+Shift+L and macOS SwiftUI accessible toolbar
Command-Shift-L plus inspector button copy a local physical disk pickup card:
**exact catalog disk ID first**, recorded disk label, private owner-entered
sticker/shelf tag, catalogued entry count, shared-sticker warning and the last
independently time-bounded OS pairing state. Normal register rows on both
platforms now visibly include the exact catalog ID to disambiguate identical
labels. A stale/incomplete OS scan means **NOT VERIFIED**, not connected.
All clipboard data stays local. No historical mount letter/path is treated as
current; no live file I/O, CTF/SQLite/annotation write, hash match or auto-delete.
Windows compiled Rust card tests cover ID/Cyrillic/shared labels and failed
OS proof, and dual-client source contracts and
[UX24 manual runbook](docs/qa/UX24_PHYSICAL_DISK_PICKUP_CARD_20261102.md)
cover focus, keyboard, accessibility and source switch. Exact-HEAD native 4/4,
integration/offline kit and installed upgrade 3/3 **must be checked** before
any green claim. Hardware GUI, historic/modified CTF, real p95 150ms, code
signatures and notarization remain release blockers. Do not merge/publish stable.


### UX24 linked CTF generation-check repair (2026-11-02)

UX24 initial commit [5b1c0aed](https://github.com/i-shutov/whereisit-ai/commit/5b1c0aed141885455c12df7af0d7db1f6efe02f1) failed [exact-head offline CI run 41437613128](https://github.com/i-shutov/whereisit-ai/actions/runs/41437613128), **Check integrated Rust contracts → cargo fmt --all -- --check**, due solely to two new Win32 formatting differences. Both are repaired within draft PR #57 without disabling checks. During the same audit we found that the new disk-only clipboard action reused a loaded linked CTF volume register without an independent per-action SHA check. Both native clients now asynchronously call pinned `ctf_volumes_pinned` before copying from a linked CTF; stale generation, selection or scope fails closed and never updates the clipboard. Synthetic SQLite mode copies its current loaded register; there is no source mutation. The dual-native integration regression now checks these pinned gates. Exact new HEAD CI remains required before declaring builds green; no merge or stable release.


### UX24 Windows exact-HEAD compiler repair (2026-11-02)

At exact source [35e449fb](https://github.com/i-shutov/whereisit-ai/commit/35e449fb95b68726bdc50ea91fd602cbe2b44770) the Linux Rust/fmt/Clippy job and offline-kit succeeded, but Windows native CI [41438217085](https://github.com/i-shutov/whereisit-ai/actions/runs/41438217085) and installed-upgrade [41438217102](https://github.com/i-shutov/whereisit-ai/actions/runs/41438217102) failed at `cargo test --workspace --locked` / “Build current Windows installer from exact source”: Rust E0308 in `copy_disk_pickup_card`, passing `Option<String>` from `shelf_collision_warning` into `Option<&str>` helper. Apply explicit `.as_deref()` on that temporary inside the same call, in the SAME draft PR without disabling tests. All four final exact-HEAD workflows must be rechecked; an earlier Linux/offline green does not certify the corrected commit. No source CTFS, caches, files or private owner data are touched.



## UX25 — candidate clipboard generation recheck, draft PR #57 (2026-11-02)

Found a real read-only provenance defect: the physical pickup **list** and
current-page **candidate paths** could be copied from a previously verified
linked CTF page without rechecking the original source SHA when the operator
later pressed Copy. Both native clients now asynchronously re-pin the
external generation for each such clipboard action. Windows serializes
the pending check and compares page generation, source ID, offset and disk
focus; SwiftUI also invalidates superseded clipboard commands, checks
catalog scope and response generation. Failed, stale or changed source
responses never write the clipboard. Local SQLite mode uses the current
ready page and does not claim a source SHA.

Corrected the misleading "selected file included" phrase: UX21's
cross-disk candidate mode excludes the whole source catalog disk.
Native Rust guard tests + dual-client regression and operator runbook:
docs/qa/UX25_CANDIDATE_CLIPBOARD_SHA_RECHECK_20261102.md.
Linked CTF is read-only; owner data never enters CI or clipboard QA.
CI and manual USB/GUI/historical-CTF/signing/p95 release gates remain
separate. PR remains draft/unmerged, stable 1.0 BLOCKED.


## UX26 — event-driven native mount invalidation (2026-11-02, PR #57)

A meaningful source-level defect remained after UX23: despite 6-second polling,
USB unplug could leave the *last verified* green disk badge visible until the
next tick. Both native clients now revoke old physical presence immediately
upon OS mount-change events. Windows WM_DEVICECHANGE distinguishes removal-
pending (invalidate only) from completed arrival/removal (invalidate and new
full OS scan); SwiftUI NSWorkspace willUnmount/didUnmount/didMount revokes
in-flight mount-root/candidate-copy intents and their response generations.
No event, historical drive letter, user shelf tag or catalog entry is treated
as proof of live bytes. Periodic fallback, explicit pair DB, generation-pinned
read-only CTF and separate annotations remain intact.

Evidence contract: compiled Windows event tests, both-native source regression,
new docs/qa/UX26_EVENT_DRIVEN_USB_HOTPLUG_20261102.md; the final exact-HEAD
Actions results must be verified separately before any PASS claim. Manual USB
hardware/Win10/11/Mac ARM/Intel, historic CTF, signing/notarization and real
p95 <=150ms remain stable 1.0 gates. Do not merge draft #57 automatically.
