# Indexed RAM and full-Entry SQLite A/B

Synthetic-only evidence. Six warm samples after a first run; no cold-cache or p95 claim. RAM computes the complete result bitmap, exact count, and first 250 IDs. SQLite computes a hydrated page of 250 complete Entry values with a continuation probe. These are deliberately distinct workloads; the SQL A/B variants themselves have identical full Entry response shape.

## stress-unique-3m — 3,046,027 records

Names/folder paths/descriptions distinct: 3,046,027 / 234,388 / 28,114.
Canonical ingestion/interning: 6.152 s. Index construction: 6.842 s. Integer-key duplicate grouping: 0.039 s.
RSS after build / peak (KiB): {'peak_rss_kib': 1144132, 'rss_kib': 1144132}. This process loads canonical data, not the raw CTF; startup is NOT directly comparable to the direct-CTF arena test.

| Case | RAM full set + count + page, ms | SQL original full page, ms | SQL index only, ms | SQL index + fixed predicates, ms |
|---|---:|---:|---:|---:|
| Q0: ext:mkv AND size:>4GiB | 1.190 | - | - | - |
| Q1: name:Запись-0000200 | 0.603 | - | - | - |
| Q2: path:Медиатека AND ext:flac | 28.537 | - | - | - |
| Q3: description:редкое | 1.027 | - | - | - |
| Q4: NOT description:редкое AND ext:mp3 | 0.980 | - | - | - |
| Q5: category:featured AND ext:mkv | 0.053 | - | - | - |
| Q6: музыка OR name:0000042 | 1.311 | - | - | - |
| Q7: name:zzzzzznomatch | 0.054 | - | - | - |
| Q8: name:з | 211.702 | - | - | - |
| Q9: path:album-0077 AND ext:flac | 1.403 | - | - | - |
| Q10: name:*0000042*.mkv | 0.295 | - | - | - |
| Q11: (ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB | 2.092 | - | - | - |
| Q12: size:>9000000000B | 0.074 | - | - | - |

Native SQL full pages, including all fields, are checked against the released core. RAM first-page IDs and exact counts are checked against the reference; the report also records a SHA-256 digest of all ordered matching IDs for future exhaustive parity. Fixed-predicate category optimization assumes the known synthetic category assignment; it is not a claim that arbitrary categories have been solved.

## stress-unique-payload — 100,000 records

Names/folder paths/descriptions distinct: 100,000 / 7,744 / 18,451.
Canonical ingestion/interning: 0.404 s. Index construction: 1.632 s. Integer-key duplicate grouping: 0.001 s.
RSS after build / peak (KiB): {'peak_rss_kib': 179676, 'rss_kib': 179676}. This process loads canonical data, not the raw CTF; startup is NOT directly comparable to the direct-CTF arena test.

| Case | RAM full set + count + page, ms | SQL original full page, ms | SQL index only, ms | SQL index + fixed predicates, ms |
|---|---:|---:|---:|---:|
| Q0: ext:mkv AND size:>4GiB | 0.027 | - | - | - |
| Q1: name:Запись-0000200 | 0.011 | - | - | - |
| Q2: path:Медиатека AND ext:flac | 0.430 | - | - | - |
| Q3: description:редкое | 0.570 | - | - | - |
| Q4: NOT description:редкое AND ext:mp3 | 0.576 | - | - | - |
| Q5: category:featured AND ext:mkv | 0.002 | - | - | - |
| Q6: музыка OR name:0000042 | 0.597 | - | - | - |
| Q7: name:zzzzzznomatch | 0.002 | - | - | - |
| Q8: name:з | 2.562 | - | - | - |
| Q9: path:album-0077 AND ext:flac | 0.070 | - | - | - |
| Q10: name:*0000042*.mkv | 0.024 | - | - | - |
| Q11: (ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB | 0.192 | - | - | - |
| Q12: size:>9000000000B | 0.002 | - | - | - |

Native SQL full pages, including all fields, are checked against the released core. RAM first-page IDs and exact counts are checked against the reference; the report also records a SHA-256 digest of all ordered matching IDs for future exhaustive parity. Fixed-predicate category optimization assumes the known synthetic category assignment; it is not a claim that arbitrary categories have been solved.

