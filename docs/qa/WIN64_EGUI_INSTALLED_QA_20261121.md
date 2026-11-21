# WhereIsIt AI — Windows x64 native Rust/egui installed QA candidate

**Checkpoint: 2026-11-21.** W4 focus is Windows 10/11 **x64 (AMD64/Win64)**, not x86 and not a macOS release. This is an isolated **unsigned QA candidate**, not stable 1.0 and not an official replacement of immutable RC3 assets.

## Why

The existing production-style Windows installer (installers/windows/WhereIsItAI.iss) still packages atlas-windows.exe, the legacy **Win32 API UI**. Win32 here describes a GUI API family, not necessarily a 32-bit executable: an old Win32 API GUI can be compiled as x64. The intended 1.0 UX is the separate native Rust/egui GUI built as atlas-desktop-egui.exe. Therefore an x64 installer alone does not prove correct UX implementation.

## Win64-only implementation

- Dedicated Inno Setup 6 file: installers/windows/WhereIsItAI-EguiWin64QA.iss. Isolated AppId, product name, per-user program directory, Start Menu shortcut and installer filename. No upgrade of legacy RC3 AppId, file associations, privilege elevation, desktop shortcut, or post-install GUI autostart.
- Payload: Rust/egui executable renamed whereisit-egui-win64.exe, matching native CLI, LICENSE, THIRD_PARTY_NOTICES.md and dependency license files. No React, Node, personal SQLite or CTF assets.
- Inno only allows x64-compatible Windows, while hosted CI runs on Windows-2022 AMD64. The headless smoke independently reads the PE header Machine field and **requires 0x8664 AMD64** for the exact installed GUI and CLI. SHA-256 installed EXEs must match the newly compiled Rust/egui and CLI source binaries, not legacy Win32 UI.
- Before install, refuse any existing QA install or database path. Installed CLI initializes a generated SQLite catalog outside the application directory. After silent uninstall, the catalog must remain byte-for-byte unchanged. No real user data, deletion, destructive cleanup or hidden migration is performed.
- Hosted GitHub Actions Windows job builds and executes the installed QA smoke, writes SHA256SUMS.txt and provenance.json with source commit, run ID, GUI/CLI and installer SHA, target UI, AMD64, unsigned and stable_ready=false. Publishes a separate artifact: WhereIsItAI-egui-Win64-installed-QA-unsigned.
- The self-hosted Windows 11 local administrator runner is **not** used for PR code, installer builds or privileged arbitrary commands. Its separate trusted-main manual identity smoke remains unchanged.

## Reproduction on a trusted clean Windows x64 QA environment

1. Run cargo build --release --locked -p atlas-desktop-egui -p atlas-cli.
2. Run .\installers\windows\build-egui-win64-qa.ps1.
3. Run .\installers\windows\smoke-egui-win64-qa.ps1 in an isolated test user profile.

The test does **not** launch a GUI. The installed QA executable must then undergo a separate interactive Win10/Win11 RDP test after verifying artifact SHA/provenance and application identity.

## Exit and open gates

Only a passing exact-head GitHub-hosted Windows CI job can mark the **isolated Win64 egui installed QA smoke** as PASS. Physical GUI run, PR58 visual and functional parity, fresh RC3-to-next-RC migration, authentic CTF roundtrip in the original program, real 3M-entry performance, physical USB, accessibility, Windows signing/SmartScreen reputation and final stable readiness are NOT established by this code or test. release/ACCEPTANCE.json remains stable_ready=false.

The VERSION value currently labels the historical RC3 source generation; QA candidate filenames always include egui-win64-QA and must **not** be attached to the existing v1.0.0-rc.3 release or retagged. Stable installation identity and future upgrade policy require a separate explicit decision and fresh version after independent QA.

## Read-only exact-run artifact verification on Windows 11

After independently confirming a successful GitHub-hosted workflow run and the exact source SHA, download and extract the separate `WhereIsItAI-egui-Win64-installed-QA-unsigned` Actions artifact. From a trusted checkout **matching that SHA**, run the PowerShell 5.1 preflight script (read-only):

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\qa\windows11-egui\Verify-Win64EguiCandidate.ps1 -CandidateDirectory 'C:\QA\ExtractedWin64Candidate' -ExpectedCommit '<exact-40-hex-SHA>' -ExpectedRunId '<Actions-run-id>'
```

The preflight rejects wrong repository source SHA, run ID, non-Win64/egui provenance, missing manifests, unexpected additional setups, digest mismatch and any stable/signature misrepresentation. **Self-reported hashes inside an unsigned artifact alone do not establish publisher authenticity.** The operator must verify the trusted repository workflow URL, Actions run and artifact source independently; do not bypass OS security prompts or use unknown mirrors.

Manual installed GUI scenarios and redacted acceptance matrix: [Win11 native egui acceptance checklist](WIN11_WIN64_EGUI_MANUAL_CHECKLIST_20261121.md). Do not call this a physical GUI PASS merely because the script passed.

## Separate QA user data (no shared production sidecar)

The isolated Win64 installer compiles `atlas-desktop-egui.exe` with the opt-in `qa-isolated` feature, **only** for this side-by-side QA artifact. That feature forces the Win64 app's physical-drive binding sidecar and eframe theme/language settings into a distinct per-user `LOCALAPPDATA/WhereIsIt AI egui Win64 QA` directory and refuses to fall back to the production `AtlasCatalog` sidecar or common eframe settings. An invalid/unavailable local profile prevents the instrumented QA GUI from starting. Inno uninstall deliberately **does not erase** those QA-local user files. The existing default native egui preview binary is restored before uploading the unrelated generic Windows artifact, to avoid accidentally distributing the QA-isolated binary as the default GUI.

These guards isolate default app settings, **not arbitrary catalogs explicitly opened by the operator**. Use synthetic SQLite only; do not open real personal catalogs in QA or publish their paths/screenshots.

## PowerShell 5.1 native CLI transport regression

Initial hosted Windows smoke failed at the synthetic SQLite init check. The existing legacy installer smoke used PowerShell 7, whereas the new Win64 QA smoke intentionally used **Windows PowerShell 5.1**; JSON in a native command-line argument is not reliably quoted in PowerShell 5.1. The QA smoke now reuses the established `atlas serve` JSON-lines transport over UTF-8 **without BOM**, with stdout/stderr drained concurrently and only sanitized error codes displayed. This correction is not a claim of PASS until exact-head installed smoke is rerun. The regression is isolated to test transport, not product SQLite or egui functionality.
