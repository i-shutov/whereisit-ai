# External WhereIsIt CTF — read-only native search mirror

The original CTF is the authoritative source. This is the first end-to-end
native **service/CLI** integration of the content-addressed cache; the Windows
and macOS GUI file-watching/switching interface is a subsequent separate gate.

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
- The UI has not yet connected its Open CTF action to this service.
- Native-only categories/annotations, legacy category mapping and original
  WhereIsIt acceptance of *modified* exports remain release blockers.
- Platform-specific power-failure durability of directory publication is not
  proven on Windows/macOS; always retain the authoritative source and backup.
