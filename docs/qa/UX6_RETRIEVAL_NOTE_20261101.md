# UX6 — Offline disk retrieval note & shelf-label search (2026-11-01)

Implemented in draft PR #57 on the native Win32 and SwiftUI clients. This is a **human disk-retrieval aid** for mostly unplugged media. No live file or content equality checks occur when copying.

## Contract

- Select a catalog file/film from a global query. Inspector identifies the *catalogued* disk, owner sticker/shelf tag, recorded relative file path, OS pairing state from the **last scan**, and candidate-copy metadata. It never invents current mount letter or content hash.
- Windows: copy the current read-only locator via **Copy locator text**, View menu or **Ctrl+Shift+C**. If there is no selected file, refuse with a status message. Standard Ctrl+C on selected inspector text remains available even if a toolbar button is hidden.
- macOS: **Copy retrieval note / Command-Shift-C** in the inspector. The note includes explicit last-probe uncertainty and warns that equal-name-and-size candidates do not establish byte equality.
- Windows and macOS disk-register filters now search owner shelf/sticker tags without altering the active file query, disk binding, current file selection, or source generation. Windows uses stable filtered-row -> actual catalog ID mapping.
- Clipboard is strictly **local** and explicitly user-invoked; never log or publish clipboard contents. Normal CTF, mirror, derivative and separate annotation DBs remain unchanged.

## Native/manual acceptance

1. In a synthetic 3-disk catalog, add a Cyrillic shelf label to disk 2, leave disks 1/3 unlabelled; filter by a mixed-case shelf substring. Only disk 2 should display, and scoped disk selection must still resolve to disk 2. Clear filter and confirm global search sees all disks.
2. Search globally for a file, copy the note, paste to a local text editor. Check catalog disk label/ID, sticker, recorded relative path, metadata-only copy count and honest physical status. Copying with no selected row must not paste a stale selected file.
3. Unplug disk after successful OS scan, refresh physical OS state, copy again; only a currently and explicitly OS-verified match may say connected. Repeated cloned filesystem UUIDs and incomplete probes must be conflict/unverified, never guessed.
4. Test Windows 10/11 native clipboard, Ctrl+Shift+C, Ctrl+C in edit, Ctrl+L filter, focus traversal, 880/1180/1366px and 125/150% scaling; test Mac ARM64/Intel Cmd+Shift+C, VoiceOver, split pane and dark/light mode. If locator hidden, show it before Mac copy.
5. Switch to a different linked CTF generation; previously saved shelf tag must not appear in the new scope. Copy remains a read-only operation; source bytes/generation SHA unchanged and no service or OS mount-path probing triggered.

**Not accepted by CI alone:** actual USB plug/unplug/clone/label verification, human screen-reader ergonomics, historical blocked CTFs, original-program modified CTF roundtrip, real p95 150ms, signed/notarized stable 1.0. Public CI uses only synthetic data. Previously documented real 44-disk / 3,046,027-row verification remains distinct.
