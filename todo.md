# Top Priority
- [ ] Setup automatic tweaking of emulator settings on install or launch. Suggestions of commonly adjusted settings are welcome from the planning agent.
    - [ ] RetroArch
        - [x] save/savestate directory config
        - [x] enable borderless fullscreen
        - [x] set netplay username to RomM login username
        - [x] volume gain to -18
        - [x] disable discord rich presence
        - [x] RetroAchievements credentials if available
        - [x] disable hardcore mode and leaderboard notifications by default
    - [x] Dolphin standalone
        - [x] disable Confirm on Stop (ShowLaunchWarning=False)
        - [x] fullscreen, analytics off, launch warning off, volume 70
        - [x] SkipIPL = False (boot animation)
        - [x] XInput GCPad controller defaults
    - [x] DuckStation
        - [x] memory card config, fullscreen, RetroAchievements enabled (login via DuckStation UI — token is machine-encrypted, cannot be pre-filled)
        - [x] disable auto update, 4x resolution, volume 60%
        - [x] pause menu hotkey on controller guide button
        - [x] setup wizard auto-completed
        - [x] SDL controller mapping (Pad1 AnalogController defaults)
        - [x] cloud saves/states working (fixed underscore state naming detection)
        - [x] disable RA hardcore mode and leaderboard notifications by default
    - [x] PCSX2
        - [x] antiblur
        - [ ] controller auto-config
        - [ ] controller guide button to open pause menu
        - [ ] RetroAchievements credentials if available
        - [ ] audio volume to 40%
        - [ ] resolution to 1080p or equivalent
    - [x] Azahar (3DS) — volume 40%, fullscreen, F1 toggles fullscreen, Esc closes emulator; controller setup note shown in UI (user must use in-app Auto Map)
        - [x] discord presence off
        - [x] confirm before closing off
        - [x] volume to 40%
        - [x] launch fullscreen
        - [x] controller setup note visible in emulator list
    - [ ] Cemu (Wii U) — needs keys.txt present in emulator dir for game decryption; ensure_cemu_settings() should handle sourcing/placing it
        - [x] update check off, discord presence off
    - [ ] Eden (Switch) — needs remaining config + firmware/keys setup (prod.keys, title.keys, firmware)
        - [x] discord presence off, confirm before closing off, telemetry off
    - [ ] Xemu (Xbox) — needs default config (ensure_xemu_settings() expansion) and BIOS file setup
        - [x] update check off, vsync on
    - [ ] RPCS3 (PS3) — first-run wizard is a full blocker; needs ensure_rpcs3_settings() to suppress wizard and write default config
    - [x] PPSSPP (PSP) — play and save states confirmed working; RA not yet configured (see RA section)
    - [x] ShadPS4 (PS4) — launches and loads games fine
    - [ ] MAME (Arcade) — not yet tested; needs autoinstall source and default config
    - [ ] FBNeo (Arcade) — not yet tested; needs autoinstall source and default config
    - [ ] Xenia / Xenia Canary (Xbox 360) — ISO extension stripped on install (game won't load); also installs titles/updates/DLC to a directory within the emulator folder (content\) — needs install path handling and game launch path resolution
    - [x] Redream (Dreamcast) — fullscreen, volume 40, cloud saves/states working


- [x] Setup automatic download and install of BIOS files following emulator install or launch for emulators that require them
    - [x] RetroArch core firmware metadata in retroarch-core-list.json (58 cores)
    - [x] Firmware auto-install on game install (InstallFinalizeWorker) and game launch (_perform_game_action)
    - [x] Region-aware firmware directory routing (keyword matching)
    - [x] .7z/.rar firmware extraction via staging directory (preserves existing files)
    - [x] Dolphin: auto-configure SkipIPL=False for boot animation
    - [x] Dolphin: auto-configure XInput GCPad controller mapping
    - Platform firmware testing status:
        - [x] Dreamcast — confirmed working (Flycast/RetroArch)
        - [x] GameCube — confirmed working (Dolphin standalone)
        - [x] PlayStation 1 — confirmed working (DuckStation standalone)
        - [ ] PlayStation 2 — server has 5 BIOS files, PCSX2 has firmware_directories
        - [ ] Game Boy — server has dmg_boot.bin, RetroArch system/
        - [ ] Game Boy Advance — server has gba_bios.bin, RetroArch system/
        - [ ] Game Boy Color — server has cgb_boot.bin, RetroArch system/
        - [ ] TurboGrafx CD — server has 3 syscard files, RetroArch system/
        - [ ] Sega Saturn — server has saturn_bios.bin, RetroArch system/
        - [ ] Sega CD — server has 3 regional BIOS files, RetroArch system/
        - [ ] Sega 32X — server has 3 BIOS files, RetroArch system/
        - [ ] PlayStation 3 — server has PS3UPDAT.PUP, RPCS3 needs firmware_directories added
        - [ ] Xbox — server has 3 files, Xemu needs firmware_directories added
        - [ ] Nintendo Switch — server has keys + firmware, Eden needs firmware_directories added

## RetroAchievements for emulators
- [ ] Research and implement RetroAchievements for emulators which support it. Prefill configs with the credentials the user has already input in Settings (may need to add a field for password in Settings). Keep in mind that the user will need username and password for the emulators while the launcher uses the api key to do lookups.
    - [ ] PPSSPP: write RA credentials to memstick/PSP/SYSTEM/PPSSPP.INI [Achievements] section (Username, Token keys) during ensure_ppsspp_settings()

# Medium Priority
- [ ] Tweak design of emulator auto-install window, it's pretty basic right now and could be more robust.
- [ ] RomM server already has a maximum saves/states implementation, remove the save retention slot limit code and implement a system to utilize the openapi functions instead.

# Low Priority
- [ ] Collect remaining RetroArch core BIOS documentation for: Neo Geo CD (neocd), SwanStation/DuckStation (libretro), LRPS2 (PS2), Citra (3DS), FBNeo (Arcade), SAME_CDI, GAM4980 (BBK). Add firmware entries to retroarch-core-list.json once docs are found.


# Future-Plans
nothing to see here

# Dream Features
- all quiet


# Project Cleanup Tasks


# Tasks For When I've Run Out Of Ideas
- [ ] Cross-platform support (Windows, Linux, Mac)