# Top Priority
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
- [ ] Design and implement reusable logic to locate saved game files/folders that can be reused across emulators for cloud sync. Add a new field to emulator-autoprofiles.json to specify the save strategy. Add the new field to the Emulators page UI for editing.
    - [ ] Implement emulator-specific save location strategies for those that require custom handling.
        - [x] PPSSPP
        - [x] PCSX2
        - [x] RPCS3
        - [x] RetroArch
        - [x] Duckstation
        - [ ] ShadPS4
        - [x] Dolphin
        - [ ] Cemu
        - [ ] Xemu
        - [ ] Pico8
        - [ ] ReDream
        - [ ] Xenia
        - [ ] Eden
        - [ ] Redream
        - [ ] Azahar
- [ ] Design and Implement a method for detecting when server files have updated and allow updating games and emulators from the server without overwriting user configs or save files. This would mostly be used for updating emulators and native games.

# Medium Priority
- all done

# Low Priority
- all done

# Future Ideas
- [ ] Implement PCGamingWiki integration for Windows game information and grabbing default save locations.
- [ ] Implement RetroAchievements integration for browsing achievements.
- [ ] Implement cloud save manager, as a sub page of Game Details, to view and manage cloud saves and states.

# Dream Features
- [ ] Fullscreen experience that works like emulation station or bigbox.

# Project Cleanup Tasks
- [ ] Implement separation of concerns for better code organization and optimization. Codebase should be modular and easy for the AI to understand and modify.