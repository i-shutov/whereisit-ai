# UX38 — explicit search intent wins over live filename lookup (2026-11-03)

Scope: **only** draft PR #57 in `i-shutov/whereisit-ai`. This is an integrated Windows Win32 + macOS SwiftUI correctness slice for the 450 ms search-first UX, not a mockup.

## Reproduced failure class

With a simple file phrase in the search box, the operator could click **This disk** or **Duplicates** before the 450 ms debounce expired. In SwiftUI, the task was keyed to the whole `@Published query`, not to user edits. Since the text stayed identical while the scope changed, the timer could issue `search()` and unexpectedly replace the explicitly requested disk/duplicate view with **all disks**. Programmatic restoration of a previously scoped search could trigger the same unintended global reload. In Win32 the normal `load()` already cancels the timer, but a rejected **This disk** (no selected disk) exited before `load()`.

## Implementation

- Windows: cancel the outstanding native Win32 timer at the **FIND / FIND_DISK / DUPLICATES command boundary**, including a rejected explicit disk-only request. Existing cancellation on every loaded page and the stale-query comparison remain.
- macOS: a native TextField `Binding` calls `userEditedSearchText`; only this method arms a pending candidate and advances a monotonic edit token. `.task(id: liveFilenameTaskID)` sleeps 450 ms, then validates **both** token and still-matching candidate. Every explicit `load()` or attempt at `This disk` invalidates the pending token, stopping accidental global scope takeover. Programmatic `query` changes do not arm typeahead.
- No CTF, cached SQLite/Parquet, private annotations, source SHA checks, or OS-volume pairings modified. Search stays global by default only for *actual user typing* or explicit Search; separate disk and duplicate scopes win when selected.

## Synthetic regression and native acceptance

1. CI: run `python3 -m unittest tools.qa.test_usb_disk_locator_contract` (source wiring contract) and Linux Rust format/unit checks, Windows x64 native Rust tests and installer smoke, macOS ARM64/Intel SwiftUI builds and installer smoke. Read the actual **exact-HEAD** Jobs/steps before claiming PASS.
2. On both real native GUIs, select a disk, type a plain phrase (at least two Unicode characters), and immediately choose **This disk** (before 450 ms). Wait >1 second. The caption, selected catalog disk ID and paginated results must remain disk-scoped. Repeat with **Duplicates**; it must not switch back to normal all-disk search.
3. Start typeahead, then **Back**, **browse disk**, **Find copies**, **browse recorded candidate folder**, or **Stop/Retry page**. No delayed global query may change the selected scope; the existing exact-ID and generation-gated page restoration must remain intact.
4. With no disk chosen, type and immediately click **This disk**: the explanatory error remains, with no late global search on Win32 or SwiftUI. Verify full Boolean/field expressions stay Return-only.
5. Type two Unicode characters, wait >450 ms without explicit navigation; global search should still run and display the correct scope. Edit again rapidly; only the final phrase should drive results. Verify Search/Enter continues working with pinned linked CTF, including SHA mismatch rejection.
6. Manual accessibility: screen reader search hint, focus after Back, keyboard shortcuts, 880/1180/1366 px, light/dark themes, HiDPI. Automated native builds do **not** establish visual or actual USB acceptance.

Never label metadata candidates content-identical, assert that a catalogued file exists on an OS-observed volume, or auto-delete. Existing genuine 44-disk/3,046,027-row checks remain historically valid; no owner private data is included. Stable 1.0 still requires original modified and two provider-blocked historic CTFs, human Win10/11/Mac ARM+Intel GUI/USB testing, signing/notarization and real p95 ≤150 ms.
