# WP — reversible read-only annotation activation overlay (2026-10-30)

## Source and stack
- Repository: **i-shutov/whereisit-ai only**. Base PR #40 HEAD \`e661b482107393332ad663fa89ec6a9c868612c7\`; #40 depends on draft #39 → #38 → #37. Original main \`e8a16abd15fd4880ec2affa41b660a3b1f2aa5ba\`.
- This WP must remain a stacked draft against \`feat/ctf-annotation-confirmation-cas-20261030\`, not main. No parent branches are rebased or merged.
- Scope is **one reviewed annotation at a time**: activate a read-only projection of a prior user's confirmed note, inspect a verified overlay, and revoke it. No automatic binding or new-generation editing is enabled.

## Contract and durability
- \`ctf_annotations_activate_pinned\` requires the standard pinned CTF path, cache root, new generation SHA, existing external \`.wia.sqlite\`, \`expected_document_id\`, old/new entry IDs, old \`expected_revision\`, and \`activation:"user_approved_reversible_projection"\`.
- An IMMEDIATE transaction CAS-checks document and old annotation revision, the journal's selected entry ID, immutable old-note digest, and exact current target fingerprint. If the old source note changed (including deletion/recreation revision ABA), the activation fails. Competing/duplicate activations fail closed. No sidecar is created implicitly.
- Additive \`annotation_activations\` keeps **a complete per-annotation private snapshot** (old ID, metadata, note, tags, revision, digest, target generation and ID). Original \`annotation_entries\` and \`annotation_document.generation\` are never rewritten or deleted. This is a reversible overlay, not an in-place migration, nor an external OS backup of the whole SQLite file.
- \`ctf_annotations_activated_list_pinned\` reports **only projections whose original revision+digest and target fingerprint still match**. Stale activations are counted in \`conflicts\`, not returned as usable records. Unconfirmed/no-match records remain unprojected; old-generation LIST and new-generation SET retain the established conflict guard.
- \`ctf_annotations_deactivate_pinned\` requires exact document, old/new IDs, pinned new generation and \`rollback:"user_confirmed_revoke_projection"\`. It changes the journal row to \`revoked\` while retaining the entire private snapshot for audit; original notes were never touched. Replaying revoked activation is deliberately rejected (new explicit migration design required).
- CTF and derived SQLite are read-only; \`read_pinned\` validates actual source SHA around operations. An external write racing after commit can lead to a stale inactive generation-bound record and \`source_changed\`; it cannot silently alter the original source. The generation MUST be revalidated before reading projection.

## Synthetic QA
\`\`\`sh
cargo fmt --all --check
cargo test --locked -p atlas-service --test ctf_annotation_activation_contract
cargo test --locked -p atlas-service --test ctf_annotation_sidecar_contract
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
\`\`\`
CI: Linux Rust/fmt/Clippy, Windows x64 native installer, macOS ARM64/Intel native packages. Only synthetic CTF data in public CI; never publish real CTF/SQLite, local paths/names, private hashes, credentials or private note contents.

## Limitations and next WP
- This is **read-only activation**. Editing projected notes in the new generation and native GUI reconciliation dialogs require a separate review, as do whole-file backup/restore/upgrade, user-meaningful collision resolution, and performance testing with production-size data.
- Stable 1.0 is blocked by original WhereIsIt modified-CTF test, both historical private CTFs, physical GUI Win10/11 and macOS Intel/ARM tests, upgrade QA, Authenticode + Apple notarization and measured SLO. v1.0.0-rc.2 is unsigned prerelease. Do not merge or release without all gates.
