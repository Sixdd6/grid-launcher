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
- [x] Supports screen resolutions from 720p and up, good for small handhelds AND large screens

## Supported Platforms

| Emulator | Platforms |
|----------|-----------|
| RetroArch | Multi-system (PlayStation 1, Nintendo, Sega, and many more via cores) |
| DuckStation | PlayStation 1 |
| PCSX2 | PlayStation 2 |
| PPSSPP | PlayStation Portable (PSP) |
| RPCS3 | PlayStation 3 |
| ShadPS4 | PlayStation 4 |
| Dolphin | GameCube, Wii |
| Cemu | Wii U |
| Azahar | Nintendo 3DS |
| Eden | Nintendo Switch |
| Pico-8 | Pico-8 |
| Xemu | Xbox (Original) |
| Xenia / Xenia Canary | Xbox 360 |
| MAME | Arcade |
| FBNeo | Arcade |
| Redream | Sega Dreamcast, Sega Naomi |

Native Windows PC games are also supported by archiving the installed files and are extracted to a subfolder upon installation. This is intended for legally obtained DRM-free games only. You are responsible for what you do with this, don't come complain to me when a hacked copy of a game installs a rootkit on your pc.

## PS3 Game Archiving

RPCS3 support requires PS3 content to be installed to specific paths depending on whether the game is a disc dump or a digital copy. rom-mate-neo auto-detects the archive layout and routes files accordingly - but the archive itself must be structured correctly.

### Disc Dump

Wrap the dump in a game-ID folder containing `PS3_DISC.SFB` and the 'PS3_GAME' directory. This is the preferred structure for disc-based games and routes the install to RPCS3's `/games/` directory:

```
BLUS30336/
├── PS3_GAME/
│   ├── PARAM.SFO
│   ├── USRDIR/
│   │   └── EBOOT.BIN
│   └── ...
└── PS3_DISC.SFB
```

### Digital / PSN Title

Wrap the content in a game-ID folder containing the 'PS3_GAME' directory. Routes to `dev_hdd0/game/`:

```
BLUS30336/
└── PS3_GAME/
    ├── PARAM.SFO
    ├── USRDIR/
    │   └── EBOOT.BIN
    └── ...
```

### Bare Disc Dump (No Wrapper)

If the archive contains `PS3_GAME/` at the root with no game-ID folder, rom-mate-neo will attempt to read the title ID from `PARAM.SFO` and synthesize the correct install path automatically:

```
PS3_GAME/
├── PARAM.SFO
├── USRDIR/
│   └── EBOOT.BIN
└── ...
```

### Nested dev_hdd0 Layout

Full `dev_hdd0` trees are also supported and are merged directly into the active RPCS3 data root. Trophy directories (`NPWR#####`) are automatically linked to the correct trophy path:

```
dev_hdd0/
├── game/
│   └── BLUS30336/
│       └── PS3_GAME/
└── home/
    └── 00000001/
        └── trophy/
            └── NPWR00001/
```

### ISO / Disc Image

`.iso` files are extracted automatically via 7-Zip before classification. The extracted layout is then classified using the rules above.

### Notes

- Game ID directories must match the pattern `XXXX#####` (four uppercase letters followed by five digits), e.g. `BLUS30336`.
- The presence of `PS3_DISC.SFB` determines whether the install targets the `/games/` VFS path (disc) or `dev_hdd0/game/` (digital).

## Cloud Sync Notes
- Game Details now displays `Manage Saves` or `Emulator Saves`, `Manage States` depending on the active emulator capabilities.
- Shared-save emulators such as Xemu and Redream surface emulator-wide backups, be warned that actions can affect all games using the same shared media.
- Native Windows games use PCGamingWiki to automatically discover save locations. All configured directories are bundled into a single versioned archive per session, preserving the standard 3-save retention. A Browse button allows manual path additions when automatic lookup fails.

## Save Archive Format

rom-mate uploads saves to RomM as zip archives. The archive layout varies by save type:

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

## Third-Party Software

This project bundles the following third-party software:

- **7-Zip** — Copyright © 1999-2026 Igor Pavlov. Licensed under GNU LGPL. The unRAR code is licensed under a mixed license (GNU LGPL + unRAR restriction). See [assets/tools/7z/License.txt](assets/tools/7z/License.txt) for full license details. Source code: https://github.com/ip7z/7zip
- **RetroArch assets** — PNG image files in [assets/retroarch-assets](assets/retroarch-assets) sourced from the libretro/retroarch-assets repository. Licensed under Creative Commons Attribution 4.0 International (CC BY 4.0). Source: https://github.com/libretro/retroarch-assets
