param(
    [switch]$Build = $true,
    [switch]$Release,
    [switch]$SetAsActive,
    [string]$CompanionPath,
    [string]$SystemPath = "$env:WINDIR\System32\FieldGlass.scr"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$distScr = Join-Path $repoRoot "dist\windows\FieldGlass.scr"
$buildScript = Join-Path $repoRoot "scripts\windows\build-screensaver.ps1"

function Resolve-CompanionPath {
    param(
        [string]$RequestedPath,
        [string]$RepoRoot
    )

    if ($RequestedPath) {
        return (Resolve-Path $RequestedPath -ErrorAction Stop).Path
    }

    # Prefer release builds — debug Tauri binaries need a dev server running
    $candidates = @(
        (Join-Path $RepoRoot "target\release\inat-companion.exe"),
        (Join-Path $RepoRoot "target\debug\inat-companion.exe"),
        (Join-Path $RepoRoot "crates\inat-companion\target\release\inat-companion.exe"),
        (Join-Path $RepoRoot "crates\inat-companion\target\debug\inat-companion.exe")
    )

    if ($env:LOCALAPPDATA) {
        (Join-Path $env:LOCALAPPDATA "Programs\Field Glass\Field Glass.exe")
        (Join-Path $env:LOCALAPPDATA "Programs\Field Glass\Field Glass.exe")
    }

    if ($env:ProgramFiles) {
        $candidates += (Join-Path $env:ProgramFiles "Field Glass\Field Glass.exe")
    }

    if (${env:ProgramFiles(x86)}) {
        $candidates += (Join-Path ${env:ProgramFiles(x86)} "Field Glass\Field Glass.exe")
    }

    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            return (Resolve-Path $candidate).Path
        }
    }

    return $null
}

if ($Build) {
    if ($Release) {
        & $buildScript -Release
    } else {
        & $buildScript
    }
    if ($LASTEXITCODE -ne 0) {
        throw "Build script failed"
    }
}

if (-not (Test-Path $distScr)) {
    throw "Screensaver file not found: $distScr"
}

Write-Host "Installing screensaver to: $SystemPath" -ForegroundColor Cyan
Copy-Item -Force $distScr $SystemPath

$resolvedCompanion = Resolve-CompanionPath -RequestedPath $CompanionPath -RepoRoot $repoRoot
if ($resolvedCompanion) {
    # Warn if using a debug build (needs dev server)
    if ($resolvedCompanion -like '*\debug\*') {
        Write-Host "WARNING: Using debug companion build. It will only work while 'cargo tauri dev' is running." -ForegroundColor Red
        Write-Host "Run .\build-companion.ps1 first for a standalone companion." -ForegroundColor Red
    }
    $sidecarPath = Join-Path (Split-Path -Parent $SystemPath) "FieldGlass.companion-path.txt"
    Set-Content -Path $sidecarPath -Value $resolvedCompanion -NoNewline
    Write-Host "Using companion executable: $resolvedCompanion" -ForegroundColor Green
    Write-Host "Saved companion path sidecar: $sidecarPath" -ForegroundColor Green
} else {
    Write-Host "Could not auto-discover companion executable." -ForegroundColor Yellow
    Write-Host "Run .\build-companion.ps1 first, then re-run this script." -ForegroundColor Yellow
}

Write-Host "Installed successfully." -ForegroundColor Green

if ($SetAsActive) {
    Write-Host "Setting active screensaver in current user registry..." -ForegroundColor Cyan
    Set-ItemProperty -Path "HKCU:\Control Panel\Desktop" -Name SCRNSAVE.EXE -Value $SystemPath
    Set-ItemProperty -Path "HKCU:\Control Panel\Desktop" -Name ScreenSaveActive -Value "1"
    Write-Host "Registry updated. Open Screen Saver Settings to confirm." -ForegroundColor Green
}

Write-Host "\nNext:" -ForegroundColor Yellow
Write-Host "  1) Run: control desk.cpl,,1"
Write-Host "  2) Select FieldGlass"
Write-Host "  3) Click Preview and Settings"
