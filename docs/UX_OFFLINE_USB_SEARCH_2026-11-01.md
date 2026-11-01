# WhereIsIt AI — UX redesign for an offline USB collection
Date: 2026-11-01
Repository: `i-shutov/whereisit-ai`
Baseline main: `cabdb3d2b139a67729a795ba9c34e4d3144c9c9f`
Status: **implementation in progress; no stable 1.0 acceptance**.

## Product brief — the actual owner workflow

The operator has approximately 40 intermittently connected external USB drives, primarily containing films and other media. The existing original WhereIsIt workflow is useful because it answers three questions while the hardware is disconnected: **Where is this file? Which physical disk should I retrieve? What other copies are catalogued?** This is *not* a general-purpose DAM, media player, cloud library or auto-delete utility.

Prioritize speed and information density over decorative screenshots. The primary screen must be search, not catalog administration. Search must work across all catalogued disks by default; browsing a single disk must be an explicit scope change.

## Interface model (Windows 10/11 Win32 and macOS ARM64/Intel SwiftUI)

A compact search-first desktop shell with three contextual regions:

1. **Disk register (left):** real catalog labels and stored disk identity; a reliable connected/offline badge only after an independent OS-level presence/identity check. Otherwise label the state **physical connection not verified**. Catalog timestamps and last known root path are historical metadata, not current drive-letter mappings.
2. **Search results (center):** dense, sortable file table with Name, Disk, Kind, Size, Date, candidate-copy count and recorded path. Preserve pagination, keyboard selection, selection generation and source SHA pinning; never preload 3M rows.
3. **Disk locator (right):** selected file name, exact catalog disk label and identifier, recorded relative path, size, and separate count of *metadata* duplicate candidates; selectable/copyable text. Explicit instruction to find the labeled physical disk, not an unverified “online” claim. Small Windows windows may collapse this pane, retaining existing native detail access.

Search is the dominant action; Browse catalog and Duplicates are first-class context shifts. The right inspector must never trigger CTF writes, override annotation conflicts or infer content hashes. For a linked CTF, all operations remain generation-pinned read-only; annotations remain in an independent user-owned SQLite.

## Delivery plan — integrated functional slices, not a micro-PR chain

| Stage | Real deliverable | Acceptance evidence | Status |
|---|---|---|---|
| UX0 — Disk locator | One selected-search-result -> reliable catalog disk label / recorded location panel on **both native OS clients**, with copy/navigation and explicit unverified connection state | Exact-HEAD Windows Rust tests, macOS compiled app, 4-platform native CI, human screenshots/keyboard walkthrough still pending | **In progress** |
| UX1 — Search-first shell | Prominent global search on app launch, persisted filter/sort and resizable/collapsible panes; keyboard-first, single-screen layout at 1366×768 and high DPI | Pure Windows 880/1100/1180/1366px geometry tests; Ctrl+F, Command+F and all-disk search wiring; platform builds/installer smoke; manual visual/hardware QA still pending | **In progress (keyboard + adaptive first slice)** |
| UX2 — Physical drive identity | Native Windows GUID/serial and macOS filesystem UUID probes; explicit pairing and unpairing in an independent user SQLite; scoped by CTF generation or SQLite path; fail-closed states and six-second refresh. No historical-letter inference or auto-clone validation | Synthetic binding/collision tests and native CI pending; real unplug/replug + hardware and visual QA still required | **Implementation candidate in PR #57, not accepted** |
| UX3 — Film copies | Selected result → exact normalized name + recorded size candidates across catalogued volumes, paginated, with back navigation; metadata only, never verified file equality or deletion. | Synthetic SQLite + pinned CTF fixture with generation-change rejection, native wiring checks; real-device acceptance pending. | **Implementation candidate in PR #57; not accepted** |
| UX4 — Polish and acceptance | Native accessibility, keyboard traversal, reliable cancellation/progress, contrast/light-dark themes, window geometry, upgrades and observed SLO | Owner acceptance on Win10/11, Mac Intel/ARM; RC-to-RC upgrade; measured real 3M-row latency | Planned |

No new browser/runtime dependency is introduced; the current Rust service API and native clients stay authoritative. Support for content previews/posters, movie-title grouping, and content hashes depicted in exploratory mockups is **not** implied by those images and is outside UX0.

### UX3 incremental integration: selected-result cross-disk copies (PR #57)

The native clients now offer **Find copies on other disks**, distinct from broad
**Duplicates**. Windows uses the locator action/Ctrl+D; macOS uses the locator
action/Command+D. Both preserve a return-to-results context, reuse the bounded
result tables, and navigate the list of catalogued disks for the selected file.

