# Windows 1.1 stable acceptance — 23 November 2026

Status: IN_PROGRESS. This pre-build record does not claim an unobserved exact-1.1.0 GUI PASS.

## Scope

Windows x64, Rust/egui, unsigned installer. Tabs/workspaces (#87), archive slices/sunburst (#88) and explicit metadata-only local analyzer (#89) are integrated. macOS/Linux products remain deferred. USB pairing/bound-device rescan stays disabled; no file deletion or cleanup commands. The owner requested completion through stable publication after acceptance.

## Already observed

The 1.1.0-rc.1 exact-main source b5e36674a901d05d32f3e74f0e80e84c8725befd passed 14 checks / 6 workflows. Its installed product was upgraded from the published 1.0.0 in actual Windows 11 Administrator RDP GUI. Restart, uninstall, published-1.0 rollback and final RC reinstall passed. Two external synthetic SQLite files and a separate user-store were byte-identical, with quick_check=ok; Unicode notes/tags, category assignments and disk preferences remained readable. Search (466 matches), retained context, 60 disks, slices/sunburst navigation, explicit local analysis and one-row review CSV were observed. The detailed sanitized post-build record is PR #90.

## Remaining exact stable gates

- Measure search on the complete private derived catalog: six fixed classes, 30 warm samples/class, p50/p95 and process memory/I/O; isolate measurements from compilation/GUI automation. Search p95 target is 150 ms. Do not call uncontrolled cache state a cold-cache test.
- Measure bounded analytics and cancel/retry on the complete catalog, preserve source SQLite, and report background latency/memory separately from search.
- Verify supported CTF import/export and unchanged roundtrip on private copies, independent full CSV/Parquet 15-field equality, changed-description export opened in original WhereIsIt, and vendor-saved copy imported by the native parser. No raw data, names, private digests, queries, paths or screenshots are public.
- Validate the successful exact-main 1.1.0 product artifact and installed executable hashes. Actually operate the final installer and installed GUI, upgrade from published 1.0 with external metadata preserved, and check restart.
- Record PASS/FAIL and remaining P0/P1 before readiness/publication. Preserve v1.0.0 and RC assets; publication is an explicit final action, never a main-push side effect.

Only synthetic regressions and sanitized aggregates enter GitHub. The original CTF is never an output. Preparation keeps stable_ready=false. Post-build evidence identifies the tested source/artifact explicitly; later documentation/readiness changes do not retroactively certify a different installer.

## Full-catalog RC checkpoint and release fix

A dedicated private SQLite copy contains 44 volumes / 3,046,133 entries. RC search: six classes with 30 warm samples each, maximum p95 50.875 ms and process peak working set 9,424,896 bytes. These are RC observations; repeat the search on the exact stable payload. Cache state is uncontrolled, not a cold-cache result.

Bounded analytics (128 nodes/depth 4), seven fresh worker processes, matched independent SQL: 2,805,176 files, 239,207 ordinary directories and 62,440,113,273,118 logical file bytes. Archive-container rows are separate from the ordinary-directory counter and their aggregate sizes are not added again. Observed p50 28,389 ms / nearest-rank p95 36,599 ms, peak working set 29,835,264 bytes; source SQLite hash was unchanged across this API measurement. This preliminary RC workload is background aggregation, not the search SLO or final stable GUI acceptance.

Actual RC GUI on this complete copy: cancel showed an explicit cancelled state; retry allowed switching to Search, a new Boolean query and page 251–500 while analysis continued. The full chart eventually rendered correct complete totals and an explicit omitted-node/byte notice. P1 found: its native request asked for 5,000 nodes, producing a several-minute first chart on this workload. The release fix bounds the initial native request to 128 nodes while preserving full totals, explicit truncation and branch navigation. A native-request-to-real-service regression with 300 generated files checks the bounded page, full byte/file totals and exact remainder accounting. Fixed exact-main GUI retest is required before closing this P1.
