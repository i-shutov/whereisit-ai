# WhereIsIt AI / Desktop egui — R2 native preview

This is a **real Rust/egui executable**, not the PR58 React UI. The visual and behavioral
authority remains `apps/ui-prototype` from PR #58. This R2 slice uses the existing
`atlas-service` Rust API for real **SQLite v3** operations, not a simulated JS backend.

## Build and run

Pinned UI: eframe/egui 0.32.3 (declared MSRV Rust 1.85); project Rust remains 1.90.0.
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
