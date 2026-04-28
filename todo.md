# Top Priority

## Other Active Tasks
- [ ] Setup automatic tweaking of emulator settings on install or launch.
    - [x] RetroArch, Dolphin, DuckStation, PCSX2, Azahar, Cemu, Eden, Xemu, RPCS3, PPSSPP, ShadPS4, Xenia, Redream — complete
    - [ ] MAME (Arcade) — not yet tested; needs autoinstall source and default config
    - [ ] FBNeo (Arcade) — not yet tested; needs autoinstall source and default config
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
    - [x] Eden, Xemu, RPCS3, PPSSPP, ShadPS4, Xenia, Redream — complete

- [x] Setup automatic download and install of BIOS files — complete for all supported platforms

## RetroAchievements for emulators
- [ ] Research and implement RetroAchievements for emulators which support it. Prefill configs with the credentials the user has already input in Settings. Keep in mind that the user will need username and login token for the emulators while rom-mate uses the api key to do lookups.

## PS3 Trophy Data
- [ ] **Research** — Investigate how RPCS3 stores trophy unlock data on disk (location within `dev_hdd0`, file format, per-user layout) and whether it can be extracted/restored without breaking RPCS3's internal trophy state
- [ ] **Design** — Determine whether trophy data fits the existing slotted cloud save model (single archive per game per user) or requires a dedicated trophy-backup slot separate from save data
- [ ] **Evaluate API** — Check `openapi.json` for any saves/states endpoint that could carry trophy archives, or whether a new server-side concept is needed
- [ ] **Implement** (if feasible) — Trophy backup/restore as a Manage Saves action in the PS3 game details panel with clear UI warning that restore overwrites all trophy progress for that game


# Medium Priority
- [ ] Tweak design of emulator auto-install window, it's pretty basic right now and could be more robust/informative.
- [ ] RomM server already has a maximum saves/states implementation, remove the save-retention/slot-limit code and implement the openapi functions instead.

# Low Priority
all clear

# Future-Plans
nothing to see here

# Dream Features
nothing to see here


# Project Cleanup Tasks
nothing right now

# Tasks For After v1.0 Release
- [ ] Investigate cross-platform support
    - [x] Windows - Native exe Support
        - [x] Using pyinstaller to generate an executable
    - [ ] Linux
    - [ ] MacOS