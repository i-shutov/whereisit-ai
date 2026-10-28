# Native UI QA

## Windows x86-64 smoke test

A debug build of the Rust/Win32 app was cross-compiled on Linux for `x86_64-pc-windows-gnu` using Rust 1.90 and MinGW-w64. It was launched with `/usr/lib/wine/wine64` in an isolated Wine prefix and an Xvfb display. The tested catalog was synthetic: 10 volumes, 10 files per volume, and one directory per volume. No user catalog was used.

Observed in the running application:

- The catalog opened and displayed all 10 volumes.
- Selecting a volume populated its root, and double-clicking the Cyrillic directory `Музыка` showed its 10 files.
- Global search `flac` returned 50 files across the catalog; Cyrillic paths, names, sizes, and timestamps were visible.
- `deliverables/native-preview.png` is a cropped screenshot of the global search on that synthetic catalog.

After the initial smoke test, the frontend received save-dialog and category escaping fixes, `search_fast`, and readable local dates and IEC sizes. The updated x86-64 binary was rebuilt and its global search was run again in Wine; the screenshot shows that updated binary. The save and category fixes, CTF operations, export dialogs, pagination on a large catalog, cancellation, DPI, accessibility, and keyboard behavior still need runtime testing on an actual Windows system. A Wine smoke test does not replace Windows testing.

## macOS source review

The SwiftUI and AppKit source was manually reviewed against the JSON envelope, core request fields, optional metadata, C string ownership, background dispatch, and generation-based stale table-result filtering. A save-dialog overwrite handling issue and category backslash escaping issue were reported to the frontend owner and corrected. The macOS SDK was not available here: there was no native build or runtime test. The corrected source still requires a Mac build and smoke test.
