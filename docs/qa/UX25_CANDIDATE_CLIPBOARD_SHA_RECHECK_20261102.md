# UX25 — on-demand linked CTF clipboard SHA verification (2026-11-02)

Scope: same draft PR #57, native Windows Win32 + macOS SwiftUI. This is
a manual operator runbook, not evidence that the owner's hardware or historic
CTF sources were retested.

## Defect corrected

Previously, **Copy pickup list** and **Copy page paths** reused the page-time
CTF SHA check. If the external file changed after the page loaded, pressing a
clipboard command could copy stale metadata without verifying its original
generation again. Both native clients now issue an independent asynchronous
`ctf_volumes_pinned` generation check for **each clipboard action** in
linked CTF mode. In SQLite catalog mode they copy the current loaded page
without treating it as linked CTF proof.

Source disk is explicitly excluded from the **other-disk** name+size
candidate lists. The old claim that the selected file was included in these
lists was inaccurate and has been removed.

## Acceptance walkthrough

1. On Windows and macOS, link a disposable synthetic CTF, select a file,
   show candidate copies on other disks, then use **Copy pickup list**
   (Win Ctrl+Shift+P / Mac Command+Shift+P) and **Copy page paths**
   (Win Ctrl+Shift+M / Mac Command+Shift+M). A read-only SHA check must
   complete before the clipboard is updated. Verify exact disk IDs, labels,
   bounded page scope, and explicit **OTHER catalog disks** wording.
2. Change a **test copy** of the CTF after loading candidates but before
   using each clipboard action. The new action must refuse copying and
   leave the clipboard as it was, with an actionable status. Never alter
   owner originals; the original source is read-only.
3. Begin clipboard verification, then navigate to another page, change
   candidate source/disk focus, switch catalogs or press Stop waiting.
   A late reply must not restore old clipboard text. Trigger two actions
   rapidly on Mac: only the newest action may copy.
4. Confirm the Win32 serialized in-flight guard prevents overlapping
   generation checks from authorizing the wrong action. SQLite mode is
   allowed to copy the current ready page immediately, with no CTF claims.
5. Exercise Cyrillic names, exact duplicate catalog labels with different
   IDs, empty candidate page, 250-row bound and screen-reader notices.
   Repeat on Win10/11 and Mac ARM64/Intel, 880/1180/1366px and HiDPI.

No hashes/content equality, current file presence, safe deletion, historical
mount letter or successful physical USB identity is inferred. The metadata
is copied only to the local clipboard. CTF originals remain strictly
read-only, derivative caches and annotations remain separate. Tests:
Windows compiled guard unit test, dual-native source regression, exact-head
four-platform CI and installed RC2→RC3 upgrades.

RC3 remains unsigned prerelease. Stable 1.0 still requires the real
hardware/GUI/OS validation, original modified and historic CTF checks,
signing/notarization and observed real p95 <=150 ms.
