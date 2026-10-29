# Indexed RAM and full-Entry SQLite A/B

Synthetic-only evidence. Six warm samples after a first run; no cold-cache or p95 claim. RAM computes the complete result bitmap, exact count, and first 250 IDs. SQLite computes a hydrated page of 250 complete Entry values with a continuation probe. These are deliberately distinct workloads; the SQL A/B variants themselves have identical full Entry response shape.

## indexed-payload-heavy — 100,000 records

Names/folder paths/descriptions distinct: 98,022 / 46 / 1.
Canonical ingestion/interning: 0.334 s. Index construction: 0.105 s. Integer-key duplicate grouping: 0.001 s.
RSS after build / peak (KiB): {'peak_rss_kib': 30412, 'rss_kib': 30412}. This process loads canonical data, not the raw CTF; startup is NOT directly comparable to the direct-CTF arena test.

| Case | RAM full set + count + page, ms | SQL original full page, ms | SQL index only, ms | SQL index + fixed predicates, ms |
|---|---:|---:|---:|---:|
| Q0: ext:mkv AND size:>4GiB | 0.030 | 2.598 | 0.719 | 0.653 |
| Q1: name:Запись-0000200 | 0.010 | 2.454 | 2.438 | - |
| Q2: path:Медиатека AND ext:flac | 0.167 | 246.106 | 246.327 | 1.229 |
| Q3: description:редкое | 0.029 | 482.544 | 480.708 | 478.264 |
| Q4: NOT description:редкое AND ext:mp3 | 0.029 | 175.565 | 6.675 | 454.537 |
| Q5: category:featured AND ext:mkv | 0.002 | 24.845 | 25.160 | 0.758 |
| Q6: музыка OR name:0000042 | 0.035 | 569.728 | 567.888 | 466.340 |
| Q7: name:zzzzzznomatch | 0.002 | 0.542 | 0.548 | - |
| Q8: name:з | 1.428 | 6.673 | 7.158 | - |
| Q9: path:album-0077 AND ext:flac | 0.004 | 5.307 | 5.393 | - |
| Q10: name:*0000042*.mkv | 0.020 | 2.108 | 2.167 | - |
| Q11: (ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB | 0.074 | 22.965 | 23.710 | 4.044 |
| Q12: size:>9000000000B | 0.003 | 0.141 | 0.155 | 0.084 |

Native SQL full pages, including all fields, are checked against the released core. RAM first-page IDs and exact counts are checked against the reference; the report also records a SHA-256 digest of all ordered matching IDs for future exhaustive parity. Fixed-predicate category optimization assumes the known synthetic category assignment; it is not a claim that arbitrary categories have been solved.

## indexed-repeated-3m — 3,046,027 records

Names/folder paths/descriptions distinct: 35,300 / 687 / 1.
Canonical ingestion/interning: 1.618 s. Index construction: 0.817 s. Integer-key duplicate grouping: 0.095 s.
RSS after build / peak (KiB): {'peak_rss_kib': 196652, 'rss_kib': 195840}. This process loads canonical data, not the raw CTF; startup is NOT directly comparable to the direct-CTF arena test.

| Case | RAM full set + count + page, ms | SQL original full page, ms | SQL index only, ms | SQL index + fixed predicates, ms |
|---|---:|---:|---:|---:|
| Q0: ext:mkv AND size:>4GiB | 1.375 | 10.916 | 0.461 | 0.381 |
| Q1: name:Запись-0000200 | 0.136 | 45.457 | 45.950 | - |
| Q2: path:Медиатека AND ext:flac | 2.791 | 2273.590 | 2280.757 | 0.540 |
| Q3: description:редкое | 0.081 | 73.127 | 73.283 | 62.296 |
| Q4: NOT description:редкое AND ext:mp3 | 0.109 | 579.072 | 7.615 | 17.819 |
| Q5: category:featured AND ext:mkv | 0.057 | 621.866 | 621.056 | 0.746 |
| Q6: музыка OR name:0000042 | 0.152 | 141.741 | 141.370 | 62.321 |
| Q7: name:zzzzzznomatch | 0.131 | 0.556 | 0.600 | - |
| Q8: name:з | 6.057 | 151.915 | 150.613 | - |
| Q9: path:album-0077 AND ext:flac | 0.095 | 57.515 | 57.433 | - |
| Q10: name:*0000042*.mkv | 0.146 | 18.621 | 18.495 | - |
| Q11: (ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB | 1.462 | 406.126 | 402.389 | 11.660 |
| Q12: size:>9000000000B | 0.131 | 1.454 | 1.476 | 1.269 |

Native SQL full pages, including all fields, are checked against the released core. RAM first-page IDs and exact counts are checked against the reference; the report also records a SHA-256 digest of all ordered matching IDs for future exhaustive parity. Fixed-predicate category optimization assumes the known synthetic category assignment; it is not a claim that arbitrary categories have been solved.

## indexed-unique-deep — 100,000 records

Names/folder paths/descriptions distinct: 98,022 / 46 / 1.
Canonical ingestion/interning: 0.172 s. Index construction: 0.089 s. Integer-key duplicate grouping: 0.001 s.
RSS after build / peak (KiB): {'peak_rss_kib': 30132, 'rss_kib': 30132}. This process loads canonical data, not the raw CTF; startup is NOT directly comparable to the direct-CTF arena test.

| Case | RAM full set + count + page, ms | SQL original full page, ms | SQL index only, ms | SQL index + fixed predicates, ms |
|---|---:|---:|---:|---:|
| Q0: ext:mkv AND size:>4GiB | 0.029 | 1.304 | 0.546 | 0.476 |
| Q1: name:Запись-0000200 | 0.009 | 2.130 | 2.095 | - |
| Q2: path:Медиатека AND ext:flac | 0.174 | 243.450 | 243.803 | 1.002 |
| Q3: description:редкое | 0.003 | 3.251 | 3.271 | 2.513 |
| Q4: NOT description:редкое AND ext:mp3 | 0.005 | 30.309 | 1.205 | 1.067 |
| Q5: category:featured AND ext:mkv | 0.003 | 22.661 | 22.815 | 0.523 |
| Q6: музыка OR name:0000042 | 0.030 | 10.430 | 10.334 | 82.396 |
| Q7: name:zzzzzznomatch | 0.003 | 0.359 | 0.368 | - |
| Q8: name:з | 1.396 | 5.844 | 5.897 | - |
| Q9: path:album-0077 AND ext:flac | 0.006 | 5.030 | 5.035 | - |
| Q10: name:*0000042*.mkv | 0.026 | 1.781 | 1.784 | - |
| Q11: (ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB | 0.068 | 21.817 | 21.784 | 3.293 |
| Q12: size:>9000000000B | 0.003 | 0.132 | 0.131 | 0.069 |

Native SQL full pages, including all fields, are checked against the released core. RAM first-page IDs and exact counts are checked against the reference; the report also records a SHA-256 digest of all ordered matching IDs for future exhaustive parity. Fixed-predicate category optimization assumes the known synthetic category assignment; it is not a claim that arbitrary categories have been solved.

