# Overview
The application is a game launcher and manager for RomM which connects to the server API specified in `openapi.json`.
It is built in Rust with Tauri 2 and a Svelte 5 frontend, and ships as a Windows installer and a self-contained AppImage on Linux.

# Top Bar
The top bar is 58px tall: the GRID logo and wordmark on the left, a centred pill
group with the five views (Library, Server, Downloads, Emulators, Settings) in that
order, and on the right a connection status dot, the server menu (Reconnect,
Disconnect, Open RomM in browser) and the app-update badge when a release notice is
stored. `Ctrl+1`..`Ctrl+5` switch views from the keyboard.

Views stay mounted and switch with the `hidden` attribute, so scroll positions and
selections survive a switch. Behind the content sits blurred background art — the
cover of the last game viewed, at the opacity set in Settings › Appearance.

A 28px download strip sits at the bottom of the window. It is hidden while nothing is
transferring; otherwise it reads `⬇ <title> · <percent> · <speed>` beside a small
sparkline of the current transfer's last 60 seconds (network in the primary colour, disk
in teal) and an "Open Downloads" link. Clicking anywhere on the strip opens the Downloads
view.

# Main Sections
There are several main sections to the application with buttons across the top bar to navigate between them:
- Library
- Server
- Discover (ON HOLD — not built; see below)
- Downloads
- Emulators
- Settings

- **Library** contains a 220px filter rail — All games, Recent (played in the last 30
  days), Updates, then each installed platform, every entry with a count — beside a
  toolbar (search by title, sort by Recently played / Recently installed / Title /
  Platform, card size Small / Medium / Large) and one grid of cover cards. Each rail
  entry has its own empty state. Clicking a card opens the details sub view; the hover
  Play launches directly. The card size is remembered as `ui.card_size_library`.

  **Moved and missing files.** At startup and on every Library refresh, GRID checks
  that each installed game's files are still on disk. It never deletes anything, and it
  looks only inside the current library folder, at the fixed places a game can sit
  (`games/<Platform>/`, the older `<library>/<Platform>/`, the library folder itself),
  by the game's own folder or file name — there is no disk-wide search. A game whose
  folder or archive moved to one of those places is pointed there silently, and plays
  from it. A game whose files are not found is **Missing**: its card shows a MISSING
  badge, and its hover action opens the details sub view instead of launching. A game
  installed outside the library (a native game elsewhere, a PS3 game inside RPCS3's
  `dev_hdd0`) is only checked, never searched for. Missing is worked out on each check,
  not saved: a drive that comes back clears it. Nothing is checked while a library move
  runs or while the library folder cannot be reached (the Unreachable library dialog
  handles that). In the details sub view a Missing game shows "Files not found", Play and
  Uninstall are disabled with that reason, and it offers:
  - **Locate…** — pick (or type) the folder that holds the game: the game's folder,
    archive or file must be inside it (or it is the game's folder itself, holding the
    launch file). The folder may be anywhere, because the user chose it. GRID points the
    Library row there and moves no file; a folder without the game is refused with the
    reason.
  - **Remove from library (keeps files)** — the same action as for a game with no server
    id below: the Library row goes, no file is touched.
- **Server** mirrors Library: a rail of the server's platforms (alphabetical by name) with ROM counts, a
  platform header (name, ROM count, installed count, a firmware chip with an Install
  action when the server offers firmware, and the platform's default emulator chip
  linking to the Emulators view), a search box that filters the loaded platform list by
  title, platform or genre, and a grid whose not-installed cards render dimmed until hovered. The hover action is
  Install for a game that is not installed and Play for one that is. The card size is
  remembered as `ui.card_size_server`. Downloads and installs are queued and handled in
  the background.
