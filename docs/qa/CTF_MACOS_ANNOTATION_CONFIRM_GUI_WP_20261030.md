# WP — macOS explicit annotation candidate confirmation (2026-10-30)

**Stack:** draft PR #44 at exact parent `d303afd94bd6434b8242109df969426a1c78a46e`; this WP targets that branch and does not modify earlier PRs, main or releases.

## Vertical

Linked external CTF remains the read-only authority. In the macOS native **Review notes…** sheet, select an unconfirmed old row, explicitly select one of at most eight exact *structural* candidate entry IDs (even if only one candidate exists), then approve a separate **Confirm mapping (do not activate notes)** dialog. No default target is chosen. No-match rows and already-confirmed/stale confirmations are not silently resolved. Ambiguous matches remain visible, including truncation. Pagination still reads each 20-row page with fresh source-generation verification.

The native call is `ctf_annotations_confirm_pinned`, with pinned `path/cache_root/generation`, local user-owned `annotation_db`, exact `expected_document_id`, `old_entry_id`, `new_entry_id`, `expected_revision`, and explicit `confirmation=user_selected_exact_candidate`. The service revalidates original CTF SHA, immutable derived cache, metadata fingerprint and old revision; a one-to-one, immutable audit record is stored only in the distinct user sidecar. No user annotation is moved, activated, overwritten or associated automatically. After success **or CAS conflict**, selection is discarded and the page is reloaded; the user must select again.

## Synthetic regression and CI

`cargo test --locked -p atlas-service --test ctf_annotation_native_pages_contract` includes old-document, old-revision, invalid candidate, missing-consent, duplicate confirmation, success-without-activation and source-generation drift. It also proves original CTF and derived cache bytes unchanged. All tests use synthetic CTFs; never upload private CTFs, sidecars or their file paths/hashes. `cargo fmt --all --check`, `cargo test --locked --workspace`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, plus native Windows x64/macOS ARM64/Intel compile/installer smoke are required. Inspect exact-head Actions before claiming PASS.

## Remaining work and manual gates

Windows confirmation selector is **not** part of this bounded macOS WP; the existing Windows preview remains intentionally read-only. Follow with a compact Windows native confirmation dialog, then separate explicit activation/revocation and backup/restore GUI controls. Verify the macOS sheet manually on Intel and ARM64 with privately selected synthetic notes, including cancel, candidate choice, change of source during review, concurrent sidecar edit, paging and stale confirmation, and compare on-disk snapshots. CI compilation never substitutes for human GUI QA. Stable 1.0 remains blocked by original WhereIsIt modified-CTF verification, two historical CTFs, Win10/11 and Mac manual GUI/upgrade acceptance, Authenticode/Apple notarization and performance SLO. The opaque 0x10 extension is not proven to be lost descriptions. v1.0.0-rc.2 remains unsigned prerelease.
