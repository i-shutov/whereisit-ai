# PR #58 → native egui parity matrix

Baseline main `ddd9215b0926c5e74482a3cbf7f10c229a66fb88`; PR #58 reference code `apps/ui-prototype` / safety-corrected `202970d7`, contract `docs/UX_REFERENCE_CONTRACT_PR58_2026-11-16.md`. The prototype is an in-memory UX simulation; its physical state, scan latency, hashes, video fields and analytics are not production evidence. This is a partial audit, not a parity certification. PASS below refers only to the explicitly described case; PARTIAL includes missing visual, interaction or error-state evidence. USB hardware is outside this block.

| Reference screen / expected state | Native implementation and service | Actions, errors, navigation/persistence | Status / priority |
|---|---|---|---|
| Header, tabs and global search | `menu`, `search_toolbar`; SQLite `search_fast` | Ctrl+F, Enter, global query exercised on generated catalog; errors shown in inspector; prototype workspace tabs absent | PARTIAL / P1 |
| 60+ compact disks, capacity and free space | `disk_sidebar`, `volumes`, sidecar preferences | 62 synthetic volumes shown; two lines per volume, no free-space bar, no sorting; label filter exists | PARTIAL / P1 |
| Disk filter and sort | Sidebar name/id/sticker filter; volume metadata | Filter code exists; sort controls absent; no hardware state inferred | PARTIAL / P1 |
| Virtual results table and pages | `egui_extras::TableBuilder::body.rows`, service limit 250 | GUI moved from 1–250 to 251–300 for 300 generated files; FTS/Boolean supported by service, not all combinations GUI-tested here | PARTIAL / P1 |
| File inspector and disk/folder navigation | selected `Entry`; `children_focused` and `copy_candidates` | Synthetic file on second folder page → containing folder → Alt+Left restored selection and query; nested/error cases remain | PARTIAL / P0 |
| Back preserves scope, selection, scroll | history stores route, page, selection, selected volume and scroll offset | Back and selection verified; scroll restoration implemented and unit checked but not measured in a scrolled GUI session | PARTIAL / P0 |
| Disk properties, F2, sticker, note | `disk_preferences_*`, `physical_shelf_labels`, guarded legacy fallback | PR #68 merge GUI evidence covers Unicode save/reopen and clear; this block did not repeat restart/persistence | PARTIAL / P1 |
| Category browsing/editing | category service exists | No corresponding egui UI in inspected client | MISSING / P1 |
| Metadata duplicate candidates | `copy_candidates`, exact normalized name+size | GUI and service other-disk result verified on two synthetic disks; Back restored original selection; no claim of byte equality | PARTIAL / P0 |
| Scan, progress, Esc and rollback | `scan`, `scan_progress`, `scan_cancel` | Continuation GUI observed 51,920 entries and cancelled; 62 volumes/482 entries and quick_check=ok afterwards; progress PNG and exact-head hosted GUI retest pending | PARTIAL / P0 |
| Export CSV/JSON/Parquet, no overwrite | `export_*`, scoped search/folder, `overwrite:false` | GUI CSV/no-clobber, JSON and Parquet passed on 482 synthetic entries; JSON and Parquet independently counted; screenshots with private output paths withheld; candidate export disabled | PARTIAL / P1 |
| RU/EN, light/dark | language flag and egui visuals | Light toggle verified in current session; startup defaults are Russian/dark, preference persistence is not implemented; full bilingual comparison remains open | PARTIAL / P1 |
| Keyboard control | Ctrl+F, Ctrl+L, Alt+Left, F2, Alt+Enter, Esc | Keyboard search/result/inspector/Back exercised; mouse timing/path blocked by WGC geometry; full traversal remains | PARTIAL / P1 |
| Archive slices, sunburst | `ArchiveSlicesView`, `VolumeSunburstView` in prototype | No corresponding native screen or live aggregate binding | MISSING / P1 |
| Local analyzer and review-only comparison | `LocalDiskAnalyzer` prototype only | No native equivalent; no backup or delete claim | MISSING / P1 |
| Video-header plugins, sparse/content hashes and four video match modes | Prototype simulations | No native validated parser/measurements shown here; speculative claims deferred beyond current 1.0 acceptance, not presented as implemented | MISSING / P2 |
| Original CTF compatibility, OS identity / physical USB | Backend components and prototype mock | Not tested here; no physical test and no compatibility claim | BLOCKED / P1 |

The generated screenshot atlas documents only real egui states; the prototype could not be executed in this environment without its unavailable Bun/dependencies. Paired visuals, DPI matrix and measured pointer movement remain open. See `PR58_EGUI_VISUAL_REVIEW.md` and `PR58_EGUI_DEFECTS.md`.
