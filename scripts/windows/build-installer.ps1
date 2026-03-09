<#
.SYNOPSIS
    Builds the Field Glass MSI installer using WiX v4.

.DESCRIPTION
    Orchestrates the full build pipeline:
      1. Builds the screensaver .scr (release)
      2. Builds the companion app (release, Tauri)
      3. Compiles the WiX installer into an MSI

    Prerequisites:
      - Rust toolchain (cargo)
      - Node.js + npm (for companion frontend)
      - WiX v4 CLI: dotnet tool install --global wix
      - WiX UI extension: wix extension add WixToolset.UI.wixext

.PARAMETER SkipBuild
    Skip building .scr and companion (use existing artifacts).

.PARAMETER SkipFrontend
    Skip npm install for companion frontend (passed to build-companion.ps1).

.PARAMETER ProductVersion
    Version string for the MSI (default: 0.1.0.0).

.EXAMPLE
    .\build-installer.ps1
    .\build-installer.ps1 -SkipBuild -ProductVersion "1.0.0.0"
#>
param(
    [switch]$SkipBuild,
    [switch]$SkipFrontend,
    [string]$ProductVersion = "0.1.0.0"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$installerDir = Join-Path $repoRoot "installer\windows"
$distDir = Join-Path $repoRoot "dist\windows"

# Artifact paths
$scrPath = Join-Path $distDir "FieldGlass.scr"
$companionPath = Join-Path $repoRoot "target\release\inat-companion.exe"
$iconPath = Join-Path $repoRoot "crates\inat-companion\icons\icon.ico"
$licensePath = Join-Path $installerDir "License.rtf"
$wxsPath = Join-Path $installerDir "Product.wxs"
$msiPath = Join-Path $distDir "FieldGlass.msi"

# ── Step 0: Check prerequisites ─────────────────────────────────────────

Write-Host "Checking prerequisites..." -ForegroundColor Cyan

# Check for WiX CLI
$wixCmd = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wixCmd) {
    Write-Host "ERROR: WiX CLI not found." -ForegroundColor Red
    Write-Host ""
    Write-Host "Install WiX v4:" -ForegroundColor Yellow
    Write-Host "  dotnet tool install --global wix"
    Write-Host ""
    Write-Host "Then add the UI extension:" -ForegroundColor Yellow
    Write-Host "  wix extension add WixToolset.UI.wixext"
    exit 1
}

Write-Host "  WiX CLI: $(wix --version)" -ForegroundColor Green

# ── Step 1: Build artifacts ─────────────────────────────────────────────

if (-not $SkipBuild) {
    Write-Host ""
    Write-Host "Building screensaver (.scr)..." -ForegroundColor Cyan
    $buildScr = Join-Path $repoRoot "scripts\windows\build-screensaver.ps1"
    & $buildScr -Release
    if ($LASTEXITCODE -ne 0) {
        throw "Screensaver build failed"
    }

    Write-Host ""
    Write-Host "Building companion app..." -ForegroundColor Cyan
    $buildCompanion = Join-Path $repoRoot "scripts\windows\build-companion.ps1"
    $companionArgs = @()
    if ($SkipFrontend) {
        $companionArgs += "-SkipFrontend"
    }
    & $buildCompanion @companionArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Companion build failed"
    }
}

# ── Step 2: Verify all inputs exist ─────────────────────────────────────

Write-Host ""
Write-Host "Verifying build artifacts..." -ForegroundColor Cyan

$requiredFiles = @{
    "Screensaver .scr"  = $scrPath
    "Companion .exe"    = $companionPath
    "App icon .ico"     = $iconPath
    "License .rtf"      = $licensePath
    "WiX source .wxs"   = $wxsPath
}

$missing = @()
foreach ($entry in $requiredFiles.GetEnumerator()) {
    if (Test-Path $entry.Value) {
        $size = (Get-Item $entry.Value).Length
        $sizeStr = if ($size -gt 1MB) { "$([math]::Round($size / 1MB, 1)) MB" } else { "$([math]::Round($size / 1KB, 1)) KB" }
        Write-Host "  OK  $($entry.Key): $($entry.Value) ($sizeStr)" -ForegroundColor Green
    } else {
        Write-Host "  MISSING  $($entry.Key): $($entry.Value)" -ForegroundColor Red
        $missing += $entry.Key
    }
}

if ($missing.Count -gt 0) {
    throw "Missing required files: $($missing -join ', '). Run without -SkipBuild to build them."
}

# ── Step 3: Build MSI ───────────────────────────────────────────────────

Write-Host ""
Write-Host "Building MSI installer..." -ForegroundColor Cyan
Write-Host "  Version: $ProductVersion"

New-Item -ItemType Directory -Force -Path $distDir | Out-Null

# Build with WiX v4 CLI
$wixArgs = @(
    "build",
    $wxsPath,
    "-arch", "x64",
    "-ext", "WixToolset.UI.wixext",
    "-d", "ProductVersion=$ProductVersion",
    "-d", "ScrPath=$scrPath",
    "-d", "CompanionPath=$companionPath",
    "-d", "IconPath=$iconPath",
    "-d", "LicensePath=$licensePath",
    "-o", $msiPath
)

Write-Host "  wix $($wixArgs -join ' ')" -ForegroundColor DarkGray
& wix @wixArgs

if ($LASTEXITCODE -ne 0) {
    throw "WiX build failed (exit code $LASTEXITCODE)"
}

# ── Step 4: Report ──────────────────────────────────────────────────────

if (Test-Path $msiPath) {
    $msiSize = [math]::Round((Get-Item $msiPath).Length / 1MB, 1)
    Write-Host ""
    Write-Host "MSI built successfully:" -ForegroundColor Green
    Write-Host "  $msiPath ($msiSize MB)"
    Write-Host ""
    Write-Host "To install:" -ForegroundColor Yellow
    Write-Host "  msiexec /i `"$msiPath`""
    Write-Host ""
    Write-Host "To install silently:" -ForegroundColor Yellow
    Write-Host "  msiexec /i `"$msiPath`" /qn"
} else {
    throw "MSI file not found after build: $msiPath"
}
