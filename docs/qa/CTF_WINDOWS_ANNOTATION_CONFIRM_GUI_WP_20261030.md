# WP — Windows native explicit annotation candidate confirmation (2026-10-30)

**Scope:** Windows x64 native client only, stacked on draft PR #45 (exact parent `2c8e44f7b84af5612d2ae1061a60b43821f62e0d`). Earlier draft PRs #37–#45 are unchanged. This is NOT a stable 1.0 release and must not be merged into main ahead of its reviewed dependencies.

## User-visible sequence

1. Link an existing external CTF; select an existing local user-owned `*.wia.sqlite` via the OS file picker. External CTF is always **read-only**; `CTF-Mirrors` is a disposable derivative, never a user annotation source.
2. Native Windows TaskDialog shows a **generation-pinned** 20-row page. Native radio controls present *only eligible* old annotation IDs, including candidate state/count; unmatched, stale-revision/payload and previously confirmed rows are not offered for approval. The **Show all conflicts** action retains a bounded read-only summary including blocked IDs. Navigation reissues `ctf_annotations_preview_pinned`; no previously viewed page implies a live generation.
3. The user explicitly selects an old row and separately chooses one of the returned (at most eight) structurally matching candidate IDs. Radio defaults are disabled in **both** dialogs, even when there is a single candidate. Candidate summary contains only structural fields available from the verified read-only service snapshot (entry, volume, parent, modified time); no unsupported filename or content identity is invented. A second **Yes/No** OS confirmation defaults to **No**.
4. Only after both choices and the final consent is `ctf_annotations_confirm_pinned` sent with exact `expected_document_id`, `expected_revision`, old/new entry IDs, `confirmation=user_selected_exact_candidate` and the source generation captured by the linked CTF session. The service rechecks source SHA, structural fingerprint, document ID and revision CAS. Replayed, stale, ambiguous and changed-source requests fail closed.
5. The successful operation writes an **inactive audit mapping only** in the separate user-owned SQLite; it does not activate or migrate annotations and never writes the external CTF or derivative cache. Success and error both re-request the preview page; stale selections cannot survive a reload.

## Reproduction

Rust 1.90, from repository root:

```shell
cargo test --locked -p atlas-windows
cargo test --locked -p atlas-service --test ctf_annotation_native_pages_contract
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

The Windows-only `annotation_preview_tests::no_default_or_stale_candidate_is_ever_implicitly_selected` covers selection of distinct candidates, rejection of out-of-range indexes, stale confirmation/revision, already confirmed rows, absent document ID and matching-generation state. The existing synthetic service integration contract tests missing user consent, stale document/revision, invalid candidate, replay, unchanged source/cache and SHA generation drift. CI is required on **the exact PR head** for Linux, Windows x64, macOS ARM64/Intel, including installers. Windows TaskDialog runtime behavior is NOT established by compilation alone.

## Manual GUI checks still required

On clean Windows 10 and 11, open the native chooser with 0, 1, 8, 20 and 21 old notes. Check cancel at every stage, no radio preselection, exact candidate selection, duplicate confirmation, unrelated source changes while the modal is open, simultaneous notes writers and stale conflicts. Run as an installed app and after upgrading the existing unsigned prerelease. Ensure no private path/CTF/hash appears in public CI/logs; private fixtures stay on local machines. The GUI's structural metadata does not prove semantic or byte identity; a future reviewed UX may need richer disambiguation.

## Blocking stable-1.0 gates

Original-WhereIsIt modified-CTF check (#2), both historical private CTFs (#19), manual Win10/11 and macOS Intel/ARM GUI + upgrade QA, Authenticode and Apple notarization, and real measured performance SLO remain **blocked**. Opaque extension `0x10` (#23) is not proved to encode the alleged 1,782 descriptions. `v1.0.0-rc.2` remains an **unsigned prerelease**.

**Next separate WP:** explicit, independently user-approved activation and revocation controls from native Windows/macOS annotation review, including CAS, pending-state UX and conflict diagnostics. Do not auto-activate on confirmation.

No merged PRs, no stable release, no user CTF/SQLite, private paths, private source hashes or secrets may be published.
