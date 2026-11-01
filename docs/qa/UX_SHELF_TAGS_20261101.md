# UX5 — private disk sticker and shelf tags (2026-11-01)

This is an offline retrieval aid for 40 intermittently connected USB disks, not evidence of filesystem presence or file equality. Implemented in draft PR #57, not a visual mockup.

## Live features
Windows Win32: select a catalog disk or search result; type the physical case/shelf tag in the disk-register sidebar; press Save tag. Register and selected-file inspector display it, separate from the OS identity badge. Empty Save clears it.

macOS SwiftUI: edit and explicitly Save in the selected-file disk locator. The disk register shows sticker tags; the *view-only* disk register filter also searches them. The global file-search scope is unaffected.

The shared Rust service uses physical_shelf_labels (private read) and physical_shelf_label_set (explicit confirmed Save). One-line Unicode text, 80 characters maximum. The table is in a separate user-owned physical-volume-bindings.sqlite, keyed by catalog generation/source, platform and catalog volume ID. No CTF/source catalog/mirror/annotation changes. A shelf tag can never switch physical_status to connected and does not automatically move to a different generation.

## Synthetic and manual QA
1. With a synthetic catalog, save Cyrillic tag for disk 2 and verify its register and locator display; disk 1 stays blank.
2. Switch selection mid-edit: no silent write or transfer to another disk. Empty Save clears only the selected label. Reopen the same catalog source generation and check user-side persistence.
3. Switch CTF generations; old label does not appear automatically. Pair/unpair and unplug/replug; tag stays a human hint, never a connected badge.
4. Reject multiline/control characters, >80 characters, and unconfirmed service writes; confirm physical_status unchanged.
5. Test Win10/11 at 1366x768 and 125/150% scaling, Mac ARM64/Intel light/dark, keyboard, Narrator/VoiceOver and long Unicode labels. CI native builds alone do NOT prove manual ergonomics.

## Unchanged stable gate
Real owner CTF evidence (44 disks / 3,046,027 records) remains previously validated, not synthetic. The historical raw inputs remain provider-blocked. Original modified CTF, human hardware/GUI checks, live p95 150ms, Authenticode and Apple notarization remain blockers. RC3 unsigned prerelease only, PR #57 draft/unmerged.
