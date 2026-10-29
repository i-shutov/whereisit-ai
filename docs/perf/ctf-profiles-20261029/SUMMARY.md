# CTF RAM / SQLite / columnar measured study

All data below are synthetic. No personal catalog was uploaded. Times are milliseconds; medians of six warm repetitions after one first run. First run is not a cold-OS-cache measurement. The released application returns hydrated Entry objects; the other comparisons return ordered IDs only.

## 100,000 entries

Synthetic CTF: 72736655 bytes. Unchanged roundtrip equal: True.
CTF read / parse / arena construction / ordered indexes: 0.059 / 0.156 / 0.424 / 0.090 seconds.
Hot row size: 40 bytes; major arena capacities: 11617216 bytes, excluding raw CTF and associative containers. Process high-water RSS is separately recorded and includes parsing/serialization transient allocations.

| Case | RAM scan | RAM candidate lists | app | sqlite-direct | sqlite-tuned | sqlite-ram-tuned | duck-file | duck-ram | duck-parquet | duck-sorted | polars-ram | polars-parquet |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Q0: ext:mkv AND size:>4GiB | 0.010 | 0.002 | 2.161 | 0.853 | 0.089 | 0.089 | 1.995 | 2.701 | 6.408 | 5.672 | 1.227 | 4.221 |
| Q1: name:Запись-0000200 | 4.427 | 4.447 | 2.509 | 22.819 | 23.013 | 22.603 | 4.717 | 2.999 | 7.272 | 7.009 | 1.880 | 6.058 |
| Q2: path:Медиатека AND ext:flac | 0.051 | 0.022 | 251.575 | 8.123 | 0.788 | 0.796 | 11.873 | 5.167 | 20.156 | 26.386 | 3.505 | 20.037 |
| Q3: description:редкое | 0.062 | 0.008 | 468.579 | 390.564 | 394.559 | 360.918 | 1.830 | 2.759 | 5.092 | 5.235 | 1.613 | 3.265 |
| Q4: NOT description:редкое AND ext:mp3 | 0.026 | 0.011 | 164.036 | 372.283 | 372.254 | 348.390 | 2.063 | 3.248 | 5.489 | 6.009 | 1.716 | 3.628 |
| Q5: category:featured AND ext:mkv | 0.018 | 0.003 | 23.536 | 0.777 | 0.140 | 0.135 | 0.806 | 1.007 | 4.868 | 4.899 | 0.813 | 2.823 |
| Q6: музыка OR name:0000042 | 0.831 | 0.832 | 563.656 | 386.936 | 386.754 | 364.016 | 76.646 | 73.103 | 105.983 | 102.612 | 5.460 | 25.124 |
| Q7: name:zzzzzznomatch | 7.361 | 7.332 | 0.614 | 22.881 | 22.791 | 23.413 | 3.219 | 1.559 | 5.474 | 5.190 | 1.258 | 4.763 |
| Q8: name:з | 0.024 | 0.034 | 5.893 | 0.185 | 0.181 | 0.174 | 6.843 | 4.994 | 9.051 | 6.577 | 2.488 | 6.897 |
| Q9: path:album-0077 AND ext:flac | 2.179 | 1.640 | 5.632 | 298.588 | 83.142 | 84.057 | 11.526 | 5.770 | 20.274 | 27.227 | 3.381 | 15.463 |
| Q10: name:*0000042*.mkv | 3.306 | 0.809 | 2.194 | 23.600 | 23.224 | 25.581 | 4.354 | 2.668 | 6.545 | 6.385 | 1.591 | 5.288 |
| Q11: (ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB | 0.079 | 0.078 | 22.620 | 2.791 | 2.795 | 2.918 | 13.480 | 7.371 | 23.364 | 30.305 | 3.951 | 20.227 |
| Q12: size:>9000000000B | 0.447 | 0.000 | 0.171 | 17.946 | 19.384 | 19.483 | 0.751 | 0.599 | 4.599 | 4.732 | 0.564 | 3.683 |

### Storage / startup / duplicate grouping

| Engine | File bytes | Load/copy seconds | Peak RSS KiB | Duplicate build ms |
|---|---:|---:|---:|---:|
| Rust arena process | original CTF retained | 0.729 | 306632 | 4.909 |
| app | 237166592 | 15.651325073 | 231200 |  |
| sqlite-direct |  | None | 91448 | 29.594532999993817 |
| sqlite-tuned |  | None | 96820 | 29.392111999982262 |
| sqlite-ram-tuned |  | 0.21146396599999662 | 276004 | 30.47919499999807 |
| duck-file |  | 0.010116202999995494 | 112500 | 14.737626999988152 |
| duck-ram |  | 0.3374946320000163 | 447300 | 11.892863000014131 |
| duck-parquet |  | 0.009217493999983617 | 194392 | 16.20912199999225 |
| duck-sorted |  | 0.009202315999999655 | 163784 | 15.885395999987395 |
| polars-ram |  | 0.08192983899999717 | 234128 | 11.439494000001105 |
| polars-parquet |  | 0.00016622100000063256 | 197864 | 13.62758999999869 |

DuckDB + Parquet preparation: `{"duckdb_version": "1.5.5", "threads": 2, "import_tsv_s": 0.6155710010000064, "point_index_build_s": 0.383415398000011, "duckdb_bytes": 17575936, "parquet": {"canonical": {"seconds": 0.20066172500000334, "bytes": 1281264}, "sorted": {"seconds": 0.2860207520000131, "bytes": 1408043}}, "canonical_has_raw_ctf_provenance": false, "warning": "These derived analytical files omit raw CTF bytes. Add original CTF size before comparing durable footprints.", "memory": {"peak_rss_kib": 814984, "rss_kib": 88580}}`

**Coverage:** all compared query first-page ID sequences and exact match counts are asserted against the Rust reference. Duplicate group and member counts are also asserted. This does not prove all possible expressions, payload types or all full result sequences. CTF external-application acceptance is not tested.

## 100,000 entries

Synthetic CTF: 4884931 bytes. Unchanged roundtrip equal: True.
CTF read / parse / arena construction / ordered indexes: 0.004 / 0.024 / 0.030 / 0.090 seconds.
Hot row size: 40 bytes; major arena capacities: 11490240 bytes, excluding raw CTF and associative containers. Process high-water RSS is separately recorded and includes parsing/serialization transient allocations.

| Case | RAM scan | RAM candidate lists | app | sqlite-direct | sqlite-tuned | sqlite-ram-tuned | duck-file | duck-ram | duck-parquet | duck-sorted | polars-ram | polars-parquet |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Q0: ext:mkv AND size:>4GiB | 0.010 | 0.002 | 1.255 | 0.838 | 0.089 | 0.086 | 2.010 | 2.736 | 6.525 | 5.741 | 1.157 | 4.600 |
| Q1: name:Запись-0000200 | 4.428 | 4.427 | 2.103 | 22.466 | 22.971 | 23.527 | 4.747 | 3.038 | 7.262 | 7.027 | 1.902 | 6.343 |
| Q2: path:Медиатека AND ext:flac | 0.040 | 0.021 | 250.019 | 7.868 | 0.776 | 0.786 | 12.085 | 5.031 | 26.037 | 26.761 | 4.401 | 19.882 |
| Q3: description:редкое | 0.421 | 0.007 | 3.300 | 1.499 | 1.488 | 1.518 | 1.276 | 2.120 | 4.301 | 4.153 | 1.210 | 2.879 |
| Q4: NOT description:редкое AND ext:mp3 | 0.015 | 0.008 | 30.461 | 2.475 | 1.152 | 1.187 | 2.199 | 3.899 | 6.109 | 5.964 | 1.391 | 3.410 |
| Q5: category:featured AND ext:mkv | 0.016 | 0.003 | 23.101 | 0.784 | 0.145 | 0.137 | 0.830 | 1.091 | 4.879 | 5.127 | 0.828 | 3.075 |
| Q6: музыка OR name:0000042 | 6.262 | 6.237 | 10.676 | 98.737 | 97.962 | 99.045 | 76.583 | 69.988 | 105.665 | 105.081 | 4.969 | 24.476 |
| Q7: name:zzzzzznomatch | 7.347 | 7.366 | 0.370 | 22.968 | 23.055 | 23.110 | 3.215 | 1.552 | 5.519 | 5.196 | 1.261 | 4.754 |
| Q8: name:з | 0.017 | 0.017 | 5.720 | 0.185 | 0.178 | 0.171 | 6.766 | 4.993 | 9.033 | 6.651 | 2.515 | 6.822 |
| Q9: path:album-0077 AND ext:flac | 2.180 | 1.698 | 5.207 | 294.794 | 81.375 | 83.029 | 11.847 | 5.586 | 21.112 | 27.889 | 2.030 | 15.624 |
| Q10: name:*0000042*.mkv | 3.228 | 0.794 | 1.798 | 22.518 | 23.486 | 25.379 | 4.327 | 2.712 | 6.636 | 6.326 | 1.553 | 5.444 |
| Q11: (ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB | 0.080 | 0.075 | 21.644 | 2.769 | 2.792 | 2.754 | 14.220 | 7.508 | 26.873 | 22.219 | 3.627 | 20.405 |
| Q12: size:>9000000000B | 0.414 | 0.000 | 0.133 | 19.486 | 20.062 | 19.300 | 0.760 | 0.607 | 4.694 | 4.898 | 0.556 | 3.711 |

### Storage / startup / duplicate grouping

| Engine | File bytes | Load/copy seconds | Peak RSS KiB | Duplicate build ms |
|---|---:|---:|---:|---:|
| Rust arena process | original CTF retained | 0.147 | 40292 | 4.988 |
| app | 79523840 | 10.238898283 | 32532 |  |
| sqlite-direct |  | None | 79896 | 29.403974000018707 |
| sqlite-tuned |  | None | 89264 | 29.41957299998421 |
| sqlite-ram-tuned |  | 0.07359212900001921 | 111124 | 29.705998999986605 |
| duck-file |  | 0.010089611000012155 | 113180 | 13.971470000001318 |
| duck-ram |  | 0.26092329400000835 | 309468 | 21.224197000009326 |
| duck-parquet |  | 0.009298160000014377 | 226168 | 16.615900999994437 |
| duck-sorted |  | 0.009594745000015337 | 201308 | 14.539904000002934 |
| polars-ram |  | 0.08189468300000158 | 233768 | 10.75739199998793 |
| polars-parquet |  | 0.00016805399999952897 | 197176 | 11.688754999994444 |

DuckDB + Parquet preparation: `{"duckdb_version": "1.5.5", "threads": 2, "import_tsv_s": 0.3663891199999938, "point_index_build_s": 0.2890465199999994, "duckdb_bytes": 17313792, "parquet": {"canonical": {"seconds": 0.15768271700000014, "bytes": 1280666}, "sorted": {"seconds": 0.1977152320000073, "bytes": 1406128}}, "canonical_has_raw_ctf_provenance": false, "warning": "These derived analytical files omit raw CTF bytes. Add original CTF size before comparing durable footprints.", "memory": {"peak_rss_kib": 612344, "rss_kib": 89444}}`

**Coverage:** all compared query first-page ID sequences and exact match counts are asserted against the Rust reference. Duplicate group and member counts are also asserted. This does not prove all possible expressions, payload types or all full result sequences. CTF external-application acceptance is not tested.

