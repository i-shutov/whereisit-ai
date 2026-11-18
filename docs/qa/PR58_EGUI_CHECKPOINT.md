# PR58 egui parity checkpoint

Base `ddd9215b0926c5e74482a3cbf7f10c229a66fb88`; branch `qa/pr58-egui-parity-win10-20261118`. PR #68 was already merged and was not reopened. Work uses generated catalog data only; no physical USB, FlowBridge, display-resolution or DPI changes.

## Observed

- PASS: Windows egui Ctrl+F → `clip-001` → Enter → keyboard-select result → inspector → containing folder → Alt+Left restored query and selected result. No measured latency or mouse distance.
- PASS: 62-volume generated catalog, first 250 results and enabled next-page control observed. Clicking next has not been confirmed.
- PASS: separate service fixture returns only disk 62 as metadata candidate for source disk 61; no byte verification claimed. The egui request now explicitly asks for other disks before pagination. GUI retest after the fix remains open.
- PASS: opt-in egui renderer F12 writes clean synthetic PNGs; four inspected images in atlas. WGC continues to time out for desktop automation and mouse geometry was unavailable.
- Previous PR #68 evidence (not newly rerun here): real synthetic NTFS scan progress, Esc cancellation and SQLite rollback; disk properties and sticker persistence; hosted bundle 60-disk QA. Current code retains those paths but exact-new-head GUI regression remains open.
- PASS locally on the working tree: `cargo test --workspace --locked`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and strict Clippy with `qa-screenshots` enabled.

## Remaining work

Paired PR58/reference atlas; GUI confirmation of other-disk candidates; complete 250+ pagination, disk properties/restart, scan/progress/Esc, errors and export/no-clobber on new head; pointer movement and timing; theme and language comparisons; small-window and DPI modes within owner constraints; folder item beyond first page and scroll restoration; categories/analytics. See matrix and defects. USB hardware intentionally excluded. Full PR58 parity and stable 1.0 are **not accepted**. Hosted exact-head CI and bundle provenance must be checked after publication; current local QA screenshot build is not a hosted package.
