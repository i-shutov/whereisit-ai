# UX33 — live OS pairing badges in search results

Date 2026-11-02. Integrated only in draft PR #57. No stable 1.0.

## States (metadata disclosure, not content verification)

The Disk cell is ID-first and compact, including when the right pane is
hidden. Markers are: [OS+] only for a *fresh, complete native OS probe*
with explicit pairing and a unique verified physical filesystem ID;
[OFF] only for a paired ID absent from a complete scan; [ID!] for identity
conflict; [?] for stale, incomplete, missing or unverified state. They
do not check whether any indexed filename still exists on the OS mount.
Never use them to claim equal contents or to delete any file.

Windows ListView updates only Disk subitems; macOS SwiftUI recomputes
captions and VoiceOver/Help strings from published paired OS state.
No source CTF/SQLite/caches are written. Personal shelf stickers are
user-owned annotations, not verified physical serials.

## Manual acceptance checklist (not established by CI)

1. Windows 10/11: search all disks and confirm leading exact #ID plus
   badge with inspector hidden/open at 880, 1100, 1180 and 1366px.
   Select a result and keep it selected while six-second OS checks run;
   arrow/Tab/Shift+F10 navigation must not jump or drop keyboard focus.
2. macOS ARM64/Intel: repeat on normal and HiDPI displays. Confirm
   VoiceOver and Help fully describe current paired OS state and explicitly
   disclaim checking file bytes, even when the narrow visible cell clips.
3. Unpaired catalog disk and any incomplete/failed scan must show [?].
   A user-entered label, historical drive letter, sticker, or CTF record
   must never turn the marker positive.
4. Explicitly pair a real USB volume and attach it. Verify [OS+] appears
   only after a complete OS scan and service UUID matching. The file
   itself has not been checked for existence or equal bytes.
5. Unplug while the results page remains open: the OS notification must
   revoke [OS+] immediately to [?]; only a subsequent complete scan may
   yield [OFF]. Repeat after wake or >12s without a valid recheck.
6. Add a second mounted filesystem with the same UUID: ambiguous
   identity becomes [ID!] or [?], never a guessed disk or file path.
   Repeat with two equal human stickers and Cyrillic labels.
7. Verify the new exact HEAD: native Linux/Windows/macOS ARM64+Intel,
   offline kit, integration source checks, installed RC2-to-RC3 upgrades.
   CI uses synthetic fixtures only; do not upload private owner files.

RC3 is unsigned prerelease. Original modified/historical CTF raw bytes,
real owner Win/Mac USB/GUI tests, signing/notarization and observed
large-source search p95 <=150ms remain required for stable 1.0.
