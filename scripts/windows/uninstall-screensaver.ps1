<#
.SYNOPSIS
    Removes all locally installed Field Glass artifacts.

.DESCRIPTION
    Cleans up everything placed by install-screensaver.ps1 or manual testing:
      - .scr from System32
      - companion-path sidecar from System32
      - active-screensaver registry keys
      - settings JSON
      - photo cache (images + SQLite DB)
      - build artifacts (target/, dist/)

    Does NOT uninstall an MSI — use "Add/Remove Programs" for that.

.PARAMETER KeepSettings
    Preserve settings.json (only remove cache + installed files).

.PARAMETER KeepCache
    Preserve the photo cache (only remove installed files + settings).

.PARAMETER SkipClean
    Skip `cargo clean` and dist/ removal.

.EXAMPLE
    .\uninstall-screensaver.ps1
    .\uninstall-screensaver.ps1 -KeepSettings
    .\uninstall-screensaver.ps1 -KeepCache -SkipClean
#>
param(
    [switch]$KeepSettings,
    [switch]$KeepCache,
    [switch]$SkipClean
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")

# ── Stop companion process if running ──────────────────────────────────

$companion = Get-Process -Name "field-glass", "Field Glass" -ErrorAction SilentlyContinue
if ($companion) {
    Write-Host "Stopping companion app..." -ForegroundColor Yellow
    $companion | Stop-Process -Force
    Start-Sleep -Milliseconds 500
    Write-Host "  Stopped." -ForegroundColor Green
} else {
    Write-Host "Companion app not running." -ForegroundColor DarkGray
}

# ── Remove .scr and sidecar from System32 ─────────────────────────────

$scrPath = Join-Path $env:WINDIR "System32\FieldGlass.scr"
$sidecarPath = Join-Path $env:WINDIR "System32\FieldGlass.companion-path.txt"

foreach ($file in @($scrPath, $sidecarPath)) {
    if (Test-Path $file) {
        Remove-Item -Force $file
        Write-Host "  Removed: $file" -ForegroundColor Green
    } else {
        Write-Host "  Not found: $file" -ForegroundColor DarkGray
    }
}

# ── Clear active-screensaver registry ──────────────────────────────────

$desktopKey = "HKCU:\Control Panel\Desktop"
$currentScr = (Get-ItemProperty -Path $desktopKey -Name "SCRNSAVE.EXE" -ErrorAction SilentlyContinue)."SCRNSAVE.EXE"

if ($currentScr -and $currentScr -like "*FieldGlass*") {
    Set-ItemProperty -Path $desktopKey -Name "SCRNSAVE.EXE" -Value ""
    Write-Host "  Cleared active screensaver registry entry." -ForegroundColor Green
} else {
    Write-Host "  Registry: not set to FieldGlass (no change)." -ForegroundColor DarkGray
}

# ── Remove settings ───────────────────────────────────────────────────

if (-not $KeepSettings) {
    $configDir = Join-Path $env:APPDATA "field-glass"
    if (Test-Path $configDir) {
        Remove-Item -Recurse -Force $configDir
        Write-Host "  Removed settings: $configDir" -ForegroundColor Green
    } else {
        Write-Host "  No settings directory found." -ForegroundColor DarkGray
    }
} else {
    Write-Host "  Keeping settings (--KeepSettings)." -ForegroundColor Yellow
}

# ── Remove cache ──────────────────────────────────────────────────────

if (-not $KeepCache) {
    $cacheDir = Join-Path $env:LOCALAPPDATA "field-glass"
    if (Test-Path $cacheDir) {
        $cacheSize = [math]::Round((Get-ChildItem -Recurse $cacheDir | Measure-Object -Property Length -Sum).Sum / 1MB, 1)
        Remove-Item -Recurse -Force $cacheDir
        Write-Host "  Removed cache: $cacheDir ($cacheSize MB)" -ForegroundColor Green
    } else {
        Write-Host "  No cache directory found." -ForegroundColor DarkGray
    }
} else {
    Write-Host "  Keeping cache (--KeepCache)." -ForegroundColor Yellow
}

# ── Clean build artifacts ─────────────────────────────────────────────

if (-not $SkipClean) {
    $distDir = Join-Path $repoRoot "dist"
    if (Test-Path $distDir) {
        Remove-Item -Recurse -Force $distDir
        Write-Host "  Removed dist/." -ForegroundColor Green
    }

    Write-Host "  Running cargo clean..." -ForegroundColor Cyan
    Push-Location $repoRoot
    try {
        $ErrorActionPreference = "SilentlyContinue"; cargo clean 2>&1 | Out-Null; $ErrorActionPreference = "Stop"
        Write-Host "  cargo clean complete." -ForegroundColor Green
    } finally {
        Pop-Location
    }
} else {
    Write-Host "  Skipping build clean (--SkipClean)." -ForegroundColor Yellow
}

# ── Summary ───────────────────────────────────────────────────────────

Write-Host ""
Write-Host "Uninstall complete." -ForegroundColor Green
Write-Host ""
Write-Host "Note: if you installed via MSI, use Add/Remove Programs to uninstall that separately." -ForegroundColor Yellow
