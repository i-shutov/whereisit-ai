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

## UX2 — Snapshot-completeness safeguard / keyboard OS refresh (2026-11-01)

The physical locator's absence check is now intentionally fail-closed for
**incomplete** OS mount snapshots on both native clients. Win32 treats
unknown/disappearing logical roots as probe failure, not missing paired disks.
SwiftUI ignores known network mounts but rejects any visible local mount
without a readable filesystem UUID instead of silently truncating its list.
The UI exposes Ctrl+R (Windows) and Command+R (Mac) to refresh physical
identities without rerunning catalog search. Tests and manual interrupted-mount,
network-share and screen-reader scenarios are detailed in
[UX2 QA](qa/PHYSICAL_USB_IDENTITY_WP_20261101.md). This is source integration,
not validated actual USB hardware or manual GUI acceptance. An OS volume UUID
is not a globally unique USB hardware identifier; clones require manual review.

## UX3 disk-level candidate navigation — 2026-11-01

The same draft PR #57 now implements **complete name+recorded-size candidate
counts per catalogued disk**, independently of the current result page. Both
Win32 and SwiftUI can focus candidates to one catalog disk and switch to all
registered disks, with bounded pages, unchanged original selected entry ID and
Back navigation. The service returns `disk_groups`, `all_total` and a
filtered `total`; linked CTF validates generation SHA for every focused page.
These are catalogue metadata candidates, never proof of byte equality, an
attached physical USB disk or permission to delete a file. Manual UX,
accessibility, device and 1366x768/HiDPI acceptance remain outstanding.
See [UX3 QA](qa/UX_GLOBAL_SEARCH_AND_PAIRING_PICKER_20261101.md).

## UX4 — OS-verified copy-disk navigation (2026-11-01)

Native candidate-disk chips/combobox now display catalogued **name+recorded
size** candidate counts alongside the **current OS pairing state** (verified,
offline, conflict or unverified). Windows Ctrl+G/View and macOS Command+G/
accessible button explicitly request a **fresh complete native OS mount scan**
before focusing a disk that has exactly one paired, verified candidate group.
Zero or two+ eligible disks never trigger an automatic guess; operator chooses
manually. Win32 refreshes copy-picker badges when OS mount status changes;
SwiftUI validates probe generation, source scope and selected file ID before
asynchronous navigation. This changes no CTF/SQLite files or user bindings.

Paired filesystem volume UUID is not a guaranteed manufacturer USB serial,
and **OS presence never implies the catalogued file actually exists**. No
content hashes/equality or deletion permission is inferred. Exact linked CTF
source-generation pinning is unchanged. Manual Win10/11 and macOS ARM/Intel
USB clone/plug/unplug, keyboard and accessibility acceptance is still blocked.


## UX5 — user-entered disk sticker/shelf tags (2026-11-01)

An offline USB catalog must tell the operator which physical labeled disk to retrieve without claiming it is mounted. The new Win32 sidebar Save control and macOS SwiftUI locator Save control store a user-authored sticker/shelf tag in a private user-owned SQLite sidecar, scoped to catalog generation, platform, and catalog disk ID. The native register/inspector show it; the macOS register filter can search it. Empty Save clears it. Native Rust service validates Unicode length, control characters and explicit confirmation. **A sticker tag is not an OS identity or a content hash and never changes a connected/offline state.** Original CTF, derivative caches, annotation store and catalog rows stay untouched.

See [UX5 QA](qa/UX_SHELF_TAGS_20261101.md). Exact-HEAD four-platform CI and real Win10/11/Mac ARM/Intel keyboard/a11y/USB checks remain required; stable 1.0 remains blocked.


## UX6 — retrieval note and shelf-tag discoverability (2026-11-01)

