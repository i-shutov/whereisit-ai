# UX15 — dense file-kind results and keyboard focus (2026-11-02)

Scope: draft PR #57 on Windows 10/11 Win32 and macOS ARM64/Intel SwiftUI.
The interface renders catalogue metadata only; no USB file access, content hashing,
physical-presence inference or source CTF mutation.

## CI contract

- Windows pure Rust: the Kind header and loaded-page sort stay in lockstep with
  entry JSON, numeric Size/Date/Candidates sort indices shift correctly, missing
  metadata stays last; current result-page focus rejects pending/empty pages,
  refuses an out-of-range stale selection and can select the first current row.
- Windows adaptive widths: 7 columns (Name, Disk, Kind, Size, Modified,
  Candidates, recorded Path). At 880px Path remains visible; when the third
  pane is visible at intermediate widths, Path collapses because the inspector
  retains the complete recorded location.
- macOS: Kind is a sortable **loaded-page-only** field; the compact native
  Table includes all 7 columns. Command-E selects the first current result only
  if there is no valid selection and focuses the native Table. Page loading
  drops its prior selection/focus; no stale page action is possible.
- Dual-native Python source-contract test; exact-HEAD Linux Rust tests,
  Rustfmt/Clippy; Win32 x64 and Mac ARM64/Intel builds/installers; offline,
  integration and RC2->RC3 installed-upgrade gates. CI does not prove visual QA.

## Manual owner acceptance (still required)

1. At 880/1100/1180/1366px and 100%/150%/200% DPI, open 40+ registered
   disks. Search a film title globally with Enter, then Ctrl-E / Command-E.
   Verify focus enters the **current** result table, and arrow keys move
   selection while its exact catalogue disk ID/locator updates.
2. Distinguish a file, directory and archive by the **Kind** column. Sort
   by Kind, then by Size; confirm only the currently loaded <=250 rows move.
   The selected file must retain its stable entry ID across sorting.
3. Start page Next or a new global query while focus was on a result, then
   trigger Ctrl-E / Command-E before the new page verifies. Neither client
   may expose stale selection/previous paths. Empty results must remain empty.
4. Move to disk register (Ctrl-L / Command-L), then back to results. Ensure
   this focus movement changes neither current global search nor physical
   pairing, metadata duplicate scope or linked CTF generation.
5. With a real disk disconnected / reconnected, test Narrator/VoiceOver.
   Independently verify mount identity before any "connected" claim; file
   content and historical locations are not automatically verified.

Do not put real CTF, SQLite, filenames, sticker labels, source SHA or private
paths in a test log, screenshot artifact or issue. Stable 1.0 remains blocked
by owner hardware and GUI acceptance, original modified / historical CTF
validation, signing/notarization and real <=150ms p95.
