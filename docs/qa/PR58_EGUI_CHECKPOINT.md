# PR58 egui parity checkpoint

Base `ddd9215b0926c5e74482a3cbf7f10c229a66fb88`; branch `qa/pr58-egui-parity-win10-20261118`. PR #68 was already merged and was not reopened. Work uses generated catalog data only; no physical USB, FlowBridge, display-resolution or DPI changes.

## Observed

- PASS: Windows egui Ctrl+F → `clip-001` → Enter → keyboard-select result → inspector → containing folder → Alt+Left restored query and selected result. No measured latency or mouse distance.
- PASS: 62-volume generated catalog; keyboard Next changed a 300-result query from 1–250 to 251–300.
- PASS: both service and GUI return only disk 62 as metadata candidate for source disk 61; Back restored selection. No byte verification claimed. The request asks for other disks before pagination.
- PASS: opt-in egui renderer F12 writes clean synthetic PNGs; seven inspected images in atlas. WGC continues to time out for desktop automation and mouse geometry was unavailable.
- Subsequent exact-head GUI regression on `684ff551`: disk properties and sticker persisted after restart, scan progress and Esc rollback passed, and CSV export/no-clobber passed. JSON/Parquet were exercised by the hosted synthetic harness, not individually through GUI.
- PASS locally on the working tree: `cargo test --workspace --locked`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and strict Clippy with `qa-screenshots` enabled.

## Remaining work

Paired PR58/reference atlas; pointer movement and timing; language and theme comparisons and persistence; small-window and DPI modes within owner constraints; measured scroll restoration; categories/analytics. See matrix and defects. USB hardware intentionally excluded. Full PR58 parity and stable 1.0 are **not accepted**.

## Continuation — 2026-11-19

- PASS: `page-270.mkv` from a 301-entry synthetic folder → inspector → containing folder opened page 251–301, selected and scrolled to the file; Alt+Left restored the search and inspector. [Renderer capture](pr58-egui-atlas/egui-large-folder-selection.png). The service computes the page in the same SQLite read snapshot and order as `children`; wrong parent is rejected. Back scroll restoration is implemented, but has not been measured end-to-end in the GUI.
- PASS: nonexistent synthetic path was rejected inside the scan dialog, which remained open. [Scan dialog](pr58-egui-atlas/egui-scan-dialog.png); [error](pr58-egui-atlas/egui-scan-invalid-root.png).
- PASS: a synthetic NTFS scan displayed 51,920 processed entries; after Esc the interface said the snapshot was not saved. Service validation after rollback showed 62 volumes, 482 entries and `quick_check=ok`. [Cancelled state](pr58-egui-atlas/egui-scan-cancelled.png); progress was observed in the accessibility tree but its PNG was not captured before cancellation. The first Esc was sent while the automation tree was temporarily unavailable; the second was sent after the cancellation control became available.
- The PR58 reference was subsequently run in an isolated copy with freshly resolved pnpm dependencies, and its search workspace was visually observed; this was not a Bun-locked run. No reference PNG has yet been added, so paired atlas is still incomplete.

Those changes were committed as `684ff551`; [exact-head CI #463](https://github.com/i-shutov/whereisit-ai/actions/runs/44477205518) passed Linux, Windows, macOS ARM and Intel. [Bundle provenance and 60-volume synthetic QA](https://github.com/i-shutov/whereisit-ai/pull/69#issuecomment-7002492036), [F2 persistence](https://github.com/i-shutov/whereisit-ai/pull/69#issuecomment-7002391061), and [GUI CSV/no-clobber](https://github.com/i-shutov/whereisit-ai/pull/69#issuecomment-7002434546) are recorded in PR comments.

## Subsequent theme check

On `684ff551`, keyboard navigation via View → Dark mode switched the native app to light mode; [renderer image](pr58-egui-atlas/egui-light-62-disks.png) was reviewed and contains only synthetic data. WGC again returned `FrameArrived timed out`; mouse navigation, full RU/EN comparison, theme persistence and reference screenshots remain unverified. This is not a completed paired atlas or full visual acceptance.

GUI JSON and Parquet continuation: using the same binary from `684ff551` (subsequent `048b41e2` changes only QA documentation and an image), exported the generated 482-entry catalog through the native dialogs. Both reported 482 rows; JSON was parsed independently and contained 482 entries, and an isolated DuckDB read counted 482 Parquet rows. Raw output and private paths remain local. CSV/no-clobber is recorded in the preceding PR comment. Source inspection shows theme and language initialize to dark and Russian at startup; preference persistence is not implemented. Only the current-session light toggle was visually verified.

## Appearance persistence continuation — 2026-11-19

- PASS locally: chose EN and light via keyboard in a native `qa-screenshots` build on the synthetic 62-volume catalog, closed normally, and restarted with the same isolated settings. English and light were restored: [renderer image](pr58-egui-atlas/egui-en-light-after-restart.png). The settings file contained exactly the language and theme keys. A unit test covers round-trip, invalid values and absence of search/path state. Russian/dark remain defaults. This is a working-tree build, not a hosted exact-head bundle.
- PASS locally: workspace tests, `cargo fmt --all -- --check`, strict workspace Clippy and strict desktop Clippy with `qa-screenshots`. Dependency notices were regenerated and verified against the lockfile.
- OPEN UX-07: a pre-existing status string remained Russian immediately after switching to English; after restart the status was English. Full bilingual GUI acceptance remains incomplete.
- BLOCKED: paired prototype/egui atlas. An isolated prototype reference capture failed on unsupported color functions; a conversion attempt stalled the browser tab. No reference PNG is claimed. Pointer geometry remains unavailable; no system display settings were changed. Other remaining items are listed in the matrix and defects.

## Reference capture and status-language continuation — 2026-11-19

- PASS: actual prototype browser screenshots were captured and saved via a temporary loopback-only image sink, then converted from the returned JPEG to PNG. Six reference images are paired with native egui screenshots in the visual review; fixtures and viewport sizes differ and prototype values are simulated. PNGs were inspected for private data.
- PASS: GUI switched RU → EN → RU while a `1–250` result status was already displayed; the same status changed languages immediately. A regression test covers page and export responses, numeric values and an export filename. This was a working-tree `qa-screenshots` build, not a hosted bundle.
- PASS locally: workspace tests, fmt check, strict workspace Clippy, and strict desktop Clippy with the QA screenshot feature.
- PARTIAL: progress/cancel/error/export/overwrite reference pairs remain absent; pointer geometry, scrolled Back measurement, systematic bilingual audit and DPI matrix remain open. No system display settings were changed and no physical USB was tested. Full GUI acceptance, parity and stable 1.0 are not claimed.
