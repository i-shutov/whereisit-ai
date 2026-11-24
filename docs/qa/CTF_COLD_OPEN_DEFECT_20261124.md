# P1-CTF-COLD-OPEN — 24 November 2026

Status: **OPEN**. Windows 1.1.0 owner reports that opening a roughly 140 MB CTF on Windows 10 is still processing records after 15 minutes. This is a product usability defect. The owner is not required to recreate another QA machine.

The previous acceptance incorrectly treated successful complete import and warm SQLite search speed as sufficient first-open evidence. Earlier full-import observations took approximately 8–10 minutes. They should have triggered this defect before publication. Existing roundtrip/data-integrity results do not establish acceptable cold-open performance.

## Confirmed implementation and first repair

Native Open CTF calls `ctf_sync` and waits for a fully imported, indexed, validated, immutable SQLite generation before displaying the catalog. Parsing and insertion happen outside the UI thread, but the catalog remains unavailable throughout that work. A progress counter alone does not make it usable.

Fresh import already deferred FTS insert triggers, but still updated six secondary B-tree indexes for every entry. The repair defers these non-unique secondary indexes inside the same fresh-v3 import transaction and reconstructs their saved definitions before commit. Primary keys, foreign keys and parent validation stay enabled. Populated catalogs keep normal incremental index maintenance. Errors and panics roll back both data and schema.

Eight synthetic core regressions pass, covering search parity, descriptions and later edits, missing-trigger refusal, populated-catalog fallback, successful index restoration, error rollback, panic rollback and refusal to suspend a replaced unique index.

## Measurements so far

Local release-mode service benchmark, generated 300,001-entry CTF, empty derivative cache each time, same Windows 11 host, uncontrolled OS cache:

| Implementation | Ready plus first volume-list response |
|---|---:|
| Released implementation | 23,388 ms |
| Deferred secondary indexes | 11,076 ms |

This is a single sample per implementation, not p50/p95, Windows 10 verification, physical cold-cache evidence or GUI acceptance. It demonstrates an improvement, not closure of the defect. The manual ignored benchmark exercises `ctf_sync` and pinned browsing with temporary derivatives; its optional local-copy mode emits only phase/count/timing aggregates and verifies source bytes are unchanged.

A full-catalog service run after that repair processed 44 volumes / 3,046,133 entries: readiness 196,250 ms; first volume-list response 196,415 ms. Peak working set observed during polling was 730,959,872 bytes (sampled, not a complete process-lifetime capture). Source byte identity passed. Record insertion finished between 60 and 65 seconds; the indexing phase continued until approximately 155 seconds; validation completed around 195 seconds. These phase boundaries have 5-second sampling precision and do not isolate individual secondary indexes from FTS. This remains unacceptably slow for first opening and does **not** close the defect.

The owner confirms original WhereIsIt opens this catalog in about one second. The owner explicitly rejects waiting approximately 190 seconds for search even if browsing becomes available sooner. Both browsing and search must operate on the read-only metadata snapshot without waiting for complete SQLite conversion. A faster complete conversion and a browse-only preview are intermediate work, not closure. See the [Everything architecture study](../architecture/EVERYTHING_CTF_SEARCH_REVIEW_20261124.md) for documented principles and the proposed shared snapshot/query boundary.

The direct-preview service path now fully parses/validates the CTF, retains its parsed metadata once, and materializes only requested directory-page rows. It uses a bounded current snapshot with a unique per-open generation, source-hash checks, stable eventual-import entry IDs, and no writable source handle. Native integration runs the derived index builder independently from foreground browsing and defers promotion until an active page request finishes.

A release-mode run on the same full working copy measured **1,442 ms to browsable state / 1,787 ms through the first nonempty directory page**. Source byte identity passed, 44 volumes / 3,046,133 entries were reported, and no SQLite cache/index was created by opening. This is one service sample with uncontrolled OS cache; it excludes application startup/rendering and is not a Windows 10 or installed-GUI PASS. An earlier preliminary run measured 1,478 / 1,821 ms.

Four synthetic preview regressions pass for Unicode/order/path and import-ID parity, source-change rejection, invalid-parse isolation, exact-generation close, and same-path/same-bytes reopen isolation. GUI verification and final candidate CI remain pending. Do not publish or mark stable-ready on the strength of service measurements. The next acceptance must measure user action through the first usable directory page and search, separately from subsequent warm queries, and record peak memory, cancellation and source integrity. Background search preparation must not occupy the foreground browsing request slot or enable mutation of the source catalog.

No private paths, source hashes, names, raw rows, catalogs or screenshots belong in this report or public CI. The published 1.1.0 tag/assets remain unchanged.
