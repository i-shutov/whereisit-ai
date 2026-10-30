# WP — generation-pinned linked duplicate-candidate export (2026-10-30)

## Baseline and scope

Based on `main` `e8a16abd15fd4880ec2affa41b660a3b1f2aa5ba`.
Branch `feat/ctf-pinned-duplicate-export-20261030` retains the
shared grouping-key refactor from the prior branch checkpoint
(`83e80791736f8e0e1bb19c0a7ec43a3f90ec171e`).

The service exports all filtered **metadata duplicate candidates** to a new
CSV, Parquet or JSON output while preserving the original CTF and read-only
SQLite mirror. Native Windows and macOS Export use the same explicit
`mode:"duplicates"` when Duplicates view is selected.

- `ctf_duplicates::grouping` defines one set of grouping semantics for
  pages and full exports. Window count happens **after** query/volume
  filtering, before `duplicate_count > 1`; folders are excluded.
- `exports::export_pinned_selected` uses the existing streaming exporter
  without the editable catalog's `duplicates::export_snapshot`, creating
  or writing any `duplicate_cache_*` tables, or importing a user source.
- `ctf_mirror::read_pinned` checks exact source SHA and immutable SQLite
  stamp at both ends. `ctf_export::export` stages then publishes with
  `persist_noclobber`; failed work removes staging.
- `verified:false` means **candidate content identity is not proven**,
  even when `source_sha_checked:true`. `duplicate_mode:"hash"` remains
  unsupported. Explicit page/subtree scope is rejected for exports.

## Reproducible tests and CI

```sh
cargo fmt --all --check
cargo test --locked -p atlas-service --test ctf_pinned_duplicates_contract
cargo test --locked -p atlas-service --test ctf_linked_export_contract
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

`.github/workflows/build.yml` tests Linux workspace, Windows x64 installer
smoke, macOS ARM64/Intel app, PKG and DMG smoke. Public fixtures are **only
synthetic**: two disks, mixed file sizes, name normalization, scope filters,
page/full-report parity, same-size source replacement, no-clobber and
physical CSV/JSON/Parquet row counts. No private CTF/SQLite/user paths,
names, hashes, media or secrets are uploaded.

## Pending release gates and next WP

This feature is not authorization for release 1.0. Privately test two
historical CTF samples and a modified CTF in original WhereIsIt; reconcile
durable native annotations outside disposable caches with explicit orphan
and ambiguity conflicts; perform manual Win10/11 and Mac Intel/ARM GUI/DPI
and upgrades; sign with Authenticode and Apple Developer ID notarization;
and measure interactive SLOs. The opaque 0x10 extension is **not** assumed
to represent 1,782 lost text descriptions without documentation.
`v1.0.0-rc.2` remains an unsigned prerelease.

**Next technical WP:** durable annotation sidecar, document identity and
conflict-visible reconciliation after external CTF generation changes.
