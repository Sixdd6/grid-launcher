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
    - [ ] RPCS3
        - Need to figure out how to handle the different types of games (ISO, PKG+RAP).
    - [ ] ShadPS4
        - Not sure games are loaded here, need to investigate.
- [ ] Implement updating installed games from the server.

# Medium Priority
- [x] Switch emulator default exe should be eden.exe.
- [ ] Slight scaling issue with game details, if window is maximized before viewing game details the layout is off.

# Low Priority
- [x] Also hide the 'Windows 9x' platform in the default emulator selection list.

# Future Ideas
- [ ] Offload emulator configurations to a separate file that can be updated independently.
- [ ] Implement the Dracula theme as the main dark theme.
- [ ] Implement cloud sync integration for game saves.
- [ ] Implement PCGamingWiki integration for Windows game information and default save locations.
- [ ] Implement RetroAchievements integration for browsing achievements.

# Dream Features
- [ ] Fullscreen experience that works like emulation station or bigbox.

# Project Cleanup Tasks
- [ ] Implement separation of concerns for better code organization.