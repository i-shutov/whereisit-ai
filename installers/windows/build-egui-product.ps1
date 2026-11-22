param([string]$Version = '', [string]$ISCC = '', [string]$BuildDir = '', [string]$OutputDir = '')
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$expected = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
if (-not $Version) { $Version = $expected }
if ($Version -ne $expected -or $Version -notmatch '^\d+\.\d+\.\d+(-rc\.\d+)?$') { throw 'Egui product version must match VERSION.' }
# RC remains prerelease-only; stable 1.0 requires the explicit owner-approved Windows scope.
$releaseLabel = 'unsigned prerelease'
if ($Version -notmatch '-rc\.\d+$') {
    $acceptance = Get-Content (Join-Path $root 'release/ACCEPTANCE.json') -Raw | ConvertFrom-Json
    $scope = $acceptance.release_scope
    if ($Version -ne '1.0.0' -or $acceptance.version -ne $Version -or $acceptance.channel -ne 'stable' -or
        $scope.target_version -ne $Version -or $scope.unsigned_windows_stable_approved -ne $true -or
        $scope.usb_binding_enabled -ne $false -or @($scope.ship_platforms).Count -ne 1 -or
        $scope.ship_platforms[0] -ne 'windows-x64') { throw 'Stable Windows package requires explicit owner-approved scope.' }
    $releaseLabel = 'unsigned Windows release'
}
if (-not $BuildDir) { $BuildDir = Join-Path $root 'dist/windows-egui-product/payload' }
if (-not $OutputDir) { $OutputDir = Join-Path $root 'dist/windows-egui-product' }
$gui = Join-Path $BuildDir 'atlas-desktop-egui.exe'
$cli = Join-Path $BuildDir 'atlas.exe'
foreach ($exe in @($gui, $cli)) { if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Missing standard product executable: $exe" } }
$actual = & $cli --version
if ($LASTEXITCODE -ne 0 -or $actual.Trim() -ne "WhereIsIt AI $Version") { throw 'Compiled CLI version differs from VERSION.' }
if (-not $ISCC) {
    $pf86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
    if ($pf86) {
        $candidate = Join-Path $pf86 'Inno Setup 6/ISCC.exe'
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { $ISCC = $candidate }
    }
    if (-not $ISCC) { $found = Get-Command ISCC.exe -ErrorAction SilentlyContinue; if ($found) { $ISCC = $found.Source } }
}
if (-not $ISCC -or -not (Test-Path -LiteralPath $ISCC -PathType Leaf)) { throw 'Inno Setup 6 ISCC.exe required.' }
$out = Join-Path $OutputDir "WhereIsItAI-egui-$Version-windows-x64-setup.exe"
if (Test-Path -LiteralPath $out) { throw 'Refusing product installer overwrite.' }
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
& $ISCC "/DAppVersion=$Version" "/DReleaseLabel=$releaseLabel" "/DBuildDir=$BuildDir" "/DOutputDir=$OutputDir" (Join-Path $PSScriptRoot 'WhereIsItAI-EguiWin64.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC failed: $LASTEXITCODE" }
if (-not (Test-Path -LiteralPath $out -PathType Leaf)) { throw 'Missing egui product installer.' }
Write-Host "EGUI_PRODUCT_INSTALLER_BUILT: $releaseLabel"