The native Windows and macOS apps can explicitly copy a **catalog retrieval note** for a selected file to the local clipboard: disk label and catalog ID, recorded location, private operator-entered physical sticker/shelf label, metadata candidate count and the most recent OS pairing state. Windows: Copy locator text / Ctrl+Shift+C. Mac: Copy retrieval note / Command-Shift-C. Windows disk-register Ctrl+L filter now matches private sticker/shelf tags as well as labels and catalog IDs, just as Mac's register filter already does. This is a retrieval aid, *not* verified presence of bytes, a current path, matching hashes, nor a reason to delete copies. The source remains read-only and generation-pinned; saving a sticker is a separate explicit user-SQLite action. Real hardware, screen-reader, keyboard, 1366x768/HiDPI, multi-OS and p95 acceptance remain outstanding. See [UX6 QA](qa/UX6_RETRIEVAL_NOTE_20261101.md).


## UX7 — selected file to exact catalog disk in left register (2026-11-01)

Win32 Show disk / Ctrl+J and SwiftUI Show disk in register / Command-J provide a direct retrieval route from a selected global-search result to a physical-shelf lookup. The action filters only the register to the exact current catalog ID (id:42 for disk 42); partial IDs, labels and private sticker tags cannot produce a misleading match. It does not clear global/copy results, alter search scope, change selection, access a mounted USB drive or claim hash equality. Selecting the shown disk with a separate click is the deliberate browse operation. An absent/stale result or unknown disk fails closed. See docs/qa/UX7_EXACT_DISK_REGISTER_REVEAL_20261101.md. CI is required on the final SHA; real GUI, keyboard/a11y and hardware acceptance remain open.


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
[UX8 QA](qa/UX8_CANDIDATE_DISK_SHELF_LOCATOR_20261102.md).
The **new exact-HEAD CI results must be checked** before declaring this block
green. PR stays draft/unmerged; no stable 1.0, no private data in git.

## UX9 — offline physical disk pickup checklist (2026-11-02)

Native Windows and macOS can locally copy a physical retrieval shortlist after **Find copies**. This uses complete per-catalog-disk name+size metadata candidate counts, not the loaded page, and prints exact disk IDs plus private user shelf tags and explicitly last-checked OS pairing states. Windows Ctrl+Shift+P and macOS Command-Shift-P accompany visible actions. Connected candidates appear first only after independent OS pairing/status verification; offline, unverified and conflicting identity states are distinct. Order among equal states is by descending count and exact numeric ID. The text cannot establish current file presence, byte/hash equality or permission to delete; recorded paths and tags are not OS hardware evidence. Recheck external CTF generation on each actual service page read. No CTF, cache, private notes or source bytes are written by copying. See [UX9 QA](qa/UX9_PHYSICAL_PICKUP_LIST_20261102.md); CI-only evidence does not close real hardware/GUI/release gates.


## UX10 — safe async page transitions and current-page path handoff (2026-11-02)

Win32 and SwiftUI now clear previous search/candidate rows immediately on a new asynchronous page request, with selection invalidated. A stale page is never presented as the newly requested scope, and clipboard/verified-copy actions remain unavailable until the matching latest page succeeds. Failed SHA-generation validation leaves the page unready and empty; it does not silently fall back to previously loaded catalog rows. This is **in addition** to the existing generation-token checks on asynchronous replies.

A new **Copy page paths** action (Windows Ctrl+Shift+M / View menu / native button; Mac Command-Shift-M / accessible native button) copies at most the **current 250-row candidate page** to the local clipboard with exact catalog disk IDs, recorded names, historical relative paths, stored sizes, scope and page range/total. It does not fetch more pages, access USB hardware, claim those historical files currently exist or export private data to CI. The header unmistakably says **CURRENT PAGE ONLY**, so it cannot masquerade as a complete all-disk export. Linked CTF generation is checked by each service request, not by the later clipboard action; no CTF/cache/user DB is mutated.

Synthetic compiled Win32 unit tests check page range, disk IDs and newline-safe text; cross-platform source wiring tests cover stale page invalidation, keyboard/a11y commands and explicit uncertainty. Manual Win10/11, macOS ARM64/Intel, HiDPI/1366×768, plugged/unplugged USB, keyboard and screen reader testing remain required. See [UX10 QA runbook](qa/UX10_CURRENT_PAGE_PATH_HANDOFF_20261102.md). Do not assert final exact-HEAD CI green until all four workflow families finish successfully.


