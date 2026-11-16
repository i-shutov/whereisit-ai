# UI reference prototype (not the Rust/egui desktop app)

This folder contains a **React + TypeScript + Express** interactive design/reference implementation for a future cross-platform Rust/egui desktop client. It is deliberately **excluded from all native release packages** and is not connected to real Rust/SQLite/CTF services.

## Running locally

```sh
cd apps/ui-prototype
bun install --frozen-lockfile
bun run lint
bun run build
bun run dev
```

The demo binds to **127.0.0.1:3000 only**. Do not expose it to the internet. It uses generated in-memory sample disks and filenames; changing disk properties or simulating scans does **not** change a real catalog. Browsers may optionally read user-selected local file metadata for a **review-only** comparison. The demo must never be used to certify a backup.

## Safety limits

- No original WhereIsIt `.CTF` compatibility evidence is provided here; the CTF inspector is illustrative.
- No real physical-USB detection is performed by this simulator. OS mount paths, SHA pins, latency samples and volumes are synthetic.
- File size / filename / duration / codec / sparse fingerprints yield **candidate** matches, never proof of identical bytes, same stream or existing backups.
- UI offers a CSV review list only. It neither deletes real files nor emits executable cleanup scripts. The folder picker requests **read-only** access.
- The proposed `+16 B/file` and `seconds per 5 TB` performance estimates have not been measured on hardware.
- Web dependencies and `@google/genai` (currently unused) must not be bundled into the native Rust/egui product.

## Engineering handoff

See [`docs/UX_EGUI_VIDEO_CATALOG_2026-11-16.md`](../../docs/UX_EGUI_VIDEO_CATALOG_2026-11-16.md). Implementation target is a separate `apps/desktop-egui` Rust crate using the existing `atlas-service` APIs and preserving the existing Win32 and SwiftUI clients until full feature parity and physical QA.
