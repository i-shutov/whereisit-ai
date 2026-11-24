# DuckDB RAM executor experiment — 24 November 2026

Status: **historical materialized-table experiment; native follow-up measured, not release-accepted**. The owner asked whether in-memory DuckDB would be preferable for search. No default production dependency, installer or published release was changed by this experiment. The subsequent [native CTF executor prototype](NATIVE_DUCKDB_CTF_PROTOTYPE_20261124.md) reads the actual parsed CTF directly and records separate end-to-end measurements; do not combine its timings with this Python experiment.

## Scope and method

Local Windows 11 x64, installed Python DuckDB **1.4.1**, 4 threads. The input was an existing full metadata Parquet export with **3,046,133 rows**, not the original CTF and not a freshly decoded Arrow stream. `CREATE TABLE AS SELECT FROM read_parquet` materialized ten metadata columns plus an extension column in an in-memory database. Extension auto-install/autoload were disabled; external access was disabled after reading the explicit local file. No SQLite extension, network access or source writes were used.

The final experiment set `memory_limit=1GB` and disabled temporary spill by setting an empty temporary-directory configuration. Each query returned the ten normal entry fields for up to 250 displayed rows. It compared exact match counts, ordered first-page IDs and continuation flags with a read-only pristine SQLite derivative of the same catalog. All ten cases agreed. This is not a new all-field/all-row comparison, proof for arbitrary queries, CTF-to-DuckDB integration or Windows 10 acceptance.

Sort order explicitly emulated SQLite's ASCII-only `NOCASE` plus entry ID. Five repeated samples per query were collected; p95 is the nearest-rank maximum of these five samples. OS cache was uncontrolled. Timings include the DuckDB query and fetching the bounded result, but **exclude live CTF SHA verification, CTF parsing, construction of Arrow columns, native GUI startup/rendering and JSON service transport**. The Rust service timings include CTF source verification and JSON results, so ratios between these columns are not a controlled executor-only comparison.

## Final strict-RAM measurements

- Parquet-to-memory table load: **850.522 ms**.
- First ordinary text query after loading: **377.730 ms**.
- Observed peak process working set: **850,067,456 bytes**; this includes the Python harness and reference connection, and was sampled rather than a strict allocator limit.
- Temporary spill: **0 bytes**. An earlier 512 MB run spilled 145,031,168 bytes and is not presented as strict RAM.

| Query class | DuckDB p50 ms | DuckDB p95 ms | Rust direct service p95 ms |
|---|---:|---:|---:|
| Empty query, sorted page | 1273.222 | 1397.899 | 180.243 |
| Extension | 22.525 | 25.307 | 187.914 |
| Ordinary text | 369.824 | 380.475 | 220.871 |
| Boolean | 129.350 | 135.820 | 182.564 |
| Unicode name, no matches | 55.490 | 60.214 | 1075.860 |
| Absent name | 55.919 | 57.730 | 753.164 |
| Absent ordinary text | 361.367 | 382.391 | 3204.331 |
| Absent path | 191.097 | 193.052 | 2639.702 |
| Absent description | 24.082 | 24.753 | 455.354 |
| Extension + size, exact count and page | 30.079 | 32.825 | 411.639 |

The Rust run opened the CTF snapshot in 1,733 ms and completed opening through its first ordinary text search at 4,200 ms, including building its in-memory global name order. It built no SQLite mirror. Its complete result pages matched the existing SQLite executor for the ten benchmark cases. These are service observations, not exact installed-GUI latency.

## Interpretation and required follow-up

DuckDB deserves a native columnar-backend prototype: full scans and aggregates are substantially promising, and its memory use fits this tested host. It is not uniformly faster: the sorted unfiltered page and frequent text-match page were slower in this experiment. DuckDB currently computes the ASCII sorting expression and evaluates its query over a materialized table; the Rust service retains a reusable name order and can stop a fast page early. Investigate cached sort keys/order and actual Arrow input before choosing a universal executor.

Keep the common query grammar and immutable CTF snapshot boundary. Prototype an Arrow-compatible columnar representation once, then measure native DuckDB queries over it without per-row SQL insertion or a mandatory FTS build. DuckDB documents direct Arrow integration with vectorized execution; converting our CTF representation to those columns still has a cost and is not automatically zero-copy. [DuckDB/Arrow integration](https://duckdb.org/2021/12/03/duck-arrow).

Unicode normalization needs an explicit shared contract. The installed DuckDB's `lower('ΟΣ')` yields `οσ`, while Rust/Python string lowercase yields `ος`; `lower('İ')` also differs. These are synthetic probes. The ten full-data cases agreeing does not eliminate this incompatibility. Feed consistently normalized columns or implement an equivalent tested normalization path. Validate masks, recursive branch/category scopes, cancellation and large-offset behavior too.

Do not replace filename substring/mask semantics with a default linguistic FTS configuration. The documented FTS extension has language/token normalization options and is a separately constructed index. It is not required for these measured scalar-filter queries. [DuckDB FTS documentation](https://duckdb.org/docs/current/core_extensions/full_text_search).

SQLite retains durable user state and existing native catalogs during this work. Removing that proven persistence layer does not accelerate CTF reads; any later migration needs its own data-preservation and crash-recovery evidence. The native follow-up restores linked tabular exports and copy candidates, with synthetic equivalence, decoded 15-field Parquet and cancellation/no-clobber coverage. Exact product-artifact acceptance remains pending. Keep `stable_ready=false`.

## Native direct-read checkpoint

Actual Windows GUI interaction on a working CTF copy observed 44 volumes, global results, Boolean search, page 251–500, volume filtering, a no-result path query, Escape cancellation of a long Boolean path query and successful retry. No precise native startup timing was captured. Containing-folder navigation is covered by synthetic service/UI tests; a successful physical button action was not established in this run. No GUI screenshot or raw accessibility tree is committed.

The final complete workspace run passed **366 tests**, with three manual benchmarks ignored. Formatting and workspace/all-target Clippy with warnings denied also passed. This includes the added exhaustive GLOB/Unicode parity and multi-volume heap-pagination regressions. The exact stable installer, native DuckDB integration and a new Windows 10 artifact remain unaccepted. Original CTFs and published 1.1.0 assets remain unchanged. Only aggregate measurements and synthetic examples appear in this report.
