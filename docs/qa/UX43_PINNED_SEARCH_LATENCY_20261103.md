# UX43 — pinned-search latency and fresh original SHA (2026-11-03)

## Integrated, native functionality
- **Core Rust:** pinned linked-CTF reads obtain the expected original SHA from a strictly synced in-process pin, validate source size and cache identity, then perform **one full original CTF SHA-256 after the query, before returning anything**. This removes one redundant full-file hash; it does **not** trust source mtimes or skip the required SHA recheck on any reuse. Missing pin, changed source (even same-size/restored mtime), changed SQLite cache or failed I/O fail closed. The immutable mirror still gets its periodic full hash and every page remains bounded.
- **Windows 10/11 Win32:** a successful exact-generation page shows its *local operation duration* in the native status bar. It is cleared on navigation; rejected, cancelled and stale replies cannot set the timing. No query, path, CTF or SHA is logged or persisted.
- **macOS ARM64/Intel SwiftUI:** the same local request-to-accepted-page duration appears after native service/FFI verification. A generation token prevents an older reply from supplying a sample, and Stop clears the measurement. The native accessible status is unchanged except for that timing text.
- **Safety:** a single duration is **not p95** and cannot establish the real hardware SLO. The UI explicitly says `not p95`. Search, disk IDs, the live OS volume identity gates, metadata duplicates, personal annotations and native clipboard rules are otherwise unchanged.

## Synthetic and integration CI
- Existing `ctf_mirror_contract` same-byte-length/restored-mtime mutation tests must continue to reject the original CTF after a rewrite, with source and cache bytes unchanged by queries. Other pinned search, export and duplicate contracts remain required.
- Compiled Windows latency formatting test verifies pending/failed pages cannot claim a result. Dual-native Python source-wiring regression checks generation guards and that the pinned source SHA remains computed **exactly once** on every successful read.
- Run all exact-HEAD native Linux, Windows x64, macOS ARM64 + Intel, integrated offline kit, cross-native regression and installed RC2→RC3 upgrades. A build is not GUI or USB acceptance.

## Owner-only real validation (do not publish raw evidence)
1. On real Windows 10/11 and macOS ARM64/Intel, open the owner-selected CTF read-only; verify generation pin and external source permissions.
2. Search films across **all catalog disks** and browse several pages; observe the *local page ms* label (a request-to-page measurement, not isolated SQL, and not accepted p95). Repeat >=30 warmed, representative searches including empty, broad, name and video-expression queries. Record only sanitized aggregates, hardware/runtime, sample count and p50/p95 calculation in a private log; never raw CTF paths/queries/names/hashes.
3. Modify a private **copy** of a synthetic CTF at constant byte length and restore its mtime. Every subsequent pinned page must refuse the stale generation, even if the cached SQLite catalog is unchanged. Do not edit an owner original.
4. Select a file, find same-name+size metadata candidates on other drives and compare USB plug/unplug/UUID conflict states by actual OS. A fast page must not display connected unless the original physical identity check passes. Exercise keyboard, VoiceOver/accessibility and 880/1366px/HiDPI.
5. Measure hardware **p95 <=150 ms** separately for a representative full workflow; do **not** mark it PASS based on a single page timing or synthetic CI.

## Release boundary
Draft PR #57 stays unmerged; RC3 stays unsigned prerelease. Real hardware p95, original modified CTF, two provider-blocked historical CTF bytes, physical USB/GUI, signing and notarization still block stable 1.0. No external media content or hashes are inspected.
