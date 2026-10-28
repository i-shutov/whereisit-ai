# ADR-0001 · Storage engine

Status: provisional acceptance, subject to workload benchmark (2026-10-28).

**Decision:** native Rust/SQLite remains durable transactional primary store; explore DuckDB/Parquet as optional analytics and duplicates accelerator; Polars for evaluation/exports, not startup RAM data structure.

Reason: primary UX is tree navigation, mutable categories, single-record updates, Boolean/FTS search and fetching the first 100–250 rows. SQLite supports indexes/FTS5/WAL/foreign keys and concurrent reader behavior. DuckDB optimizes scans, grouping and columnar exports; its ART index is designed mainly for very selective queries. Polars lazy streaming does not replace a durable transactional DB.

The previous 3M-row SQLite copy is about 2.16GB, vs source CTF 144.8MB: this is a clear optimization target, but **not** proof that a new database engine would be better. Current copy repeats paths/search fields and preserves opaque original CTF for fidelity.

**Experiment gate:** same source hash, 3M rows, comparable metadata columns and opaque payload preservation; cold/warm latency for name/extension/category/Boolean/duplicates, first page vs COUNT; peak RSS; disk bytes; incremental updates; migration/rollback; Unix/DOS date integrity and CTF roundtrip. No engine switch without reproducible evidence.

Refs: https://sqlite.org/fts5.html ; https://duckdb.org/docs/current/sql/indexes ; https://docs.pola.rs/user-guide/lazy/sources_sinks/ .