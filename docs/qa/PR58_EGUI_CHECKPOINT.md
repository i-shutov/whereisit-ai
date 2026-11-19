# PR58 egui parity checkpoint

Base `ddd9215b0926c5e74482a3cbf7f10c229a66fb88`; branch `qa/pr58-egui-parity-win10-20261118`. PR #68 was already merged and was not reopened. Work uses generated catalog data only; no physical USB, FlowBridge, display-resolution or DPI changes.

## Observed

- PASS: Windows egui Ctrl+F → `clip-001` → Enter → keyboard-select result → inspector → containing folder → Alt+Left restored query and selected result. No measured latency or mouse distance.
- PASS: 62-volume generated catalog; keyboard Next changed a 300-result query from 1–250 to 251–300.
- PASS: both service and GUI return only disk 62 as metadata candidate for source disk 61; Back restored selection. No byte verification claimed. The request asks for other disks before pagination.
- PASS: opt-in egui renderer F12 writes clean synthetic PNGs; seven inspected images in atlas. WGC continues to time out for desktop automation and mouse geometry was unavailable.
- Previous PR #68 evidence (not newly rerun here): real synthetic NTFS scan progress, Esc cancellation and SQLite rollback; disk properties and sticker persistence; hosted bundle 60-disk QA. Current code retains those paths but exact-new-head GUI regression remains open.
- PASS locally on the working tree: `cargo test --workspace --locked`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and strict Clippy with `qa-screenshots` enabled.

## Remaining work

Paired PR58/reference atlas; disk properties/restart, scan/progress/Esc, errors and export/no-clobber on new head; pointer movement and timing; theme and language comparisons; small-window and DPI modes within owner constraints; folder item beyond first page and scroll restoration; categories/analytics. See matrix and defects. USB hardware intentionally excluded. Full PR58 parity and stable 1.0 are **not accepted**. Hosted exact-head CI and bundle provenance must be checked after publication; current local QA screenshot build is not a hosted package.

## Continuation — 2026-11-19

- PASS: `page-270.mkv` from a 301-entry synthetic folder → inspector → containing folder opened page 251–301, selected and scrolled to the file; Alt+Left restored the search and inspector. [Renderer capture](pr58-egui-atlas/egui-large-folder-selection.png). The service computes the page in the same SQLite read snapshot and order as `children`; wrong parent is rejected. Back scroll restoration is implemented, but has not been measured end-to-end in the GUI.
- PASS: nonexistent synthetic path was rejected inside the scan dialog, which remained open. [Scan dialog](pr58-egui-atlas/egui-scan-dialog.png); [error](pr58-egui-atlas/egui-scan-invalid-root.png).
- PASS: a synthetic NTFS scan displayed 51,920 processed entries; after Esc the interface said the snapshot was not saved. Service validation after rollback showed 62 volumes, 482 entries and `quick_check=ok`. [Cancelled state](pr58-egui-atlas/egui-scan-cancelled.png); progress was observed in the accessibility tree but its PNG was not captured before cancellation. The first Esc was sent while the automation tree was temporarily unavailable; the second was sent after the cancellation control became available.
- The PR58 reference was subsequently run in an isolated copy with freshly resolved pnpm dependencies, and its search workspace was visually observed; this was not a Bun-locked run. No reference PNG has yet been added, so paired atlas is still incomplete.

These are local working-tree results pending the continuation commit and its exact-head CI. Remaining: properties/restart, GUI export and overwrite, RU/EN and themes, pointer timing/distance, measured scroll restoration, and the full paired PR58 atlas. The current evidence does not prove complete parity or stable 1.0.
