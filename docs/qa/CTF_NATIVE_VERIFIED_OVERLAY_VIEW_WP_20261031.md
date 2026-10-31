# Verified live annotation overlays — native Windows/macOS (2026-10-31)

## Scope and invariants

This bounded WP stacks on draft PR #48 (exact parent `450840d4be4e15a32afd00489d359b65907da3c1`). It **does not merge or rewrite** #37–#48. `main` at branch creation: `e8a16abd15fd4880ec2affa41b660a3b1f2aa5ba`.

A historical `annotation_activations.state='active'` record is *not* proof that a note is safe to show at a different entry. Both native viewers read **only** `ctf_annotations_activated_list_pinned` (never `ctf_annotations_preview_pinned` for valid live-note data). This backend verifies the external CTF generation SHA, current original note revision and payload, confirmed structural target and activation journal, **on each independently requested 20-record page**. The native clients additionally fail closed if any returned item is missing the explicit `read_only=true` / `activation=explicit_reviewed_projection` contract.

The viewer displays the backend's independently verified `total` live notes and `conflicts` rejected/stale active audits as **different counters**. Unconfirmed/revoked mappings are not projected and are **not counted** in the `conflicts` counter; the UI states that explicitly. Page navigation, open, cancel and close never write. Separate confirmation, activation and revocation flows from parent PRs remain unchanged. The source CTF remains an external read-only authority; mirror SQLite/Parquet are disposable; private `*.wia.sqlite` is user-owned and independent. Never substitute historical journal contents for a live projection.

Windows: `Review notes…` → select local user sidecar → `Verified overlays (read-only)…`; bounded native TaskDialog, no default selection, previous/next page requires fresh backend read. macOS: `Review notes and overlays…` → same local sidecar → `Verified live notes…`; explicit Back rereads the conflict page with a fresh SHA pin. Private note excerpts are shown **locally in the GUI only**; never log or upload their content.

## Reproducible synthetic CI

Rust 1.90, checkout exact branch HEAD:

```sh
cargo test --locked -p atlas-service --test ctf_annotation_activation_contract
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Require GitHub Actions **on the exact final HEAD**, all four jobs: Linux workspace/fmt/Clippy, Windows x64 native+unsigned installer smoke, macOS ARM64 and Intel native SwiftUI+PKG/DMG smoke. Unit regressions cover missing unsafe verification flags, journal-only records, stale count, bounded pages, no default consent. Synthetic service integration verifies repeated pinned overlay reads are byte-identical for CTF, derived cache and private user SQLite. Build/install smoke does **not** constitute a manual GUI or physical real-CTF acceptance. Never mark a pending CI as PASS; record final run, SHA and artifact URLs in the PR HANDOFF.

## Manual QA required before stable 1.0

1. On each Win10/11 and macOS ARM64/Intel machine, start with a private **local** source CTF, derived mirror and separate sidecar. In the original generation record two notes; change only a *copy* of external CTF, re-sync the mirror and explicitly confirm+activate one mapping. Verify one live note appears, unconfirmed note stays hidden and counters differentiate them.
2. Edit the original note (return to its original generation on a separate local fixture), then re-open the changed CTF. Its active audit must be reported in **stale/conflict count** and the note must not appear. Verify audit revocation is still possible.
3. After every page, externally change the source CTF (including same-size/same-mtime), verify the next backend call fails with generation drift and no private notes are shown or silently rebound.
4. Verify no writes to the source CTF, the derived cache, or sidecar while merely opening, paging or closing. Only explicit consent flows can change the *user* sidecar.
5. Compare original-WhereIsIt modified-CTF interoperability, both historical private CTF files, real SLO/3M-entry performance, clean upgrades/rollback, Authenticode and Apple Developer ID/notarization separately. These remain **BLOCKED** pending physical evidence.

## Privacy and acceptance

Public repository and CI use synthetic data only: **no user CTF, SQLite, private paths, filenames, source SHA, original descriptions, secrets or CTF-derived blobs**. All logs/PR comments contain only synthetic test results and public code HEAD. Opaque flag `0x10` has no verified meaning; do not claim 1,782 lost description texts.

This PR is draft and may not be merged until the full stack is independently reviewed. `v1.0.0-rc.2` is **unsigned prerelease**, not stable 1.0.
