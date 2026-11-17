# WhereIsIt AI / Desktop egui — R2 native preview

This is a **real Rust/egui executable**, not the PR58 React UI. The visual and behavioral
authority remains `apps/ui-prototype` from PR #58. This R2 slice uses the existing
`atlas-service` Rust API for real **SQLite v3** operations, not a simulated JS backend.

## Build and run

Pinned UI: eframe/egui/egui_extras 0.37.0 on Rust 1.100.0. Core libraries are upgraded together and tested; Cargo.lock and license inventory are regenerated in isolated CI.
The package is a member of the root workspace; `Cargo.lock` is generated/extended on
the integration branch by an audited lockfile CI bootstrap before locked CI is accepted.

```sh
cargo run --locked -p atlas-desktop-egui -- --catalog /path/to/existing.atlas.sqlite
cargo test --locked -p atlas-desktop-egui
cargo check --locked -p atlas-desktop-egui
```

Initial native vertical: global search, 60+-disk register, filter by selected disk, paged
search results, read-only inspector, navigation to a recorded directory and back, metadata
copy candidates and verified source errors. Open existing / New empty SQLite in separate
worker threads. No fake USB connected badges, full-video hashes, live filesystem content
access or deletion. **Physical OS USB pairing and CTF workflows are queued for later R3/R4
after the app's native foundation is assembled.** Do not call this release-ready or remove
legacy Win32/SwiftUI implementations.

Native GUI QA for Windows/Mac ARM+Intel is mandatory even if cross-platform CI builds pass.
Original user CTF sources are read-only and must never be committed or uploaded to CI.

## R3: safe native SQLite export vertical

The native desktop can now export the **whole current search** (including disk
scope and Boolean query) or **the selected folder and its descendants** to a
brand-new CSV, JSON or Parquet file via the existing Rust `atlas-service`
pipeline. The export never overwrites existing output files or the active
SQLite/WAL/SHM. It is intentionally unavailable inside the metadata-only
copy-candidate screen rather than silently exporting an unrelated search.
The UI disables concurrent catalog operations while a background worker runs;
real progress/cancellation and physical GUI tests remain separate gates.

`Route::export_request` has contract tests for exact scope and no-clobber.
There is no CTF write integration and no deletion workflow in this release slice.
