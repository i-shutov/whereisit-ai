# WP — Paged read-only native annotation reconciliation (2026-10-30)

**Base:** draft PR #43 at `a8e9df3bdf8a55a202532eb29a15eb8bc9e51586` (stack #42 → #41 → #40 → #39 → #38 → #37). **Changes:** Windows and macOS native clients, one synthetic cross-platform service regression. Do not merge this stacked PR directly into `main` before parents are reviewed.

## User contract

1. Link an **external local CTF** in read-only mode and choose an **existing** `*.wia.sqlite` private annotation sidecar through the OS open dialog. Never enumerate private user stores or upload bytes/filenames/digests.
2. Each request invokes `ctf_annotations_preview_pinned` with **the original authoritative CTF**, its SHA-bound generation, separate derived cache root, chosen user sidecar, `offset`, and `limit=20`. The native clients **re-request** each page; neither CTF nor the user sidecar is mutated.
3. Every cross-generation preview page shows a bounded slice and total. Navigation is allowed only when results indicate a previous/next page; the server caps requests at 50. A new source SHA (including same-size/mtime mutations) **fails closed**, never silently rebinds notes.
4. macOS: compact modal sheet with Previous/Next, loading state, Close and session token invalidation on relink/close. Windows: local modal summary with YES=next, NO=previous, CANCEL=close; unavailable actions do not load pages. Both show ambiguity, candidate truncation, stale confirmations, and state distinctions.
5. A unique structural candidate is **not approved**. This WP implements **no confirmation, activation, revocation, recovery or backup writes**. Notes and all audit tables remain in the separate user-owned SQLite. The CTF is always authoritative; the mirror is disposable.

## QA and regression

Run with Rust 1.90: `cargo test --locked --workspace`, `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`. The Windows native unit tests cover 1st/middle/last/empty pagination and stale/ambiguous warning text. The new **synthetic-only** `ctf_annotation_native_pages_contract` exercises 24 saved notes, two full/partial preview pages, disjoint IDs, beyond-end offset, repeated deterministic pagination, unchanged CTF/mirror/user sidecar, and source generation drift between pages. Use CI Windows x64, Linux and macOS ARM64/Intel native build/install smoke for exact HEAD.

Manual native QA remains outstanding: open/close during page request, 0/1/20/21/40/41 rows, source update between pages, original external CTF unchanged, and no private path/hash in logs. CI installer smoke **does not** count as physical GUI acceptance.

## Stable 1.0 gates

No stable 1.0 claim until modified CTF is verified in the **original WhereIsIt**, both historical user CTFs are accepted locally, manual Win10/11 + Mac Intel/ARM GUI and installed upgrade QA pass, Authenticode + Apple signing/notarization are valid and target real-data SLO met. Treat opaque extension 0x10 as unknown semantics, not proof of 1,782 lost descriptions. `v1.0.0-rc.2` remains unsigned prerelease.

## Next WP

Explicit *user-selected* exact candidate confirmation from native preview with refreshed document/revision/generation CAS and clear handling for stale/replayed conflicts. Then separately expose activate/revoke and backup/restore controls only after focused review.
