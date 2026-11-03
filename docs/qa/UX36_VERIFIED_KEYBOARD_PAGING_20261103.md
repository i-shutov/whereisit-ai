# UX36 — verified result-page navigation and focus (draft PR #57)

Date: 2026-11-03. Native Win32 and SwiftUI. This is a **manual QA
runbook**, not a claim that manual hardware acceptance already happened.

## Operator workflow

Search globally for a common filename on a synthetic multi-disk catalog,
yielding over 500 results. Open the disk locator. Next/Previous paginates
only the **250 entries of the current service page**, retaining the applied
global query or selected copy-disk filter. Windows: Ctrl+Alt+PageDown /
Ctrl+Alt+PageUp. Mac: Command+Option+Right / Left Arrow. Both also offer
native Previous/Next buttons, disabled when unavailable. The Mac pager
announces the currently verified page; pending, failed and stopped reads
are explicitly not verified. On a successful page transition, the first
fresh result receives native selection and keyboard focus; the disk
inspector follows that exact row and its catalog disk ID.

## Synthetic regression

1. Run native four-target build/installer smoke, `cargo fmt --all -- --check`,
   `cargo test --workspace` and
   `python -m unittest tools.qa.test_usb_disk_locator_contract` from the
   **exact draft PR head SHA**. Native Windows Rust unit tests check page
   boundaries, failed/pending pages, and arithmetic overflow. The Python
   contract checks both native event paths and source pinning.
2. Use 0, 1, 250, 251 and 501 synthetic rows. First page cannot go backward;
   the last page cannot go forward. In-flight and failed responses must
   disable both actions; Stop must remove the rows and prohibit navigation.
   Retry rechecks the source. A missing linked CTF generation must not
   fall back to local SQLite.
3. Navigate page 1→2→3→2 using keyboard, then inspect file metadata, disk ID,
   shelf conflict and fresh OS badge. No row from the previous page is
   permitted to remain selected while the new request is pending. Selection
   of row 1 happens **only after** the matching generation succeeds; empty
   or ambiguous pages do not invent selection. Try another global Search
   before the slow page reply: the old response must be ignored.
4. In a copied-metadata-candidates scope, focus a disk then page with the
   same shortcuts. Verify **other catalog disks only**, count semantics,
   the original Back context and no file hash/content equivalence claim.
5. Linked CTF: privately change the generation between pages and confirm
   a failed/stale page has no selected result or fresh navigation button.
   Never upload CTF or resulting local cache, filenames, paths or hashes.
6. On actual Win10/11 and Mac ARM64/Intel, manually inspect keyboard focus,
   VoiceOver/Windows screen reader, narrow 880/1366px windows and HiDPI,
   plus USB unplug/clone handling. No claimed GUI acceptance before this
   real check.

## Safety and release

The reader remains read-only for linked CTF; every service page is
generation-SHA-verified. OS pairing stays separate, is based on a fresh
complete OS UUID scan, and cannot establish file byte presence/equality.
No deletion suggestion based on metadata candidates. All CI fixtures must
be synthetic. Genuine 44-disk/3,046,027-entry earlier acceptance remains
separate evidence. No stable 1.0 or PR merge without outstanding gates.
