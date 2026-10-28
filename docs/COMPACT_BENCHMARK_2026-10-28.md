# Compact v3: measured synthetic benchmark, 2026-10-28

**This is NOT the user's CTF catalog.** The deterministic generator creates 44 volumes and 3,046,027 records with synthetic names and roughly 1.2% nonempty annotation rows. Original CTF raw-record provenance is not included, so these byte counts cannot be substituted for measurements of the user's 2.16 GB v1 file.

Source: [`f77f1fdacd1966b92ba8d51e0dc7664df97e00ba`](https://github.com/i-shutov/whereisit-ai/commit/f77f1fdacd1966b92ba8d51e0dc7664df97e00ba).
CI: [run 40723571909](https://github.com/i-shutov/whereisit-ai/actions/runs/40723571909), large job `122940713380`.
Aggregate evidence archive: [artifact 12832313506](https://github.com/i-shutov/whereisit-ai/actions/runs/40723571909/artifacts/12832313506), SHA-256 `bf8ce6e313956959ceb2b9c01bcbeada42a42d5d828e4f547a87040a6272c12c` (the artifact ZIP, not the database).

## Size and fidelity

| Metric | Measured result |
|---|---:|
| Volumes / records | 44 / 3,046,027 |
| v1 bytes before duplicate cache | 1,319,284,736 |
| v3 bytes before duplicate cache | 1,130,594,304 |
| Reduction | 14.3025% |
| Optional description/tag rows | 36,520 |
| Entry mismatches during verified copy | 0 |
| Foreign-key errors | 0 |
| SQLite quick_check | ok |
| FTS integrity during copy | passed |
| Verified copy elapsed | 92.056 s |
| v1 import + vacuum elapsed | 338.517 s |
| Metadata duplicate candidates in tested movie scope | 1,015,300 |
| Duplicate page equality | passed |

The planned 30% storage reduction is **not achieved** by this experiment. It demonstrates real hot/cold separation and value-preserving migration, not the final compactness target.

## First-page query timings

The report labels the third sorted value of six warm samples as `warm_median_us` (the lower central sample, not an interpolated median). Below it is converted to milliseconds. Each query also had a first run; OS caches were not flushed, so no result is claimed to be a cold-cache measurement or a p95 guarantee.

| Query | Total matches | v1 warm lower-middle, ms | v3 warm lower-middle, ms |
|---|---:|---:|---:|
| `ext:mkv AND size:>4GiB` | 1,015,300 | 446.528 | 382.457 |
| `name:Запись-0000200` | 88 | 48.050 | 48.361 |
| `path:Медиатека AND ext:flac` | 1,015,344 | 2,387.448 | 2,309.041 |
| `description:редкое` | 30,448 | 75.831 | 83.182 |
| `NOT tag:favorite AND ext:mp3` | 1,011,291 | 647.950 | 767.977 |
| `category:"Synthetic category" AND ext:mkv` | 138,450 | 573.472 | 558.696 |
| `музыка OR name:0000042` | 30,536 | 82.661 | 154.465 |

All seven first-page ID sequences and total match counts agreed. This does **not** imply equality of every page was exhaustively tested; small pagination tests separately cover that behavior.

**Regressions are present:** annotations and especially unqualified cross-field search pay for reconstructing the search document and joining sparse rows. A broad path query remains above two seconds. The hot/cold change is not a blanket speedup, and the candidate does not meet every planned search SLO. These remain optimization/acceptance work rather than being hidden behind successful correctness tests.

## Reproduction

```sh
cargo run --release --locked -p atlas-service --example compact_benchmark -- 3046027 > metrics.json
```

The benchmark creates and removes its own temporary databases. It never loads a personal catalog. The same workflow also completed successfully at 100,000 records. `resources.txt` in each artifact contains `/usr/bin/time -v` observations; no peak-memory figure is claimed here without inspecting that file.

## Required next evidence

Measure the private original catalog locally, preserve its raw CTF provenance, verify complete unchanged CTF roundtrip, profile broad path/category ordering and sparse annotation predicates, and re-run cold/warm measurements on Windows and macOS. Do not mark stable 1.0 acceptance on the basis of this synthetic run alone.
