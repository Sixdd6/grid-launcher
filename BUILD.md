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

`build.sh` accepts one or more build targets:

| Command | Result |
| --- | --- |
| `./build.sh` | Default — builds the single-file `dist/grid-launcher` binary (backward compatible) |
| `./build.sh --onefile` | Same as the default — builds the single-file binary explicitly |
| `./build.sh --appimage` | Builds only the AppImage package |
| `./build.sh --onefile --appimage` | Builds both targets sequentially |

**Sequential execution and exit codes:** When multiple targets are given, they run one after another in the order listed. If any target fails, the script stops and exits with a non-zero status; it exits `0` only when all requested targets succeed. Unknown flags (and the unsupported `--windows` flag) cause the script to print usage and exit non-zero without building.

> **Windows cross-compilation is not supported.** `build.sh` only produces Linux artifacts. To build for Windows, run `build.bat` or `build.ps1` on a Windows machine.

## What the Scripts Do

This applies to all three scripts (`build.ps1`, `build.bat`, `build.sh`):

1. **Verify Setup** — Checks that the virtual environment exists
2. **Activate Environment** — Sets up the Python environment with required dependencies (the Linux script activates via `.venv/bin/activate` instead of `.venv\Scripts\Activate.ps1`)
3. **Install Dependencies** — Ensures PyInstaller is installed
4. **Stage Assets** — Runs `scripts/stage_assets.py` to copy only the assets the app actually references into `build/bundle-assets`
5. **Build Executable** — Creates a standalone `grid-launcher` executable with the staged assets bundled
6. **Report Status** — Shows success/failure and output location

### Asset staging (`scripts/stage_assets.py`)

The full `assets/` tree stays in the repo for development, but builds bundle only the reachable subset. The script derives that subset from the source code on every run, so it cannot drift:

- **Platform logos** — imported from `PLATFORM_LOGO_FILES` in `grid_launcher/server/platform_metadata.py`
- **Gamepad glyphs** — `input_*` PNG stems found in `grid_launcher/` and `grid-launcher.py`
- **SVG icons** — `svg/<name>` literals found in `grid_launcher/`, `grid-launcher.py`, and `tests/`
- **`tools/7z`** — copied for `--platform windows` only (7z.exe/7z.dll are unused on Linux)

The script recreates its output directory each run and exits non-zero if a derived file is missing from `assets/`. Builds then pass `--add-data "build/bundle-assets:assets"` (`;` on Windows), so runtime asset paths are unchanged. Run it manually with:

```bash
python scripts/stage_assets.py --platform linux --output build/bundle-assets
```

`assets/icons/grid-launcher.ico` (Windows `--icon`) and `assets/svg/io.github.Sixdd6.GRIDLauncher.svg` (AppImage icon rasterization) are read from the source tree and are intentionally not staged.

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

#### AppImage

To package GRID Launcher as a portable AppImage instead of a single binary, run:
```bash
./build.sh --appimage
```

**Prerequisites:** In addition to the same Python 3.12+/venv prerequisites as the regular Linux build, the AppImage build needs:
- `wget` — to download `appimagetool`
- `rsvg-convert` — to rasterize the app icon (apt: `librsvg2-bin`; dnf: `librsvg2-tools`)

**Runtime dependency (7z):** At runtime, extracting downloaded RetroArch/emulator archives uses the system `7z` binary when available (`7zip` on apt/dnf, or the legacy `p7zip-full`/`p7zip` packages on older distros), falling back to the bundled pure-Python `py7zr`. Installing `7zip` is recommended for faster, more reliable extraction. **Flatpak is not required** — GRID Launcher ships as an AppImage/native binary and auto-installs native/AppImage emulator builds only.

**Output:** On successful build, the AppImage will be located at:
```
./dist/grid-launcher-<version>-x86_64.AppImage
```

The `<version>` is detected automatically from `git describe --tags --always --dirty` (a leading `v` is stripped, e.g. `v0.7.0` → `0.7.0`). The build also generates `grid_launcher/version.py` so the running app reports the same version (shown in the window title). On a checkout with no tags the version falls back to `0.0.0-dev`.

The desktop entry embeds `X-AppImage-UpdateInformation` (GitHub releases zsync), so AppImages built by CI can self-update via tools like `AppImageUpdate`. CI also publishes the matching `.zsync` file next to the AppImage.

> **Note:** `appimagetool` is downloaded automatically to the project root on first use, so no manual installation is required.
>
> The standard `./build.sh` (no flags) still produces `dist/grid-launcher` (a single self-contained binary) — that path is unchanged.

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

# Stage the bundled asset subset
python scripts\stage_assets.py --platform windows --output build\bundle-assets

# Run build
python -m PyInstaller --noconfirm --clean --windowed --onefile --name grid-launcher --add-data "build\bundle-assets;assets" --add-data "retroarch-core-list.json;." --add-data "emulator-autoprofiles.json;." --add-data "romm-platform-cores.json;." grid-launcher.py
```

### Linux
```bash
# Activate virtual environment
source .venv/bin/activate

# Install PyInstaller
python -m pip install pyinstaller

# Stage the bundled asset subset
python scripts/stage_assets.py --platform linux --output build/bundle-assets

# Run build
python -m PyInstaller --noconfirm --clean --windowed --onefile --name grid-launcher --add-data "build/bundle-assets:assets" --add-data "retroarch-core-list.json:." --add-data "emulator-autoprofiles.json:." --add-data "romm-platform-cores.json:." grid-launcher.py
```
