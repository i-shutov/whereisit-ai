# WP — Windows native annotation overlay approval and revocation (2026-10-31)

**Repository**: `i-shutov/whereisit-ai`; stacked on draft PR #47 at `b7223021399dcc0a2ddb286efe6194860d394096`. Do not rewrite, merge, or rebase draft parents #37–#47 without independent review. `main` was verified as `e8a16abd15fd4880ec2affa41b660a3b1f2aa5ba` at branch creation. This is **not** stable v1.0.

## Contract and Windows user flow

1. Link an external CTF **read-only** and choose an existing private `*.wia.sqlite` annotation sidecar. `CTF-Mirrors` and Parquet are disposable derivatives, never sources of user notes. The service captures and verifies an exact CTF generation SHA on every linked request, including the first and every subsequent 20-note review page.
2. In the Windows native TaskDialog, explicitly choose a saved old entry (no default radio choice). The page distinguishes **unconfirmed**, **confirmed but inactive**, and **active audit** rows, while the separate “Show all conflicts” view keeps stale, ambiguous and revoked records visible. A historical audit `active` flag is **not** proof of a currently valid projected note; only `ctf_annotations_activated_list_pinned` verifies that for consumers.
3. For an unconfirmed note, **Choose target…** still requires a second explicit radio choice of exact structural CTF candidate plus a separate Yes/No confirmation, default **No**. This saves only an *inactive* audit mapping. No automatic candidate selection or note migration.
4. For a fresh, confirmed but inactive note, **Activate confirmed mapping…** is a separate explicit choice plus a fresh Yes/No dialog defaulting **No**. Windows sends `ctf_annotations_activate_pinned` with document ID, old/new IDs, expected old-note revision and `activation=user_approved_reversible_projection`. Backend re-verifies source SHA, original annotation content and revision, exact structural candidate, and journal CAS before retaining an audit snapshot in the user-owned SQLite. It does **not** modify the original CTF or derivative cache.
5. For an active journal mapping, **Revoke active mapping…** requires another explicit choice plus a new Yes/No dialog defaulting **No**. Windows sends `ctf_annotations_deactivate_pinned` with document ID, exact active old/new IDs and `rollback=user_confirmed_revoke_projection`. Revocation remains possible even if the old note became stale, so an invalid active audit is not stranded. The original old note and full activation snapshot remain in the private journal; revoked pairs are not replayable.
6. Service success or conflict invalidates the user’s old selection and rereads a freshly generation-verified page. Changed source, mismatched document/revision/content/target, pending concurrent writers, duplicate/replayed operations and changed current generation all fail closed. No GUI-only state grants write permission.

## Automated regression and reproduction

Rust **1.90**; the public CI must use **synthetic fixtures only**:

```bash
cargo test --locked -p atlas-windows
cargo test --locked -p atlas-service --test ctf_annotation_activation_contract
cargo test --locked -p atlas-service --test ctf_annotation_native_pages_contract
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Windows `annotation_preview_tests::overlay_requires_explicit_confirmed_pair_and_fresh_note` exercises activate/revoke request construction, exact target, document/revision CAS, no implicit activation, stale/ambiguous/revoked rejection, and revocation of a stale audit. The synthetic service integration test additionally checks that an active but obsolete audit remains visible for explicit revocation while the independent verified reader refuses to project its stale note, and then checks exact revocation/rollback. The existing synthetic flow verifies unchanged external CTF and derivative mirror, absence of migration, and non-replayability.

**Gate**: exact-head GitHub Actions Linux workspace+fmt+Clippy, Windows x64 native build/Inno Setup install/uninstall smoke, macOS ARM64 and Intel native+PKG/DMG smoke. These are CI checks, not physical/manual GUI acceptance. Add the exact commit, run URL, PASS/FAIL, artifact provenance and any fixes to this PR handoff only after verified results.

## Required manual Windows QA — NOT yet PASS

On clean Windows 10 and Windows 11, with original WhereIsIt separately available for compatibility tests:

- Exercise 0, 1, 8, 20 and 21-note pages; use a changed CTF and a separately selected existing user annotation sidecar. Confirm there is **no default radio option** and every modal can be cancelled without writing.
- Approve one mapping; verify it remains inactive. Separately approve activation; check fresh verified overlay is shown only via `activated_list`. Separately revoke; verify disappearance from projection, original note/backup retained and replay denied.
- Change CTF bytes without relying on mtime/size, modify the old note between page and confirmation, concurrently edit the annotation store, use stale source generations, and revisit conflicts across pages; all must fail closed without changing CTF or derivative SQLite.
- Verify active-but-stale audit can be revoked; a revoked or stale mapping cannot be activated. Check install, uninstall, upgrade from unsigned rc.2 and recovery after restart, including file access and migrations. Run the same semantic/upgrade scenarios on macOS ARM64 and Intel using the separate macOS UI runbook.
- Keep private CTF/SQLite, local paths, filenames, source hashes, secrets and captured screenshots containing private data out of GitHub, public CI logs and artifacts. Record only redacted counts/performance aggregates in release QA.

## Blocking gates / next handoff

Stable 1.0 is **BLOCKED** until independent original-WhereIsIt modified-CTF compatibility (#2), both historical private CTFs (#19), manual native Win10/11 and macOS Intel/ARM GUI/upgrade, Authenticode/Apple notarization, measured target SLO and review of issues #2–#8/#23. The opaque `0x10` field does **not** establish the alleged 1,782 descriptions. `v1.0.0-rc.2` remains an unsigned prerelease; never publish a stable release or merge unreviewed stacked PRs.

**Next bounded WP after this PR's exact-head CI is green:** verified read-only native overlay presentation and explicit conflict counters (not only historical activation audit), followed by physical acceptance. All work is in this repository only.
