A proposal: RomM becomes one optional storefront beside Amazon Games, Epic Games, GOG and Steam. The card is built on Game stores (what main has). It opens where a connected user lands with Open GRID on › Automatic: **Library › RomM › Discover**. Everything not listed here matches Game stores.

**Status: signed off, not built (step D1; user decisions 14–17).** The Emulators row and Open config folder (see Changes 2026-11-15) and the folding, remembered rail groups (Changes 2026-11-16) are signed off and built (5a62b77). The decisions and contracts are in `docs/romm-storefront-plan.md` (user decisions 1–13, Step 0). TV mode's version is the **TVRommStorefront** card (step D2). Source: `docs/design/prototype/romm.js` and `romm.css`, built by `applayout.py` on top of the GameStores card.

## Changes 2026-11-17

- **Discover and Open GRID on are removed until the Discover rework (user decision 2026-11-17).** The RomM group has no **Discover** row (connected: Installed, then the platform rows; connecting at startup: Installed only, with the spinner flag); a stale `romm:discover` selection falls back to RomM › Installed, or All games. RomM › Installed's empty state has no **Open Discover** button and reads "Install games from a platform under it.". **Settings › Appearance › Open GRID on** is hidden; GRID always opens on Library › All games (`[ui] open_on` is still stored and ignored). The sections below that describe Discover and Open GRID on are kept for the rework and do not apply now. The prototype no longer shows the Discover row, the Open Discover button or the Open GRID on row; Discover's own prototype code (`discover.js` / `discover.css`) stays.
- **Rail scrollbar lane and wrapping names (user, 2026-11-17):** the desktop rail's scrollbar has its own 8px lane (styled, so WebKitGTK no longer overlays it on the rows; `scrollbar-gutter: stable` keeps the lane while nothing scrolls), and the rail's right padding drops to 2px beside it. Row labels wrap onto more lines (1.3 line height) instead of an ellipsis, as in TV mode, so a long RomM platform name such as "TurboGrafx-16 / PC Engine CD-ROM²" is shown whole. Built in `RailPane.svelte`.

## Changes 2026-11-16

