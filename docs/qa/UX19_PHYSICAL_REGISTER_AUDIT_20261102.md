# UX19 — native physical disk register audit (manual acceptance)

Date: 2026-11-02. Scope: draft PR #57, Windows 10/11 Win32 and macOS SwiftUI ARM64/Intel. **NOT a hardware certification or stable 1.0 approval.**

## Operator scenario

Start with an owner-approved, **read-only linked CTF** or separately copied SQLite catalog containing multiple catalog disks. Keep originals untouched and do not upload local paths, labels or CTF generation hashes to GitHub. In the left native register inspect the small six-count audit: connected / paired offline / identity conflict / unverified, plus missing/shared owner stickers. These counts refer to *catalog disk IDs*, not current media bytes.

- **Windows:** Physical disks → Show OS-verified connected disks; shortcut **Ctrl+Shift+O**. Show unverified disks with **Ctrl+Shift+U**. Menu actions re-probe current OS volumes first.
- **macOS:** two native accessible checkmark/questionmark icons in the disk-register header; **Command+Shift+O/U**. The previous positive badges clear immediately while the asynchronous recheck runs.
- **Either OS:** edit left register filter to \`state:connected\`, \`state:offline\`, \`state:conflict\`, \`state:unverified\`, \`tag:missing\`, or \`tag:shared\`. Bad/unknown reserved filter values show **no** rows. Clear via Windows Ctrl+L then Escape or macOS Command+L and clear button.
- Search a film across **all disks** before and after changing any register filter; selected file, search query, page, duplicates mode, native inspector and physical bindings must **not** change. Only a deliberate click in the register changes browse scope. Exact \`id:42\` and candidate \`copies:\` filters remain independent.
- Connect and explicitly pair one disposable test USB filesystem volume, refresh, inspect the connected filter. Unplug it, refresh, inspect paired-offline result. With an incomplete native scan, no row may be shown as connected or offline; all must be unverified. Use a separately controlled duplicate UUID situation to verify conflict; **do not format or alter real user disks**.
- On at least two catalog disks, add the same *private* sticker tag, verify \`tag:shared\` warns for both exact IDs, while \`tag:missing\` shows only disks with no owner tag. Sticker match must never be used as hardware identity.
- Test 880/1100/1366 widths and HiDPI; keyboard-only controls, Windows native listbox row mapping, macOS VoiceOver audit count and button labels, visible focus, dynamic plug/unplug refresh. Repeat with offline CTF source generation change: search must fail its existing source-SHA guard, not continue old results. Never claim file equality from a matching name/size.

## Automated evidence and boundaries

The synthetic Windows Rust test \`physical_audit_filters_keep_exact_ids_and_fail_closed_on_scan_failure\` covers four catalog IDs (4, 42, 142, 99), collisions, tag warnings, incomplete probes, audited counts and stable filtered row mapping. \`tools/qa/test_usb_disk_locator_contract.py\` checks actual Win32/SwiftUI source wiring, keyboard, and fail-closed core trust boundary. Required exact-HEAD CI: Linux Rustfmt/test/Clippy; Windows x64 installer; macOS ARM64/Intel installers; offline acceptance; integration wiring; installed RC2→RC3 upgrades. No user source data in CI.

Human live-USB and GUI acceptance, original modified CTF and provider-blocked historical CTF, Authenticode/Apple notarization and measured real p95 <=150 ms remain outstanding. PR #57 stays draft/unmerged; release stays unsigned RC3 prerelease, not stable.
