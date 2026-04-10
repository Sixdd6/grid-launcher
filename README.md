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
- [ ] RetroAchievements integration and browsing
- [ ] Cross-platform support (Windows, Linux, Mac)

## Cloud Sync Notes
- Game Details now displays `Manage Saves` or `Emulator Saves`, `Manage States` depending on the active emulator capabilities.
- Shared-save emulators such as Xemu and Redream surface emulator-wide backups, be warned that actions can affect all games using the same shared media.
