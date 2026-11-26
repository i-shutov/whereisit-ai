# Exact-source egui product upgrade and rollback on a disposable hosted runner.
param([Parameter(Mandatory)][string]$SourceRoot)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or -not $env:RUNNER_TEMP) {
    throw 'Hosted GitHub Actions Windows runner required.'
}
$targetRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$sourceSha = (git -C $SourceRoot rev-parse HEAD).Trim()
$targetSha = (git -C $targetRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $targetSha -ne $env:TARGET_SHA -or $targetSha -notmatch '^[0-9a-f]{40}$') { throw 'Wrong exact target source.' }
$sourceVersion = (Get-Content -LiteralPath (Join-Path $SourceRoot 'VERSION') -Raw).Trim()
$targetVersion = (Get-Content -LiteralPath (Join-Path $targetRoot 'VERSION') -Raw).Trim()
$pair = switch ($targetVersion) {
    '1.0.0' { @{ source = '1.0.0-rc.4'; sha = '751fa5454c813e60b61800b517685ccab7e9776a' } }
    '1.1.0-rc.1' { @{ source = '1.0.0'; sha = '938fece2d74793c2eda7a1b84082ab5db3f8a092' } }
    '1.1.0' { @{ source = '1.0.0'; sha = '938fece2d74793c2eda7a1b84082ab5db3f8a092' } }
    '1.1.1' { @{ source = '1.1.0'; sha = '7f684f47aab42fb211dfa0af1085ace311ef9f8d' } }
    default { throw "Unsupported target release: $targetVersion" }
}
if ($sourceVersion -ne $pair.source -or $sourceSha -ne $pair.sha) { throw 'Wrong exact predecessor source.' }
$old = Join-Path $SourceRoot "dist/windows-egui-product/WhereIsItAI-egui-$sourceVersion-windows-x64-setup.exe"
$new = Join-Path $targetRoot "dist/windows-egui-product/WhereIsItAI-egui-$targetVersion-windows-x64-setup.exe"
$work = Join-Path $env:RUNNER_TEMP ('whereisit-egui-upgrade-' + [guid]::NewGuid().ToString('N'))
New-Item -Path $work -ItemType Directory | Out-Null
$installed = Join-Path $work 'Installed'
$cli = Join-Path $installed 'whereisit-ai-cli.exe'
$gui = Join-Path $installed 'whereisit-ai-egui.exe'
$db = Join-Path $work 'external.sqlite'
$sidecar = Join-Path $work 'generated-user-sidecar.sqlite'
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
    $probeArgs = @($Phase, '--cli', $cli, '--db', $db, '--fixture-root', $fixture,
        '--snapshot', $snapshot, '--source-release', $sourceVersion, '--target-release', $targetVersion)
    if ($targetVersion -eq '1.1.1') { $probeArgs += @('--user-store', $sidecar) }
    & python $probe @probeArgs
    if ($LASTEXITCODE -ne 0) { throw "Synthetic catalog probe failed during $Phase." }
}
function Verify-External([string]$CatalogSha, [string]$CtfSha) {
    if ((File-Sha $db) -ne $CatalogSha -or (File-Sha $placeholder) -ne $CtfSha -or
        (File-Sha $sidecar) -ne $script:sidecarSha) {
        throw 'Installer lifecycle changed generated external data.'
    }
}

$oldSha = File-Sha $old
$newSha = File-Sha $new
Install-Product $old $SourceRoot $sourceVersion
Run-Probe prepare
$beforeUpgrade = File-Sha $db
$ctfSha = File-Sha $placeholder
# Older pairs use a generated marker file. The hotfix pair asks the 1.1.0 CLI
# to create the actual application-owned preferences SQLite store via its API.
if ($targetVersion -ne '1.1.1') {
    & python -c "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute('CREATE TABLE sidecar_fixture (value TEXT NOT NULL)'); c.execute('INSERT INTO sidecar_fixture VALUES (?)', ('synthetic user sidecar',)); c.commit(); c.close()" $sidecar
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $sidecar -PathType Leaf)) { throw 'Could not create synthetic external user sidecar.' }
}
$sidecarSha = File-Sha $sidecar
Install-Product $new $targetRoot $targetVersion
Verify-External $beforeUpgrade $ctfSha
Run-Probe verify
$beforeRollback = File-Sha $db
Remove-Product
Verify-External $beforeRollback $ctfSha
Install-Product $old $SourceRoot $sourceVersion
Verify-External $beforeRollback $ctfSha
& $cli validate $db | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Reinstalled predecessor could not validate the external catalog.' }
Remove-Product
Verify-External $beforeRollback $ctfSha

$out = Join-Path $targetRoot 'dist/qa-egui-upgrade'
New-Item -Path $out -ItemType Directory -Force | Out-Null
@{
    format = "whereisit-egui-$sourceVersion-to-$targetVersion-upgrade-v1"
    platform = 'windows-x64'
    source_version = $sourceVersion
    target_version = $targetVersion
    source_commit = $sourceSha
    target_commit = $targetSha
    source_package_sha256 = $oldSha
    target_package_sha256 = $newSha
    external_data_preserved = $true
    external_sidecar_sha256 = $sidecarSha
    external_sidecar_preserved = (File-Sha $sidecar) -eq $sidecarSha
    catalog_search_and_category_parity = $true
    rollback_reinstall_and_validation = $true
    synthetic_only = $true
    gui_observed = $false
    status = 'pass'
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'UPGRADE.json') -Encoding utf8
