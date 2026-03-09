param(
    [switch]$SkipFrontend,
    [switch]$OpenOutput
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$frontendDir = Join-Path $repoRoot "frontend"
$tauriDir = Join-Path $repoRoot "crates\inat-companion"

# 1. Build frontend assets (unless skipped)
if (-not $SkipFrontend) {
    Write-Host "Installing frontend dependencies..." -ForegroundColor Cyan
    Push-Location $frontendDir
    try {
        npm install
        if ($LASTEXITCODE -ne 0) {
            throw "npm install failed"
        }
    }
    finally {
        Pop-Location
    }
}

# 2. Build companion with Tauri (embeds frontend into binary)
Write-Host "Building companion app (production)..." -ForegroundColor Cyan

Push-Location $tauriDir
try {
    cargo tauri build
    if ($LASTEXITCODE -ne 0) {
        throw "cargo tauri build failed"
    }
}
finally {
    Pop-Location
}

# 3. Verify output
$releaseBinary = Join-Path $repoRoot "target\release\inat-companion.exe"
if (-not (Test-Path $releaseBinary)) {
    throw "Expected release binary not found: $releaseBinary"
}

$size = (Get-Item $releaseBinary).Length / 1MB
Write-Host "`nCompanion built successfully:" -ForegroundColor Green
Write-Host "  $releaseBinary ($([math]::Round($size, 1)) MB)"

Write-Host "`nTo install alongside screensaver:" -ForegroundColor Yellow
Write-Host "  .\install-screensaver.ps1 -Release -SetAsActive"

if ($OpenOutput) {
    Start-Process explorer.exe (Split-Path -Parent $releaseBinary) | Out-Null
}
