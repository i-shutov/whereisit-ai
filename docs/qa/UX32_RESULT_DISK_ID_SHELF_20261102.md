# UX32 — Exact catalog disk ID and shelf label in global search

2026-11-02. Existing draft PR #57 only. No merge or stable release.

## Functional vertical

With approximately 40 normally unplugged USB disks, duplicate historical
labels make the Disk field ambiguous, especially with the inspector hidden.
Both native result tables now show the exact catalog disk ID FIRST. A
separately user-authored physical sticker/shelf hint follows (if assigned),
then the historical catalog label. Shared stickers are flagged explicitly,
and unknown disk IDs are not guessed.

- Win32 dense ListView: #42 • Полка Б [SHARED STICKER] • Archive.
- SwiftUI native Table: same semantics, full description via VoiceOver and
  hover Help even when a narrow disk cell truncates visible label text.
- The sticker never changes OS pairing, connected/offline/conflict status,
  file content identity or search scope. It is not a hardware serial.
- Changes touch presentation only. Linked CTF stays generation-pinned and
  read-only; private sticker notes stay in the separate user-owned SQLite.
- Windows unit tests cover Cyrillic, duplicate labels/stickers, missing IDs
  and sanitization of display line breaks. Python source-wiring tests cover
  both native clients. CI uses synthetic tests only.

## Manual GUI / hardware acceptance remaining

1. At 880, 1100, 1180, 1366 pixels in Windows 10/11, hide the inspector,
   search global results with identical disk labels, ensure leading #ID can
   still be read, and inspect native keyboard/selection/accessibility.
2. Add a Cyrillic shelf tag to just one disk, then deliberately reuse it
   for another ID. Confirm the exact IDs and SHARED STICKER indicator.
3. Repeat on macOS ARM64 and Intel at HiDPI with VoiceOver; confirm full
   accessible information even when visual text is clipped.
4. After USB unplug/replug, never infer that a historical file still exists.
   Only fresh, explicitly paired OS-volume verification may say connected.
5. Relink another source generation. Old private shelf annotations must not
   leak into a different catalog generation. Do not upload CTF/SQLite.
6. Run exact-HEAD Linux/Windows/macOS ARM/Intel native builds, offline,
   integration and installed upgrade checks; do not infer human GUI acceptance.

Stable 1.0 still blocked by real-device GUI, original modified/historical
CTF availability, signing/notarization and actual p95 <=150ms measurement.
