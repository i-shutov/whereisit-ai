# UX48 — reserved OS installation volume is not an offline USB disk

2026-11-03 · i-shutov/whereisit-ai · draft PR #57 only

## Why this is a safety-critical locator requirement

The application indexes approximately forty disconnected removable drives.
A Windows `C:\` or macOS `/` system volume is not automatically a removable
film drive. Older user pairings of a system volume must not yield a green
**OS connected** disk badge, a physical pickup suggestion or a live-root
Explorer/Finder action. The product uses observed volume identity, not a
historical CTF letter. Neither filesystem UUID nor volume serial is a
manufacturer hardware identity or proof of file contents.

## Implemented

- **Win32:** get the real Windows installation directory via
  `GetSystemWindowsDirectoryW`, obtain its **currently mounted volume root**
  with `GetVolumePathNameW`, require a complete OS observation containing
  that root before offering any pairing. Exclude the current installation
  volume; don't hard-code C:, don't reject all `DRIVE_FIXED` disks (many
  external USB HDDs are reported fixed).
- Refuse new pairing if the startup mount is indeterminate, missing or
  changes while the native picker/confirmation is open. Previously saved
  mapping to that root never qualifies for the positive connected badge.
  Explorer and clipboard OS-root commands refuse the startup mount.
- **SwiftUI:** deny pairing and positive status for live `/`,
  `/System/Volumes` and `/System/Volumes/*`, revalidate the same rule
  after confirmation and across the two live-root probes.
- The full raw OS observations are retained so the backend can detect
  simultaneous UUID clones. Offline catalog search and metadata candidate
  copies work unchanged while all USB hardware is unplugged.

## Synthetic evidence and manual acceptance

Run `cargo test --workspace --locked` and
`python -m unittest tools.qa.test_usb_disk_locator_contract`.
The Windows compiled tests cover non-default OS mount root, case-insensitive
root comparisons, uniquely paired USB identity, missing root and simultaneous
UUID clone collision. Both clients compile in native Windows/macOS CI.

**Manual Win10/11:** launch on real hardware, open picker and verify Windows
installation volume is absent even when the system root is not C:. Verify
a genuine USB HDD reported DRIVE_FIXED is still offered, a disconnected
catalog entry remains searchable, unplug/replug refreshes only after full
OS verification, and a legacy system-volume pairing never shows OS+ or opens
the OS root. Test Ctrl+F/keyboard picker/VoiceOver analogue and 1366x768.

**Manual macOS Intel/ARM:** startup `/` and System/Volumes mountpoints are
absent from pairing and cannot show connected. Real USB APFS/HFS/exFAT disks
may be paired only after an explicit choice and complete unique UUID
observation; test cloned UUIDs and mount/unmount, keyboard/VoiceOver, Finder
and small window. Do not infer manufacturer serial, file existence, content
equality, safe deletion or a measured 150 ms p95.

## Release boundaries

Only synthetic CI fixtures; never upload private owner CTF, derived caches,
private paths, names, SHA or secrets. CTF generation SHA remains rechecked
per linked request and source is read-only. Previous real 44-volume /
3,046,027-entry acceptance is distinct from synthetic code tests.
Modified-original and two provider-blocked historical CTF, manual native
hardware/GUI, signing/notarization and measured real p95 <=150 ms still
block stable 1.0. Do not merge draft PR #57.
