# UX20 — native asynchronous catalog-page Stop waiting / Retry page

**Scope:** draft PR #57, Win10/11 Win32 and macOS SwiftUI ARM64/Intel, synthetic fixtures only in CI.

## Run steps
1. Launch a catalog with at least three offline catalog disks and an exact filename searched across them; verify the current page and selected-file locator. Test 880px and 1366×768 windows, keyboard focus, screen reader and HiDPI.
2. Start a deliberately delayed **new** global catalog search. Before its page returns, use Windows **Ctrl+Shift+X** or macOS **Command+Shift+X**, or the **Stop waiting** control in the bottom strip. Check old rows, selected inspector, copy actions and page counts are cleared, no stale result focus is possible; status explicitly says the reader may still finish.
3. Allow the abandoned source read to return. It MUST NOT restore rows, selection or candidate-disk state. The recorded search text/scope/offset remains available for retry. Do not treat Stop waiting as OS/file I/O abortion; the reader may still consume resources until it finishes.
4. Click **Retry page** (Windows Ctrl+Shift+Y, macOS Command+Shift+Y). Confirm a new request in the *same applied* scope returns its own valid page. Change the search while a previous operation is pending and verify only the newest generation is actionable.
5. Repeat in **Find copies** mode with candidate disks 4, 42 and 142, including the exact register filter `copies:`, then stop mid-page: no previous candidate group or picker survives. After Retry, metadata groups return only with a fresh successful response, never with a guessed USB connection or hash claim.
6. In linked external CTF read-only mode, first run on an immutable synthetic CTF fixture; change the generation fixture between stop and retry. Retry MUST perform the normal pinned SHA validation and reject a mismatched generation with no stale page fallback. Original CTF/source bytes and private sidecars must remain identical. Never stage private CTF/SQLite/path/SHA in CI.
7. Simulate core read error: the failed current page is empty and Retry is offered. Check keyboard navigation, VoiceOver/Windows accessibility labels, absence of overlapped buttons, and the distinction from the existing **Cancel scan** control (the latter is service-level scan cancellation).

## Safety/acceptance
This feature invalidates *UI responses*, not running native service workers. The `pending` count still includes abandoned operations until they actually return; mutations may remain blocked for that time. No automatic duplicate deletion; name, size or OS mounting does not prove byte/hash equality or on-disk presence. This CI contract does not certify manual Win10/11/macOS Intel+ARM64 or real USB plug/unplug usability. Stable 1.0 remains blocked by original modified CTF, two historical CTF byte access, manual GUI/hardware, signatures/notarization and owner p95 ≤150ms.
