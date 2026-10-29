# ADR-0003 — CTF as authoritative interoperable catalog; content-bound derivative caches

**Date:** 2026-10-29. **Decision:** accepted user requirement. **Implementation:** isolated prototype and tests; native UI/search coordination not integrated yet.

## Requirement

The original WhereIsIt must remain free to open and save a CTF independently. WhereIsIt AI must reopen the resulting CTF, identify the current generation, and automatically rebuild derived indexes when required. A persistent index/Parquet/SQLite derivative may accelerate queries but must not silently become a second master of CTF-representable data.

Two real CTF attachments have been supplied for read-only acceptance. They are not redistributable test fixtures by default: no upload into this public repository, public Actions artifacts, logs or releases. File receipt is not a completed benchmark. The local execution environment failed before binary access in this session, so no counts, SHA-256 hashes or timings for those attachments are claimed here.

## Identity model

**Name/path locates; byte digest validates.** Filename alone, last-write time, file size, inode or Windows file identifier is insufficient to validate a derived index. The prototype's cache key hashes this tuple:

`manifest format + source SHA-256 + source byte length + parser version + index format version + normalization version + sort version + index kind + options SHA-256 + optional native-overlay SHA-256`.

Thus:

- Same name and different bytes: different generation, never reuse as current.
- Same size and restored modification time but different bytes: still invalidated.
- Renamed/moved file with identical bytes: a compatible content-addressed cache can be reused.
- Two byte-identical copies can share immutable *derived bytes*. This does not merge their editable document identities or user annotations.
- Same source with a new parser/normalizer/sort/index option: rebuild the affected derivative.
- If a derived index depends on native annotations, their generation/digest is part of the recipe. Otherwise native edits need not invalidate CTF-only indexes.

An adjacent, human-readable locator named after the CTF may point into an application-managed cache directory. It is a hint, not authority. The current prototype writes immutable generations as `cache/<combined-key>/manifest.json` and `index.bin`; an adjacent locator and the application cache coordinator remain to be implemented.

## Source and cache lifecycle

1. Capture CTF bytes without opening the source for writing. Reopen the path and verify a second complete SHA-256/length. A mismatch means the source was changing: do not build/publish a current index from that capture.
2. Parse the captured immutable bytes. A matching hash is not proof of a supported/valid CTF. Truncated or unsupported sources must fail explicitly; never silently show the previous generation as if current.
3. Load a cache only when source identity, recipe, manifest format, artifact size and artifact digest all agree. Artifact bytes must still be validated by the actual index decoder; a checksum is not authentication against a malicious writer.
4. Build off the UI thread from one snapshot. Write a unique temporary generation, sync its files, publish the complete directory. Never update the active index in place. Readers retain their previous immutable snapshot until a coherent replacement is ready.
5. Revalidate the live source before selecting the built generation as current. If an external editor saved during building, retain/discard the old-derived cache as a cache of the old digest and rebuild for the current source. Do not associate old row offsets with the new CTF.
6. Missing derivative: rebuild. Corrupt derivative: quarantine/rebuild explicitly. The prototype identifies corruption and refuses overwrite; automatic quarantine/coordinator behavior is not yet implemented.

File-change notifications are invalidation hints, not proofs of identity or completeness. Reopening, returning focus, export/save, missing watcher events and periodic verification must be coordinated by the application. An external writer can change a file immediately after any check: without cooperative locking there is no permanent 'current forever' guarantee. Responses must be associated with the captured generation; do not mix bytes from one generation with IDs from another. The double-read check detects ordinary racing saves but is not a general filesystem snapshot or writer lock.

## Storage responsibilities

| File/layer | Authority | Deletable/rebuildable? |
|---|---|---|
| Original/published CTF | CTF-representable data, preserved unknown original fields | No |
| Search index, cached SQLite mirror, Arrow/Parquet analytics snapshot | Purely derived data, tagged with source hash and recipe | Yes, only if all values are reproducible from authoritative sources |
| Native annotation/edit sidecar | Only attributes/edits not yet represented in CTF | No; must be backed up separately |
| RAM query structures | Snapshot of one verified source generation and applicable overlay | Yes |

The existing writer cannot yet encode all native categories/tags in CTF. Those values must not be silently lost on rebuild. Keep them separate from disposable cache generations, bound to a persistent document identity. If an external save renames/removes/reorders entries, automatically applying annotation rows by old offsets or row numbers is unsafe. Reconcile with verified record identities; retain ambiguous/orphaned data and report conflicts instead of guessing or deleting it.

## External edit versus unsaved local edit

With no pending AI edit, external save means invalidate → recapture → parse → rebuild → switch generation.

With pending edits based on digest A and live CTF at digest B, **do not overwrite B**. The prototype provides `verify_edit_base`, an optimistic check returning a conflict on a changed digest. This is NOT an atomic compare-and-replace primitive and has no source-save function. The eventual save coordinator needs a validated platform protocol, preserved prior generation/backup and explicit reconciliation/Save As on conflicts. No cross-file atomicity is assumed among CTF, index, Parquet and native sidecar.

A new external file version may legitimately require a full index rebuild. SHA-256 identifies stale data; it does not infer the semantic edit or provide conflict-free merging by itself. Full rebuild is the correctness baseline; incremental rebuild is a later optimization gated by differential correctness tests.

## Prototype implementation

- `crates/atlas-service/ctf_sidecar/mod.rs`: capture identity, hash-bound recipe, manifest validation, immutable cache publication, edit-base conflict detection. Independent of SQL schema and no source writes.
- `crates/atlas-service/tests/ctf_sidecar_contract.rs`: byte-identity, rename/copy, same-name replacement, same-size/restored-mtime edits, recipe changes, missing/corrupt/partial cache, conflict and read-limit tests. These are generated fixture tests, not measurements of private uploads.
- `crates/atlas-service/examples/ctf_real_acceptance.rs`: local read-only acceptance runner. Uses temporary copies for SQL import, CTF re-export, CSV/Parquet and simulated external edits; emits aggregate JSON without internal entry names, paths or actual sampled query text.
- `.github/workflows/ctf-sidecar-contract.yml`: Linux, Windows, macOS ARM64 and Intel contract checks. CI invokes only the generated `--self-test` fixture.

The artifact published by the acceptance example is a test summary, not a production search index. Full search/cache integration, file watching, automatic rebuilding, reconciliation and original WhereIsIt GUI testing remain separate work.

## Real-data acceptance checklist

For each local input: verify immutable source SHA before/after; parse/count; raw unchanged CTF roundtrip; CTF→SQLite→CTF byte comparison; fixed query first-page ID and count equality versus a Rust reference; metadata duplicate participant counts; CSV/Parquet row counts; rename cache reuse; changed CTF invalidation; unsaved-edit-base conflict. The real runner does not claim full arbitrary-query equivalence, complete export-value equality, p95/cold-cache measurements, hash-verified file-content duplicates or legacy-app acceptance.

A changed catalog inside the acceptance runner is written only to a private temporary *copy*. No original CTF is overwritten. The normal product continues using the released schema until this branch is reviewed and integrated.
