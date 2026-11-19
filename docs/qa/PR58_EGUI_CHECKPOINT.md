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

Paired PR58/reference atlas; GUI JSON/Parquet; pointer movement and timing; language and theme comparisons; small-window and DPI modes within owner constraints; measured scroll restoration; categories/analytics. See matrix and defects. USB hardware intentionally excluded. Full PR58 parity and stable 1.0 are **not accepted**.

## Continuation — 2026-11-19

- PASS: `page-270.mkv` from a 301-entry synthetic folder → inspector → containing folder opened page 251–301, selected and scrolled to the file; Alt+Left restored the search and inspector. [Renderer capture](pr58-egui-atlas/egui-large-folder-selection.png). The service computes the page in the same SQLite read snapshot and order as `children`; wrong parent is rejected. Back scroll restoration is implemented, but has not been measured end-to-end in the GUI.
- PASS: nonexistent synthetic path was rejected inside the scan dialog, which remained open. [Scan dialog](pr58-egui-atlas/egui-scan-dialog.png); [error](pr58-egui-atlas/egui-scan-invalid-root.png).
- PASS: a synthetic NTFS scan displayed 51,920 processed entries; after Esc the interface said the snapshot was not saved. Service validation after rollback showed 62 volumes, 482 entries and `quick_check=ok`. [Cancelled state](pr58-egui-atlas/egui-scan-cancelled.png); progress was observed in the accessibility tree but its PNG was not captured before cancellation. The first Esc was sent while the automation tree was temporarily unavailable; the second was sent after the cancellation control became available.
- The PR58 reference was subsequently run in an isolated copy with freshly resolved pnpm dependencies, and its search workspace was visually observed; this was not a Bun-locked run. No reference PNG has yet been added, so paired atlas is still incomplete.

Those changes were committed as `684ff551`; [exact-head CI #463](https://github.com/i-shutov/whereisit-ai/actions/runs/44477205518) passed Linux, Windows, macOS ARM and Intel. [Bundle provenance and 60-volume synthetic QA](https://github.com/i-shutov/whereisit-ai/pull/69#issuecomment-7002492036), [F2 persistence](https://github.com/i-shutov/whereisit-ai/pull/69#issuecomment-7002391061), and [GUI CSV/no-clobber](https://github.com/i-shutov/whereisit-ai/pull/69#issuecomment-7002434546) are recorded in PR comments.

## Subsequent theme check

On `684ff551`, keyboard navigation via View → Dark mode switched the native app to light mode; [renderer image](pr58-egui-atlas/egui-light-62-disks.png) was reviewed and contains only synthetic data. WGC again returned `FrameArrived timed out`; mouse navigation, full RU/EN comparison, theme persistence and reference screenshots remain unverified. This is not a completed paired atlas or full visual acceptance.
