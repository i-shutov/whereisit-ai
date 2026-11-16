# UX8 — physically locating candidate disks with private stickers

**Status:** source implemented in draft PR #57; human Win10/11, macOS ARM64/Intel,
HiDPI, accessibility and real USB hardware acceptance remain **pending**.
Run only with the owner's local catalog; do not publish filenames, private
paths, user SQLite/CTF, source hashes or screen captures containing private data.

## Purpose and invariants

In a name+recorded-size *metadata* candidate list, each disk option must say
the **exact catalog disk ID**, its catalog label, its separately saved
**user-provided shelf/sticker tag**, candidate count and current OS-verification
state. A sticker is an offline retrieval aid, **not** a mounted-device identity,
proof of actual file bytes or permission to delete a copy. Current UUID-based
OS pairing is independent and may collide on cloned filesystems.

## Manual acceptance matrix

1. With synthetic disk IDs **4**, **42** and **142**, use duplicate catalog
   labels "Archive" but set private shelf tags **Rack A / 04** for disk 4 and
   **Полка Д / USB-42** for disk 42. Leave 142 untagged.
2. Globally search a synthetic film/file present on all three disks with
   identical normalized filename and recorded size. Use **Find copies**
   (Win32 Ctrl+D; macOS Command+D). Inspect every chooser item: exact #4,
   #42, #142, correct sticker or "no sticker tag", counts and state; no
   inference from an accidentally matching label such as "id:42".
3. Change the sticker tag for a candidate disk while copy mode remains open.
   Confirm Win32's chooser updates without losing disk focus, and SwiftUI's
   button text and VoiceOver label update. Verify the private tag does not
   change OS mount/connected status, copy counts or CTF bytes.
4. On Mac, rapidly switch between two catalog generations while shelf-label
   requests are outstanding; only the active catalog's sidecar tags may
   appear. Save a new sticker while an older list request is still pending:
   the previous response must not overwrite the saved value. Repeat with
   a failed sidecar read (empty/unverified is safer than old tags).
5. Check an unpaired disk, a paired-but-unplugged disk, an ambiguous cloned
   UUID and a uniquely OS-verified connected disk. The chooser must state,
   respectively, **not verified**, **paired/offline**, **identity conflict**
   and **OS verified now**, independent of stickers. Confirm a failed or
   incomplete OS probe never labels a disk connected.
6. Navigate with keyboard only and screen reader (Narrator on Win10/11,
   VoiceOver on Mac ARM/Intel). Candidate options must be discernible by
   catalog ID and sticker; Mac must announce "metadata candidates" and
   lack of content verification. Test 880, 1100, 1180 and 1366px windows,
   scaling/HiDPI, long Unicode names and 80-character sticker labels.
7. Switch between a linked CTF generation and a SQLite catalog, including
   different IDs with the same catalog label. Tags must be scoped privately.
   On each linked CTF candidate-page request, generation SHA validation
   must remain enforced; never edit or upload the source, mirror or user
   data. Verify Back preserves the source result and no automatic deletion
   or content-equivalence assertion appears.

## CI evidence contract

Run exact-HEAD Linux Rustfmt/Clippy/unit tests, Windows native+installer,
macOS ARM64+Intel native+installer, Python native wiring, offline acceptance
kit and installed RC2→RC3 upgrades. CI synthetic fixtures do not replace
owner hardware/manual GUI, original modified CTF/historical-CTF coverage,
real 150ms p95 measurement, signing or notarization.
