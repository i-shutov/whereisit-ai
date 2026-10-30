# WP — explicit CAS-guarded annotation candidate confirmations (2026-10-30)

## Exact parent and implementation

- Original `main`: `e8a16abd15fd4880ec2affa41b660a3b1f2aa5ba`.
- Draft stack: #37 `9ceb308cede82c91acc31792d3416396ec07d870` → #38 `3172ddeabdc1ca71025a075f4606e4292b194793` → #39 `535d0ba221cf787a473986d1cd8e2a89c7a7fbbe`.
- Work branch: `feat/ctf-annotation-confirmation-cas-20261030`, based on **exact #39 HEAD**. PR must target the #39 branch until parents are reviewed and merged. Do not rebase/force-push competing branches.
- Scope: explicit service-level user selection and an *audit journal*, **not activation, copy, GUI dialogs, CTF export or automatic migration**.

## Contract

`ctf_annotations_confirm_pinned` requires original read-only `path`, derived `cache_root`, current hash-verified `generation`, existing external user-owned `annotation_db`, `expected_document_id` from preview, `old_entry_id`, `new_entry_id`, positive `expected_revision`, and an explicit `confirmation: "user_selected_exact_candidate"`.

The request is accepted only when the old record still exists at that revision in the exact document, the old and new generations differ, and the selected target exists in the pinned *new* catalog with **the same exact disk label, relative path, kind, and size**. This is only a structural candidate, **not content identity**; the user must inspect it. No fuzzy fallback, ID reuse, implicit accept on unique matches, missing-store creation or old-name reassociation.

An IMMEDIATE SQLite transaction records the one-to-one choice in the separate `.wia.sqlite` sidecar's additive `annotation_confirmations` table. One old annotation → one new ID per generation, and one new ID ← one old annotation per generation. Duplicate confirmation, stale revision/document, wrong candidate and target collision fail closed. Existing annotation rows, their notes, tags and revisions, and the `annotation_document`'s source generation are **never changed**. Preview returns `confirmed_selection` only on confirmed rows; unconfirmed and no-match records remain visible. Ordinary LIST continues returning `generation_conflict` with empty items; SET continues refusing new-generation modifications. A later external CTF edit invalidates its generation pin. The journal remains local, durable and inactive.

The pinned source read is SHA-checked before and after the operation. Because external programs are not locked, an external save racing with the post-write SHA check may lead to a recorded but **inactive generation-bound confirmation** and a `source_changed` response. Treat such cases as a retry/review, never as an active mapping. This WP does not claim atomicity with an external WhereIsIt writer.

## Synthetic regression / verification

- `cargo fmt --all --check`
- `cargo test --locked -p atlas-service --test ctf_annotation_sidecar_contract`
- `cargo test --locked --workspace`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`

Synthetic-only integration tests cover explicit confirmation token, lost-update CAS, document CAS, exact candidate matching, idempotency conflict, competing old annotations claiming one new entry, no-store creation, audit-row persistence, unresolved conflict visibility, lack of automatic migration, untouched original CTF and SQLite derived mirror, and source generation change.

CI matrix gate: Linux Rust workspace/fmt/Clippy, Windows x64 native installer, macOS ARM64/Intel package smoke. **Do not mark PASS until the exact HEAD checks finish.** Do not publish private CTF/SQLite, source file paths/names, user-source SHA, or credentials.

## Next bounded WP

Implement explicit *activation* of reviewed mappings (with full immutable old-note backup, current-generation CAS, collision handling and lossless reverse audit) and native Windows/macOS conflict review UI. This journal alone does not expose notes as current-generation annotations.

Remaining 1.0 gates: real original-WhereIsIt modified-CTF roundtrip, two historical private CTFs, human GUI/DPI/upgrade QA on Win10/11 and macOS ARM64/Intel, Authenticode and Apple signing/notarization, and measured SLOs. `v1.0.0-rc.2` remains **unsigned prerelease**, not stable 1.0. No automatic merge or release.
