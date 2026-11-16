# UX39 — compact native catalog commands, search-first (2026-11-03)

The old admin-heavy top row hid the operator's main job: find a file across
~40 *disconnected* catalogued USB disks, identify the exact labeled disk, and
inspect catalogued metadata candidates on other carriers.

## Live integrated implementation

Windows Win32 now has one native **Catalog tools…** popup containing Open,
New, Add disk, Link read-only CTF, Import CTF into SQLite, integrity check,
compact copy, linked annotation review and Cancel scan. An **Export** submenu
offers CSV/Parquet/CTF/JSON and calls the *existing* guarded EXPORT command
after selecting its exact original format index; linked CTF export cannot
choose CTF. The legacy hidden controls are not removed from service wiring.
The first row displays a compact offline-search identity rather than seven
admin buttons. macOS SwiftUI uses native **Catalog** and **Export** menus
with the same existing model actions and safety predicates.

The global Search / This disk / Duplicates / Back, dense table, volume register,
physical disk locator and keyboard actions remain native. Neither menu
reads actual media files, assumes a historical letter is mounted, changes CTF
source bytes, or bypasses generation SHA verification.

## Synthetic CI and required manual GUI acceptance

1. Compile Win32 x64 and SwiftUI macOS ARM64 + Intel. Run Windows
   `compact_catalog_toolbar_tests`, all native Rust tests and
   `python3 -m unittest tools.qa.test_usb_disk_locator_contract`.
   Check native Linux, Windows, Mac ARM/Intel, offline kit, integration and
   installed RC2→RC3 workflows **for this exact HEAD**, not an earlier one.
2. On Windows 10/11 at 880, 1180 and 1366 logical px, 100%/150% scaling,
   verify no hidden buttons overlap search and the popup opens from keyboard,
   clicks and ESC. Test each Export submenu item and resulting dialog
   extension. Test check, compact, Open/New, synthetic Add disk, linked
   CTF read-only mode, annotation review and cancel during a scan.
3. On macOS Intel/ARM64, repeat the same actions with native menus,
   Return/Command-F search, result selection, VoiceOver and keyboard.
   CTF export must be disabled in linked mode. No user data should be
   exported during test; use a synthetic scratch catalog and compare source
   generation before/after.
4. Check physical pairing still fails closed on a clone/ambiguous UUID,
   unplug and resume. No menu choice may claim current file contents.
   Verify no change to search page or selection when opening/dismissing menus.

**Not accepted by CI:** human window/keyboard/VoiceOver tests, genuine
modified original CTF, blocked historical CTF bytes, physical USB checks,
measured real 3M-row p95 ≤150 ms, signing/notarization. Draft PR #57 remains
unmerged; unsigned RC3 prerelease is not stable 1.0.
