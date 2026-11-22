# RC4 to stable egui product upgrade and rollback on a disposable hosted runner.
param([Parameter(Mandatory)][string]$SourceRoot)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or -not $env:RUNNER_TEMP) {
    throw 'Hosted GitHub Actions Windows runner required.'
}
$targetRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$sourceSha = git -C $SourceRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $sourceSha -ne '751fa5454c813e60b61800b517685ccab7e9776a') { throw 'Wrong RC4 source.' }
$targetSha = git -C $targetRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $targetSha -ne $env:STABLE_SHA -or $targetSha -notmatch '^[0-9a-f]{40}$') { throw 'Wrong stable target source.' }
$old = Join-Path $SourceRoot 'dist/windows-egui-product/WhereIsItAI-egui-1.0.0-rc.4-windows-x64-setup.exe'
$new = Join-Path $targetRoot 'dist/windows-egui-product/WhereIsItAI-egui-1.0.0-windows-x64-setup.exe'
$work = Join-Path $env:RUNNER_TEMP ('whereisit-egui-upgrade-' + [guid]::NewGuid().ToString('N'))
New-Item -Path $work -ItemType Directory | Out-Null
$installed = Join-Path $work 'Installed'
$cli = Join-Path $installed 'whereisit-ai-cli.exe'
$gui = Join-Path $installed 'whereisit-ai-egui.exe'
$db = Join-Path $work 'external.sqlite'
$fixture = Join-Path $work 'fixture'
$snapshot = Join-Path $work 'baseline.json'
$probe = Join-Path $PSScriptRoot 'upgrade_catalog_probe.py'
$placeholder = Join-Path $fixture 'external-user-ctf-placeholder.CTF'
$flags = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART')

function File-Sha([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Run-Installer([string]$Path, [string[]]$Arguments) {
    $result = Start-Process -FilePath $Path -ArgumentList $Arguments -Wait -PassThru -WindowStyle Hidden
    if ($result.ExitCode -ne 0) { throw "Installer returned $($result.ExitCode)." }
}
function Install-Product([string]$Package, [string]$BuildRoot, [string]$Version) {
    Run-Installer $Package ($flags + @('/DIR="' + $installed + '"'))
    $installedVersion = & $cli --version
    if ($LASTEXITCODE -ne 0 -or $installedVersion.Trim() -ne "WhereIsIt AI $Version") { throw 'Installed product version mismatch.' }
    if ((File-Sha $gui) -ne (File-Sha (Join-Path $BuildRoot 'target/release/atlas-desktop-egui.exe')) -or
        (File-Sha $cli) -ne (File-Sha (Join-Path $BuildRoot 'target/release/atlas.exe'))) {
        throw 'Installed executables differ from the exact-source product build.'
    }
}
function Remove-Product {
    $uninstallers = @(Get-ChildItem -LiteralPath $installed -File -Filter 'unins*.exe')
    if ($uninstallers.Count -ne 1) { throw 'Expected one uninstaller for the product identity.' }
    Run-Installer $uninstallers[0].FullName $flags
    Start-Sleep -Seconds 2
    if ((Test-Path -LiteralPath $gui) -or (Test-Path -LiteralPath $cli)) { throw 'Product executables survived uninstall.' }
}
function Run-Probe([string]$Phase) {
    & python $probe $Phase --cli $cli --db $db --fixture-root $fixture --snapshot $snapshot --source-release 1.0.0-rc.4 --target-release 1.0.0
    if ($LASTEXITCODE -ne 0) { throw "Synthetic catalog probe failed during $Phase." }
}
function Verify-External([string]$CatalogSha, [string]$CtfSha) {
    if ((File-Sha $db) -ne $CatalogSha -or (File-Sha $placeholder) -ne $CtfSha) {
        throw 'Installer lifecycle changed generated external data.'
    }
}

$oldSha = File-Sha $old
$newSha = File-Sha $new
Install-Product $old $SourceRoot '1.0.0-rc.4'
Run-Probe prepare
$beforeUpgrade = File-Sha $db
$ctfSha = File-Sha $placeholder
Install-Product $new $targetRoot '1.0.0'
Verify-External $beforeUpgrade $ctfSha
Run-Probe verify
$beforeRollback = File-Sha $db
Remove-Product
Verify-External $beforeRollback $ctfSha
Install-Product $old $SourceRoot '1.0.0-rc.4'
Verify-External $beforeRollback $ctfSha
& $cli validate $db | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Reinstalled RC4 could not validate the external catalog.' }
Remove-Product
Verify-External $beforeRollback $ctfSha

$out = Join-Path $targetRoot 'dist/qa-egui-upgrade'
New-Item -Path $out -ItemType Directory -Force | Out-Null
@{
    format = 'whereisit-egui-rc4-to-1.0-upgrade-v1'
    platform = 'windows-x64'
    source_version = '1.0.0-rc.4'
    target_version = '1.0.0'
    source_commit = $sourceSha
    target_commit = $targetSha
    source_package_sha256 = $oldSha
    target_package_sha256 = $newSha
    external_data_preserved = $true
    catalog_search_and_category_parity = $true
    rollback_reinstall_and_validation = $true
    synthetic_only = $true
    gui_observed = $false
    status = 'pass'
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'UPGRADE.json') -Encoding utf8
