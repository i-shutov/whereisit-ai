# JSON API v1

All clients call the same service. Success: `{"ok":true,"data":...}`. Failure: `{"ok":false,"error":{"code":"...","message":"..."}}`. The normal envelope uses `op` and a UTF-8 `db` filesystem path. Commands are synchronous; native UIs invoke them on workers. `atlas serve` processes one JSON line at a time and emits one response line.

| op | Additional parameters | Result |
|---|---|---|
| `init` | — | Schema version; creates database if absent |
| `stats` | — | Volumes, entries and logical file bytes |
| `volumes` | — | Array of volume snapshots |
| `children` | `volume_id`, optional `parent_id`, `offset`, `limit` | Direct children with total |
| `search` | `query`, optional `volume_id`, `offset`, `limit` | Page and exact total; count may be expensive |
| `search_fast` | Same | Page and `has_more`, without counting all matches |
| `scan` | `root`, `label`, optional `job_id` | New immutable volume snapshot |
| `cancel` / `progress` | `job_id` | Scan cancellation/progress; available while scan is running |
| `categories` | — | Category definitions |
| `create_category` | `name`, optional `parent_id` | Created category ID |
| `assign_category` | `entry_id`, `category_id` | Assigns directly; folder assignments apply to descendant search |
| `update_entry` | `entry_id`, `description`, `tags` string or array | Changes native annotations |
| `explain_query` | `query` | Parameterized SQL and SQLite query plan |
| `duplicates` | `mode`: `name_size`, `name`, `size`; optional `query`, `volume_id`, `offset`, `limit` | Candidate members with group/count, `verified:false` |
| `import_ctf` | `path`, optional `utc_offset_minutes` | Atomic multi-volume import and diagnostics |
| `ctf_info` | `path`; `db` is not required | Structural counts, format and SHA-256 |
| `export_ctf` | `path`, optional `name`, `overwrite`, `allow_lossy` | Whole catalog in observed CTF variant |
| `export_csv` / `export_parquet` / `export_json` | `path`, selection below, optional `overwrite`; CSV `bom` | Streamed export, row count, byte count (editable SQLite) |
| `ctf_export_pinned` | Original CTF `path`, `cache_root`, 64-hex `generation` from `ctf_sync`, new `output`, `format` (`csv`, `parquet`, `json`), `mode` (`search`, `children`, `all`, `duplicates`), optional `query`, `volume_id`, `parent_id` for children, `duplicate_mode` for duplicates | Verified read-only CTF generation; streamed full selection, output SHA-256, row and byte count; no-clobber. Duplicate report explicitly has `verified:false` (metadata candidates, not content hashes). |
| `ctf_duplicates_pinned` | Original CTF `path`, `cache_root`, `generation`, `duplicate_mode` (`name_size`, `name`, `size`), optional `query`, `volume_id`, `offset`, `limit` | Read-only metadata candidate page: duplicate count/group, `has_more`, `total:null`, `verified:false`; no mutable duplicate cache |
| `ctf_annotations_list_pinned` | Pinned CTF `path`, `cache_root`, `generation`, explicit `annotation_db` ending `.wia.sqlite`; optional `offset`, `limit` | Lists user-owned annotations with optimistic revisions. Returns `generation_conflict` with **no projected entries** after external CTF changes. Read-only; does not create a store. |
| `ctf_annotations_set_pinned` | Same pinned fields, `entry_id`, `expected_revision`, `note`, `tags` strings | Creates or updates a **separate user-owned** annotations store. Revision 0 inserts, latest revision updates. Empty note+tags removes entry. Never modifies original CTF or derived mirror. |
| `ctf_annotations_confirm_pinned` | Original read-only pinned fields, existing `annotation_db`, `expected_document_id`, `old_entry_id`, `new_entry_id`, positive `expected_revision`, `confirmation:"user_selected_exact_candidate"` | User-confirmed metadata candidate, CAS-guarded append-only audit record in user-owned store; **does not activate or migrate notes**. |

Page limits are clamped to 1–1000. Syntax/type errors return an error instead of broadening an export. Native UIs use 250-row pages. Unknown CTF timestamps return `modified_local` as local calendar text, without a UTC suffix; `modified_unix` remains null unless its conversion basis is known.


