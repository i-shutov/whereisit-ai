# UX30 — incomplete linked-CTF and stale metadata clipboard fail closed
Date: 2026-11-02. Existing draft PR #57, no merge.

## User-visible vertical

On Windows Win32 and macOS SwiftUI, a linked external CTF **without a
trusted generation SHA** must remain in read-only linked mode. The disk
register and global or per-disk search must NEVER silently fall back to a
different local SQLite catalog. Missing generation returns an explicit error
and removes actionable stale rows, rather than showing unrelated files.
Windows native edit/maintenance actions and macOS mutation controls remain
disabled whenever an external source path exists, even if its generation
token is temporarily unavailable.

Selected-file clipboard actions now recheck the original CTF source before
copying: Windows Ctrl+Shift+C selected locator; macOS disk label, recorded
path and Command+Shift+C retrieval note. The current page, selected entry
ID, exact catalog disk and source scope must still match when asynchronous
verification returns. The physical disk pickup card on macOS also fails
closed on missing generation and rechecks its generation/page/selection.
Clipboard is unchanged after refusal. These are *historical catalog
metadata*, never asserted current OS file paths, hash-verified copies or
permission to delete.

No changes to external CTF, generated SQLite/Parquet, private annotations,
physical pairings, user files or network. Only synthetic tests are in CI.

## Automated validation

- Compiled Windows Rust unit:
  `missing_generation_never_authorizes_local_fallback` asserts path-vs-pin
  combinations. Rustfmt, Clippy and Windows native compile remain mandatory.
- Python dual-client contracts:
  `test_ux30_incomplete_linked_source_never_falls_back_or_writes`,
  `test_ux30_selected_file_clipboard_is_generation_and_page_pinned`.
- Both macOS native targets compile the SwiftUI source. Run exact-head
  Linux/Windows/Mac ARM64+Intel native CI, offline acceptance, integration
  regressions, and installed RC2→RC3 Windows/Mac upgrades. Never describe a
  queued, failed or cancelled job as green.

## Manual acceptance still required

1. With a synthetic linked CTF, search globally and select a file. Copy the
   locator/retrieval note and verify only the current source data is copied.
2. Clear/invalidate the generation in a debug-only test session while
   keeping the linked path. Search, duplicate view, candidate copies, disk
   card and all selected-row copies must refuse without showing local SQLite
   results. No edit/maintenance action becomes available.
3. Change source or selection during the async clipboard check; pasteboard
   must not be updated. Restart normally with a valid generation, confirming
   search and physical disk locator still function.
4. Inspect Win10/11, Mac ARM64 and Intel at 880 and 1366px including keyboard,
   VoiceOver/Narrator and USB disconnect; CI is not hardware GUI acceptance.

Original WhereIsIt modified-CTF compatibility, two previously supplied but
provider-blocked historic raw CTFs, owner physical USB/GUI checks, real p95
<=150 ms, signing/notarization remain release gates. Existing genuine
44-disk / 3,046,027-entry evidence is separate and is NOT rerun here.
RC3 stays unsigned prerelease; stable 1.0 blocked.
