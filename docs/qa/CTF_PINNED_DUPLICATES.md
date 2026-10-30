# Read-only linked CTF metadata duplicate candidates

## Exact scope

Use the native Windows/macOS **Link CTF** action, then choose a disk or
**All disks**, set an optional Boolean query such as `ext:mkv OR ext:flac`,
select **Name + size / Name / Size**, and click **Duplicates**.

The original CTF is the authority. This feature never writes the CTF or
the immutable SQLite generation. All entry metadata are read through the
existing full-source-SHA and cached-index stamp `ctf_*_pinned` gate.

The API is:

```json
{
  "op": "ctf_duplicates_pinned",
  "path": "/private/original.ctf",
  "cache_root": "/private/CTF-Mirrors",
  "generation": "<returned-by-ctf_sync>",
  "duplicate_mode": "name_size",
  "query": "(ext:mkv OR ext:mp4) AND size:>500MiB",
  "volume_id": null,
  "offset": 0,
  "limit": 250
}
```

The `duplicate_mode` choices are `name_size`, `name`, `size`.
`hash` remains explicitly **unsupported**: a matching name/size never
proves identical bytes. The grouping is based on all matching *files*
after query and volume filtering, before pagination. Entries representing
directories are excluded.

The result is wrapped in `{generation, source_sha_checked, result}`.
`result` has `items` with `duplicate_count` and `duplicate_group`,
`verified:false`, `basis:metadata_candidates`, `offset`, `limit` and
`has_more`. Exact `total` is null (not zero): do not infer it from the
current page. All emitted counts refer to the **selected query scope**.
A subsequent page is a new SHA-pinned request; if another application
edits the CTF, the old generation fails with `source_changed`.

## Correctness and performance boundary

The implementation runs one **read-only SQLite SELECT** with a window
count partitioned by indexed normalized names and/or sizes. A bounded
`limit+1` window yields `has_more`; only paginated row details are
hydrated. No `CREATE TABLE`, `INSERT`, `UPDATE` or mutable duplicate
cache is permitted. Local SQLite may use its own temporary query work
storage; it cannot modify the read-only original or the published mirror.

This is an initial **correctness** vertical, NOT a proven fast 3-million
entry duplicates index. The first global query may be expensive; it runs
outside the UI thread. Later work can add an independently validated,
immutable, per-generation derived duplicates cache. Do not claim fast
subsecond global duplicates or a content-hash match without measured
evidence.

CSV/Parquet/JSON duplicate-candidate export now uses the same immutable
source-generation check as normal linked reports. From the Duplicates view
choose **Export…** (Windows) or **Export** (macOS). The service call is
`ctf_export_pinned` with `mode:"duplicates"`, `duplicate_mode`, the
optional Boolean `query` and `volume_id`. It exports **every candidate
in the filtered grouping scope**, not merely the visible GUI page.
The output reports `duplicate_candidates:true`, `verified:false`,
`basis:"metadata_candidates"`, and the grouping mode. Neither the original
CTF nor its read-only SQLite mirror is written; the output is a new,
no-clobber file verified against the pinned source both before and after
streaming. A linked duplicate report cannot include a parent subtree or
an offset/limit: such requests fail explicitly instead of exporting a
different scope. A hash-based duplicate report remains unsupported.
See `CTF_PINNED_EXPORT.md` for publication and stage cleanup contracts.

## Test contract

`cargo test --locked -p atlas-service --test ctf_pinned_duplicates_contract`

Tests build **synthetic** CTFs: two disks with repeated Unicode-casefold
names, same/different sizes, a directory, mixed extensions. They verify:
1. file-only name/size/name+size grouping, with explicit `verified:false`;
2. optional Boolean query, disk-scoped counts and deterministic pagination;
3. unsupported hashes, invalid inputs and immutable SQLite bytes;
4. same-size external CTF rewrite invalidates a pinned request;
5. full duplicate candidate CSV, JSON and physical Parquet row counts match
   the selected metadata grouping, not a paginated or broader search; stale
   generation, no-clobber, hash mode, and illegal scopes fail closed.

Full CI additionally requires Linux tests/fmt/Clippy, Windows native
installer and both macOS architecture app/package smoke jobs. No private
CTF file, cache or user-specific metadata are used by public GitHub CI.

**Not closed:** original WhereIsIt modified-CTF acceptance; opaque 0x10
metadata semantics; two private historical CTF tests; real 3M duplicate
latency/RSS benchmarks; manual DPI/UX verification; code signing/notarization;
user annotation reconciliation. This capability is not a stable 1.0 release.
