A proposal: back up games from the server to a USB flash drive or SSD, with their art, details, emulators and firmware. Then install them from that drive on any computer with GRID, with or without a network. The card is built on Proposed app and starts in Library on a connected drive. Everything not listed here matches Proposed app.

Not built yet. Branch `feat/game-backups`.

- **One folder per drive.** A drive is set up once. GRID makes a `gridpack` folder at the drive's root and never reads, writes or deletes anything outside it. A drive is a backup drive when `gridpack/gridpack.json` is on it, so GRID recognises it on any computer. A drive is identified by where it is mounted (its mount path, or its drive letter on Windows); a pack has no id of its own. GRID watches for removable drives (on Linux `/run/media/<user>` and `/media`, on Windows removable drive letters, on macOS `/Volumes`). A drive shows up within a few seconds of being plugged in and goes away when it is removed.
- **Settings › Backup drives** is a new pane, second in the alphabetical rail (Appearance, Backup drives, Cloud saves, …). It lists the drives that are connected, then the drives that GRID remembers but are not connected.
  - **Each drive is a card:** the pack name (or the volume label before setup), the label, the file system and the size, and a status (Ready for backups, Not set up, Not connected).
  - **A set-up drive** shows "For Linux and Windows" with **Change…**. Change… opens the same checkboxes inline. Adding a computer queues that OS's build of every emulator already on the drive ("Windows builds of 2 emulators added to the backup queue for Couch kit"). Removing one deletes those builds, and games and firmware stay. At least one must stay ticked.
  - **It also shows** a usage bar split into other files, GRID and free space, with a legend. Under it: the number of games, the emulators, the firmware, the date of the last backup and the folder path. Then Show in Library, Open folder, Rename… (inline) and Remove GRID files…, which needs a second confirmation. It deletes only `gridpack/`, says how much it will free, and leaves the games installed on this computer.
  - **A drive that is not set up** has **Set up for GRID…**. That opens an inline form with the pack name (the volume label by default), **Computers this pack is for** (Linux and Windows checkboxes, with this computer's OS ticked) and the note that only a `gridpack` folder is made. On a **FAT32** drive the form warns that files over 4 GB can't go on it and suggests exFAT. GRID never formats a drive.
  - **A remembered drive that is not connected** shows when it was last seen, its game count and its size, with Forget.
  - **Every backup also copies** has one default for the backup dialog: the server's firmware for the systems backed up, on by default. Covers, fanart, screenshots, logos, game details and the emulator each system needs always go with each game. Emulators are built for every computer the drive is for. Proton goes with Windows games only when the pack is for Linux.
  - **What's in the gridpack folder** (collapsed) shows the layout below.
- **Three ways to start a backup**, all opening the same dialog:
  - **Details:** Back up to drive…, under Install/Play and Uninstall. When a game is on a connected drive already, a line under it reads "On Couch kit".
  - **Select mode on the Server and Library grids:** Select in the toolbar (beside Size) turns it on, and the button reads Done while it is on. Clicking a cover picks it instead of opening Details, and Shift-click picks a range. A picked cover shows a filled check at its top left and a primary ring, and shrinks slightly. The hover bar is hidden. A selection bar pins to the bottom of the grid panel with the count and total size ("3 selected · 4.2 GB"), Select all N / All N selected, Clear and **Back up to drive…**. Esc or Done leaves select mode. Select is not offered on Discover.
  - **A whole platform:** "Back up all 11…" on the right of a Server platform header, and on a Library platform row (installed games only).
- **The backup dialog** opens over Details, like the other dialogs.
  - **Title:** "Back up 3 games", "Back up Cinder Peaks" or "Back up PlayStation · 4 games".
  - **Drive:** one radio row per connected, set-up drive, with the pack name, label, file system and free space, and a usage bar where this backup's share is hatched. A drive without room reads "Needs 76 GB · 61.4 GB free" in the danger colour, and choosing it disables the main button ("Not enough space"). Only set-up drives are listed. With none connected, the dialog says so, names any connected drive that is not set up, and offers **Set up a drive…**, which opens Settings › Backup drives.
  - **Also copy:** Emulators is always included and shown with a fixed tick ("PCSX2 for Linux and Windows · 120 MB", or "Already on the drive"). Firmware is a checkbox (systems and file counts), on by default from Settings. It is disabled with the reason when the drive already has the firmware or the server has none.
  - **Notes:** games already on the chosen drive are skipped ("1 game is already on Couch kit and will be skipped: Moss & Marrow"). On a FAT32 drive, games with a file over 4 GB are listed as not possible.
  - **Footer:** "11.1 GB to write", Cancel, and **Back up 3 games**, which reads "Nothing to back up" when everything is already on the drive.
- **Backups run in Downloads.** Each game, emulator and firmware set is a row with the kind badge **Backup**. Its status reads "Downloading from server 45%", then "Writing to SAMSUNG T7 70%". The sparkline is network-coloured while downloading and disk-coloured while writing. A queued row reads "Waiting · to Couch kit", and each row has Cancel. The strip reads "Backing up Velvet Static to Couch kit · 45% · downloading". A backup's game rows are written first, then its emulators, then its firmware. Each finished row goes to Completed as "Written to Couch kit".
  - **Notifications are one card per backup, not one per game:** "3 games added to the backup queue for Couch kit", then "3 games and 1 emulator backed up to Couch kit".
  - **If the drive is removed mid-backup,** the row being written fails ("Failed — SAMSUNG T7 was removed"). The rest of that drive's queue is cancelled as one Completed row ("13 more items · Cancelled — SAMSUNG T7 was removed · back up again to finish"). One error card says how many items were written. A half-written file is never left as a valid entry: GRID writes to a temporary name and renames it when the file is complete, and removes leftovers the next time the drive is connected.
- **Library › Backup drives:** a rail section under the platforms, with one row per connected, set-up drive (a drive icon, the pack name and its game count; pointing at a row shows the volume label).
  - **The header** shows the pack name, "8 games · 13.3 GB · on SAMSUNG T7" and chips for the drive's emulators and firmware.
  - **The grid** is the drive's games, with platform tags, search, sort and size. A game already installed shows Play. Any other game shows **Install**, which copies the game from the drive (no network). Its Downloads row and the strip read "Copying from SAMSUNG T7".
  - **Installing a game also sets up its system from the drive.** When the game's system has no emulator and the drive has one, GRID installs that emulator from the drive and makes it the system's emulator ("DuckStation from Couch kit added to download queue"). It also installs the drive's firmware for that system when the system needs it ("PlayStation firmware installed from Couch kit"). Windows games bring their Proton version the same way.
  - **Details from a drive** shows "From Couch kit" in the header line, **Install from drive** and **Remove from drive…** (two clicks).
  - **Select mode on a drive** has Install N games and Remove from drive… instead of Back up.
- **Use without a server.** The Connect screen has **Use without a server** under Connect, after an "or" divider. It needs only the library folder. GRID then opens on Library, on the first connected backup drive.
  - **In the top bar,** the connection dot is grey, the chip reads "Offline · no server", and its menu offers Connect to a server….
  - **Server** shows one panel: "Not connected to a server. Your Library and backup drives work without one" and Connect to a server….
  - **Library, backup drives, Downloads, Settings and play** all work offline. Backing up needs a server, so Back up to drive… is disabled offline, with that reason.
  - **Connecting later** keeps everything that was installed offline. Games installed from a drive keep their server id, so updates, cloud saves and play activity work once the computer is connected to that server.

**The gridpack folder**

```
gridpack/
  gridpack.json                  pack name, OS list, format version, created/updated, game index
  games/<platform>/<file>        exactly what the server gives: zip, 7z, iso, chd, or a folder
  assets/<platform>/<game>/      cover.jpg, fanart.jpg, logo.png, screenshots/1.jpg …
  emulators/<emulator>/<version>/<os>/   the release archive GRID would download, one per computer the pack is for
  firmware/<platform>/           the firmware files as the server serves them
```

`<platform>` is RomM's platform slug (`genesis`, `snes`, `ngc`, `win`), as in the ClasPlatf example drive. `<game>` is the file or folder name without its extension. The paths in `gridpack.json` are relative to `gridpack/`, so the drive works under any mount point. A draft:

```json
{
  "format": 1,
  "name": "Couch kit",
  "os": ["linux", "windows"],
  "created": "2026-10-12T18:20:00Z",
  "updated": "2026-11-07T09:02:00Z",
  "games": [
    {
      "platform": "genesis", "platform_name": "Sega Genesis",
      "source": { "server": "https://romm.home.lan", "rom_id": 1234, "igdb_id": 1991 },
      "name": "Aladdin", "summary": "…", "region": "USA",
      "companies": ["Virgin Games"], "release_date": "1993-11-11",
      "genres": ["Platform"], "game_modes": ["Single player"], "players": "1",
      "version": "1.0", "size": 2097152, "sha1": "…",
      "files": [ { "path": "games/genesis/Aladdin (USA).zip", "size": 2097152, "server_file_id": 5678, "name": "Aladdin (USA).zip", "category": "game" } ],
      "assets": { "cover": "assets/genesis/Aladdin (USA)/cover.jpg", "fanart": "…", "logo": "…", "screenshots": ["…"] },
      "added": "2026-11-07T09:01:00Z"
    }
  ],
  "emulators": [ { "name": "DuckStation", "version": "v0.1-9226", "os": "linux-x86_64", "platforms": ["psx"], "path": "emulators/duckstation/v0.1-9226/linux-x86_64/DuckStation.AppImage" } ],
  "firmware": [ { "platform": "psx", "files": [ { "name": "scph5501.bin", "size": 524288, "md5": "…" } ] } ]
}
```

Unlike the example drive, `games` is an array (not keyed by "0", "1" …), every game carries its server source, numbers are numbers, and each file is an object (path, size, server file id, name, category) so a drive install plans exactly like a server install. GRID rewrites `gridpack.json` through a temporary file and a rename after each finished item, so a removed drive always leaves a valid index.

**Decided**

1. A backup always downloads the game from the server, so the drive gets the server's original file, even when the game is installed here (the installed copy may be extracted or patched).
2. Each pack is for a set of computers (Linux, Windows), picked at setup and changed in Settings. Every emulator on the drive has a build for each one.
3. Firmware is a per-backup checkbox, on by default, with no warning. Players supply firmware from their own consoles.
4. One pack per drive. A game on several drives is listed under each drive, and Install copies it from the drive being viewed.
5. A drive is identified by its mount path or drive letter; packs have no id.
6. A manually added emulator (no download source) is copied as a zip of its program folder, for this computer's OS only.
7. A backed-up game waits in a staging folder in the library between downloading and writing.

The prototype's drives, sizes and dates are made up. "Prototype: unplug / plug in" buttons on each drive card simulate removing and reconnecting the drive.
