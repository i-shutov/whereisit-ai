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

Page limits are clamped to 1–1000. Syntax/type errors return an error instead of broadening an export. Native UIs use 250-row pages. Unknown CTF timestamps return `modified_local` as local calendar text, without a UTC suffix; `modified_unix` remains null unless its conversion basis is known.


## Linked CTF: durable user-owned annotations (experimental WP)

The original CTF is authoritative and always opened read-only. Its hash-checked generation and the immutable derived SQLite mirror are not an editable catalog. Native notes/tags are stored in an explicitly selected user-owned SQLite file with a name ending in .wia.sqlite **outside** the cache_root. No annotation is written into the CTF, mirror, Parquet export or the original WhereIsIt catalog. Store files are personal data and MUST NOT be uploaded to public CI or attached to GitHub issues.

After ctf_sync, callers must use the exact returned generation and provide an explicit annotation_db path. ctf_annotations_list_pinned returns state=uninitialized if no file exists and does not create one. ctf_annotations_set_pinned uses a transaction with optimistic expected_revision: 0 means no current row; a successful insertion returns revision 1; later edits require the current revision and increment it. Clearing both note and tags deletes the annotation. Annotation notes are limited to 16 KiB UTF-8 and tags to 8 KiB; larger payloads fail closed. The store contains its own randomly generated opaque document_id, original entry's disk label/relative path/type/size and source generation for later **review**, not for speculative automatic mapping.

If the original WhereIsIt modifies the CTF, the old pinned generation is rejected with source_changed, including same-size replacements. After a new ctf_sync, an old annotations store produces state=generation_conflict, items=[], requires_explicit_reconciliation=true. SET rejects annotation_generation_conflict. **No old entry ID is assigned to the new CTF, even if filename/size match**. Manual reconciliation of candidates and conflict resolution are a distinct follow-up WP; this WP does not pretend to solve them. The user-owned file must be backed up independently and must not be placed in disposable generation caches.

All operations recheck original CTF source SHA at both ends through ctf_mirror::read_pinned, and check the cache stamp. Calls are synchronous and belong on a worker in the native UI. Operations use a separate SQLite rollback-journal store, not the derived read-only catalog. Avoid selecting symlink paths as user annotation destinations.

Example requests use caller-local paths and only illustrative synthetic values (never include private paths in logs or public bug reports):

~~~json
{"op":"ctf_annotations_set_pinned","path":"synthetic.ctf","cache_root":"mirror","generation":"<64 hex chars from ctf_sync>","annotation_db":"my-notes.wia.sqlite","entry_id":7,"expected_revision":0,"note":"My note","tags":"offline"}
{"op":"ctf_annotations_list_pinned","path":"synthetic.ctf","cache_root":"mirror","generation":"<64 hex chars from ctf_sync>","annotation_db":"my-notes.wia.sqlite","offset":0,"limit":100}
~~~


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