- **The Library rail's groups fold, and GRID remembers them** (user request 2026-11-16: "add the collapsing storefront rows, separate the different storefronts from each other visually, and add memory so the collapsed/expanded storefronts persist their state across screens and restarts in desktop and tv mode"). Backup drives, Amazon Games, Epic Games, GOG, RomM and Steam each open and close from their heading. Closed or open is **one remembered list shared with TV mode** (user decision 2026-11-16: closing GOG in either mode closes it in both). Section **Library rail › Folding groups** below.
- **Each group sits in its own inset panel** in the rail, so one storefront never runs into the next. Headings are now sentence case at 12px (the TV rail's polish), not 10px capitals.
- Source: `docs/design/prototype/railfold.js` / `railfold.css` (loaded after `romm_emu.*`). The TVRommStorefront card has the TV side (`tvrailfold.*`).

## Changes 2026-11-15

- **RomM › Emulators:** the server's Emulators platform is now a row in the RomM group like any other platform (user decision 2026-11-15: "the Emulators platform should be there too if the server has Emulators"). Its view lists the emulator files with Install / Installed / Update / Uninstall. Emulator files still never join RomM › Installed, All games, Recent, Updates or Discover. Section **RomM › Emulators** below.
- **Open config folder** (left with Settings › Connection) is back, on **Settings › Library** under the library folder. Section **Settings › Library** below.

## Try it

- **RomM state:** a dashed **Prototype** select sits on Library › RomM › Installed (header), on the RomM card in Settings › Game stores (corner) and in Settings › Systems (under the heading). It switches RomM between: Connected, Connecting (at startup), Can't reach RomM, Sign-in expired, Not connected (games installed), Never connected (nothing installed). Every screen follows it.
- **Welcome:** Settings › Game stores › **Prototype: open the welcome**, or **Prototype: welcome after a Python import**. **Prototype: start again** reloads the card.
- **Connect from the card:** set **Never connected**, then RomM card › **Connect RomM…**. The dialog's dashed button picks the next result (works, can't reach the server, rejected (401), another user's token). A successful first connect opens **Choose your systems**; Skip or Install returns to Settings › Game stores.
- **Locate files:** Settings › Systems › PlayStation (open the row) › **Locate files…**. Tick files, or **Use the whole folder**. Nintendo Switch shows "not verified" keys; PlayStation 3 shows the PUP picker; PlayStation 2 shows the "install an emulator first" state.
- **Backups without RomM:** set **Not connected**, then Library › RomM › Installed › **Backup all 11…**.
- **Emulators:** Library › RomM › **Emulators** (first platform row). Install Pico-8 or Ymir (progress, then Installed), Update PCSX2, Uninstall… RetroArch (inline confirm), Install DuckStation (fails: wrong processor; Retry then works), Install an Other file, open **N other builds…**. The dashed **Prototype** select in the header switches: files loaded, Loading, Can't load the list, Nothing for this computer, No Emulators platform (row hidden; selection falls back to RomM › Installed).
- **Open config folder:** Settings › Library.
- **Folding groups (2026-11-16):** click a group heading in the Library rail, or Tab to it and press Enter or Space. Close **RomM** while Discover shows: the heading takes the selected fill. Reload the card: the groups stay as you left them. Open the TVRommStorefront card in the same browser: it shows the same groups closed (both cards read localStorage `grid.railClosed`; clear it to start fresh).
- **Details without RomM:** set **Sign-in expired**, open Lantern Vale (it has an update) and its Saves and Achievements tabs. Hidden Kiln (PlayStation) is an installed game with no RomM link; Sky Lantern (Super Nintendo) came from another RomM server.

## Removed

- The top bar's **Server** pill, the **server chip** (user@host) with its status dot and **server menu** (Disconnect, Open RomM in browser). The views are **Library, Downloads, Settings**; **Ctrl+1..3**.
- The **Server view**. Its Discover and platform screens are rows in the Library's RomM group.
- The **offline mode** ("Offline · no server" chip, the Server view's "Not connected to a server" panel `bk-srv-off`, "Use without a server" on Connect).
- The **Connect screen** (replaced by the welcome) and **Settings › Connection** (replaced by the RomM card).
- The Library's **PLATFORMS** section (platform rows live only in the RomM group, and only while connected).
- **Choose your systems' "Disconnect" footer** (the app has it; the prototype never drew it). Its footer is Skip for now / Install N emulators.

## Library rail

Order: **All games, Recent, Updates** → **BACKUP DRIVES** → storefront groups by name: **Amazon Games, Epic Games, GOG, RomM, Steam** (`STOREFRONT_ORDER`).

- All games, Recent and Updates include installed RomM games in every RomM state (and installed store games). Their counts always include both.
- **RomM group** (`rommRailGroup`): heading = the RomM mark (monogram "R", never a logo) + **ROMM** + a flag.
  | State | Flag | Rows |
  |---|---|---|
  | Connected | none | **Installed** (count), **Discover** (no count), one row per platform (count), including **Emulators** when the server has that platform |
  | Connecting at startup (`busy`, stored connected) | spinner, title "Connecting to RomM" | **Installed**, **Discover** (loading) |
  | Can't reach | **!** "Can't reach RomM" | **Installed** only |
  | Sign-in expired | **!** "RomM sign-in expired" | **Installed** only |
  | Not connected, RomM games installed | **off** "Not connected" | **Installed** only |
  | Not connected, none installed (`in_rail` false) | — | no group |
- **Platform rows:** the union of the server's platforms and the installed RomM platforms, sorted by label. **Count = RomM's game count for that platform** (user decision 14). The server's **Emulators** platform is one of these rows (2026-11-15): same sort, its own count and view (RomM › Emulators below). Installed games RomM doesn't have show in the platform's grid but are not counted. Installed games carry the dot on their covers.
- When the selected row disappears (Discover or a platform while RomM drops), selection falls back to **RomM › Installed**, or **All games** when the group is gone.

### Folding groups (2026-11-16)

Every group under All games, Recent and Updates folds: **Backup drives** (shown while at least one set-up drive is connected, as before) and the storefront groups **Amazon Games, Epic Games, GOG, RomM, Steam** (each shown under its existing rule). All games, Recent and Updates never fold.

**Look** (the Discover and platform screens show the same rail):

- **Group panel:** each group is an inset panel inside the rail: `surface` fill, 10px corners (`r-row` + 2), 2px padding, 2px between its rows. 8px between Updates and the first group, 6px between groups. No divider lines: the panels do the separating, the way the TV rail's spacing does.
- **Heading** (a full-width button, the panel's first line, min 30px tall): chevron · mark · name · flag · (closed) count.
  - Chevron: `chevronRight` 12px in a 14px box, `text-muted`; points right when closed, turns 90° to point down when open (150ms ease; no turn animation with reduced motion).
  - Mark: the store's monogram tile, now 16px (10px letter); RomM's "R" tile; **Backup drives has no mark** (as on TV).
  - Name, sentence case, 12px / 600, `text-h`, one line with an ellipsis: **Backup drives**, **Amazon Games**, **Epic Games**, **GOG**, **RomM**, **Steam**. The 10px capitals (BACKUP DRIVES, AMAZON GAMES…) are retired.
  - Flag, right-aligned, unchanged and shown **open or closed**: **!** (needs attention: login expired, store tool missing, Steam key or client, Can't reach RomM, RomM sign-in expired), **off** (not connected, games still installed), the spinner (RomM connecting). Its tooltip and accessible name are unchanged.
  - Count, **closed only**, 11px `text-muted`, 6px after the flag: stores and RomM show their **Installed** row's number (tooltip and accessible name "3 installed"); Backup drives shows the number of drives listed ("2 drives"). No count when the Installed row shows **!** (Steam not installed); the flag says it. Open, the heading has no count (the rows carry them).
  - Hover: `field` fill. Keyboard focus: the 2px `focus` ring, inset.
- **Rows** sit under the heading inside the panel: store and RomM rows start at 24px (under the mark), drive rows at 20px (their own drive icon, so the name keeps room in the 220px rail). Rows are otherwise unchanged.

**Behaviour:**

- **Click** anywhere on the heading opens or closes the group. **Keyboard:** the heading is a button in the Tab order, before its rows; **Enter** or **Space** toggles it; focus stays on the heading. A closed group's rows are not rendered, so Tab skips them. The arrow keys keep driving the grid (the rail has no arrow-key movement today; this adds none).
- **Closing the group that holds the selected row:** the selection does not change and the grid keeps showing that view. The closed heading **holds the selection**: the selected row's look (`selected` fill and the 3px `primary` bar), tooltip "Showing GOG › Installed", accessible description "Holds the view you're in". Opening the group shows the row selected as before.
- **Jumps from elsewhere open the group:** Settings › Game stores › Show in Library, a drive's Show in Library, Open Discover, a store view's Open Available, a notification's Show: when the target row's group is closed, it opens (and that is remembered, like a click). **Startup and selection fallback never open a group:** Open GRID on › Automatic landing on RomM › Discover with RomM closed keeps it closed (the heading holds the selection); RomM dropping from Discover to RomM › Installed leaves the group as it was.
- Toggling a group never moves the selection, never scrolls the grid and never changes Select mode.
- Accessibility: each group is a `section` named by its storefront; the heading button has `aria-expanded` and `aria-controls` (its rows, a `role="group"` with the same name).

**Memory** (shared by desktop and TV mode; the TVRommStorefront card points here):

- **What is remembered:** the set of **closed** groups, by id: `drives`, `amazon`, `epic`, `gog`, `romm`, `steam` (the `STOREFRONT_ORDER` ids plus `drives`). One list for both modes: closing GOG in desktop mode closes it in TV mode and the other way round.
- **Where:** the config file, `[ui] rail_closed = ["gog", "drives"]` (a list of ids, any order), beside `[ui] open_on` and `[ui] video_volume`. It is not a secret and holds nothing else.
- **Default:** a group whose id is not in the list is **open**: a group never seen, a fresh profile, a missing key, or a file written by an older GRID. A value that isn't a list of strings reads as empty (all open), with no error shown. Ids GRID doesn't know are kept as they are when the list is written back.
- **When it is written:** each time a group opens or closes (a click, Enter or Space, a TV A, or a jump that opens it). Best effort: if saving fails, the state holds for the session; no notification.
- **Lasts across:** switching rail rows, views (Library, Downloads, Settings and back), the RomM Discover and platform screens (they show the same rail), desktop and TV mode switches (same list, no reload) and restarts.
- **A group that goes and comes back keeps its state:** a store disconnected and later reconnected, the last drive unplugged and plugged back in, RomM leaving the rail (never connected, nothing installed) and returning. Hiding a group never removes its id from the list; only opening it does.
- **RomM while not usable:** Connecting, Can't reach, Sign-in expired and Not connected (games installed) all keep the remembered state; the group's rows change with the state, not its open or closed. Closed, it shows its flag and the Installed count.

## RomM › Installed

- Header: "RomM · Installed", "11 games · 21.2 GB · from romm.home.lan". Chips: "Signed in as player" (Connected, opens Settings › Game stores), "Connecting to romm.home.lan…" (spinner), "Not connected · Connect…" (Not connected). Actions: **Backup all N…**, **Select**. Sort, Size and search as the Library ("Search by title"; folded: "Search installed RomM games…").
- Grid: every installed RomM game, with platform tags, Library sort and search. **Emulator files are not games:** a file installed from RomM › Emulators never shows here, nor in All games, Recent, Updates or Discover, and is not in the Installed count or the RomM card's "installed here" count.
- **Banner** (one line above the grid, warning edge):
  - Can't reach: "**Can't reach RomM** · romm.home.lan didn't answer. Installed games still play." · **Retry** (Retrying…).
  - Sign-in expired: "**RomM sign-in expired** · Sign in again to browse, install and sync saves. Installed games still play." · **Sign in again…** (opens the sign-in dialog).
  - Not connected: no banner; the **off** flag and the chip are enough, as for a store.
- Empty: "No RomM games installed yet" · "Install games from Discover or a platform under it." · **Open Discover** (connected); "Install games from RomM once it's connected again." (no action) otherwise. Search with no match: "No installed RomM games match "…"".
- Not connected: RomM games show **no UPDATE tag** (Disconnect clears updates). Can't reach and Sign-in expired keep the last known UPDATE tags.

## RomM › Discover and platform screens

- The pill stays on **Library**; the rail is the Library rail.
- **Discover** is the old Server › Discover unchanged ("Recently added · New on RomM."), except the words. While connecting at startup it shows a panel: spinner "Connecting to romm.home.lan…" · "Discover and the platforms show when RomM answers. Installed games play now."
- **Platform screen** = the old Server platform screen (folding header, Firmware and Emulator chips that open Settings › Systems, **Backup all N…**, **Select**, search "Search title, platform or genre", Size). Counts line (PlayStation): "4 games on RomM · 1 installed · plus 1 only on this computer": the first number is the rail row's count (RomM's games), "installed" counts RomM's games installed here, and the installed games RomM doesn't have are named after a "plus", not counted in it.
- Grid = the server's games merged with installed rows (matched by `rom_id`; Play for installed, Install for the rest), plus installed rows the server doesn't have, tagged on the cover:
  - **NOT LINKED**: no `rom_id` (Hidden Kiln).
  - **NOT ON ROMM**: from another RomM server, or gone from this one (Sky Lantern).
  - These rows never show in Discover.

## RomM › Emulators (2026-11-15)

The server's **Emulators** platform (RomM's platform named "Emulators", `isEmulatorsPlatform`; one ROM per folder `ps2`, `pico`, `multi`…, its files the builds; `docs/server-emulator-packages.md`). Installs go through the existing server-emulator path, which stays.

**The rail row**
- Shows only while RomM is **Connected** and the server has an Emulators platform with at least one file. Hidden in every other state, like the other platform rows (Connecting at startup, Can't reach, Sign-in expired, Not connected), and hidden when the server has no Emulators platform.
- Label **Emulators**. Place: **sorted by label with the other platform rows** (the default; in the prototype it lands first, before GameCube). Why: the user asked for it as one of RomM's platforms, and a separate slot would need a divider the group has nowhere else.
- **Count** = what this computer can install: the emulators plus the other files the view lists (not the "other builds"). 8 in the prototype (5 emulators + 3 other files); 0 when nothing is for this computer.
- Key `platform:<slug>` of that platform, as every platform row (the prototype uses `platform:emulators`). When the row disappears while selected (RomM drops, or the server loses the platform), selection falls back to **RomM › Installed**, as for any platform.

**Header** (the platform screen's folding header, minus what doesn't apply)
- Title **Emulators**. Counts line: "5 emulators · 3 other files · 3 installed" ("installed" counts emulators installed at any version plus installed other files). Loading: "Loading…". Can't load: "On romm.home.lan". Nothing for this computer: "No builds for this computer".
- Chips: "Builds for Linux x64" (plain chip; title "GRID installs the newest build for this computer's system and processor"; the words name the running computer: "Windows x64", "Linux 32-bit ARM", "macOS (Apple silicon)"…) and **Settings › Systems** (opens Settings › Systems).
- Search, placeholder "Search emulators or files" (folded: "Search Emulators…"); it matches emulator name, system, file name and folder.
- **Not here:** Firmware chip, Emulator chip, Backup all N…, Select, Size, Sort, cloud saves or achievements, cover art.

**Body: three sections of rows, not covers**
1. **Emulators** · note "Named for GRID. Each installs as an emulator that Settings › Systems sets up, and RomM's newer builds show as updates." One row per emulator GRID knows that has a build for this computer, by name. The row offers the **newest build for this computer** (the server-emulator rule: system and CPU from the arch tag; a Mac takes `universal` when there is no newer build for its own CPU).
   - Row: a monogram tile (first letter), **name**, the system(s) it runs ("PlayStation 2"; a `multi` emulator from GRID's own list: "GameCube, Wii"; RetroArch "Many systems, through its cores"), then "v2.0.2 · 38 MB", then the file name in mono ("PCSX2 (v2.0.2) (arch-linux-x64).AppImage").
   - **Not installed:** primary **Install**.
   - **Not installed, GRID's catalog copy installed:** **Install**, plus the line "GRID's catalog copy of DuckStation is installed. This build installs beside it as "DuckStation (server)".".
   - **Queued** (another install is running): line "Queued · waits for the download before it"; **Cancel**.
   - **Downloading:** "Downloading · 46% · 17 of 38 MB" with a bar; **Cancel**. After Cancel: notification "Cancelled PCSX2"; the row returns to Install (or Update).
   - **Installing** (unpacking and checking the program): "Installing · checking the program", an indeterminate bar, no button.
   - **Installed:** pill "✓ Installed", **Uninstall…**; line "Set up in Settings › Systems · Open in Systems" (link).
   - **Update available** (the installed version is older than the newest build here): primary **Update to v2.0.2**, **Uninstall…**; line "**Installed: v1.6.0** · v2.0.2 is on RomM · Open in Systems".
   - **Install failed, wrong processor** (the program inside isn't for this computer; nothing registered): red edge, line "Couldn't install. The program in this file is built for Linux ARM64, not Linux x64. Check the file's arch tag on RomM. Nothing was set up."; **Retry**. Error notification: "Couldn't install DuckStation: its program is built for Linux ARM64. Nothing was set up." Any other failure puts the server-emulator path's existing error text on the same line.
   - **Uninstall confirm** (inline under the row): "**Uninstall PCSX2 v1.6.0?** GRID deletes it from the emulators folder in your library. Games and saves stay. PlayStation 2 games set to it can't start until another emulator is picked." (RetroArch: "Games set to RetroArch can't start…") · **Uninstall** (danger) / Cancel. Notification "PCSX2 uninstalled"; the row returns to Install.
   - Success notifications: "Pico-8 v0.2.7a6 installed from RomM · set up for Pico-8" (RetroArch: "… set up for its systems"); update: "PCSX2 updated to v2.0.2".
2. **Other files** · note "These don't follow GRID's naming, so GRID installs them as plain files in your library's Emulators folder. GRID doesn't set them up for a system or check them for updates. How to name builds" (link: opens the packaging guide in the browser). Every file that doesn't register: not named to the rule, no arch tag or no CPU in it, an emulator name GRID doesn't know, an extension GRID doesn't unpack.
   - Row: a plain tile, the **file name** in mono, line "<folder> folder · <size> · <why>", why being one of "Not named for GRID", "It has no arch tag", "Its arch tag has no processor", "GRID doesn't know an emulator called "Ares Nightly"", "GRID doesn't unpack .rar files".
   - States: **Install** (secondary, not primary: nothing is set up afterwards), Queued, Downloading, Installing (as above), **Installed** (pill "✓ Installed", **Uninstall…**, line "In your library's Emulators folder · Show folder", which opens that folder in the file manager).
   - **How this install differs:** the file lands in the library's Emulators folder like any RomM file (unpacked when it's an archive GRID knows), is **not registered** as an emulator, isn't in Settings › Systems, gets no defaults and **no updates**. This view is the only place it shows. Notification "xemu-v0.8.5-x86_64.AppImage installed in your library's Emulators folder".
   - Uninstall confirm: "**Uninstall xemu-v0.8.5-x86_64.AppImage?** GRID deletes it from the Emulators folder in your library." · Uninstall / Cancel; notification "xemu-v0.8.5-x86_64.AppImage uninstalled".
3. **"6 other builds GRID doesn't install on this computer"** (a closed disclosure; hidden while searching) · note "GRID installs the newest build for this computer's system and processor (Linux x64)." Dimmed rows, no buttons: file name, "<folder> folder · <size> · <reason>", reason "For Windows x64", "For macOS (universal)", "For Linux 32-bit ARM", "Older than v2.0.2". An emulator whose builds are all for other computers (Dolphin here) shows only in this list.

**Whole-view states**
- **Loading:** a panel "⟳ Loading the Emulators platform from romm.home.lan…" over four skeleton rows.
- **Can't load the list** (the request failed while RomM is still Connected): panel "**Can't load the Emulators platform**" · "romm.home.lan didn't answer. Installed emulators still work." · **Retry**. If RomM's state turns to Can't reach, the row goes and selection falls back to RomM › Installed with its banner.
- **Nothing for this computer** (files exist, none for this computer, no other files): panel "**Nothing here for this computer**" · "RomM's Emulators platform has 6 builds, all for other systems or processors. GRID installs builds for Linux x64." · **How to name builds**; then the other-builds list. Rail count 0.
- **Empty** (the platform has no files): no row, as for a platform with no games.
- **Search, no match:** "No emulators or files match "…"".
- **Not connected / Can't reach / Sign-in expired / Connecting:** no row (rail table above).
- Installs also show in **Downloads**, as server emulator installs do today.

## Details (RomM games)

- RomM not connected (any of not connected, can't reach, expired, connecting):
  - **Saves tab:** panel "**Cloud saves need RomM**" · "<why> This game's saves stay on this computer and still work; GRID uploads and restores them when RomM is back." · the fix action.
  - **Achievements tab:** "**Achievements need RomM**" · "<why> Your progress shows here from RomM; achievements still unlock in the emulator." · the fix action.
  - **Update to v1.2** is disabled (title "Updates need RomM") with the line "Updates need RomM".
  - **Backup to drive…** stays enabled (from the installed copy).
- `<why>`: "RomM isn't connected." / "GRID can't reach romm.home.lan right now." / "Your RomM sign-in expired." / "GRID is still connecting to romm.home.lan."
- The fix action: **Open Game stores** (not connected), **Retry** (can't reach), **Sign in again…** (expired).
- Unlinked row: "Not linked to a RomM game · **Link…**" (connected) or "Not linked to a RomM game · Linking needs RomM". Its Saves tab while connected: "**Link this game to sync its saves**" · "Cloud saves are kept per RomM game. Link <title> to its game on romm.home.lan first." · **Link…**.
- Row from another server: "From another RomM server (romm.old.lan) · not on romm.home.lan".

## Settings

- Rail (alphabetical): Appearance, Backup drives, Cloud saves, **Game stores**, Library, RetroAchievements, Systems, Updates. No Connection.
- **Game stores** pane: "Game stores" · "Connect the places your games come from. Each one gets a group in the Library. RomM is your own game server: GRID browses it, installs its games with an emulator and syncs their saves. …". Cards by name: Amazon Games, Epic Games, GOG, **RomM**, Steam. Every "Settings › Stores" string reads "Settings › Game stores".

### RomM card

- Head: RomM mark, **RomM**, sub-line, pill.
  | State | Sub-line | Pill |
  |---|---|---|
  | Connected | Signed in as player · romm.home.lan | ✓ Connected |
  | Connecting / reconnecting | same | spinner Connecting… |
  | Can't reach | same | Can't reach (warning) |
  | Sign-in expired | same | Sign-in expired (warning) |
  | Not connected, server stored | Not signed in · romm.home.lan | Not connected (dashed card) |
  | Never connected | Your own game server | Not connected (dashed card) |
- Connected: fields **Server** (link, opens it in the browser), **User**, **Sign-in** ("API token · kept in the OS keyring" / "Password · …"), **Status** ("Connected · checked 2 min ago"), **Games** ("45 on the server · 11 installed here"). Actions: **Show in Library** (primary), **Edit…**, **Reconnect** (Reconnecting…), **Open in browser**, **Disconnect…** (destructive).
- Can't reach: warning "**Can't reach RomM.** romm.home.lan: connection refused. Installed RomM games still play; browsing, installs, cloud saves and firmware downloads wait until it's back." Status "Can't reach · connection refused · tried just now" (`last_error`). Actions: **Retry** (primary), Edit…, Open in browser, Disconnect….
- Sign-in expired: warning "**Your RomM sign-in expired.** Sign in again to browse, install and sync saves. Installed games still play." Status "Sign-in expired · RomM answered 401". Actions: **Sign in again…** (primary), Edit…, Open in browser, Disconnect….
- Not connected: "Connect your RomM server to browse its games in the Library, install them with an emulator, and sync their saves and firmware. The 11 RomM games installed here still play." With a stored server: Server, User, Status "Not connected". Actions: **Connect RomM…**, Open in browser (only with a stored server).
- **Disconnect confirm** (in the card): "**Disconnect RomM?** GRID deletes your RomM sign-in from the OS keyring and stays open. Installed RomM games stay and still play. Cloud saves, firmware downloads and updates stop." · **Disconnect** / Cancel. Card after: Not connected, server and user kept. Notification: "RomM disconnected. Installed RomM games stay and still play."
- Notifications: "Connected to RomM at romm.home.lan as player", "Signed in to RomM again as player", "Reconnected to romm.home.lan", "Connected to RomM again · romm.home.lan".

### Settings › Library: Open config folder (2026-11-15)

- The user chose "Settings › General, near the library folder row". The app has no General page; the library folder row lives on **Settings › Library**, so it goes there.
- Under the **Library folder** block, a second block in the same style: heading **Config folder** · note "GRID's settings file lives here. Games, emulators and saves are in the library folder." · the path in the path box (`/home/player/.config/grid-launcher` in the prototype: the OS's config folder for GRID, the one `open_config_folder` opens) · secondary button **Open config folder** (sentence case like Change…; the old label "Open Config Folder" is retired).
- Click: opens the folder in the file manager (`open_config_folder` creates it first). No notification on success; on failure an error notification "Couldn't open the config folder: <reason>".
- Always enabled, also while a library move runs.

### Sign-in dialog (`RommSignInForm`)

- Titles and ledes:
  - **Connect to RomM**: "GRID browses your RomM server, installs its games with an emulator and syncs their saves." Button **Connect**.
  - **Edit RomM sign-in** (filled in): "Change the server or the sign-in. Installed RomM games stay; GRID links them to the server's games when it connects." Button **Save and connect**.
  - **Sign in to RomM again** (from expired): "RomM no longer accepts GRID's token for player. Enter a new one." Server URL and Username are shown read-only. Button **Sign in**.
- Fields: **Server URL** (placeholder https://romm.example), **Use an API token** (checkbox, on by default; `auth_kind` decides), **Username** (password mode only), **API token** / **Password**. Help: "Make a token in RomM under your user settings. GRID keeps it in the OS keyring, never in a file." / "GRID keeps your password in the OS keyring, never in a file."
- The button is disabled until URL + secret (+ username for a password) are filled; busy: "Connecting…".
- Errors (red, under the fields):
  - "Couldn't reach https://romm.example. Check the address, and that the server is running."
  - "RomM rejected this token (401). Check it, or make a new one." / "RomM rejected the username or password (401)."
  - `UsernameMismatch`: "This token belongs to "guest", not "player". Use a token for player, or disconnect RomM first to sign in as someone else."

### Choose your systems (decision 9)

- Opens after the **first successful RomM connect** when setup is needed (today's rule), from the welcome or from the card.
- Rows: the systems with games (server or installed); the catalog-only systems aren't listed. Footer: **Skip for now** / **Install N emulators**; "You can change these later in Settings › Systems."
- From the card, Skip and Install return to **Settings › Game stores**. From the welcome they go to the Library (RomM › Discover).

### Systems (works without RomM)

- Lede: "Pick an emulator for each system you play. GRID installs, updates and sets it up, with or without RomM." With RomM: "… Systems on your RomM server are listed with their game counts." No needs-RomM panel.
- **Rows** (two groups):
  - The main list: systems with games (server or installed), with an emulator chosen, or installing. Sorted as today (no emulator first, then by name). Server-only platforms (Atari Jaguar) show only while RomM is connected.
  - **More systems** · "GRID can set these up too. They move up when you have games for them or pick an emulator." Catalog systems with no games and no emulator, by name, closed, with a neutral **Not set up** pill and "No games yet"; open: "<emulator> is recommended for <system>." · **Install <emulator>** and the firmware block.
  - Windows (Proton) row and **Other emulators** as today.
- Count line per row: "48 on RomM · 3 installed" / "48 on RomM" (connected); "3 installed" / "No games yet" (without RomM). Windows: "11 on RomM · runs through Proton" / "Runs through Proton".
- **Install recommended (N)** counts only systems with games.
- **Firmware block** (replaces "<system> firmware · from your server · Install"): "**Firmware** · 3 files · 2 installed" (or "all installed"), then the buttons, a reason line and one line per expected file:
  - **Install from RomM** (primary while a required file is missing and RomM can install it; "Installing…"). Disabled with title and line "Install from RomM needs RomM · **Connect RomM…** · or locate files you have." while RomM is off, or "Not on your RomM server · locate files you have." when the server has none for the platform.
  - **Locate files…** (primary when RomM can't install and a required file is missing). PlayStation 3: **Locate PS3UPDAT.PUP…**.
  - No emulator chosen: both disabled, "Install an emulator first. GRID puts firmware into its folders."
  - File lines: icon + `name` + "Required · Japan" / "Optional · Europe" (+ "· no fixed checksum" for per-console keys) + status **Installed · verified** (check), **Installed · not verified** (? ring), **Missing** (warning, required) or **Missing · optional**.
  - The row pill says "! Firmware needed" while a required file is missing.
- **Locate files dialog** (stand-in for the system file chooser): "Locate PlayStation firmware" · "Pick firmware files you have, or a folder. GRID checks each file's size and MD5, then puts the ones it knows into DuckStation's folders." · path line · file list with checkboxes · **Use the whole folder** / Cancel / **Open**. PS3: "Locate PS3UPDAT.PUP", one file (radio), no folder button.
- Checking: "Checking 5 files…" · spinner "Comparing sizes and MD5s with the files DuckStation needs".
- **Report** ("PlayStation firmware" / "PS3 system software"): "5 files from /home/player/Downloads/bios. Installed into DuckStation's folders." (folder: "Searched /home/player/Downloads/bios and the folders in it."), then sections:
  - **Installed · N**: `scph5501 (1).bin → scph5501.bin` · "Size and MD5 match; installed under the name DuckStation looks for"; a zip member: `scph5502.bin from bios-pack.zip` · "Size and MD5 match"; PUP: "RPCS3 installed PS3 system software 4.92".
  - **Installed · not verified · N** (decision 13): `prod.keys` · "GRID has no checksum for this file, so it can't check it. If games don't start, dump it from your console again."
  - **Not installed · N**: "This isn't the scph5502.bin GRID knows (size or MD5 differs)"; "This PS3UPDAT.PUP isn't the one GRID knows (size or MD5 differs)"; "GRID doesn't know this file for PlayStation" (picked files only).
  - Folder scans count files that match nothing: "1 other file in the folder matched nothing."
  - Nothing installed: "… Nothing was installed." · **Choose other files…** / **Done**.

### Cloud saves and RetroAchievements

- RomM not connected: a panel under the heading, the form stays visible and editable.
  - Cloud saves: "**Cloud saves need RomM**" · "<why> GRID keeps your emulator saves and states on your RomM server. These settings apply when it's connected. GOG and Epic games sync with their own store's cloud." · fix action.
  - RetroAchievements: "**Achievement progress needs RomM**" · "<why> Details › Achievements shows your progress from RomM. Signing in here still turns on achievements in RetroArch, DuckStation and Dolphin." · fix action. The RA token stays editable: it is written into the emulators and doesn't need RomM.

### Appearance › Open GRID on

- Below Theme: "Open GRID on" · hint "Here and in TV mode" · options **Automatic: Discover when RomM is connected, else Library**, **Discover (when RomM is connected)**, **Library**.
- RomM not connected and the choice isn't Library: "RomM isn't connected, so GRID opens on All games."

## Welcome (replaces Connect)

- Full window, theme toggle in the corner. "**Welcome to GRID**" · "One Library for the games on your RomM server and the stores you buy from."
- Python import: notice "**Your settings from the previous version are filled in.** Enter your RomM token to reconnect, or skip it for now.", the library folder and the RomM server and username filled in, the RomM row open, and the import notification "Imported 3 emulators and 5 games from the previous version. Enter your RomM token to reconnect." over the welcome.
- **1 Library folder** (required): "Games, emulators and saves go here." · field · **Browse…**. Empty: "Choose a folder to continue." and both footer buttons disabled.
- **2 Connect your games · optional**: "Connect the ones you use. Add or change them any time in Settings › Game stores." Rows by name, each with its mark, name, a sub-line and its action:
  - Amazon Games, Epic Games, GOG: "Sign in on <store>'s own page. GRID never sees your password." · **Connect…** (the store login window, over the welcome).
  - RomM: "Your own game server. GRID browses it and installs its games with an emulator." · **Connect…** opens the sign-in form inline (same fields, errors and dashed result button as the dialog) · **Connect** / Cancel.
  - Steam: "Installed Steam games show as soon as Steam is set up here." · **Set up…** opens the Steam card's form inline (client check, account, Web API key, Connect Steam, Test) · Close.
  - Connected: "Signed in as sam_plays" / "Connected as player · romm.home.lan" / "SteamID64 … · SamPlays" with a ✓ Connected pill (RomM also **Edit**).
- Footer: nothing connected: **Skip for now** + **Continue** (disabled) · "Nothing connected yet. Skip for now opens GRID with your library folder." Something connected: **Continue** only · "Next: choose the systems you play." (after a first RomM connect) or "GRID opens on your Library."
- No emulator step without RomM (0.11). With RomM connected, Continue → Choose your systems → the Library on RomM › Discover; otherwise the Library on All games.

## Backups without RomM (decisions 8 and 12)

- Select, **Backup all N…** and Details › Backup to drive… work with RomM off.
- The dialog: lede "RomM isn't connected, so GRID copies the games installed on this computer, with the art and details it has, to the drive's gridpack folder. Nothing downloads."
- Source: "Copy installed games from this computer" is ticked and disabled · "8 games from this computer (19.1 GB) · without RomM, only installed copies". No "server has newer files" choice.
- "**Can't backup without RomM · N**": one line per game, "<platform> · <local reason>. Connect RomM to back it up from the server." (e.g. "Installed outside the library folder", "Its files weren't found on this computer"). A row with no `rom_id`: "It isn't linked to a RomM game. Link it to its RomM game first."
- Also copy: Emulators "… · from GRID's catalog; builds hosted on your RomM server are skipped"; Firmware unticked and disabled "Needs RomM · skipped: Sega Saturn".
- Other words: Settings › Backup drives "Copy games to a USB drive, …", "Firmware from RomM for the systems you backup"; an empty drive "Nothing on this drive yet. Backup games from the Library."

## Inventory checklist

Top bar and views
- [ ] Three pills (Library, Downloads, Settings), Ctrl+1..3; no chip, dot or server menu; theme toggle stays.

Library rail
- [ ] Order: All games, Recent, Updates · Backup drives · Amazon, Epic, GOG, RomM, Steam.
- [ ] No PLATFORMS section.
- [ ] RomM group Connected: Installed (n), Discover, platform rows with RomM's game counts.
- [ ] Connecting: spinner flag, Installed + Discover.
- [ ] Can't reach: "!" flag, Installed only.
- [ ] Sign-in expired: "!" flag, Installed only.
- [ ] Not connected with installed games: "off" flag, Installed only.
- [ ] Never connected, nothing installed: no group.
- [ ] Selection falls back to Installed (or All games).
- [ ] All games / Recent / Updates include installed RomM games in every state.

Library rail › Folding groups (2026-11-16)
- [ ] Each group in its own inset panel; 8px under Updates, 6px between groups; no dividers.
- [ ] Heading: chevron, mark (none for Backup drives), sentence-case name, flag; closed adds the count.
- [ ] Open heading: chevron down, no count. Closed heading: chevron right, count (Installed / drives), rows not rendered.
- [ ] Flags stay visible closed: Amazon "!", RomM "off", RomM spinner, Steam "!" (no count).
- [ ] Click, Enter and Space toggle; focus stays; Tab skips a closed group's rows; arrows still drive the grid.
- [ ] Closed group holding the selection: selected fill and bar, "Showing … › …" tooltip; selection and grid unchanged.
- [ ] Jumps (Show in Library, Open Discover, Open Available, notification Show) open a closed target group.
- [ ] Startup on RomM › Discover and RomM's selection fallback leave a closed RomM closed.
- [ ] Memory: survives row and view switches, Discover and platform screens, mode switches and restarts; shared with TV mode.
- [ ] A group that disappears and comes back keeps its state; never-seen groups are open.

RomM › Installed
- [ ] Header, counts, chips per state, Backup all N…, Select, prototype select.
- [ ] Banner: Can't reach · Retry (Retrying…).
- [ ] Banner: Sign-in expired · Sign in again….
- [ ] No banner when not connected; no UPDATE tags when not connected.
- [ ] Empty (connected / not), no search match.

Discover and platforms
- [ ] Discover in the Library ("New on RomM."), pill on Library.
- [ ] Discover while connecting (spinner panel).
- [ ] Platform screen: header, counts "N games on RomM · M installed · plus K only on this computer" (N matches the rail row), chips, Backup all N…, Select, search.
- [ ] Merged covers: Play / Install; NOT LINKED; NOT ON ROMM; these never in Discover.

RomM › Emulators (2026-11-15)
- [ ] Row only while Connected and the server has an Emulators platform with files; sorted by label; count = emulators + other files for this computer.
- [ ] Row gone (RomM drops, platform gone) → selection falls back to RomM › Installed.
- [ ] Header: title, counts line, "Builds for <system CPU>" chip, Settings › Systems chip, search; no firmware or emulator chip, Backup all, Select, Size.
- [ ] Emulator rows: Install, catalog-copy line, Queued, Downloading (Cancel), Installing, Installed, Update to vX, Install failed (wrong processor, Retry), inline Uninstall confirm, notifications.
- [ ] Other files: why line, Install / progress / Installed (Show folder) / Uninstall confirm; not registered, no updates.
- [ ] Other builds disclosure with reasons; hidden while searching.
- [ ] Loading, Can't load (Retry), Nothing for this computer, search no match.
- [ ] Emulator files never in RomM › Installed, All games, Recent, Updates, Discover or their counts.

Settings › Library
- [ ] Config folder block under Library folder; Open config folder opens it; error notification.

Details
- [ ] Saves tab needs RomM (three whys, three fix actions).
- [ ] Achievements tab needs RomM.
- [ ] Update disabled + "Updates need RomM".
- [ ] Unlinked: Link… / "Linking needs RomM"; Saves "Link this game to sync its saves".
- [ ] Another server's row line.

Settings › Game stores
- [ ] Pane renamed, rail alphabetical, no Connection; cards Amazon, Epic, GOG, RomM, Steam.
- [ ] RomM card: Connected, Connecting, Can't reach, Sign-in expired, Not connected (stored), Never connected.
- [ ] Disconnect confirm and the after state; notifications.
- [ ] Sign-in dialog: Connect, Edit (filled in), Sign in again (read-only URL/user); token/password toggle; busy; four results.
- [ ] First connect from the card → Choose your systems → Skip/Install → back on Game stores.

Settings › Systems
- [ ] Without RomM: no Atari Jaguar, counts "N installed"/"No games yet", More systems group, no needs-RomM panel.
- [ ] With RomM: server counts, Atari Jaguar ("No emulator GRID installs plays this system").
- [ ] Firmware block: all states of Install from RomM (normal, primary, Installing…, needs RomM, not on server) and Locate files….
- [ ] No-emulator firmware state.
- [ ] File lines: verified, not verified, missing, missing · optional.
- [ ] Locate dialog (files, whole folder), Checking…, report (installed incl. rename and zip member, not verified, not installed, matched-nothing count, nothing installed).
- [ ] PS3: Locate PS3UPDAT.PUP… (single pick) and its report.
- [ ] Install recommended counts only systems with games.

Other settings
- [ ] Cloud saves needs-RomM panel (form editable).
- [ ] RetroAchievements needs-RomM panel (token editable).
- [ ] Appearance › Open GRID on: three options, hint, the "opens on All games" line.

Welcome
- [ ] New user: folder, five rows, Skip for now / Continue (disabled).
- [ ] Empty folder state.
- [ ] Amazon/Epic/GOG via the login window over the welcome; connected rows.
- [ ] RomM inline form, errors, connected row with Edit.
- [ ] Steam inline form.
- [ ] Continue after RomM → Choose your systems → Library › RomM › Discover; without RomM → All games.
- [ ] Python import: notice, filled folder/server/user, RomM row open, import notification.

Backups without RomM
- [ ] Dialog lede, locked source, Can't backup list (local reasons, unlinked), emulators line, firmware skipped.
- [ ] Words: Backup drives lede and firmware checkbox, empty drive text.

## Decisions made by the designer

The user signed these off with the TV card on 2026-11-15 (decisions 14–17 in the plan). Where the two cards' words differed, these words win (decision 17), and the TV card now uses them.

1. **Platform row count = RomM's game count only.** Settled by the user (decision 14): installed games RomM doesn't have show in the grid but are not counted, in the rail row or the platform screen's first number. Installed games keep the cover dot. TV mode does the same.
2. **Two cover tags** for installed rows the server doesn't have: **NOT LINKED** (no rom id, the plan's "Not linked") and **NOT ON ROMM** (another server, or gone from the server). Count line says "plus N only on this computer". Settled by the user (decision 15); TV mode has both tags.
3. **Startup "Connecting" state:** while `romm_restore` runs for a stored connection, the group shows Installed + Discover (Discover shows a connecting panel) and a spinner flag, so Automatic can land on Discover; platform rows appear when the list loads. If the probe fails, selection falls back to Installed.
4. **No chip for Can't reach / Sign-in expired** in the Installed header; the banner carries it (stores use a chip plus a state panel).
5. **Welcome footer:** while nothing is connected, **Skip for now** is the way on and **Continue** is disabled; once something is connected, only **Continue** shows. A RomM connect in the welcome does not jump away at once: Choose your systems follows **Continue**, so the user can connect the other rows first.
6. **Systems groups the catalog:** systems with no games and no emulator move to a calm **More systems** group (closed, neutral "Not set up"), so the list without RomM isn't 80 orange "No emulator" rows. Install recommended counts only systems with games.
7. **Choose your systems lists only systems with games** (server or installed), as today; catalog-only systems are not offered there.
8. **Cloud saves and RetroAchievements forms stay editable** without RomM, under the panel; the RA token is written into emulators and doesn't need RomM.
9. **Updates:** Disconnect clears RomM UPDATE tags (plan 0.6); Can't reach and Sign-in expired keep the last known ones, with Update disabled ("Updates need RomM").
10. **Locate files** uses a GRID report after the native chooser; a folder is a second button in the chooser step (GTK can't pick files and a folder in one dialog). Picked files that match nothing are listed as refused; folder files that match nothing are only counted.
11. Sign-in result **UsernameMismatch** words, and **Edit** keeping installed games ("GRID links them to the server's games when it connects").
12. **Backups without RomM:** a row with no rom id is listed under "Can't backup without RomM" with "Link it to its RomM game first" (it has no pack identity either way).

## Notes for implementers

- Discover and the platform screens are drawn here with the old Server view's DOM (`S.view 'server'`) under the Library pill; in the app they are RomM views inside `Library.svelte` (Step 3). Their behaviour is the Server view's (folding header, select, Backup all N…, chips).
- Rail keys: `romm:installed`, `romm:discover`, `platform:<slug>` (plan 0.3). The prototype uses its own platform codes in the key.
- **Folding groups (2026-11-16):** one shared store for both modes, keyed `'drives' | StoreSource | 'romm'`, read from and written to `[ui] rail_closed` through the `uiSettings` store. It replaces TV mode's three session-only fold states (`lib.folds.drives` in `tv/tabs/library/library.svelte.ts`, `folds` in `tv/stores/storeState.svelte.ts`, `tvRomm.open`). On the desktop, `RailPaneEntry`'s `heading` / `headingMark` / `headingFlag` / `indent` become a group (id, name, mark, flag, closed count, rows) rendered as described above; the pure parts (which group a key belongs to, the closed count, holds-selection, parsing the stored list) belong in a `.ts` module with vitest. Jumps that select a row in a closed group open it through the same store; startup (`openOn`) and the RomM selection fallback must not. Source: `docs/design/prototype/railfold.js` / `railfold.css`.
- **Emulators row (2026-11-15):** plan 0.3 drops the Emulators shelf from the platform rows; that changes. `rommRailGroup` keeps it (`isEmulatorsPlatform`) with its own count; `RommStatus.installed_count`, RomM › Installed, All games, Recent, Updates and Discover still leave Emulators-platform rows out. The view's data is the server-emulator path's (`ServerPackage` folders and files, `ServerBuildInfo`, setup's newest-build rule); files that don't parse are the Other files, installed as an ordinary RomM file of the Emulators platform. Source: `docs/design/prototype/romm_emu.js` / `romm_emu.css`.

## Settled 2026-11-15 (were open questions)

1. **Catalog copy and server build:** keep both. The server build registers beside GRID's catalog copy, as drawn; the user picks per system in Settings › Systems.
2. **Open config folder:** stays on Settings › Library under the library folder. No General page.
3. **"How to name builds"** opens https://github.com/Sixdd6/grid-launcher/blob/main/docs/server-emulator-packages.md in the browser.

- Words live in `app/src/lib/romm/words.ts`; this README is the list.
- The prototype's catalog systems (PS3, Switch, GBA, PSP, Dreamcast), firmware files and Locate results are placeholders, not `firmware-known.json` data. No real BIOS names or hashes are implied beyond common file names.
- Notifications and the store login window sit above the welcome (z-order), so the import notice and store sign-in work there.
