#!/usr/bin/env bash
# Build script for GRID Launcher
# This script builds the application into a standalone Linux executable

echo ""
echo "================================"
echo "GRID Launcher Build Script"
echo "================================"
echo ""

# Build mode: default is the standalone onefile binary; --appimage builds an AppImage.
BUILD_MODE="$1"

# pygame 2.6.1 has no Python 3.14 wheel — build requires Python 3.12 or 3.13.
BUILD_PYTHON=$(command -v python3.12 || command -v python3.13)
if [ -z "$BUILD_PYTHON" ]; then
    echo "ERROR: Python 3.12 or 3.13 is required to build (pygame has no 3.14 wheel)"
    echo "Install with: sudo dnf install python3.12  # or python3.13"
    exit 1
fi

# Recreate the venv if it is missing or was built with the wrong Python.
VENV_PYTHON=".venv/bin/python"
if [ -f "$VENV_PYTHON" ]; then
    VENV_VER=$("$VENV_PYTHON" -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
    BUILD_VER=$("$BUILD_PYTHON" -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
    if [ "$VENV_VER" != "$BUILD_VER" ]; then
        echo "Venv Python ($VENV_VER) does not match build Python ($BUILD_VER) — recreating venv..."
        rm -rf .venv
    fi
fi
if [ ! -f "$VENV_PYTHON" ]; then
    echo "Creating virtual environment with $("$BUILD_PYTHON" --version)..."
    "$BUILD_PYTHON" -m venv .venv
fi

# Use the venv's pip/python directly — avoids activation edge cases.
VENV_PIP=".venv/bin/pip"
VENV_PYTHON=".venv/bin/python"

# Install app dependencies + PyInstaller into the venv.
echo "Activating virtual environment..."
echo "Checking dependencies..."
"$VENV_PIP" install -r requirements.txt
"$VENV_PIP" install pyinstaller

if [ "$BUILD_MODE" = "--appimage" ]; then
    # Run PyInstaller in onedir mode for AppImage packaging
    echo ""
    echo "Building AppImage..."
    echo ""

    # --windowed is a no-op on Linux (PyInstaller will print a harmless warning); kept for CLI consistency with the Windows scripts
    "$VENV_PYTHON" -m PyInstaller \
        --noconfirm \
        --clean \
        --windowed \
        --onedir \
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

    if [ $BUILD_EXIT_CODE -ne 0 ]; then
        echo ""
        echo "BUILD FAILED"
        echo "Exit code: $BUILD_EXIT_CODE"
        exit $BUILD_EXIT_CODE
    fi

    # Rasterize the app icon (requires rsvg-convert from librsvg2-bin)
    echo ""
    echo "Rasterizing app icon..."
    rsvg-convert -w 256 -h 256 \
        assets/svg/io.github.Sixdd6.GRIDLauncher.svg \
        -o appimage/grid-launcher.png

    # Assemble AppDir
    echo "Assembling AppDir..."
    rm -rf AppDir
    mkdir -p AppDir/usr/bin
    cp -r dist/grid-launcher/* AppDir/usr/bin/
    cp appimage/AppRun AppDir/AppRun
    chmod +x AppDir/AppRun
    cp appimage/grid-launcher.desktop AppDir/grid-launcher.desktop
    cp appimage/grid-launcher.png AppDir/grid-launcher.png

    # Download appimagetool if it isn't already present
    if [ ! -x "./appimagetool" ]; then
        echo "Downloading appimagetool..."
        wget -q -O appimagetool \
            "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
        chmod +x appimagetool
    fi

    # Build the AppImage
    echo "Building AppImage..."
    ./appimagetool --appimage-extract-and-run AppDir grid-launcher-x86_64.AppImage
    BUILD_EXIT_CODE=$?

    if [ $BUILD_EXIT_CODE -eq 0 ]; then
        echo ""
        echo "================================"
        echo "BUILD SUCCESSFUL!"
        echo "================================"
        echo ""
        echo "AppImage location:"
        echo "  ./grid-launcher-x86_64.AppImage"
    else
        echo ""
        echo "BUILD FAILED"
        echo "Exit code: $BUILD_EXIT_CODE"
        exit $BUILD_EXIT_CODE
    fi
else
    # Run PyInstaller
    echo ""
    echo "Building executable..."
    echo ""

    # --windowed is a no-op on Linux (PyInstaller will print a harmless warning); kept for CLI consistency with the Windows scripts
    "$VENV_PYTHON" -m PyInstaller \
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
fi
