# Independent export validation

Checked: 2026-10-28T11:35:10.998500+00:00.

This audit independently read the completed CSV exports with Python’s standard `csv` reader and the Parquet exports with **DuckDB 1.4.1**. It did not call the export service again, query or modify the database, or alter the exported files.

| Export case | Rows | Columns | All values equal | Cyrillic name/path rows | NULL dates |
|---|---:|---:|---|---:|---:|
| music_sample | 603 | 15 | Yes | 419 | 603 |
| small_disk | 11 | 15 | Yes | 0 | 11 |
| music_duplicates | 32,889 | 15 | Yes | 21,327 | 32,889 |

Every CSV field matched the corresponding Parquet value after normalizing Parquet NULL to an empty CSV field. Column names and order matched all 15 schema columns. Row counts also agreed with the completed service results. Names and paths containing Cyrillic text survived exactly; no private filename, path or disk label is reproduced in this report.

All integer columns, including `size`, are Parquet BIGINT (signed 64-bit). Python received integer objects, and exact decimal CSV size values matched them without floating-point conversion. Aggregate and maximum sizes below describe these test selections, not the entire catalog.

| Export case | Sum of size bytes | Maximum size bytes | Rows above 4 GiB | Local date strings |
|---|---:|---:|---:|---:|
| music_sample | 101,176,664,775 | 1,496,320,749 | 0 | 603 |
| small_disk | 232,766 | 81,920 | 0 | 10 |
| music_duplicates | 282,084,823,890 | 254,089,215 | 0 | 32,889 |

## SQLite to CTF roundtrip

The CTF exported from the imported SQLite database was independently hashed and compared with the supplied original. Both contain **144,766,574 bytes** and have SHA-256:

`REDACTED_SHA256`

The independent length and SHA-256 checks matched. This validates the unchanged source catalog’s SQLite → CTF preservation, in addition to the reader/writer’s direct roundtrip tests.

## Scope

These checks validate exported values, integer representation, nullable fields, Cyrillic text, and preservation of the unchanged supplied catalog. They do not prove arbitrary new CTF acceptance by the original WhereIsIt application. Original-application acceptance remains separately tracked by the VM validation task. Native category/tag mappings and unsupported CTF variants retain their documented limitations.

## Additional real files above 4 GiB

An explicitly requested additional pair of exports selected real catalogued movie files larger than 4 GiB from one volume. The independent Python CSV and DuckDB 1.4.1 audit verified **321 rows, all 15 columns, and every field value**. All 321 sizes exceed 4 GiB. Parquet represents them as BIGINT; Python receives exact integer values, matching the CSV decimals without float conversion.

| Metric | Verified value |
|---|---:|
| Minimum file size, bytes | 4,316,109,809 |
| Maximum file size, bytes | 40,898,551,720 |
| Sum of file sizes, bytes | 2,920,310,820,302 |
| Rows with Cyrillic names/paths | 320 |
| NULL Unix dates / matching blank CSV fields | 321 |
| Local DOS date strings | 321 |

This supplements the initial three export selections, which contained no file above 4 GiB. It closes that specific coverage gap without changing the database or rerunning the earlier benchmark suite.
