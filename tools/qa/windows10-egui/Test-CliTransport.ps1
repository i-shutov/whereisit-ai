#Requires -Version 5.1
param([Parameter(Mandatory=$true)][string]$Cli)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2
$script:Cli = (Resolve-Path -LiteralPath $Cli).Path
$utf8NoBom = [Text.UTF8Encoding]::new($false, $true)
$source = Join-Path $PSScriptRoot 'Invoke-WhereIsItEguiQA.ps1'
$tokens = $null; $errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($source,[ref]$tokens,[ref]$errors)
if ($errors.Count) { throw 'Runner parse error' }
foreach ($name in @('Invoke-CliRaw','Invoke-Cli')) {
    $fn = $ast.Find({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name }, $true)
    if (-not $fn) { throw "Missing function $name" }
    Invoke-Expression $fn.Extent.Text
}
$literal = Invoke-CliRaw '{"op":"not_an_operation"}'
if ($literal.error.code -eq 'invalid_json') { throw "Literal ASCII JSON rejected: $($literal.error.message)" }
$serialized = @{op='not_an_operation'} | ConvertTo-Json -Compress -Depth 10
$parsed = Invoke-CliRaw $serialized
if ($parsed.error.code -eq 'invalid_json') { throw "Serialized ASCII JSON rejected: $($parsed.error.message)" }
$root = Join-Path $env:TEMP ('whereisit-qa-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $root | Out-Null
try {
    $unicode = -join ([char[]](0x041a,0x0438,0x043d,0x043e,0x0020,0x96ea))
    foreach ($label in @('ASCII disk', $unicode)) {
        $media = Join-Path $root $label
        New-Item -ItemType Directory $media | Out-Null
        [IO.File]::WriteAllText((Join-Path $media 'file.txt'),'fixture')
        $scan = Invoke-Cli @{op='scan';db=(Join-Path $root 'catalog space.sqlite');root=$media;label=$label;hash=$false}
        if (-not $scan.ok) { throw 'scan failed' }
    }
    $volumes = Invoke-Cli @{op='volumes';db=(Join-Path $root 'catalog space.sqlite')}
    $labels = @($volumes.data | ForEach-Object { $_.label })
    if ($labels -notcontains 'ASCII disk' -or $labels -notcontains $unicode) { throw 'UTF-8 response did not round-trip labels' }
    $bad = Invoke-CliRaw '{invalid'
    if ($bad.ok -ne $false -or $bad.error.code -ne 'invalid_json') { throw 'Malformed request was not rejected' }
    $failure = Invoke-Cli @{op='not_an_operation'} -AllowFailure
    if ($failure.ok -ne $false -or -not $failure.error.code) { throw 'Error response not decoded' }
    Write-Host 'PASS: native CLI UTF-8 round-trip, spaced paths, malformed JSON, service errors'
} finally {
    Remove-Item -LiteralPath $root -Recurse -Force
}
