# WP — macOS explicit annotation overlay activation and revocation (2026-10-30)

**Repository:** `i-shutov/whereisit-ai`, stacked draft on PR #46 at `a8b9bd868feaecd7a537a3fcaa639f5579a3fca3`. All earlier PRs #37–#46 remain unchanged and unmerged. This is **not** a stable release.

## Architectural contract

- External CTF is the authoritative **read-only** source, with a SHA-256 generation check on every service call, including the preview page. The separate derivative SQLite/Parquet cache is **disposable**, not a user annotation database.
- The user explicitly chooses an old note and a structural candidate to create an *inactive* confirmation. **Only a separate subsequent action** may activate that confirmed exact old/new mapping.
- The macOS review sheet exposes **Activate…** for a fresh confirmation with matching revision, payload, target and no activation history. Consent is a separate native confirmation. Backend `ctf_annotations_activate_pinned` revalidates generation, document, original note revision and digest, confirmation and target fingerprint; a snapshot is stored only in the user's `.wia.sqlite`.
- **Revoke…** is available for an audited active mapping, **including a now-stale old annotation**. The user separately confirms revocation. `ctf_annotations_deactivate_pinned` revalidates source generation, document ID and the exact active old/new audit pair, preserves original notes and activation snapshot, and marks the audit entry `revoked`. A revoked mapping cannot be activated again by replay.
- The read-only `ctf_annotations_preview_pinned` response now provides `activation_state` (`null`, `active`, `revoked`) and `activation_target_entry_id` for each old note, if an audit row exists. These are **historical audit status**, not proof of a presently valid projection; `ctf_annotations_activated_list_pinned` independently validates the old note, target and source before projecting it.
- The native sheet never activates on mapping confirmation, never automatically rebinds ambiguous entries, never changes CTF, never mixes native notes with derivative caches. Rejected requests discard old selections and reload a freshly verified page. Close during a pending mutation is disabled.

## Reproduction and regression

From the repository root, Rust 1.90:

```shell
cargo test --locked -p atlas-service --test ctf_annotation_activation_contract
cargo test --locked -p atlas-service --test ctf_annotation_native_pages_contract
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

The synthetic `ctf_annotation_activation_contract` verifies `null → active → revoked` read-only audit visibility, no implicit activation after confirmation, stale-note conflict, rollback/replay rejection, immutable CTF bytes and unchanged derivative mirror. Full platform CI on the **exact final PR HEAD** is required: Linux workspace/fmt/Clippy, Windows x64 installer smoke and macOS ARM64/Intel native build + PKG/DMG smoke. Those checks do **not** replace physical GUI QA.

## Manual macOS acceptance (still NOT performed)

On real/virtual macOS ARM64 and Intel: verify the exact candidate from a changed external CTF, choose/confirm an inactive mapping, then **separately** activate and revoke it. Check cancellation at each stage, 0/1/8/20/21 notes, stale old-note revision or content, same-size source modification between sheet and consent, simultaneous sidecar writer, restart/upgrade from rc.2, and resolution across multiple conflict pages. Verify that the original CTF and derived SQLite remain unchanged and user-owned notes/backup survive. Inspect both valid overlays and stale audit entries; do not interpret the audit `active` flag as validation of an obsolete note.

## Stable 1.0 blockers

Original WhereIsIt modified-CTF import (#2), two historical private CTFs (#19), manual Windows 10/11 and macOS Intel/ARM GUI and upgrade, Authenticode + Apple Developer ID/notarization, target performance SLO and issues #2–#8 and #23 are OPEN. Unknown opaque `0x10` must **not** be equated with 1,782 absent descriptions. `v1.0.0-rc.2` remains an **unsigned prerelease**; no stable publication. Public CI must use synthetic fixtures only. Never upload private CTF, SQLite, paths, filenames, private SHA, or secrets.

**Next bounded WP:** Windows native explicit activation/revocation controls, followed by a verified read-only native overlay viewer and cross-platform manual release gates.
