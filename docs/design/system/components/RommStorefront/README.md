A proposal: RomM becomes one optional storefront beside Amazon Games, Epic Games, GOG and Steam. The card is built on Game stores (what main has). It opens where a connected user lands with Open GRID on › Automatic: **Library › RomM › Discover**. Everything not listed here matches Game stores.

**Status: proposed, not built (step D1).** The decisions and contracts are in `docs/romm-storefront-plan.md` (user decisions 1–13, Step 0). TV mode's version is the **TVRommStorefront** card (step D2). Source: `docs/design/prototype/romm.js` and `romm.css`, built by `applayout.py` on top of the GameStores card.

## Try it

- **RomM state:** a dashed **Prototype** select sits on Library › RomM › Installed (header), on the RomM card in Settings › Game stores (corner) and in Settings › Systems (under the heading). It switches RomM between: Connected, Connecting (at startup), Can't reach RomM, Sign-in expired, Not connected (games installed), Never connected (nothing installed). Every screen follows it.
- **Welcome:** Settings › Game stores › **Prototype: open the welcome**, or **Prototype: welcome after a Python import**. **Prototype: start again** reloads the card.
- **Connect from the card:** set **Never connected**, then RomM card › **Connect RomM…**. The dialog's dashed button picks the next result (works, can't reach the server, rejected (401), another user's token). A successful first connect opens **Choose your systems**; Skip or Install returns to Settings › Game stores.
- **Locate files:** Settings › Systems › PlayStation (open the row) › **Locate files…**. Tick files, or **Use the whole folder**. Nintendo Switch shows "not verified" keys; PlayStation 3 shows the PUP picker; PlayStation 2 shows the "install an emulator first" state.
- **Backups without RomM:** set **Not connected**, then Library › RomM › Installed › **Backup all 11…**.
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
  | Connected | none | **Installed** (count), **Discover** (no count), one row per platform (count) |
  | Connecting at startup (`busy`, stored connected) | spinner, title "Connecting to RomM" | **Installed**, **Discover** (loading) |
  | Can't reach | **!** "Can't reach RomM" | **Installed** only |
  | Sign-in expired | **!** "RomM sign-in expired" | **Installed** only |
  | Not connected, RomM games installed | **off** "Not connected" | **Installed** only |
  | Not connected, none installed (`in_rail` false) | — | no group |
- **Platform rows:** the union of the server's platforms and the installed RomM platforms, sorted by label. **Count = the games in that platform's view** (the server's games plus installed games the server doesn't have). Installed games carry the dot on their covers.
- When the selected row disappears (Discover or a platform while RomM drops), selection falls back to **RomM › Installed**, or **All games** when the group is gone.

## RomM › Installed

- Header: "RomM · Installed", "11 games · 21.2 GB · from romm.home.lan". Chips: "Signed in as player" (Connected, opens Settings › Game stores), "Connecting to romm.home.lan…" (spinner), "Not connected · Connect…" (Not connected). Actions: **Backup all N…**, **Select**. Sort, Size and search as the Library ("Search by title"; folded: "Search installed RomM games…").
- Grid: every installed RomM game (not the hidden Emulators shelf), with platform tags, Library sort and search.
- **Banner** (one line above the grid, warning edge):
  - Can't reach: "**Can't reach RomM** · romm.home.lan didn't answer. Installed games still play." · **Retry** (Retrying…).
  - Sign-in expired: "**RomM sign-in expired** · Sign in again to browse, install and sync saves. Installed games still play." · **Sign in again…** (opens the sign-in dialog).
  - Not connected: no banner; the **off** flag and the chip are enough, as for a store.
- Empty: "No RomM games installed yet" · "Install games from Discover or a platform under it." · **Open Discover** (connected); "Install games from RomM once it's connected again." (no action) otherwise. Search with no match: "No installed RomM games match "…"".
- Not connected: RomM games show **no UPDATE tag** (Disconnect clears updates). Can't reach and Sign-in expired keep the last known UPDATE tags.

## RomM › Discover and platform screens

- The pill stays on **Library**; the rail is the Library rail.
- **Discover** is the old Server › Discover unchanged ("Recently added · New on RomM."), except the words. While connecting at startup it shows a panel: spinner "Connecting to romm.home.lan…" · "Discover and the platforms show when RomM answers. Installed games play now."
- **Platform screen** = the old Server platform screen (folding header, Firmware and Emulator chips that open Settings › Systems, **Backup all N…**, **Select**, search "Search title, platform or genre", Size). Counts line: "4 games on RomM · 2 installed · 1 only on this computer".
- Grid = the server's games merged with installed rows (matched by `rom_id`; Play for installed, Install for the rest), plus installed rows the server doesn't have, tagged on the cover:
  - **NOT LINKED**: no `rom_id` (Hidden Kiln).
  - **NOT ON ROMM**: from another RomM server, or gone from this one (Sky Lantern).
  - These rows never show in Discover.

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
- [ ] RomM group Connected: Installed (n), Discover, platform rows with view counts.
- [ ] Connecting: spinner flag, Installed + Discover.
- [ ] Can't reach: "!" flag, Installed only.
- [ ] Sign-in expired: "!" flag, Installed only.
- [ ] Not connected with installed games: "off" flag, Installed only.
- [ ] Never connected, nothing installed: no group.
- [ ] Selection falls back to Installed (or All games).
- [ ] All games / Recent / Updates include installed RomM games in every state.

RomM › Installed
- [ ] Header, counts, chips per state, Backup all N…, Select, prototype select.
- [ ] Banner: Can't reach · Retry (Retrying…).
- [ ] Banner: Sign-in expired · Sign in again….
- [ ] No banner when not connected; no UPDATE tags when not connected.
- [ ] Empty (connected / not), no search match.

Discover and platforms
- [ ] Discover in the Library ("New on RomM."), pill on Library.
- [ ] Discover while connecting (spinner panel).
- [ ] Platform screen: header, counts "… only on this computer", chips, Backup all N…, Select, search.
- [ ] Merged covers: Play / Install; NOT LINKED; NOT ON ROMM; these never in Discover.

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

## Decisions made by the designer — confirm with the user

1. **Platform row count = the games in that platform's view** (server games plus installed games the server doesn't have), not the bare server ROM count; the two only differ when installed rows aren't on the server. Installed games keep the cover dot.
2. **Two cover tags** for installed rows the server doesn't have: **NOT LINKED** (no rom id, the plan's "Not linked") and **NOT ON ROMM** (another server, or gone from the server). Count line says "N only on this computer".
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
- Words live in `app/src/lib/romm/words.ts`; this README is the list.
- The prototype's catalog systems (PS3, Switch, GBA, PSP, Dreamcast), firmware files and Locate results are placeholders, not `firmware-known.json` data. No real BIOS names or hashes are implied beyond common file names.
- Notifications and the store login window sit above the welcome (z-order), so the import notice and store sign-in work there.