## UX11: full native OS-volume scan (2026-11-02)

A physical disk's connected/offline badge must not depend on whether Windows currently assigns it a drive letter. Win32 enumerates all live OS volume GUIDs and reports filesystem serial plus current mount-point hints (including folder-mounted or letterless media). Any incomplete scan fails closed. SwiftUI includes hidden mounted local volumes in its UUID scan; remote mounts do not count and unreadable local UUIDs fail closed. Only explicit per-catalog-generation pairing and a complete live OS check can label a disk connected. No recorded path, user sticker, filename+size candidate count or OS mount proves file bytes or content hash. See [UX11 QA](qa/UX11_ALL_VOLUME_PROBE_20261102.md). Real devices and native GUI still need manual acceptance.


## UX12 — ambiguous operator sticker / shelf tags (2026-11-02)

An operator may reuse a physical sticker on multiple registered catalog disks.
The private physical_shelf_labels response now returns sorted ID-only collision
groups for the exact catalog generation/platform/requested disk IDs (Unicode
lowercase, collapsed whitespace). This is a physical retrieval warning, never
a uniqueness constraint or OS-volume identity statement.

Native Win32 shows SHARED STICKER in the disk register, copy picker, selected
file locator and copied physical pickup checklist. Native SwiftUI shows the
same in the register, candidate picker, inspector and shelf editor with
VoiceOver-accessible exact other disk IDs; Mac reloads server-derived warnings
after saving a tag and ignores stale scope/token responses. All evidence
remains distinct from OS pairing, file existence and content equality. No
source CTF/cache mutation, no hash-equality or deletion claims.

Runbook: qa/UX12_SHELF_COLLISION_AUDIT_20261102.md.
Run final exact-HEAD Linux/Windows/macOS ARM/Intel, integration, offline and
installed-upgrade CI; human 1366x768/HiDPI, keyboard, screen reader and real
USB tests remain open. PR #57 draft; stable release blocked.

## UX13 — show candidate disks in the physical register (2026-11-02)

Both native shells expose a compact view-only `copies:` disk register mode from the selected file's metadata candidate groups (Windows Ctrl+Shift+K, macOS Command-Shift-K/Candidates). Matching uses exact catalog disk IDs, not shelf strings or historical drive letters; all-disk search and current result selection are preserved. The register returns to its normal filter when copy mode ends. Unit and native-source wiring regressions test 4/42/142 collisions, filtered index mapping and keyboard accessibility. Manual 1366×768/HiDPI, OS mount and actual physical pickup inspection are required before acceptance. See [UX13 runbook](qa/UX13_CANDIDATE_DISK_REGISTER_20261102.md).


## UX14 — prioritized physical retrieval in candidate disk register (2026-11-02)

The native candidate-only left register now becomes a usable physical pickup
shortlist instead of an unsorted list of catalog volume names. Both Win32 and
SwiftUI order the exact candidate catalog disk IDs by **last OS observation**
(connected only when explicitly paired and OS-verified, then offline, unverified,
conflict), descending complete per-disk name+recorded-size candidate count,
then ascending exact disk ID. Each row visibly leads with the disk ID and the
candidate count, followed by operator sticker/shelf label and OS state;
shared-sticker warnings remain visible. This order is independent of the
current 250-row results page. Filter mode changes only the left register,
without altering global search, selected result, physical pairing or CTF.
Source remains read-only and generation-pinned.

Rows explicitly distinguish *name+size metadata candidates* from verified
content copies. Last OS observations are not a live byte-presence check.
New compiled Rust ordering/caption tests, dual-native wiring assertions and
manual Windows/macOS runbook: [UX14 QA](qa/UX14_CANDIDATE_REGISTER_PRIORITY_20261102.md).
Exact-head CI and actual Win10/11/macOS ARM/Intel accessibility, 1366x768,
HiDPI and unplug/replug acceptance remain mandatory; draft PR #57 is not a
stable release.

