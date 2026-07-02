@echo off
REM Build script for rom-mate-neo
REM This script builds the application into a standalone Windows executable

echo.
echo ================================
echo rom-mate-neo Build Script
echo ================================
echo.

REM Check if virtual environment exists
if not exist ".venv\Scripts\activate.bat" (
    echo ERROR: Virtual environment not found at .venv
    echo Please run: python -m venv .venv
    exit /b 1
)

REM Activate virtual environment
echo Activating virtual environment...
call .venv\Scripts\activate.bat

REM Install/check PyInstaller
echo Checking dependencies...
python -m pip list | findstr PyInstaller >nul
if errorlevel 1 (
    echo Installing PyInstaller...
    python -m pip install pyinstaller
)

REM Run PyInstaller
echo.
echo Building executable...
echo.

python -m PyInstaller ^
    --noconfirm ^
    --clean ^
    --windowed ^
    --onefile ^
    --name rom-mate-neo ^
    --add-data "assets;assets" ^
    --add-data "retroarch-core-list.json;." ^
    --add-data "emulator-autoprofiles.json;." ^
    --hidden-import brotli ^
    --hidden-import pyzstd ^
    --hidden-import pyppmd ^
    --hidden-import keyring.backends.Windows ^
    rom-mate.py

REM Check if build was successful
if %errorlevel% equ 0 (
    echo.
    echo ================================
    echo BUILD SUCCESSFUL!
    echo ================================
    echo.
    echo Executable location:
    echo   .\dist\rom-mate-neo.exe
    echo.
    echo You can now:
    echo   1. Run it directly: .\dist\rom-mate-neo.exe
    echo   2. Move it to any location
    echo   3. Share the executable file
) else (
    echo.
    echo BUILD FAILED
    echo Exit code: %errorlevel%
    exit /b %errorlevel%
)
