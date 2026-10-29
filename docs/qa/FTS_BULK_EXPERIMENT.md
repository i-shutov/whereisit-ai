# Bulk FTS5 rebuild on initial SQLite v3 CTF import (experiment)

The baseline importer updates the external-content trigram FTS5 index through
`core_ai` and `payload_ai` after every entry insertion. For a brand-new v3
database, this experiment temporarily removes those two insertion triggers
inside the *same transaction*, inserts all core/payload/CTF rows as usual,
runs `INSERT INTO entries_trigram(entries_trigram) VALUES('rebuild')` on the
current `entries` view, restores the original trigger SQL from
`sqlite_master`, analyzes planner statistics and commits. A failed operation
rolls back the entire transaction, including the trigger DDL.

For v1/v2 or an already populated v3 catalog, the existing incremental trigger
path is retained. An empty v3 database missing either required trigger refuses
the optimized mode instead of silently publishing an incomplete index.

The experimental API is `Catalog::bulk_import_many_fast_fts`; the CTF importer
uses it with the same source bytes and catalog schema. No CTF decoding,
write-compatibility or user annotation representation is changed.

## Acceptance

Synthetic tests must verify the exact first-page IDs and counts for ordinary,
FTS, description and Boolean queries, strict FTS integrity, post-commit
trigger updates, failed-transaction rollback and nonempty-database fallback.
The independent native Linux CLI and all-platform installer CI must pass.

**Only then** compare actual import seconds, SQLite size, memory, query/CTF
roundtrip, CSV/Parquet result parity on the exact same private source and
hardware. A synthetic win does not establish a real-data speedup. Do not merge
an optimization that increases regression risk without a clear repeatable gain.
The original user CTF never leaves the private local machine.

No signed/production release claims are made in this experiment.
