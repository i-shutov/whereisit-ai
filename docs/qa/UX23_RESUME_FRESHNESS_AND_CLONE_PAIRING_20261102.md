# UX23 — Physical presence freshness after sleep and clone-safe pairing

Date: 2026-11-02. Draft PR #57; not a stable-release acceptance.

## Correctness contract

- A "connected" state requires independently enumerated, complete native OS
  filesystem identities and an explicit generation-scoped, service-verified
  catalog-disk binding. Historic drive letters, shelf labels, file paths and
  name/size comparisons do not count as proof.
- Monotonic 12-second age starts before native enumeration, not when the
  potentially delayed service reply arrives. An expired green state is
  unverified until a successful new OS scan and binding check.
- Windows invalidates old state on foreground activation, timers and before
  commands. Its pairing confirmation rejects a newly inserted same-UUID clone
  even when the original UUID+description is still present.
- macOS SwiftUI rejects stale async status replies, rescans after app activation
  and NSWorkspace wake, and computes badges with the same age policy.
- No physical presence check verifies that the catalogued file currently exists
  or is byte/hash-equal. Never auto-delete matching metadata records.
- External linked CTF stays read-only with per-use generation SHA validation;
  physical binding and optional sticker notes stay in separate user databases.

## Synthetic automation

Windows: compiled \`physical_probe_freshness_and_pairing_tests\` for age and
simultaneous clones. Both native clients: regression
\`python -m unittest tools.qa.test_usb_disk_locator_contract\`.
Run final exact-HEAD native CI on Linux, Windows x64, macOS ARM64/Intel,
plus offline kit, integration regressions and installed RC2→RC3 upgrades.
Build/installer smoke is NOT human OS sleep or physical USB acceptance.

## Manual Win10/11 and Mac ARM64/Intel runbook — still outstanding

1. On disposable, explicitly paired USB media, verify catalogue disk ID and
   physical sticker separately from the currently OS-observed filesystem UUID.
2. With a green badge, sleep for over 12 seconds, resume; connected must not
   remain based solely on the old scan. New complete scans may reconnect it.
3. Delay native scanning/service response beyond 12 seconds and confirm no
   late green label; keyboard disk focus and pickup note must fail closed.
4. Insert a second cloned-UUID disposable volume during the Windows pairing
   modal, while the first remains mounted. Refuse pairing before the service
   call, and check the service rejects any collision as an independent gate.
5. Repeat with folder-mounted/letterless Windows volumes and hidden local
   macOS volumes. An incomplete snapshot means unverified, not offline.
6. Inspect dense register/candidate picker/locator on 1366x768 and HiDPI,
   accessibility with Narrator/VoiceOver, native keyboard and clipboard.
7. Do not publish private catalog snapshots, mount labels, filenames, CTF
   source SHA or user sidecar databases in reports.

Stable 1.0 remains BLOCKED by real original-modified/historical CTF acceptance,
owner manual hardware/GUI, signing/notarization and real p95<=150ms target.
