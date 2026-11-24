# Native DuckDB CTF executor prototype

24 November 2026. Development experiment; not a release or installer acceptance.

## Boundary

The optional `atlas-service/duckdb-query` feature exposes the existing immutable,
parsed CTF snapshot as a native DuckDB table function. The table function supplies
only projected columns, in bounded vectors, from the same shared Rust catalog.
It does not import records into SQLite, read Parquet, launch Python, or create a
DuckDB database file. This prototype measures native vectorized execution; it
does not yet provide a persistent Arrow cache or a once-materialized RAM table.
Normalization and path construction currently happen when projected vectors are
requested, so the earlier Python materialized-table measurements cannot predict
its performance.

The common typed query AST emits bound SQL parameters. Rust supplies normalized
Unicode text, SQLite-compatible GLOB matching, ASCII-only case-folded sort keys,
and stable global entry IDs. DuckDB returns a bounded page of IDs; normal service
rows are materialized from the authoritative snapshot. SQLite remains the durable
store for native catalogs and user state.

The engine uses four threads, a 512 MB DuckDB memory limit, disabled temporary
spill, disabled external access and disabled extension auto-install/autoload.
These settings are verified after opening the in-memory connection. The memory
limit does not bound the Rust snapshot or total process working set. Cancellation
is checked per vector and before publication. Full source identity checks remain
part of the service boundary. No network access or source mutation is required.

## Build and deployment

Normal product builds keep this dependency disabled. The desktop development
feature `qa-duckdb` enables the experimental executor; explicit service requests
can select `executor: "rust"` or `executor: "duckdb"` for comparisons. Requesting
DuckDB from a build without the feature returns an explicit unsupported error.

The Rust dependency is pinned to `duckdb = 1.4.1`. Local Windows QA uses the
official `libduckdb-windows-amd64.zip` for DuckDB 1.4.1, verified before extraction:

```text
SHA256 704877e85b556640b0f20a0f07a1c1f404320210b245f51b6da0087a1c92bb76
```

Set `DUCKDB_LIB_DIR` to the extracted library directory for linking and provide
the matching DLL beside the isolated QA executable or on its process-local PATH.
The DLL is not vendored into the repository or existing installer. The optional
`duckdb-bundled` service feature is available for source builds; no bundled build,
Windows 10 compatibility or offline installer acceptance is implied here.

## Verification

The service's existing synthetic SQLite query/page equivalence tests also run
against DuckDB when the feature is enabled. Additional tests exercise empty scans,
parent nullability across vector boundaries, Unicode normalization, wildcard
semantics, cancellation and connection reuse. Copy candidates and tabular exports
continue to use the immutable source and bounded streaming operations independently
of the selected search executor.

The ignored release-mode `direct_ctf_first_search_and_query_latency` benchmark
accepts `CTF_BENCH_EXECUTOR`. A private working copy and optional read-only SQLite
reference stay outside the repository. The benchmark reports only aggregate
latencies and Windows process working set, compares complete displayed pages,
and checks source bytes before and after. OS caches are uncontrolled. This does
not measure an installed GUI or establish strict disk-cold timings.

## Full-catalog native result

Local release-mode Windows 11 x64 run: 44 volumes / 3,046,133 entries. Opening
took **1,558 ms**; opening through the first ordinary text search took **2,412 ms**.
No complete import, persistent search index, SQLite mirror, intermediate Parquet
or DuckDB database file was built. All ten benchmark classes agreed with the
read-only SQLite reference for the complete displayed page and count/continuation
behavior. The source-byte checks passed. Five samples per class; nearest-rank p95
is the maximum of five observations.

| Query class | Native DuckDB p50 ms | Native DuckDB p95 ms | Rust p50 ms | Rust p95 ms |
|---|---:|---:|---:|---:|
| Empty query | 239.400 | 258.549 | 167.432 | 168.084 |
| Extension | 302.800 | 306.256 | 174.071 | 179.366 |
| Ordinary text | 807.265 | 832.077 | 197.052 | 201.010 |
| Boolean | 368.656 | 369.400 | 159.381 | 162.399 |
| Unicode name | 310.668 | 324.895 | 919.122 | 934.166 |
| Absent name | 310.472 | 317.717 | 643.519 | 668.650 |
| Absent ordinary text | 831.083 | 899.080 | 2543.461 | 2785.067 |
| Absent path | 520.910 | 535.474 | 2299.121 | 2360.520 |
| Absent description | 252.570 | 255.185 | 412.137 | 416.886 |
| Extension + size, exact count | 399.807 | 421.703 | 401.356 | 404.423 |

Windows reported a process-lifetime peak working set of **1,032,208,384 bytes**,
with **790,110,208 bytes** at the end of the query loop. This includes the parsed
CTF, DuckDB, service results and read-only reference connection. It is not an
executor-only allocation measurement. Service timings include source SHA checks,
normalization/path construction, query execution and JSON serialization, unlike
the earlier Python materialized-table experiment. The measurements support
continuing the native prototype, not declaring DuckDB universally preferable or
claiming one-second vendor-equivalent opening.

The Rust comparison used the same feature-enabled release executable with an
explicit Rust executor, sequentially after the DuckDB run, with the same reference
and five-sample harness. It opened in **1,520 ms**, completed opening through first
search in **3,644 ms**, and reported peak working set **1,032,302,592 bytes**. Both
executors matched the reference. These are uncontrolled-cache observations, not
randomized trials. DuckDB improved first-search startup and several full scans;
the Rust reusable name-order path retained faster common repeated pages. A
universal executor replacement is not justified by these results. Next investigate
bounded string-buffer reuse and routing that preserves shared query semantics,
then repeat the end-to-end measurements before enabling a product default.

## Native desktop checkpoint

The isolated release-mode `qa-isolated,qa-duckdb` desktop build was exercised
through the Windows desktop on the full working copy. Observed: 44 volumes,
global results, Boolean extension/size query, page 251–500, a candidate on another
disk with the metadata-only warning, selection of that candidate, and a successful
physical action opening its containing catalog folder. Exporting that folder
created a new CSV with seven records (folder plus six children), all 15 fields,
and a completed-export status. A subsequent extension search returned rows again.
No SQLite files appeared in the isolated application-data directory. Original
and working-copy source hashes remained unchanged.

Escape cancellation was attempted, but both chosen long requests had already
completed before Escape; this run does not establish GUI cancellation. Engine
cancellation/reuse is covered by a deterministic synthetic test. GUI JSON/Parquet
export, a packaged offline installer, and installed Windows 10 performance remain
unverified for this prototype. Existing release artifacts were not replaced.

## Remaining product parity

The direct adapter currently routes browsing, search, candidates and tabular
exports. Analytics/sunburst requests and writable annotation/CTF-edit operations
still require explicit import into a native catalog. This is an unfinished
integration boundary, not full 1.1 parity. Before a product hotfix, audit every
shipped operation: provide direct read-only aggregates, preserve workspace context,
and make any explicit conversion/edit transition visible and safe. Do not hide
conversion behind the Open command again or declare the cold-open defect closed
on search-only coverage.

Keep `stable_ready=false`. Published 1.1.0 artifacts remain immutable.
