# Overview
The application is a game launcher and manager for RomM which connects to the server API specified in `openapi.json`.
It is built using Python + PySide6 and is intended to ship as a self-contained executable on Windows and a wrapper-based launch target on Linux.

# Top Bar
The top bar includes navigation buttons for the main sections of the application and shows the current signed-in user on the right.
It should also surface connection/download state in a lightweight, always-visible way while transfers are active.

# Main Sections
There are several main sections to the application with buttons across the top bar to navigate between them:
- Library
- Server
- Downloads
- Emulators
- Settings

- **Library** contains a grid layout of all currently installed games, represented by cover art and sorted for quick browsing. Clicking a game opens the details sub view.
- **Server** contains a vertical list of server-supported platforms on the left and a searchable grid layout of games for the selected platform on the right. Downloads and installs should be queued and handled in the background.
- **Downloads** contains a list of queued, active, installing, completed, and failed download jobs. Each entry should display status/progress details and support the appropriate action such as Cancel, Retry, or Dismiss.
- **Emulators** contains the list of emulators used to launch games. Users can add, update, remove, and assign defaults by platform. Each emulator entry includes a name, executable path, launch arguments, save strategy, ignore rules, and optional custom save/state directories.
- **Settings** contains the application settings arranged by panels for server connection, library path, appearance, debug options, and cloud save sync behavior.

## Emulator Configuration
- Emulators should support launch arguments with placeholders such as `%rom%`, `%core%`, `%RPCS3_GAMEID%`, and `%ps3_gameid%`.
- Save strategy should support `auto`, `single_file`, and `folder` modes.
- Users should be able to define ignored files/extensions and semicolon-separated save/state directories for cloud sync handling.
- The app should support default emulator assignment per platform, including RetroArch core selection where applicable.
- Known emulator packages downloaded from the server may be auto-configured using bundled emulator profiles.

## Sub Views

### First Run Setup
On first launch, or when required configuration is missing, the app should prompt for:
- Server URL
- API token
- Library path

The user must complete this setup before continuing into the main application.

### Game Details View
Clicking on a game in the Library or Server sections opens a sub view with more information about the game and a button bar with Back, Details, and cloud save/state actions when supported.
The sub view should include a larger cover art image, title/description/platform/rating information, and any available screenshots in a vertical scrollable area.

The action area should be left-aligned in the center column and should update based on the game state and may include:
- `Install Game` / `Install App`
- `Play`
- `Config`
- `Uninstall`

The button bar area should be left-aligned above the cover art in the left column and include:
- 'Back'
- 'Details'
- `Manage Saves` for normal per-game cloud-save platforms
- `Emulator Saves` for shared/global save media such as Xemu and Redream VMUs
- `Manage States` when state sync is supported for that platform

### Manage Saves/States
Clicking the Manage Saves, Emulator Saves, or Manage States buttons in the button bar should switch the middle column of the Game Details view immediately and then populate the cloud list asynchronously for responsive feedback. The view should include a button to manually upload the latest save/state for that scope.

The Saves/States should be listed in a vertical column with the name of the emulator used, file size, upload time and display relative 'x hours/minutes ago'. Each save in the list should also have right-aligned buttons: one to manually restore the save from the server, overwriting the local save (with confirmation prompt explaining the overwrite) and one to delete the save from the server (with confirmation prompt).

When the panel is showing `Emulator Saves`, the entries represent shared emulator media rather than a single game's private save files, and the UI should warn the player that restoring or deleting those entries can affect every game using that shared emulator storage.

### Native Game Settings
For installed native games, the details view should expose a game settings dialog where the player can:
- choose the launch executable discovered from the install directory
- set custom launch parameters for that specific game

Valid launch targets should include `.exe`, `.bat`, `.cmd`, `.ps1`, and `.sh` files where relevant.

# Cloud Save Synchronization
The application now includes cloud save and state synchronization for emulator-based games using the RomM saves/states API.

Core behavior should include:
- manual upload and restore actions from the Game Details view
- optional automatic download of the latest cloud save before launch
- optional automatic upload of saves when a game closes
- optional skipping of cloud download when the local save appears newer
- separate handling for save files and emulator state files
- per-emulator configuration that determines how save discovery and filtering works
- shared-save emulator scopes that surface emulator-wide backups with explicit warnings instead of pretending they are per-game saves
- immediate UI view switching with asynchronous cloud-record loading so the details panel remains responsive

## Folder-based saves
For emulators which store saves in a subfolder, the archive should include the relevant subfolder and its contents.

## Single-file saves
For emulators which store saves as a single file in a common location, memory cards for example, the archive should include the file or files relevant to the game being saved.
Configured save/state roots may point to either directories or explicit files from the emulator autoprofiles.
When multiple companion files belong to one logical save upload, the client should package them into a single archive for upload and unpack that archive back into the proper local location during restore.
Distinct savestate slots should remain separate logical uploads.

Special shared-media handling should include:
- Xemu save uploads should use one shared emulator-media backup scope per user to avoid accidental cross-game rollback confusion.
- Redream regular saves should retain one latest backup per VMU slot globally per user.
- Redream savestates should remain per-game safe to sync and must recognize both title-based and hash-based numeric `*.0.sav` naming.

## Session-aware sync behavior
The sync flow should track recent game sessions so that auto-upload focuses on files changed during or immediately after play.
Some emulator/platform combinations may limit state-sync actions when that behavior is not reliable.

## Screenshot upload
When uploading saves or states, the app should attempt to attach the most recently captured screenshot as a companion `screenshotFile`.
- PPSSPP and RetroArch attach screenshots via file sidecars: PPSSPP replaces the state file extension with an image extension; RetroArch appends an image extension to the full state filename.
- All other supported emulators (DuckStation, PCSX2, Dolphin, RPCS3, Cemu, Azahar, Xemu, Eden, MAME, FBNeo, Xenia) attach screenshots by scanning a known emulator screenshot directory for the most recently captured image within the current game session window. No screenshot is attached if no image was captured during the session.
- Emulator profiles define the screenshot directories to scan via the `screenshot_directories` key in `emulator-autoprofiles.json`.

# Additional Platform-Specific Behavior
- RetroArch support should include installed core detection and per-platform core assignment.
- RetroArch firmware auto-install supports: BIOS files routed to `system/` with optional subdirectory, `.opt` core option files routed to `config/<corename>/`, MAME-format zip archives preserved intact, path-preserving zip extraction for archives with internal directory structure, and saves-directory placement for cores that require it. All routing is driven by per-core metadata in `retroarch-core-list.json`.
- RPCS3 / PS3 handling should support the game-launch requirements needed by that emulator setup.
- Theme selection should support `system`, `dark`, and `light` modes and apply consistently across the app UI.
- Settings should include an option to enable debug prints and a shortcut to open the config folder.