# rom-mate-neo
A simple, nice-looking launcher for RomM.

[![Latest Build](https://github.com/Sixdd6/rom-mate-neo/actions/workflows/pyinstaller-windows.yml/badge.svg)](https://github.com/Sixdd6/rom-mate-neo/actions/workflows/pyinstaller-windows.yml)

## Feature Checklist
- [x] Library view with cover art grid for installed games
- [x] Server view with platform list and game grid for games on your server
- [x] Emulator auto-install from RomM server with autopopulated launch arguments
- [x] Settings view with necessary configuration options
- [x] Connection to RomM server via API
- [x] Game launching functionality
- Game management
    - [x] install
    - [ ] update
    - [x] delete
- [x] Light/Dark Themes
- [x] Responsive layout
- [x] Cloud save support and browsing
- [ ] RetroAchievements integration and browsing
- [ ] Cross-platform support (Windows, Linux, Mac)

## Architecture
- See `ARCHITECTURE.md` for the current module map and change guide.

## Cloud Sync Notes
- Game Details now exposes `Manage Saves`, `Manage States`, or `Emulator Saves` depending on the active emulator and platform capabilities.
- Shared-save emulators such as Xemu and Redream surface emulator-wide backups with restore/delete warnings because those actions can affect all games using the same shared media.
- Redream sync now covers global VMU cards plus per-game hash-based `*.0.sav` savestates, and the details panel swaps immediately before loading cloud records asynchronously.