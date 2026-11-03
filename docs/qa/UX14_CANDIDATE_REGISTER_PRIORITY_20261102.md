# UX14 — exact-ID ranked candidate disk register: acceptance (2026-11-02)

Scope: draft PR #57; synthetic CI fixtures only. This feature is a **metadata
pickup shortlist**, not a filesystem viewer or content-hash verifier.

## Automated contract

- Rust unit test candidate_register_prioritizes_os_verified_then_counts_without_fuzzy_ids:
  unsorted source volume IDs 4,42,142; verified disk outranks offline even at
  lower count; unverified/conflict handling; original ListBox index mapping
  unchanged; duplicates/invalid/unknown groups cannot create disk rows.
- Rust unit test exact_id_count_and_unverified_state_precede_historical_label:
  leading catalog ID and total name+size count, Cyrillic sticker, shared
  warning, last OS state and sanitization of unsafe catalog linebreaks.
- Python source-wiring test test_ux14_candidate_register_is_prioritized_retrieval_not_content_evidence
  verifies both real native clients consume metadata groups, render exact ID,
  make SwiftUI register match the pickup ordering, and expose accessible labels.
- GitHub: confirm the **exact PR-head** SHA on Linux Rust tests/fmt/Clippy,
  Windows native installer, macOS ARM64 and Intel native installers, offline
  acceptance, native integration regressions and installed RC2→RC3 upgrades.

## Manual 1366×768/HiDPI, Win10/11, Mac ARM64/Intel

1. Search a file across all catalogued disks. Choose Find copies; then show
   Candidates in the left disk register (Ctrl+Shift+K / Command+Shift+K).
   Inspect three nonadjacent IDs with duplicate catalog labels. Each row must
   lead with exact #ID and **complete per-disk name+size candidate count**,
   not a count from the currently loaded 250-row page.
2. Give two disks the same private shelf sticker. Verify SHARED STICKER is
   announced by screen reader and clearly visible; do not silently identify a
   physical disk from the sticker or historical drive letter.
3. Explicitly pair one disk to a unique live OS UUID with the right device
   physically connected; rescan with Ctrl+R / Command+R. Verify this disk
   precedes offline/unverified/conflicting rows. Unplug and rescan: no false
   connected badge; simulate incomplete probe and duplicate UUID to ensure
   unverified/conflict, never misleading offline or connected.
4. Navigate with keyboard and screen reader through the filtered list, then
   click a listed disk. Check that selection maps to its **actual catalog ID**,
   not sorted row index, and that no automatic OS mount/file-open occurs.
5. Return to global results and clear the register filter. The search query,
   selected file, result page and unmodified CTF source must remain intact.
   Confirm the left pane stays usable at 880/1100/1180/1366px and scaled DPI.

No confidence from CI installer smoke alone: retain human screenshots,
keyboard/VoiceOver/Narrator evidence and real USB reconnect logs privately.
No source/private labels/path files or hashes in public CI artifacts. The
source CTF remains immutable; matching name and recorded size **does not**
verify bytes, movie identity or authorize deletion. RC3 remains unsigned;
stable 1.0 blocked by original modified CTF, historical inputs, GUI,
signing/notarization and real 150ms p95.
