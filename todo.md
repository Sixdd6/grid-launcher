# Top Priority
- [x] Fix cover art not loading in server view.
- [x] Launching a Windows game currently tries to use an emulator.
- [x] Setup default launch arguments for emulators.
    - [x] Azahar
    - [x] Eden
    - [x] RetroArch
    - [x] Duckstation
    - [x] PCSX2
    - [x] PPSSPP
    - [x] Cemu
    - [x] Citra
    - [x] Xemu
    - [x] Dolphin
    - [x] Pico8
    - [x] ReDream
    - [x] Xenia
    - [x] RPCS3
    - [ ] ShadPS4
        - Not sure how games are loaded here, need to investigate.
- [ ] Implement updating installed games from the server.
- [ ] Implement cloud sync integration for game saves and states.
    - [x] PPSSPP
    - [ ] PCSX2
    - [x] RPCS3
    - [x] RetroArch
    - [ ] Duckstation
    - [ ] ShadPS4
    - [ ] Dolphin
    - [ ] Cemu
    - [ ] Citra
    - [ ] Xemu
    - [ ] Pico8
    - [ ] ReDream
    - [ ] Xenia
    - [ ] Eden
    - [ ] Ryujinx
    - [ ] Yuzu
    - [ ] Sudachi
    - [ ] Redream
    - [ ] Azahar

# Medium Priority
- [x] Switch emulator default exe should be eden.exe.
- [x] Slight scaling issue with game details, if window is maximized before viewing game details the layout is off.
- [x] Tweak the download progress bar show the installing progressbar animation when there are queued downloads too.
- [x] Show an install percentage next to the download progress bar when installing games, based on the total size of the files being installed.
- [x] Cache game cover art when installed so it can be viewed while offline and cleanup the cached images when the game is uninstalled.
- [ ] Implement a first-run wizard to setup the required paths and settings.

# Low Priority
- all done

# Future Ideas
- [x] Offload emulator configurations to a separate file that can be updated independently.
- [x] Implement the Dracula theme as the main dark theme.
- [ ] Implement PCGamingWiki integration for Windows game information and default save locations.
- [ ] Implement RetroAchievements integration for browsing achievements.

# Dream Features
- [ ] Fullscreen experience that works like emulation station or bigbox.

# Project Cleanup Tasks
- [ ] Implement separation of concerns for better code organization.