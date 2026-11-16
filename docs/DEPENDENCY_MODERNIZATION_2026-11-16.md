# Dependency modernization — 2026-11-16

Owner requested **latest available stable dependencies throughout the product**, not legacy pins.

## Target stable versions (observed 2026-11-16)

| Component | From | To | Notes |
|---|---:|---:|---|
| Rust toolchain | 1.90.0 | 1.100.0 | Whole workspace; CI pins exact toolchain |
| eframe / egui_extras | 0.32.3 (draft) | 0.37.0 | Latest published stable, same version |
| rusqlite | 0.39.0 | 0.43.0 | Bundled SQLite 3.54.1 via libsqlite3-sys 0.40.0 |
| parquet | 56 | 60.1.0 | Breaking major, export parity regression required |
| sha2 | 0.10 | 0.11.1 | Breaking RustCrypto API; preserve true SHA-256 |
| csv | 1.x | 1.4.0 | Exact published stable |
| chrono | 0.4.x | 0.4.45 | Exact published stable |
| serde, serde_json, thiserror, tempfile, walkdir and transitive crates | existing | highest compatible at lock refresh | `cargo generate-lockfile` resolves, logs actual versions |
| windows-sys | 0.61.2 | 0.61.2 | Already latest verified stable |
| React/TypeScript/Vite and other UI prototype dependencies | existing | `bun update --latest` | Require typecheck and production build |

## Rules

- Update lockfile and full license inventory atomically; `THIRD_PARTY_NOTICES.md` SHA must match `Cargo.lock`.
- No unreviewed data formats or user SQLite writes in dependency migration. Exact-source CTF and full CSV/Parquet parity must be revalidated.
- Release blockers remain unsigned packages, original CTF tests, real native UI QA, and performance SLO; dependency update never implies release acceptance.
- If any new major dependency fails its source/ABI contract, record the precise regression and repair code rather than silently falling back or weakening tests.
- `.github/workflows/egui-lock-prepare.yml` may push only to the named isolated feature branch, after reading branch head; no forced updates or stable releases.
