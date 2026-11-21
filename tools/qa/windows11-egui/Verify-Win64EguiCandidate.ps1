#Requires -Version 5.1
<#
.SYNOPSIS
  Read-only preflight for a hosted native Rust/egui Windows x64 QA installer.
.DESCRIPTION
  Validates a *previously extracted* GitHub Actions artifact against explicit
  exact source SHA and run ID. Does not download, execute or install anything.
  In-artifact hashes are integrity checks, NOT a substitute for verifying
  the trusted GitHub Actions run, its URL and the artifact download origin.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$CandidateDirectory,
    [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{40}$')][string]$ExpectedCommit,
    [Parameter(Mandatory=$true)][ValidatePattern('^[0-9]+$')][string]$ExpectedRunId
)
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$dir = (Resolve-Path -LiteralPath $CandidateDirectory -ErrorAction Stop).Path
$provenanceFile = Join-Path $dir 'provenance.json'
$sumFile = Join-Path $dir 'SHA256SUMS.txt'
foreach ($f in @($provenanceFile, $sumFile)) {
    if (-not (Test-Path -LiteralPath $f -PathType Leaf)) {
        throw 'Required Win64 candidate manifest is missing.'
    }
}
$p = Get-Content -LiteralPath $provenanceFile -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]$p.channel -cne 'unsigned-isolated-qa' -or
    [string]$p.platform -cne 'windows-x64' -or
    [string]$p.binary_machine -cne 'AMD64' -or
    [string]$p.target_ui -cne 'Rust/egui' -or
    [string]$p.signing -cne 'unsigned' -or
    $p.stable_ready -ne $false -or
    $p.user_catalogs_included -ne $false) {
    throw 'Win64 candidate provenance safety labels do not match the QA contract.'
}
if ([string]$p.commit -ine $ExpectedCommit) {
    throw 'Candidate source commit does not match requested exact SHA.'
}
if ([string]$p.run_id -ne $ExpectedRunId) {
    throw 'Candidate GitHub Actions run ID does not match requested run.'
}
$version = [string]$p.version
if ($version -notmatch '^\d+\.\d+\.\d+(-rc\.\d+)?$') {
    throw 'Invalid candidate version in provenance.'
}
foreach ($key in @('egui_sha256', 'cli_sha256', 'installer_sha256')) {
    $digest = [string]$p.$key
    if ($digest -notmatch '^[0-9a-fA-F]{64}$') { throw "Missing or invalid provenance digest: $key" }
}
$expectedName = "WhereIsItAI-egui-win64-QA-$version-setup.exe"
$exe = Join-Path $dir $expectedName
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    throw 'Expected native egui Win64 QA installer is missing.'
}
$allSetups = @(Get-ChildItem -LiteralPath $dir -Filter '*-setup.exe' -File)
if ($allSetups.Count -ne 1 -or $allSetups[0].Name -cne $expectedName) {
    throw 'Unexpected additional or mismatched setup executable in QA artifact.'
}
$lines = @(Get-Content -LiteralPath $sumFile -Encoding ASCII |
    Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
if ($lines.Count -ne 1 -or $lines[0] -cnotmatch '^([0-9a-fA-F]{64})  ([^\\/]+)$') {
    throw 'Unexpected SHA256SUMS format; require one hash and one setup filename.'
}
$sumDigest = $matches[1]
$sumName = $matches[2]
if ($sumName -cne $expectedName) { throw 'SHA256SUMS filename mismatch.' }
$actual = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
if ($actual -ine $sumDigest -or $actual -ine [string]$p.installer_sha256) {
    throw 'Win64 QA setup hash differs from SHA256SUMS or provenance.'
}
Write-Host 'WIN64_EGUI_QA_PREFLIGHT_PASS: artifact manifest and installer integrity'
Write-Host "SOURCE_SHA: $($p.commit)"
Write-Host "ACTIONS_RUN_ID: $($p.run_id)"
Write-Host "INSTALLER_SHA256: $($actual.ToLowerInvariant())"
Write-Host 'SIGNING: unsigned; compare this evidence with the trusted GitHub-hosted Actions run before execution'
