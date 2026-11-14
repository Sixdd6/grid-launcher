A proposal: GOG (with GOG offline installers), Epic Games, Amazon Games and Steam in GRID's Library, beside the RomM games. The card is built on Game backups (what main has) and opens on Library › GOG › Installed, with a GOG install running, one queued and a Steam install that Steam runs. Everything not listed here matches Game backups.

**Status: proposed, not built.** The decisions it follows are in `docs/external-stores-plan.md` (2026-11-13). TV mode's version is the **TVStores** card.

## Try it

- **Library rail:** under Backup drives, one group per store in alphabetical order (Amazon Games, Epic Games, GOG, Steam). Click **Installed** or **Available** under a store.
- **States:** every store view has a dashed **Prototype** select in its header (the same select sits in the corner of each store card in Settings › Stores). It switches that store to: loading owned games, nothing installed, everything installed, store tool not installed, store tool install failed, login expired, not connected; Steam adds no Web API key, game details private, key rejected, Steam not installed.
- **Amazon Games** starts with an expired login: its Available row shows "!", its Available view asks to sign in again.
- **Details:** click any store cover. Lanternfall (GOG) has an update and a cloud save conflict; Ashen Tide has DLC; Brightwater came from an offline installer; Iron Orchard and Northern Static (Epic) are Not supported; Quiet Comet (Steam) is installing in Steam.
- **Install dialog:** Available › Install on a GOG game (Marrowind Keep has languages and DLC). Its dashed button shows the not-enough-space state.
- **Login window:** Settings › Stores › Amazon Games › Sign in again…, or Connect… on a store set to Not connected. The dashed buttons finish the sign-in or fail it (first a network error inside the window, then a failed sign-in).
- **Offline installer:** GOG › Install from installer… (store header, or the GOG card in Settings › Stores). The stand-in file chooser lists one file for each case.
- **Settings › Stores**, **Downloads** and the notification cards show every store event; the Steam card's dashed button picks what the next Test returns.
- **GOG backups (2026-11-13):** GOG › Installed › **Select** (or **Backup all 3…**, or Details › Backup to drive…), pick Ashen Tide (installed copy) and Brightwater (installer files), then Backup to drive…. The dialog's dashed button moves Brightwater's installer away. Library › Couch kit shows two GOG games after its RomM games (Marrowind Keep as installer files, Paper Lanterns as an installed copy); Install restores them. Lanternfall while playing and Copper Saints while installing show the refusals.
- **RomM games with a GOG installer in their archive (2026-11-13):** Server › Windows: Install **Grey Meridian** (runs silently), **Foxglove** (falls back to its setup window), **Ashgrove** (DLC for the installed Harbor Lights), **Iron Lullaby** (unrecognised installer) and **Low Tide Radio** (DLC for Pale Summit, which isn't installed); Server › Linux: **Paper Orbit** (.sh). Follow them in Downloads.

## Library

- **Rail group per store.** Like Backup drives: the store's name is the group heading (with its mark), then two rows, **Installed** and **Available**, each with a count. A store shows when it is connected, when its login expired, or when it still has games installed here after a Disconnect (GOG also after an offline install with no login).
  - The heading carries a small badge when the store needs attention: **!** (login expired, store tool missing or failed, Steam without a key or client) or **off** (not connected, installed games only).
  - Available's count is a spinner while the owned list loads, and **!** while it can't load.
- **Header:** "GOG · Installed" (or Available), then "3 games · 67.0 GB" (Installed) or "5 games you own and haven't installed · updated 5 min ago" (Available). Chips: "Signed in as sam_plays" (opens Settings › Stores), "Cloud saves: on" (GOG, Epic), "Login expired · Sign in again", "gogdl not installed". Actions on the right: **Install from installer…** (GOG only) and **Refresh** (Available). Search filters by title. **Select** (beside Size) and **Backup all N…** are offered in GOG › Installed only (see GOG games on backup drives); other store views have no Select.
- **All games, Recent and Updates** include installed store games, each with its store's tag (mark and name) in the corner. Platform rows don't: store games live in their store group. Updates counts store updates.
- **Store covers:** the installed dot; tags **UPDATE**, **SAVE CONFLICT** (red), **NOT SUPPORTED**; while installing a percentage tag ("38%", "QUEUED") and a progress bar along the cover's bottom edge (teal for a Steam install). Hover action: **Play**, **Install** (opens the install dialog), **Install in Steam**, **Installing… / Queued / Updating… / Verifying…** (opens Details), **Installing in Steam** (disabled, "Managed by Steam"), **Details** (Not supported, or a store that isn't signed in). The cloud button (GOG, Epic, installed) opens Details on Cloud saves.
- **States in place of the grid** (a panel with the store's mark, a title, one line and its action):
  - Loading: "Loading your Epic Games games…" over skeleton covers.
  - Nothing installed: "No GOG games installed yet" · Open Available / Install from installer… (GOG). Steam: "Games you install in Steam itself show here too."
  - Everything installed: "Every GOG game you own is installed" · Refresh.
  - Login expired (Available): "Your Amazon Games login expired" · Sign in again…. Installed keeps working.
  - Not connected (Available): "GOG isn't connected" · Connect GOG….
  - Store tool missing: "Epic Games needs legendary … It's 14 MB." · Install legendary (14 MB); installing: the button reads "Installing legendary…" (disabled); failed: "Couldn't install legendary: the download didn't match its checksum, so GRID deleted it." · Try again.
  - Steam, no key: "Steam needs your Web API key to list the games you haven't installed" · Add your key in Settings…; key rejected (403); game details private (with the Steam privacy path) · Refresh / Open Settings › Stores; Steam not installed (Installed): "Steam isn't installed" · Check again.

## Install dialog (GOG, Epic, Amazon)

- Header: cover, "Install Marrowind Keep", store mark · "22.0 GB download".
- **Version** (GOG, when both exist): Linux "Runs natively on this computer" (Recommended, default) or Windows "Runs through GE-Proton10-17". Epic and Amazon are Windows only and skip this.
- **Options:** Compatibility tool (Windows version; the Settings › Systems default; GRID makes a Wine prefix for the game), Language (when the store offers several), Install folder with Change… (the store's default from Settings › Stores).
- **DLC you own:** a checkbox per DLC with its size, ticked by default.
- Space line: "34.0 GB on disk · 412 GB free on /home", or in red "Needs 34.0 GB · 18.4 GB free on /home", which turns the button into **Not enough space** (disabled). Footer: "24.7 GB to download", Cancel, **Install**.
- Install queues the game ("Marrowind Keep added to download queue" · Open Downloads). Signed out or missing its tool, Install doesn't open the dialog; an error card says why with Sign in again / Install gogdl.

## Details for a store game

- No RomM sections: no server saves list, no "not linked to a server game", no Achievements, no Related row. An installed GOG game has **Backup to drive…** and "On Couch kit" when a connected drive holds it; an Epic, Amazon or Steam game says why it can't be backed up ("Epic Games games can't be backed up: legendary installs only from Epic's servers." / "…nile installs only from Amazon's servers." / "Steam games can't be backed up here. Steam has its own Backup and restore.").
- **Left column buttons:** Play (Stop while it runs) · Install… / Install in Steam · Cancel install · 45% (Cancel update / Cancel verify) · Not supported (disabled) · **Update to 2.2** · **Game Settings** (not Steam) · **Verify and repair** (Verify in Steam) · **Uninstall** (two clicks: "Confirm uninstall"; Steam: "Uninstall in Steam…", which opens Steam's own dialog).
- **Lines under them:** last played; "Runs natively · Linux version" / "Runs through GE-Proton10-17 · Windows version" / "Launches through Steam"; cloud state ("Cloud saves synced 2 hours ago", "Cloud save conflict · Resolve", "Cloud saves off for this game", "This game has no cloud saves"); "Installed from an offline installer"; "Installed in Steam, outside GRID"; for a store that isn't signed in "Your GOG login expired. Sign in again"; for Not supported "Needs the EA app. GRID can't install games that start through another company's launcher; install it from the Epic Games Launcher instead."
- **Steam install in progress:** a box "Installing in Steam · 52%" with its bar and "Managed by Steam. Pause or cancel it in Steam.", and a disabled Cancel.
- **Header:** title; "GOG · 2023 · Developer · Genre"; chips: store (mark + name), Playing, v2.1, Linux/Windows version, Offline installer, Not supported.
- **Tabs:** Overview (the store's description; DLC you own with Install / Remove per DLC; facts: Store, Developer, Released, Genres, Size, Version "v2.1 · GOG v2.2", Folder), Media (the store's screenshots), **Cloud saves** (GOG and Epic, installed only).
- **Cloud saves tab:** status ("Synced 2 hours ago · GRID syncs with GOG before the game starts and after it closes" · Sync now / Syncing…); the conflict: "These saves changed in two places" with two sides, This computer (newer, "Changed today, 21:40 · 3 files · 2.1 MB", **Keep this computer's save**, "Uploads it to GOG") and GOG cloud ("Changed today, 19:12 on another computer", **Use the cloud save**, "Replaces the save on this computer"); off and none states; Save locations (the path the store reports; a Windows game's is inside its prefix); "RomM cloud saves are off for store games."
- **Launch:** with cloud saves on, "Syncing cloud saves for Ashen Tide…" then "Launching Ashen Tide…"; when the game closes, "Cloud saves uploaded to GOG for Ashen Tide". A conflict opens **"Which save should Lanternfall use?"** before the game starts, with the two sides and **Don't launch**. Steam: "Starting Moonlit Ferry through Steam…"; the Playing chip shows while Steam runs it, and Stop reads "Running in Steam" (Steam games are quit from the game).
- **Game Settings** (a dialog): Install folder (read-only), Executable ("Set by GOG. Picking another one makes it your choice; GOG updates won't change it."), Launch parameters, Compatibility tool and Wine prefix (read-only) for a Windows version, or "Linux games run directly.", and "Sync this game's cloud saves with GOG". Save / Cancel.

## Settings › Stores

A pane in the alphabetical rail (… RetroAchievements, **Stores**, Systems, Updates). A lede, then one card per store in the rail's order.

- **GOG, Epic Games, Amazon Games card:** mark, name, "Signed in as sam_plays", a status pill (Connected / Login expired / Not connected).
  - Connected: Account; Games you own ("8 · 3 installed · updated 5 min ago", a spinner while loading); Store tool ("gogdl 1.1.2 · installed", "Installing gogdl…" with a bar, "Not installed · 12 MB" · Install, or the checksum error · Try again); License ("heroic-gogdl · GPL-3.0 · github.com/…" link); Install folder · Change…; Cloud saves (GOG, Epic) "Sync before a game starts and after it closes". Actions: Show in Library, Install from installer… (GOG), Refresh games, **Disconnect…**.
  - Login expired: a warning line, and **Sign in again…** in place of Show in Library.
  - Not connected: what connecting does and which tool it downloads (name, licence, size), **Connect GOG…**, Install from installer… (GOG works without a login), the tool row ("Installs when you connect · 12 MB") and the licence. A failed sign-in leaves its error under the lede.
  - **Disconnect** asks inline: "Disconnect GOG? GRID deletes your GOG login and gogdl from this computer. The 3 installed games stay and still play; they can't install, update or sync cloud saves until you connect again." · Disconnect / Cancel.
- **Steam card:** "SteamID64 76561198000000001 · SamPlays", pill Connected / Installed games only / Not connected.
  - Lede: installed games show as soon as Steam is set up; Available needs the SteamID64 and the user's own Web API key.
  - Steam client: "Steam client found · /usr/bin/steam", or "Steam isn't installed…" · Check again.
  - **Account:** radios for the accounts found in Steam's files ("SamPlays 7656… signed in to Steam on this computer", "couchguest … has signed in on this computer before"), and **Enter a SteamID64** with a field.
  - **Web API key:** saved, a masked field ending in its last four characters with Change… / Remove; not saved, a password field. Help: where to make one (any domain works), kept in the OS keyring, sent only to Steam.
  - **Test** and **Save** (Connect Steam when not connected). Results under the key: "The key works · 7 games owned"; "Steam rejected the key (403 Forbidden)…"; "The key works, but this profile's game details are private…" with "In Steam: your profile › Edit Profile › Privacy Settings › set Game details to Public."; "That isn't a SteamID64. It has 17 digits and starts with 7656."; "Couldn't reach api.steampowered.com…"; "Asking Steam…" while it waits.
  - Disconnect asks inline: GRID forgets the SteamID64 and deletes the key from the keyring; Steam games leave the Library and stay installed in Steam.

## Login window

- A separate window GRID opens (not a dialog in the main window): title bar "Sign in to GOG — GRID" with the store mark and ✕; a read-only address line with a lock ("https://login.gog.com/auth"); the store's own page inside (the card shows a hatched placeholder, never an imitation of the store's page); a footer "GRID never sees your password. This window closes on its own when you've signed in." and Cancel.
- Loading: "Loading login.gog.com…". Success: "Signed in. Closing…", the window closes, then "Connected to GOG as sam_plays" (or "Signed in to Amazon Games as …" after an expired login). If the store's tool isn't installed it installs next ("Installing gogdl (12 MB) for GOG…", a Downloads row, "gogdl installed"), then the owned list loads ("GOG: 8 games found · 5 to install").
- Network error: a warning strip inside the window ("Couldn't reach login.gog.com. Check your connection; the page reloads when you try again."). A failed sign-in closes the window, leaves the error on the store's card and shows an error card with **Try again**. Cancel, ✕ or Esc closes it silently; nothing changes.

## Offline GOG installer

- **Entry points:** Install from installer… in the GOG store header (Installed and Available), in the empty Installed state and on the GOG card in Settings › Stores (connected or not).
- **File chooser:** the system's own (the card shows a stand-in listing ~/Downloads). Then "Reading the installer… Looking for the game's goggame info in …".
- **Recognised:** the game's cover and name, "Linux version · v1.1.0 · 1.1 GB" or "Windows version", "From a GOG offline installer · <file>", and how it installs: Linux "GRID unpacks the game files from it. The installer script isn't run."; Windows "GRID runs the installer silently under GE-Proton10-17. If it can't run silently, GRID tries once more with gentler settings, then opens its own setup window: finish it there and GRID adds the game when it closes." (with the compatibility tool picker). Install folder, "It joins your GOG games. Updates and cloud saves come from GOG while you're signed in." (or, signed out, "Sign in to GOG later…"), space, **Install**.
- **A newer installer of an installed game:** "Ashen Tide is installed already (v1.0.4). Installing this replaces its files with v1.0.5. Saves and settings stay." · **Update**.
- **DLC installer:** "Install DLC for Ashen Tide", "Digital Artbook · DLC for Ashen Tide · 118 MB", "Adds to the installed Ashen Tide (the Linux version)…" · **Install DLC**.
- **Refusals** ("Can't install this file", with Choose another file… and OK): macOS .pkg; not a GOG installer (no goggame info); DLC whose base game isn't installed ("This is DLC for Tidewater Tales, which isn't installed. Install Tidewater Tales first, from GOG or from its own installer."); a missing .bin part ("This installer comes in 6 parts. …-1.bin isn't in the same folder."); the other OS's installer of a game installed here ("Ashen Tide is installed here as the Linux version. … uninstall Ashen Tide first, then open this installer again.").
- **Progress:** a Downloads row (kind **Installer**): "Unpacking the game files 45%" (Linux), "Running the installer silently · 30%" (Windows), and on fallback "Waiting for the installer's setup window · finish it there" with the card "The Quarry Nine installer couldn't run silently, so its setup window opened. Finish it there; GRID adds the game when it closes." Done: "Quarry Nine installed from its installer"; Details then says "Installed from an offline installer" and shows the Offline installer chip.

## Downloads and notifications

- **Store rows** share Downloads with RomM rows: the title, a store badge (mark and name), the kind badge (none for a game; Update, Repair, Content for DLC, Store tool, Installer), the status line, bar, sparkline and Cancel. Lines: "Downloading 45% · 41.2 MB/s", "Installing 90%", "Verifying files 45%", "Downloading 40% · checking its SHA-256" (a store tool), the installer lines above, "Waiting for a slot". Steam rows: "Installing in Steam 52% · Managed by Steam" with a disabled Cancel. Completed: "Installed · 26.0 GB", "Updated to 2.2", "Verified · 2 files repaired", "Installed by Steam", "Installed from its installer", "Cancelled", and failed rows in red with Retry and Dismiss ("Failed · legendary stopped: Epic's download server didn't answer"). The strip names the store: "Copper Saints · GOG · Downloading 63% · 41.2 MB/s".
- **Store installs share the one download slot** with RomM installs (one at a time, first in, first out). A store tool download starts at once. Steam installs run beside the queue (Steam runs them).
- **Notification cards:** connected / signed in; "GOG: 8 games found · 5 to install"; tool installing / installed; added to download queue (· Open Downloads); installed (· Play); updated; verify result; update available ("Lanternfall: update 2.2 is available on GOG" · Show, which opens Library › Updates); syncing / uploaded / synced cloud saves; kept or restored a conflicting save; uninstalled ("Its cloud saves stay in your GOG account."); disconnected; error cards for a failed sign-in (· Try again), signed-out install (· Sign in again), missing tool (· Install gogdl), Steam not installed, Steam private profile on Refresh.

## RomM games with a GOG installer in their archive (2026-11-13)

The user can zip a GOG offline installer (`setup_*.exe` with its `.bin` parts, or a Linux `.sh`) and upload it to RomM as a Windows or Linux game. GRID never uploads. When GRID downloads such a game it finds the installer in the archive and runs the offline-installer flow. **The game stays a RomM game**: a normal Windows/Linux platform row with RomM cloud saves, play activity, update checks and drive backups. Nothing about it is a GOG store game.

- **Details before install:** a line under the buttons, "Its archive holds a GOG Windows installer. GRID runs it after the download, under GE-Proton10-17." (Linux: "…a GOG Linux installer. GRID runs it after the download."; DLC: "…a GOG DLC installer for Harbor Lights."). After install: "Installed by running its GOG installer" ("Added to Harbor Lights by its GOG installer").
- **Covers and Details while it runs:** "Installing…" / "Queued", the percentage tag and the bottom bar (as on store covers).
- **Downloads row** (no new kind badge; it is a game install): "Windows · Downloading 45% · 38.2 MB/s", then a second phase, "Windows · Running the GOG installer silently under GE-Proton10-17 · 40%" (Linux: "Unpacking the GOG installer 40%": the .sh is read as a zip and its `data/noarch/` copied; the script never runs). On a Windows host the .exe runs natively and the line drops "under …".
- **Wizard fallback:** after the silent run and a milder silent run both fail (2026-11-14), "Waiting for the installer's setup window · finish it there" on the row and the strip, and the card "Foxglove's installer couldn't run silently, so its setup window opened. Finish it there; GRID adds the game when it closes." Desktop only: TV mode runs Windows installers silently only and never opens the setup window (user decision 2026-11-13; when both silent runs fail there, the row and the card give the reason with Retry and Delete installer, see the TVStores card).
- **Done:** "Windows · Installed · ran its GOG installer" ("Added to Harbor Lights · ran its GOG installer" for DLC); card "Grey Meridian installed" · Play.
- **Failures** (the row goes to Completed in red with Retry and Dismiss; the download is removed):
  - Unrecognised installer: "Failed · Its archive holds an installer GRID doesn't recognise (setup.exe has no GOG game info). The download was removed." Card "Couldn't install Iron Lullaby: its archive holds an installer GRID doesn't recognise" · Open Downloads.
  - DLC whose base game isn't installed: "Failed · Its archive holds a DLC installer for Pale Summit, which isn't installed. Install Pale Summit first; the download was removed." Card "Couldn't install Low Tide Radio: it's DLC for Pale Summit, which isn't installed" · **Install Pale Summit**.
  - An installer that exits with an error (not a separate prototype game): "Failed · The installer stopped with an error (exit code 1). The download was removed.", with the same card.

## GOG games on backup drives (2026-11-13)

This replaces "store games are not backed up" for GOG. GOG games that are not from RomM (installed with gogdl or from an offline installer) can go on a backup drive. Epic, Amazon and Steam games still can't, and their Details say why.

- **Starting a backup:** Details › **Backup to drive…**; **Select** in GOG › Installed (covers pick with the check and ring; the bar reads "2 selected · 24.5 GB to write", Select all N, Clear, **Backup to drive…**; Esc or Done leaves); **Backup all N…** in the GOG › Installed header.
- **The dialog** is Game backups' dialog: title, lede ("GRID writes each GOG game to the drive's gridpack folder: its installer files when GRID has them, else a copy of the installed game. Saves always go with it. Nothing downloads, and no GOG login is needed."), **Drive** (radio rows with space and the hatched share; "Needs … · … free" in red), and **What goes on the drive**, a row per game with its tag:
  - **Installer files**: "brightwater_1_3_58810.sh · 2.9 GB, and its saves" (a game GRID installed from an installer that is still where it was).
  - **Installed copy**: "Installed with gogdl, which keeps no installer, so the installed files are copied · 21.6 GB, and its saves", or "Its installer isn't at ~/Downloads/… any more, so the installed files are copied". A Windows game adds "Windows games are copied without their Wine prefix. The save folders GOG reports are copied from it."
  - The note "Covers and the store's details go too. GRID adds no emulator or Proton for GOG games; the computer that installs a Windows game uses its own compatibility tool."
  - Notes: already on the drive (skipped); refusals in the warning colour: "Copper Saints is being installed or updated. Wait for it to finish.", "Lanternfall is running right now; close it to copy it."
  - Footer: "24.5 GB to write · nothing to download", Cancel, **Backup N games** / Not enough space / Nothing to backup.
- **Downloads:** each game is a **Backup** row, "GOG · Installer files and saves · Reading its GOG details", then "Copying from this computer to SAMSUNG T7 45%"; done "… · Copied from this computer · Written to Couch kit". One card per backup: "2 games added to the backup queue for Couch kit", then "2 games backed up to Couch kit".
- **Library › a drive:** its GOG games follow its RomM games, each with a **GOG** tag; the header counts both ("9 games · 51.4 GB · on SAMSUNG T7"), a chip reads "GOG: 2 games", and the rail count includes them. Hover action: Install (Play when installed here).
- **Details from a drive:** "From Couch kit" leads the header line; **Install from drive** and **Remove from drive…** (two clicks); a line says how it restores: "On the drive as its installer files · v1.2 and saves. Installing runs the installer again." or "On the drive as a copy of an installed game · v1.1.0, with its saves. Installing copies it back."
- **Restoring:** from installer files, "Copying the installer from SAMSUNG T7 60%", then "Running the installer · 40%", done "Installed from SAMSUNG T7 · ran its installer · saves restored". From an installed copy, "Copying from SAMSUNG T7 45%", done "Copied from SAMSUNG T7 · saves restored". No network or GOG login is needed. The game then appears in GOG › Installed (an installer restore also says "Installed from an offline installer"); signed in to GOG, it gets updates and cloud saves as usual.


## Inventory checklist

Screens
- [ ] Library rail: four store groups, heading with mark, Installed / Available with counts
- [ ] Rail badges: ! (attention), off (not connected), spinner count, ! count
- [ ] Store view header: title, counts line (Installed / Available), chips, Install from installer…, Refresh
- [ ] Store covers in a store view, and with store tags in All games / Recent / Updates
- [ ] Install dialog (GOG with versions, languages, DLC; Epic / Amazon Windows only)
- [ ] Details for an installed, available, updating, Not supported, offline-installed, Steam, Steam-installing game
- [ ] Details › Cloud saves tab (synced, syncing, conflict, off, none)
- [ ] Conflict prompt before launch
- [ ] Game Settings dialog (Linux version, Windows version)
- [ ] Settings › Stores pane: GOG/Epic/Amazon card (connected, expired, not connected, disconnect confirm) and Steam card
- [ ] Login window (loading, page, network error, closing)
- [ ] Offline installer: file chooser, reading, recognised (Linux, Windows, update, DLC), five refusals
- [ ] Downloads rows for every store job kind, active, queued and completed
- [ ] Strip line for a store job

States
- [ ] Owned list loading (rail spinner, skeleton grid, card spinner)
- [ ] Nothing installed / everything installed
- [ ] Login expired (rail !, chip, Available panel, Settings card, Details line, Install disabled)
- [ ] Not connected with installed games left (rail off, Available panel, Settings card)
- [ ] Store tool missing / installing / failed (Available panel, chip, Settings row)
- [ ] Steam: no key, key rejected, game details private, client not installed, connected with private profile
- [ ] Install queued, installing (cover tag and bar), update, verify, DLC install
- [ ] Steam install managed by Steam (cover, Details box, Downloads row, disabled Cancel)
- [ ] Not supported (cover tag, faded art, disabled Install with reason)
- [ ] Not enough space (dialog)
- [ ] Playing a store game; Steam "Running in Steam"

Prompts
- [ ] Uninstall (two clicks); Steam uninstall opens Steam
- [ ] Disconnect store (inline confirm); Disconnect Steam
- [ ] Cloud conflict (Details and before launch)
- [ ] Installer refusals with Choose another file…

Notifications
- [ ] Every card listed under Downloads and notifications, with its action button where it has one

RomM installer archives (2026-11-13)
- [ ] Details line before and after install (Windows, Linux, DLC)
- [ ] Cover and Details progress while it runs
- [ ] Downloads row: download phase, installer phase (.exe silent, .sh unpack), wizard wait, done, DLC done
- [ ] Wizard-fallback card
- [ ] Failures: unrecognised installer, DLC without its base (Install <base> action), installer error exit
- [ ] The installed game is a plain RomM game (cloud saves, play activity, updates, Backup to drive)

GOG backups (2026-11-13)
- [ ] Select mode in GOG › Installed (button, picks, bar, Esc/Done)
- [ ] Backup all N… in the GOG › Installed header; Details › Backup to drive…
- [ ] Backup dialog: drives, Installer files / Installed copy rows, installer moved, Windows prefix note, skipped, refusals (installing, running), not enough space, no drive
- [ ] Downloads Backup rows and the two cards
- [ ] Drive grid: GOG covers with GOG tag, header count, GOG chip, rail count
- [ ] Details from a drive: header, Install from drive, Remove from drive… (two clicks), restore line
- [ ] Restore: installer re-run rows, copy rows, done card; game lands in GOG › Installed
- [ ] Epic, Amazon, Steam Details: why they can't be backed up

Edge cases
- [ ] Steam games installed outside GRID show in Installed ("Installed in Steam, outside GRID"); Proton and Steam runtimes never show
- [ ] Offline GOG install with no GOG login: GOG group shows, Available asks to connect
- [ ] Windows installer: silent, then a milder silent run, then its own wizard (desktop only; in TV mode it fails with the reason from its log, Retry and Delete installer, and keeps the installer)
- [ ] Multi-part installer with a part missing
- [ ] DLC installer for a base game that isn't installed
- [ ] Installer for the other OS version of an installed game
- [ ] A store tool download fails its SHA-256 check
- [ ] Sign-in cancelled (silent), network error (in window), sign-in failed (card + error on Settings)

## Decisions made by the designer — confirm with the user

1. **Two new cards**, GameStores (desktop, built on Game backups) and TVStores (built on TV mode), so Game backups and TV mode keep showing main.
2. **Rail order** of store groups is alphabetical by store name (Amazon Games, Epic Games, GOG, Steam), like platforms and Settings, and the groups sit under Backup drives.
3. **Store marks** are neutral monogram tiles in a store colour, not the stores' logos (their brand rules would need checking).
4. **Installed store games also appear in All games, Recent and Updates** (with a store tag), but not under the platform rows.
5. **A store stays in the rail after Disconnect while it has games installed here** (badge "off"; Available asks to connect). With nothing installed it leaves the rail. Steam is the exception: disconnecting removes its group, because Steam owns those installs.
6. **Install from installer…** lives in the GOG group header (Installed and Available), the empty GOG Installed state and the GOG card in Settings › Stores; it works without a GOG login.
7. **An installer for the other OS version** of a game installed here is refused (uninstall first) rather than replacing the install. A Linux game takes Linux DLC installers only.
8. **Progress on a store cover** is a percentage tag plus a bar along the cover's bottom edge (teal for Steam). RomM covers don't have the bar today; the same bar could go on them.
9. **Store installs share the single download slot** with RomM installs; store tool downloads skip the queue; Steam installs run beside it.
10. **The GOG install dialog defaults to the Linux version** when there is one ("Recommended").
11. **Steam Uninstall and Verify** open Steam's own dialogs; GRID shows no confirm of its own.
12. **Steam account picker** lists every account found in Steam's files, with the one signed in to Steam ticked, plus manual entry.
13. **Login window** shows a read-only address line with a lock so the user can check the store's domain; Cancel is silent, a network error stays inside the window, a failed sign-in closes it with an error card.
14. **Cloud conflict** is decided per game with two choices (keep this computer's, use the cloud's) and a third, Decide later / Don't launch; a launch never proceeds over a conflict without a choice.
15. **Per-game cloud saves switch** in Game Settings, plus a per-store switch in Settings › Stores.
16. **CHANGED 2026-11-13 (user decision):** was "Select mode is hidden in store views (store games can't be backed up)". Now GOG games can be backed up, so Select and Backup all N… are in GOG › Installed; Epic, Amazon and Steam views still have no Select.

### New with the 2026-11-13 decisions

17. **A RomM installer archive needs no new kind badge** in Downloads: it is a game install with a second phase, "Running the GOG installer…" / "Unpacking the GOG installer…", after the download.
18. **A failed installer removes the download** and offers Retry; a DLC whose base isn't installed offers **Install <base>** on its error card.
19. **GOG backups are automatic per game:** installer files when GRID still finds the installer it installed from, else the installed copy; there is no per-game choice in the dialog. Saves go with both.
20. **GRID keeps the installer's path** for games installed from an offline installer (it doesn't copy the installer into the library), so a moved or deleted installer means an installed-copy backup.
21. **Restoring a GOG backup needs no GOG login**; the game joins GOG › Installed and gets updates and cloud saves once signed in.
22. **GOG games on a drive** follow its RomM games in the drive's grid with a GOG tag, and count in the rail and header. In this prototype they install one at a time; in the app, select mode's Install N games includes them.
23. **GOG backup refusals:** a game being installed or updated, and a game that is running. A Windows game's Wine prefix is not copied, only the save folders GOG reports.
24. **CHANGED 2026-11-13 (user decision), TV only:** the earlier TV notice that a setup window opens over TV mode is gone; TV mode is silent-only (TVStores #3, #7). The desktop keeps silent-first with the wizard fallback.
