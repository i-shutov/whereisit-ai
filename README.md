# WhereIsIt AI

Native offline disk catalog for **Windows 10/11 and macOS 13+**: Rust core, SQLite/FTS5, Win32/SwiftUI, WhereIsIt CTF import/export, Boolean search, categories, metadata duplicate candidates, CSV/Parquet.

## Status (2026-10-28)

- Atlas Catalog v0.1 source is packaged locally; full source **has not yet been pushed into this repository**. This repository starts as a coordination and import checkpoint.
- Real USB-drives.CTF: 44 volumes, 3,046,027 entries. Unchanged CTF → SQLite → CTF SHA-256 roundtrip verified.
- Windows native GUI compiled and smoked under Wine; **real Windows QA and Mac builds are pending**.
- Original WhereIsIt acceptance of *new/modified* CTF is pending; this must not be confused with unchanged-file roundtrip.
- Windows Inno Setup and macOS pkg/dmg build scripts have been prepared in the new local finalization snapshot, but no signed installation builds have passed CI yet.

## Storage approach

Primary SQLite (transactional tree, indexes, FTS5); evaluate DuckDB/Parquet for bulk analytics and duplicate groups on the same 3M-record workload. Polars is a comparison/export tool, not a durable catalog DB. Do **not** load 3M rows into RAM at startup.

## Workflow

Read [ROADMAP_TO_1.0](docs/ROADMAP_TO_1.0.md). M0 source/bootstrap is the **first blocking task**. After source import, isolated coding agents work via PRs on CTF, storage, Windows, macOS and QA. No agent is considered launched solely because its Issue exists.

**Privacy:** never commit source CTF, converted user SQLite, media listings, software license keys, or actual disk paths. No automatic media hashing, content indexing or deletion.