# PR #58 visual review — partial evidence

Windows 10 Pro, egui debug build with opt-in `qa-screenshots`, synthetic SQLite and isolated sidecar. No system DPI or resolution change. The renderer supplied the 1320×760 client image directly; it is not a desktop or window-compositor crop. All published images were inspected: only generated fixture data appear. The QA feature is disabled by default; build with `cargo build --locked -p atlas-desktop-egui --features qa-screenshots`, set `WHEREISIT_QA_SCREENSHOT_DIR` to an existing absolute private directory, and use F12. Each PNG is created with `create_new`; the renderer captures application contents only, so do not use a real catalog when publishing.

| State | Native evidence | Reference / limitations |
|---|---|---|
| 62 generated volumes, page 1 (1–250) | [egui](pr58-egui-atlas/egui-62-disks-first-page.png) | PR58 pair not yet captured |
| Global search, two synthetic disks | [egui](pr58-egui-atlas/egui-global-shared-film.png) | query `shared-film`, two records; exact-name/size candidates are not byte evidence |
| Selected result / inspector | [egui](pr58-egui-atlas/egui-inspector.png) | `clip-001` generated name, no private data |
| Containing folder | [egui](pr58-egui-atlas/egui-containing-folder.png) | selected file restored on the first folder page; Back was verified through accessibility state |
| Other-disk candidate | [egui](pr58-egui-atlas/egui-other-disk-candidate.png) | selected #61, returned #62, explicitly not byte verified; Back restored #61 |
| 300 results, pagination | [1–250](pr58-egui-atlas/egui-300-first-page.png), [251–300](pr58-egui-atlas/egui-300-second-page.png) | Next activated through keyboard; no pointer distance measured |
| File beyond page 1 | [egui](pr58-egui-atlas/egui-large-folder-selection.png) | `page-270` selected on folder page 251–301 and Back verified |
| Scan | [dialog](pr58-egui-atlas/egui-scan-dialog.png), [invalid root](pr58-egui-atlas/egui-scan-invalid-root.png), [cancelled](pr58-egui-atlas/egui-scan-cancelled.png) | progress 51,920 observed in accessibility output, but not captured in a PNG |
| Local search, disk properties, scan dialog/progress/cancel/error, export/overwrite | NOT CAPTURED | prior PR #68 evidence does not substitute for an atlas of this build |
| PR58 paired screens and analytical screens | BLOCKED | reference app source reviewed, runtime dependencies not installed; no paired screenshots claimed |

Update 2026-11-19: the reference search workspace was run and visually observed using an isolated pnpm install without the Bun lockfile; no reference PNG was saved. The source and runtime are a simulated UX reference and not evidence of real disk data or analytics.

Observed navigation A/B: Ctrl+F, type `clip-001`, Enter; reverse Tab to result, Enter; reverse Tab to “Open containing folder”, Enter; Alt+Left. Search, selected record and inspector returned. Pointer/click count and reaction times were not measured. Windows Graphics Capture still timed out, and mouse input reported unavailable geometry; two automation calls stalled. No resolution workaround was applied. This renderer capture solves the PNG contamination problem, not the mouse-control problem or visual parity.