## Linked CTF: durable user-owned annotations (experimental WP)

The original CTF is authoritative and always opened read-only. Its hash-checked generation and the immutable derived SQLite mirror are not an editable catalog. Native notes/tags are stored in an explicitly selected user-owned SQLite file with a name ending in .wia.sqlite **outside** the cache_root. No annotation is written into the CTF, mirror, Parquet export or the original WhereIsIt catalog. Store files are personal data and MUST NOT be uploaded to public CI or attached to GitHub issues.

After ctf_sync, callers must use the exact returned generation and provide an explicit annotation_db path. ctf_annotations_list_pinned returns state=uninitialized if no file exists and does not create one. ctf_annotations_set_pinned uses a transaction with optimistic expected_revision: 0 means no current row; a successful insertion returns revision 1; later edits require the current revision and increment it. Clearing both note and tags deletes the annotation. Annotation notes are limited to 16 KiB UTF-8 and tags to 8 KiB; larger payloads fail closed. The store contains its own randomly generated opaque document_id, original entry's disk label/relative path/type/size and source generation for later **review**, not for speculative automatic mapping.

If the original WhereIsIt modifies the CTF, the old pinned generation is rejected with source_changed, including same-size replacements. After a new ctf_sync, an old annotations store produces state=generation_conflict, items=[], requires_explicit_reconciliation=true. SET rejects annotation_generation_conflict. **No old entry ID is assigned to the new CTF, even if filename/size match**. The read-only ctf_annotations_preview_pinned operation supplies a bounded manual reconciliation preview after a source-generation conflict; it never changes the user-owned store or binds an old entry ID to a new one. A separate confirmation/migration operation and native conflict UI are not implemented. The user-owned file must be backed up independently and must not be placed in disposable generation caches.

All operations recheck original CTF source SHA at both ends through ctf_mirror::read_pinned, and check the cache stamp. Calls are synchronous and belong on a worker in the native UI. Operations use a separate SQLite rollback-journal store, not the derived read-only catalog. Avoid selecting symlink paths as user annotation destinations.

Example requests use caller-local paths and only illustrative synthetic values (never include private paths in logs or public bug reports):

~~~json
{"op":"ctf_annotations_set_pinned","path":"synthetic.ctf","cache_root":"mirror","generation":"<64 hex chars from ctf_sync>","annotation_db":"my-notes.wia.sqlite","entry_id":7,"expected_revision":0,"note":"My note","tags":"offline"}
{"op":"ctf_annotations_list_pinned","path":"synthetic.ctf","cache_root":"mirror","generation":"<64 hex chars from ctf_sync>","annotation_db":"my-notes.wia.sqlite","offset":0,"limit":100}
~~~




### Annotation reconciliation preview (read-only)

After a newly synchronized generation differs from the annotation store's saved generation, call ctf_annotations_preview_pinned with the same path, cache_root, new generation, annotation_db, optional offset and limit (default 50, maximum 50). LIST continues to return generation_conflict with items:[]; SET continues to fail closed. Preview requires the new generation to have been pinned by ctf_sync and verifies full CTF source SHA before and after the operation. It does not create any sidecar.

For each old annotation record, the preview compares the exact original disk label, relative path, kind, and size against the new read-only core. Results have candidate_state values no_match, unique_unconfirmed, ambiguous, a list of candidate new IDs and structural context, and requires_confirmation:true **even when there is exactly one candidate**. Up to eight candidates are returned per record; candidate_count_lower_bound, candidate_count_exact and candidates_truncated distinguish incomplete enumeration. Preview is paged by ascending old entry ID; it never executes an unbounded materialization of all notes. Exact metadata may still identify different bytes or ambiguous duplicates; there is no implied content-hash equality. This first preview does **not** accept mappings, edit either database, or migrate notes. No fuzzy/name-only fallback is silently applied.

Synthetic request shape (only local user paths and data; do not include private replies in public CI logs):

~~~json
{"op":"ctf_annotations_preview_pinned","path":"synthetic.ctf","cache_root":"mirror","generation":"<new 64 hex generation>","annotation_db":"my-notes.wia.sqlite","offset":0,"limit":50}
~~~