- **Discover** (ON HOLD since 2026-09-09 by user decision; not part of the current app — kept here as the reference if it is revived as a redesign) contains collapsible, cached recommendation sections built from the server library — Short But Fun, New on Server, Highly Rated (All-Time), Recommended for You, Browse by Genre, and Your Watchlist — with genre/platform filters and a manual refresh. Sections load in the background and fall back to the cached view when the server is unreachable.
- **Downloads** is a full view, capped at 1100px and centred, with three stacked
  segments in this order — Active (downloading, installing, cancelling), Queued, and
  Completed (finished, failed, or cancelled) — each with a count, under the legend
  "Active: downloading or installing · Queued: waiting for a slot · Completed: finished,
  failed, or cancelled". Each row shows the title, a kind badge (none for a base game;
  Update, Content, Emulator, Compat tool, or Firmware otherwise), the platform, the
  status/progress line, a progress bar, a 120×38 sparkline panel of the last 60 seconds
  (network in the primary colour, disk in teal, with the rate and ETA under it), and the
  action for its state: Cancel, Retry and Dismiss, or Dismiss. A live firmware row offers
  no action. The Completed segment keeps the last 50 entries.
- **Emulators** is a rail of four category panes, each capped at 1100px and centred:
  **Installed** lists every configured emulator (name, path, arguments) with Edit and
  Delete — Edit opens the manual form as a sheet beside the list — and, for an RPCS3
  entry with a downloaded PS3 firmware package, the note and the Install PS3 Firmware
  button; **Add from catalog** has a search box, one row per catalog entry with its
  provider and an Install / Installed button, and a Manual tab for the hand-typed form
  (name, executable path, arguments, with the profile auto-fill); **Platform defaults**
  has **Set up platforms…** (the first-run platform picker, see First Run Setup) and
  shows one card per server platform with the emulator select (only compatible
  emulators, plus "(none)") and, for a RetroArch default, the core select; **Compat
  tools** (Linux only) is the wine/proton default picker and its install catalog, plus a
  umu-run line: which umu-run a Proton launch uses ("system" with its path, "GRID-managed"
  with its release, or "not installed"), with Install (none installed) or Reinstall (GRID's
  copy; fetches the newest release), busy while its Downloads row runs, and a note that the
  first Proton launch downloads the Steam Linux Runtime (about 200 MB). Installing a Proton
  tool while no umu-run is available also queues umu-launcher as its own Downloads row; its
  failure never fails the Proton install. Each
  emulator entry includes a name, executable path, launch arguments, save strategy,
  ignore rules, and optional custom save/state directories.
- **Settings** is a rail of six panes: **Connection** (server URL, user, whether a
  credential is stored in the OS keyring and whether the session is verified, Reconnect,
  Disconnect), **Library** (the library folder and Change…: the new folder is typed or
  picked with Browse…, and is refused when it is not a full path, is the current folder,
  is inside it or contains it, cannot be created or written, or while a download or
  install, a running game, a cloud transfer or a library move is active; then **Move
  existing files** or **Start fresh**. Move first shows what moves — the `games`,
  `emulators` and `saves` folders, each "same drive" (a rename, instant) or "copied and
  checked" (another drive), with "Same drive — instant" or "Copies N GB; X GB free" — and
  says what waits while it runs. Start move runs it in the background as one row in
  Downloads ("Library move", with a line such as `games/ · 1.2 of 5.6 GB`); the pane shows
  "Moving to <new>…" and Change… is disabled. While the move runs or waits, installs,
  uninstalls, game and emulator launches, emulator and firmware installs, cloud transfers
  and another folder change are refused with "The library is being moved…"; an
  auto-upload that is due waits for the move. Copied files are checked (every file's
  size; a full SHA-256 for `saves`, the first and last MiB for `games` and `emulators`)
  before anything switches; then every stored path is rewritten (installed games,
  emulator entries and the emulator config files that hold absolute library paths) and
  the library folder changes. The old copies are deleted only if they did not change
  during the move (a changed one is kept and named in the finish toast), and the old
  folder goes only when it is empty. Cancel (in Downloads) is offered only while files
  copy and leaves the old library exactly as it was; once the copy is done the move can
  no longer be undone. If GRID closes or crashes, the move resumes at the next start
  ("Resuming the library move to <new>"); a move that stopped with an error waits —
  Retry runs it again, and while it is still copying Cancel undoes it. Unrelated files in
  the old folder stay. Start fresh asks
  "Delete the old game files, or leave them on disk?". Either way the games whose files
  are in the old folder leave the library and new installs go to the new folder; games
  installed outside the old folder, emulators and saves are not touched. Leave removes
  the library entries only. Delete first shows the folders it removes and their total
  size, and removes them only through the guarded uninstall after that confirmation; a
  game whose files could not be removed stays in the library. The old folder is
  remembered, so no later uninstall can remove it or its top-level folders), **Cloud saves** (restore before launch, upload after exit, skip when the
  local save is newer, upload delay, a save retention limit — default 10, 0 = unlimited;
  the server's autocleanup applies it to slotted saves and the app prunes the others; an
  install that still has the old default 3 moves to 10 once), **RetroAchievements** (username
  and a write-only token; Save fans the credentials out to every emulator config that
  supports them, Clear removes them; saving the username, by Save or Log In, also sets it
  on the server account — the username only, never the token — when it differs from the
  server's, so the Achievements tab can show progress, and asks the server to refresh that
  progress; if the server update fails the local save still stands and a short notice
  says so; the server-side name is never cleared), **Updates** (the running version, when the startup check
  ran, the release notice with its Open release and Dismiss buttons, and the note that
  the launcher only ever checks — it never downloads or installs an update), and
  **Appearance**.
