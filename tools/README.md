# Developer validation tools

These Python utilities are optional development tools. **Python and DuckDB are not dependencies of the native Atlas application or its Rust runtime.** Use Python 3.10 or newer. The tools contain no private catalog paths or volume identifiers and write aggregate JSON without result filenames, paths, disk labels or entry IDs.

## Benchmark an existing catalog

Build or obtain the Atlas CLI separately, then provide an existing catalog database:

```sh
python tools/benchmark_catalog.py --cli /path/to/atlas --db /path/to/catalog.sqlite --output benchmark.json
```

On Windows, pass the path to `atlas.exe`. This command measures seven fixed searches: an unfiltered first page, Cyrillic and Latin name substrings, a Boolean movie expression with a size filter, music extensions, a path/music expression, and an MKV name glob. Each case requests a page of 250 entries once, then five more times; the report includes the first duration, warm median and warm maximum. Returned row counts and `has_more` are reported. The default does not compute exact totals for every search.

To measure the optional exact count as a separate operation:

```sh
python tools/benchmark_catalog.py --cli /path/to/atlas --db /path/to/catalog.sqlite --output counted-benchmark.json --exact-count
```

Default requests are statistics and searches. The tool does not scan disks, import/export CTF, create temporary categories, edit annotations, run `ANALYZE`, or flush caches. Use a current Atlas database; opening an older database with the CLI may apply that CLI's ordinary schema migration behavior. Close other writers before benchmarking. Page changes during repeated requests cause the benchmark to fail rather than silently combine inconsistent results.

Duplicate-candidate analysis requires an explicit flag because it **creates or updates derived cache tables**:

```sh
python tools/benchmark_catalog.py --cli /path/to/atlas --db /path/to/catalog.sqlite --output duplicates-benchmark.json --duplicates
```

It measures candidates for all files, movie extensions and music extensions using name-and-size metadata. No file contents are compared, so these are not verified byte-identical duplicates. The first call can build or reuse a cache; it is not guaranteed to be a cold-cache build. A five-minute timeout applies per service request and can be changed with `--timeout SECONDS`. A timed-out child service is stopped; SQLite handles rollback of its unfinished transaction.

The process uses one persistent `atlas serve` child. It records Python/platform information, visible CPU count and the CLI binary's SHA-256, but does not isolate concurrent machine load or evict OS caches. Do not compare runs as a controlled experiment unless those conditions and the database revision are controlled separately. `--output` must be a new file; existing outputs and database sidecars are protected from replacement.

## Independently compare CSV and Parquet

The benchmark uses only the Python standard library. The independent Parquet checker additionally needs the optional developer dependency tested for this project:

```sh
python -m pip install duckdb==1.4.1
python tools/verify_exports.py exported.csv exported.parquet --output export-check.json
```

Without DuckDB, the checker fails with an installation instruction; it does not report an incomplete comparison as successful. The detected DuckDB version is recorded in the JSON. Pinning 1.4.1 reproduces the validated reader version.

The checker reads existing files only. It does not access the application database or invoke Atlas. It verifies:

- Exactly the 15 Atlas export column names, in order, and matching row counts.
- Every CSV/Parquet field, row by row; Parquet NULL is normalized to a blank CSV field.
- Parquet BIGINT types for integer columns, signed 64-bit values, and exact nonnegative sizes without floating-point conversion.
- UTF-8 text equality, aggregate Cyrillic name/path coverage, nullable dates/parents and valid timezone-free local date strings.
- Aggregate size minima/maxima/sums and the count of rows above 4 GiB.

Reports contain aggregates and no row contents. A mismatch reports the row number but omits its values. Comparison assumes both exports have the same ordering and scope; it cannot prove that two equally wrong exports accurately reflect their source database. CSV fields are bounded at 16 MiB and Parquet is read in batches of 2,048 rows with a 128 MiB DuckDB memory limit. `--output` is optional; JSON is also printed to stdout. Existing output files are not overwritten.

## Checks performed on these utilities

Both utilities passed Python bytecode compilation. The committed `verify_exports.py` was run against the completed real `large_movies` export pair: 321 rows, 15 columns, exact field equality, and all 321 file sizes above 4 GiB. The largest size was 40,898,551,720 bytes. The reusable benchmark was not rerun against the large database merely to test its wrapper; the measured runtime evidence remains in [the validation report](../docs/VALIDATION.md).
