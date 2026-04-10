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
        - [ ] Not sure how games are loaded here, need to investigate.

## Emulator Cloud Sync
- [x] Design and implement reusable logic to locate saved game files/folders that can be reused across emulators for cloud sync. Add a new field to emulator-autoprofiles.json to specify the save strategy. Add the new field to the Emulators page UI for editing.
    - [x] Implement emulator-specific save location strategies for those that require custom handling.
        - [x] PPSSPP
        - [x] PCSX2
        - [x] RPCS3
        - [x] RetroArch
        - [x] Duckstation
        - [x] ShadPS4
        - [x] Dolphin
        - [x] Cemu
        - [x] Xemu
        - [x] Pico8
        - [x] ReDream
        - [x] Xenia
        - [x] Eden
        - [x] Azahar
        - [x] FBneo
        - [x] MAME

## Native Cloud Sync
- [ ] Implement PCGamingWiki lookup for native Windows game information and grabbing default save locations.
- [ ] Design and Implement a method for detecting when the server has an update available for a native game and allow updating from the server without overwriting user configs or save files.

# Medium Priority
- [x] Implement RetroAchievements integration for browsing achievements.
    - [x] RA API client (`fetch_game_achievements`, `resolve_ra_game_id`)
    - [x] `RetroAchievementsWorker` background thread
    - [x] Achievements panel in game details (`build_achievements_panel`)
    - [x] Achievements button in game details (shown when `ra_id` available)
    - [x] RA username + API key in Settings

# Low Priority
- all done

# Future Ideas
- [ ] Fullscreen experience that works like emulation station or bigbox.

# Dream Features
- all quiet

# Project Cleanup Tasks
- [x] Implement separation of concerns for better code organization and optimization. Codebase should be modular and easy for the AI to understand and modify.