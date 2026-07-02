#!/usr/bin/env bash
# Build script for GRID Launcher
# This script builds the application into a standalone Linux executable

echo ""
echo "================================"
echo "GRID Launcher Build Script"
echo "================================"
echo ""

# Check if virtual environment exists
if [ ! -f ".venv/bin/activate" ]; then
    echo "ERROR: Virtual environment not found at ./.venv"
    echo "Please run: python3 -m venv .venv"
    exit 1
fi

# Activate virtual environment
echo "Activating virtual environment..."
source .venv/bin/activate

# Install/check PyInstaller
echo "Checking dependencies..."
python -m pip list | grep -qi PyInstaller
if [ $? -ne 0 ]; then
    echo "Installing PyInstaller..."
    python -m pip install pyinstaller
fi

# Run PyInstaller
echo ""
echo "Building executable..."
echo ""

# --windowed is a no-op on Linux (PyInstaller will print a harmless warning); kept for CLI consistency with the Windows scripts
python -m PyInstaller \
    --noconfirm \
    --clean \
    --windowed \
    --onefile \
    --name grid-launcher \
    --add-data "assets:assets" \
    --add-data "retroarch-core-list.json:." \
    --add-data "romm-platform-cores.json:." \
    --add-data "emulator-autoprofiles.json:." \
    --hidden-import brotli \
    --hidden-import pyzstd \
    --hidden-import pyppmd \
    --hidden-import keyring.backends.SecretService \
    --hidden-import keyring.backends.kwallet \
    grid-launcher.py

BUILD_EXIT_CODE=$?

# Check if build was successful
if [ $BUILD_EXIT_CODE -eq 0 ]; then
    echo ""
    echo "================================"
    echo "BUILD SUCCESSFUL!"
    echo "================================"
    echo ""
    echo "Executable location:"
    echo "  ./dist/grid-launcher"
    echo ""
    echo "You can now:"
    echo "  1. Run it directly: ./dist/grid-launcher"
    echo "  2. Move it to any location"
    echo "  3. Share the executable file"
else
    echo ""
    echo "BUILD FAILED"
    echo "Exit code: $BUILD_EXIT_CODE"
    exit $BUILD_EXIT_CODE
fi
