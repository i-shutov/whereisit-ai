#Requires -Version 5.1
<#
.SYNOPSIS
  Verify hosted native egui QA preview and run it in Administrator RDP.
.DESCRIPTION
  No downloads, installation, drivers, other projects or source file deletion.
  User must independently validate the trusted Actions artifact download origin.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$ArtifactDirectory,
    [Parameter(Mandatory=$true)]
    [ValidatePattern('^[a-fA-F0-9]{40}$')][string]$ExpectedCommit,
    [ValidateRange(2,100)][int]$DiskCount = 60
)
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
if ($env:RUNNER_NAME -or $env:CI -eq 'true' -or -not [Environment]::UserInteractive) {
    throw 'Interactive Windows desktop required; runner service is not GUI acceptance.'
}
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Windows x64 required.' }
$os = Get-CimInstance Win32_OperatingSystem
if ($os.Caption -notmatch 'Windows 11') { throw 'Windows 11 required.' }

$src = (Resolve-Path -LiteralPath $ArtifactDirectory -ErrorAction Stop).Path
$sumFile = Join-Path $src 'SHA256-windows-x64.txt'
$provFile = Join-Path $src 'provenance-windows-x64.json'
foreach ($f in @($sumFile,$provFile)) {
    if (-not (Test-Path -LiteralPath $f -PathType Leaf)) {
        throw 'Missing checksum or provenance in Actions artifact.'
    }
}
$p = Get-Content -LiteralPath $provFile -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]$p.commit -ine $ExpectedCommit -or
    [string]$p.scope -cne 'QA-preview-only' -or
    [string]$p.architecture -cne 'windows-x64' -or
    [string]$p.signing -cne 'unsigned' -or
    $p.user_catalogs_included -ne $false) {
    throw 'Outer manifest does not match reviewed exact-source Windows x64 QA scope.'
}
$zips = @(Get-ChildItem -LiteralPath $src -File -Recurse -Filter 'WhereIsItAI-egui-preview-*-windows-x64.zip')
if ($zips.Count -ne 1) { throw 'Expected exactly one native egui Windows x64 ZIP.' }
$sum = (Get-Content -LiteralPath $sumFile -Raw -Encoding ASCII).Trim()
if ($sum -cnotmatch '^([a-fA-F0-9]{64})  ([^\\/]+\.zip)$') {
    throw 'Unexpected SHA256 manifest format.'
}
$expectedHash = $matches[1]
$expectedName = $matches[2]
if ($expectedName -cne $zips[0].Name) { throw 'QA ZIP filename mismatch.' }
if ((Get-FileHash -LiteralPath $zips[0].FullName -Algorithm SHA256).Hash -ine $expectedHash) {
    throw 'QA ZIP SHA-256 mismatch.'
}

$local = [Environment]::GetFolderPath('LocalApplicationData')
if (-not [IO.Path]::IsPathRooted($local)) { throw 'Private local QA directory unavailable.' }
$root = Join-Path $local ('WhereIsItAI-QA\Win11-RDP-' + [Guid]::NewGuid().ToString('N'))
$preview = Join-Path $root 'preview'
New-Item -Path $preview -ItemType Directory -ErrorAction Stop | Out-Null
Expand-Archive -LiteralPath $zips[0].FullName -DestinationPath $preview -ErrorAction Stop
$manifestPath = Join-Path $preview 'QA_MANIFEST.json'
$gui = Join-Path $preview 'WhereIsItAI-egui-preview.exe'
$cli = Join-Path $preview 'whereisit-ai-cli.exe'
$qa = Join-Path $preview 'Invoke-WhereIsItEguiQA.ps1'
foreach ($f in @($manifestPath,$gui,$cli,$qa)) {
    if (-not (Test-Path -LiteralPath $f -PathType Leaf)) {
        throw 'Incomplete native egui QA preview payload.'
    }
}
$m = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]$m.commit -ine $ExpectedCommit -or
    [string]$m.scope -cne 'local QA preview only' -or
    [string]$m.version -cne [string]$p.version) {
    throw 'Inner QA manifest mismatch.'
}
foreach ($entry in @(
    @{ file=$gui; expected=$m.gui_sha256 },
    @{ file=$cli; expected=$m.cli_sha256 }
)) {
    if ([string]$entry.expected -notmatch '^[a-fA-F0-9]{64}$') {
        throw 'Missing binary SHA-256.'
    }
    if ((Get-FileHash -LiteralPath $entry.file -Algorithm SHA256).Hash -ine [string]$entry.expected) {
        throw 'Native executable SHA-256 mismatch.'
    }
}
$catalogFiles = @(Get-ChildItem -LiteralPath $preview -File -Recurse |
    Where-Object { $_.Extension -in @('.sqlite','.ctf','.db') })
if ($catalogFiles.Count -gt 0) { throw 'Unexpected private catalog-like file in QA payload.' }
$bytes = [IO.File]::ReadAllBytes($qa)
if ($bytes.Length -lt 3 -or $bytes[0] -ne 0xEF -or
    $bytes[1] -ne 0xBB -or $bytes[2] -ne 0xBF) {
    throw 'PowerShell 5.1 bundled runner must have UTF-8 BOM for Cyrillic.'
}

Write-Host 'WIN11_QA_ARTIFACT_INTEGRITY_PASS'
Write-Host "SOURCE_SHA: $ExpectedCommit"
Write-Host 'Unsigned candidate: verify hosted GitHub Actions origin independently'
$evidence = Join-Path $root 'evidence'
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $qa -PreviewDir $preview -ExpectedCommit $ExpectedCommit -EvidenceRoot $evidence -DiskCount $DiskCount -ManualReview -CaptureScreenshot -CategoryChecks
if ($LASTEXITCODE -ne 0) { throw 'Native GUI QA returned failure; inspect private report.' }
$reports = @(Get-ChildItem -LiteralPath $evidence -File -Recurse -Filter 'qa_report.json')
if ($reports.Count -ne 1) { throw 'Expected one private QA report.' }
$report = Get-Content -LiteralPath $reports[0].FullName -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]$report.source_commit -ine $ExpectedCommit -or
    $report.stable_release_ready -ne $false -or
    [string]$report.hardware_usb_acceptance -cne 'NOT_RUN') {
    throw 'QA report source or release-claim mismatch.'
}
Write-Host 'WIN11_INTERACTIVE_EVIDENCE_COLLECTED: manual review required'
Write-Host 'LOCAL_ONLY: redact report and screenshot before sharing'
Write-Host 'USB, original-app modified CTF, 3M p95 and signing remain NOT_ACCEPTED'
