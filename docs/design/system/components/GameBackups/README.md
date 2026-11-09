A proposal: backup games from the server to a USB flash drive or SSD, with their art, details, emulators and firmware. Then install them from that drive on any computer with GRID, with or without a network. The card is built on Proposed app and starts in Library on a connected drive. Everything not listed here matches Proposed app.

Built on branch `feat/game-backups`. `SPEC.md` ("Game backups") describes what shipped.

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
  - **Details:** Backup to drive…, under Install/Play and Uninstall. When a game is on a connected drive already, a line under it reads "On Couch kit".
  - **Select mode on the Server and Library grids:** Select in the toolbar (beside Size) turns it on, and the button reads Done while it is on. Clicking a cover picks it instead of opening Details, and Shift-click picks a range. A picked cover shows a filled check at its top left and a primary ring, and shrinks slightly. The hover bar is hidden. A selection bar pins to the bottom of the grid panel with the count and total size ("3 selected · 4.2 GB"), Select all N / All N selected, Clear and **Backup to drive…**. Esc or Done leaves select mode. Select is not offered on Discover.
  - **A whole platform:** "Backup all 11…" on the right of a Server platform header, and on a Library platform row (installed games only).
- **The backup dialog** opens over Details, like the other dialogs.
  - **Title:** "Backup 3 games", "Backup Cinder Peaks" or "Backup PlayStation · 4 games".
  - **Drive:** one radio row per connected, set-up drive, with the pack name, label, file system and free space, and a usage bar where this backup's share is hatched. A drive without room reads "Needs 76 GB · 61.4 GB free" in the danger colour, and choosing it disables the main button ("Not enough space"). Only set-up drives are listed. With none connected, the dialog says so, names any connected drive that is not set up, and offers **Set up a drive…**, which opens Settings › Backup drives.
  - **Lede:** "GRID copies games installed on this computer and downloads the rest from romm.home.lan, then writes each one, with its art and details, to the drive's gridpack folder." When nothing is copied from this computer it reads "GRID downloads each game from romm.home.lan and writes it …".
  - **Source** sits between Drive and Also copy, and shows only when a requested game is installed on this computer.
    - **Copy installed games from this computer** is a checkbox, ticked by default. Its note counts the sources: "4 games from this computer (12.3 GB) · 2 downloaded · 2 to choose". Unticked, it reads "Every game downloads from romm.home.lan" and the rows and the choice are hidden.
    - **One row per game being written**, with its platform and a tag: **From this computer** (drive icon) or **Download** (download icon). A game that downloads although it is installed says why under its name: "Its files weren't found on this computer" (Missing), "Installed outside the library folder" (a native game moved with Locate…, or a PS3 game in RPCS3's `dev_hdd0`), "Its install contains links", or a name that can't go on a Windows pack. Xbox 360 games and Emulators-platform packages always download. A game that is not installed has no note. With more than 6 games the rows sit in a collapsed **Show each game** (with the count).
    - **What is copied.** An install that is exactly the server's files (a single file as downloaded, or a multi-file game whose files match by name and size) goes on the drive as those files, the same as a download. Any other install (an extracted game, a PS4 tree, a native Linux or Windows game) goes as an uncompressed tar in parts of at most 4 GiB, so it fits FAT32. A Windows game's Wine prefix is never copied; when one is in the backup the section says "Windows games are copied without their Wine prefix, so saves kept in the prefix stay on this computer."
    - **Out of date.** When the server has newer files for a local game, its tag reads **Choose below** ("the server has newer files") and a warning-edged block asks: "Lantern Vale: the server has newer files (v1.2; this computer has v1.1)", with two radios, **Use this computer's copy** and **Download v1.2 from the server** ("an older copy" when the installed copy has no version tag). With more than one such game, the block's title counts them and two bulk buttons set every choice: **Use this computer's copies** and **Download all**. Until every out-of-date game has a choice the main button reads **Choose for 2 games** and is disabled. Picking a copy changes the row's note to "an older copy than the server's" or "the newer version".
  - **Also copy:** Emulators is always included and shown with a fixed tick ("PCSX2 for Linux and Windows · 120 MB", or "Already on the drive"). Firmware is a checkbox (systems and file counts), on by default from Settings. It is disabled with the reason when the drive already has the firmware or the server has none. When the pack is for Linux and the selection has Windows games, a third checkbox, **Make Windows games work offline**, is off by default. It adds the Steam Linux Runtime (about 0.3 GB) that umu-run otherwise downloads on its first launch. Installing Proton from the drive puts the runtime where umu expects it, so the first launch needs no network.
  - **Notes:** games already on the chosen drive are skipped ("1 game is already on Couch kit and will be skipped: Moss & Marrow"). On a FAT32 drive, games that download and have a file over 4 GB are listed as not possible; a game copied from this computer is split into parts, so the limit does not apply to it.
  - **Footer:** "11.1 GB to write · 3.2 GB to download" (the second half appears when some games come from this computer; emulators and firmware count as downloads), Cancel, and **Backup 3 games**, which reads "Choose for N games" (disabled) while an out-of-date choice is open, "Not enough space" when the drive is too small, and "Nothing to backup" when everything is already on the drive.
- **Backups run in Downloads.** Each game, emulator and firmware set is a row with the kind badge **Backup**. Its status reads "Downloading from server 45%", then "Writing to SAMSUNG T7 70%". A game copied from this computer reads "Getting details from server" (its details and art still come from the server), then "Copying from this computer to SAMSUNG T7 45%", and the strip ends "· copying from this computer". It finishes as "Copied from this computer · Written to Couch kit". While it copies, Uninstall, Update and Locate… on that game are refused ("This game is being backed up. Wait for it to finish."), and the library folder can't be moved. The sparkline is network-coloured while downloading and disk-coloured while writing. A queued row reads "Waiting · to Couch kit", and each row has Cancel. The strip reads "Backing up Velvet Static to Couch kit · 45% · downloading". A backup's game rows are written first, then its emulators, then its firmware. Each finished row goes to Completed as "Written to Couch kit".
  - **Notifications are one card per backup, not one per game:** "3 games added to the backup queue for Couch kit", then "3 games and 1 emulator backed up to Couch kit".
  - **If the drive is removed mid-backup,** the row being written fails ("Failed — SAMSUNG T7 was removed"). The rest of that drive's queue is cancelled as one Completed row ("13 more items · Cancelled — SAMSUNG T7 was removed · backup again to finish"). One error card says how many items were written. A half-written file is never left as a valid entry: GRID writes to a temporary name and renames it when the file is complete, and removes leftovers the next time the drive is connected.
- **Library › Backup drives:** a rail section under the platforms, with one row per connected, set-up drive (a drive icon, the pack name and its game count; pointing at a row shows the volume label).
  - **The header** shows the pack name, "8 games · 13.3 GB · on SAMSUNG T7" and chips for the drive's emulators and firmware.
  - **The grid** is the drive's games, with platform tags, search, sort and size. A game already installed shows Play. Any other game shows **Install**, which copies the game from the drive (no network). Its Downloads row and the strip read "Copying from SAMSUNG T7".
  - **Installing a game also sets up its system from the drive.** When the game's system has no emulator and the drive has one, GRID installs that emulator from the drive and makes it the system's emulator ("DuckStation from Couch kit added to download queue"). It also installs the drive's firmware for that system when the system needs it ("PlayStation firmware installed from Couch kit"). Windows games bring their Proton version the same way.
  - **Details from a drive** shows "From Couch kit" in the header line, **Install from drive** and **Remove from drive…** (two clicks). A game backed up from a computer's install adds "A copy of an installed game · v1.1", plus "(the server had newer files)" when it was out of date then; a Windows one adds "Its Wine prefix wasn't copied, so saves kept there aren't on the drive". Installing it puts the tree back in this computer's platform folder (a new, empty Wine prefix for a Windows game on Linux) and keeps the copy's version, so an out-of-date copy shows Update once connected.
  - **Select mode on a drive** has Install N games and Remove from drive… instead of Backup.
- **Use without a server.** The Connect screen has **Use without a server** under Connect, after an "or" divider. It needs only the library folder. GRID then opens on Library, on the first connected backup drive.
  - **In the top bar,** the connection dot is grey, the chip reads "Offline · no server", and its menu offers Connect to a server….
  - **Server** shows one panel: "Not connected to a server. Your Library and backup drives work without one" and Connect to a server….
  - **Library, backup drives, Downloads, Settings and play** all work offline. Backing up needs a server, so Backup to drive… is disabled offline, with that reason.
  - **Connecting later** keeps everything that was installed offline. Games installed from a drive keep their server id, so updates, cloud saves and play activity work once the computer is connected to that server.

**The gridpack folder**

```
gridpack/
  gridpack.json                          pack name, OS list, format version, created/updated, the index
  games/<platform>/<file>                a single-file game, exactly what the server gives: zip, 7z, iso, chd …
  games/<platform>/<fs name>/<file> …    a multi-file or native game
  games/<platform>/<name>/installed.tar.001 …  a game copied from a computer's install: an uncompressed tar in parts of at most 4 GiB
  assets/<platform>/<game>/              cover.jpg, fanart.jpg, logo.png, screenshot-01.jpg … (up to 8)
  emulators/<emulator>/<version>/<os>/   the release archive GRID would download, one per computer the pack is for
  firmware/<platform>/<file>             the firmware files as the server serves them
```

`<platform>` is RomM's platform slug (`genesis`, `snes`, `ngc`, `win`), as in the ClasPlatf example drive. `<game>` is the file or folder name without its extension. `<emulator>` is the lowercased profile name and `<os>` is `linux-x86_64` or `windows-x86_64`. Every name is made safe for FAT, exFAT and NTFS, and two games that would share a path get ` [rom <id>]`. The paths in `gridpack.json` are relative to `gridpack/`, so the drive works under any mount point. The shape, as the `Pack` types in `crates/grid-core/src/backup/pack.rs` read and write it:

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
      "source": {
        "server": "https://romm.home.lan", "rom_id": 1234, "igdb_id": 1991,
        "fs_name": "Aladdin (USA).zip", "server_updated_at": "2026-10-01T12:00:00Z",
        "cover_small_path": "…", "cover_large_path": "…",
        "screenshot_urls": ["…"], "fanart_urls": ["…"], "logo_url": "…"
      },
      "name": "Aladdin", "summary": "…", "region": "USA", "languages": "En",
      "tags": "", "revision": "", "rating": "",
      "companies": ["Virgin Games"], "release_date": "1993-11-11",
      "genres": ["Platform"], "game_modes": ["Single player"], "franchises": ["Aladdin"],
      "players": "1", "version": "1.0", "size": 2097152, "sha1": "…",
      "files": [
        { "path": "games/genesis/Aladdin (USA).zip", "size": 2097152, "server_file_id": 5678,
          "name": "Aladdin (USA).zip", "category": "game", "top_level": true, "md5": "…", "sha1": "…" }
      ],
      "assets": {
        "cover": "assets/genesis/Aladdin (USA)/cover.jpg",
        "fanart": "assets/genesis/Aladdin (USA)/fanart.jpg",
        "logo": "assets/genesis/Aladdin (USA)/logo.png",
        "screenshots": ["assets/genesis/Aladdin (USA)/screenshot-01.jpg"]
      },
      "added": "2026-11-07T09:01:00Z"
    },
    {
      "platform": "ps2", "platform_name": "PlayStation 2",
      "source": { "server": "https://romm.home.lan", "rom_id": 2201, "server_updated_at": "2026-09-14T08:00:00Z" },
      "name": "Neon Orchard", "version": "v1.1", "size": 6538067968, "files": [],
      "installed": {
        "archive": "tar", "layout": "extracted", "name": "Neon Orchard (USA)",
        "parts": [
          { "path": "games/ps2/Neon Orchard (USA)/installed.tar.001", "size": 4293918720, "md5": "…" },
          { "path": "games/ps2/Neon Orchard (USA)/installed.tar.002", "size": 2244149248, "md5": "…" }
        ],
        "tree_bytes": 6537412608, "tree_files": 412, "launch": "Neon Orchard (USA).iso",
        "rom_file_name": "Neon Orchard (USA) (v1.1).7z", "server_updated_at": "2026-09-14T08:00:00Z",
        "version": "v1.1", "outdated": true, "from_os": "linux", "row": {}
      },
      "added": "2026-11-09T10:12:00Z"
    }
  ],
  "emulators": [
    { "name": "DuckStation", "kind": "emulator", "source_id": "stenzek/duckstation", "version": "v0.1-9226",
      "os": "linux-x86_64", "platforms": ["psx"],
      "path": "emulators/duckstation/v0.1-9226/linux-x86_64/DuckStation.AppImage",
      "supplementals": [], "asset_name": "DuckStation.AppImage", "size": 40000000 }
  ],
  "firmware": [
    { "platform": "psx", "platform_name": "Sony PlayStation",
      "files": [ { "name": "scph5501.bin", "path": "firmware/psx/scph5501.bin", "size": 524288, "md5": "…" } ] }
  ]
}
```

Required: `format`, `name`, `os`, `created`, `updated`; per game `platform`, `platform_name`, `source` (`server`, `rom_id`), `name`, `size` and `added` (`files` may be empty or missing for a game with an `installed` block); per file `path`, `size`, `server_file_id` and `name` (`top_level` defaults to true); per emulator `name`, `kind`, `source_id`, `version`, `os`, `platforms`, `path`, `asset_name` and `size`; per firmware set `platform`, `platform_name` and `files`, each with `name`, `path` and `size`. Everything else defaults when missing. An asset field with no file is left out (`cover_small` is the small cover when there is no large one).

- `source.server` is the server URL without userinfo. The other `source` fields rebuild the game's details and its image cache keys, so its art shows offline.
- `emulators[].kind` is `emulator` or `compat_tool` (Proton, umu-launcher and the Steam Linux Runtime). `source_id` is a catalog `owner/repo`, `romm:<folder>/<emulator>` for a server build, or `local:<name>` for a manually added emulator zipped from its program folder; a `local:` entry also has `program` (the program's path inside the zip) and `args` (its launch options). `supplementals` are the pack paths of the release's extra files, stored beside the archive.
- `files[].md5` and `sha1` are RomM's hashes, kept for identification. For an archive RomM hashes the decompressed contents, not the archive, so GRID never checks a file's bytes against them; each write is checked by reading it back against the bytes GRID downloaded. `firmware[].files[].md5` is the raw file's MD5 (RomM serves firmware as is), and an install from the drive checks it.
- `os` holds only `linux` and `windows`; any other value makes the file unreadable.
- `installed` is a copy of the game as installed on the computer that backed it up. `archive` is `tar`; `layout` is `extracted`, `native`, `multi` or `file`; `name` is the folder (or file) name in the platform folder. `parts` are in order, each with the MD5 of the bytes GRID wrote, and the game's `size` is their sum. `tree_bytes` is the space a restore needs. `launch` and `row.native_executable` are paths inside the tree; `row` holds only relative paths and plain values (launch parameters, working folder, PS4 ids, included DLC), never an absolute path. `server_updated_at` and `rom_file_name` are the installed copy's, so updates are detected after a restore; `outdated` records that the server had newer files at backup time. An unknown `archive` or `layout` keeps the pack readable, and Install says "This backup needs a newer GRID". The parts rebuild without GRID: `cat installed.tar.* | tar x`. The Wine prefix (`prefix/` in a native game's folder) is left out.
- Keys GRID does not know are kept when it rewrites the file, on the pack and on each game, file, emulator and firmware entry. A `format` greater than 1 is read-only.

Unlike the example drive, `games` is an array (not keyed by "0", "1" …), every game carries its server source, numbers are numbers, and each file is an object (path, size, server file id, name, category) so a drive install plans exactly like a server install. GRID rewrites `gridpack.json` through a temporary file and a rename after each finished item, so a removed drive always leaves a valid index.

**Decided**

1. A game installed on this computer is copied from the library instead of downloaded, when **Copy installed games from this computer** is ticked (the default). An install that is exactly the server's files goes as those files; any other goes as an uncompressed tar in parts of at most 4 GiB on every file system. If the server has newer files, the user picks this computer's copy or the server's version, per game. Missing games, installs outside the library folder, PS3 games in RPCS3's `dev_hdd0`, Xbox 360 games, Emulators-platform packages and trees with links download from the server. The Wine prefix is never copied, so saves kept in it stay on this computer. (This replaces "a backup always downloads the game from the server".)
2. Each pack is for a set of computers (Linux, Windows), picked at setup and changed in Settings. Every emulator on the drive has a build for each one.
3. Firmware is a per-backup checkbox, on by default, with no warning. Players supply firmware from their own consoles.
4. One pack per drive. A game on several drives is listed under each drive, and Install copies it from the drive being viewed.
5. A drive is identified by its mount path or drive letter; packs have no id.
6. A manually added emulator (no download source) is copied as a zip of its program folder, for this computer's OS only.
7. A downloaded game waits in a staging folder in the library between downloading and writing. A game copied from this computer is written straight to the drive (an archive as big as the install would need that much space again in staging); each part is checked by reading it back.
8. A pack that carries Proton also carries umu-launcher. The Steam Linux Runtime is an optional checkbox, off by default ("Make Windows games work offline").

The prototype's drives, sizes and dates are made up. Of its installed games, Lantern Vale and Neon Orchard are out of date, Ironclad Drift is installed outside the library folder, Brass Comet is Missing, and Harbor Lights is a Windows game (the Wine-prefix note); Library › All games › Select › Select all 9 › Backup to drive… shows every case. "Prototype: unplug / plug in" buttons on each drive card simulate removing and reconnecting the drive.