## UX15 — dense loaded-page Kind + results keyboard focus (2026-11-02)

Both native search tables now show the catalogue **Kind** of a row (file,
directory, archive) without an extra filesystem probe. Kind can sort ONLY the
current <=250-row page; Windows maintains exact column/sort indices in its
7-column adaptive layout, macOS uses native Table columns. Ctrl+E / Command+E
focuses a verified current page, selecting its first row only when no valid
selection exists; a pending/empty page cannot reactivate historical rows.
See [manual UX15 runbook](qa/UX15_DENSE_RESULTS_AND_FOCUS_20261102.md).
Build CI is not visual, accessibility, USB hardware or owner-performance QA;
linked source stays read-only and generation pinned, stable 1.0 blocked.


## UX16 — indexed metadata video-extension search (2026-11-02)

Win32 Ctrl+Shift+V / SwiftUI Command+Shift+V toggle a visible, reversible `type:video` global search predicate, or `(<original>) AND type:video`, maintaining boolean OR precedence. The core matches 13 recorded filename extensions only. This is not film identification, byte equality, online file proof or a deletion suggestion. Linked CTF SHA-pinning/read-only and private disk identity stores remain unchanged. Synthetic tests and [manual UX16 runbook](qa/UX16_VIDEO_EXTENSION_SEARCH_20261102.md) included; real hardware, UI and performance SLO not accepted.

## UX17 — exact filename across all disks, irrespective of size (2026-11-02)

A selected real file row can issue an editable, literal `name_exact:"..."` query across the complete offline catalog, irrespective of recorded byte size. Windows Win32 exposes View/Ctrl+Shift+N; macOS SwiftUI provides the locator button and Command+Shift+N. Exact match escapes quotes/backslashes, binds the parameter (no SQL interpolation), rejects query overflow, hides stale rows, and Back restores prior search and page. This complements the stricter same-name+size metadata candidates and can identify alternative file sizes, **not equivalent bytes or actual present USB files**. Linked CTF remains SHA-generation-verified/read-only; no user pairing/annotation writes. [UX17 runbook](qa/UX17_LITERAL_FILENAME_ALL_DISKS_20261102.md). Human hardware/a11y QA and real p95 still pending; draft PR only.


## UX18 — renamed-file candidates from exact recorded bytes (2026-11-02)

Both native clients offer a selected-file global query for the **same exact
positive recorded byte size regardless of filename**, complementary to UX3
(name+size) and UX17 (literal name/any size). The query is bounded to file-kind
rows, results page normally, and returns to the prior scope through Back. It
never reads USB bytes, verifies hashes, infers live mounts, or recommends
deletion; the match is particularly weak for common file sizes. Disabled for
stale selection and missing/zero/invalid sizes. Linked original CTF generation
is SHA-verified per request as before. See
[UX18 runbook](qa/UX18_SAME_SIZE_RENAMED_CANDIDATES_20261102.md).
Physical GUI QA and real 3M-row search SLO remain unverified.

## UX19 — physical disk audit (2026-11-02)

The native left register adds last OS-checked counts for **connected** (only an explicitly paired and independently observed current OS filesystem UUID), **paired offline** (identity absent in a complete OS scan), **conflict** (cloned/colliding UUID or pairing), and **unverified** (unpaired/failed/pending scan). Separate counts flag disks with missing/shared private shelf stickers, which are retrieval hints, never hardware evidence. These are audit counts of registered catalog disks, **not live file counts**.

