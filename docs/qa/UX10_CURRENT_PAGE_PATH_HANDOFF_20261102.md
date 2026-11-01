# UX10 — stale-page protection and current-page candidate path manifest

Date: 2026-11-02. Repository: i-shutov/whereisit-ai, draft PR #57.

## Native acceptance walkthrough

1. Open a synthetic catalog with two disks and >250 exact-name+size metadata candidates. Search across all disks, select a file, enter **Find copies**, then choose a specific catalog disk. The UI must hide the preceding scope's rows immediately; before the new page is received no old selected path may be copied or used as a newly loaded result. Navigate Previous/Next rapidly, including while a query is still running. The latest request alone may populate the table.
2. Windows 10/11: inspect the copy-mode row at 880/1100/1180/1366px and HiDPI. There must be no overlap between disk selector, Copy page paths, Copy pickup list, Verified copy, and Notes controls. Use native button, View menu and **Ctrl+Shift+M**, including with the right inspector collapsed. Clipboard copy must restore original inspector text and preserve search scope.
3. macOS ARM64/Intel: use **Command-Shift-M** or the accessible Copy page paths button. VoiceOver should announce that the action covers the currently loaded candidate page only. While a page is loading, Copy paths, Pickup list and Verified copy must be disabled. Verify keyboard focus and dark/light appearance.
4. Paste clipboard only to a local temporary text editor. Confirm exact catalog disk IDs, recorded file names, relative paths and stored sizes, selected scope and **rows 1–250 of >250**. The next page must have a different offset and never claim to include unvisited rows. The all-disk view must not be confused with a disk-filtered page. Embedded CR/LF/tab characters must not inject fake checklist rows.
5. In linked CTF mode, change the CTF source after the previous generation was pinned, then request another candidate page. SHA mismatch must refuse the request; the screen must not retain stale file rows or permit a stale manifest. A clipboard copy of already loaded metadata does not perform a new SHA check; it must not claim otherwise. Both source bytes and generated mirror remain unchanged.
6. Test OS unplug/replug/clone and incomplete mounts separately. Manifest paths are historic catalog metadata, no current drive letter or successful file read is implied. Filename+size matches do not establish content equality, hashes or safe deletion. Clipboard content may be private: never paste into public Actions, issues, releases or telemetry.

## Exact-head automated acceptance

Run the existing Linux Rust tests/fmt/Clippy, compiled Win32 unit tests and native Windows installer smoke, macOS ARM64/Intel native builds/installers, Python native contract/integration regression, offline synthetic kit and RC2→RC3 installed upgrades. Inspect every job and failed step on the *exact* PR SHA. Keep PR draft if even one gate is pending/failed; none of the above replaces owner physical-USB/human visual testing.