### Explicit annotation candidate confirmation (service-level journal)

A user, not an automatic heuristic, must select a target ID from the read-only reconciliation preview and explicitly confirm it. `ctf_annotations_confirm_pinned` requires the exact document ID and revision from preview, the pinned **new** generation, a positive old entry ID, a positive selected new entry ID, and `confirmation:"user_selected_exact_candidate"`. The selected new entry must still have the exact old disk label, relative path, kind and size. This is a **structural match only**, never proof of equal file contents. Same-generation confirmation is invalid; missing stores are not created.

A successful operation records an additive durable `annotation_confirmations` row in the **user-owned** `.wia.sqlite`, using an IMMEDIATE transaction and one-to-one constraints. It returns `state:"confirmed_not_activated"` and `annotation_rows_migrated:false`. Stale revisions/documents, repeated choices and target collisions return explicit conflicts and never overwrite prior decisions. The preview includes `confirmed_selection` for already approved rows; if the old annotation revision has subsequently changed, `confirmation_revision_conflict:true` invalidates the old review; a changed note/tags payload with the same revision is detected by an internal sidecar-only digest and sets `confirmation_payload_conflict:true`. Both set `requires_confirmation:true`. All unresolved records remain visible. **Existing annotation rows, notes, original source generation, CTF and derived mirror stay unchanged.** LIST remains in `generation_conflict` after recording confirmations; SET still rejects edits to the new generation. Explicit non-destructive read-only activation with a private snapshot and revocation is now available (below); native review dialogs, writable migration and whole-file recovery are future work. A later source edit invalidates the previous generation pin.

Synthetic example (never publish real local paths, private names or hashes):

~~~json
{"op":"ctf_annotations_confirm_pinned","path":"synthetic.ctf","cache_root":"mirror","generation":"<new 64-hex generation>","annotation_db":"my-notes.wia.sqlite","expected_document_id":"<document id from preview>","old_entry_id":7,"new_entry_id":12,"expected_revision":1,"confirmation":"user_selected_exact_candidate"}
~~~

See [bounded confirmation WP](qa/CTF_ANNOTATION_CONFIRMATION_CAS_WP_20261030.md) for risks and reproducible tests.

### Explicit, reversible read-only activation (service-level)

After a user has explicitly recorded an exact candidate via `ctf_annotations_confirm_pinned`, a *second* explicit command `ctf_annotations_activate_pinned` can create a **read-only activation projection**. Supply the same pinned source/cache/new generation and external user-owned `.wia.sqlite`, `expected_document_id`, `old_entry_id`, `new_entry_id`, `expected_revision`, and `activation:"user_approved_reversible_projection"`. It refuses stale document/revision, changed note/tags (including ABA), absent or stale confirmation, mismatched target, and repeat activations.

The separate sidecar records a full private per-note snapshot and activation state; **original annotation entries and generation are not changed**. No CTF or derived cache is written. `ctf_annotations_activated_list_pinned` returns only still-valid, hash-generation-pinned, read-only projections (`items`, `total`) and the count of incompatible activations (`conflicts`); stale entries are **not** shown as active. `ctf_annotations_deactivate_pinned` requires `expected_document_id`, `old_entry_id`, `new_entry_id`, and `rollback:"user_confirmed_revoke_projection"`, retaining the private snapshot in a revoked journal row. Revoked activation cannot be replayed automatically. Ordinary LIST/SET remain conflict-guarded when the external CTF generation differs; there is no migrated, writable annotation store.

The operation is lossless because the original notes never move and the complete snapshot stays in the separate user-owned sidecar. This is **not** a guarantee of full-file backup against disk failure, nor proof that candidate files have matching contents. See [activation WP](qa/CTF_ANNOTATION_ACTIVATION_OVERLAY_WP_20261030.md) for limitations and synthetic regression coverage.

## Export selection

`mode:"search"` selects the full result of `query`, optionally within `volume_id`. An empty query selects all entries in scope. `mode:"children"` requires `volume_id` or `parent_id`: a non-null parent includes that selected entry and every descendant, while a null parent selects the volume. `mode:"duplicates"` uses `duplicate_mode:"name_size"|"name"|"size"` and the query/volume filter. Pagination does not truncate exports.

