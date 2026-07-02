# Build script for GRID Launcher
# This script builds the application into a standalone Windows executable

Write-Host "================================" -ForegroundColor Cyan
Write-Host "GRID Launcher Build Script" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""

# Check if virtual environment exists
if (-not (Test-Path ".\.venv\Scripts\Activate.ps1")) {
    Write-Host "ERROR: Virtual environment not found at .\.venv" -ForegroundColor Red
    Write-Host "Please run: python -m venv .venv" -ForegroundColor Yellow
    exit 1
}

# Activate virtual environment
Write-Host "Activating virtual environment..." -ForegroundColor Yellow
& .\.venv\Scripts\Activate.ps1

# Check if PyInstaller is installed
Write-Host "Checking dependencies..." -ForegroundColor Yellow
$pyInstaller = & python -m pip list | Select-String "PyInstaller"
if (-not $pyInstaller) {
    Write-Host "Installing PyInstaller..." -ForegroundColor Yellow
    python -m pip install pyinstaller
}

# Run PyInstaller
Write-Host ""
Write-Host "Building executable..." -ForegroundColor Yellow
Write-Host ""

python -m PyInstaller `
    --noconfirm `
    --clean `
    --windowed `
    --onefile `
    --name grid-launcher `
    --add-data "assets;assets" `
    --add-data "retroarch-core-list.json;." `
    --add-data "romm-platform-cores.json;." `
    --add-data "emulator-autoprofiles.json;." `
    --hidden-import brotli `
    --hidden-import pyzstd `
    --hidden-import pyppmd `
    --hidden-import keyring.backends.Windows `
    grid-launcher.py

# Check if build was successful
if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "================================" -ForegroundColor Green
    Write-Host "BUILD SUCCESSFUL!" -ForegroundColor Green
    Write-Host "================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Executable location:" -ForegroundColor Cyan
    Write-Host "  .\dist\grid-launcher.exe" -ForegroundColor White
    Write-Host ""
    Write-Host "You can now:" -ForegroundColor Cyan
    Write-Host "  1. Run it directly: .\dist\grid-launcher.exe" -ForegroundColor White
    Write-Host "  2. Move it to any location" -ForegroundColor White
    Write-Host "  3. Share the executable file" -ForegroundColor White
} else {
    Write-Host ""
    Write-Host "BUILD FAILED" -ForegroundColor Red
    Write-Host "Exit code: $LASTEXITCODE" -ForegroundColor Red
    exit $LASTEXITCODE
}
