# Top Priority
- [ ] Setup automatic tweaking of emulator settings on install or launch. Suggestions of commonly adjusted settings are welcome from the planning agent.
    - [x] RetroArch
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
        - [x] controller auto-config
        - [x] controller guide button to open pause menu
        - [x] RetroAchievements credentials if available
        - [x] audio volume to 40%
        - [x] resolution to 1080p or equivalent
    - [x] Azahar (3DS) — volume 40%, fullscreen, F1 toggles fullscreen, Esc closes emulator; controller setup note shown in UI (user must use in-app Auto Map)
        - [x] discord presence off
        - [x] confirm before closing off
        - [x] volume to 40%
        - [x] launch fullscreen
        - [x] controller setup note visible in emulator list
    - [x] Cemu (Wii U) — keys.txt firmware install, portable mode, controller defaults (XInput Pro Controller), full settings template
        - [x] update check off, discord presence off
        - [x] keys.txt placed in portable/ via firmware pipeline
        - [x] portable mode enforced (portable/ directory created)
        - [x] XInput Pro Controller profile written to portable/controllerProfiles/controller0.xml
        - [x] full working settings.xml template on fresh install (Cubeb audio, correct defaults)
    - [x] Eden (Switch) — firmware/keys auto-install, fullscreen, audio, QSettings annotations, controller note
        - [x] discord presence off, confirm before closing off (confirmStop=2), telemetry off
        - [x] fullscreen on launch, firstStart=false, pauseWhenInBackground, enable_gamemode, colorful_dark theme
        - [x] check_for_updates off
        - [x] audio volume 40%, mute when in background
        - [x] Anime4K scaling filter (scaling_filter=6)
        - [x] prod.keys and title.keys auto-installed to user/keys/ via switch-keys.zip
        - [x] Switch firmware auto-installed to user/nand/system/Contents/registered/ via switch-firmware.zip
        - [x] QSettings key\default=false annotations written so Eden preserves all enforced settings
        - [x] controller setup note visible in emulator list (GUID-bound — user must use in-app Controls → Configure)
    - [x] Xemu (Xbox)
        - [x] update check off, vsync on
        - [x] portable mode (xemu.toml in exe dir)
        - [x] BIOS from server (firmware_directories: ["."])
        - [x] 1080p resolution ([display.window] startup_size + [display.quality] surface_scale)
        - [x] volume 40% ([audio] volume_limit = 0.4)
        - [x] controller note (SDL auto-detects layout; GUID-required for custom bindings)
        - [x] RetroAchievements: not supported
    - [x] RPCS3 (PS3) — portable mode, wizard suppression, firmware install button, PS3 game install pipeline fixed
        - [x] first-run wizard suppressed (GuiSettings.ini infoBoxEnabledWelcome=false)
        - [x] portable mode enforced (portable/ directory created next to rpcs3.exe)
        - [x] fullscreen on launch, Master Volume 40%
        - [x] Discord presence off, auto-update off, exit confirmation off (GuiSettings.ini)
        - [x] PS3UPDAT.PUP firmware routing via firmware_directories: ["."]
        - [x] "Install PS3 Firmware" button in emulator row (triggers rpcs3.exe --installfw)
        - [x] PS3 7z game install fixed — extracted_path set to extracted_dir (RPCS3 uses game ID not %rom%)
        - [x] fs_name + fs_extension combined for correct archive filename (RomM splits these fields)
        - [x] folder-backed ROM downloads fixed — rom_nested_file_name from files[0].file_name
        - [x] multi-file folder ROMs use file_ids=<base_id> to download only the base game archive
        - [x] PS3 junction depth fix — junctions created at game-ID level only (NPUB12345/), not at dev_hdd0/ or games/ root
        - [x] PS3UPDAT.PUP auto-downloaded in background on RPCS3 install; "Install PS3 Firmware" button appears without needing to launch a game first
        - [x] games.yml written to portable/config/games.yml (RPCS3 portable mode data root fix)
        - [x] Game ID detection prefers directories containing PS3_GAME/ subdirectory; NPWR trophy dirs never used as game ID
        - [x] NPWR trophy directories junctioned to portable/dev_hdd0/home/00000001/trophy/<NPWRID>/ automatically
        - [ ] Manual test: install a PS3 game end-to-end and verify junctions, games.yml entry, trophy junction, and launch via RPCS3
    - [x] PPSSPP (PSP) — play and save states confirmed working
        - [x] RetroAchievements credentials written to memstick/PSP/SYSTEM/PPSSPP.INI [Achievements] section (AchievementsEnable, AchievementsUserName, AchievementsToken, AchievementsChallengeMode=False)
        - [ ] Manual test: launch a PSP game with RA enabled and verify achievements unlock
    - [x] ShadPS4 (PS4) — launches and loads games fine
    - [ ] MAME (Arcade) — not yet tested; needs autoinstall source and default config
    - [ ] FBNeo (Arcade) — not yet tested; needs autoinstall source and default config
    - [x] Xenia / Xenia Canary (Xbox 360)
        - [x] .xex added to preferred launch file extensions (GOD format)
        - [x] STFS content install (apply_xenia_content_without_ui reads TitleID/ContentType from binary header, places to content\0000000000000000\<TitleID>\<ContentType>\)
        - [x] Update/DLC auto-queued after base game install
        - [x] Manual "Install Update/DLC" button in details panel
        - [x] Xbox 360 platform detection (is_xbox360_platform)
    - [x] Redream (Dreamcast) — fullscreen, volume 40, cloud saves/states working


