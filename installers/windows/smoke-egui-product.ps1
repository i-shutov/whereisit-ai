# Hosted synthetic lifecycle contract for the side-by-side egui product installer.
param([string]$Installer = '', [string]$InstallDir = '', [string]$Catalog = '', [string]$BuildDir = '')
$ErrorActionPreference = 'Stop'
if ($env:CI -ne 'true') { throw 'This installed-product smoke is restricted to hosted CI.' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$version = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
if (-not $Installer) { $Installer = Join-Path $root "dist/windows-egui-product/WhereIsItAI-egui-$version-windows-x64-setup.exe" }
if (-not $BuildDir) { $BuildDir = Join-Path $root 'dist/windows-egui-product/payload' }
if (-not (Test-Path -LiteralPath $Installer -PathType Leaf)) { throw 'Missing exact egui product installer.' }
$tmp = $env:RUNNER_TEMP
if (-not $tmp) { $tmp = [IO.Path]::GetTempPath() }
if (-not $InstallDir) { $InstallDir = Join-Path $tmp 'WhereIsItAI-EguiProduct-SmokeInstall' }
if (-not $Catalog) { $Catalog = Join-Path $tmp 'whereisit-egui-product-smoke.atlas.sqlite' }
if (Test-Path -LiteralPath $InstallDir) { throw 'Refusing to reuse existing product install path.' }
if (Test-Path -LiteralPath $Catalog) { throw 'Refusing to reuse existing generated catalog path.' }
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
function Invoke-InstalledCliJson([string]$Json) {
    $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = $cli; $start.Arguments = 'serve'; $start.UseShellExecute = $false
    $start.CreateNoWindow = $true; $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = $utf8NoBom; $start.StandardErrorEncoding = $utf8NoBom
    $process = New-Object System.Diagnostics.Process; $process.StartInfo = $start
    $oldInputEncoding = [Console]::InputEncoding
    try {
        [Console]::InputEncoding = $utf8NoBom
        if (-not $process.Start()) { throw 'Could not start installed CLI.' }
        $outTask = $process.StandardOutput.ReadToEndAsync(); $errTask = $process.StandardError.ReadToEndAsync()
        $bytes = $utf8NoBom.GetBytes($Json + [char]10)
        $process.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length); $process.StandardInput.Close()
        $process.WaitForExit(); $stdout = $outTask.GetAwaiter().GetResult(); $null = $errTask.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) { throw 'Installed CLI serve transport failed.' }
        $lines = @($stdout -split '\r?\n' | Where-Object { $_ -ne '' })
        if ($lines.Count -ne 1) { throw 'Installed CLI returned ambiguous JSON.' }
        return ($lines[0] | ConvertFrom-Json -ErrorAction Stop)
    } finally { [Console]::InputEncoding = $oldInputEncoding; $process.Dispose() }
}
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Windows x64 required.' }
$expectedGui = Join-Path $BuildDir 'atlas-desktop-egui.exe'; $expectedCli = Join-Path $BuildDir 'atlas.exe'
foreach ($exe in @($expectedGui, $expectedCli)) { if ((Get-PeMachine $exe) -ne 0x8664) { throw "Source binary is not AMD64: $exe" } }
Invoke-Silent $Installer @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',('/DIR="' + $InstallDir + '"'))
$gui = Join-Path $InstallDir 'whereisit-ai-egui.exe'; $cli = Join-Path $InstallDir 'whereisit-ai-cli.exe'
foreach ($exe in @($gui, $cli)) { if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Missing installed product EXE: $exe" }; if ((Get-PeMachine $exe) -ne 0x8664) { throw "Installed EXE is not AMD64: $exe" } }
if ((Get-FileHash -LiteralPath $gui -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $expectedGui -Algorithm SHA256).Hash) { throw 'Installed GUI SHA differs from staged standard product GUI.' }
if ((Get-FileHash -LiteralPath $cli -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $expectedCli -Algorithm SHA256).Hash) { throw 'Installed CLI SHA differs from staged standard product CLI.' }
$legacy = Join-Path $root 'target/release/atlas-windows.exe'
if ((Test-Path -LiteralPath $legacy -PathType Leaf) -and (Get-FileHash $gui -Algorithm SHA256).Hash -eq (Get-FileHash $legacy -Algorithm SHA256).Hash) { throw 'Product package contains legacy Win32 UI.' }
foreach ($file in @('LICENSE','THIRD_PARTY_NOTICES.md')) { if (-not (Test-Path -LiteralPath (Join-Path $InstallDir $file) -PathType Leaf)) { throw "Missing legal notice: $file" } }
if ((& $cli --version).Trim() -ne "WhereIsIt AI $version" -or $LASTEXITCODE -ne 0) { throw 'Installed CLI version mismatch.' }
$response = Invoke-InstalledCliJson (@{op='init'; db=$Catalog} | ConvertTo-Json -Compress)
if ($response.ok -ne $true -or -not (Test-Path -LiteralPath $Catalog -PathType Leaf)) { throw 'Could not create generated external SQLite.' }
$before = (Get-FileHash -LiteralPath $Catalog -Algorithm SHA256).Hash
$uninstallers = @(Get-ChildItem -LiteralPath $InstallDir -Filter 'unins*.exe' -File)
if ($uninstallers.Count -ne 1) { throw 'Expected exactly one product uninstaller.' }
Invoke-Silent $uninstallers[0].FullName @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART')
Start-Sleep -Seconds 2
if ((Test-Path -LiteralPath $gui) -or (Test-Path -LiteralPath $cli)) { throw 'Product binaries survived uninstall.' }
if (-not (Test-Path -LiteralPath $Catalog) -or (Get-FileHash -LiteralPath $Catalog -Algorithm SHA256).Hash -ne $before) { throw 'Uninstall removed or changed generated external SQLite.' }
Write-Host 'EGUI_PRODUCT_INSTALL_SMOKE_PASS: AMD64, standard GUI/CLI hashes, notices, side-by-side identity, uninstall preserves external SQLite'
