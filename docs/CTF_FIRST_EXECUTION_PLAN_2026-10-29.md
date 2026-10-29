# WhereIsIt AI — CTF-first: execution plan in five large blocks

**Status:** plan approved for staged execution, not a claim of completion.
**Baseline:** `v1.0.0-rc.1`, `main` at `7b771385cb15545861156d036dd5433f8ff590fc`.
**Companion draft:** [PR #17](https://github.com/i-shutov/whereisit-ai/pull/17).
**Inputs:** user-provided `e-books.CTF` (4,488,883 bytes) and `Mp3-60Gb.CTF` (2,420,847 bytes). These are **private**; never commit or upload to this public GitHub repository, its Actions or its release assets.

## Block 1 — Real CTF characterization and reproducible baseline (P0)

Work as one large research package: full binary structure and parser diagnostics, raw SHA-256/volume/item counts, unknown fields, encoding, hierarchy, descriptions, dates, rare/dense payload and real-world distributions. Run unchanged `CTF → parse → write → CTF` and `CTF → SQLite → export → CTF`; check exact byte identity, record parity, errors and loss-prevention. Benchmark first import, repeated open, RAM indexing, current SQLite, columnar projection, duplicates, 13+ Boolean/path/category/search cases, deep pagination, first-page versus count, cold/warm startup, peak/current RSS and output file size. Keep the originals read-only.

**Current blocker:** uploaded inputs have been identified and materialized in the assistant file layer, but the local Python/container execution endpoint returns `ClientError` even for simple file checks. **This is a tooling blocker, not a parser result.** Until restored, use the already compiled [local acceptance runner from Actions 40829350733](https://github.com/i-shutov/whereisit-ai/actions/runs/40829350733), executed **on the owner's workstation** with those two CTF paths. Its standard output is aggregate JSON omitting file names/paths, and raw inputs stay local. The test runner does not establish interoperability with the original application. Do not claim measurements until the actual runner output is verified. A new job may compile a runner, but should not request uploading original CTF into public CI.

**Exit gate:** two real-input per-file reports with raw input SHA-256, explicit parser status, roundtrip equality, exact counts/coverage and performance/memory table; known CTF variants and limitations; immutable JSON/Markdown aggregate-only artifacts. On failure, correct the parser before moving to implementation.

## Block 2 — CTF-first authoritative source and hash-bound sidecars (P0)

Bring [PR #17](https://github.com/i-shutov/whereisit-ai/pull/17) from prototype to production coordinator and native API. CTF stays an independently editable original; derived caches are immutable and keyed by `SHA256(source bytes)+size+parser/schema/normalizer/sort/options(+overlay revision)`. A filename/path is a locator and UI label, **not** a validity or identity key. Byte-identical renamed/moved CTF may reuse derived caches, while personal annotation identities must not silently merge.

Detect independent external edits on reopen/rebuild, verify content for every cache reuse (not mtime/size alone), and never treat an index as current after file content changes. External change must trigger invalidation and bounded rebuild; keep a snapshot for in-flight readers. Publish only complete generations after checksums. Require resource limits and avoid traversing untrusted cache manifests into arbitrary paths. Use a shared cache root outside of original catalog directories if desired.

**Exit gate:** Windows/macOS/Linux CI, same-size/restored-mtime edit, rename, replace, corrupt/truncated cache, mismatched parser version, concurrent readers/builders, and source-change during capture; no source rewrite, no exposure of private paths. Measurable cache-hit/rebuild/open times; GUI exposes current source digest and cache state.

## Block 3 — Production high-performance RAM/mmap engine (P1)

Turn prior research into a complete Rust backend, not 13 hard-coded predicates. Share the query AST with the tested SQL backend; implement Unicode case folding, text substring/trigram and glob semantics, path-boundary conditions, numeric constraints, inherited categories, Boolean set algebra, stable sort, keyset page, and duplicate candidate filters. Hot entry records are compact; common names/descriptions interned; directory hierarchy avoids full-path repetition. Rare metadata and opaque CTF sections hydrate only on detail/export. Use packed vectors, posting lists or bitmaps chosen by selectivity, and a versioned persisted/mmap-able index.

**Bench gate:** replay ALL supported query grammar and randomized metamorphic queries on both real CTFs; compare **complete ordered ID sets** and every returned field against the SQL reference, not only first 250 or counts. Profile 100k, 3M repeated-name, 3M unique-name and both real datasets; RAM cap at 8/16 GB, multiple open catalogs, first-run/warm, first-page/COUNT, short queries and broad paths. Keep a validated SQLite fallback until parity and memory limits meet acceptance.

## Block 4 — Independent edits, durable native data and CTF checkpoints (P1)

Native-specific tags, categories, notes, stable IDs and edit journal are **authoritative**, not disposable derived search cache, because the current CTF writer cannot encode all of them. Bind edit records to an explicit source generation and durable identity. When external WhereIsIt edits the source, detect the hash change, rebuild indexes and reconcile native annotations with evidence-based matching. Identical names, rename/move, duplicates, insertions and deletion must not produce silent wrong matches; ambiguity is an explicit conflict. Never silently overwrite an externally changed CTF.

CTF exports write new generation files, verify decoded and unknown raw bytes, source hash and native-state policy, then atomically publish a versioned manifest pointer; retain previous generation and backups. Work out Windows/macOS fsync and rename semantics, crash recovery, concurrent external editing and error injection. Parquet and index are derived from the manifest's committed generation and can be recreated.

**Exit gate:** externally edited CTF plus native annotations does not lose data; clean three-way replay and conflict cases, source snapshot protection, interrupted checkpoint recovery, original WhereIsIt acceptance of **modified** CTF, and no lossy export without explicit warning/opt-in.

## Block 5 — Native integration, installer QA and release (P2)

Expose engine mode, source hash, index-build/cache status, last sync and conflicts through the existing service/API and compact native Win32/macOS UIs. Implement progress/cancel, fast first-page responses, background count, bulk exports and disk/category navigation. Confirm clean install, upgrades, uninstall while preserving original CTF and native journal on Windows 10/11 x64 and macOS Intel/Apple Silicon. Update bundled SQLite to a WAL-reset-fixed version before stable release; see [Issue #16](https://github.com/i-shutov/whereisit-ai/issues/16).

**Exit gate:** all-platform CI, true local real-CTF acceptance, signed/notarized builds if credentials are available, reproducible SHA256/artifact manifest, release QA and checkpoint. Ship candidates as RC until all high-severity blockers are resolved.

## Workflow and checkpoint contract

- **Only one integration owner.** Implementers use isolated branches and PRs; never interpret “branch created” or “CI job started” as proof that independent coding agents actually ran.
- After **each block**, publish an `ACCEPTANCE.md` with commit SHA, source/data SHA, environment versions, exact commands, metrics JSON, pass/fail matrix, blocked items, known regressions and next-block dependencies. Link GitHub Actions jobs and immutable artifacts. Use checkpoint tags only for reviewed, accepted work.
- Do not ship the user-supplied private CTF or generated private SQLite/Parquet/RAM dumps via the public repository. Share only aggregate metrics and optional redacted synthetic fixtures.
- Keep `v1.0.0-rc.1` and its test reports unchanged for performance reproducibility. Do not force a production architecture switch based on one synthetic benchmark.
- If a block is blocked by the local executor, continue independent design/tests on **synthetic data**, and record the real-data gate as unfulfilled. Synthetic CI success cannot substitute for actual source-file testing.

## Recommended order

**B1 → B2 → B3 → B4 → B5**, with B2 contract tests and B3 prototype experiments able to progress in parallel while B1 is blocked. The immediate real-file priority is B1; the next production-ready merge candidate is PR #17 only after review and tested integration, not merely the test-only source identity module.
