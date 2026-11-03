# UX41 — physical USB disk register sort, Windows and macOS
Date: 2026-11-03
Branch: `ux/search-first-disk-locator-20261101`, same draft PR #57

## Scope
Native Win32 and SwiftUI offer three switchable, **view-only** register orders:
**Catalog order**, **Owner sticker / shelf A–Z (untagged last)**, and
**Last independently verified OS state** (verified connected, paired offline,
identity conflict, unverified). Exact catalog disk ID breaks ties. Sort is
persisted only in user UI preferences: Win32 `ui-layout.json`, macOS
`UserDefaults`. It does not modify original read-only CTF, SQLite/Parquet
derivative caches, user annotations or explicit physical pairings.

Win32: View → Cycle disk register order; **Ctrl+Shift+T**.
SwiftUI: compact arrow-up/down icon in DISKS header, **Command+Shift+T**,
with VoiceOver current order, hint and tooltip.

Critically `copies:` candidate-only mode **retains existing OS-status /
metadata-count ranking** regardless of the ordinary register sort; other
filters (`id:`, `state:`, `tag:`, label/sticker) retain their membership,
but reorder by the selected mode. List row→original catalog index mapping
is preserved; selecting the currently scoped catalog disk must still address
the exact ID and never alter the global search or mounted volume pairing.

## Synthetic automated checks
- Compiled Windows Rust: duplicate Cyrillic shelf labels, missing tags last,
  deterministic ID tie, last OS state, complete absence of verified OS scan,
  default/corrupt/legacy preferences, stable ListBox index mapping.
- Python `tools/qa/test_usb_disk_locator_contract.py`: both native actions,
  keyboard shortcuts, persisted UI-only settings, copy-mode bypass, no new
  CTF/OS call on register sort, accessibility contract.
- The normal native Windows/macOS builds plus Linux, offline acceptance,
  integration regressions and installed RC2→RC3 upgrades must all succeed
  at the **same new HEAD**. CI source checks do not equal manual GUI QA.

## Manual native acceptance (not yet executed)
1. Open an offline catalog containing 40+ registered disks. At 1366×768 and
   880px, inspect clipping/tooltip/accessibility. Cycle sorting three times
   with keyboard, menu and VoiceOver/Windows screen reader. Check preference
   survives restart and a corrupt preference defaults harmlessly.
2. Assign Cyrillic owner shelf labels in the private sidecar to two different
   disks, leave another blank, select a disk by exact ID, and switch between
   sort modes. Check every displayed row's underlying catalog identity and
   scope, especially after filtering to a single ID and clearing filter.
3. Run a broad film search, select result, enter copies-on-other-disks mode
   and `copies:` register filter. Sort cycling must NOT move candidate
   disks out of their OS status/count priority; Back must restore context.
4. With only one disk plugged in, explicitly pair that catalog ID. Then
   unplug it, interrupt a mount scan, connect a cloned UUID and allow TTL
   to expire. OS sorting may use **connected** only after a fresh complete
   independently verified native probe; failed/incomplete/colliding probes
   must not imply positive media presence.
5. Confirm neither sorting nor stale scope writes the CTF, reuses a
   historical drive letter, claims current file presence or byte equality,
   offers automatic duplicate deletion, or bypasses generation SHA checks.

**Release remains blocked:** owner-modified original CTF, both historical
CTF raw-byte access, human Win10/11 + macOS ARM/Intel GUI/USB QA, signing
and notarization, and measured real ~3M-record search p95 ≤150 ms.
Prior genuine 44-disk/3,046,027-record evidence remains historical evidence,
not these synthetic tests. No stable 1.0, no merge without owner approval.