Both clients support view-only register filters \`state:connected\`, \`state:offline\`, \`state:conflict\`, \`state:unverified\`, \`tag:missing\`, \`tag:shared\`. Reserved invalid state/tag expressions match no disks (do not become fuzzy labels). Windows **Ctrl+Shift+O/U** and macOS **Command+Shift+O/U** run a fresh OS volume UUID check and filter to connected/unverified respectively. They do not affect current search query, selected disk browsing scope or file selection. Failed/incomplete probes make every physical state unverified; they never assert offline or connected from a historical mount letter. Operator must pair each catalog disk explicitly; duplicate filesystem UUIDs are not unique physical-disk serials. See [UX19 runbook](qa/UX19_PHYSICAL_REGISTER_AUDIT_20261102.md). CI-native builds/unit tests are not human USB/keyboard/a11y acceptance.


## UX20 — native slow-page stop/retry (2026-11-02)

Win32 Ctrl+Shift+X/Y and SwiftUI Command+Shift+X/Y now offer Stop waiting and Retry page alongside the existing query and scoped results. Stop discards current page, selection and metadata candidate disk groups immediately, increments request generation and rejects any late completion. It does **not** stop background CTF/SQLite filesystem I/O or rewrite CTF/cache/user notes. Retry repeats the exact applied query/page/scope with pinned linked CTF source SHA revalidation. Failure also leaves a retry action; no old result may return as new. The primary search continues across all disks. Keyboard and accessible native controls plus synthetic Rust and Python dual-client wiring tests and the [UX20 QA runbook](qa/UX20_ASYNC_PAGE_STOP_RETRY_20261102.md) accompany the change. Build and manual native UI/USB acceptance remain independent release gates; PR #57 draft, stable blocked.

## UX21 — strict other-media copy candidates (2026-11-02)

Native **Find copies on other disks** passes `other_disks_only: true` to the read-only service for both SQLite and generation-SHA-pinned linked CTF. The filter excludes every item on the original catalog volume **before** paging, grouped disk counts and focused pages. `all_total` includes the original disk; `other_total` and the presented page include only other media. Unflagged API requests remain backward compatible and all results are metadata coincidences, never verified file equality. See [UX21 QA](qa/UX21_ONLY_OTHER_DISKS_20261102.md). Exact-HEAD CI and manual owner acceptance remain required; do not merge or publish stable 1.0.


## UX23 — sleep/resume OS state freshness and clone-safe Windows pairing (2026-11-02)

Native Win32 and SwiftUI expire independently OS-verified connection states
after 12 monotonic seconds counted BEFORE OS enumeration, rather than after a
delayed service response. Expired status becomes unverified; app foreground and
wake recheck the native OS, and neither a sticker nor historical drive letter
establishes presence. Windows pairing now rejects a second same-UUID volume
arriving during the modal confirmation, even if the original UUID+mount still
exists; the Rust service retains its independent collision rejector.

Compiled Windows expiry/clone tests, both-platform source checks and
[manual UX23 runbook](qa/UX23_RESUME_FRESHNESS_AND_CLONE_PAIRING_20261102.md)
are part of draft PR #57. Exact-HEAD CI is required before claiming PASS;
real Win10/11/Mac ARM/Intel physical sleep/hardware/GUI and p95, signing,
original/historical CTF acceptance are still open. No merge or stable 1.0.


## UX24 — physical disk-only pickup card (2026-11-02)

A catalog disk chosen directly in the left register may need retrieval even
without a file selected. Both native frontends now provide a locally copied
physical pickup card with **exact catalog disk ID ahead of the recorded label**,
manual private shelf/sticker tag, catalogued entry count, shared-tag warnings
and independently checked last OS pairing state. Windows View/Ctrl+Shift+L;
macOS SwiftUI inspector button and always-visible compact toolbar
Command-Shift-L (also works when inspector collapsed). Normal register rows
show exact catalog IDs to distinguish repeated labels.

Only a previously explicit paired UUID confirmed by a complete fresh OS scan
can display connected; stale/incomplete means **not verified**. A tag and OS
filesystem UUID do not prove the physical manufacturer serial or file presence.
No catalog/CTF/SQLite/user pairing writes, no live file read or automatic
metadata-match deletion. [UX24 manual runbook](qa/UX24_PHYSICAL_DISK_PICKUP_CARD_20261102.md).
Manual physical hardware/GUI/a11y and real SLO remain unaccepted; draft PR only.


UX24 CTF-read consistency: for a linked original CTF, each independent disk pickup card request now rechecks `ctf_volumes_pinned` generation SHA asynchronously before copying. Generation/source/selection changes reject the copy; a stale UI register alone is never proof of source validity. This gate was added after identifying a missing check in the initial implementation, along with fixing a Rustfmt-only CI failure in the same draft PR. Manual hardware acceptance remains open.



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


## UX26 — event-driven USB mount invalidation (2026-11-02)

Windows Win32 handles native `WM_DEVICECHANGE` volume/device-node arrival,
completed removal, removal-pending and query events: it immediately clears
cached positive OS pairing status, then probes live OS volumes afresh **only**
after completed changes. macOS SwiftUI listens to `NSWorkspace` will-unmount,
did-unmount and did-mount notifications: pending removal immediately revokes
positive statuses and in-flight OS-root/candidate-copy intents; completed
changes trigger a new complete OS UUID scan and independently checked binding.
Existing 6-second polling and 12-second monotonic expiry remain fallback if
notifications are delayed or missed. No OS event itself proves hardware
identity, file presence, equal file bytes, or content hashes; all positive
statuses still require a successful complete native OS probe + explicit scoped
binding. UI search, CTF generation pinning and user-side annotations unchanged.

Windows compiled event-classification tests, dual native source contracts and
[UX26 manual hotplug runbook](qa/UX26_EVENT_DRIVEN_USB_HOTPLUG_20261102.md)
are supplied in the same draft PR #57. Exact-HEAD four-platform native,
integration, offline acceptance and upgrade CI must pass; physical Win10/11,
Mac ARM/Intel plug/unplug, cloned volume, pending unmount and VoiceOver/
keyboard/HiDPI acceptance remain unverified. Stable 1.0 blocked.


## UX27 — containing catalog folder from a selected search result (2026-11-02)

Native Win32 and SwiftUI users may inspect related catalogued files next to
a selected movie by browsing its exact \`parent_id\` on its exact \`volume_id\`,
including the root for files with a null parent. The previous bounded search,
page offset and scope are restored by Back. Windows Ctrl+Shift+F or a locator
button/View menu; macOS Command-Shift-F or Folder toolbar/inspector. The
recorded relative path is a historical breadcrumb, **never** a verified OS
mount or file existence claim. Linked CTF uses SHA-pinned read-only children
pages. Synthetic tests plus native builds are necessary but do not replace
human GUI/USB acceptance; see \`docs/qa/UX27_RECORDED_PARENT_FOLDER_20261102.md\`.

## UX28 — candidate copy to recorded folder with two-level Back (2026-11-02)

A selected file in the **other-catalog-disks name+recorded-size candidate**
results may now browse its exact recorded containing folder, not just the
original global search result. Win32 Ctrl+Shift+F and macOS Command-Shift-F
work on a verified current candidate page, including with the inspector hidden.
A separate transient candidate-return context preserves the exact source
entry ID, focused candidate disk, page offset and view-only register filter.
Back first re-fetches that candidate page (fresh CTF generation SHA), then
returns to the original global search from the existing source return context.
The catalog-only folder load is itself SHA-pinned; pending/stale pages,
non-files and invalid parent/disk IDs refuse navigation. A new explicit search
or disk navigation clears the transient history. No physical file existence,
content equality or live OS mount is asserted. Synthetic Windows unit tests,
dual-native wiring regressions and [UX28 runbook](qa/UX28_CANDIDATE_FOLDER_RETURN_20261102.md)
accompany this draft vertical. Human USB/GUI and real performance gates remain
open; no merge or stable 1.0.

## UX29 — open the freshly verified physical disk root (2026-11-02)

Windows Win32 and macOS SwiftUI now offer native Explorer/Finder root
navigation from an exact catalog disk selection (Ctrl/Command-Shift-B).
This supplements, and never replaces, offline finding by catalog ID and
owner shelf tag. It checks linked original CTF SHA (including fail-closed
missing generation), explicit paired OS identity, and **two complete native
OS probes** with a unique unchanged UUID/mount root. No catalog relative
path or historical drive letter becomes an executable OS path, and opening
the root cannot prove a film's presence, contents or hash equality.
Synthetic tests + native CI are required, manual GUI/USB remains unaccepted.
See [UX29 runbook](qa/UX29_OS_VERIFIED_ROOT_EXPLORER_FINDER_20261102.md).

## UX30 — fail-closed incomplete linked source + selected metadata copies (2026-11-02)

Cross-platform native safety/ergonomics repair in draft PR #57: a linked CTF
path without generation SHA stays read-only, never falls back to the ordinary
SQLite search, candidate or disk register. Windows selected locator and macOS
selected label/path/retrieval note independently recheck source SHA before
local clipboard copying, rejecting selection/page/source changes. The macOS
physical disk pickup card also refuses partial linked sessions. Synthetic
Windows unit and dual-client integration tests; both native targets and
manual GUI/USB acceptance remain separate. See
[UX30 manual QA](qa/UX30_LINKED_SOURCE_CLIPBOARD_FAIL_CLOSED_20261102.md).
No real CTF or private data is committed, and no content equality or
physical file presence is inferred. No merge / stable 1.0.

## UX31 — selected-result contextual actions and compact macOS toolbar (2026-11-02)

Both **native** clients expose useful file actions without requiring the
third locator pane: Windows Win32 result-row right-click / Shift+F10 and
macOS SwiftUI selected Table context menu. Actions navigate recorded catalog
parent IDs, compare name+recorded-size metadata candidates on other disks,
search same filename or size, reveal exact catalog disk IDs and copy a
generation-rechecked retrieval note / physical disk card. Windows hit-tests
the clicked current ListView row rather than trusting previous selection;
stale/incomplete pages and invalid disks cannot authorize a menu. Mac groups
the previous four top-row Folder/Disk card/OS root/Finder buttons into an
accessible **File actions** menu, retaining Command-Shift-F/L/H/B while the
inspector is hidden; corrected mismatched VoiceOver labels for OS-root actions.
This does **not** read current file contents or prove USB presence. See
[UX31 manual runbook](qa/UX31_NATIVE_RESULT_CONTEXT_ACTIONS_20261102.md).
Compiled Rust context tests and cross-platform native source contracts
accompany the same draft PR #57. Exact-HEAD CI and human Win10/11 /
macOS ARM64+Intel 880/1366px, HiDPI, keyboard/a11y/hardware checks remain
required. Stable 1.0 stays blocked, RC3 unsigned prerelease only.

## UX32 — exact physical retrieval identifiers in global results (2026-11-02)

Both native search tables show exact catalog ID before user shelf sticker and
recorded disk label. This helps identify one of ~40 unplugged drives without
opening the inspector. Shared stickers remain ambiguous, unknown IDs are not
guessed and SwiftUI supplies complete VoiceOver/Help text. This is catalog
metadata, not OS-verified presence or byte/hash equality. No CTF/cache/user
annotation changes. Manual GUI/hardware and exact-HEAD CI are still required.
See docs/qa/UX32_RESULT_DISK_ID_SHELF_20261102.md.


## UX33 — current OS pairing markers in dense search results (2026-11-02)

To answer "which physical disk should I fetch?" without opening the
inspector, both native result tables place a brief state immediately after
the exact catalog ID: #42 [OS+] • Shelf • Archive, with shared tags still
flagged. [OS+] requires an explicit paired disk UUID and fresh complete OS
scan plus verified identity from the service; [OFF] is a paired UUID
absent in a complete scan; [ID!] is conflict; [?] is missing, stale or
unverified. These are *volume presence/identity* markers, never file
existence, byte/hashing equality or permission to delete.

Windows updates just the loaded 250-row Disk subitems after native
hotplug, expiry and status refresh without losing ListView focus/selection.
macOS uses its existing published OS state and gives a full accessible
description with the no-bytes-verified disclaimer. No filesystem scan per
file row, no use of historical disk letters, CTF mutations or notes writes.
Windows compiled synthetic tests and dual-native Python contracts added.
See [UX33 runbook](qa/UX33_LIVE_OS_BADGES_IN_RESULTS_20261102.md).
Native exact-HEAD CI and human hardware/visual tests remain essential;
stable 1.0 release gates are unchanged.

## UX34 — consistent live OS status on every retrieval surface (2026-11-02)

Fixed an independent status inconsistency: the search-row badge already
required an actual verified OS identity, while secondary native disk/copy
views sometimes trusted only the physical-status service string. Windows
now reuses one freshly verified ID+TTL state in the native register,
copy-disk picker, inspector, pickup cards and clipboard, ranking, filtering
and sole-connected shortcut; macOS central physicalState requires exactly
one current OS observation of a verified non-empty filesystem UUID.
Synthetic regression/runbook: [UX34](qa/UX34_CONSISTENT_OS_STATUS_20261102.md).
Exact new-head CI and real USB/GUI acceptance remain separate release gates.

## UX35 checkpoint — exact reselected search film (2026-11-03)

Windows Win32 and macOS SwiftUI now reselect the exact entry and catalog disk
only after Back obtains a new, generation-pinned page. Keyboard focus returns
to the current result; two-level candidate → folder → original-search navigation
preserves both selections. Stale, failed, missing or ambiguous rows fail closed.
See [UX35 manual runbook](qa/UX35_EXACT_BACK_SELECTION_20261103.md).
No live file, historical mount, hash or deletion claims; exact-HEAD CI and
hardware/GUI release gates still apply.


## UX36 — verified keyboard-first pagination and fresh result focus (2026-11-03)

Windows Ctrl+Alt+PageUp/PageDown and macOS Command+Option+Left/Right
Arrow navigate the currently verified search/copy-candidate page.
The current query, explicit disk scope and contextual Back history survive.
Both clients disable paging during pending, stopped or failed reads,
never infer next-page availability from historical state, and requery
the original linked CTF with an independently checked source generation
SHA on each new page. A successful, exact-generation page focuses its
first actual row; no previous page entry is reused during loading.
Windows has checked offset arithmetic. macOS announces verified pager
state for accessibility. See
[UX36 runbook](qa/UX36_VERIFIED_KEYBOARD_PAGING_20261103.md).
Physical file presence, hash equality and safe deletion are NOT implied.
Synthetic native tests are not hardware/GUI acceptance; stable 1.0 remains blocked.

## UX37 — native debounced filename preview (2026-11-03)

Integrated implementation candidate in the existing **draft PR #57**:
the Windows search field uses a native one-shot `EN_CHANGE` 450 ms timer;
macOS SwiftUI uses a cancellable query-keyed task. Pausing after a
2+ character plain Unicode filename phrase automatically searches
**all catalogued disks** through the existing bounded asynchronous query
and current-generation response guards. Each linked CTF page still rechecks
original generation SHA; typeahead never accesses a physical movie file.

Boolean operators, `name:`, `ext:`, `type:video`, `size:`, path
punctuation, and single characters are not previewed; Enter/Return
submits those expressions explicitly. Win32 programmatic text updates
cancel pending timers, preventing stale search from overriding recorded
folder/Back and disk/copy navigation. Mac revalidates the query immediately
before dispatch. Users do not need to mount a catalog disk to search it.
No query is persisted, no CTF/SQLite writes, and no connectivity, hash,
file-presence or duplicate-equality claims. Physical pairing stays verified
only by a separate complete, fresh OS probe.

[UX37 manual acceptance protocol](qa/UX37_LIVE_FILENAME_SEARCH_20261103.md).
Exact-HEAD native 4/4 + integration/offline/upgrades are required before
calling the implementation build-validated. Human 880/1366px/HiDPI,
Win10/11 and Mac ARM/Intel GUI/keyboard/VoiceOver/USB, modified original
and provider-blocked historical CTF checks, signatures/notarization and
measured real p95 <=150 ms remain open. RC3 unsigned prerelease;
stable 1.0 blocked, draft PR unmerged.
