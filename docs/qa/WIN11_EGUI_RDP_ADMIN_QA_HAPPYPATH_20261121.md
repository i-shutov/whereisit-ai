# WhereIsIt 1.0 — Windows 11 Administrator interactive QA

**Prepared: 2026-11-21. Source-only handoff: NOT_RUN on physical Win11 GUI.** Scope is native Rust/egui Windows x64 QA preview with the real category UI from PR #73, not legacy Win32 and not stable 1.0.

## Codex downloads the program itself

Codex must obtain the exact-source **WhereIsIt AI Windows x64 GUI/CLI and installed QA candidate** without asking the owner to transfer these binaries. Follow [WIN11_CODEX_GET_PROGRAM_20261121.md](WIN11_CODEX_GET_PROGRAM_20261121.md): trusted checkout, `gh auth`, `gh run download`, checksum/provenance verification, then GUI launcher. The owner's private CTF and legitimate original WhereIsIt are separate local prerequisites.

## Exact artifact origin

From a successful exact-HEAD GitHub-hosted native CI workflow, download the artifact named **WhereIsItAI-egui-qa-windows-x64**. Extract this outer GitHub Actions archive into a fresh private directory; it must contain one preview ZIP, SHA256-windows-x64.txt and provenance-windows-x64.json. Independently confirm the repository, successful workflow run, source SHA and artifact download origin in GitHub before executing unsigned software. Hashes packaged alongside unsigned bytes establish consistency, not authenticity.

## Run in the existing Windows 11 Administrator RDP session

Use a trusted checkout containing tools/qa/windows11-egui/Invoke-VerifiedWin11EguiRdpQA.ps1. Do not use the GitHub runner Windows service to launch this script. **Administrator is the approved user**; no separate standard-user account is required.

```powershell
$sha = '<40-character exact SHA from successful hosted CI>'
$artifact = 'C:\WhereIsItQA\Extracted-Hosted-Windows-Artifact'
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\qa\windows11-egui\Invoke-VerifiedWin11EguiRdpQA.ps1 -ArtifactDirectory $artifact -ExpectedCommit $sha -DiskCount 60
```

The script validates exact provenance, preview ZIP SHA-256, embedded GUI/CLI hashes, absence of catalog payloads and Windows PowerShell 5.1 BOM; it then expands into a unique private LOCALAPPDATA directory and invokes the existing native egui QA runner in the **current interactive desktop**. The QA runner creates 60 synthetic volume snapshots, checks SQLite/search/exports/no-clobber and exact category assignments, opens the GUI, captures a window screenshot locally and prompts for independent manual review.

Review the screenshot and JSON/Markdown report before publishing any evidence. Do not upload private paths, CTF, SQLite, local Windows identity or unredacted screenshots. If any manual step fails, keep the evidence locally and report only a redacted failure description and source SHA.

## Scope limits

Pass of this script is evidence of the selected native QA preview scenario and manual observations only. It is **not** evidence of an installed lifecycle of the isolated Inno Win64 egui QA installer; follow docs/qa/WIN11_WIN64_EGUI_MANUAL_CHECKLIST_20261121.md separately for installed setup/launch/reinstall/uninstall, long NTFS progress/cancel/rollback and screen/DPI acceptance.

Also not accepted by this harness: physical USB unplug/replug, modified CTF opened by the original WhereIsIt, 3M real-data p95, signing/SmartScreen, macOS GUI and full PR58 UX parity. release/ACCEPTANCE.json remains stable_ready=false. Existing other-project runner services and system policies must remain untouched.
