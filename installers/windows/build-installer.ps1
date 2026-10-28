param(
    [string]$Version = '',
    [string]$ISCC = '',
    [string]$BuildDir = '',
    [string]$OutputDir = ''
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$expected = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
if (-not $Version) { $Version = $expected }
if ($Version -ne $expected -or $Version -notmatch '^\d+\.\d+\.\d+(-rc\.\d+)?$') { throw 'Installer version must match VERSION.' }
if (-not $BuildDir) { $BuildDir = Join-Path $root 'target/release' }
if (-not $OutputDir) { $OutputDir = Join-Path $root 'dist/windows' }
$cli = Join-Path $BuildDir 'atlas.exe'
if (-not (Test-Path (Join-Path $BuildDir 'atlas-windows.exe'))) { throw 'Missing atlas-windows.exe. Build Rust release first.' }
if (-not (Test-Path $cli)) { throw 'Missing atlas.exe. Build Rust release first.' }
$actual = & $cli --version
if ($LASTEXITCODE -ne 0 -or $actual.Trim() -ne "WhereIsIt AI $Version") { throw 'Compiled binary version differs from installer version.' }
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
