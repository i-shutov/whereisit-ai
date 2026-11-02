# UX29 — open an OS-verified physical disk in Explorer / Finder
Date: 2026-11-02. Existing draft PR #57.

## Real native retrieval workflow

A selected global-search file or left-register disk may request **Open OS root**:
Windows Win32 native locator / View menu / Ctrl+Shift+B; macOS SwiftUI
Finder toolbar (even with collapsed locator) / contextual inspector /
Command-Shift-B. It opens ONLY the currently observed OS disk root. It NEVER
reconstructs a supposed file location from a historical CTF path, drive
letter, label or personal shelf tag. Copy OS root (Ctrl/Command-Shift-H)
continues to copy this verified root without opening anything.

Safety conditions for BOTH actions:
1. Exact current catalog disk ID and explicit previously confirmed OS volume
   pairing must exist. Offline / unverified / conflict / failed probe refuse.
2. In linked-CTF mode, revalidate the original CTF generation SHA on each
   action; a missing generation is failure, NOT unlinked/SQLite fallback.
3. A fresh complete native OS enumeration plus service status confirms the
   unique paired filesystem UUID. A second independent OS enumeration must
   return exactly that UUID and the same current mount root. Same-UUID clones
   on any second mount or changed roots reject the action.
4. Windows launches the absolute OS root as a ShellExecuteW path, NOT as a
   command line; macOS uses NSWorkspace.open on the live mounted-volume URL.
   Failure or disappearing USB root is reported. A successful OS launch request
   does NOT verify the catalogued file exists, its contents/hash or deletion.
5. Hotplug notifications revoke pending actions and positive state; source
   and selection changes invalidate stale asynchronous callbacks.

## Synthetic CI regression

Windows compiled `open_native_root_refuses_changed_mount_or_cloned_uuid`
covers same root, remount, UUID clone, disconnected and wrong catalog ID.
Python `test_ux29_open_verified_native_root_and_refuse_missing_ctf_pin`
checks source gating, both native action pipelines and discoverable keyboard
controls. Existing UX22–UX28 regressions stay active; exact HEAD must pass
Linux Rust tests/fmt/clippy, Windows native, macOS ARM64/Intel native,
integration, offline kit, installed RC2→RC3 upgrades. Synthetic probes and
compilation are not physical USB/visual acceptance.

## Human acceptance, pending

On Windows 10/11 and macOS ARM64/Intel: locate a film while all disks are
disconnected; confirm OS-root action refuses. Connect the shelf-labelled
drive, explicitly pair it and refresh; keyboard/visible button/accessibility
must open Explorer/Finder at the TRUE ROOT, not a historical file path.
Disconnect during linked CTF validation, between probes, immediately before
OS launch; no stale root may be selected, and OS rejection must be reported.
Remount with a different letter/mount point; a new probe must select only the
new root. Attach same-filesystem-UUID clones at different roots; reject.
Mutate linked source or remove its generation pin; refuse. Test 880/1366px,
HiDPI, VoiceOver/Windows screen reader and clipboard search-state retention.

Already documented real CTF acceptance (44 disks / 3,046,027 entries) is
distinct from this synthetic UX29 regression. The two historical CTF raw
payloads remain provider-blocked; do not request their re-upload. RC3 remains
unsigned prerelease. Stable 1.0 remains blocked by modified-original CTF,
historical CTF, physical/GUI QA, signing/notarization and measured real
search p95 <=150ms. Never publish private CTF/SQLite/paths/hashes.
