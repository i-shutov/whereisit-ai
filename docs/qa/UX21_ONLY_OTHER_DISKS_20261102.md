# UX21 — other disk means another catalog volume

**Context:** draft PR #57, native Windows Win32 and macOS SwiftUI. CI uses generated synthetic catalogues only.

1. Create three synthetic catalog volumes A, B and C. On A put two files named Film.MKV of equal recorded size; on B one same-name+size file; on C one same-name but differently sized file. All drives may be disconnected.
2. From a selected A file invoke **Find copies on other disks** (Ctrl+D / Command+D). Results, disk chooser and temporary `copies:` register must contain only **B**; neither of the two A files or the C different-size file may appear. Verify exact catalog volume ID and independent last OS-probed state; never infer a current mount from a historic letter.
3. Read the generated-only service response: `total=1`, `other_total=1`, `all_total=3`, `distinct_disks=1`, `source_volume_id=A`. At limit 1 or 250, SQL paging must not omit or reintroduce source-disk records. Explicitly focusing A with the flag returns an empty result, not source files.
4. Try an A file with no B match: a clean **zero** external-candidate result is expected; do not claim missing bytes. Back must restore the previous search. Unflagged service requests remain compatible with older all-volumes behavior; non-boolean `other_disks_only` must fail.
5. Use a synthetic linked read-only CTF and verify each other-media request rechecks the source generation SHA. Modify only the *synthetic test source* before Retry: the next request must fail `source_changed`, with no stale page restored. Never upload actual user CTF or derivative SQLite caches.
6. Test 880px and 1366×768, Windows 10/11 at normal/high DPI and macOS Intel/ARM. Verify keyboard/VoiceOver and Windows accessibility, disk identity collisions, OS probe failure, candidate focus and pagination.

**Boundary:** name+recorded-size agreement is **not** byte identity, a hash, actual file presence, a safe auto-delete target, or proof of connected USB hardware. RC3 remains unsigned. Stable 1.0 stays blocked by original modified/historical CTF validation, manual real hardware and GUI tests, signing/notarization and real-catalog p95 ≤150 ms.
