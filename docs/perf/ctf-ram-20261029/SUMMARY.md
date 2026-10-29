# CTF RAM / SQLite / columnar measured study

All data below are synthetic. No personal catalog was uploaded. Times are milliseconds; medians of six warm repetitions after one first run. First run is not a cold-OS-cache measurement. The released application returns hydrated Entry objects; the other comparisons return ordered IDs only.

## 100,000 entries

Synthetic CTF: 4758839 bytes. Unchanged roundtrip equal: True.
CTF read / parse / arena construction / ordered indexes: 0.003 / 0.022 / 0.030 / 0.081 seconds.
Hot row size: 40 bytes; major arena capacities: 14373760 bytes, excluding raw CTF and associative containers. Process high-water RSS is separately recorded and includes parsing/serialization transient allocations.

| Case | RAM scan | RAM candidate lists | app | sqlite-direct | sqlite-tuned | sqlite-ram-tuned | duck-file | duck-ram | duck-parquet | duck-sorted | polars-ram | polars-parquet |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Q0: ext:mkv AND size:>4GiB | 0.007 | 0.002 | 0.954 | 0.551 | 0.083 | 0.080 | 2.025 | 3.481 | 2.922 | 2.351 | 1.797 | 2.885 |
| Q1: name:Запись-0000200 | 10.326 | 10.783 | 2.000 | 32.303 | 33.278 | 27.614 | 0.922 | 3.113 | 1.407 | 1.273 | 1.982 | 1.841 |
| Q2: path:Медиатека AND ext:flac | 0.028 | 0.024 | 74.777 | 1.908 | 0.316 | 0.327 | 2.246 | 4.769 | 3.100 | 2.470 | 2.574 | 2.692 |
| Q3: description:редкое | 0.434 | 0.007 | 4.646 | 1.625 | 1.537 | 1.444 | 1.203 | 2.445 | 1.831 | 1.572 | 1.434 | 2.056 |
| Q4: NOT description:редкое AND ext:mp3 | 0.012 | 0.006 | 20.931 | 1.686 | 0.907 | 0.909 | 2.145 | 3.892 | 3.036 | 2.367 | 2.102 | 2.903 |
| Q5: category:featured AND ext:mkv | 0.040 | 0.010 | 17.281 | 1.612 | 0.367 | 0.372 | 0.819 | 1.138 | 1.747 | 1.671 | 1.082 | 2.191 |
| Q6: музыка OR name:0000042 | 3.363 | 3.298 | 12.133 | 28.307 | 28.528 | 25.435 | 2.330 | 15.372 | 3.023 | 2.894 | 4.381 | 10.381 |
| Q7: name:zzzzzznomatch | 13.397 | 12.657 | 0.466 | 33.020 | 33.405 | 30.622 | 0.681 | 1.756 | 0.969 | 0.951 | 1.204 | 1.100 |
| Q8: name:з | 0.067 | 0.052 | 7.238 | 0.413 | 0.385 | 0.380 | 3.839 | 5.514 | 4.494 | 4.016 | 4.060 | 3.641 |
| Q9: path:album-0077 AND ext:flac | 4.661 | 1.494 | 3.056 | 117.323 | 33.287 | 33.434 | 0.863 | 2.520 | 1.543 | 1.253 | 1.398 | 1.346 |
| Q10: name:*0000042*.mkv | 4.227 | 0.986 | 2.809 | 34.251 | 33.374 | 28.371 | 0.824 | 3.461 | 1.135 | 1.049 | 2.131 | 1.294 |
| Q11: (ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB | 0.048 | 0.041 | 22.627 | 1.080 | 1.027 | 1.037 | 2.502 | 4.922 | 3.552 | 2.791 | 3.624 | 3.906 |
| Q12: size:>9000000000B | 0.434 | 0.000 | 0.097 | 29.738 | 31.587 | 27.371 | 0.335 | 0.358 | 0.492 | 0.456 | 0.479 | 0.920 |

### Storage / startup / duplicate grouping

| Engine | File bytes | Load/copy seconds | Peak RSS KiB | Duplicate build ms |
|---|---:|---:|---:|---:|
| Rust arena process | original CTF retained | 0.136 | 45700 | 60.523 |
| app | 45252608 | 7.637519913 | 32160 |  |
| sqlite-direct |  | None | 48508 | 10.881000000011909 |
| sqlite-tuned |  | None | 59472 | 10.095014999990326 |
| sqlite-ram-tuned |  | 0.05084035700002687 | 74504 | 10.06223499999237 |
| duck-file |  | 0.00894330699998136 | 94584 | 3.9691300000015417 |
| duck-ram |  | 0.09786836899999685 | 139136 | 4.129653000006783 |
| duck-parquet |  | 0.009078340000002072 | 96224 | 5.972072999981037 |
| duck-sorted |  | 0.008991890000004332 | 95432 | 4.208752000010918 |
| polars-ram |  | 0.011508888999998135 | 129248 | 4.1695419999996375 |
| polars-parquet |  | 0.0001802620000148636 | 119508 | 4.658611000024848 |

DuckDB + Parquet preparation: `{"duckdb_version": "1.5.5", "threads": 2, "import_tsv_s": 0.23142625500000236, "point_index_build_s": 0.10387030600000458, "duckdb_bytes": 2895872, "parquet": {"canonical": {"seconds": 0.04915907399998787, "bytes": 421238}, "sorted": {"seconds": 0.10880015600000092, "bytes": 312872}}, "canonical_has_raw_ctf_provenance": false, "warning": "These derived analytical files omit raw CTF bytes. Add original CTF size before comparing durable footprints.", "memory": {"peak_rss_kib": 242748, "rss_kib": 92908}}`

**Coverage:** all compared query first-page ID sequences and exact match counts are asserted against the Rust reference. Duplicate group and member counts are also asserted. This does not prove all possible expressions, payload types or all full result sequences. CTF external-application acceptance is not tested.

## 3,046,027 entries

Synthetic CTF: 144712884 bytes. Unchanged roundtrip equal: True.
CTF read / parse / arena construction / ordered indexes: 0.076 / 0.595 / 0.687 / 3.159 seconds.
Hot row size: 40 bytes; major arena capacities: 302060516 bytes, excluding raw CTF and associative containers. Process high-water RSS is separately recorded and includes parsing/serialization transient allocations.

| Case | RAM scan | RAM candidate lists | app | sqlite-direct | sqlite-tuned | sqlite-ram-tuned | duck-file | duck-ram | duck-parquet | duck-sorted | polars-ram | polars-parquet |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Q0: ext:mkv AND size:>4GiB | 0.126 | 0.002 | 10.452 | 9.517 | 0.078 | 0.080 | 20.996 | 34.083 | 73.296 | 17.040 | 13.775 | 44.392 |
| Q1: name:Запись-0000200 | 310.787 | 319.834 | 46.146 | 957.437 | 941.627 | 1091.849 | 57.252 | 44.605 | 85.506 | 6.883 | 38.405 | 72.171 |
| Q2: path:Медиатека AND ext:flac | 0.162 | 0.015 | 2269.008 | 31.546 | 0.327 | 0.324 | 59.850 | 59.339 | 133.668 | 18.857 | 42.632 | 123.967 |
| Q3: description:редкое | 1.511 | 0.008 | 86.479 | 106.111 | 101.001 | 62.778 | 13.943 | 22.457 | 46.755 | 11.732 | 12.421 | 25.703 |
| Q4: NOT description:редкое AND ext:mp3 | 0.183 | 0.005 | 602.259 | 45.428 | 26.173 | 26.376 | 23.214 | 36.477 | 71.854 | 19.229 | 26.579 | 37.440 |
| Q5: category:featured AND ext:mkv | 0.216 | 0.008 | 609.380 | 10.699 | 0.384 | 0.396 | 3.609 | 6.072 | 13.108 | 8.379 | 6.171 | 9.245 |
| Q6: музыка OR name:0000042 | 11.497 | 12.454 | 155.782 | 95.857 | 93.765 | 95.430 | 244.857 | 175.411 | 364.712 | 33.370 | 109.238 | 203.855 |
| Q7: name:zzzzzznomatch | 374.791 | 372.459 | 0.533 | 956.435 | 966.261 | 1070.871 | 38.823 | 25.141 | 63.477 | 6.709 | 24.116 | 61.140 |
| Q8: name:з | 1.439 | 1.332 | 173.182 | 9.559 | 9.715 | 9.521 | 89.576 | 74.546 | 87.384 | 54.673 | 45.933 | 81.202 |
| Q9: path:album-0077 AND ext:flac | 18.546 | 17.614 | 56.459 | 426.385 | 119.725 | 114.453 | 43.740 | 37.247 | 126.094 | 5.163 | 30.443 | 105.888 |
| Q10: name:*0000042*.mkv | 310.279 | 139.756 | 17.284 | 989.820 | 994.614 | 1125.685 | 57.105 | 41.317 | 81.889 | 7.527 | 34.952 | 60.891 |
| Q11: (ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB | 0.221 | 0.160 | 390.872 | 10.356 | 12.252 | 10.833 | 63.692 | 61.872 | 147.005 | 22.070 | 68.468 | 144.451 |
| Q12: size:>9000000000B | 33.078 | 0.002 | 1.565 | 849.303 | 875.537 | 1008.215 | 5.071 | 13.478 | 48.480 | 3.107 | 1.748 | 35.382 |

### Storage / startup / duplicate grouping

| Engine | File bytes | Load/copy seconds | Peak RSS KiB | Duplicate build ms |
|---|---:|---:|---:|---:|
| Rust arena process | original CTF retained | 4.517 | 1078912 | 2610.143 |
| app | 1386143744 | 250.657299588 | 806016 |  |
| sqlite-direct |  | None | 94304 | 349.0448899999592 |
| sqlite-tuned |  | None | 327004 | 353.94061099998453 |
| sqlite-ram-tuned |  | 1.511607411 | 1623688 | 329.57093799996073 |
| duck-file |  | 0.009677044999989448 | 301964 | 68.3962989999145 |
| duck-ram |  | 2.2385119599999825 | 1558108 | 53.18587400006436 |
| duck-parquet |  | 0.008582894000028318 | 214268 | 94.39147399996273 |
| duck-sorted |  | 0.009048746999951618 | 187428 | 31.398289999970075 |
| polars-ram |  | 0.4090379430000439 | 1242664 | 155.7959240000173 |
| polars-parquet |  | 0.0001675630001045647 | 460256 | 196.17668900002627 |

DuckDB + Parquet preparation: `{"duckdb_version": "1.5.5", "threads": 2, "import_tsv_s": 4.476173530999972, "point_index_build_s": 0.3820811219999882, "duckdb_bytes": 180105216, "parquet": {"canonical": {"seconds": 1.2408136220000188, "bytes": 23889697}, "sorted": {"seconds": 1.9501041959999839, "bytes": 11970915}}, "canonical_has_raw_ctf_provenance": false, "warning": "These derived analytical files omit raw CTF bytes. Add original CTF size before comparing durable footprints.", "memory": {"peak_rss_kib": 2045072, "rss_kib": 104424}}`

**Coverage:** all compared query first-page ID sequences and exact match counts are asserted against the Rust reference. Duplicate group and member counts are also asserted. This does not prove all possible expressions, payload types or all full result sequences. CTF external-application acceptance is not tested.

