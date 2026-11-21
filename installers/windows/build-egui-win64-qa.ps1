# Standalone unsigned Win64 native Rust/egui QA installer; never replaces legacy RC3.
param([string]$Version = '', [string]$ISCC = '', [string]$BuildDir = '', [string]$OutputDir = '')
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$expected = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
if (-not $Version) { $Version = $expected }
if ($Version -ne $expected -or $Version -notmatch '^\d+\.\d+\.\d+(-rc\.\d+)?$') {
    throw 'QA installer version must match VERSION.'
}
if (-not $BuildDir) { $BuildDir = Join-Path $root 'target/release' }
if (-not $OutputDir) { $OutputDir = Join-Path $root 'dist/windows-egui-win64-qa' }
$gui = Join-Path $BuildDir 'atlas-desktop-egui.exe'
$cli = Join-Path $BuildDir 'atlas.exe'
foreach ($exe in @($gui, $cli)) {
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Missing release executable: $exe" }
}
$actual = & $cli --version
if ($LASTEXITCODE -ne 0 -or $actual.Trim() -ne "WhereIsIt AI $Version") {
    throw 'Compiled CLI version differs from VERSION.'
}
if (-not $ISCC) {
    $pf86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
    if ($pf86) {
        $candidate = Join-Path $pf86 'Inno Setup 6/ISCC.exe'
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { $ISCC = $candidate }
    }
    if (-not $ISCC) {
        $found = Get-Command ISCC.exe -ErrorAction SilentlyContinue
        if ($found) { $ISCC = $found.Source }
    }
}
if (-not $ISCC -or -not (Test-Path -LiteralPath $ISCC -PathType Leaf)) { throw 'Inno Setup 6 required.' }
$out = Join-Path $OutputDir "WhereIsItAI-egui-win64-QA-$Version-setup.exe"
if (Test-Path -LiteralPath $out) { throw 'Refusing QA installer overwrite.' }
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
& $ISCC "/DAppVersion=$Version" "/DBuildDir=$BuildDir" "/DOutputDir=$OutputDir" (Join-Path $PSScriptRoot 'WhereIsItAI-EguiWin64QA.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC failed: $LASTEXITCODE" }
if (-not (Test-Path -LiteralPath $out -PathType Leaf)) { throw 'Missing compiled egui Win64 installer.' }
Write-Host 'WIN64_EGUI_INSTALLER_BUILT: unsigned QA only'