- [x] Setup automatic download and install of BIOS files following emulator install or game launch for emulators that require them
    - [x] RetroArch core firmware metadata in retroarch-core-list.json (90+ cores with firmware, 80+ cores with .opt config_files)
    - [x] Firmware auto-install on game install (InstallFinalizeWorker) and game launch (_perform_game_action)
    - [x] Region-aware firmware directory routing (keyword matching)
    - [x] .7z/.rar firmware extraction via staging directory (preserves existing files)
    - [x] RetroArch .opt config file distribution via config_files metadata (all cores with core options API)
    - [x] Firmware file keyword filter — only explicitly listed filenames accepted per core (prevents cross-platform file misrouting)
    - [x] MAME-format zip preservation — explicitly named .zip firmware saved as archive (e.g. naomi.zip, dc_boot.zip)
    - [x] Path-preserving zip extraction (extract_with_paths flag) for zips with internal directory structure
    - [x] RetroArch :\-prefix path resolution for savefile_directory and savestate_directory from retroarch.cfg
    - [x] Firmware install debug logging (rom_mate.library.firmware_install, gated by debug_prints config)
    - [x] saves_files routing infrastructure for saves-directory firmware placement
    - [x] Dolphin GC BIOS (IPL.bin) — auto-install to system/dolphin-emu/Sys/GC/<region>/ via dolphin-gc-bios.zip
    - [x] Firmware metadata gaps filled: DuckStation, SwanStation, GearLynx, ParaLLEl N64 (64DD), NeoCD, FBNeo (neogeo.zip), LRPS2 (pcsx2/bios), Citra (saves_files)
    - [x] Dolphin: auto-configure SkipIPL=False for boot animation
    - [x] Dolphin: auto-configure XInput GCPad controller mapping
    - Platform firmware testing status:
        - [x] Dreamcast — confirmed working (Flycast/RetroArch)
        - [x] GameCube — confirmed working (Dolphin standalone + RetroArch dolphin_libretro with IPL BIOS)
        - [x] PlayStation 1 — confirmed working (DuckStation standalone)
        - [x] PlayStation 2 — server has 5 BIOS files, PCSX2 has firmware_directories
        - [x] Game Boy — server has dmg_boot.bin, RetroArch system/
        - [x] Game Boy Advance — server has gba_bios.bin, RetroArch system/
        - [x] Game Boy Color — server has cgb_boot.bin, RetroArch system/
        - [x] TurboGrafx CD — server has 3 syscard files, RetroArch system/
        - [x] Sega Saturn — server has saturn_bios.bin, RetroArch system/
        - [x] Sega CD — server has 3 regional BIOS files, RetroArch system/
        - [x] Sega 32X — server has 3 BIOS files, RetroArch system/
        - [x] Sega Naomi — confirmed working (Flycast/RetroArch; naomi.zip MAME format, saved as archive)
        - [x] PlayStation 3 — PS3UPDAT.PUP firmware routing via firmware_directories: ["."], "Install PS3 Firmware" button added
        - [x] Xbox — server has 3 files, Xemu firmware_directories added (["."])
        - [x] Nintendo Switch — server has keys + firmware, Eden firmware_directories added

## RetroAchievements for emulators
- [ ] Research and implement RetroAchievements for emulators which support it. Prefill configs with the credentials the user has already input in Settings. Keep in mind that the user will need username and login token for the emulators while rom-mate uses the api key to do lookups.

# Medium Priority
- [ ] Tweak design of emulator auto-install window, it's pretty basic right now and could be more robust.
- [ ] RomM server already has a maximum saves/states implementation, remove the save-retention/slot-limit code and implement the openapi functions instead.

# Low Priority
all clear

# Future-Plans
nothing to see here

# Dream Features
- [ ] Investigate incorporating PS3 trophy data into existing achievements UI


# Project Cleanup Tasks


# Tasks For When I've Run Out Of Ideas
- [ ] Cross-platform support (Windows, Linux, Mac)