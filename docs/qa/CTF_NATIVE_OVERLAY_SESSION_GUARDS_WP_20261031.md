# Native verified-overlay page/session guards — bounded WP (2026-10-31)

## Dependency and safety

This WP stacks **only** on draft PR #49, exact parent `69a2d7713ea6b09f58f889f291d601b09f8abde0`. The upstream #37–#49 draft chain is not rebased, merged or rewritten; `main` at branching was `e8a16abd15fd4880ec2affa41b660a3b1f2aa5ba`. Earlier PR #34 (linked read-only CSV/Parquet/JSON export) is already merged.

External CTF remains authoritative and **read-only**. Every backend `ctf_annotations_activated_list_pinned` call rechecks the source generation SHA. SQLite/Parquet mirrors are disposable; private `*.wia.sqlite` annotations are stored separately. Neither a historical `state=active` audit nor an unconfirmed candidate is a verified note. No private source bytes, paths, names, SQLite files, source hashes or credentials belong in the public repo/CI.

## Fixed failure modes

* **Page replay / mismatched offset:** Both native clients require a nonempty verified page to echo the *requested* `offset` and `limit=20` before any private note text is presented. Incorrect/missing coordinates fail closed, including a response from a previous page. Valid empty `uninitialized` / `no_cross_generation_activations` states display no note text.
* **Windows cross-review late response:** Captured annotation session epoch accompanies async replies. Choosing a sidecar, relinking, resetting the catalog, or closing the review invalidates pending annotation response handlers. Late preview, confirmation, activation/revocation or verified-page UI responses cannot render in the next review or start further UI actions; normal non-annotation operations retain their original behavior. This does **not** undo an explicit write already dispatched/authorized.
* **macOS duplicate page / late offset:** Reject a second page request while one is loading, bind the async response to its requested offset and existing review session; close/relink invalidates that session. On error or source drift, no notes are displayed.
* **CTF changed between page views:** The synthetic backend integration test changes the external CTF while a verified overlay exists, asserts `source_changed` and unchanged user sidecar + derived mirror, then restores the source and verifies a fresh pinned response. After explicit revocation, verified results must contain no projected rows.

## Reproduction and regression

Use Rust 1.90 and the exact HEAD of this draft PR:

```sh
cargo test --locked -p atlas-service --test ctf_annotation_activation_contract
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

CI must separately pass Linux test/fmt/Clippy, Windows x64 Rust/native build and unsigned installer smoke, macOS ARM64 and Intel SwiftUI + PKG/DMG smoke. Only generated synthetic CTF test data are permitted in public jobs.

Manual acceptance still required on Win10/11 and macOS ARM64/Intel: open one user sidecar, navigate 20-row verified pages, change the external source while the dialog is open, retry and verify no old text appears; close/reopen with a different sidecar; explicitly activate then verify, revoke then verify; cancel modal and ensure no silent migration. For Windows, inspect the interim status after a late reply; for macOS, check Back/Close and disabled navigation during load. Repeat clean install and upgrade from rc.2. CI cannot establish these physical GUI claims.

## Release gates

This WP is a defensive contract, **not** 1.0 acceptance. Original WhereIsIt modified-CTF interchange (#2), both historical private CTFs (#19), manual UI/upgrade on all targets (#4–#6), real 3-million-entry performance/storage SLO (#3/#8), Authenticode and Apple Developer ID/notarization (#7) remain BLOCKED. Opaque flag `0x10` (#23) has no verified decoding semantics and must not be equated with 1,782 lost descriptions. `v1.0.0-rc.2` is unsigned prerelease. Do not merge these drafts or publish stable without independent gate evidence.

Next WP: manual GUI/upgrade harness and reproducible local-only private-CTF acceptance runner; collect **redacted** measurements and pass/fail per real gate, never upload the raw sources.
