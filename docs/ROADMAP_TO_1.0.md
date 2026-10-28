# WhereIsIt AI · Roadmap to installers and v1.0

**M0 · GitHub baseline (BLOCKING):** import local Atlas Catalog v0.1 source into main without overwriting this checkpoint, rename user-facing product to WhereIsIt AI, enable CI. Source lives in a locally supplied Git bundle/ZIP; do not reinvent it from README.

**M1 · CTF:** decode/migrate original categories, flags and groups; golden fixtures for Unicode, long names, unknown fields and mutations; use original WhereIsIt to validate a newly written CTF. Lossy export requires explicit opt-in, never silent loss.

**M2 · Storage and search:** same-dataset A/B benchmark SQLite / optimized schema / DuckDB-Parquet / Polars, 3,046,027 rows; categories, Boolean search, suffix/prefix/substring and duplicates. SQLite remains default until measured evidence supports changing it. Benchmark cold and warm, RSS and disk size, correctness, migrations/rollback. Broad-category and all-file duplicate first run are known slow spots.

**M3 · Native UX:** compact 1366×768 layout, virtual list, Disk column in global results, progress/cancel, persistent settings, real Windows 10/11 and Mac Intel/Apple Silicon smoke.

**M4 · Installers:** Inno Setup Windows per-user setup/Programs entry/uninstall; macOS .pkg and .dmg on both architectures; SHA256 and reproducible CI artifacts. Signed final releases require Windows code-signing certificate and Apple Developer ID/notarization secrets configured out of band.

**M5 · End-to-end QA:** 44 volumes/3M entries real-data checks, CTF roundtrip and modified-file acceptance, CSV/Parquet exact equality, dates/Unicode/big files, crash recovery, install/upgrade/uninstall and privacy review.

**M6 · Release:** green Windows/Intel Mac/Apple Silicon CI, signed/notarized installers, release notes, SHA256 checksums, tagged immutable source and QA checkpoint. Never label untested candidates as verified installers.

## Work ownership

- CTF agent (High reasoning): crates/ctf-format and CTF docs.
- Storage agent (High reasoning): crates/atlas-core and benchmarks; no edits to CTF module.
- Windows agent (medium/high): apps/windows, installers/windows, corresponding CI.
- macOS agent (High): apps/macos, installers/macos, corresponding CI.
- QA agent (medium): docs/QA*, independent tests, release acceptance.
- MAIN integrator: service/API compatibility, multi-platform CI, merges, source of truth, checkpoints and releases.

All agents: separate agent/* branches, one Issue+PR, tests and before/after metrics, no user data in the repository, no unsupported compatibility claims.

## Measurable release goals (targets, not achieved values)

- Warm selective search p95 ≤150ms, large inherited category ≤500ms after optimization; search pages limited to 100–250 rows.
- Movie/music duplicate first grouping ≤2s, cached page ≤100ms; full-library aggregation must be cancellable and show progress.
- Measure and aim for ≥30% SQLite footprint savings vs the prior 2.16GB compact copy **without losing fidelity or speed**; migrations must have rollback.

## Checkpoints

After every milestone: reviewed PR, CI artifact, `CHECKPOINT.md` with commit SHA/QA status, manifest SHA256, no personal CTF or SQLite uploads.