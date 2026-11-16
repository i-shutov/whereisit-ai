# WhereIsIt AI development

User priorities: native Windows + macOS application; fast compact metadata-only disk catalogs; indexed Boolean expressions and categories; offline duplicate candidates for movies/music; CSV and Parquet exports of branches/results; mandatory CTF import and export.

Keep original user catalogs read-only. Never silently omit unsupported CTF sections, overwrite source catalogs, execute cataloged files, or claim validated compatibility without evidence. Preserve Unicode names. No media thumbnail, content indexing, network, or hashing by default.

Core is Rust and SQLite; Windows UI uses Win32 native controls, macOS uses SwiftUI/AppKit and the C ABI. No web runtime. Long work runs outside UI threads. Use query pagination and streaming bulk operations.

Ownership during parallel work: rust_core owns crates/atlas-core; ctf_import owns crates/ctf-format and docs/CTF_FORMAT.md; native_desktop owns apps/windows and apps/macos; root owns integration, CLI/FFI, architecture, plan, QA, packaging. Coordinate changes outside your area before editing. Persist final artifacts is root's responsibility.

Meaningful tests must cover data integrity, parser bounds, query semantics, cancellation, exports, and offline access. Never place private source catalogs or software license files in source distribution or public CI. Record actual validated platforms separately from intended targets.

## 2026-11-16 target UI decision

**Target for new v1.0 desktop client is Rust/egui**, tracked by `docs/decisions/ADR-0004-egui-desktop.md`. Existing Win32/SwiftUI code must remain until parity and physical QA; PR #57 contains valuable native service/UI work. `apps/ui-prototype` (PR #58) is React/Express *in-memory design reference only*: no Rust/egui production code, no real CTF/SQLite, no verified video header plugin/scan latency. Never package it as the native product. No local-file deletion or executable cleanup export. Never infer backups, byte-exact equality or stream identity from metadata/sparse samples. Maintain `release/ACCEPTANCE.json: stable_ready=false` until measured independent physical acceptance and signing/notarization.
