# PR #58 visual review — partial evidence

Windows 10 Pro, egui debug build with opt-in `qa-screenshots`, synthetic SQLite and isolated sidecar. No system DPI or resolution change. The renderer supplied the 1320×760 client image directly; it is not a desktop or window-compositor crop. All published images were inspected: only generated fixture data appear. The QA feature is disabled by default; build with `cargo build --locked -p atlas-desktop-egui --features qa-screenshots`, set `WHEREISIT_QA_SCREENSHOT_DIR` to an existing absolute private directory, and use F12. Each PNG is created with `create_new`; the renderer captures application contents only, so do not use a real catalog when publishing.

| State | Native evidence | Reference / limitations |
|---|---|---|
| 62 generated volumes, page 1 (1–250) | [egui](pr58-egui-atlas/egui-62-disks-first-page.png) | PR58 pair not yet captured |
| Same register in light theme | [egui](pr58-egui-atlas/egui-light-62-disks.png), [English/light after restart](pr58-egui-atlas/egui-en-light-after-restart.png) | Keyboard toggle and isolated-settings restart passed; no PR58 pair |
| Global search, two synthetic disks | [egui](pr58-egui-atlas/egui-global-shared-film.png) | query `shared-film`, two records; exact-name/size candidates are not byte evidence |
| Selected result / inspector | [egui](pr58-egui-atlas/egui-inspector.png) | `clip-001` generated name, no private data |
| Containing folder | [egui](pr58-egui-atlas/egui-containing-folder.png) | selected file restored on the first folder page; Back was verified through accessibility state |
| Other-disk candidate | [egui](pr58-egui-atlas/egui-other-disk-candidate.png) | selected #61, returned #62, explicitly not byte verified; Back restored #61 |
| 300 results, pagination | [1–250](pr58-egui-atlas/egui-300-first-page.png), [251–300](pr58-egui-atlas/egui-300-second-page.png) | Next activated through keyboard; no pointer distance measured |
| File beyond page 1 | [egui](pr58-egui-atlas/egui-large-folder-selection.png) | `page-270` selected on folder page 251–301 and Back verified |
| Scan | [dialog](pr58-egui-atlas/egui-scan-dialog.png), [invalid root](pr58-egui-atlas/egui-scan-invalid-root.png), [cancelled](pr58-egui-atlas/egui-scan-cancelled.png) | progress 51,920 observed in accessibility output, but not captured in a PNG |
| Local search, disk properties | [disk search](pr58-egui-atlas/egui-disk-search.png), [properties](pr58-egui-atlas/egui-disk-properties.png) | Newly captured; properties image shows the upper scroll region only |
| Progress, export/overwrite | NOT CAPTURED | GUI actions previously tested as noted in checkpoint; a paired image is still missing |
| PR58 paired screens and analytical screens | PARTIAL | See paired continuation below; native analytical screens remain absent |

Update 2026-11-19: the reference search workspace was run and visually observed using an isolated pnpm install without the Bun lockfile; no reference PNG was saved. The source and runtime are a simulated UX reference and not evidence of real disk data or analytics.

Further capture attempt: an isolated prototype copy used `html2canvas` to render the viewport to a loopback-only PNG sink. It failed on unsupported `oklab`/`oklch` colors; a conversion attempt made the browser tab unresponsive. No resulting PNG is accepted as evidence. This is separate from the working native egui renderer capture and does not prove reference parity.

## Paired continuation — 2026-11-19

The earlier capture failure was bypassed: an in-app browser screenshot of the actual prototype viewport was sent to a temporary loopback-only form and saved, then converted from the browser-returned JPEG to PNG. It is a browser compositor image, not an html2canvas re-render; JPEG compression is present. PNGs were inspected for private paths and foreign windows. The prototype runs from an isolated copy with pnpm-resolved dependencies rather than its Bun lockfile, and contains mock catalog data; a simulated scan in this session changed its in-memory count from 60 to 61. Native images use separate synthetic SQLite fixtures and 1320×760 renderer output; reference is 1265×712. Thus these are corresponding *states*, not pixel-accurate or same-data comparisons.

| State | PR58 reference | Native egui | Observed difference |
|---|---|---|---|
| Main and 60+ disks | [60-disk mock](pr58-egui-atlas/pr58-60-disks.png) | [62 volumes](pr58-egui-atlas/egui-62-disks-first-page.png) | Prototype has compact one-line disks, free-space bars, workspace tabs and video columns; egui has a two-line registry and metadata-only columns. |
| Global search and inspector | [Interstellar mock](pr58-egui-atlas/pr58-global-search-inspector.png) | [shared-film](pr58-egui-atlas/egui-global-shared-film.png), [inspector](pr58-egui-atlas/egui-inspector.png) | Different fixture/query; video stream fields in prototype are simulated and not native facts. |
| Scoped search | [empty result on mock disk 1](pr58-egui-atlas/pr58-disk-search-empty.png) | [one result on synthetic disk 62](pr58-egui-atlas/egui-disk-search.png) | Scope state is comparable, result counts and fixtures are different. |
| 250+ results | [251–500 of 1046 mock entries](pr58-egui-atlas/pr58-pagination-second.png) | [251–300 of 300](pr58-egui-atlas/egui-300-second-page.png) | Both show a second page, with different data and totals. |
| Properties/sticker | [mock disk](pr58-egui-atlas/pr58-disk-properties.png) | [synthetic disk, upper section](pr58-egui-atlas/egui-disk-properties.png) | egui separates local properties from source catalog; image omits lower scroll region. |
| Scan setup | [mock dialog](pr58-egui-atlas/pr58-scan-dialog.png) | [native dialog](pr58-egui-atlas/egui-scan-dialog.png) | Prototype plugin/timing claims are mock; native path uses actual metadata-only backend. |

The requested pairs for visible progress, Esc cancellation, scan error, export and overwrite refusal are still missing; do not mark the atlas complete. The prototype's background scan is simulated and completed before a progress screenshot could be captured. Local GUI RU → EN → RU now immediately translated an existing `1–250` status; [EN frame](pr58-egui-atlas/egui-language-switch-en.png).

Observed navigation A/B: Ctrl+F, type `clip-001`, Enter; reverse Tab to result, Enter; reverse Tab to “Open containing folder”, Enter; Alt+Left. Search, selected record and inspector returned. Pointer/click count and reaction times were not measured. Windows Graphics Capture still timed out, and mouse input reported unavailable geometry; two automation calls stalled. No resolution workaround was applied. This renderer capture solves the PNG contamination problem, not the mouse-control problem or visual parity.
