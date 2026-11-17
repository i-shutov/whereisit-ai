# Native egui side-by-side QA ZIP; never replaces the official Win32 installer.
$ErrorActionPreference = 'Stop'
if ($env:CI -ne 'true' -or $env:RUNNER_OS -ne 'Windows') { throw 'CI runner only' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$version = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
$exe = Join-Path $root 'target/release/atlas-desktop-egui.exe'
$cli = Join-Path $root 'target/release/atlas.exe'
$script = Join-Path $root 'tools/qa/windows10-egui/Invoke-WhereIsItEguiQA.ps1'
$instructions = Join-Path $root 'tools/qa/windows10-egui/README.md'
if (-not (Test-Path $cli -PathType Leaf) -or -not (Test-Path $script -PathType Leaf)) {
    throw 'Matching native CLI or Windows 10 QA runner is missing.'
}
if (-not (Test-Path $exe -PathType Leaf)) { throw 'Native egui binary missing' }
$out = Join-Path $root 'dist/qa-egui'
$payload = Join-Path $out 'windows-x64'
if (Test-Path $payload) { throw 'Refusing QA bundle overwrite' }
New-Item -ItemType Directory $payload -Force | Out-Null
Copy-Item $exe (Join-Path $payload 'WhereIsItAI-egui-preview.exe')
Copy-Item $cli (Join-Path $payload 'whereisit-ai-cli.exe')
# PowerShell 5.1 must read UTF-8 Cyrillic source with an explicit BOM.
$scriptText = Get-Content -LiteralPath $script -Raw -Encoding UTF8
[IO.File]::WriteAllText(
    (Join-Path $payload 'Invoke-WhereIsItEguiQA.ps1'),
    $scriptText,
    [System.Text.UTF8Encoding]::new($true)
)
Copy-Item $instructions (Join-Path $payload 'README-Win10-QA.md')
@{
    schema = 'whereisit-egui-qa-bundle-v1'
    commit = (git rev-parse HEAD)
    version = $version
    gui_sha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
    cli_sha256 = (Get-FileHash -LiteralPath $cli -Algorithm SHA256).Hash.ToLowerInvariant()
    scope = 'local QA preview only'
} | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $payload 'QA_MANIFEST.json') -Encoding UTF8
Copy-Item (Join-Path $root 'LICENSE') $payload
Copy-Item (Join-Path $root 'THIRD_PARTY_NOTICES.md') $payload
Copy-Item (Join-Path $root 'licenses') (Join-Path $payload 'licenses') -Recurse
@'
WhereIsIt AI native egui QA PREVIEW. Unsigned, not a stable installer.
Use backed-up or synthetic catalogs. Do not treat CI as physical USB acceptance.
No private CTF, user SQLite, source paths or signing keys are included.
'@ | Set-Content (Join-Path $payload 'README-QA.txt') -Encoding utf8
$forbidden = @(Get-ChildItem $payload -Recurse -File | Where-Object { $_.Extension -in @('.ctf','.sqlite','.db') })
if ($forbidden.Count -gt 0) { throw 'Private catalog-like files in QA package' }
$zip = Join-Path $out "WhereIsItAI-egui-preview-$version-windows-x64.zip"
if (Test-Path $zip) { throw 'QA archive exists' }
Compress-Archive -Path (Join-Path $payload '*') -DestinationPath $zip -CompressionLevel Optimal
$sha = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
"$sha  $(Split-Path $zip -Leaf)" | Set-Content (Join-Path $out 'SHA256-windows-x64.txt') -Encoding ascii
@{scope='QA-preview-only';version=$version;commit=(git rev-parse HEAD);architecture='windows-x64';signing='unsigned';user_catalogs_included=$false} | ConvertTo-Json | Set-Content (Join-Path $out 'provenance-windows-x64.json') -Encoding utf8
Write-Host 'PASS: side-by-side egui Windows QA archive and SHA256'