- **Appearance** (under Settings) chooses the theme — follow the OS, dark, or light —
  turns the background art on or off, sets its fade from 0 to 60 percent with a live
  preview behind the pane, and sets the default card size for the Library and Server
  grids. They are stored under `[ui]` in `config.toml` as `theme`, `background_fade`
  (0 is off), `card_size_library` and `card_size_server`.
- **Cards** (both grids) scale on hover with a gradient overlay, a centred Play or
  Install, and an action row of Details, Cloud sync and More. Badges: an installed dot
  top-right, an `UPDATE` tag top-left, a cloud icon bottom-right when the platform has
  cloud sync configured, and a short platform name bottom-left. `Ctrl+F` focuses the
  current view's search box.

## Emulator Configuration
- Emulators should support launch arguments with placeholders such as `%rom%`, `%core%`, `%RPCS3_GAMEID%`, and `%ps3_gameid%`.
- Save strategy should support `auto`, `single_file`, and `folder` modes.
- Users should be able to define ignored files/extensions and semicolon-separated save/state directories for cloud sync handling.
- The app should support default emulator assignment per platform, including RetroArch core selection where applicable.
- Every emulator install reads the header of the program it is about to register (ELF,
  PE or Mach-O). A program built for another processor or system is never picked; when a
  build has nothing that runs here, the install fails and its Downloads row says what the
  program is built for ("pico8 is built for 32-bit ARM Linux, and this computer is 64-bit
  x86 Linux") and, for a server build, which file's `(arch-…)` tag to check. When a launch
  fails because its program cannot run here, the error says so in the same words instead
  of "Exec format error".
- Installing a folder from the server's "Emulators" platform that follows the packaging standard (`docs/server-emulator-packages.md`) installs each emulator in it that GRID knows, from just its newest build for this system, the same way first-run setup does: under `emulators/<Name>`, registered with its server source and version. Emulators › Installed's **Update** for such an entry compares that version with the newest build for this system in the same folder (number by number, so 2.0.10 is newer than 2.0.9; a pre-release such as 0.2.7a6 or 2.0.0-rc.1 is older than its release, and a lone letter such as 0.2.5g is a later fix) and installs it over the same entry. A package that does not follow the standard installs as below.
- A package installed from the server's "Emulators" platform is registered as an emulator. It is downloaded and extracted under `games/Emulators/<package>` like a game, and its hidden library row is kept. GRID then picks the executable for this system (`.exe` on Windows; an AppImage, `.sh` or executable file on Linux) and matches it to a bundled emulator profile. A match gives the profile's name and arguments; no match gives the ROM title and `%rom%`. When another emulator entry already has that name, the package is added as "<Name> (server)", so a catalog entry and its update source stay unchanged. The profile's user-data folders are linked into `saves/`, the emulator is configured and gets default platforms like a catalog install, and its Downloads row shows the Emulator badge. Reinstalling the package updates the same entry and keeps the user's arguments. Deleting the emulator under Emulators › Installed also uninstalls its server package.

## Sub Views

### First Run Setup
On first launch, or when required configuration is missing, the Connect screen asks for:
- Server URL
- API token (or username and password)
- Library folder — only while none is stored. It is pre-filled with `~/Games/GRID`
  (`%USERPROFILE%\Games\GRID` on Windows), has Browse…, and is required: nothing can
  install without it. A folder that cannot be stored stops the connect with the reason.

The Connect button stays disabled until every shown field is filled. The library folder
can be changed later under Settings › Library.

**Choose your platforms.** After the first successful connect of a fresh install (setup
never finished or skipped, no emulator configured, no game installed), a platform picker
comes before the shell. It lists the server's platforms that need an emulator (native PC
platforms and the "Emulators" platform are left out) with their ROM counts, in
alphabetical order, none ticked:
- Ticking a platform pre-selects GRID's recommended emulator for it; a select offers the
  other compatible emulators installable on this system. The recommendation is the
  platform's standalone emulator (an ordered list per platform in
  `platform-emulators.json`, then keyword matching); RetroArch is recommended only for a
  platform no standalone emulator serves (NES, SNES, Game Boy, Arcade…), and is offered
  at all only when a RetroArch core is known for the platform.
