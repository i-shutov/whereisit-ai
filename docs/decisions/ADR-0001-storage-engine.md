# ADR-0001: Primary storage SQLite; optional analytical backend DuckDB

Status: **Accepted provisionally**, benchmark gate before v1.0.  
Date: 2026-10-28.  
Applies to: WhereIsIt AI, 3,046,027 offline catalog records, 44 media volumes.

## Decision

Use SQLite as the durable, editable, indexed metadata store. Keep runtime in native Rust. Evaluate DuckDB as an optional batch engine for duplicate grouping/statistics and Parquet export and Polars as a developer-side comparison; do not load the entire database into RAM on launch.

## Rationale

The default path is low-latency point and range lookup, tree navigation, Boolean expressions, mutable categories/annotations, transactionally safe catalog refresh, and 100–250-row display pages. SQLite's B-trees, WAL, FTS5 trigram, foreign keys and single-file portability suit this mixed OLTP/search workload. A 3M-row in-memory DataFrame would incur startup/resident-memory and index recreation costs, and would not itself solve cross-session updates or offline durability. DuckDB excels at bulk scans, GROUP BY and columnar export, not every tiny indexed lookup; its ART index has a documented selectivity/memory trade-off. Polars is a lazy computation engine, not a substitute for transactional storage.

SQLite's current footprint (~2,161,020,928 B for the compact migrated copy vs CTF 144,766,574 B) is an implementation problem, not evidence that SQLite itself is fundamentally unsuitable. Major opportunities: relative-path representation, eliminating repeated normalized strings, leaner category closure, separate original CTF recovery data, and alternative FTS5 materialization. Preserve all original metadata and search functionality.

## Required same-data experiment

1. Freeze a SHA-256 identified copy of the 44-disk database, toolchain versions and host hardware; isolate from GUI and other writers.
2. Measure bytes allocated by SQLite `dbstat`/`page_count`, list of indexes, FTS and duplicates cache. Capture `EXPLAIN QUERY PLAN` for exact name, prefix, substring (Unicode/Cyrillic), AND/OR/NOT, category (374k subtree), 50k-page scan, duplicate groups and subtree export.
3. Export canonical `volume_id`, `parent_id`, `name`, `size`, `kind`, `path`, `ext`, `category` in stable order to Parquet; import the same columns into a persistent DuckDB file, measure file size/index size. Keep distinct legacy blobs to avoid apples-or-oranges compression comparisons.
4. Run each task 5+ warm times, one cold-cache run with explicit state, record median/p95 and peak RSS. `SELECT ... LIMIT 250` and `COUNT(*)` are separate tasks. Test the cost of incremental update, category reassignment and crash rollback.
5. Optionally use Polars lazy scan with predicate/projection pushdown and a memory cap for `GROUP BY` and export; compare correctness by stable IDs/counts and hashes. Do not claim in-memory results if only a truncated/filtered subset was tested.
6. Decision after evidence: stay SQLite, hybrid DuckDB, or reconsider store. Any change requires migration/rollback and CTF→DB→CTF roundtrip revalidation.

## Guardrails

- Analytical engine is not introduced into installers until savings/latency benefit is demonstrated and external runtime/size/license/CI cost is measured.
- No concurrent writers to the source catalog in measurement; never mutate the user's original CTF.
- DuckDB/Polars are entirely optional for v0.1 and do not modify the primary catalog schema through unreviewed operations.
- Peak RAM is a hard requirement; tests need explicit budgets on 8 GB/16 GB machines.

## References

- SQLite FTS5: https://www.sqlite.org/fts5.html
- DuckDB ART indexes: https://duckdb.org/docs/current/sql/indexes
- DuckDB performance: https://duckdb.org/docs/current/guides/performance/how_to_tune_workloads
- Polars lazy sources: https://docs.pola.rs/user-guide/lazy/sources_sinks/
