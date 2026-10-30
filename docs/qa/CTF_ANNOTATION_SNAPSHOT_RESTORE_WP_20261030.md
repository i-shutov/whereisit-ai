# WP — verified private annotation sidecar snapshot and restore (2026-10-30)

## Scope / ancestry

Repo **i-shutov/whereisit-ai** only. Parent [draft PR #41](https://github.com/i-shutov/whereisit-ai/pull/41) exact SHA `8e1e267111c6fa4fa79ababcbf62ae48e0eda62b`, stacked on #40 → #39 → #38 → #37. Original main `e8a16abd15fd4880ec2affa41b660a3b1f2aa5ba`. This WP does **not** merge those drafts and does **not** modify any original CTF or derived mirror.

## Commands

- `ctf_annotations_backup_pinned`: usual local `path` (CTF), `cache_root`, pinned `generation`, `annotation_db` (existing user store), `expected_document_id` (CAS from a previous annotation list), `backup_path` (**new** file ending `.wia.backup.sqlite`), `backup:"user_approved_new_snapshot"`. Produces an independent complete consistent SQLite snapshot using SQLite `VACUUM INTO`, with **all** private tables including existing confirmations, activations and future audit tables. Validates database application ID, user schema version, identity, complete table inventory, SQLite integrity, foreign keys; returns local `backup_sha256` and original-generation metadata. File is atomically published without overwriting any existing path. Keep digest and file private, do not paste either into GitHub issues/CI.
- `ctf_annotations_restore_pinned`: same pinned CTF context, `annotation_db` (**new** output filename ending `.wia.sqlite`), `backup_path` existing local `.wia.backup.sqlite`, `expected_document_id`, `expected_backup_sha256` from a previous backup, `restore:"user_approved_new_sidecar"`. Verifies source and staged snapshot, SHA and structural/semantic integrity; atomically publishes **only** a new user-owned sidecar. Rejects existing files, CAS mismatch, bad SQLite/foreign keys and changed CTF generation. Does not auto-rebind notes from an old generation. If `requires_explicit_reconciliation:true`, use the existing preview/confirm/activate workflow.

Both commands use the same `ctf_mirror::read_pinned` source SHA checks *before and after preparing* the snapshot. Copy/publish only follows a successful pinned validation. SQLite source stays read-only during `VACUUM INTO`, and derived caches are never destinations. No private paths, names, note text or source hashes are emitted to GitHub.

**Storage safety:** snapshot file limit 8 GiB. The destination directory must exist and be writable. A same-filesystem hard link from a private staging directory publishes atomically and rejects replacement of an existing destination; unsupported filesystems fail rather than falling back to an unsafe overwrite. On Unix the snapshot file is mode 0600. Windows ACLs remain the responsibility of the chosen private destination directory. A snapshot is whole-file recovery, not a merge, and it does not automatically delete a stale source store. A crash or failed `fsync` can leave a newly published destination needing inspection. Do not use backup SHA values as proof of the source CTF's identity.

## Synthetic verification

```sh
cargo fmt --all --check
cargo test --locked -p atlas-service --test ctf_annotation_snapshot_contract
cargo test --locked -p atlas-service --test ctf_annotation_activation_contract
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Tests cover complete sidecar copying including an extra audit table; unchanged CTF/mirror/live-store bytes; restore after live-note edit; same-generation and conflicted-generation behavior; no overwrite; document CAS; tampered snapshot; absent annotations; and cache-path exclusion. Do not inject real user data into public CI.

## Remaining gates / handoff

Next bounded WP: **compact native Windows/macOS conflict-review dialog**, using preview, explicit confirm, activate and revoke, and exposing separate private backup/restore actions rather than implicitly migrating notes. Review UI must never auto-accept single-candidate metadata matches. Stable 1.0 still blocked on original WhereIsIt modified CTF, two historical private CTFs, physical Win10/11 and Mac ARM/Intel GUI + upgrade QA, Authenticode / Developer ID notarization, and production SLO. Opaque CTF flag 0x10 remains unspecified, not proven to mean 1,782 lost descriptions. Latest release v1.0.0-rc.2 is unsigned prerelease.

