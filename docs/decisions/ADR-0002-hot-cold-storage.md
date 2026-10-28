# ADR-0002 — Hot indexed fields and sparse cold payload

**Status:** Accepted as a storage-design constraint; implementation and migration pending.  
**Date:** 2026-10-28.  
**Related:** [ADR-0001](ADR-0001-storage-engine.md), [storage Issue #3](https://github.com/i-shutov/whereisit-ai/issues/3).

## Problem and evidence

The original CTF is 144,766,574 bytes, while the smallest validated SQLite snapshot of 44 volumes and 3,046,027 entries is 2,161,020,928 bytes. The v1 schema holds `relative_path`, `name_norm`, `search_text`, `description` and `tags` in **every** `entries` row. Trigram FTS indexes `search_text = name + path + description + tags`, materializing and indexing repeated data. Imported CTF maintains a `ctf_entries` row for each entry with DOS times, flags and an `opaque` BLOB even when the opaque field is empty. This is not proof that SQLite is unsuitable: it is a physical schema that has not separated indexed fields from infrequent/sparse metadata.

The user requires a clear distinction between *indexed fields* and *rare payload*: absence of a payload must not cause an extra payload row; search/list/tree/duplicates must not read it by default. "Rare" may mean both *non-null for a small fraction* and *rarely accessed*. Confirm actual distributions (non-empty descriptions/tags/opaque by entry, byte histograms, compressed/uncompressed cost) before migration.

## Design constraints

1. **Hot indexed core**: dense, minimal `entry_id`, `volume_id`, `parent_id`, `kind`, `name`, `size`, needed timestamps and indexing-specific search keys. Index the queries actually required for disk navigation, name/extension/size, categories and duplicate grouping. Derive extension or store a compact normalized key only where measured to be beneficial. Hot lookup must not join payload.
2. **Sparse optional payload**: separate `entry_payload(entry_id PRIMARY KEY REFERENCES entries(id), description, tags, additional_metadata...)` or equivalent typed tables. **No row for empty/default payload**; no per-row JSON object for millions of empty values. Keep domain-specific optional attributes typed where they need to become searchable. Load details via explicit `get_entry_details(id)` or `batch_get_payload(ids)`; fetch only the currently displayed page (if Description column visible) and stream bulk exports.
3. **Sparse text search**: keep name/path candidates separate from *payload* text posting lists. Only entries with nonempty descriptions/tags participate in payload FTS; preserve existing `description:` / `tag:`, unqualified search, AND/OR/NOT, Unicode and exact/wildcard semantics. FTS is an index, not a reason to copy every field into a second per-entry concatenated `search_text`. Any negative search must use correct universe semantics, not an accidental inner join.
4. **Path hierarchy**: canonical directory tree is `parent_id` plus name. Avoid materializing full `relative_path` on every file unless the path-search benchmark proves it required; consider a compact folder-path cache/trie or path posting index, with valid invalidation on renames/moves. Path glob and substring search must remain correct.
5. **Legacy CTF fidelity**: separate *dense legacy primitives* (flags/DOS timestamps if common) from *rare opaque extensions* rather than assuming all legacy data is sparse. Preserve encoding, original record ordering, unknown bytes, flags and version fields. Apply optional lossless compression to cold byte payload only after verifying **byte-for-byte** unchanged CTF → SQLite → CTF. New/modified CTF must still be verified in original WhereIsIt. Do not deduplicate or transform opaque bytes semantically.
6. **No eager hydration**: a search page requests at most the page of hot rows; detail panes and optional Description columns are hydrated on demand. Exports are streaming and may join cold rows in bounded batches. Startup must not materialize 3M entries or payload in RAM.
7. **Storage engine**: retain SQLite unless controlled same-dataset benchmarks demonstrate another engine is better. Optional DuckDB/Parquet for batch analytics does **not** justify loading rare payload to memory for indexed lookup.

## Example conceptual schema (not an executable migration)

```sql
CREATE TABLE entries_core (
    id INTEGER PRIMARY KEY,
    volume_id INTEGER NOT NULL,
    parent_id INTEGER,
    name TEXT NOT NULL,
    kind INTEGER NOT NULL,
    size INTEGER NOT NULL,
    modified_time INTEGER
);
CREATE INDEX entries_core_tree ON entries_core(volume_id, parent_id, name, id);
CREATE TABLE entry_payload (
    entry_id INTEGER PRIMARY KEY REFERENCES entries_core(id) ON DELETE CASCADE,
    description TEXT,
    tags TEXT,
    additional_data BLOB
);
-- Insert an entry_payload row ONLY if at least one payload value exists.
-- Dedicated FTS/search tables can index nonempty name/path/description/tag
-- fields separately. Do not repeat the whole entry in a concatenated text row.
-- Legacy fixed-size CTF data is a distinct concern; it may be dense.
```

The precise schema, FTS strategy and whether compression is useful must be chosen from measured distributions and query plans; the sketch is not an implementation command.

## Migration and acceptance gates

- Make `schema-v2` in a **copy** of the real catalog; existing `schema-v1` must remain readable and restorable. Never change/rebuild the original user database in-place without backup, journal/rollback and explicit version checks.
- Report actual row counts and bytes for core, optional payload, `ctf_entries`, CTF opaque sections, FTS, B-tree indexes and duplicate cache. Measure `dbstat`, page_count, p50/p95 and RSS before/after.
- Validate all 44 volumes, 3,046,027 entries and a deterministic set of result IDs for Boolean queries, category inheritance, name/path/description/tag queries, duplicate candidate groups and exports; preserve large integer file sizes, DOS dates and Unicode.
- Confirm missing payloads yield no payload record; description/tag queries still find all previously searchable entries; unsupported CTF descriptions continue to be preserved losslessly even if not decoded.
- Revalidate unchanged CTF → new SQLite → CTF **SHA-256 identity**, plus synthetic mutated CTF and explicit loss-prevention behavior. No silent truncation or omitted fields.
- Measure with cold/warm cache; benchmark target is >=30% storage reduction vs 2,161,020,928 B **without correctness or latency regressions**. This is a goal, not a guaranteed outcome.
- Deliver migration/backout plan, tests, documentation and measured results through Issue #3 and a reviewed PR. Do not infer success from a synthetic sample alone.

**Decision:** hot/cold separation is a priority of M2, preceding any proposed rewrite to DuckDB or a full in-memory Polars data frame.