- The server's own emulators are offered too, when its "Emulators" platform follows the
  packaging standard (`docs/server-emulator-packages.md`): one folder per platform named by
  its slug, plus a shared `multi` folder, each holding builds named
  `<Emulator> (v<version>) (arch-<system>-<cpu>).<ext>` (system `linux`, `windows` or
  `macos`; cpu `x64`, `arm64`, `armhf`, or `universal` for macOS). For each emulator
  GRID knows, the newest build for this system and processor appears as "<Name> <version> · from your
  server": a slug folder for its own platform, `multi` for the platforms the emulator
  serves (RetroArch only where a core is known). It comes after the catalog download
  (which stays the recommendation) and before catalog RetroArch; where the catalog has
  nothing for a platform (Pico-8), the server build is the recommendation. Files that do
  not follow the standard (including a bare `arch-linux` with no processor), builds for
  another system or processor and emulators GRID does not know are not offered. A chosen build downloads just that one file, installs under
  `emulators/<Name>` and registers with its source (`romm:<folder>/<emulator>`) and
  version; the platform's default goes to its entry whatever name it gets (e.g. "Pico-8
  (server)").
- A platform nothing installable serves shows "No emulator available" and cannot be
  ticked; one whose recommended emulator is already configured shows "Installed".
- **Install N emulators** (N counts emulators, not platforms) queues one catalog install
  per chosen emulator and opens the app at once. A card over the views follows the
  installs ("Setting up 3 emulators · 1 done"), with Retry for a failed one, and ends
  with how many platforms are ready.
- As each emulator installs, every platform it was chosen for gets it as its default
  emulator, replacing an automatic default but never an explicit "(none)". A RetroArch
  choice becomes the default only when a compatible core is actually installed; otherwise
  the platform is left without a default and the card says so. A chosen default waits
  for its own emulator, so a failed install that is retried still gets it.
- **Skip for now** opens the app with nothing installed. Either way the picker is not
  shown again on its own; **Emulators › Platform defaults › Set up platforms…** opens it
  any time (Cancel closes it).

