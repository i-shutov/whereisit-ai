# UX42 — compact 40-disk register with adjustable native panes
Date: 2026-11-03
Branch: `ux/search-first-disk-locator-20261101`, draft PR #57 (no merge)

## Integrated functionality
- **Win32:** native draggable separator between the physical disk register and result table; 200–400px requested width, clamped by window size to leave at least a 600px table at the supported 880px minimum. The third disk locator automatically shrinks/collapses if it cannot fit; original full recorded path remains in the table. View menu and Ctrl+Alt+Left/Right adjust width by 40px without mouse. Only private versioned `ui-layout.json` stores the requested width, not a CTF, source path, query, pairing, or result identity. Drag/resize does **not** dispatch searches or change catalog disk IDs.
- **macOS SwiftUI:** native HSplitView supports resizing the disk register; width range widened. New **compact rows (default)** show ID, label, sticker/OS status and duplicate-candidate count in one scannable line, with warning for duplicate shelf stickers. Command-Shift-9 toggles detailed two-line rows. The full accessible label and tooltips remain, including status uncertainty; the view preference lives only in UserDefaults. Selected search and disk scope do not change.
- Neither rendering mode asserts media presence without a complete fresh native UUID check; no file hashes, deletion, CTF source write or historical letter inference.

## Reproducible synthetic validation
Compiled pure Windows Rust: width clamping, 880px minimum 600px results, locator fit, splitter hit zones, corrupt/old preference fallback. Dual-native Python source contract covers event wiring, native keyboard alternatives, persisted view-only state, VoiceOver full labels and candidate mode. CI: Linux Rust/QA, Windows Win32 installer and macOS Intel/ARM64, offline kit, integration regression and installed RC2→RC3 upgrades at the same exact PR HEAD. Do not equate CI PASS with hardware/visual acceptance.

## Required manual owner GUI/USB review (not executed)
1. Windows 10 and 11, 880/1120/1366px & HiDPI: drag the left separator, use Ctrl+Alt+arrows, relaunch, and resize window narrower/wider. At 880px the table remains >=600px and long names can be inspected via the disk locator/tooltip. Check keyboard focus remains on the current result while resizing.
2. Mac ARM64 and Intel: on an offline 40+ disk catalog, inspect compact/detailed modes, VoiceOver and Command-Shift-9, HSplitView drag and relaunch. Inspect long Cyrillic labels, owner sticker collision warnings, offline/unverified badges, selected-copy focused filters and retained selection.
3. Search for a film across all disks, navigate into copy candidates and Back. Widen/compact repeatedly; neither copy candidate rankings nor exact catalog ID, current selection, SHA pin or explicit disk scope should change.
4. With real USB devices: connect, unplug, clone UUID, interrupt mount enumeration. Only independent complete OS scans + explicit unique pairing may mark connected; resize/mode toggles do not synthesize state.

**Release remains blocked:** provider-blocked historical two raw CTFs (already provided), modified original-app CTF, human Win10/11/Mac ARM/Intel GUI+USB acceptance, signatures/notarization, real p95 <=150 ms. Genuine prior 44 disk/3,046,027 entry acceptance remains real historical evidence, not these synthetic CI inputs. RC3 unsigned prerelease only. No stable, no merge without owner approval.
