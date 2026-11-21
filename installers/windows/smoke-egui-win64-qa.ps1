# Headless installed Win64 egui QA: exact AMD64 binaries, notices, external SQLite preservation.
# No privileged self-hosted runner, real catalog, interactive GUI, or stable release claim.
param([string]$Installer = '', [string]$InstallDir = '', [string]$Catalog = '')
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$version = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
if (-not $Installer) {
    $Installer = Join-Path $root "dist/windows-egui-win64-qa/WhereIsItAI-egui-win64-QA-$version-setup.exe"
}
if (-not (Test-Path -LiteralPath $Installer -PathType Leaf)) { throw 'Missing exact Win64 QA installer.' }
$tmp = $env:RUNNER_TEMP
if (-not $tmp) { $tmp = [IO.Path]::GetTempPath() }
if (-not $InstallDir) { $InstallDir = Join-Path $tmp 'WhereIsItAI-EguiWin64QA-SmokeInstall' }
if (-not $Catalog) { $Catalog = Join-Path $tmp 'whereisit-egui-win64-qa-smoke.atlas.sqlite' }
if (Test-Path -LiteralPath $InstallDir) { throw 'Refusing to reuse existing installed application path.' }
if (Test-Path -LiteralPath $Catalog) { throw 'Refusing to reuse existing catalog path.' }
function Get-PeMachine([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    try {
        $reader = [IO.BinaryReader]::new($stream)
        if ($reader.ReadUInt16() -ne 0x5A4D) { throw 'Not an MZ executable.' }
        $stream.Seek(0x3C, [IO.SeekOrigin]::Begin) | Out-Null
        $offset = $reader.ReadInt32()
        if ($offset -lt 64 -or $offset -gt ($stream.Length - 6)) { throw 'Invalid PE header offset.' }
        $stream.Seek($offset, [IO.SeekOrigin]::Begin) | Out-Null
        if ($reader.ReadUInt32() -ne 0x00004550) { throw 'Invalid PE signature.' }
        return $reader.ReadUInt16()
    } finally { $stream.Dispose() }
}
function Invoke-Silent([string]$Exe, [string[]]$Arguments) {
    $p = Start-Process -FilePath $Exe -ArgumentList $Arguments -Wait -PassThru -NoNewWindow
    if ($p.ExitCode -ne 0) { throw "Installer/uninstaller returned $($p.ExitCode)" }
}
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Windows x64 required.' }
$expectedGui = Join-Path $root 'target/release/atlas-desktop-egui.exe'
$expectedCli = Join-Path $root 'target/release/atlas.exe'
foreach ($exe in @($expectedGui, $expectedCli)) {
    if ((Get-PeMachine $exe) -ne 0x8664) { throw "Source binary not AMD64: $exe" }
}
Invoke-Silent $Installer @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',('/DIR="' + $InstallDir + '"'))
$gui = Join-Path $InstallDir 'whereisit-egui-win64.exe'
$cli = Join-Path $InstallDir 'whereisit-ai-cli.exe'
foreach ($exe in @($gui, $cli)) {
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Missing installed EXE: $exe" }
    if ((Get-PeMachine $exe) -ne 0x8664) { throw "Installed EXE not AMD64: $exe" }
}
if ((Get-FileHash -LiteralPath $gui -Algorithm SHA256).Hash -ne
    (Get-FileHash -LiteralPath $expectedGui -Algorithm SHA256).Hash) {
    throw 'Installed GUI SHA differs from Rust/egui release EXE.'
}
if ((Get-FileHash -LiteralPath $cli -Algorithm SHA256).Hash -ne
    (Get-FileHash -LiteralPath $expectedCli -Algorithm SHA256).Hash) {
    throw 'Installed CLI SHA differs from release EXE.'
}
$legacy = Join-Path $root 'target/release/atlas-windows.exe'
if ((Test-Path -LiteralPath $legacy -PathType Leaf) -and
    ((Get-FileHash -LiteralPath $gui -Algorithm SHA256).Hash -eq
    (Get-FileHash -LiteralPath $legacy -Algorithm SHA256).Hash)) {
    throw 'Win64 QA installer incorrectly packaged legacy Win32 UI.'
}
foreach ($file in @('LICENSE','THIRD_PARTY_NOTICES.md')) {
    if (-not (Test-Path -LiteralPath (Join-Path $InstallDir $file) -PathType Leaf)) {
        throw "Missing legal notice: $file"
    }
}
$installedVersion = (& $cli --version).Trim()
if ($LASTEXITCODE -ne 0 -or $installedVersion -ne "WhereIsIt AI $version") {
    throw 'Installed CLI version mismatch.'
}
$request = @{op='init'; db=$Catalog} | ConvertTo-Json -Compress
$answer = & $cli request $request
$exitCode = $LASTEXITCODE
if ($exitCode -ne 0) { throw "Installed CLI request failed (exit=$exitCode)." }
if (-not $answer) { throw 'Installed CLI returned no response.' }
try {
    $response = $answer | ConvertFrom-Json -ErrorAction Stop
} catch {
    throw 'Installed CLI returned non-JSON data.'
}
if ($response.ok -ne $true) {
    $code = if ($response.error -and $response.error.code) { [string]$response.error.code } else { 'unavailable' }
    throw "Installed CLI refused synthetic SQLite init (error_code=$code)."
}
if (-not (Test-Path -LiteralPath $Catalog -PathType Leaf)) { throw 'External SQLite missing.' }
$before = (Get-FileHash -LiteralPath $Catalog -Algorithm SHA256).Hash
$uninstallers = @(Get-ChildItem -LiteralPath $InstallDir -Filter 'unins*.exe' -File)
if ($uninstallers.Count -ne 1) { throw 'Expected exactly one QA uninstaller.' }
Invoke-Silent $uninstallers[0].FullName @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART')
Start-Sleep -Seconds 2
if (Test-Path -LiteralPath $gui -PathType Leaf) { throw 'GUI survived uninstall.' }
if (Test-Path -LiteralPath $cli -PathType Leaf) { throw 'CLI survived uninstall.' }
if (-not (Test-Path -LiteralPath $Catalog -PathType Leaf)) { throw 'Uninstaller deleted external catalog.' }
$after = (Get-FileHash -LiteralPath $Catalog -Algorithm SHA256).Hash
if ($before -ne $after) { throw 'Uninstaller changed external catalog bytes.' }
Write-Host 'EGUI_WIN64_INSTALL_QA_PASS: AMD64 PE, GUI/CLI identity, notices, uninstall, external catalog preserved'
