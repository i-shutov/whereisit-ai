# PR73 / PR78 review follow-up

2026-11-21. Incremental code-merge preparation; this is not stable-release approval.

The GitHub review/inline-thread lists were empty at this checkpoint. Historical PR73 audit comments were checked against the source rather than treated as new unresolved defects. Duplicate category IDs/names, recursive hierarchy, stale successful-mutation results, Back high-offset recovery, malformed replies, retry, and atomic membership operations already have source fixes and synthetic tests.

Independent follow-up found and repaired:

| PR | Finding | Repair and regression |
|---|---|---|
| 73 | Parent existence check could race a concurrent delete; SQLite could reuse the deleted highest ID and create a self-parent. | One immediate transaction covers parent lookup and insertion. A deterministic two-connection WAL/busy-handler handshake reproduced old `Ok(1)` and now gets `not_found` with no inserted row. Pure nonpositive-ID validation runs before acquiring the write lock; a held-writer regression preserves `invalid_request` for 0 and -1. |
| 73 | Assign/Remove `not_found` after external entry deletion left stale selection and ready assignment buttons. | Clear the deleted selection/memberships and reload the current query at the existing offset/history. Both mutation purposes and unrelated-error behavior have an actual egui reply-state regression. |
| 78 | Strict dynamic table-length validation rejected catalogs produced by the old exporter with a fixed 184 declaration. | Accept only the exact historical packed marker `0x040000b8` in addition to the correct dynamic size; derive all reads and bounds from actual count. Synthetic legacy import rewrites a corrected declaration, malformed sentinels and unrelated mismatches still fail. |

Pre-publication results: 33 core tests, 37 egui tests, and 96 CTF/service tests passed. Each new regression failed against its old production path. Scoped formatting and strict Clippy passed. Only generated test data was used; no private catalogs, paths, hashes, screenshots or logs are included.

Merge order is PR73 into main first, then retarget PR78 from the PR73 feature branch to main and verify its remaining diff/checks. PR78's final integration head must include the reviewed PR73 head and current main. Exact-head hosted CI and final installed-GUI observations are recorded in the PR descriptions after measurement. No merge or stable tag is performed by this checkpoint.

Existing product-level gates remain visible in the acceptance documentation: full PR58 feature parity, authenticated signing/notarization, physical USB identity and broader DPI/platform QA. These incremental fixes do not retire legacy clients or set `stable_ready=true`.
