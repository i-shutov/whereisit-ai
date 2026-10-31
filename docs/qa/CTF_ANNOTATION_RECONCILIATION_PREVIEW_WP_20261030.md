# WP — read-only generation reconciliation preview (2026-10-30)

## Exact dependency snapshot

- Main: e8a16abd15fd4880ec2affa41b660a3b1f2aa5ba.
- Parent draft PR #38 HEAD: 3172ddeabdc1ca71025a075f4606e4292b194793 (separate user-owned annotations); stacked over draft PR #37 HEAD 9ceb308cede82c91acc31792d3416396ec07d870.
- New branch: feat/ctf-annotation-reconciliation-preview-20261030, PR base must be the #38 branch. Never merge or retarget before prerequisites are reviewed.
- This change is a bounded **read-only service preview**, not an annotation migration or desktop dialog.

## Behavior and safety contract

- New JSON op: ctf_annotations_preview_pinned using path, cache_root, generation, annotation_db, bounded offset/limit.
- Requires pinned, fully SHA-verified source generation, immutable SQLite mirror, and user-owned .wia.sqlite outside cache. Reads only; uninitialized preview does not create sidecars.
- For different generations, pages old annotations (50 max) by old entry ID and compares *exact structural fingerprint* (disk label, relative path, kind, size) against current read-only core. Zero, one and multiple candidates remain explicit; every result requires confirmation, including a unique structural match. No opaque 0x10 interpretation or content-hash claim.
- At most eight candidates are returned per old annotation. Additional candidates produce candidates_truncated=true and candidate_count_exact=false, never an invented exact count. Old revision and note/tags are accessible only in local responses for informed review.
- Same generation returns current_generation; absent sidecar returns uninitialized; modified original before resync returns source_changed. Preview never changes generation, mapping, revision, annotations, CTF or cache. LIST and SET fail-closed behavior is unchanged.
- Private CTF/SQLite, user paths, names, hashes, secrets and local responses must never appear in public CI artifacts or PR comments. Synthetic fixtures only. This operation may be CPU-intensive on large zero-byte entries; no SLO without measurements.

## Reproduce

~~~sh
cargo fmt --all --check
cargo test --locked -p atlas-service --test ctf_annotation_sidecar_contract
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
~~~

The synthetic regression creates old keep/dupe/gone and externally modified keep/two dupe/new catalogs; checks 0/1/2 candidates, unique-match unconfirmed status, pagination, absent store, stale generation rejection, LIST conflict after resync, and byte preservation of user-owned sidecar, original CTF and derived mirror. Verify exact HEAD Linux/Windows x64/macOS ARM64/Intel 4/4 and record actual CI results, not hypothetical PASS.

## Next WP / remaining 1.0 gates

Next: **explicit, auditable confirmation protocol** with old revision CAS, destination generation binding, unresolved conflicts retained, then Win32/SwiftUI review/edit UI. Never mutate original CTF or disposable mirror and preserve user note backups.

1.0 remains **blocked** by original-WhereIsIt modified CTF, two private historical CTFs, manual GUI Win10/11 and Mac Intel/ARM with upgrade/DPI QA, Authenticode/Apple notarization and measured SLOs. v1.0.0-rc.2 is unsigned prerelease, not 1.0. No stable release or unreviewed merge.
