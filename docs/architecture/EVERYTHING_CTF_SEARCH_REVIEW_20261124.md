# Everything architecture and the CTF first-search defect

Date: 24 November 2026. Status: researched design proposal; P1-CTF-COLD-OPEN remains OPEN. This document does not certify a fixed installer. Sources are official voidtools documentation and posts by its developer, read on this date. Historical implementation descriptions are labelled explicitly.

## Documented architecture

1. **Use the existing metadata structure.** For NTFS, initial indexing reads the Master File Table; subsequent updates consume the USN Journal. These are separate stages: USN is not the initial directory inventory. This avoids repeatedly traversing the ordinary directory/file API for the initial inventory. [Developer explanation, 2022](https://www.voidtools.com/forum/viewtopic.php?t=12371).

2. **Keep the working search catalog in memory.** Current documentation says the database is resident while Everything runs and is saved to disk daily and on exit. Names and paths are always indexed; size, dates, attributes and sort indexes have explicit memory costs. Unindexed properties are fetched when required. [Indexes](https://www.voidtools.com/en-us/support/everything/indexes/).

3. **Compile and execute the query over metadata.** In a 2013 technical explanation, the developer describes a lightweight MFT copy in memory and queries compiled to bytecode. This is historical evidence of the design, not proof of the exact current 1.5 instruction set or matching implementation. [Developer explanation, 2013](https://www.voidtools.com/forum/viewtopic.php?t=2030).

4. **Make layout and ordering part of performance.** The developer explains in February 2026 that searching in Name order benefits from more linear memory reads; matching full paths costs more; an empty/all-files query need not run a matching search. Expensive result sorting is a separate stage and benefits from optional fast-sort structures. [Developer discussion, 2026](https://www.voidtools.com/forum/viewtopic.php?p=80463).

5. **Support offline snapshots directly.** EFU file lists contain names and properties, can be opened directly or included in the index, and are searched/sorted without accessing the original files. This is the closest architectural analogue to our offline CTF use case. It establishes an approach, not CTF format support in Everything. [File Lists](https://www.voidtools.com/support/everything/file_lists/).

6. **Persist compact metadata.** An older, explicitly provisional DB-format description contains sorted folder/file records and parent-folder offsets with compact name encoding. It demonstrates historical compact representation and parent references. It must not be treated as a current supported binary-format specification or copied into our format implementation. [Historical DB description](https://www.voidtools.com/support/everything/db/).

The FAQ's rough figures are 35 MB RAM for 250,000 files and 100 MB for one million, with configuration-dependent costs. They are vendor estimates, not measurements on our host or promises for catalogs with descriptions and preserved opaque CTF records. The same FAQ describes roughly five seconds for a 250,000-file Windows installation and about a minute for one million files. This distinction does not excuse our multi-minute delay: we already have the metadata in a CTF file. [FAQ](https://www.voidtools.com/faq/).

Public sources do not establish every current internal data structure, query optimizer rule, SIMD implementation or persistence detail. This study does not claim those internals were inspected or reverse-engineered.

## Our confirmed critical path

The released native Open CTF flow waits for `ctf_sync` to produce a fully imported, indexed and validated SQLite mirror. In addition to record insertion, that path constructs secondary B-tree indexes and an FTS5 trigram index over a text projection containing name, full relative path, description and tags. It also validates the generated database and verifies source/cache identities before publishing the generation. All of this precedes search availability.

Moving that pipeline to another thread preserves UI event processing but does not give the user a usable search. Allowing only folder browsing during it also fails the owner's requirement. Faster warm SQLite queries do not measure opening an existing CTF or its first search.

The first repair defers secondary index construction and reduces a measured full-mirror build to approximately 196 seconds on the local Windows 11 workload. The direct read-only view then demonstrates that the same full CTF metadata can become browsable in 1.442 seconds, with a first nonempty directory page at 1.787 seconds. These are single service measurements with uncontrolled OS cache and exclude native startup/rendering. **No equivalent direct-search result has yet been measured or accepted.**

## Proposed architecture for WhereIsIt

Keep Rust and SQLite, but put a compact metadata snapshot and a common query representation on the interactive path:

```text
CTF bytes -> bounds/tree/section validation -> immutable metadata snapshot
                                         -> browse + search immediately
                                         -> optional accelerators/cache

SQLite user state -> notes/categories/preferences overlay -> query + export
```

The following are our proposed engineering choices, inferred from the study and our measurements; they are not claims about Everything's undocumented implementation:

- **Open is not import.** Explicit conversion to an editable SQLite catalog remains a separate operation. Opening/searching an existing CTF must not require that conversion to finish.
- **One metadata engine.** CTF, future NTFS inventory, and ordinary portable scans feed the same snapshot/query interface. MFT and USN are useful future Windows scanning adapters; they are unnecessary for opening a pre-existing CTF and do not solve this defect themselves.
- **One query grammar.** Parse Boolean expressions once into a shared typed representation. Use backends for snapshot execution and SQLite execution; regression-test their matching, Unicode, fields, wildcard, scope and pagination semantics against each other. Do not bolt on a filename-only fallback that silently changes advanced queries.
- **Compact rows.** Use IDs, parent references, numeric columns and shared string storage. Avoid a separately allocated full path and lowercased concatenated search document for every row. Reconstruct returned paths and evaluate path predicates lazily. Preserve original CTF opaque data without duplicating it into every derived representation.
- **Search is available with the snapshot.** Compile predicates once and evaluate metadata directly. Build small extension/scope/name/sort accelerators only where measurements justify them. A full trigram index is optional acceleration, not a prerequisite for an ordinary or Boolean search. A scan backend must still implement correct result ordering/counting/pagination and cancellation; merely stopping after the first arbitrary matches is insufficient.
- **SQLite has explicit responsibilities.** Retain transactional user notes, categories, workspaces, preferences and explicit native catalogs. Any cached search representation is disposable and rebuildable. Preserve source identity, fail-closed parsing, Unicode, no-clobber export and source-readonly guarantees.
- **Bound memory and result work.** A memory-resident design is only useful if allocation and layout are controlled. Page rows and UI results; avoid copying all matches or all paths for each keystroke. Measure the query, count and sort stages separately. Keep rapid-query cancellation outside the UI thread.
- **Account for verification separately.** Current pinned reads fully hash the source around reads, so first-response costs include more than matching. Measure those passes explicitly. Do not silently weaken the existing source-change contract to improve timings; any alternative snapshot-version/locking strategy needs an explicit decision and source-replacement regressions.

The owner subsequently directed implementation of this redesign. [ADR-0005](../decisions/ADR-0005-direct-ctf-metadata-search.md) records the changed CTF read/query boundary while preserving SQLite and ADR-0003 authority rules. The current preview-only candidate is an intermediate implementation and must not be released as the completed fix.

## Acceptance required before a hotfix

Measure exact-source release-mode builds on the full working copy, with no source mutation and no existing derived cache: time to usable native window, first directory page, first real search, absent-result full scan, Boolean/Unicode/name/path/description/extension/size and scoped queries. Include result/ordering/count parity with the existing engine, cancellation, rapid re-query, peak private memory and working set, repeat opening and persistence correctness.

Record OS cache conditions honestly and separate service, native GUI and installed-artifact measurements. The owner's Windows 10 report remains valid; local Windows 11 evidence does not retroactively certify Windows 10. Keep `stable_ready=false` until the corrected exact installer meets the revised gates. Published 1.1.0 tag and assets remain immutable. No private catalogs, paths, hashes or raw reports are included here.
