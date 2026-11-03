# UX18 — exact recorded byte-size search for possibly renamed files

Date: 2026-11-02. Draft PR #57, same native Windows Win32 / macOS SwiftUI clients.

## Why this exists

A movie can be renamed between two disconnected USB-disk catalogs. UX3 matches normalized **name + exact size**, while UX17 matches exact name regardless of size. Neither finds a renamed file whose only shared recorded attribute is its byte count. UX18 adds an explicit, conservative **size-only candidate search**. It can also match completely unrelated files: **neither matching size nor filename proves equal bytes, hashes, movie identity, USB presence, or safe deletion**.

## Operator path

1. Search all catalogued disks, optionally with the existing \`type:video\` extension preset, then select a **current loaded file row** with a known positive recorded size. No USB needs to be connected.
2. Windows **View → Find same recorded byte size (any filename)** or **Ctrl+Shift+S**; macOS inspector **Find same byte size (any filename)** or **Command-Shift-S**. This opens a global \`size:<exact integer bytes> AND kind:file\` search. Query syntax is visible and editable.
3. The central existing dense table shows different recorded names, disk labels, byte sizes and paths, paginated to at most 250 loaded rows. Use the left register to locate a *catalogued* disk; the live-OS pairing state remains independently checked. **Back** returns to the preceding search query, scope and page.
4. Do **not** use size-only hits as verified duplicates or delete candidates. Where needed, compare content manually after securely connecting and checking the identified physical media.

## Fail-closed and performance constraints

- Disabled for a stale/pending page, an unselected row, a directory, an absent/zero size or a size above \`i64::MAX\` (Windows; macOS uses signed recorded sizes). No unbounded prefetch, live file access, content hashing or mutating operation.
- Uses the existing \`size:\` SQL bound numeric query plus newly explicit \`kind:file\` predicate; no path, label or historical drive letter is used as USB identity.
- Linked CTF still uses the existing \`ctf_search_pinned\` page path and independently re-verifies original generation SHA **on each request**. Source CTF stays read-only; the SQLite/Parquet mirror is derivative, and pairing/sticker/annotation sidecars stay separate.
- Returned result pages are metadata evidence only. Queries may be costly on a cold 3-million-entry catalog; the real \`p95 ≤150ms\` target **is not established** by synthetic tests.

## Regression and manual QA

Automated: pure Win32 Rust size-query tests (positive integer, zero/missing and signed overflow); core SQLite search fixture across three distinct disconnected catalog volumes with renamed names, exact 1-byte difference, zero-size and a same-size directory excluded; bounded pagination, intentional volume filters and invalid kind rejection. Native source wiring regression includes both platforms, keyboard commands, page freshness, Back and precise uncertainty labels. Require exact-HEAD Linux/Windows/macOS ARM64+Intel native CI, offline acceptance, source regression, RC2→RC3 upgrade jobs. No production/user CTF or private paths are used in CI.

Manual: Windows 10/11 and macOS ARM64/Intel, screen reader and 880–1366 px/HiDPI, focus/Back and toolbar state, selected file on unplugged disk, mixed name/same-size results, CTF generation change between pages, live mount unplug/replug. Preserve unrelated on-disk data and annotations. This is **not** a release/signing/notarization or actual hardware acceptance.
