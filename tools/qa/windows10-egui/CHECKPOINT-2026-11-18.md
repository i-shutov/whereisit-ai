# Windows 10 QA checkpoint — 2026-11-18

Baseline source: `355c0934536a95b253cb23bd231431e8f89e994b`. The local preview was assembled from this checkout and is not a hosted CI artifact.

- SOURCE_COMPILED: PASS locally; release GUI and CLI match the previous GUI-cancellation test binaries.
- Windows PowerShell 5.1 native transport regression: PASS for UTF-8, paths with spaces, invalid JSON and service errors.
- Synthetic runner: PASS for 6 headless volumes and 60 volumes with native window startup/close; validation, search, CSV/JSON/Parquet export, no-clobber and sidecar catalog immutability passed. A local GUI screenshot was visually checked; it is not published because peripheral window fragments are visible. Physical USB was not tested.
- GUI negative tests on baseline: nonexistent root, directory junction and catalog located inside the scan root were rejected. Errors appeared behind the scan dialog while the old status remained; the accompanying fix places the message in the dialog and sets rejection status. The fix was retested in the local release GUI and has a regression test.
- GUI_LAUNCHED: PASS locally. GUI_WORKFLOWS_ACCEPTED: NOT ACCEPTED as complete; earlier targeted GUI evidence is documented in `CHECKPOINT-NTFS-2026-11-17.md` and PR discussion, not a complete hosted-bundle acceptance.
- PHYSICAL_USB_VERIFIED: NOT TESTED at user request, not a blocker for this Windows NTFS test. FEATURE_PARITY_ACCEPTED: NO. STABLE_1_0_READY: NO.
- Hosted CI: run #455 failed its identity gate on an earlier SHA. Run #456 on baseline `355c0934` was in progress at this checkpoint. No hosted result or bundle is implied for the accompanying fix commit.
- Screen capture: existing QA runner saved a local PNG; Windows Graphics Capture in the automation helper still times out. Display resolution and DPI were not changed.
