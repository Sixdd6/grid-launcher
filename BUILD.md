# Building rom-mate-neo

Two build scripts are provided for easy building on Windows.

## Quick Start

### Option 1: PowerShell (Recommended)
```powershell
.\build.ps1
```

### Option 2: Command Prompt / Batch
```cmd
build.bat
```

## What the Scripts Do

1. **Verify Setup** — Checks that the virtual environment exists
2. **Activate Environment** — Sets up the Python environment with required dependencies
3. **Install Dependencies** — Ensures PyInstaller is installed
4. **Build Executable** — Creates a standalone `rom-mate-neo.exe` with all assets bundled
5. **Report Status** — Shows success/failure and output location

## Output

On successful build, the executable will be located at:
```
.\dist\rom-mate-neo.exe
```

The executable is self-contained and can be:
- Run directly: Double-click `rom-mate-neo.exe`
- Moved anywhere on your system
- Distributed to others (single file, no dependencies needed)

## Troubleshooting

### Virtual Environment Not Found
If you see "Virtual environment not found", initialize it first:
```powershell
python -m venv .venv
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

### Build Fails
Check the output for specific errors. Common issues:
- Missing Python 3.12+ — Update Python
- Missing dependencies — Try: `python -m pip install -r requirements.txt`
- Disk space — Ensure ~2GB free space for build artifacts

## Manual Build (if scripts don't work)

```powershell
# Activate virtual environment
.\.venv\Scripts\Activate.ps1

# Install PyInstaller
python -m pip install pyinstaller

# Run build
python -m PyInstaller --noconfirm --clean --windowed --onefile --name rom-mate-neo --add-data "assets;assets" --add-data "retroarch-core-list.json;." --add-data "emulator-autoprofiles.json;." rom-mate.py
```
