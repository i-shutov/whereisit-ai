$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "../..")
try {
    cargo build --release -p atlas-windows --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed" }
    Write-Host "Built: target/x86_64-pc-windows-msvc/release/atlas-windows.exe"
} finally { Pop-Location }
