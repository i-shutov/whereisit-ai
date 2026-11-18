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

## R4: real metadata scan, progress and transactional cancellation

The PR58-first native desktop now scans a user-selected **real folder or disk root**
into a **new SQLite volume snapshot**, using the existing `atlas-service` metadata-only
`scan` operation. It never opens source file contents or calculates hashes.

During the scan, the UI remains responsive; a separate short-lived service worker
polls the actual recorded entry count. A visible **Cancel scan** action becomes
available only after the service confirms that the job was registered. Cancellation
is sent independently of the scanning worker. The backend owns atomic rollback:
a cancelled scan does not persist a partial volume snapshot. The SQLite catalog
must be outside the selected scan tree to avoid indexing its own database/WAL.

This slice has not yet passed real Windows/macOS manual GUI acceptance. It does
not perform physical USB identity pairing, CTF parsing or content-based matching.

## Private shelf sticker labels (R6)

The egui disk register and properties read and write operator-authored
shelf labels in the existing `physical_shelf_labels` sidecar table. For v3
catalogs, the `disk_preferences` service saves the sticker and extended user
properties atomically with stale-edit checks. For supported older catalogs,
a guarded shelf-only fallback validates the catalog and protects the source
from sidecar aliasing. Labels use the **same private sidecar
location and SQLite-scoped catalog identity** as the legacy native clients:
`LOCALAPPDATA/AtlasCatalog/physical-volume-bindings.sqlite` on Windows,
`~/Library/Application Support/AtlasCatalog/physical-volume-bindings.sqlite`
on macOS. Explicit Save or clearing the text changes only the sidecar, **never
the source SQLite catalog or CTF**.

The disk list is filterable by the shelf sticker; shared sticker collisions
are displayed as warnings with other catalog disk IDs. A sticker is purely a
human retrieval hint and never proves the presence or identity of a USB volume.
Backend-generated collision checks remain the authority.

The adapter includes synthetic service tests for save/read/collision/clear and
ensures the original catalog bytes are not changed. Physical OS USB pairing,
original-app CTF compatibility and manual Win/Mac GUI acceptance remain outside
this block.