If the server rejects the stored credential (HTTP 401), at startup or during a session, the app returns to this screen. The server URL (and username, for a password) is filled in, the credential field is empty, and a line explains why ("The server rejected your token. Enter a new one."). The stored credential stays in the OS keyring until a new connect succeeds. When the server is unreachable or denies permission (HTTP 403), the app stays open and shows "Not connected" with Retry.

### Unreachable library
GRID cannot run without a valid library. When the library folder cannot be reached — the
folder is gone, its drive is not connected (for example a USB disk unplugged after a
move), the path is a file, or GRID may not read it — and GRID has something stored there
(an installed game, an emulator, or a library move in progress), a blocking dialog
replaces the whole app at startup, or as soon as a library move finds its library folder
gone. It names the folder and the reason, and offers exactly two choices: **Relocate the
library…** (type the folder or pick it with Browse…; it must hold a `games`, `emulators`
or `saves` folder; GRID rewrites its stored paths to it and moves no files, then starts
normally) and **Close without changing anything** (GRID exits and writes nothing). There
is no "continue anyway". During startup with an unreachable library nothing writes into
it: the layout migration and a library move's recovery wait until it is reachable. A
library path that is not set yet is a first run (First Run Setup above), and a set
folder that GRID never stored anything in is created again by the next install; neither
shows the dialog.

### Game Details View
Clicking a game in the Library or Server sections opens a popup, at most 1040×680, centred
over a dimmed and blurred shell. Esc and the ✕ close it.

The left column is fixed at 240px: the large cover, then the primary action (Play, Stop,
Install / Install App, or Cancel while an install is live), Update when the server holds a
newer copy, Install Update / Install DLC for PS4 and Xbox 360 games, Game Settings for
native games, Uninstall, and a cloud button that opens the Saves tab. Under them the column
states when the game was last played and which emulator — and, for RetroArch, which core —
would launch it.

Uninstall never removes a protected folder (a filesystem root, home, a library folder, its
top-level and platform folders, a folder that holds the game's install folder, a path with
`..`); when it keeps one, an error toast names each kept folder and the reason. This applies
to Details › Uninstall, deleting a server-package emulator, and Start fresh › Delete.

The right side carries the title, one header line (platform · release year · developer ·
genres · rating), chips for the playing state, the identification state, region and language
flags and the version, then up to five tabs:

- **Overview** — the summary, a metadata grid (developer, companies, release, genres, game
  modes, players, franchises), the first six screenshots, and a Related row filtered to
  titles the server actually holds for that platform.
- **Media** — every screenshot plus the trailer and any server-hosted video. Clicking a tile
  opens a fullscreen viewer with arrows, Esc and a caption.
- **Saves** — the Manage Saves / Emulator Saves / Manage States panels described below.
- **Files** — every file the server lists with its size and its version (the version tag
  parsed out of the file name, else the file's last-modified date), the installed-versus-
  server version line, the PS4 / Xbox 360 content files, and the platform's firmware row
  with an Install action when the server offers firmware.
- **Achievements** — shown only when the server lists RetroAchievements achievements for
  the game. The data comes from the server only: the game's achievement list and the
  progress of the server account. A summary line (unlocked of total, points earned of
  total, the hardcore count when there is one) sits above the rows; each row has the badge
  (the locked badge while not earned), title, description, points and, once earned, the
  unlock date with a hardcore mark. Earned rows come first, newest first, then the locked
  rows in the game's own order. Badges load as they scroll into view. Progress needs a
  RetroAchievements username on the server account; without one the tab lists the
  achievements without progress and says "Set your RetroAchievements username in
  Settings to see your progress."

The last tab is remembered for the rest of the session. When the remembered tab is
Achievements and the game has none, the popup opens on Overview.

**Installed games with no server id.** A Library row can have no server ROM id: a game
imported from the previous app, or one installed before ids were recorded. Such a row
cannot use Play, cloud saves or update checks.

- **Auto re-link on connect.** After each successful connect, restore or retry, GRID reads
  the server's ROMs for the platforms that have such rows. It compares titles case-folded,
  with punctuation ignored, and matches the row's platform against the server platform's
  display name, custom name, name or slug. It links a row only when exactly one server ROM
  matches, and never to a ROM another installed row already holds. Every other row stays
  unlinked. A linked row gets its covers from the server on the same pass.
- For a row that stays unlinked, Details shows "Not linked to a server game" and two actions
  in place of Play / Install:
  - **Link to server game** opens a searchable list of that platform's server games (the
    same title / platform / genre search as the Server view). Choosing one links the row,
    and Details re-opens on the linked game. Games another installed row holds are not
    listed. Needs a connection.
  - **Remove from library (keeps files)** deletes the Library row only. It never uninstalls
    and never touches a file. A confirm names the folder that stays on disk.

