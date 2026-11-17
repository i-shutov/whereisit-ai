#Requires -Version 5.1
<#
.SYNOPSIS
  Isolated native egui QA on an existing interactive Windows 10 Pro / RDP machine.
.DESCRIPTION
  Uses ONLY the unpacked QA-preview EXE and CLI, generates synthetic local
  test media, a separate SQLite catalog, and machine-local redacted evidence.
  Does not install drivers/services, alter firewall/SSH/VPN, or touch FlowBridge.
  -SkipGui runs headless synthetic service checks (also usable in hosted CI).
#>
[CmdletBinding()]
param(
    [string]$PreviewDir = $PSScriptRoot,
    [string]$EvidenceRoot = (Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'WhereIsItAI-QA'),
    [ValidateRange(2, 100)][int]$DiskCount = 60,
    [string]$ExpectedCommit = '',
    [switch]$SkipGui,
    [switch]$ManualReview,
    [switch]$CaptureScreenshot
)
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = $utf8NoBom
[Console]::OutputEncoding = $utf8NoBom

$script:Checks = [ordered]@{}
$script:PreviewProcess = $null
$script:Cli = ''
$script:Report = ''
$script:RunFolder = ''
$script:Manifest = $null
$script:Catalog = ''
$script:Manual = [ordered]@{}
$script:StartUtc = (Get-Date).ToUniversalTime().ToString('o')

