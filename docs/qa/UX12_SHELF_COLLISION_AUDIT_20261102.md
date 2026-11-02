# UX12 — shared physical sticker warning
2026-11-02 · draft PR #57 · i-shutov/whereisit-ai

A repeated operator-entered sticker can make retrieval of one of ~40 offline
disks ambiguous. The private physical_shelf_labels API returns ID-only
collision groups of current registered catalog disk numbers, scoped to the
exact catalog-generation and platform. Unicode case and repeated whitespace
normalize for this warning; duplicates are permitted. No CTF writes, no
changes to OS UUID pairing, and no assumptions about current file bytes.

## Automated synthetic checks

Run cargo fmt --all --check, cargo test --workspace and Clippy, then
python -m unittest tools.qa.test_usb_disk_locator_contract. Rust service
tests cover Cyrillic case/whitespace, multiple groups, partial ID subset,
generation/platform isolation, explicit clearing and unverified OS status.
CI must show final PR-HEAD native Linux/Windows/macOS ARM64/Intel, offline
acceptance, integration and installed RC2-to-RC3 upgrades PASS before
reporting success. No owner CTF/SQLite, paths, hashes or stickers in logs.

## Manual GUI and hardware checks (not established by CI)

1. On synthetic disks #1–4 save Стеллаж Б / стеллаж  б and USB-04 / usb-04.
   Expect separate SHARED STICKER warnings on each conflicting disk with exact
   other numeric IDs in Win32 and SwiftUI register, selected locator and
   candidate picker. Copy pickup list: the warning must also be included.
2. Change or clear a sticker. On refresh conflicts must disappear. Switch
   catalog generation and ensure no stale private tags or warnings leak.
   Global search and candidate-scoped pages remain unchanged.
3. At 880/1100/1180/1366 px and HiDPI review keyboard-only navigation and
   Windows Narrator / macOS VoiceOver. Confirm warning text, not color alone.
4. Plug and unplug actual USB devices including duplicated filesystem UUIDs.
   Human tags must never mark a volume connected; OS identity conflicts must
   still fail closed. Metadata name+size does not verify content or justify
   deletion, even on a connected disk.

Prior real owner CTF acceptance (44 disks, 3,046,027 entries) is genuine;
this CI uses synthetic data. Two historical raw inputs are provider-blocked,
not absent. Original modified CTF, manual Win10/11/Mac ARM/Intel UX, authentic
signing/notarization and measured p95 <= 150ms are still release blockers.
RC3 unsigned prerelease; stable 1.0 blocked. Keep #57 draft and unmerged.