The button bar area should include:
- `Manage Saves` for normal per-game cloud-save platforms
- `Emulator Saves` for shared/global save media such as Xemu and Redream VMUs
- `Manage States` when state sync is supported for that platform

### Manage Saves/States
Clicking the Manage Saves, Emulator Saves, or Manage States buttons in the button bar should switch the middle column of the Game Details view immediately and then populate the cloud list asynchronously for responsive feedback. The view should include a button to manually upload the latest save/state for that scope.

The Saves/States should be listed in a vertical column with the name of the emulator used, file size, upload time and display relative 'x hours/minutes ago'. Each save in the list should also have right-aligned buttons: one to manually restore the save from the server, overwriting the local save (with confirmation prompt explaining the overwrite) and one to delete the save from the server (with confirmation prompt).

When the panel is showing `Emulator Saves`, the entries represent shared emulator media rather than a single game's private save files, and the UI should warn the player that restoring or deleting those entries can affect every game using that shared emulator storage.

### Native Game Settings
A game is native when its server platform's identity (the RomM platform slug: `win`, `win9x`, `linux`, …) is a Windows or Linux one, not because of the platform's display name; a custom name such as "PC Fangames & Ports" on slug `win` is still native, and the name rule ("Windows…"/"Linux…") applies only when the slug is unknown.

For installed native games, the details view should expose a game settings dialog where the player can:
- choose the launch executable discovered from the install directory
- set custom launch parameters for that specific game

Valid launch targets should include `.exe`, `.bat`, `.cmd`, `.ps1`, and `.sh` files where relevant. On Windows, a `.bat`/`.cmd` starts through cmd.exe and a `.ps1` through `powershell.exe -NoProfile -ExecutionPolicy Bypass -File`; on Linux, a `.sh` starts through its shebang (the install sets its execute bit).

On Linux, a Windows game whose compat tool is a Proton install runs through `umu-run` with `PROTONPATH` set to that tool. The launch uses a system `umu-run` on `PATH` first, else GRID's own copy: the upstream umu-launcher zipapp, downloaded into the managed compat-tools folder (one shared copy, checked against the release's published SHA-256 digest; a mismatch fails the install and deletes the download). GRID's copy is a Python program, so its launch first checks that `python3` is 3.10 or newer and otherwise fails with a message saying so. With no umu-run at all, the launch fails with "umu-run is not installed…" and the Library, Server and Details launch errors offer **Install umu-launcher**, which queues the same install; the player launches again when it finishes. GRID never updates its copy on its own.

#### Launch info in `game.json`
A native game's RomM entry can list a `game.json` file next to the archive. Besides its metadata (`version`, `year`, `tags`, `included_dlc`), it can carry an optional `launch` section:

```json
{
  "launch": {
    "executable": "bin/start.bat",
    "arguments": "-windowed \"Profile One\"",
    "working_dir": "bin"
  }
}
```

