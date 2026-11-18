# PR #58 → native egui parity matrix

Baseline main `ddd9215b0926c5e74482a3cbf7f10c229a66fb88`; PR #58 reference code `apps/ui-prototype` / safety-corrected `202970d7`, contract `docs/UX_REFERENCE_CONTRACT_PR58_2026-11-16.md`. The prototype is an in-memory UX simulation; its physical state, scan latency, hashes, video fields and analytics are not production evidence. This is a partial audit, not a parity certification. PASS below refers only to the explicitly described case; PARTIAL includes missing visual, interaction or error-state evidence. USB hardware is outside this block.

| Reference screen / expected state | Native implementation and service | Actions, errors, navigation/persistence | Status / priority |
|---|---|---|---|
| Header, tabs and global search | `menu`, `search_toolbar`; SQLite `search_fast` | Ctrl+F, Enter, global query exercised on generated catalog; errors shown in inspector; prototype workspace tabs absent | PARTIAL / P1 |
| 60+ compact disks, capacity and free space | `disk_sidebar`, `volumes`, sidecar preferences | 62 synthetic volumes shown; two lines per volume, no free-space bar, no sorting; label filter exists | PARTIAL / P1 |
| Disk filter and sort | Sidebar name/id/sticker filter; volume metadata | Filter code exists; sort controls absent; no hardware state inferred | PARTIAL / P1 |
| Virtual results table and pages | `egui_extras::TableBuilder::body.rows`, service limit 250 | First 250 and enabled next shown; second-page GUI action not verified in this block; FTS/Boolean supported by service, not all combinations GUI-tested here | PARTIAL / P1 |
| File inspector and disk/folder navigation | selected `Entry`; `children` and `copy_candidates` | Synthetic file → containing folder → Alt+Left restored selected item and query on current page; root/large folder and scroll restoration need testing | PARTIAL / P0 |
| Back preserves scope, selection, scroll | history stores route, offset, selection and selected volume | Back and selection verified on one result; selection restoration only when row is present in returned page; scroll and asynchronous error handling remain to be evaluated | PARTIAL / P0 |
| Disk properties, F2, sticker, note | `disk_preferences_*`, `physical_shelf_labels`, guarded legacy fallback | PR #68 merge GUI evidence covers Unicode save/reopen and clear; this block did not repeat restart/persistence | PARTIAL / P1 |
| Category browsing/editing | category service exists | No corresponding egui UI in inspected client | MISSING / P1 |
| Metadata duplicate candidates | `copy_candidates`, exact normalized name+size | Service other-disk filter now requested before pagination; two-disk fixture service result verified, GUI end-to-end after fix not yet verified; no claim of byte equality | PARTIAL / P0 |
| Scan, progress, Esc and rollback | `scan`, `scan_progress`, `scan_cancel` | Prior merged exact-head GUI observed progress and Esc with unchanged catalog; current changes did not touch scan; repeat at this head still pending | PARTIAL / P0 |
| Export CSV/JSON/Parquet, no overwrite | `export_*`, scoped search/folder, `overwrite:false` | Prior hosted bundle passed synthetic runner; current-head GUI tests and screenshots pending; candidate export disabled | PARTIAL / P1 |
| RU/EN, light/dark | language flag and egui visuals | Controls exist; neither persistence nor full side-by-side theme/localization comparison verified here | PARTIAL / P1 |
| Keyboard control | Ctrl+F, Ctrl+L, Alt+Left, F2, Alt+Enter, Esc | Keyboard search/result/inspector/Back exercised; mouse timing/path blocked by WGC geometry; full traversal remains | PARTIAL / P1 |
| Archive slices, sunburst | `ArchiveSlicesView`, `VolumeSunburstView` in prototype | No corresponding native screen or live aggregate binding | MISSING / P1 |
| Local analyzer and review-only comparison | `LocalDiskAnalyzer` prototype only | No native equivalent; no backup or delete claim | MISSING / P1 |
| Video-header plugins, sparse/content hashes and four video match modes | Prototype simulations | No native validated parser/measurements shown here; speculative claims deferred beyond current 1.0 acceptance, not presented as implemented | MISSING / P2 |
| Original CTF compatibility, OS identity / physical USB | Backend components and prototype mock | Not tested here; no physical test and no compatibility claim | BLOCKED / P1 |

The generated screenshot atlas documents only real egui states; the prototype could not be executed in this environment without its unavailable Bun/dependencies. Paired visuals, DPI matrix and measured pointer movement remain open. See `PR58_EGUI_VISUAL_REVIEW.md` and `PR58_EGUI_DEFECTS.md`.
