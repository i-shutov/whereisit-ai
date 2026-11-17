# Egui preview QA packaging — 2026-11-17

**Scope: source integration candidate, not a stable or notarized release.**

The Win32/SwiftUI official installers remain unchanged and still ship their previously
accepted clients. An independent Windows unsigned native egui EXE ZIP and macOS
ARM64/Intel ad-hoc signed preview .app ZIP are produced by the same exact-SHA CI.

Preview artifacts use the `dist/qa-egui/` directory, distinct bundle ID
`ai.whereisit.egui.preview`, a versioned ZIP, SHA256 and provenance. The complete
third-party license inventory is included. Never bundle user SQLite/CTF data.

Each preview must be physically launched, resized and exercised on Windows 10/11
and macOS ARM/Intel, including RU/EN, 1366x768, Retina/DPI, read-only browse,
search, export, scan/cancel and USB unplug. GitHub-hosted compilation alone does
not prove a usable GUI. Authenticode, Developer ID and notarization are still
future stable-release requirements. No stable tag or release flag changes.
