# Real Windows 10 Pro RDP QA — WhereIsIt AI egui

This is a **local, source-pinned, offline QA handoff** for a pre-existing Windows
10 Pro 22H2 x64 interactive RDP machine. It also works on Windows 11. No
remote access credentials, machine hostname, or FlowBridge settings are stored
in GitHub.

## Prerequisites

- Stay in the existing interactive RDP desktop (the process must have a visible
  window). PowerShell **5.1** works. No Python and no new network services needed.
- Download exact `WhereIsItAI-egui-qa-windows-x64` from the GitHub Actions
  artifact for a specific native CI run. Unpack the outer Actions artifact,
  verify the inner preview ZIP against `SHA256-windows-x64.txt`, then unpack it.
- The inner ZIP now contains `WhereIsItAI-egui-preview.exe`, the matching
  `whereisit-ai-cli.exe`, `QA_MANIFEST.json`, this script, license notices
  and an explanatory README. DO NOT download and execute unverified binaries.

Example from Windows PowerShell (adjust file paths to your download directory):

```powershell
$zip = "$env:USERPROFILE\Downloads\WhereIsItAI-egui-preview-1.0.0-rc.3-windows-x64.zip"
$expectedLine = (Get-Content "$env:USERPROFILE\Downloads\SHA256-windows-x64.txt" -Raw).Trim()
$expected = ($expectedLine -split '\s+')[0]
if ((Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
    throw "Release preview hash mismatch"
}
$dir = "$env:USERPROFILE\Documents\WhereIsItAI-QA\preview-binaries"
New-Item -ItemType Directory $dir -Force | Out-Null
Expand-Archive -LiteralPath $zip -DestinationPath $dir
& "$dir\Invoke-WhereIsItEguiQA.ps1" -DiskCount 60 -ManualReview -CaptureScreenshot
```

If PowerShell execution policy blocks local unsigned scripts, use the current
session only: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File
"$dir\Invoke-WhereIsItEguiQA.ps1" -DiskCount 60 -ManualReview`.
Never change machine-wide policies.

## What is really exercised?

The script generates 60 tiny **synthetic** media roots in a uniquely named
subfolder of `Documents\WhereIsItAI-QA`, invokes the **real native Rust CLI**
via UTF-8 stdin to create 60 SQLite volume snapshots, validates global search,
schema, scoped full CSV/JSON/Parquet exports, export no-clobber, private shelf
label collision handling, and fail-closed physical-volume truth.

It then opens **the native egui GUI** with `--catalog` in the active RDP
session, waits for a real top-level window, optionally captures ONLY this
window's rectangle (locally), and presents a manual GUI acceptance checklist.
It closes only the preview process it started. No service/driver install,
firewall/SSH/VPN/registry changes, FlowBridge file edits, or cleanup of other
applications is performed.

**Automated smoke is NOT equivalent to manual acceptance.** In particular,
the QA host might not have actual pass-through USB hardware. Physical unplug,
replug, drive-letter reassignment and UUID conflicts must be marked BLOCKED
until verified on real devices; historical CTF compatibility is deferred until
the rest of the app is assembled.

The report remains **local only** at
`Documents\WhereIsItAI-QA\Run-*/qa_report.json` and `qa_report.md`.
Screenshots may contain user-specific information and must be manually redacted
before being copied into a public issue. Do not upload the synthetic catalog
or any private CTF to GitHub. For more detailed manual scenarios follow
[issue #66](https://github.com/i-shutov/whereisit-ai/issues/66).

**Windows PowerShell 5.1:** the CI packager writes the included QA runner
with an explicit UTF-8 BOM so Cyrillic fixture filenames and UTF-8 CLI
responses survive the legacy shell. Windows CI invokes it with the real
`powershell` shell (5.1), not PowerShell 7.
