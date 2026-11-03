# UX40 — Disk-register-first category disclosure (2026-11-03)

## Why this is one integrated UX vertical

The owner's main operation is to search a file across ~40 intermittently attached USB disks, then identify the **physical labeled carrier**. Previously both native clients permanently reserved much of the left sidebar for secondary category administration. At 740px window height, Windows split the disk-register sidebar in half, forcing excessive scrolling through the media collection. The fix prioritizes disk retrieval without removing category support.

## Implemented on native Windows and macOS

- **Windows 10/11 Win32:** category list, new-category editor, filter and assign actions are **collapsed by default**; the disk register takes the available height. **View → Show / hide category tools**, **Ctrl+Shift+A** reveal or hide all category controls together. If the disappearing panel held keyboard focus it transfers to the still-visible disk filter. Otherwise the selected file and keyboard focus remain untouched. Visibility is saved in the existing versioned **UI-only** `ui-layout.json`, never in the catalog, CTF or user annotation database. Old/corrupt preferences safely default to collapsed.
- **macOS SwiftUI (ARM64/Intel):** a compact accessible **Show/Hide category tools** disclosure appears below the disk register. **Command+Shift+A** toggles it, and `@AppStorage` persists the preference. The existing editable category list and operations are preserved when expanded.
- Neither implementation changes the all-disk search expression, search page generation, selected catalog disk, CTF verification, OS UUID probe/pairing or metadata-copy mode. More visible disks **does not mean** newly verified physical disks or files. Previous category assignments are unchanged.

## Synthetic automated acceptance

- Rust pure geometry regression at the 740px target height: sidebar height 594px gives disk-register allocation 558px collapsed vs 297px expanded; height minimums hold for small windows.
- Rust preference parsing regression: missing, malformed, older-version and explicit true/false states; default collapsed. Original locator visibility/width tests retained.
- Cross-platform `tools/qa/test_usb_disk_locator_contract.py`: both native controls, persistent settings, keyboard routing, 3 hidden Win32 category actions, disclosure focus safety, and no search/probe side effects.
- Exact-head workflow checks: Linux, Windows x64, macOS ARM64/Intel, offline acceptance, integration regressions, and installed RC2→RC3 upgrade jobs. Record actual run IDs only after completion. These tests cannot prove real-device UI behavior.

## Required real GUI/USB manual acceptance (not yet executed)

1. Windows 10 and 11, 100%/150% scale, widths 880/1180/1366, heights 550/740: verify full native disk register shows many entries without collision/overlap; category controls initially hidden. Toggle from View and Ctrl+Shift+A; verify category create/filter/assign still work using a **synthetic scratch catalog**, and closing/reopening preserves visibility.
2. While results are selected, toggle categories and verify selection, keyboard focus and search scope do not reset. If the focus is in the category editor when collapsing, it moves to the disk filter. Validate screen-reader exposure and hidden controls are absent from Tab order.
3. macOS ARM64 and Intel: same scenario, native keyboard shortcut and VoiceOver disclosure; verify persisted state and List scroll. Check narrow window with locator collapsed.
4. Unplug and replug a real external disk, including a colliding or unpaired filesystem UUID. `OS verified` appears **only after a fresh complete OS check plus unique explicit catalog pairing**. Category display preferences cannot change this truth.
5. Linked CTF must remain read-only, and the original generation SHA independently verified for each fresh page. Category toolbar actions must not bypass linked-mode restrictions.

**Still blocking stable 1.0:** original-app modified CTF roundtrip, two provider-blocked historical CTF byte streams, human native Win10/11/macOS Intel/ARM USB and accessibility acceptance, signing/notarization, real source-search p95 ≤150ms. Only unsigned RC3 is released. Draft PR #57 must not be merged before review.
