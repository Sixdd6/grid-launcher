# rom-mate-neo
A simple, responsive launcher for RomM.
Please be aware that this application is created using AI tools/coding, if this is a problem for you I welcome you to make your own.

[![Latest Build](https://github.com/Sixdd6/rom-mate-neo/actions/workflows/pyinstaller-windows.yml/badge.svg)](https://github.com/Sixdd6/rom-mate-neo/actions/workflows/pyinstaller-windows.yml)

## Desktop Mode Screenshots

![Library Tab](.github/images/desktop5.png)
![Server Tab](.github/images/desktop4.png)
![Game Details](.github/images/desktop1.png)
![Emulators Tab](.github/images/desktop2.png)
![Settings Tab](.github/images/desktop3.png)

## TV Mode Screenshots
![Home Tab](.github/images/tv1.png)
![Library Tab](.github/images/tv5.png)
![Server Tab](.github/images/tv2.png)
![Platform View](.github/images/tv3.png)
![Game Details](.github/images/tv4.png)

## Features
- Library tab with cover art grid for installed games
- Server tab with platform list and game grid for games on your server
- Settings tab with configuration options for server address and client token, retroachievements details, theme selection and cloud saves toggle
- Emulator auto-install from supported sources
- Light/Dark Themes
- Cloud save support and in-app management
- RetroAchievements integration and browsing
- Supports screen resolutions from 720p and up, good for small handhelds AND large screens

## Supported Platforms

| Emulator | Platforms |
|----------|-----------|
| RetroArch | Multi-system (PlayStation 1, Nintendo, Sega, and many more via cores) |
| DuckStation | PlayStation 1 |
| PCSX2 | PlayStation 2 |
| PPSSPP | PlayStation Portable (PSP) |
| RPCS3 | PlayStation 3 |
| ShadPS4 | PlayStation 4 |
| Dolphin | GameCube, Wii, Tri-Force (untested) |
| Cemu | Wii U — see note below |
| Azahar | Nintendo 3DS |
| Eden | Nintendo Switch |
| Pico-8 | Pico-8 |
| Xemu | Xbox (Original) |
| Xenia / Xenia Canary | Xbox 360 |
| MAME | Arcade |
| FBNeo | Arcade |
| Redream | Sega Dreamcast, Sega Naomi |

- Emulators can be manually added with launch arguments editable by the user. This way it is possible to use almost any emulator you could launch from a standard shortcut or batch file.

> **Cemu note:** Cemu does not reliably respect window maximization on launch, which interferes with the RomMate pause menu. The recommended workaround is [Borderless Gaming](https://github.com/Codeusa/Borderless-Gaming) — add Cemu as a favorite and it will automatically maximize the window on launch. A thin menubar will remain visible at the top of the Cemu window, but the pause menu will otherwise function correctly.

## PC Games

Native Windows PC games are also supported by archiving the installed files. When installing the files are extracted to a subfolder, based on the archive name. This is intended for legally obtained DRM-free games only.

<!> You are responsible for what you do with this, don't come complain to me when a hacked copy of a game installs a rootkit on your pc. <!>

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
- Native Windows games use PCGamingWiki to attempt to automatically discover save locations. A Browse button allows manual path additions when automatic lookup fails. Multiple directories/files can be selected from the game details page in Desktop Mode. All configured directories/files are bundled into a single versioned archive per session.

## Save Archive Format

rom-mate uploads saves to RomM as zip archives. The archive layout varies by save type:

### Emulator Saves (single file or folder)

Standard saves are uploaded as a flat zip of the relevant save files, preserving relative paths from within the emulator's save directory. The `emulator` field on the server record identifies which emulator produced the save, allowing correct routing on restore.

### Native Windows Saves (`emulator: native_multi_dir`)

Native game saves bundle all configured save directories into one archive per session:

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
- **SVG Repo** — Icons by [SVG Repo](https://www.svgrepo.com/)

## Contribution

I welcome others to contribute to this project, even Ai-assisted contribution, however keep in mind that I expect any contribution to be fully tested by yourself before you make a pull request. Any code not fully tested or tested poorly will not be accepted, you are responsible for this crucial step not your toolset or AI agents.