function Add-Check([string]$Name, [string]$State, [string]$Detail) {
    $script:Checks[$Name] = [ordered]@{ status = $State; detail = $Detail }
    Write-Host ("[{0}] {1}: {2}" -f $State,$Name,$Detail)
}
function Assert-Check([bool]$Condition, [string]$Name, [string]$Detail) {
    if (-not $Condition) { throw "$Name : $Detail" }
    Add-Check $Name 'PASS' $Detail
}
function Invoke-CliRaw([string]$Json) {
    # One UTF-8 line without BOM; do not depend on host pipeline encoding.
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $script:Cli
    $info.Arguments = 'serve'
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.StandardOutputEncoding = $utf8NoBom
    $info.StandardErrorEncoding = $utf8NoBom
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $info
    try {
        if (-not $process.Start()) { throw 'Could not start native CLI.' }
        $outTask = $process.StandardOutput.ReadToEndAsync()
        $errTask = $process.StandardError.ReadToEndAsync()
        $bytes = $utf8NoBom.GetBytes($Json + [char]10)
        $process.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
        $process.StandardInput.Close()
        $process.WaitForExit()
        $stdout = $outTask.GetAwaiter().GetResult()
        $stderr = $errTask.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) { throw "Native CLI failed (exit=$($process.ExitCode)): $stderr" }
        $lines = @($stdout -split "\r?\n" | Where-Object { $_ -ne '' })
        if ($lines.Count -ne 1) { throw 'Native CLI returned ambiguous JSON output.' }
        return ($lines[0] | ConvertFrom-Json)
    } finally { $process.Dispose() }
}
function Invoke-Cli([hashtable]$Payload, [switch]$AllowFailure) {
    $json = $Payload | ConvertTo-Json -Compress -Depth 10
    $response = Invoke-CliRaw $json
    if (-not $AllowFailure -and $response.ok -ne $true) {
        $code = [string]$response.error.code
        throw "Native service rejected request: $code ($($response.error.message))"
    }
    return $response
}
function Read-Manual([string]$Key, [string]$Instruction) {
    Write-Host ""
    Write-Host $Instruction
    $answer = (Read-Host 'PASS / FAIL / BLOCKED').Trim().ToUpperInvariant()
    if ($answer -notin @('PASS','FAIL','BLOCKED')) { $answer = 'BLOCKED' }
    $script:Manual[$Key] = $answer
}
function Save-Report {
    if (-not $script:RunFolder -or -not (Test-Path $script:RunFolder)) { return }
    $os = [Environment]::OSVersion.Version.ToString()
    $display = 'unknown'
    try {
        Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
        $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $display = ('{0}x{1}' -f $bounds.Width,$bounds.Height)
    } catch {}
    $source = if ($script:Manifest) { [string]$script:Manifest.commit } else { 'unverified' }
    $report = [ordered]@{
        schema = 'whereisit-win10-egui-qa-v1'
        source_commit = $source
        started_utc = $script:StartUtc
        finished_utc = (Get-Date).ToUniversalTime().ToString('o')
        os_kernel_version = $os
        process_architecture = [Environment]::Is64BitOperatingSystem
        interactive_session = [Environment]::UserInteractive
        session_kind = if ($SkipGui) { 'headless' } elseif ($env:SESSIONNAME -like 'RDP*') { 'RDP' } else { 'interactive' }
        display_resolution = $display
        synthetic_disk_count = $DiskCount
        scope = 'generated fixtures only; local evidence; no user CTF/SQLite read'
        checks = $script:Checks
        manual_checks = $script:Manual
        hardware_usb_acceptance = 'NOT_RUN'
        pr58_visual_parity_accepted = $false
        stable_release_ready = $false
    }
    $jsonPath = Join-Path $script:RunFolder 'qa_report.json'
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $jsonPath -Encoding UTF8
    $lines = @('# WhereIsIt AI — local Win10 egui QA', '',
        'This report covers generated fixture data ONLY. Physical GUI/USB acceptance is separate.', '',
        ("Source commit: " + $source), ("Synthetic disks: " + $DiskCount),
        ("OS kernel: " + $os), ("Desktop resolution: " + $display), '',
        '## Automated checks')
    foreach ($key in $script:Checks.Keys) {
        $entry = $script:Checks[$key]
        $lines += ('- ' + $key + ': **' + $entry.status + '** — ' + $entry.detail)
    }
    $lines += @('', '## Manual RDP review')
    if ($script:Manual.Count -eq 0) { $lines += '- NOT_RUN (requires interactive RDP review)' }
    foreach ($key in $script:Manual.Keys) {
        $lines += ('- ' + $key + ': **' + $script:Manual[$key] + '**')
    }
    $lines += @('', '## Not accepted', '- Physical unplug/replug and USB UUID pairing: NOT_RUN',
        '- PR58 complete visual/keyboard/accessibility parity: NOT_ACCEPTED',
        '- Windows Authenticode and stable v1.0: NO_GO',
        '- Logs/screenshots stay local until manually redacted')
    $lines | Set-Content -LiteralPath (Join-Path $script:RunFolder 'qa_report.md') -Encoding UTF8
    $script:Report = $jsonPath
}
try {
    $resolved = (Resolve-Path -LiteralPath $PreviewDir).Path
    $gui = Join-Path $resolved 'WhereIsItAI-egui-preview.exe'
    $script:Cli = Join-Path $resolved 'whereisit-ai-cli.exe'
    $manifestPath = Join-Path $resolved 'QA_MANIFEST.json'
    Assert-Check ((Test-Path $gui -PathType Leaf) -and
        (Test-Path $script:Cli -PathType Leaf) -and
        (Test-Path $manifestPath -PathType Leaf)) 'PACKAGE' 'Preview EXE + CLI + manifest present'
    $script:Manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($ExpectedCommit -and $script:Manifest.commit -ne $ExpectedCommit) {
        throw 'Preview source commit differs from the expected exact SHA.'
    }
    foreach ($item in @(
        @{ name='WhereIsItAI-egui-preview.exe'; expected=$script:Manifest.gui_sha256 },
        @{ name='whereisit-ai-cli.exe'; expected=$script:Manifest.cli_sha256 }
    )) {
        $file = Join-Path $resolved $item.name
        $actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne [string]$item.expected) {
            throw "Packaged binary SHA256 mismatch: $($item.name)"
        }
    }
    Add-Check 'HASH' 'PASS' 'Exact SHA-256 matches QA manifest for GUI and CLI'
    if (-not [Environment]::Is64BitOperatingSystem) {
        throw 'x64 machine required.'
    }
    if (-not $SkipGui -and -not [Environment]::UserInteractive) {
        throw 'GUI tests require an unlocked interactive/RDP desktop.'
    }
    if ($SkipGui -and $ManualReview) { throw 'ManualReview cannot run with SkipGui.' }

    $EvidenceRoot = [IO.Path]::GetFullPath($EvidenceRoot)
    $unique = 'Run-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + ([Guid]::NewGuid().ToString('N').Substring(0,8))
    $script:RunFolder = Join-Path $EvidenceRoot $unique
    New-Item -Path $script:RunFolder -ItemType Directory -ErrorAction Stop | Out-Null
    $media = Join-Path $script:RunFolder 'synthetic-media'
    New-Item -Path $media -ItemType Directory | Out-Null
    $script:Catalog = Join-Path $script:RunFolder 'synthetic-catalog.sqlite'
    $knownVolumes = @()
    for ($n=1; $n -le $DiskCount; $n++) {
        $root = Join-Path $media ('disk-{0:D3}' -f $n)
        $film = Join-Path $root 'Кино'
        New-Item -Path $film -ItemType Directory -Force | Out-Null
        [IO.File]::WriteAllText((Join-Path $film ('clip-{0:D3}.mkv' -f $n)),
            'Generated synthetic metadata fixture only', [Text.Encoding]::UTF8)
        [IO.File]::WriteAllText((Join-Path $root 'notes.txt'),
            'Generated QA fixture', [Text.Encoding]::UTF8)
        $response = Invoke-Cli @{
            op='scan'; db=$script:Catalog; root=$root
            label=('Synthetic USB #{0:D3}' -f $n); hash=$false
        }
        $id = [int64]$response.data.id
        if ($id -le 0) { throw "No volume ID for synthetic disk $n." }
        $knownVolumes += $id
    }
    Assert-Check (($knownVolumes.Count -eq $DiskCount) -and
        ((@($knownVolumes | Select-Object -Unique)).Count -eq $DiskCount)) 'SCAN' "$DiskCount separate metadata-only disk snapshots"

    $volumeResponse = Invoke-Cli @{op='volumes';db=$script:Catalog}
    Assert-Check ($volumeResponse.data.Count -eq $DiskCount) 'VOLUMES' 'Real SQLite volume register matches generated fixture'
    $result = Invoke-Cli @{op='search_fast';db=$script:Catalog;query='ext:mkv';offset=0;limit=250}
    Assert-Check ($result.data.items.Count -eq $DiskCount) 'SEARCH' 'Global ext:mkv query across generated catalog'
    $validated = Invoke-Cli @{op='validate_catalog';db=$script:Catalog}
    Assert-Check ($validated.ok -eq $true) 'CATALOG' 'SQLite integrity and schema validated'

    $export = Join-Path $script:RunFolder 'export'
    New-Item -Path $export -ItemType Directory | Out-Null
    $csv = Join-Path $export 'movies.csv'
    $json = Join-Path $export 'movies.json'
    $parquet = Join-Path $export 'movies.parquet'
    foreach ($spec in @(
        @{op='export_csv';path=$csv},
        @{op='export_json';path=$json},
        @{op='export_parquet';path=$parquet}
    )) {
        $answer = Invoke-Cli @{op=$spec.op;db=$script:Catalog;path=$spec.path;query='ext:mkv';overwrite=$false}
        if ($answer.data.rows -ne $DiskCount) {
            throw "Export scope row count is wrong for $($spec.op)."
        }
    }
    $head = [IO.File]::ReadAllBytes($parquet)
    $headerOK = $head.Length -ge 8 -and [Text.Encoding]::ASCII.GetString($head,0,4) -eq 'PAR1'
    Assert-Check ((Test-Path $csv) -and (Test-Path $json) -and $headerOK) 'EXPORT' 'CSV, JSON and Parquet full-scope exports generated'
    $priorHash = (Get-FileHash -LiteralPath $csv -Algorithm SHA256).Hash
    $repeat = Invoke-Cli @{op='export_csv';db=$script:Catalog;path=$csv;overwrite=$false} -AllowFailure
    $newHash = (Get-FileHash -LiteralPath $csv -Algorithm SHA256).Hash
    Assert-Check (($repeat.ok -eq $false) -and
        ($repeat.error.code -eq 'file_exists') -and $priorHash -eq $newHash) 'NO_CLOBBER' 'Existing export refused without byte changes'

    $catalogHashBefore = (Get-FileHash -LiteralPath $script:Catalog -Algorithm SHA256).Hash
    $sidecar = Join-Path $script:RunFolder 'private-shelf-bindings.sqlite'
    $scope = 'sqlite:' + $script:Catalog
    foreach ($id in $knownVolumes[0..1]) {
        [void](Invoke-Cli @{
            op='physical_shelf_label_set';user_store=$sidecar;catalog_key=$scope
            platform='windows';volume_id=$id;shelf_label='SYNTHETIC-BOX-01';confirmed=$true
        })
    }
    $shelves = Invoke-Cli @{
        op='physical_shelf_labels';user_store=$sidecar;catalog_key=$scope
        platform='windows';volume_ids=$knownVolumes
    }
    Assert-Check (($shelves.data.items.Count -eq 2) -and
        ($shelves.data.collisions.Count -eq 1)) 'SHELF' 'Shared sticker warning in separate user sidecar'
    $physical = Invoke-Cli @{
        op='physical_status';user_store=$sidecar;catalog_key=$scope
        platform='windows';volume_ids=$knownVolumes;observed_ids=@();probe_ok=$true
    }
    $bad = @($physical.data.items | Where-Object { $_.state -ne 'unverified' })
    Assert-Check ($bad.Count -eq 0) 'PHYSICAL_TRUTH' 'Sticker is never evidence of live USB connection'

    $catalogHashAfter = (Get-FileHash -LiteralPath $script:Catalog -Algorithm SHA256).Hash
    Assert-Check ($catalogHashBefore -eq $catalogHashAfter) 'SIDECAR_IMMUTABILITY' 'Shelf operations did not alter catalog bytes'

    if (-not $SkipGui) {
        $argsLine = '--catalog "' + $script:Catalog + '"'
        $script:PreviewProcess = Start-Process -FilePath $gui -ArgumentList $argsLine -PassThru
        $window = [IntPtr]::Zero
        $deadline = (Get-Date).AddSeconds(25)
        while ((Get-Date) -lt $deadline) {
            Start-Sleep -Milliseconds 500
            $script:PreviewProcess.Refresh()
            if ($script:PreviewProcess.HasExited) { throw 'Native egui preview exited before opening a GUI window.' }
            $window = $script:PreviewProcess.MainWindowHandle
            if ($window -ne [IntPtr]::Zero) { break }
        }
        Assert-Check ($window -ne [IntPtr]::Zero) 'GUI_LAUNCH' 'Native egui main window exists in real interactive desktop'
        if ($CaptureScreenshot) {
            Add-Type -AssemblyName System.Drawing
            Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class WhereIsItQaRect {
    [StructLayout(LayoutKind.Sequential)]
    public struct R { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out R rectangle);
}
'@
            $rect = New-Object WhereIsItQaRect+R
            if ([WhereIsItQaRect]::GetWindowRect($window,[ref]$rect)) {
                $width = $rect.Right - $rect.Left
                $height = $rect.Bottom - $rect.Top
                if ($width -gt 0 -and $height -gt 0) {
                    $bmp = New-Object System.Drawing.Bitmap($width, $height)
                    $graphics = [System.Drawing.Graphics]::FromImage($bmp)
                    try {
                        $graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,$bmp.Size)
                        $bmp.Save((Join-Path $script:RunFolder 'egui-window-local-only.png'))
                        Add-Check 'SCREENSHOT' 'PASS' 'Only preview-window bounds captured locally; review before sharing'
                    } finally { $graphics.Dispose(); $bmp.Dispose() }
                }
            }
        }
        if ($ManualReview) {
            Read-Manual 'DISK_REGISTER_60' 'In the app verify all 60 synthetic catalog disks can be browsed.'
            Read-Manual 'GLOBAL_SEARCH' 'Find clip-060.mkv and verify file, recorded path, disk label and Back.'
            Read-Manual 'RU_EN_DPI' 'Test RU/EN and theme at current RDP DPI; controls must not clip.'
            Read-Manual 'INSPECTOR_SHELF' 'Open disk properties and verify sticker hints; no false connected badge.'
            Read-Manual 'SCAN_CANCEL' 'In app, scan a new synthetic folder and cancel a longer scan; verify rollback.'
        }
        if (-not $script:PreviewProcess.HasExited) {
            [void]$script:PreviewProcess.CloseMainWindow()
            if (-not $script:PreviewProcess.WaitForExit(5000)) {
                Add-Check 'GUI_CLOSE' 'FAIL' 'App did not exit via normal window close; process kept for investigation'
            } else {
                Add-Check 'GUI_CLOSE' 'PASS' 'Native application exited normally'
            }
        }
    } else {
        Add-Check 'GUI_LAUNCH' 'NOT_RUN' 'Headless CI mode; physical RDP GUI still required'
    }
} catch {
    Add-Check 'QA_BLOCKER' 'FAIL' ([string]$_.Exception.Message)
} finally {
    Save-Report
    if ($script:PreviewProcess -and -not $script:PreviewProcess.HasExited) {
        # Stop only the process this script itself created; never stop other apps.
        try { Stop-Process -Id $script:PreviewProcess.Id -ErrorAction Stop } catch {}
    }
    if ($script:Report) { Write-Host ("Local QA evidence: " + $script:Report) }
}
if (@($script:Manual.Values | Where-Object { $_ -eq 'FAIL' }).Count -gt 0) { exit 1 }
if ($script:Checks.Contains('QA_BLOCKER')) { exit 1 }
if (@($script:Checks.Values | Where-Object { $_.status -eq 'FAIL' }).Count -gt 0) { exit 1 }
exit 0
