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
