# UX34 — one fail-closed current-OS status across the native disk locator
Date: 2026-11-02 · PR #57 · synthetic test inputs only.

## Defect and fix

UX33 made the **search result Disk cell** fail closed when the physical-status
service returned `connected` without a non-empty verified ID. Other
surfaces were still reading the raw service state: Windows disk register,
copy-disk chooser, contextual locator, clipboard pickup list, OS-state register
filters and sole-connected-disk shortcut; macOS candidate ranking, register,
OS-state filters and pickup text were based on `physicalState` which did not
check a non-empty UUID or its unique presence in the OS scan. They could
disagree with the safe results table on the **same snapshot**.

Windows now derives a single audited state map from the last complete fresh
OS probe and verified pairing IDs, used by all those physical retrieval
surfaces. Disk card applies the same predicate. Refresh invalidates the
physical UI when verified IDs change even if raw status strings do not.
macOS guards `physicalState` itself with current probe TTL, non-empty verified
UUID and exactly one UUID observation. Its results, register, copy chooser,
shortlist ranking, keyboard focus and accessibility descriptions now agree.

A filesystem UUID is not a manufacturer USB serial and cloning still requires
real device inspection. OS attached is not proof that a catalogued movie or
hash exists, and name+recorded-size candidates are never deletion evidence.

## Synthetic regression gates

- Windows pure Rust: no identity, stale/incomplete scan, valid explicit
  pairing; candidate picker, pickup list, filter and sole-connected focus.
- Cross-native Python source-contract: one audited state path used by register,
  native picker, clipboard, current disk locator and both macOS+Windows clients.
- Exact SHA: Linux, Windows x64, Mac ARM64 and Intel native builds; offline
  acceptance, integration and installed RC2→RC3 upgrades. **Not yet verified
  for this change** until the new workflows complete.

## Human hardware and GUI acceptance (still required)

1. On Win10/11 and macOS ARM/Intel, load a synthetic multi-disk catalog with
   one explicitly paired test volume and one unpaired catalog disk. Compare
   OS badges in **search row, register, picker, inspector and copied checklist**.
2. Unplug or suspend; all surfaces must go unverified on invalidation/TTL.
   Reprobe must be complete to restore a positive status.
3. On macOS mount two test filesystems exposing the same filesystem UUID;
   positive state must be withheld everywhere, including the sole-connected
   shortcut. Check Win32 interrupted enumerations and empty-ID responses.
4. Verify search/selection, Ctrl/Command+D and G shortcuts, keyboard focus,
   narrow 880px and 1366x768 windows, VoiceOver/Narrator/HiDPI. No network,
   current path or file hash inferred; no user CTF/SQLite uploaded.

Stay in draft PR #57. RC3 remains unsigned prerelease. No stable 1.0, no merge
without real hardware/GUI, historical CTF, signing/notarization and p95 gates.
