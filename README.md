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
- Light and dark themes, following the OS by default. Gamepad navigation.

## Install

Downloads are on the [Releases page](https://github.com/Sixdd6/grid-launcher/releases).

- **Linux** — `grid-launcher-<version>-x86_64.AppImage`. Mark it executable and run it.
  The AppImage carries AppImage update information, so
  [AppImageUpdate](https://github.com/AppImage/AppImageUpdate) can update it in place
  from the latest release. The app itself never downloads or installs an update; it
  only shows a banner when a newer tag exists.
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
