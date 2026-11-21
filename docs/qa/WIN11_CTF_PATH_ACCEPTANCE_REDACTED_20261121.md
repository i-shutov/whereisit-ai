# Win11 CTF and narrow-window acceptance: sanitized results

Date: 2026-11-21. Platform: Windows 11 Pro x64 build 22631, interactive Administrator session. Original application: WhereIsIt 2014 build 220. Source fixes: `06921ac`, `10a02e0`, `3a0f994`, based on PR #73 exact head `40c6b695958e49cbdad78affa314b139dd28d61b`.

This report contains synthetic cases and aggregate outcomes only. Private catalogs, filenames, paths, descriptions, hashes, licenses, raw logs and screenshots are excluded. Private source integrity was checked locally; the source was unchanged. Synthetic tests construct their own data and do not load a private catalog.

## Problems and fixes

- **P0: invalid catalog offset-table length.** A fixed 184-byte declaration described 45 blocks even when the exporter wrote 60. The writer now declares `4 * (block_count + 1)`. The parser still opens older Atlas exports with the fixed declaration and validates the actual offsets and sentinel. Synthetic format tests cover both declared lengths and reject unrelated mismatches.
- **P0: vendor-save root metadata.** The disk-header word at offset 4 declares the root metadata-blob length. The parser validates the blob and range bounds, and the writer preserves blob bytes and existing references instead of rejecting a supported vendor save. Constructed synthetic records cover preservation and malformed range rejection.
- **P1: inaccessible Path after narrowing the window.** Retained column widths could put Kind/Path outside the viewport. Bounded initial widths, clipped labels with full hover text and a shared horizontal scroller keep columns reachable. All five resize handles and virtualized rows remain. A real egui paint/input regression covers long Cyrillic strings, 250 rows, a 1366-to-1024 transition in the same Context, horizontal overflow at 880 points and vertical scrolling. The old layout failed this regression; the patched layout passed.

## Observed acceptance

| Gate | Result and scope |
|---|---|
| Synthetic CTF in original application | PASS: 60 volumes / 180 entries opened; a changed description was displayed. |
| Exported description edit on an authorized private copy | PASS: opened and checked in the original application; raw evidence remains local. |
| Vendor-saved description imported natively | PASS: description imported and found, SQLite integrity passed, native re-export matched the vendor save byte-for-byte. |
| Focused format/service regression suite | PASS: 95 tests at the CTF-fix checkpoint. |
| egui suite | PASS: 40 tests with isolated QA/screenshot features; scoped formatting and strict Clippy passed. |
| Local patched native GUI | PASS: exact captured 1024x768 window exposes Kind/Path and selected inspector; page 251-500 and vertical scrolling work. At 882x768, horizontal input exposes the full Path cells and moves headers with rows. |
| Elevated symlink fixture / GUI scan refusal | PASS: verified elevated creation; real GUI refused the symbolic-link root; no partial snapshot, SQLite integrity and foreign keys passed. |
| Earlier hosted installed QA lifecycle | PASS: install, reinstall, real launch/restart and uninstall; external SQLite, local note/sticker data and settings preserved byte-for-byte. This observation predates the patched hosted installer. |

The local patched GUI was an isolated development build. Exact-head hosted CI and the newly packaged installer's separate GUI observations are tracked in the draft PR checks and description; they must not be inferred from local test results or an older package.

## Remaining release gates

Full PR #58 parity remains incomplete, including archive/sunburst/local-analyzer workspaces and some register filters/capacity presentation. Physical USB identity, full DPI/platform coverage and authenticated signing remain independent gates. The invisible UAC prompt's client-side cause was not established; no security settings were weakened. These focused fixes do not approve stable 1.0. Keep `stable_ready=false`; no release tag, merge or automatic publication is authorized by this report.
