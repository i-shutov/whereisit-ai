#Requires -Version 7
<#
.SYNOPSIS
  Opt-in local Codex audit on the owner's interactive Administrator Win11 desktop.
.DESCRIPTION
  The owner's Administrator account is approved for Windows 11 QA.
  This script never runs inside a GitHub Actions runner service.
  Default mode is preflight only. -Execute is required to start Codex.
  Codex has a read-only sandbox and no user/project MCP configuration.
  Results remain in the invoking user's private LOCALAPPDATA.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$RepositoryPath,
    [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{40}$')][string]$ExpectedSha,
    [Parameter(Mandatory=$true)]
    [ValidateSet('categories','win64-installer','release-risks')]
    [string]$Focus,
    [switch]$Execute
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($env:RUNNER_NAME -or $env:CI -eq 'true') {
    throw 'Refusing to run Codex in a GitHub Actions runner process.'
}
if (-not [Environment]::UserInteractive) {
    throw 'Codex audit requires a real interactive Windows session.'
}
# Owner directive: Administrator is the established Win11 QA user.
# Disallow noninteractive CI/service, NOT administrator membership.
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$root = (Resolve-Path -LiteralPath $RepositoryPath -ErrorAction Stop).Path
if (-not (Test-Path -LiteralPath (Join-Path $root '.git'))) {
    throw 'An existing dedicated Git clone/worktree is required.'
}
$head = (& git -C $root rev-parse HEAD)
if ($LASTEXITCODE -ne 0 -or -not $head -or $head.Trim() -ine $ExpectedSha) {
    throw 'Local checkout does not match expected exact SHA.'
}
$dirty = @(& git -C $root status --porcelain)
if ($LASTEXITCODE -ne 0 -or $dirty.Count -ne 0) {
    throw 'Refusing Codex audit on a dirty worktree.'
}
$codex = @('codex.exe','codex.cmd','codex') |
    ForEach-Object { Get-Command $_ -CommandType Application -ErrorAction SilentlyContinue } |
    Select-Object -First 1
if (-not $codex) {
    throw 'Codex CLI is not visible in this interactive Windows 11 QA session.'
}
$prompts = @{
    categories = 'Read-only independent review of the current WhereIsIt Rust/egui category interface and SQLite category_id query. Verify duplicate names, recursive selection, unassign/reassign, Back navigation, category list virtualization, RU/EN and error handling. Do not edit files, commit, push, or claim actual GUI evidence. Report exact code references and test gaps.'
    'win64-installer' = 'Read-only audit of the current WhereIsIt AMD64 native Rust/egui installer and PowerShell 5.1 QA. Check PE architecture, installer isolation, user catalog preservation, SHA/provenance and no-clobber. Do not run setup, execute project code, edit, commit, or publish. Output a concise PASS/PARTIAL/BLOCKED matrix with evidence.'
    'release-risks' = 'Read-only release risk assessment for WhereIsIt 1.0 on Windows x64. Review release plans, Acceptance JSON, PR58 UX parity, real CTF gaps, physical GUI/USB and signing; distinguish hosted CI from physical QA. Do not change files, execute project code, push, or claim unobserved tests.'
}
Write-Host 'WIN11_CODEX_PREFLIGHT_PASS: Administrator session permitted, clean exact checkout, Codex command visible'
Write-Host "CHECKOUT_SHA: $($head.Trim())"
Write-Host "AUDIT_FOCUS: $Focus"
Write-Host "ACCOUNT_HAS_ADMIN_TOKEN: $isAdmin"
if (-not $Execute) {
    Write-Host 'CODEX_NOT_STARTED: pass -Execute explicitly after reviewing the audit focus'
    return
}
$local = [Environment]::GetFolderPath('LocalApplicationData')
if (-not [IO.Path]::IsPathRooted($local)) {
    throw 'Private local application data directory is unavailable.'
}
$private = Join-Path $local 'WhereIsItAI-QA\CodexAudits'
New-Item -ItemType Directory -Path $private -Force | Out-Null
$output = Join-Path $private ('codex-' + $Focus + '-' + [Guid]::NewGuid().ToString('N') + '.md')
$OutputEncoding = [Text.UTF8Encoding]::new($false)
$prompts[$Focus] | & $codex.Source exec --ignore-user-config --sandbox read-only --ephemeral --cd $root --output-last-message $output '-'
if ($LASTEXITCODE -ne 0) {
    throw 'Codex audit returned a nonzero exit code; keep any partial output private.'
}
if (-not (Test-Path -LiteralPath $output -PathType Leaf)) {
    throw 'Codex did not write its private final report.'
}
Write-Host 'WIN11_CODEX_READONLY_AUDIT_FINISHED: true'
Write-Host 'REPORT_PRIVATE_LOCAL_ONLY: true; inspect and redact manually before publishing'
