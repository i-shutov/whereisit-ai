# SQLite WAL durability upgrade — internal acceptance record

**Status: Linux preparation and tests passed; native Windows/macOS release matrix is a separate gate.** This work is not a signed release, does not claim original WhereIsIt compatibility, and does not read any user CTF/database.

## Why this is a release blocker

The frozen 1.0.0-rc.1 embeds SQLite 3.46.0. SQLite's documented WAL-reset race in simultaneous write/checkpoint connections is fixed in upstream SQLite 3.51.3 and later: https://www.sqlite.org/wal.html#the_wal_reset_bug . No observed corruption of a user catalog is asserted.

This branch pins `rusqlite = 0.39.0`, `libsqlite3-sys = 0.37.0`, with **bundled SQLite 3.51.3**. The isolated preparer reads the actual published sqlite3.c amalgamation, verifies its version and public-domain notice, then regenerates the root Cargo.lock and the complete registry license table. Root Cargo.lock is authoritative for the workspace; the stale nested atlas-core/Cargo.lock was removed.

## Compatibility changes

- All Rust crates use a single workspace rusqlite dependency.
- SQL INTEGER is signed 64-bit; public unsigned file sizes/counters are now converted through checked i64→u64 and u64→i64 paths. Negative values and overflow are errors, not silently wrapped.
- The rusqlite 0.39 attached-schema pragma API uses the validated `Some(schema)` name. CTF export rejects impossible negative sizes before writing.
- No automatic database migration or replacement of an existing catalog is added.

## Executed tests

- Workspace `cargo test --locked --workspace`, rustfmt and strict Clippy.
- `cargo test --locked -p atlas-core --test wal_durability -- --nocapture`:
  - actual embedded SQLite version gate, at least 3.51.3;
  - two simultaneous independent writers with committed and rolled-back records;
  - independent WAL checkpoint and read snapshot;
  - correct exact row count after reopen; SQLite integrity and FK checks;
  - force-killed test subprocess holding an uncommitted transaction; after reopening, committed data must remain and the uncommitted row must be absent;
  - intentionally negative catalog aggregates must not wrap to positive values.
- `cargo test --locked -p atlas-service --lib` covers existing CTF and service tests.

**Preparation run:** https://github.com/i-shutov/whereisit-ai/actions/runs/40861614167 .
**Prepared source commit:** 05c926cc55f6c817c575834aa2c98155925dc00a .
**Cargo.lock SHA-256:** ec0a2328b5687687e5e323a39444cc0898fa2288b2949336d54207f9967d9b97 .

## What these tests do not prove

They do not simulate a system power failure, physical media damage, or uncooperative filesystem caching, and do not constitute a stress proof that every possible WAL interleaving is safe. The upstream patched SQLite version is the principal correctness fix. They do not measure throughput on the private three-million-entry catalog; that requires a separate read-only performance/CTF acceptance checkpoint. The frozen RC.1 binary is unchanged.

## Remaining merge gates

- [x] Bundled fixed SQLite verified from its own source.
- [x] Root lockfile and public licenses regenerated reproducibly.
- [x] Linux workspace, tests, crash/rollback, Clippy pass.
- [ ] Windows native binary/install/uninstall smoke on the exact PR head.
- [ ] macOS Apple Silicon and Intel binary/PKG/install/uninstall smoke on the exact PR head.
- [ ] Review final PR diff and compare runtime version from platform binaries.
- [ ] Update release acceptance only after evidence is complete.
