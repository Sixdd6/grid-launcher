# GRID Launcher
Game Repository Interface & Downloader — A launcher for RomM.

Please be aware that this application is created using AI tools/coding, if this is a problem for you I invite you to make your own.

![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/sixdd6/grid-launcher/total)
![GitHub Release](https://img.shields.io/github/v/release/sixdd6/grid-launcher)
[![Build](https://github.com/Sixdd6/grid-launcher/actions/workflows/build.yml/badge.svg)](https://github.com/Sixdd6/grid-launcher/actions/workflows/build.yml) [![Static Badge](https://img.shields.io/badge/wiki-documentation)](https://github.com/Sixdd6/grid-launcher/wiki)

## What it is

A desktop client for a [RomM](https://github.com/rommapp/romm) server. It connects to
your server, browses its platforms and games, installs them locally, launches them
through an emulator (or directly, for Windows games), and syncs saves and save states
back to the server.

Five views: Library (installed games), Server (everything on your server), Downloads,
Emulators, Settings.

- Install and uninstall games, with archive extraction and a download queue.
- Update detection: an installed game whose server copy is newer is badged, and can be
  updated in place. Native (Windows) updates merge over the install and keep saves.
- Cloud saves and save states: manual upload/restore from a game's details, plus
  automatic restore before launch and upload after exit.
- Emulator management: add emulators by hand, or install them from their official
  sources through the built-in catalog. Installed emulators are auto-configured so
  saves, firmware and RetroAchievements land where the launcher expects them.
- Firmware: BIOS and firmware files held by your server are routed into the right
  emulator directory. RPCS3's PS3 firmware is fetched and installed on request.
- Covers, screenshots, descriptions and trailers, cached on disk so the library still
  renders when the server is unreachable.
- RetroAchievements login, so a supported emulator starts already signed in.
- Light and dark themes, following the OS by default.
- Game backups to a USB drive or SSD, installable on any computer with GRID, with or
  without a network.
- TV mode: a second, fullscreen interface for the couch, driven entirely with a game
  controller (see below).

## Install

Downloads are on the [Releases page](https://github.com/Sixdd6/grid-launcher/releases).

- **Linux**
  - Debian, Ubuntu and distributions based on them —
    `grid-launcher_<version>_amd64.deb`, installed with
    `sudo apt install ./grid-launcher_<version>_amd64.deb`.
  - Fedora, openSUSE and distributions based on them —
    `grid-launcher-<version>-1.x86_64.rpm`, installed with
    `sudo dnf install ./grid-launcher-<version>-1.x86_64.rpm` (or `zypper install` on
    openSUSE).
  - Other distributions — `grid-launcher-<version>-x86_64.AppImage`, provided for
    distributions the packages above do not cover. Mark it executable and run it. It
    carries AppImage update information, so
    [AppImageUpdate](https://github.com/AppImage/AppImageUpdate) can update it in place
    from the latest release.

  The app itself never downloads or installs an update; it only shows a banner when a
  newer tag exists. If the window comes up blank, start GRID once with
  `GRID_LAUNCHER_HW_ACCEL=never` and turn off Settings › Appearance › Hardware
  acceleration.
- **Windows** — `grid-launcher-<version>-windows-x86_64-setup.exe`, an NSIS installer
  that installs for the current user. WebView2 is downloaded by the installer if it is
  missing.

## First run

Enter your RomM server address and credentials. The token is stored in the OS keyring
(Secret Service on Linux, Credential Manager on Windows) and never written to a config
file or a log.

If a previous Python version of GRID Launcher left a `~/.grid-launcher/config.json`
behind, its settings, emulator entries and installed-game records are imported once, on
the first launch that finds no configuration of its own. A toast reports what was
imported. Tokens are **not** imported — re-enter your RomM token, and your
RetroAchievements token if you used one.

Application state lives in the platform config directory (`config.toml`, the
`grid-launcher.db` registry of installed games, and a `covers/` cache).

## TV mode

TV mode is GRID's fullscreen interface for a TV, driven with a game controller. It
reaches every feature of the desktop app: Server › Discover and your platforms, the
Library and its Backup drives, Downloads, Settings, game details, saves, and the startup
screens.

- **Entering and leaving.** The TV button in the desktop top bar opens it, or start
  GRID with `grid-launcher --tv`. Menu (Start) › Switch to desktop mode, also in
  Settings › TV mode, leaves it. The choice isn't saved: GRID starts in desktop mode
  unless it is started with `--tv`.
- **Controllers.** Xbox, PlayStation, Nintendo and 8BitDo pads, several at once. Each
  controller keeps its own button assignment (Xbox style or Nintendo style), and the
  on-screen prompts follow the controller pressed last (Settings › TV mode › Button
  prompts picks a set by hand). GRID ignores the controller while a game runs, so the
  emulator gets it. A keyboard and mouse work too.
- **Linux permissions.** GRID reads controllers from `/dev/input`; desktop distributions
  give the logged-in user access already. If a controller isn't seen, see "TV mode" in
  [BUILD.md](BUILD.md).

## Emulator setup

- **Windows** — emulators can be added manually or installed from the catalog, which
  downloads them from their official sources and configures them.
- **Linux** — catalog installs cover emulators distributed as AppImages or native
  binaries. Flatpak emulators are not supported: the launcher will not install or
  detect them. Point it at the flatpak wrapper and set the paths yourself if you use
  one.

### xemu cloud saves

Cloud save sync for xemu reads the Xbox HDD image directly, which requires a raw image
(`xbox_hdd.img`) rather than the qcow2 format xemu ships by default. Games launch fine
either way — only cloud sync needs raw. Convert your image once with:

```
qemu-img convert -O raw xbox_hdd.qcow2 xbox_hdd.img
```

Place the `.img` alongside your other xemu BIOS files (it is preferred over a `.qcow2`
when both exist). Run the conversion on an ext4/NTFS/APFS drive — the raw file stays
sparse there, but occupies its full ~8 GB on exFAT/FAT32.

## Build from source

See [BUILD.md](BUILD.md) for prerequisites, the development loop, the test gate, the
end-to-end suite, and how a release is produced.

## Third-Party Software

This repository includes the following third-party software or assets:

- **7-Zip** — `assets/tools/7z/7z.exe`, the Windows extraction fallback the app looks for beside its own binary. Copyright © 1999-2026 Igor Pavlov. Licensed under GNU LGPL. The unRAR code is licensed under a mixed license (GNU LGPL + unRAR restriction). See [assets/tools/7z/License.txt](assets/tools/7z/License.txt) for full license details. Source code: https://github.com/ip7z/7zip
- **RetroArch assets** — PNG image files in [assets/retroarch-assets](assets/retroarch-assets) sourced from the libretro/retroarch-assets repository. Licensed under Creative Commons Attribution 4.0 International (CC BY 4.0). Source: https://github.com/libretro/retroarch-assets
- **SVG Repo** — Icons by [SVG Repo](https://www.svgrepo.com/)
- **Input Prompts** — the controller button glyphs in [app/src/lib/tv/glyphs](app/src/lib/tv/glyphs), from Input Prompts 1.5A by [Kenney](https://kenney.nl/assets/input-prompts). Licensed under Creative Commons Zero (CC0 1.0). See [app/src/lib/tv/glyphs/License.txt](app/src/lib/tv/glyphs/License.txt).
