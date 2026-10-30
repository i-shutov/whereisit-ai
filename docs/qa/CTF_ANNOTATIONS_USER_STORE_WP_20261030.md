# WP — separate generation-bound native annotation store (2026-10-30)

## Dependency and scope

- Base: main at e8a16abd15fd4880ec2affa41b660a3b1f2aa5ba.
- Stacked on open, draft PR #37 (linked CTF metadata duplicates full export), exact base 9ceb308cede82c91acc31792d3416396ec07d870.
- New branch: feat/ctf-user-annotation-sidecar-20261030. PR must target the #37 branch until #37 merges; do not duplicate its pending work in main or force-push unrelated branches.
- This is one bounded service/ABI persistence WP, not manual GUI acceptance or an annotation UI.

## Functional vertical

- ctf_annotations_set_pinned and ctf_annotations_list_pinned are reachable from the common JSON service used by Rust/Win32/macOS clients.
- User explicitly selects a file ending in .wia.sqlite *outside* cache_root. Sidecar is independently durable and user-owned; cached SQLite generation remains read-only and fully disposable. No CTF mutation, no sidecar in CTF/Parquet export, no annotations in FTS/cache.
- SQLite application ID, schema version, random opaque document ID and bound generation prevent accidental reading of foreign SQLite or treating content-identical CTF copies as the same annotation document.
- SET saves note/tags scoped to an exact pinned entry and generation with optimistic revision/CAS, persists across connections, and deletes upon two empty fields. LIST is read-only and never creates a sidecar.
- Every operation passes through full source SHA generation verification before/after via ctf_mirror::read_pinned. Same-size external changes fail as source_changed. After explicit ctf_sync of a different generation, LIST returns generation_conflict with no projected entries; SET returns annotation_generation_conflict. **No old entry IDs or same-name/size guesses are silently rebound.**
- Synthetic only: integration tests verify durability, revision conflict, external same-size generation changes, unchanged original and mirror bytes, cache-root destination exclusion and foreign SQLite refusal.

## Reproduction

~~~sh
cargo fmt --all --check
cargo test --locked -p atlas-service --test ctf_annotation_sidecar_contract
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
~~~

Cross-platform acceptance requires exact HEAD of the stacked PR: Linux Rust/fmt/Clippy, Windows x64 native installer smoke, macOS ARM64 and Intel app/PKG/DMG smoke. CI run URLs and per-job PASS/FAIL must be written in PR comments **after they actually complete**. If a check fails, fix that exact failure in this branch with a negative regression test. No disabled tests, waivers or synthetic PASS claims.

## Privacy and operational boundaries

No private CTF/SQLite files, user paths, names, source SHA values, screenshots or secrets enter public PR/CI. Synthetic fixture values are invented. Annotation sidecar itself is *user data*, not a cache; it requires independent backup. Sidecar creation/modification must not follow source/mirror targets or simple symlink aliases. Post-read SHA checks are optimistic checks, not file-system locking against an uncooperative simultaneous original-WhereIsIt save; a note written during an external race remains bound to its old generation even if the API returns source_changed. Do not assume source 0x10 opaque extensions are missing text.

## Next bounded WP

1. Explicit **manual reconciliation preview** for a newly synced generation, using structural fingerprints only to generate candidate sets with zero/one/multiple matches. Treat even a unique candidate as unconfirmed until the user accepts a mapping; leave orphan/ambiguous conflicts visible.
2. After that, add native Win32 + SwiftUI editing and conflict dialogs; no silent re-assignment. Independently measure performance SLOs.

## 1.0 release gates still blocked

Original WhereIsIt modified CTF acceptance; two private historical CTFs; GUI Win10/11 and Mac Intel/ARM, DPI/upgrade/manual QA; Authenticode and Apple notarization; target latency/memory SLO evidence. v1.0.0-rc.2 is unsigned prerelease, NOT a stable 1.0. No merge or stable release authorized by this WP.
