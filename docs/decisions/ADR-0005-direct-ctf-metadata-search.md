# ADR-0005 — Direct metadata search when opening CTF

Date: 24 November 2026. Decision: owner-directed implementation, in progress; not release acceptance.

The owner rejected Windows 1.1's multi-minute first CTF open and explicitly rejected making folder browsing available while search still waits approximately 190 seconds. Following the Everything architecture study, the owner directed work to focus on this redesign.

Opening a supported CTF captures and validates an immutable source generation, then exposes browsing and full query semantics directly over its metadata snapshot. Opening must not automatically populate a complete SQLite mirror or construct an FTS5 trigram index. Explicit import to an editable native SQLite catalog remains available as a separate operation.

Rust and SQLite remain core technologies. SQLite retains transactional native catalogs and durable user state. This decision changes the interactive CTF read/query path, not the authority of user annotations or the native catalog storage engine. ADR-0003 source identity, original-CTF read-only, opaque metadata preservation and external-edit conflict rules continue to apply.

SQL and snapshot queries must share a parsed, typed query representation and validation. Boolean precedence, Unicode case handling, glob semantics, category/branch scopes, ordering and pagination must agree. Snapshot execution must not silently reduce the language to filename-only searching. Missing category mappings retain the existing explicit semantics; no invented assignments.

Metadata rows use stable IDs, parent references and shared parsed data. Materialize displayed/exported rows in bounded pages. Compute paths only when needed. Compact sort/filter accelerators may be built in memory during capture if measured latency and memory justify them; persistent accelerators remain optional and disposable. Keep scanning, query evaluation, counting and result sorting outside the UI thread, with cancellation and stale-generation rejection.

The PR58 layout stays authoritative; this backend change does not authorize redesign. Native request routing must work directly after open, without a search-preparation state or mandatory background conversion.

Acceptance requires the exact release-mode source on a full read-only working copy: opening plus first search, absent-result full scans, Boolean/Unicode/path/description/numeric/scope queries, correct sorted pages and counts, cancellation/rapid re-query, memory, repeated opening, synthetic SQL equivalence and actual native GUI use. Service-only timings and earlier warm SQLite p95 do not establish installed-app acceptance. Keep `stable_ready=false`; published releases remain immutable until a separately verified hotfix is ready.

Reference: [Everything study](../architecture/EVERYTHING_CTF_SEARCH_REVIEW_20261124.md).
