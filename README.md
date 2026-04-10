# rom-mate-neo
A simple, responsive launcher for RomM.

[![Latest Build](https://github.com/Sixdd6/rom-mate-neo/actions/workflows/pyinstaller-windows.yml/badge.svg)](https://github.com/Sixdd6/rom-mate-neo/actions/workflows/pyinstaller-windows.yml)

## Feature Checklist
- [x] Library view with cover art grid for installed games
- [x] Server view with platform list and game grid for games on your server
- [x] Emulator auto-install from RomM server with autopopulated launch arguments
    - [x] Emulator auto-download from supported source metadata (MVP)
- [x] Settings view with necessary configuration options
- [x] Game launching functionality
- Game management
    - [x] install
    - [x] update
    - [x] delete
- [x] Light/Dark Themes
- [x] Responsive layout
- [x] Cloud save support and in-app management
- [x] RetroAchievements integration and browsing

## Cloud Sync Notes
- Game Details now displays `Manage Saves` or `Emulator Saves`, `Manage States` depending on the active emulator capabilities.
- Shared-save emulators such as Xemu and Redream surface emulator-wide backups, be warned that actions can affect all games using the same shared media.
- Native Windows games use PCGamingWiki to automatically discover save locations. All configured directories are bundled into a single versioned archive per session, preserving the standard 3-save retention. A Browse button allows manual path additions when automatic lookup finds nothing.

## Save Archive Format

rom-mate uploads saves to RomM as zip archives. The archive format varies by save type:

### Emulator Saves (single file or folder)

Standard saves are uploaded as a flat zip of the relevant save files, preserving relative paths from within the emulator's save directory. The `emulator` field on the server record identifies which emulator produced the save, allowing correct routing on restore.

### Native Windows Saves (`emulator: native_multi_dir`)

Native game saves bundle all configured save directories into one archive per upload session:

```
_rom_mate_dirs.json       ← directory manifest
0/saves/game.sav          ← files from directory 0, relative to that directory root
0/profile.dat
1/settings.ini            ← files from directory 1, relative to that directory root
1/keybindings.cfg
```

**`_rom_mate_dirs.json`** maps integer index strings to the raw (unexpanded) Windows path for each directory:

```json
{
    "0": "%APPDATA%\\Stardew Valley",
    "1": "%LOCALAPPDATA%\\StardewValley"
}
```

Paths use `%ENVVAR%` tokens rather than absolute paths so archives are portable across user accounts and machines.

On restore, each `N/` prefix is decoded via the manifest, env vars are expanded, and files are written back to the correct directory. The `emulator` field on the server record is set to `native_multi_dir` to identify this format. Legacy per-directory records (emulator field `native_dir:<path>`) from an earlier format are still supported.
