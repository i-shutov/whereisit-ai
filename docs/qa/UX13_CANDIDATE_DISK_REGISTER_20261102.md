# UX13 — Candidate-only physical disk register (2026-11-02)

Scope: draft PR #57; Win10/11 native Win32 and macOS ARM64/Intel SwiftUI. **Not** an assertion of human or physical acceptance.

## Synthetic and build gates
1. Run `cargo test --workspace --locked`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`; verify Rust `candidate_only_register_uses_exact_ids_and_preserves_original_row_mapping`.
2. Run `python3 -m unittest discover -s tools/qa -p 'test_usb_disk_locator_contract.py' -v`, offline acceptance and integration CI at exact commit SHA.
3. Confirm native Windows installer and macOS ARM64/Intel app/package/smoke complete; no CI build is a manual GUI test.

## Human keyboard / USB walkthrough
1. At ~1366×768, global-search for one catalogued file with metadata-name+size candidates on at least two different disk IDs; select Find copies. Do not attach any physical disk.
2. Windows: View → **Show candidate disks in register** or Ctrl+Shift+K. Mac: the accessible **Candidates** control or Command-Shift-K. Verify the left register shows only exact IDs of candidate disks. A stored label containing #42 must NOT pull in disk #4 or #142 unless those IDs are also actual candidates.
3. Verify the central current candidate page, selected file, candidate totals and underlying search scope remain unchanged. Check keyboard/VoiceOver/Windows screen reader focus, pane clipping, dark/light contrast and tooltips. Ctrl+L / Command+L edits the `copies:` register filter; clearing it shows all catalogued disks.
4. Choose a displayed candidate disk intentionally: only then may browsing scope change. Back from copy mode, or new global search, must clear the temporary `copies:` filter.
5. Attach/replug a personally labeled USB device, refresh OS presence, and verify only an independently paired stable OS volume identity can show connected; metadata candidate membership or sticker alone never does. Conflicts remain conflicts; no current file contents or hashes are inferred.

## Safety and performance
Source CTF read-only, pinned SHA checked for each linked page. No user CTF/SQLite/raw paths/SHA are committed. Synthetic CI fixtures only. Real 44 disk / 3,046,027 entry acceptance remains historical evidence and is NOT rerun in this block. Never recommend auto-deletion from name+size. Human hardware and actual p95 150ms checks remain release blockers.
