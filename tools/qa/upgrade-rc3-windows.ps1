# Windows clean-room, genuine RC2 -> RC3 in-place upgrade QA.
# Run only inside a disposable GitHub Actions Windows runner.
param(
    [string]$OldTag = 'v1.0.0-rc.2',
    [string]$NewVersion = '1.0.0-rc.3'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:CI -ne 'true' -or -not $env:RUNNER_TEMP) {
    throw 'Upgrade simulation is restricted to an isolated CI Windows runner.'
}
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$version = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
if ($version -ne $NewVersion) { throw 'Upgrade target/version mismatch.' }
if ($OldTag -ne 'v1.0.0-rc.2') { throw 'Only the SHA-pinned RC2 predecessor is allowed.' }
$oldName = 'WhereIsItAI-1.0.0-rc.2-windows-x64-setup.exe'
$oldSha = '06b0dd7abd7d784b5e096e611cf23526df37da2ae7511dfd6d3bb4d0f69d24e4'
$upgradeWork = Join-Path $env:RUNNER_TEMP ("whereisit-upgrade-" + $env:GITHUB_RUN_ID)
if (Test-Path $upgradeWork) { throw 'Refusing to overwrite a previous upgrade test root.' }
New-Item $upgradeWork -ItemType Directory | Out-Null
$oldInstaller = Join-Path $upgradeWork $oldName
$newInstaller = Join-Path $root "dist/windows/WhereIsItAI-$NewVersion-windows-x64-setup.exe"
$installed = Join-Path $upgradeWork 'PerUserApp'
$db = Join-Path $upgradeWork 'existing-user-catalog.sqlite'
$fixture = Join-Path $upgradeWork 'synthetic-fixture'
$snapshot = Join-Path $upgradeWork 'baseline.json'
$probe = Join-Path $root 'tools/qa/upgrade_catalog_probe.py'
$out = Join-Path $root 'dist/qa-upgrade/windows'
$installedByTest = $false

function Run-Installer([string]$File, [string[]]$ArgumentList) {
    $process = Start-Process -FilePath $File -ArgumentList $ArgumentList -PassThru -Wait -NoNewWindow
    if ($process.ExitCode -ne 0) { throw "Installer returned code $($process.ExitCode)" }
}
function Verify-CliVersion([string]$Path, [string]$Expected) {
    if (-not (Test-Path $Path -PathType Leaf)) { throw 'Installed CLI missing.' }
    $v = (& $Path --version).Trim()
    if ($LASTEXITCODE -ne 0 -or $v -ne "WhereIsIt AI $Expected") {
        throw 'Incorrect installed CLI after upgrade.'
    }
}
try {
    $url = "https://github.com/i-shutov/whereisit-ai/releases/download/$OldTag/$oldName"
    Invoke-WebRequest -Uri $url -OutFile $oldInstaller -MaximumRetryCount 3 -RetryIntervalSec 2
    $found = (Get-FileHash $oldInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($found -ne $oldSha) { throw 'RC2 installer checksum differs from pinned release digest.' }
    if (-not (Test-Path $newInstaller -PathType Leaf)) {
        throw 'Current exact-source RC3 installer is missing.'
    }
    if (Test-Path $installed) { throw 'Refusing to replace existing app directory.' }
    $dirArg = '/DIR="' + $installed + '"'
    $flags = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', $dirArg)
    # This test owns the empty installation directory; cleaning may be attempted
    # only after a successful first installation.
    Run-Installer $oldInstaller $flags
    $installedByTest = $true
    $cli = Join-Path $installed 'whereisit-ai-cli.exe'
    Verify-CliVersion $cli '1.0.0-rc.2'
    & python $probe prepare --cli $cli --db $db --fixture-root $fixture --snapshot $snapshot
    if ($LASTEXITCODE -ne 0) { throw 'RC2 catalog creation failed.' }
    $originalDbSha = (Get-FileHash $db -Algorithm SHA256).Hash.ToLowerInvariant()

    Run-Installer $newInstaller $flags
    Verify-CliVersion $cli $NewVersion
    $sqliteVersion = (& $cli --sqlite-version).Trim()
    if ($LASTEXITCODE -ne 0 -or $sqliteVersion -ne '3.51.3') {
        throw 'Installed RC3 has incorrect SQLite runtime.'
    }
    $unchangedDbSha = (Get-FileHash $db -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($unchangedDbSha -ne $originalDbSha) {
        throw 'In-place upgrade modified the existing external user catalog bytes.'
    }
    & python $probe verify --cli $cli --db $db --fixture-root $fixture --snapshot $snapshot
    if ($LASTEXITCODE -ne 0) { throw 'Existing RC2 catalog not readable in RC3.' }

    $uninstallers = @(Get-ChildItem $installed -Filter 'unins*.exe')
    if ($uninstallers.Count -ne 1) {
        throw "Expected one uninstaller after in-place upgrade, got $($uninstallers.Count)"
    }
    Run-Installer $uninstallers[0].FullName @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART')
    $installedByTest = $false
    Start-Sleep -Seconds 2
    if (Test-Path $cli) { throw 'RC3 CLI survived uninstall unexpectedly.' }
    if (-not (Test-Path $db -PathType Leaf)) { throw 'Upgrade uninstall removed external user database.' }
    if (-not (Test-Path (Join-Path $fixture 'external-user-ctf-placeholder.CTF'))) {
        throw 'Uninstaller removed external CTF placeholder.'
    }
    New-Item $out -ItemType Directory -Force | Out-Null
    @{
        format = 'whereisit-rc2-to-rc3-upgrade-qa-v1'
        platform = 'windows-x64'
        old = '1.0.0-rc.2'
        new = $NewVersion
        source_commit = (git rev-parse HEAD)
        old_installer_sha256 = $oldSha
        new_installer_sha256 = (Get-FileHash $newInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
        old_db_bytes_untouched_by_installer = $true
        category_and_search_parity = $true
        original_ctf_placeholder_untouched = $true
        uninstall_preserved_catalog = $true
        actual_installed_sqlite = $sqliteVersion
        only_generated_fixture_used = $true
        status = 'pass'
    } | ConvertTo-Json | Set-Content (Join-Path $out 'UPGRADE-WINDOWS.json') -Encoding utf8
    Write-Host 'PASS: Windows RC2 -> RC3 in-place upgrade, preserved catalog and external CTF.'
}
finally {
    # Never delete arbitrary directories. Only invoke the uninstaller located
    # in the CI-owned temporary install root. Preserve evidence on a failed run.
    if ($installedByTest -and (Test-Path $installed)) {
        $uninstaller = @(Get-ChildItem $installed -Filter 'unins*.exe' -ErrorAction SilentlyContinue)
        if ($uninstaller.Count -eq 1) {
            try { Run-Installer $uninstaller[0].FullName @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') }
            catch { Write-Warning 'CI-owned test installer cleanup failed.' }
        }
    }
}
