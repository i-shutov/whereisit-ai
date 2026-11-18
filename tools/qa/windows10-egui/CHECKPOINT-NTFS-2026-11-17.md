# Windows 10 NTFS / egui QA checkpoint — 2026-11-17

## Scope

The test subject was source commit `bb5fe5e11d97c9ad1da29236cbe5b1c47d6f2fb1`, built locally on Windows 10 Pro as native debug and release executables. The catalog and test tree were isolated on local NTFS. No USB, display-resolution or DPI changes were involved. This is not stable-release acceptance or complete PR #58 parity.

## Results

- **PASS:** exact-source debug GUI disk properties: F2 focused the name; Unicode local name and sticker were saved and reopened; source catalog SHA-256 was unchanged.
- **PASS:** exact-source release GUI created and reopened a SQLite catalog, scanned 24,000 synthetic files and 24 directories, then rescanned an expanded tree as a separate snapshot with 72,000 files and 72 directories.
- **PASS:** GUI Unicode Boolean search; global scope returned two synthetic matches, selected-volume scope one; 250-row pagination advanced from 1–250 to 251–500. Inspector, folder contents and Alt+Left Back were observed.
- **PASS:** GUI exported one selected-scope result to CSV, JSON and Parquet; CSV and JSON each contain one entry and Parquet has valid leading/trailing magic. Existing CSV overwrite was rejected and its SHA-256 remained unchanged. GUI reported an old success status next to the new rejection; a fix and regression accompany this checkpoint.
- **PASS (service):** in-process release-service probe observed progress rise to 234 entries, accepted cancellation, returned `cancelled`, and showed zero volumes before and after. A subsequent scan completed with 24,024 records. This is not a GUI cancellation result.
- **PASS (service):** metadata-only scan of an existing local system directory completed with 23 entries; no source data or private path is published.
- **BLOCKED:** GUI cancellation and visible progress were not observed; accessibility snapshots were unavailable during both short scans. Both GUI scans completed and produced snapshots, so neither is claimed as cancelled.
- **BLOCKED:** Windows Graphics Capture timed out twice, including after reselecting the window; visual screenshot evidence is unavailable. Accessibility observations are not screenshots.
- **BLOCKED:** GitHub returned zero check runs and no pull-request workflow runs for baseline SHA at the time checked. This is not a green CI result.

## Performance (single synthetic run, not a benchmark)

The release-service probe completed 24,024 metadata records in 6.359 seconds, about 3,778 records/second. The whole probe (cancelled scan plus successful scan) took 6.679 seconds; sampled process CPU time was approximately 5.781 seconds and peak sampled working set was 10,833,920 bytes. Samples include probe and polling overhead and are not isolated system-wide CPU or RAM measurements. The existing-directory scan was too short for useful sampled CPU statistics.

## Outstanding

Obtain exact-new-HEAD CI and packaged-bundle verification, resolve supported screenshot capture and visually inspect screenshots, and demonstrate GUI progress plus cancellation/rollback while a scan is running. Do not infer physical USB presence, CTF compatibility, signing, or 1.0 stability from these tests.

## Follow-up: GUI progress and cancellation

On a subsequent local Windows 10 Pro run, a 272,000-file synthetic NTFS tree and empty catalog allowed GUI accessibility inspection to observe progress increasing from 9,279 to 109,998 records. The automation environment could not determine the button geometry, and tabbing to it did not finish before that scan completed.

The accompanying change adds a visible Esc shortcut for cancellation after the scan job is registered, while preserving the existing button and modal guards. In the locally built release GUI with this change, the inspector showed 9,148 processed records, then Esc produced `Scan cancelled; no snapshot saved`. The native catalog validator subsequently reported zero volumes, zero entries and `quick_check=ok`. A desktop regression covers registration and modal behavior. This was a local release build of the source tree; it is not hosted bundle or CI evidence.

Screenshot capture remains blocked: Windows Graphics Capture timed out on two attempts after window reselection. A system-window capture reached a local image editor, but its Save As dialog was not controllable through the available automation, so no screenshot was saved or published. No resolution, DPI or system settings were changed. Exact-head hosted CI and bundle still need verification.
