param(
    [string]$Version = '0.1.0',
    [string]$ISCC = '',
    [string]$BuildDir = '',
    [string]$OutputDir = ''
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (-not $BuildDir) { $BuildDir = Join-Path $root 'target/release' }
if (-not $OutputDir) { $OutputDir = Join-Path $root 'dist/windows' }
if (-not (Test-Path (Join-Path $BuildDir 'atlas-windows.exe'))) { throw 'Missing atlas-windows.exe. Build Rust release first.' }
if (-not (Test-Path (Join-Path $BuildDir 'atlas.exe'))) { throw 'Missing atlas.exe. Build Rust release first.' }
if (-not $ISCC) {
    $candidate = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe'
    if (Test-Path $candidate) { $ISCC = $candidate }
    else {
        $fromPath = Get-Command ISCC.exe -ErrorAction SilentlyContinue
        if ($fromPath) { $ISCC = $fromPath.Source }
    }
}
if (-not $ISCC -or -not (Test-Path $ISCC)) { throw 'Inno Setup 6 ISCC.exe is required.' }
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$script = Join-Path $PSScriptRoot 'WhereIsItAI.iss'
& $ISCC "/DAppVersion=$Version" "/DBuildDir=$BuildDir" "/DOutputDir=$OutputDir" $script
if ($LASTEXITCODE -ne 0) { throw "ISCC exited with $LASTEXITCODE" }
Write-Host "Installer written under $OutputDir"
