# UX7 — exact catalog disk in register: manual QA

This document describes **required** real GUI tests, not passed hardware acceptance. Test only with a synthetic public fixture; never commit a user's disk names, paths, private source SHA, source bytes or sidecar.

1. Register disks 4, 42 and 142. Add a misleading name and optional local shelf tag containing "id:42" to disk 142; a file on disk 42 must be found by global search.
2. Select the file from an all-disks result, record current page, query, selected entry, copy mode, and last OS verification state. Activate **Show disk / Ctrl+J** in Win32 or **Show disk in register / Command-J** in SwiftUI.
3. Verify only catalog disk 42 remains in the register, with its user-entered shelf hint if any, and the global results, entry ID, copy navigation and historical source remain unchanged. An explicit click on the disk is required to browse; no live USB filesystem probe or connected badge is implied.
4. Type id:4, ID:42, id:142, id:, id:-1 and id:999 as well as Cyrillic shelf-label fuzzy text; the exact syntax must not match partial numbers or misleading labels. Clear the filter and verify the register restores without changing the global file query.
5. With no file result selected, attempt the command; it must fail closed and explain why. Switch catalog generation and verify previous selection is never reused.
6. Check Ctrl+J/Command-J and visible actions at 880, 1100, 1180, 1366 widths, 100–200% scaling, Win10/11 and macOS ARM64/Intel; use keyboard-only, Narrator and VoiceOver. The two Win32 buttons on one row must be visible and separately clickable.

Exact SHA CI required: Linux cargo test/fmt/Clippy and Python wiring regression, Windows x64 Win32 build/installer, macOS Intel/ARM64 SwiftUI build/installer, offline kit and integration, installed RC2-to-RC3 upgrade. These do not substitute for physical disconnect/clone tests or the real 150ms p95.

Privacy: linked CTF is read-only and SHA-generation checked on every supported use; SQLite/Parquet caches are derived, personal shelf notes separate. An exact catalog ID is **not** a hardware serial; OS pairing remains independent and fail-closed. Name-and-size candidates are not content-verified, never deletion targets. Stable 1.0 and PR merge remain blocked pending existing gates.
