# Linked original WhereIsIt CTF in native Windows/macOS (beta)

The native UI now has two explicit catalog modes:

1. **Editable local SQLite**: New/Open/Scan/Import Copy/Category Assign/Export
   retain their existing behavior. A normal CTF import is a separate snapshot.
2. **Linked external CTF (read-only)**: Link an original .CTF file, call
   ctf_sync in the existing native service, and keep the returned generation.
   The application displays disks with ctf_volumes_pinned, browses folders
   with ctf_children_pinned and searches with ctf_search_pinned.

Linked mode does NOT write the source CTF and does not allow native category/tag
assignment, scanning, importing to a disposable mirror, duplicate grouping or
CTF output. It DOES permit **derived CSV/Parquet reports** of selected search
results or recursive folders, written to a *new* file only with a source-hash-
pinned read-only mirror. A distinct durable annotation overlay is mandatory
before permitting user metadata edits. The original CTF may still be saved
independently in WhereIsIt; any external change invalidates pinned operations.
See [CTF_PINNED_EXPORT.md](CTF_PINNED_EXPORT.md) for failure/no-clobber rules.

Every linked page checks the original CTF SHA-256 before and after the read.
An external save causes source_changed, and the client must run ctf_sync for
a new generation; it must never continue using an old row ID from a different
source. The pinned SQLite cache is read-only, file-identity checked on every
request and periodically rehashed. Same-size/restored-mtime external saves
are detected by source hashing. A first import of 3M entries may take minutes,
so source synchronization executes off the UI thread.

## Native UX scope

- Windows: compact Link CTF action alongside Import CTF; disk pane, folder
  double-click and query results use read-only CTF-backed pinned APIs. Open or
  New local catalog switches back to editable SQLite. Category edits,
  duplicates and CTF exports are disabled; CSV/Parquet reports are allowed
  only as new files through the verified pinned export API.
- macOS: compact CTF menu groups Link read-only, Refresh linked CTF and Import
  copy. Disk sidebar, folder navigation, global search and status line reflect
  linked mode; unsafe editing/CTF-export actions are disabled. CSV/Parquet
  report export remains available, without modifying the linked CTF.
- No original CTF file is written. The initial linked generation must be
  explicitly confirmed by the native sync API; never reuse arbitrary stale
  source offsets or category IDs.
- No source file watcher or automatic GUI refresh is claimed. When the user
  needs an updated source, use the explicit Refresh/Link action. The backend
  returns source_changed instead of displaying mismatched results.
- The UI presently does not provide duplicate grouping, native-only category
  edits or modified-CTF writes in linked mode. Those require durable overlays
  and original-app compatibility acceptance.

## Acceptance gates

Cross-platform CI must build actual Windows, macOS Intel and ARM64 native apps
and installers, run synthetic tests for linked disk hierarchy and source
invalidation, and smoke install/uninstall safely. Source data, derived SQLite,
user query text and private source SHA-256 must not appear in public CI.

Real user 44-volume/3,046,027-entry CTF has already passed service/CLI mirror
and pinned query benchmarks on local Linux. This does **not** replace manual
GUI ergonomic checks (1366x768), macOS Gatekeeper/notarization or original
WhereIsIt opening a modified CTF. Stable 1.0 remains blocked by those gates.
