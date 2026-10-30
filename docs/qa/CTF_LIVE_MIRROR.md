# External WhereIsIt CTF — read-only native search mirror

The original CTF is the authoritative source. This is the first end-to-end
native **service/CLI** integration of the content-addressed cache. Windows and
macOS now offer explicit **Link CTF** actions, read-only browsing/search, and
new-file CSV/Parquet export. An automatic filesystem watcher and native
annotation reconciliation are still separate future gates.

Call through the native application JSON service or installed CLI:

    atlas request '{"op":"ctf_sync","path":"/private/catalog.ctf","cache_root":"/private/mirror-cache"}'

    atlas request '{"op":"ctf_search","path":"/private/catalog.ctf","cache_root":"/private/mirror-cache","query":"ext:mp3 AND size:>10MiB","limit":100}'

The first call imports a private, unchanged CTF into a new temporary SQLite v3
database, constructs FTS5, verifies real FTS5 content integrity and SQLite
integrity, restores all triggers, checks source identity again, then atomically
publishes a read-only generation. The temporary index is built under the
explicit, trusted cache root. Original CTF is never opened for writing.

Each generation is keyed by SHA-256 and original length plus parser, normalizer,
sort and SQLite mirror recipe. Its manifest records database SHA-256/length,
linked SQLite version and numeric counts. A repeated call independently hashes
the live CTF and the cached SQLite, rejecting stale or corrupt generations.
An external CTF save creates another generation, including a same-size edit
with a restored modification timestamp. Byte-identical renamed copies may
reuse the immutable *derived* bytes.

Search opens the cache with an explicitly read-only SQLite connection and
PRAGMA query_only. It refuses edit operations; native user annotations are not
stored in these disposable generations. A final source digest after every
search prevents returning a result associated with an older generation after
an external save observed during the request. The response includes the
generation identifier so clients can associate a result with its source.

## Safety boundaries

- No automatic original CTF overwriting and no CTF save API.
- No native category/tag mutations against disposable read-only SQLite mirror.
- Missing/corrupt generation means a visible failure, not silent deletion.
- No partial cached generation is published before all checks pass.
- A private source CTF and cache root never enter public GitHub/Actions.
- A concurrent uncooperative CTF writer can save immediately after validation.
  This API is optimistic, not a filesystem lock or a permanent guarantee.
- Current normal CTF reader still handles only supported uncompressed
  Catalog 3.00 variants; unknown descriptions remain opaque.
- Search on a new 3M-row CTF can take minutes for initial build. Even warm
  requests hash the entire mirror for integrity and may take seconds. This
  prioritizes correctness, not the future <=150 ms warm GUI SLO.
- Native GUI linked-mode access is now wired to the verified pinned APIs;
  it is **explicit Link CTF**, not automatic file watching. Safe report
  export is specified in [CTF_PINNED_EXPORT.md](CTF_PINNED_EXPORT.md).
- Native-only categories/annotations, legacy category mapping and original
  WhereIsIt acceptance of *modified* exports remain release blockers.
- Platform-specific power-failure durability of directory publication is not
  proven on Windows/macOS; always retain the authoritative source and backup.

## Additional verified contract cases

The native contract suite also asserts that an unchanged CTF renamed within the
local filesystem reuses the existing content-addressed generation, without
rebuilding the SQLite mirror or touching its bytes. A malformed Boolean/search
expression is parsed before the potentially multi-minute import; malformed or
missing queries must not even create the cache directory.

A derived, non-authoritative scan timestamp is normalized when publishing a
mirror. The generation identity comes from original CTF content, not from the
clock time when a copy happened to be indexed. This does not modify DOS
timestamps preserved in the CTF entry records or the original source.

### Operational readiness

The commands are accessible through the native Rust service and CLI, and
linked generation browsing/search is wired to both compact native Windows
and macOS UIs. This is **not** a live source-file watcher, verified manual GUI
acceptance or durable native annotation store. The response includes a
generation key; both UIs must reject stale generations after source changes. A
warm-cache lookup currently performs complete SHA-256 checks of both the
source and the SQLite mirror; the first verified implementation prioritizes
correctness and remains unsuitable for subsecond queries on very large disks.

## Verified private real-source acceptance, 2026-10-30

The checksum-verified Linux CLI for commit `504aa08c`, built with SQLite
3.51.3, was executed **locally** against an original 44-disk CTF containing
3,046,027 records. This private data was not uploaded to GitHub or included
in the repository or public CI artifacts.

- First CTF-to-mirror build and validation: 293.926 seconds
- Reuse of verified unchanged generation: 2.099 seconds
- Four full-count query operations: 2.400–2.692 seconds, with consistent
  source generation and expected result counts
- Published SQLite mirror: 1,675,280,384 bytes
- Peak child-process RSS: approximately 810 MiB
- Temporary main SQLite + WAL disk during build: approximately 3.2 GiB
- Original CTF: SHA-256 unchanged after all operations
- A disposable copy of the published mirror exported a **byte-identical**
  144,766,574-byte CTF in 11.4 seconds; the published mirror was unchanged

All comparisons **passed**, but the cache-hashing warm-query latency does
not meet the intended interactive search experience. The next architectural
block is a resident generation-pinned search session with a separate
strict-validation operation and explicit stale-source invalidation. Do not
market the current API as a fast GUI search implementation until measured.

Original WhereIsIt acceptance of *modified* CTF and the two historical smaller
private CTF samples remain open gates, as do native UI integration and
the permanent overlay store for annotations.
