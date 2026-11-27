# Direct CTF annotations contract (draft)

Date: 27 November 2026. Status: proposed contract; **not implemented** and not
acceptance evidence. Scope: notes, tags, and native categories while a CTF
is open through the direct metadata snapshot, without a complete SQLite import.
Categories may follow notes/tags in a separate implementation patch, but remain
required for the Windows 1.2 release workflow.

## Authority and storage

- The original CTF remains authoritative and read-only. Annotation writes go
  only to the existing user-owned `.wia.sqlite` store.
- Reuse `crates/atlas-service/src/ctf_annotations.rs` and
  `ctf_annotation_backup.rs`. Do not add a competing annotation database or put
  user data in a disposable cache or snapshot RAM. A new exported CTF may encode
  supported accepted edits; it does not replace the durable store for native
  state which CTF cannot represent.
- Existing v1 data remains readable and backed up. Before adding category
  definitions/assignments or changing record addressing, define a versioned
  migration. Migration is transactional, preserves all existing notes, tags,
  revisions and audit rows, and has a verified backup/restore path. Never
  silently reinterpret the existing `tags` string as categories.
- A store has a random opaque `document_id`; this is user-data identity, not a
  digest of CTF bytes. Identical CTF copies are separate documents by default:
  do not auto-select or merge sidecars by content hash, name, or path. A user
  explicitly selects/creates the sidecar for a document.

## Two identities, two purposes

- Preserve the direct-preview **open generation token** returned by
  `ctf_preview_open`. It changes on each open and is required on every direct
  read/write request to reject delayed replies and stale UI actions.
- Add a separate **durable source generation**: a deterministic identity for
  source bytes plus parser/format interpretation version. It must not contain
  the per-open serial. It detects whether saved annotations belong to the same
  parsed source generation across restart. It does not establish entry
  correspondence across changed generations.
- The sidecar's document UUID distinguishes user-owned document histories,
  including byte-identical copies with distinct explicitly selected stores.
  Neither the durable source generation nor an entry address substitutes for
  this UUID.
- A record address is valid only within its durable source generation. Keep
  entry ID for addressing in that generation and retain volume/path/kind/size
  as review context. On a changed source generation, list a conflict and
  expose no annotations as attached to current entries. Do not auto-rebind by
  ID, path, name, size, or metadata candidate. Existing explicit reconciliation
  remains a separate user-reviewed workflow.

## Direct-service boundary

- Direct reads and writes validate the current preview token, source path, and
  current source bytes against the opened snapshot before and after the
  operation (or use the existing verified Windows read pin). A mismatch fails
  closed as `source_changed` / stale-session error; reopen before retrying.
- Direct annotation operations consume the already-open parsed snapshot to
  validate an entry and obtain its metadata context. They must not call
  `ctf_sync`, create/populate the disposable SQLite mirror, or import all CTF
  rows into a native catalog. One edit is bounded to one record and one
  transactional sidecar operation, off the UI thread.
- Preserve store-path protections: explicit `.wia.sqlite` destination outside
  source and derived cache, no-follow opening, recognized application/schema,
  and refusal to modify a foreign SQLite file. The original CTF stays read-only.
- `ctf_preview_open` may report annotation capability only after the direct
  service can validate and use the durable sidecar contract. Until then the
  existing `annotations_supported:false` behavior remains accurate.

## Tentative request and result shape

Names below are proposals, not registered API operations. Native routing must
allowlist them only for a direct-preview session and inject pinned identities;
ignore caller-supplied source path, generation, or catalog database fields.

`ctf_preview_annotations_list` request: `path`, `open_generation`,
`source_generation`, explicit `annotation_db`, `offset`, `limit`.

`ctf_preview_annotations_set` request: same pinned fields plus `entry_id`,
`expected_revision`, `note`, `tags`. Revision zero means insert; subsequent
updates require the current revision. An unchanged retry may return the
original successful result; a request with stale revision or a different
payload returns a conflict and never overwrites newer data. Clearing both
fields is an explicit delete and returns the new absent revision state.

Results should retain the established envelope and report `state` (`ready`,
`uninitialized`, `generation_conflict`, or `source_changed`), `document_id`,
`source_generation`, paginated items/total, and `read_only_source:true`.
SET additionally reports `entry_id`, `revision`, and `saved:true` only after
transaction commit. Generation conflict returns no projected items. Errors
must distinguish stale open token, stale source bytes, stale sidecar revision,
and invalid/unsafe store. Do not report success before durable commit.

Future category operations use this same sidecar and identity/CAS model. Their
category hierarchy, assignment, filtering, and export semantics need a separate
versioned schema contract before implementation.

## Retry, CAS, and recovery

- Existing `expected_revision` transaction CAS is the baseline. Concurrent
  writers with stale revisions must receive a conflict and reload.
- Current delete-and-recreate behavior can reset a row revision. Extend the
  durable mutation identity (for example, a monotonic per-record revision or
  operation journal) before promising replay-safe retries; prevent ABA from
  making an old request appear current.
- Define an operation ID/idempotency key if transport retry must return the
  prior result after an uncertain response. Bind it to document UUID, durable
  source generation, entry address, expected revision, and payload digest;
  reject reuse with different content. Never infer success from matching text
  alone.
- Keep the existing whole-store backup/restore semantics: hash-verified
  snapshots, restore to a new absent sidecar only, and preserve unknown/audit
  tables. Extend snapshot validation and tests when schema tables are added.

## First backend patch

First patch should be limited to `ctf_preview.rs`, `ctf_annotations.rs`,
`lib.rs`, and synthetic service contract tests. Extract/reuse sidecar storage,
CAS, and safety logic while adding direct-snapshot entry validation and the
separate durable source-generation check. Keep the open token as a mandatory
RPC freshness guard. Do not add category writes in this patch. Add tests for
same-source reopen persistence, stale open token, changed bytes/generation,
CAS conflict, delete/recreate ABA, retry behavior, unsafe path/store, and
unchanged source bytes. Update `ctf_annotation_backup.rs` only if the chosen
first schema migration changes its inventory/validation.

## Initial test matrix

| Case | Expected result |
|---|---|
| First direct list with absent sidecar | `uninitialized`; no file created |
| Save then close/reopen same source | Note/tags persist; new open token accepted with same durable source generation |
| Delayed request from prior open | Rejected; no sidecar/source change |
| Source changes between open and write | Rejected; sidecar unchanged |
| Existing sidecar belongs to another source generation | `generation_conflict`; no items projected or mutation allowed |
| Two identical copies with distinct user-selected stores | Distinct document UUIDs and edits |
| Concurrent stale revision / duplicate transport retry | No clobber; specified CAS/idempotency result |
| Delete then recreate against an old request | Old request rejected (ABA protection) |
| Unsafe destination or foreign SQLite | Refused before mutation |
| Any direct annotation operation | No `ctf_sync`, mirror creation, or full import; CTF bytes unchanged |
| Backup/restore after schema extension | Verified new-file restore; all known and unknown tables preserved |

## Current implementation status

**NOT IMPLEMENTED:** direct-preview annotation endpoints, durable source
generation separate from the per-open token, direct UI routing, category storage
or semantics, ABA-safe mutation identity, and idempotent retry journal. Current
`ctf_annotations_*_pinned` operations require `ctf_mirror::read_pinned` and the
stable mirror generation; `ctf_preview` advertises annotations unsupported.
This document is a proposal, not a claim that any listed contract or test has
passed.
