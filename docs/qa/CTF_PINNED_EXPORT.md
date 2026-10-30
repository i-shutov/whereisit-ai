# Linked CTF — verified CSV/Parquet/JSON export (candidate vertical)

**Status:** code/fixture acceptance through PR #34; not a stable 1.0 release
or a claim of complete CTF interoperability.

## User-visible path

Link the **original** WhereIsIt `.CTF` in native Win32/SwiftUI, wait for
`ctf_sync`, select an offline disk, folder or global Boolean query, then choose
**CSV** or **Parquet** and Export. The source is never rewritten. CSV/Parquet
are reports, **not** replacements for CTF. Duplicates and native annotation
edits remain unavailable in linked mode.

The native CLI/service can also export JSON. It has the same protection.

```json
{
  "op": "ctf_export_pinned",
  "path": "/example/original.CTF",
  "cache_root": "/example/CTF-Mirrors",
  "generation": "<64 hex digits returned by ctf_sync>",
  "output": "/example/new-report.parquet",
  "format": "parquet",
  "mode": "search",
  "query": "(ext:mp3 OR ext:flac) AND size:>10MiB"
}
```

Use `mode=children` with `volume_id` and optional `parent_id` for a
recursive subtree, or `mode=all` for the whole catalog. Query syntax is the
same tested Rust Boolean-query grammar used by the normal SQLite exporter.
This exports the **full matching set**, not merely the GUI's 250-row page.
The exporter streams rows in bounded groups (8192 for Parquet).

## Read/write isolation and publication contract

1. Reject unsupported format/mode, malformed query and an explicit
   `overwrite=true` before touching the original CTF.
2. Refuse an existing output, output inside the immutable CTF mirror root,
   output equal to the original CTF and output with the wrong extension.
3. Export to a disposable staging name in the destination directory, never
   directly to the user-named output.
4. The same `ctf_search_pinned` source-generation gate verifies the complete
   original CTF SHA-256 before and after streaming, the exact read-only
   SQLite generation, its stable file stamp, and periodic artifact checksum.
5. Only after the verification succeeds, calculate the staged output SHA-256
   and atomically publish it with **no-clobber** semantics. A racing new
   target is refused instead of replaced. Failed operations clean up the stage.
6. Report source generation, row count, output byte length and output SHA-256;
   no original CTF names/content/paths appear in public CI reports.

The source can be changed independently **after** an export was verified;
there is no cross-application transactional lock. The exported file is a
verified snapshot at publication-check time, not a live synchronized view.
Manual overwrite requests for existing target files are intentionally rejected:
choose a new filename. The GUI save panel may still offer OS-level overwrite
confirmation, but the service independently refuses it.

Linked mode remains **read-only**. Unlike native annotations, these exports
contain no edits that need durable reconciliation. The generated SQLite
mirror is a disposable search cache and must never become the user master.

## Synthetic verification

`cargo test --locked -p atlas-service --test ctf_linked_export_contract`

Cases: Unicode search CSV, Parquet physical row count, JSON; recursive folder
with precise inclusion/exclusion; unchanged original CTF and immutable
manifest; stale generation after a same-size external save; existing output
with an overwrite request; cache-root target; invalid format/mode; absence
of any leftover stage files. GitHub's `build.yml` must independently pass
Rust test/fmt/Clippy, Windows native installer smoke, and macOS ARM64/Intel
native packaging.

**Not yet accepted:** actual modified CTF in original WhereIsIt, semantic
decoding of 0x10/0x1c extensions, private e-books/MP3 CTF samples, native
annotation overlays, signed/notarized binaries, manual 1366x768 / DPI UX
verification, all performance SLOs. PR CI alone cannot close those gates.
