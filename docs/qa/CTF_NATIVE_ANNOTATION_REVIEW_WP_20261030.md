# WP — Native read-only annotation conflict preview (2026-10-30)

**Source branch:** stacked on draft PR #42 at \`6f00b247d6f523c652553d0e88ef43da535821c3\`. **Scope:** one bounded native read-only preview vertical for Windows x64 and macOS ARM64/Intel.

## Contract

1. Link a **local external CTF** using the existing native action (read-only; pinned generation). Native **Review notes…** is enabled only while linked and idle.
2. Choose an **existing** private \`*.wia.sqlite\` user sidecar via the OS open dialog. The UI never scans for, creates or uploads it. The Rust service rejects CTF/mirror paths and aliases.
3. Call \`ctf_annotations_preview_pinned\` with \`path\`, \`cache_root\`, \`generation\`, \`annotation_db\`, \`offset:0\`, \`limit:20\`. Service verifies source SHA and mirror before/after access; any generation drift fails closed.
4. Show explicit \`current_generation\`, \`generation_conflict\`, \`uninitialized\` states, structural candidates, ambiguity, truncated candidate counts and stale confirmation state. A *single* candidate is **not** an accepted mapping. This WP **never** calls set/confirm/activate/deactivate/restore/backup.
5. Windows: bounded read-only MessageBox review summary. macOS: compact native sheet with 20 bounded rows. Both intentionally display only the **first** page and state this limitation; future WP adds pagination and explicit user confirmation with CAS and verified re-fetch. No background upload or log of private notes/CTF, paths or digests.

## QA (synthetic only)

Run \`cargo fmt --all --check\`, \`cargo test --locked --workspace\`, \`cargo clippy --locked --workspace --all-targets -- -D warnings\`; native CI must pass Windows x64, macOS ARM64 + Intel and Linux. Windows unit tests exercise ambiguous/truncated/stale and current-generation text. Existing \`ctf_annotation_sidecar_contract\` synthetic Rust integrations cover read-only preview semantics, pinned CTF generation and conflicts. Human GUI testing is **not** inferred from CI. Confirm with a privately selected synthetic sidecar on both native platforms; check link switching during pending response, open dialog cancellation, 20-row cap and no on-disk modifications.

## Release gates

This is **not** 1.0 acceptance. The original WhereIsIt modified CTF round-trip, both historical private CTFs, Win10/11 + Intel/ARM GUI and upgrade QA, signing/notarization, and production SLO still require evidence. Do not assume 0x10 opaque extensions mean 1,782 lost descriptions.
