# CI-only installer smoke test: install -> CLI catalog -> uninstall -> data survives.
# Run on an isolated Windows runner after Inno Setup created the candidate.
param(
    [string]$Installer = '',
    [string]$InstallDir = ''
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (-not $Installer) {
    $items = @(Get-ChildItem -Path (Join-Path $root 'dist/windows') -Filter 'WhereIsItAI-*-setup.exe')
    if ($items.Count -ne 1) { throw "Expected exactly one installer, found $($items.Count)" }
    $Installer = $items[0].FullName
}
if (-not $InstallDir) { $InstallDir = Join-Path $env:RUNNER_TEMP 'WhereIsItAI-SmokeInstall' }
if (Test-Path $InstallDir) { throw 'Refusing to delete an existing install directory' }
$catalog = Join-Path $env:RUNNER_TEMP 'whereisit-smoke-data.atlas.sqlite'
if (Test-Path $catalog) { throw 'Refusing to reuse an existing test database' }

function Invoke-SilentInstaller([string]$Exe, [string[]]$Arguments) {
    $process = Start-Process -FilePath $Exe -ArgumentList $Arguments -NoNewWindow -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        throw "Installer/uninstaller returned exit code $($process.ExitCode)"
    }
}
try {
    $dirArg = '/DIR="' + $InstallDir + '"'
    Invoke-SilentInstaller $Installer @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART', $dirArg)
    $cli = Join-Path $InstallDir 'whereisit-ai-cli.exe'
    $gui = Join-Path $InstallDir 'whereisit-ai.exe'
    if (-not (Test-Path $cli) -or -not (Test-Path $gui)) {
        throw 'Installed GUI or CLI executable is missing'
    }
    foreach ($file in @('LICENSE','THIRD_PARTY_NOTICES.md','licenses/SQLite-3.51.3/COPYRIGHT.txt')) {
        if (-not (Test-Path (Join-Path $InstallDir $file) -PathType Leaf)) { throw "Missing installed notice: $file" }
    }
    $sqliteVersion = (& $cli --sqlite-version).Trim()
    if ($LASTEXITCODE -ne 0 -or $sqliteVersion -ne '3.51.3') {
        throw "Installed CLI embeds incorrect SQLite: $sqliteVersion (expected 3.51.3)"
    }
    Write-Host "PASS: installed Windows CLI embeds SQLite $sqliteVersion"
    $request = @{op='init'; db=$catalog} | ConvertTo-Json -Compress
    $answer = & $cli request $request
    if ($LASTEXITCODE -ne 0) { throw "Installed CLI init failed ($LASTEXITCODE)" }
    $response = $answer | ConvertFrom-Json
    if ($response.ok -ne $true) { throw "Installed CLI returned an error" }
    if (-not (Test-Path $catalog)) { throw 'Catalog was not created outside application directory' }
    $uninstaller = @(Get-ChildItem $InstallDir -Filter 'unins*.exe')
    if ($uninstaller.Count -ne 1) { throw 'Missing or ambiguous Inno uninstaller' }
    Invoke-SilentInstaller $uninstaller[0].FullName @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART')
    Start-Sleep -Seconds 2
    if (Test-Path $cli) { throw 'Application executable survived uninstall unexpectedly' }
    if (-not (Test-Path $catalog)) { throw 'Uninstaller removed the user catalog!' }
    Write-Host 'PASS: silent setup, installed notices, installed CLI init, uninstall, catalog preservation'
} finally {
    # Do not delete catalog on failed run: leave it for runner diagnostics.
    # The runner is ephemeral and stores no real user data.
}
