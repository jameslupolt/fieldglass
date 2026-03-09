param(
    [switch]$Release,
    [switch]$OpenOutput
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$crateDir = Join-Path $repoRoot "crates\inat-scr-windows"
$targetDir = Join-Path $repoRoot "target"
$profile = if ($Release) { "release" } else { "debug" }

Write-Host "Building Windows screensaver host ($profile)..." -ForegroundColor Cyan

Push-Location $repoRoot
try {
    $cargoArgs = @("build", "-p", "inat-scr-windows")
    if ($Release) {
        $cargoArgs += "--release"
    }

    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed"
    }
}
finally {
    Pop-Location
}

$exePath = Join-Path $targetDir "$profile\FieldGlass.exe"
if (-not (Test-Path $exePath)) {
    throw "Expected build output not found: $exePath"
}

$outDir = Join-Path $repoRoot "dist\windows"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$scrPath = Join-Path $outDir "FieldGlass.scr"
Copy-Item -Force $exePath $scrPath

Write-Host "Built screensaver:" -ForegroundColor Green
Write-Host "  $scrPath"

Write-Host "\nQuick local mode tests:" -ForegroundColor Yellow
Write-Host "  $scrPath /c"
Write-Host "  $scrPath /p 0"
Write-Host "  $scrPath /s"

if ($OpenOutput) {
    Start-Process explorer.exe $outDir | Out-Null
}
