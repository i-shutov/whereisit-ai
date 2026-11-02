# UX26 — native USB hotplug/removal invalidation (manual QA)

Date: 2026-11-02. Scope: draft PR #57, Windows Win32 and macOS SwiftUI.
No real owner CTF or other private data is necessary; use synthetic named volumes.

## Objective

A previously OS-verified **connected** catalog disk must not retain a green
badge simply because a device was unplugged between periodic refresh ticks.
OS notifications **revoke** stale observations; only explicit paired UUID plus
complete fresh OS enumeration and service validation can restore connected.
This proves neither file existence nor hash identity. Files are never opened,
modified, deleted or read from an external volume by this workflow.

## Windows 10/11 manual procedure

1. Open a synthetic catalog with two disk IDs. Pair one explicitly to an
   attached test USB disk using the existing native pairing UI. Confirm its
   state only after a successful current OS scan.
2. Unplug/disable that test volume immediately after a successful scan.
   Observe removal-pending/complete: any previous connected indication must
   clear when the device event arrives, without waiting for the next 6s tick.
   With a complete OS scan, the paired disk should become offline; with an
   incomplete scan, **NOT VERIFIED**, never falsely connected.
3. Reattach with a different Windows drive letter or mount folder: another
   fresh full-volume GUID/serial OS scan, not a historical letter, must decide
   state. Verify both disk-register and cross-disk candidate badges refresh.
4. Exercise removal queried then cancelled and device-node changes; the event
   itself must never directly assert connected. A fresh full scan may restore
   the state when the device remains mounted.
5. Force a broken/incomplete volume scan and duplicate UUID/serial observation:
   verify unverified/conflict, never an optimistic connected badge.

## macOS ARM64 and Intel manual procedure

1. Pair a synthetic catalog disk to a local mounted test USB filesystem UUID.
   Verify a complete current OS scan precedes connected.
2. Unmount using Finder/Disk Utility and watch the native UI before the next
   periodic tick. The will-unmount event immediately invalidates positive
   states and any pending Copy OS root / focus connected-copy intent. After
   did-unmount, the refreshed status should be offline or unverified.
3. Remount with a different `/Volumes` suffix; did-mount must trigger a fresh
   UUID/mount check. Test a concurrently mounted clone with the same UUID:
   conflict must never be displayed as connected.
4. If unmount is cancelled or mount events are missed, periodic 6s polling and
   monotonic 12s expiry remain safety fallback. Verify no stale asynchronous
   result re-enables a positive badge after a newer mount-change event.
5. Test VoiceOver and keyboard focus on the three-pane screen at 1366×768,
   at HiDPI and with inspector collapsed; the search query and selected catalog
   file must not change because a USB volume arrives or departs.

## Automated vs physical evidence

- Windows Rust unit test checks mapping of device-change wParam event codes
  to Ignore / Invalidate / Reprobe (no Windows driver needed).
- Python native source contract checks Win32 event handler, macOS notification
  observers, invalidation of cached OS states and old async copy requests.
- Exact-HEAD Linux/Windows/macOS ARM64+Intel Actions compile/tests and upgrade
  tests are necessary; installer smoke is NOT proof of these physical events.
- Do not commit private physical IDs, CTF bytes, paths, screenshots exposing
  the owner's names, or source SHA. Do not claim stable 1.0 without the remaining
  modified/historical CTF, manual OS/hardware, real p95, signing/notarization gates.
