# UX24 — independent physical USB disk pickup card (2026-11-02)

Scope: draft PR #57, native Windows Win32 and macOS SwiftUI. These are
manual acceptance instructions, **not** proof that owner hardware was tested.

## Operator path

1. Open a catalog with two distinct disk IDs but the same recorded label.
   In the left register choose one disk without selecting any file result.
   Windows: **View → Copy selected PHYSICAL disk pickup card** or
   **Ctrl+Shift+L**. macOS: **Disk card** on the always-visible toolbar or
   **Command-Shift-L**; the inspector also has **Copy disk pickup card**.
2. Paste the note into a local text editor. Confirm that the first key is the
   **exact catalog disk ID**, not the possibly duplicated label. Confirm the
   user-entered sticker/shelf tag (or explicit *not assigned*), recorded entry
   count and a **time-limited** OS physical state appear. If a sticker is shared,
   an explicit warning appears. No historical path is presented as a live path.
3. Search for a file across all registered disks, select it, and repeat.
   Confirm the card now refers to **that selected result's catalog disk ID**.
   Clear the file selection and choose another disk in the register; repeat.
   No other disk's sticker or OS badge may leak across selections.
4. For a linked **read-only** CTF, repeat the card action. Confirm an asynchronous
   pinned source-generation verification precedes clipboard changes. In a
   synthetic/private QA copy only, change the backing CTF generation and
   verify **no stale card is copied**. Never modify an owner original.
   Switch CTF generation or switch SQLite catalog, then test the card while
   the volume list is loading or absent. A disk ID not in the **current**
   register must result in **no clipboard write**, never a stale card.
5. Pair an actual external disk **explicitly** and verify its UUID through
   the operating system. Recheck and copy: “connected” is allowed **only**
   after an independent complete OS scan and service pairing. Sleep, unplug,
   replug with changed letters/mounts, and let the 12-second monotonic TTL
   expire; absent or stale checks must show *unverified*, not “connected”.
6. With two UUID-colliding mounted clones and with a failed/incomplete OS scan,
   repeat. The card must say conflict or unverified, never assert a unique
   physical disk. The tag and recorded label are manual retrieval hints, not
   manufacturer serial numbers.
7. Test Windows 10/11 (100%/150% scaling at 880, 1180, 1366 widths) and
   macOS ARM64/Intel (VoiceOver, focus, shortcut and collapsed inspector).
   Mac toolbar action must work with the inspector hidden.

## Invariants and CI

The card is copied to the **local clipboard only**. It neither opens current
USB files nor writes linked original CTF, SQLite derivative caches, personal
annotations or OS pairing. It never claims content hashes, equivalence, safe
deletion or actual catalogued file presence. Each linked CTF disk card independently verifies the original generation SHA
before copying (with an async selection/source guard); an already loaded
volume list alone is insufficient. SQLite cards use their currently loaded
catalog volume. No source writes or private raw data are required. Tests: compiled Windows Rust helper
for Cyrillic/duplicate labels/ID collision/failed OS proof; dual-native Python
source contract; exact-HEAD Linux fmt+Clippy+unit, Windows build, macOS
ARM64/Intel builds, offline kit, RC2→RC3 installed upgrades.

Do not publish private clipboard text, CTF sources, SQLite, paths or hashes.
Earlier 44-volume/3,046,027-entry real CTF acceptance remains independently
documented, not a fresh test of UX24. Stable 1.0 remains blocked by human
hardware/visual QA, genuine modified and provider-blocked historic CTF, real
p95 ≤150 ms, Authenticode and Apple notarization. PR #57 stays draft.
