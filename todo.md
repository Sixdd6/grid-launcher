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
    - [x] ShadPS4

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
- [x] Implement PCGamingWiki lookup for native Windows game information and grabbing default save locations.
    - [x] `rom_mate/server/pcgamingwiki.py` — API client (title search, wikitext fetch, path parser)
    - [x] `PCGamingWikiWorker` in `rom_mate/background/workers.py`
    - [x] In-memory save-path cache on main window
    - [x] `_details_cloud_mode_supported` — allow native save mode for installed games
    - [x] `update_details_action_buttons` — show Manage Saves for installed native games
    - [x] `_refresh_native_save_panel` — native-game panel with PCGamingWiki paths + Browse fallback
    - [x] `_upload_native_saves_for_game` — single combined archive upload with directory manifest
    - [x] `_restore_native_cloud_save_for_game` — manifest-driven restore to correct source directories
    - [x] `tests/test_pcgamingwiki.py` — unit tests with mocked HTTP

## Game Updating
- [x] Implement native Windows game update detection and safe install
    - [x] `build_installed_game_record` — persist `native_executable_path` to survive re-registration
    - [x] `merge_archive_into_directory` in `archive_preparation.py` — extract to same-filesystem temp dir, merge into existing dir, delete temp
    - [x] `prepare_native_game_update_without_ui` in `archive_preparation.py` — merge + re-detect exe + update server metadata
    - [x] `InstallFinalizeWorker` — `"native_update"` content kind branch
    - [x] `_apply_native_game_update_without_ui` + `_native_update_temp_dir_for_game` in `rom-mate.py`
    - [x] `_perform_game_update_action` — native branch with install-dir validation and confirmation dialog
    - [x] `_start_async_install_finalize` — route `native_update` install mode
    - [x] `_on_async_install_finalize_finished` — native update success path with toast
    - [x] `tests/test_native_game_update.py` — merge, prepare, and registry tests

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
- [ ] Fullscreen experience that works like "emulation station", "ES-DE" or "bigbox".


# Dream Features
- all quiet


# Project Cleanup Tasks


# Tasks For When I've Run Out Of Ideas
- [ ] Cross-platform support (Windows, Linux, Mac)