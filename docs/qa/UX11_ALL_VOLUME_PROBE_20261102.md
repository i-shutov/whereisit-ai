# UX11 — full native OS-volume inventory (2026-11-02)

**Draft PR #57; not a stable release.** This is the manual Win10/11 and macOS ARM64/Intel runbook, not a claim of executed hardware tests.

## Defect and correction
Previous Win32 scan relied on GetLogicalDriveStringsW. It missed online OS volumes mounted only in directories or without drive letters, risking an incorrect offline badge for an explicitly paired disk. The Win32 shell now enumerates all OS volume GUIDs (FindFirstVolumeW / FindNextVolumeW), obtains filesystem serial with GetVolumeInformationW and displays reported mount paths via GetVolumePathNamesForVolumeNameW. The GUID+serial is the pairing identity; paths/labels are only hints. A malformed volume, inaccessible local volume, unexpected iteration error or failed close makes the snapshot incomplete (no new verified/offline claim). The service still requires explicit operator pairing and detects observed-ID collisions.

macOS no longer excludes hidden mounted local filesystems via skipHiddenVolumes; any local volume lacking an accessible UUID invalidates the whole scan, while remote mounts are irrelevant. A filesystem UUID is not a manufacturer USB serial; two clones are a conflict. OS volume presence never proves a catalogued file exists or matches its bytes/hash.

## Acceptance scenarios
1. **Win10 and Win11:** pair a physically identified USB while lettered. Remove the letter; mount in an NTFS directory, and separately test no letter/mount path. Refresh Ctrl+R. Its paired UUID must not be reported absent solely because the letter disappeared. Check exact catalog disk ID and private sticker.
2. Unplug/replug with a different letter; on recheck verify connected only for exactly one UUID. Test inaccessible/transient volume, malformed/unknown root and a second USB with a cloned filesystem UUID: no false connected/offline state.
3. Windows keyboard (Ctrl+R, Ctrl+G, Ctrl+J), screen reader, 1366×768 and HiDPI, picker with multiple similar historical labels and folder-only mounted devices. Never infer files exist on the mounted media.
4. **macOS ARM64 + Intel:** check normal, hidden and renamed mount points, unmount/replug, local UUID unavailable, network mount and cloned UUID cases. Check Command+R, Command+G, VoiceOver and compact locator.
5. After test, verify original CTF/source and derivative caches unchanged, no private IDs/paths/sticker tags in public logs, no deletion based on name+size.

## Automated CI required on exact HEAD
Windows native Rust compile and unit test for canonical GUID parsing including letterless mount; macOS ARM64/Intel compile; Linux rustfmt/Clippy/unit; dual-client source wiring; offline kit; installed RC2→RC3 upgrades. CI does not establish physical USB/GUI acceptance. The real 44 disk / 3,046,027 row CTF was verified previously, not retested by this patch. Two historical raw CTFs remain provider-blocked. Remaining stable gates: original modified CTF, manual Win10/11 and Mac GUI, signing/notarization and measured real p95 ≤150ms. PR stays draft, no stable publish.