The new read-only \`copy_candidates\` service operation takes an **entry ID**
and resolves its normalized filename and recorded byte size from that actual
catalogue entry. It counts all entries and distinct catalogued volumes with
those two metadata keys, then pages over their recorded locations. The linked
operation \`ctf_copy_candidates_pinned\` independently verifies the original
external CTF generation SHA on **every** page; it never mutates the source
or the generated mirror. Synthetic SQLite and two-disk CTF tests cover
normalization, name/size collisions, bounded pagination, provenance flags,
source-change refusal and source/mirror byte preservation.

The count **includes** the selected file; copies on the same physical disk
may appear in the list. No result is a hash-verified byte match, confirmed
physical USB presence, guaranteed movie-title match, or safe deletion target.
Different filenames containing identical bytes will not be found by this
exact-name+size mode. This does not claim the ~40 owner disks were retested
for this new UI feature.

**Outstanding:** stable OS-verified physical disk identity (UX2) must precede
any "connected" display. The code still says **physical connection not
verified**. Full Win32 search-first styling, persistent sort/filter, resizable
splitters, 1366×768/HiDPI ergonomic and keyboard/accessibility inspection, real
device/large-CTF latency verification, and owner acceptance are not complete.
Final exact-HEAD four-platform native CI and upgrade gates must pass before
the PR can be considered a release candidate, and stable 1.0 remains blocked.

## UX1 implementation checkpoint — 2026-11-01

- **Windows:** Ctrl+F focuses the existing search field and selects its contents, and first launch puts the cursor in search. Pure adaptive geometry chooses a 282px selected-disk inspector from 1176px window width; table Path column collapses only while that inspector displays the full location, restoring on wider windows (or if the inspector disappears at narrow widths). Compact file table columns fit the 1180px default window and the target 1366px laptop width. A unit test checks 1100/1180/1366px geometry, source wiring checks shortcuts. This remains standard Win32 controls; final visual redesign and owner ergonomics acceptance are **not yet achieved**.
- **macOS:** Command+F focuses search without submitting unexpectedly; Enter executes the query; a separate All disks action explicitly searches across all catalogued drives. Native focus is requested when the window first appears. The three-pane inspector remains resizable. Requires manual keyboard/focus inspection on both ARM64 and Intel to call UX1 complete.
- **Still missing:** persistent sorter/filter, draggable Windows splitters, small-display / scaling visual QA, more modern Win32 visual styling (the owner rejected the old look), physical USB identity validation, and cross-disk film-title grouping. Do not call this full UX acceptance merely because CI builds or installer smoke passes.

## UX0 implementation contract

- Selection changes must refresh the locator from the **current** result page, never a stale search result; no store or disk writes.
- The inspector shows the disk name from the actual `volumes` response and the recorded file's `relative_path`, not a guessed live OS mount path.
- Empty selection gets a useful placeholder; selecting a folder does not assert that the folder exists on an attached drive.
- The `duplicate_count` field is reported only as metadata candidate evidence; it does not prove byte equality.
- A button may copy disk label/recorded path to the clipboard on macOS; Windows provides a selectable read-only native text field, compatible with Ctrl+C.
- Windows minimum width retains the existing full file table; inspector is shown only when wide enough, reducing crowding. macOS inspector uses an HSplitView and remains user-resizable.
- Unit regressions distinguish missing selection, authoritative volume labels, linked CTF read-only, and non-verified physical presence. Compile and installer smoke are not manual GUI acceptance.

## Existing RC3 and hard release gates

Current release `v1.0.0-rc.3` is an unsigned **prerelease**. Existing large real-CFT evidence: 44 volumes, 3,046,027 entries; import, search, CTF roundtrip, CSV/Parquet 15-field parity, linked export, duplicates and annotation CAS all verified under documented conditions. Do not recast these as synthetic, and do not publish private datasets, paths or source SHA. CTF `0x10` remains opaque, with no unsupported lost-description claim.

Stable 1.0 is blocked until original WhereIsIt reads a genuinely modified CTF, the two historical private CTFs can actually be run in a permitted local environment, Win10/11 and Mac Intel/ARM native GUI tests (including upgrades) are performed, Authenticode/Apple notarization is complete, and target search SLO is satisfied (historical real p95 **209.82–252.83 ms**, target 150 ms). Never disable generation SHA checks for a benchmark result.

## Handoff on every iteration

Before a change: re-read live `main`, open PRs, `HANDOFF.md`, last CI and release. Work on only this repository. Make a coherent vertical change, add specific tests, run exact-HEAD CI and name any missing physical checks. Update this plan and the PR handoff with evidence and next integrated slice. Don't merge unverified PRs or publish stable releases. Never touch other projects.

## UX2 physical UUID integration checkpoint — 2026-11-01

Both native frontends now enumerate current OS volume identities and require
explicit operator pairing to the catalog disk ID before showing a connected
state. Windows checks Mount Manager volume GUID and serial; macOS uses mounted
filesystem volume UUID. Collision, missing pairing, unsuccessful probe or source
generation mismatch fail closed. Pairings belong to a private separate
user-owned SQLite, not to linked CTF/mirror or annotations. **OS volume IDs are
not globally unique hardware serials**; clones and human-identification errors
remain an acceptance risk. See [UX2 runbook](qa/PHYSICAL_USB_IDENTITY_WP_20261101.md).
Native builds and real plug/unplug testing are not implied by the source change.

## UX1 + UX2 checkpoint — global search and collision-safe pairing (2026-11-01)

Native Enter/Search and broad Duplicates now clear any implicit disk filter and operate across all registered disks. Per-disk search is a separately labelled This disk command; applied scope is visible in native results. Both OS clients exclude concurrently duplicated volume UUIDs **from pairing choices only**, while passing every raw observed ID to the Rust conflict detector. Windows warns when a selectable local fixed mount might be a system drive. The source remains generation-pinned read-only; no hash/content-equality or deletion assertions. See [runbook](qa/UX_GLOBAL_SEARCH_AND_PAIRING_PICKER_20261101.md). Exact final-HEAD CI and real GUI/hardware checks are required; no stable release authorized.

## UX1 register findability — integrated native filter (2026-11-01)

The left register of ~40 offline USB disks now has independent label/catalog-ID
filtering (Windows Ctrl+L; Mac Command+L), a visible/total count and a clear
path. The filter is **display-only**: global search remains global; explicit
This disk searches still target the underlying selected catalog volume ID;
physical pairing and linked source generation are unchanged. Windows uses an
index map to avoid confusing a filtered row with a catalog volume, with Rust
unit tests for Cyrillic, numeric IDs and missing rows. SwiftUI uses localized
case-insensitive label filtering while preserving selection and keyboard focus.
This remains a source implementation checkpoint, not physical-device/GUI QA.


## Native layout integration: resizable/collapsible disk locator (2026-11-01)

The Win32 shell now uses an **actual mouse-resizable** 8px vertical grip between its
results table and selected-file inspector. The inspector is clamped to 220–420px,
while the results retain at least 600px whenever the third pane is visible; it
auto-collapses below 1120px. **View → Show / hide disk locator** and **Ctrl+I**
provide an explicit, reversible collapse without discarding selection or search.
At 880px the table uses adaptive columns and keeps the recorded Path readable
because no inspector is shown. No CTF, user pairing or search-scope state is
modified by these view operations.

SwiftUI already permits native pane dragging through `HSplitView`; it now has
a visible **Show/Hide locator** action with Command-Shift-I and an accessibility
label, allowing an 880px two-pane window when the inspector is hidden. Both UI
implementations remain native. The action is per-window/session for now, not
persisted across relaunch. This block includes pure Rust splitter geometry and
hit-area tests and dual-client source-wiring regressions. Manual 880/1100/1180/
1366px and HiDPI usability is **not** established by CI; see
[UX pane runbook](qa/UX_NATIVE_PANE_REVIEW_20261101.md). Exact-HEAD CI is
required before describing these changes as validated builds.

## UX1 loaded-page sorting and persistent locator (2026-11-01)

Windows native ListView header clicks and a macOS accessible page-sort picker now sort **only currently fetched results**, at most 250 rows, by name/disk/size/date/candidate-count/path, with ascending/descending selection. Stable catalog entry IDs keep the inspector and copy navigation tied to the actual selected file; missing numeric values remain last. The UI must explicitly label page-only order: this is **not** server-side globally sorted pagination across 3,046,027 records. Windows privately persists locator visibility and bounded splitter width in `%LOCALAPPDATA%/AtlasCatalog/ui-layout.json`; macOS stores locator visibility in UserDefaults. The preference never stores private search queries, CTF source paths or generation SHA, and does not change physical pairing or CTF/caches. [Native QA instructions](qa/UX_NATIVE_PANE_REVIEW_20261101.md). CI plus real GUI QA still required; stable 1.0 remains blocked.