Exports use a temporary file beside the destination and publish it by rename only after successful completion. **For editable SQLite exports only**, existing files require `overwrite:true`; native UIs set this after their native save dialog. **Linked CTF export always refuses overwrite and an existing destination**, even if the save dialog was accepted. `ctf_export_pinned` exports a generated report, never a CTF file, from a source-hash-verified read-only generation; if the CTF changes during the request the operation aborts without publishing its staging file. The active database, its WAL/shared-memory siblings and original CTF are protected. CSV preserves original field values; quote/newline escaping follows CSV rules. A CSV should be treated as data when imported into spreadsheet software.

The CSV/Parquet schema contains `id`, `volume_id`, `parent_id`, `name`, `relative_path`, `kind`, `size`, `modified_unix`, `description`, `tags`, `volume_label`, `categories`, `duplicate_count`, `duplicate_group`, `modified_local`. Sizes are exact 64-bit integers; Parquet stores nullable dates and parents. `categories` lists direct assignments separated by `; `, not a reversible encoding of arbitrary category names or inherited assignments. Native search does account for folder inheritance.

## Compatibility and resource limits

`import_ctf` checks the exact source SHA-256 to avoid importing the same file twice. Source CTF is never edited. All decoded entries and preserved legacy metadata commit in one SQLite transaction. Import cancellation and streaming CTF are not implemented in v1; the UI keeps long CTF calls outside its main thread. Catalog source memory and CTF output buffers scale with input size.

New scans do not calculate content hashes or expand archive contents. Duplicate groups are metadata candidates; `mode:"hash"` returns unsupported. Editable SQLite duplicate queries use a persistent revision-checked cache of member IDs and group keys; linked CTF duplicate queries instead use a **read-only window SELECT** and cannot write the immutable mirror. Linked duplicate-candidate CSV/Parquet/JSON export uses `ctf_export_pinned` with `mode:"duplicates"` and the same query-scoped, read-only grouping as `ctf_duplicates_pinned`. It exports the **full** matching candidate set, not a page. It refuses `offset`, `limit`, non-null `parent_id`, and `duplicate_mode:"hash"`, and never populates the editable duplicate cache. Candidate status remains `verified:false` despite source-generation verification. Public writers create new volumes and entries in a single transaction; direct external SQL writes bypass some invariants and are outside the supported API.

CTF v1 supports the observed uncompressed Catalog 3.00 and a final empty group. Unsupported flags/header variants fail closed. Populated groups, compressed/encrypted variants, multi-source CTF merging, symlink export and native-to-CTF category mapping remain outside current support. Normal CTF export rejects new native category assignments (`ctf_categories`) and nonempty native tags (`ctf_tags`) before creating an output file, because their CTF mapping is not implemented. These checks run in the same database snapshot as export. Explicit `allow_lossy:true` permits omitting those annotations from the CTF file while keeping them in SQLite. The result reports `new_categories_omitted` and `new_tags_omitted` booleans; CSV/Parquet retain both annotation fields. Unknown metadata remains attached to its original tree; moving entries to other parents is not an exposed operation.

## C ABI

`atlas_abi_version()` returns 1. `atlas_call(const char*)` borrows a valid NUL-terminated UTF-8 request and returns an owned NUL-terminated UTF-8 response. Release it exactly once with `atlas_string_free(char*)`. A null input yields an error response; freeing null is allowed. Arbitrary/dangling pointers are not valid ABI input. Panics are contained before crossing the C boundary.

The Windows client links the Rust service directly. The macOS client uses this ABI via a C module map. No local HTTP server or browser runtime is required.

## Private linked-CTF annotation recovery (stacked draft WP)

`ctf_annotations_backup_pinned` and `ctf_annotations_restore_pinned` provide owner-approved, hash-verified whole-sidecar backup and restore **to new files only**. Backups snapshot all private SQLite tables consistently; restores require the exact returned local backup SHA-256 and document ID. These are not CTF exports and never touch the read-only CTF or derived SQLite. See [snapshot/restore contract](qa/CTF_ANNOTATION_SNAPSHOT_RESTORE_WP_20261030.md) for complete fields, limitations and synthetic QA.
