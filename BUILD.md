# Building GRID Launcher

Build scripts are provided for easy building on Windows and Linux.

## Quick Start

### Windows

#### Option 1: PowerShell (Recommended)
```powershell
.\build.ps1
```

#### Option 2: Command Prompt / Batch
```cmd
build.bat
```

### Linux

```bash
chmod +x build.sh   # one-time
./build.sh
```

## What the Scripts Do

This applies to all three scripts (`build.ps1`, `build.bat`, `build.sh`):

1. **Verify Setup** — Checks that the virtual environment exists
2. **Activate Environment** — Sets up the Python environment with required dependencies (the Linux script activates via `.venv/bin/activate` instead of `.venv\Scripts\Activate.ps1`)
3. **Install Dependencies** — Ensures PyInstaller is installed
4. **Build Executable** — Creates a standalone `grid-launcher` executable with all assets bundled
5. **Report Status** — Shows success/failure and output location

## Output

On successful build, the executable will be located at:
```
.\dist\grid-launcher.exe
```

The executable is self-contained and can be:
- Run directly: Double-click `grid-launcher.exe`
- Moved anywhere on your system
- Distributed to others (single file, no dependencies needed)

### Linux

On successful build, the binary will be located at:
```
./dist/grid-launcher
```

The executable bit is already set by PyInstaller, so it can be run directly:
```bash
./dist/grid-launcher
```

## Troubleshooting

### Virtual Environment Not Found
If you see "Virtual environment not found", initialize it first:
```powershell
python -m venv .venv
```

On Linux:
```bash
python3 -m venv .venv
```

### PowerShell Execution Policy
If PowerShell refuses to run the script, you can either:
1. Temporarily allow script execution:
   ```powershell
   Set-ExecutionPolicy -Scope Process -ExecutionPolicy RemoteSigned
   .\build.ps1
   ```

2. Or use the batch file instead:
   ```cmd
   build.bat
   ```

### "Permission denied" running ./build.sh
Make the script executable first:
```bash
chmod +x build.sh
```

### Build Fails
Check the output for specific errors. Common issues:
- Missing Python 3.12+ — Update Python
- Missing dependencies — Try: `python -m pip install -r requirements.txt`
- Disk space — Ensure ~2GB free space for build artifacts

### Linux: Missing Shared Library at Runtime
If the packaged Linux binary fails at runtime with missing shared library errors (e.g. `libEGL`, `libxcb`, `libxkbcommon-x11`), install the corresponding package from your distro's package manager. This is an environment issue, not a code bug.

### Linux: ImportError for py7zr Compression Backend
`py7zr` uses dynamic/lazy imports for its compression backends (bz2/lzma/zstd/brotli/ppmd). If the built Linux binary throws an `ImportError` for one of these during archive extraction, add explicit `--hidden-import` flags to `build.sh` (e.g. `--hidden-import brotli --hidden-import pyzstd --hidden-import pyppmd`).

## Manual Build (if scripts don't work)

### Windows
```powershell
# Activate virtual environment
.\.venv\Scripts\Activate.ps1

# Install PyInstaller
python -m pip install pyinstaller

# Run build
python -m PyInstaller --noconfirm --clean --windowed --onefile --name grid-launcher --add-data "assets;assets" --add-data "retroarch-core-list.json;." --add-data "emulator-autoprofiles.json;." grid-launcher.py
```

### Linux
```bash
# Activate virtual environment
source .venv/bin/activate

# Install PyInstaller
python -m pip install pyinstaller

# Run build
python -m PyInstaller --noconfirm --clean --windowed --onefile --name grid-launcher --add-data "assets:assets" --add-data "retroarch-core-list.json:." --add-data "emulator-autoprofiles.json:." grid-launcher.py
```