- `executable` (required in the section): the launch file, relative to the game folder (the extracted archive's root). `/` and `\` both separate folders.
- `arguments` (optional): one string, split like the Custom Launch Parameters field (quotes group words).
- `working_dir` (optional): relative to the game folder; `"."` is the game folder itself. Without it, the game starts in the launch file's own folder.

Rules:
- Paths must stay inside the game folder: no absolute path, no drive prefix (`C:`), no `..`. The launch file must exist and be launchable. Fields of the wrong type count as missing; unknown keys are ignored.
- At install, a valid section sets the launch file, the arguments, and the working dir. An invalid one does not stop the install: the download row shows "game.json launch section ignored: <problem>", and the launch file is chosen automatically.
- Precedence: the player's choice in Game Settings, then `game.json`, then the automatic choice. Changing the launch file or the parameters in Game Settings makes them the player's choice, which `game.json` never replaces. Saving without changing them keeps the `game.json` launch file.
- An update reads the server's `game.json` again: a launch file it set follows the new section, and the player's choice stays.
- A game installed before its `game.json` had a launch section picks it up without a reinstall: automatically during the update check, or at once with "Reload launch info from server" in Game Settings, which reports what it found.
- Game Settings marks a launch file that came from `game.json` with "Set by game.json".
- The archive itself is never read or changed for this.

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
- save retention after each successful save upload: the newest N per game and slot are kept (a slotted upload asks the server to clean its own rom+slot group; the app prunes null-slot saves and never deletes a slotted one); 0 keeps everything. States have no limit: each state is uploaded under its own slot file name, and the server replaces the record for that file name, so it keeps one copy per state slot file
- immediate UI view switching with asynchronous cloud-record loading so the details panel remains responsive

RPCS3 save restores (automatic before launch and manual) write only the savedata folders that belong to the game, because older releases could upload other PS3 games' folders under one game. A folder belongs to the game when its name holds the game's PS3 id; when the id is unknown, when its name holds a serial found in the game's file names, install paths, or install `PARAM.SFO`, or when the folder's own `PARAM.SFO` title equals the game title (one serial only). A folder that carries another installed game's id is skipped. Skipped folders are named in a notice (a toast before launch); when no folder matches, nothing is restored. A manual restore then offers to restore every folder after an explicit confirmation. Before launch, the game's own record is preferred over another game's newer record by its file name.

Cemu save restores (automatic before launch and manual) write into the game's own title folder, `<mlc>/usr/save/<high>/<low>/user/`, which the app finds from the game's Wii U title id. This applies to records from older releases too. Before a restore replaces local save files, the app copies those files to a backup folder beside `user` (`user.grid-backup-<time>`) and names it in a notice. When the title id is unknown, nothing is restored and a notice says why. A Cemu upload puts all user folders of the title in one record whose file name starts with the title id; before launch, that record is preferred over a newer record without the id.

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

## Play activity
Every finished game session of 30 seconds or more is sent to RomM as a play session (start, end, duration). It is always on; there is no setting. Sessions without a server ROM id and Emulators-platform entries are not sent. While the server is unreachable, sessions wait in a local queue and go out after the next successful connect or restore. RomM's "last played" for the game follows the session end; the app never changes the game's play status on the server.

## Screenshot upload
When uploading saves or states, the app should attempt to attach the most recently captured screenshot as a companion `screenshotFile`.
- PPSSPP and RetroArch use file sidecars; all other supported emulators scan the emulator's screenshot directory for the most recent image within the session window. See ARCHITECTURE.md maintenance notes for routing details.

# Additional Platform-Specific Behavior
- RetroArch support should include installed core detection and per-platform core assignment.
- RetroArch firmware auto-install supports: BIOS files routed to `system/` with optional subdirectory, `.opt` core option files routed to `config/<corename>/`, MAME-format zip archives preserved intact, path-preserving zip extraction for archives with internal directory structure, and saves-directory placement for cores that require it. All routing is driven by per-core metadata in `retroarch-core-list.json`.
- RPCS3 / PS3 handling should support the game-launch requirements needed by that emulator setup.
- Theme selection should support `system`, `dark`, and `light` modes and apply consistently across the app UI.
- Settings should include an option to enable debug prints and a shortcut to open the config folder.