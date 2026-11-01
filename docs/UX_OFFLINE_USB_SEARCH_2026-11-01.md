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
| UX1 — Search-first shell | Prominent global search on app launch, persisted filter/sort and resizable/collapsible panes; keyboard-first, single-screen layout at 1366×768 and high DPI | Manual Windows 10/11 + both Mac architectures; no overflow, stable focus | Planned |
| UX2 — Physical drive identity | Registered user labels, optional physical sticker ID, mount detection by stable verified volume identity and reconciliation if IDs conflict. **Never equate historical drive letters to current USB disks** | Synthetic cross-disk collisions + real plug/unplug on target hardware | Planned |
| UX3 — Film copies | Compare metadata candidates across drives and navigate from a result to all candidates. Distinguish unverified name+size from independently hashed identity; no offline deletion | Synthetic/real parity across 40-disk catalog, no false hash claim | Planned |
| UX4 — Polish and acceptance | Native accessibility, keyboard traversal, reliable cancellation/progress, contrast/light-dark themes, window geometry, upgrades and observed SLO | Owner acceptance on Win10/11, Mac Intel/ARM; RC-to-RC upgrade; measured real 3M-row latency | Planned |

No new browser/runtime dependency is introduced; the current Rust service API and native clients stay authoritative. Support for content previews/posters, movie-title grouping, and content hashes depicted in exploratory mockups is **not** implied by those images and is outside UX0.

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
