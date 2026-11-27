#Requires -Version 5.1
<#
.SYNOPSIS
  Download exact-source native egui Win64 QA preview and installer from GitHub Actions.
.DESCRIPTION
  Runs locally in the owner's interactive Administrator session. Never checks out
  a PR, installs software, launches GUI, writes CTF or changes Execution Policy.
  Requires authenticated GitHub CLI (gh) and a verified successful hosted run.
  Refuses to overwrite any previous download directory.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [ValidatePattern('^[0-9]+$')]
    [string]$RunId,
    [Parameter(Mandatory=$true)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$ExpectedCommit,
    [Parameter(Mandatory=$true)]
    [string]$DestinationRoot
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$repo = 'i-shutov/whereisit-ai'
if ($env:RUNNER_NAME -or $env:CI -eq 'true' -or -not [Environment]::UserInteractive) {
    throw 'This downloader must run in the interactive Windows QA session, not the Actions service.'
}
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Windows x64 is required.' }

$gh = Get-Command 'gh.exe' -CommandType Application -ErrorAction SilentlyContinue |
    Select-Object -First 1
if (-not $gh) {
    throw 'GitHub CLI missing. Install the official GitHub CLI (winget install --id GitHub.cli --exact), then run gh auth login.'
}
$destination = [IO.Path]::GetFullPath($DestinationRoot)
if (Test-Path -LiteralPath $destination) {
    throw 'Refusing to overwrite an existing QA artifact destination.'
}
if ($destination -eq [IO.Path]::GetPathRoot($destination)) {
    throw 'Refusing to write QA artifact at drive root.'
}

# Always inspect the remote run before downloading; do not trust a user-entered
# run ID alone. This returns public code metadata, never a private CTF path.
$raw = & $gh.Source run view $RunId --repo $repo --json 'headSha,status,conclusion,workflowName'
if ($LASTEXITCODE -ne 0 -or -not $raw) {
    throw 'Could not query GitHub Actions run; confirm gh auth status and approved repository access.'
}
try {
    $run = ($raw -join [char]10) | ConvertFrom-Json -ErrorAction Stop
} catch {
    throw 'GitHub CLI returned an invalid workflow metadata document.'
}
if ([string]$run.headSha -ine $ExpectedCommit -or
    [string]$run.status -cne 'completed' -or
    [string]$run.conclusion -cne 'success' -or
    [string]$run.workflowName -notlike '*native CI and installer candidates*') {
    throw 'Actions run is not a successful exact-commit native CI workflow.'
}

New-Item -ItemType Directory -Path $destination -ErrorAction Stop | Out-Null
$preview = Join-Path $destination 'preview'
$installed = Join-Path $destination 'installer'
New-Item -ItemType Directory -Path $preview -ErrorAction Stop | Out-Null
New-Item -ItemType Directory -Path $installed -ErrorAction Stop | Out-Null

function Get-HostedArtifact([string]$Name, [string]$Destination) {
    & $gh.Source run download $RunId --repo $repo --name $Name --dir $Destination
    if ($LASTEXITCODE -ne 0) {
        throw 'Exact-run GitHub Actions artifact download failed. Do not use an unverified mirror.'
    }
}
Get-HostedArtifact 'WhereIsItAI-egui-qa-windows-x64' $preview
Get-HostedArtifact 'WhereIsItAI-egui-Win64-installed-QA-unsigned' $installed

# Validate the portable QA preview's external manifest and whole inner ZIP.
$previewProvenance = Join-Path $preview 'provenance-windows-x64.json'
$previewChecksums = Join-Path $preview 'SHA256-windows-x64.txt'
foreach ($file in @($previewProvenance, $previewChecksums)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
        throw 'Downloaded Win64 preview is missing a required provenance/checksum file.'
    }
}
$p = Get-Content -LiteralPath $previewProvenance -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]$p.commit -ine $ExpectedCommit -or
    [string]$p.scope -cne 'QA-preview-only' -or
    [string]$p.architecture -cne 'windows-x64' -or
    [string]$p.signing -cne 'unsigned' -or
    $p.user_catalogs_included -ne $false) {
    throw 'Portable egui preview provenance does not match exact source and QA-only scope.'
}
$zips = @(Get-ChildItem -LiteralPath $preview -File -Filter 'WhereIsItAI-egui-preview-*-windows-x64.zip')
if ($zips.Count -ne 1) { throw 'Expected exactly one Win64 egui preview ZIP.' }
$checksum = (Get-Content -LiteralPath $previewChecksums -Raw -Encoding ASCII).Trim()
if (-not ($checksum -cmatch '^([0-9A-Fa-f]{64})  ([^\\/]+\.zip)$')) {
    throw 'Win64 preview checksum manifest has an invalid format.'
}
$expectedHash = $matches[1]
$expectedFilename = $matches[2]
if ($zips[0].Name -cne $expectedFilename) {
    throw 'Win64 preview filename does not match the signed-off manifest.'
}
$actualHash = (Get-FileHash -LiteralPath $zips[0].FullName -Algorithm SHA256).Hash
if ($actualHash -ine $expectedHash) { throw 'Win64 preview ZIP SHA-256 mismatch.' }

# The installed QA candidate has a separate, already-reviewed verification
# contract including run ID, SHA and unsigned side-by-side installer identity.
$verifyInstaller = Join-Path $PSScriptRoot 'Verify-Win64EguiCandidate.ps1'
if (-not (Test-Path -LiteralPath $verifyInstaller -PathType Leaf)) {
    throw 'Trusted local installed-QA verification script is missing.'
}
& $verifyInstaller -CandidateDirectory $installed -ExpectedCommit $ExpectedCommit -ExpectedRunId $RunId
if (-not $?) { throw 'Installed egui Win64 QA artifact preflight failed.' }

# No setup.exe, GUI or user CTF is executed by this retrieval step.
Write-Host 'WIN64_QA_BUNDLES_DOWNLOADED_VERIFIED: true'
Write-Host "SOURCE_COMMIT: $ExpectedCommit"
Write-Host "GITHUB_ACTIONS_RUN_ID: $RunId"
Write-Host 'PREVIEW_ARCHIVE_SHA256_VERIFIED: true'
Write-Host 'INSTALLER_MANIFEST_SHA256_VERIFIED: true'
Write-Host 'INSTALLER_EXECUTED: false'
Write-Host 'GUI_EXECUTED: false'
Write-Host 'REAL_CTF_TOUCHED: false'
Write-Host 'Use the trusted Invoke-VerifiedWin11EguiRdpQA.ps1 with the preview directory for interactive GUI QA.'
