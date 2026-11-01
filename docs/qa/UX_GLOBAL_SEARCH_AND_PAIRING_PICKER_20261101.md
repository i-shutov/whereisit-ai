# Native global search and explicit USB pairing UX
Date: 2026-11-01. Draft PR #57; no manual physical-device or GUI acceptance implied.

## User workflow

| Action | Windows Win32 | macOS SwiftUI | Applied catalog scope |
|---|---|---|---|
| Focus file search | Ctrl+F | Command+F | No disk/file writes |
| Execute main search | Enter / Search all | Enter / Search all disks | All catalogued disks, even after browsing one |
| Explicit local search | Select catalog disk, This disk | Select catalog disk, This disk | Only chosen catalog disk |
| Cross-disk candidates | Select file, Ctrl+D | Select file, Command+D | Exact normalized name + recorded size metadata |
| Broad duplicates | Duplicates | Duplicates | Across all catalogued disks |
| Return from candidates | Back | Back | Previously displayed search context |

The left disk register still browses catalog snapshots without accessing physical media. Global Enter/Search and broad Duplicates clear any previously selected disk filter. The scope is displayed in the Windows location field and the Mac accessible caption. Empty global searches never preload all 3M catalog rows. The source CTF and pinned generation remain read-only.

## Physical safety and collisions

- A recorded disk label/old letter never implies current USB connection. Operator must match an actual sticker with an OS-mounted volume. Windows fixed drives can include INTERNAL/system drives; the pairing warning explicitly says so.
- Only a previously explicitly paired identity found in a successful fresh OS scan may be marked connected. No binding, probe failure, source-generation change and collision fail closed. A paired ID absent from a successful scan is offline.
- A currently observed duplicate OS UUID is never displayed as an unambiguous *pairing choice* in either native frontend. The service continues to receive the complete unfiltered OS observation array, so conflicts still report conflict rather than connected.
- The confirmation dialog is followed by a new OS scan; unplug/replace during confirmation must refuse pairing. Cloned UUID across hosts and host-scoped Windows GUID are NOT manufacturer hardware serial attestations.
- Only separate user-owned physical-volume-bindings SQLite is mutated by explicit pairing/unpairing. No file deletion, file-content hash, raw owner CTF or private path is involved.

## Reproducible and manual acceptance

Run Python native wiring regression, Windows compiled duplicate-mount picker unit test, Rust service tests plus Linux fmt/Clippy, Windows x64 and Mac ARM64/Intel native installer builds, offline kit and installed RC2-to-RC3 upgrade on the final SHA. Inspect actual Actions jobs rather than assume success.

Manual Win10/11 and Mac ARM/Intel checks: browse disk A, search by Enter and confirm results from other disks, choose This disk and verify narrow scope, switch into duplicates and confirm global scope, Ctrl/Command+D candidates then Back, keyboard/screen-reader navigation, 880/1100/1180/1366 px and high-DPI layout. Plug/unplug labelled USB; change Windows drive letter, clone or multi-mount UUID, cancel and replace during pairing, switch CTF generation, verify fail-closed status. Native CI cannot replace these hardware checks. Stable v1.0 remains blocked by earlier published gates including real p95 150ms, signatures and unavailable historical raw bytes.

## UX register filter integration — 2026-11-01

With roughly 40 intermittently attached catalogued disks, use **Ctrl+L** on Windows
or **Command+L** on macOS to focus the new *disk register* filter. Enter any
part of the recorded physical disk label or its catalog numeric ID. Filtering
is Unicode case-insensitive (including Cyrillic), works offline, and never
queries a current USB drive or CTF contents. It displays a live visible/total
counter, including when no disks match. Clear the filter using Escape in the
Windows filter field or the accessible clear button on macOS.

**Crucial:** this is strictly a view-only register filter. An already-selected
disk can disappear from the filtered list without silently changing file-search
scope, selected entry, physical pairing or CTF SHA pinning. Choosing a new
filtered row resolves the original catalog disk ID, **not the filtered row
number**. The global-search row remains available independently. Reopening a
catalog clears the register filter; global file search still searches all
catalogued disks, not just the filtered register. On Windows the selection is
visibly cleared when the current scoped disk is hidden, but the underlying
scope remains; the main location caption continues to explain the scope.

Acceptance: Windows compiled Rust index-map tests (including Cyrillic and
missing matches), native Linux Python source wiring, Windows x64 and macOS
ARM64/Intel builds. Manual QA: filter by label, then numeric disk ID; select
second filtered row and verify the actual original catalog ID in the locator;
filter out an active disk, run global search, clear filter and check scope;
use Ctrl/Command+F then Ctrl/Command+L, screen reader/Tab at 1100/1366 px.
This does not establish hardware USB identity or manual GUI acceptance.

## UX3 — per-disk metadata candidate focus (2026-11-01)

1. Search globally for a **file**, select it and use **Find copies on disks**
   (Win32 Ctrl+D, macOS Command+D). All-disk copy candidates use exactly the
   selected entry ID, normalized name and recorded size, never a path guess.
2. The candidate-disk selector above the result table reports counts for the
   **entire** metadata equivalence class (not just the current page). On Win32
   navigate the native combo using Tab/arrow keys; on macOS activate the
   scrollable accessible disk buttons with keyboard/VoiceOver. Select the first
   disk, another disk, then All disks. Each switch resets page offset while
   keeping the source file. Verify per-disk counts and all-disk total on
   synthetic two-disk inputs including two same-size files on one disk.
3. Page through a focused disk and use Back to restore original search.
   Unplugged/missing physical disks remain discoverable from catalogue data;
   choosing a disk **does not** imply it is currently OS-connected.
4. In the linked CTF mode change the source generation between pages (including
   when focused): service must reject stale generation before returning data.
   The original source and derived mirror must not be changed. No content
   hashes, verified-equality or deletion assertions may appear.
5. Test Windows at 880 and 1366 px, Mac ARM64/Intel with native accessibility,
   physical locator collapsed and visible, keyboard focus and HiDPI. Source
   wiring/unit/installer CI is not human GUI or actual-device acceptance.
