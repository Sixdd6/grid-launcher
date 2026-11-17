TV mode with RomM as one optional storefront beside Amazon Games, Epic Games, GOG and Steam. It is the TVStores card (TV mode plus the game stores) with `tvromm.js` / `tvromm.css` on top, and it opens on Library › RomM › Discover with RomM connected. Plan and decisions: `docs/romm-storefront-plan.md` (Step D2; user decisions 1–13). Everything not listed here is as in the TVMode and TVStores cards.

**Status: signed off, not built.** The user approved it with the desktop card (RommStorefront, Step D1) on 2026-11-15, with decisions 14–17: the platform count is RomM's games only, two cover tags (NOT LINKED, NOT ON ROMM), a never-connected GRID opens on All games, and where the two cards' words differed, desktop's words win (this card now uses them; `romm/words.ts` is shared).

**Changes 2026-11-15** (signed off and built (5a62b77)): **RomM › Emulators**, the server's Emulators platform as a row in Library › RomM with its install list (section below; user decision 2026-11-15). Emulator files still never join RomM › Installed, All games, Recent, Updates or Discover. Open config folder is desktop only (Settings › Library on the desktop card); TV mode has no such row. Try it: Library › RomM › Emulators, or Menu › Prototype only › **Show: RomM › Emulators…** (Files loaded, Loading, Can't load the list, Nothing for this computer, No Emulators platform).

**Changes 2026-11-16** (signed off and built (5a62b77)): **the Library rail's folders are remembered, shared with desktop mode, and each sits in its own panel** (user request 2026-11-16; user decision: one open/closed state for both modes). The folders already opened and closed with A for the session; now which are closed lasts across restarts and mode switches, a closed folder shows its count, and a closed folder holding the selected row wears the selected fill. Section **Library › Folders** below; the memory rules are the desktop card's (RommStorefront README § Library rail › Folding groups › Memory). Source: `tvrailfold.js` / `tvrailfold.css`.

**Changes 2026-11-17** (user decision): **Discover and Open GRID on are removed until the Discover rework.** Library › RomM has no Discover row (connected: Installed, then the platforms; connecting at startup: Installed only, with the spinner), the Menu has no Discover item, Search everything (Y on Discover) is gone with it, and **Settings › Appearance › Open GRID on** is hidden: TV mode opens on Library › All games with its rail row focused (`[ui] open_on` is still stored and ignored). The sections below that describe Discover, Search everything and Open GRID on are kept for the rework and do not apply now. The prototype no longer shows them.

TV rules that hold everywhere below:
- TV flows never send the player to desktop mode. A store's login window, RomM's web page ("Open in browser") and an emulator opened from its System page open over TV mode.
- Every size is in rem (TV mode's 1/54-of-the-screen-height root), so it reads the same from a couch at 720p, 1080p and 4K.
- Focus is always on something. A change never strands it: when a focused row goes away, focus moves to the row now in its place.
- The hint bar shows what each button does now. A control that can't run now is dimmed or disabled: A on it says why in an error card, and the hint bar shows no A for it.

## Try it

Click inside the card, then use the keyboard as a controller (TV mode's README has the full map: arrows, Enter = A, Esc = B, X, Y, Q / E = LB / RB, Z = LT, R = RT, M = Menu, V = View, G = Guide).

- **Menu › Prototype only** (open it with G on pages where M is the page's main button):
  - **Show: welcome (first run)** and **Show: welcome after a Python import**.
  - **Show: a RomM state…**: Connected; Can't reach RomM; Sign-in expired (a 401 mid-session); Not connected, with installed RomM games; Not connected, no RomM games (no group).
  - **Show: next RomM sign-in result…**: Succeeds, RomM rejects the token, Can't reach RomM. It applies to the next Connect, Sign in again, Edit or Reconnect.
  - **Show: restart GRID (RomM answers)** and **(RomM doesn't answer)**: the startup probe.
  - **Show: Settings › Game stores › RomM**, **Show: Settings › Systems**, **Show: Choose your systems (from the RomM page)**.
  - TVStores' items stay (store states, Settings › Game stores, GOG offline installer, Amazon sign-in), and TV mode's unreachable library, Couch kit and failed install.
- **Folders (2026-11-16):** Left into the rail, then A on a folder heading (Backup drives, Amazon Games, Epic Games, GOG, RomM, Steam). Close RomM while Discover shows, go to the grid and press Left: focus lands on the RomM heading, which holds the selection. Reload: the folders stay as you left them. The RommStorefront card in the same browser shares the state (localStorage `grid.railClosed`; clear it to start fresh).
- **Locate files:** Settings › Systems › PlayStation (or PlayStation 2, Nintendo Switch, Sega Saturn) › Locate files… opens `/run/media/USB-1TB/bios`. Pick `scph5501 (1).bin`, `scph1001.bin`, `BIOS pack.zip` and `readme.txt`, then Start; or Y checks the whole folder. PlayStation 3 › Locate PS3UPDAT.PUP… opens `~/Downloads` (a good file); the copy on the USB drive is a bad one.

## The shell

- **Tabs:** Library, Downloads, Settings, between the LB / RB glyphs. **No Server tab.** GRID opens on the Library (see Where GRID opens).
- **Top bar:** the GRID wordmark, the tabs (or the breadcrumb on a page), the live download and the clock. **No connection dot, user or "Offline" text.**
- **Hint bar:** the left side shows the controller's name (or "Keyboard"). **No offline line** ("Offline · Installed games still play" and "No server · …" are gone).
- **Menu (Start / Guide):** the clock, then Discover (only while RomM is connected or the startup probe runs; it opens Library › RomM › Discover), Downloads, Notifications, Settings, Switch to desktop mode, Quit GRID. **No Reconnect and no "<user> · <server>" line.**

## Library › RomM group

The rail: All games, Recent, Updates, then **Backup drives**, then the storefront groups in name order: **Amazon Games, Epic Games, GOG, RomM, Steam** (`STOREFRONT_ORDER`). **The Platforms folder is gone**: platform rows live only in the RomM group. The RomM group is a folder like a store's: the RomM mark (a monogram tile "R" in desktop's RomM colour, never a logo) and "RomM"; A opens and closes it. Its flags read, for screen readers, "Connecting to RomM", "Can't reach RomM", "RomM sign-in expired" and "Not connected", as on desktop.

| RomM | Heading flag | Rows |
| --- | --- | --- |
| Connected | none | **Installed** (count), **Discover**, then one row per platform, by name, with the RomM game count; **Emulators** among them when the server has that platform |
| Startup probe running | spinner | Installed, Discover ("Connecting to RomM…") |
| Can't reach RomM | **!** | Installed only |
| Sign-in expired | **!** | Installed only |
| Not connected, with installed RomM games | **off** | Installed only |
| Not connected, no installed RomM games | — | no group |

- Installed RomM games stay mixed into All games, Recent and Updates in every state, with their platform tag.
- **Selection falls back:** when RomM drops while Discover or a platform is open, the selection and focus move to **RomM › Installed**; when the group goes, to **All games**. A redraw never moves focus anywhere else.
- **Folders:** RomM's folder follows the rules in **Library › Folders** below, like every other folder.
- **Platform count (decision 14):** the platform row shows RomM's game count only. Installed games RomM doesn't have show in the platform's grid but are not counted. Installed games carry the installed dot on their covers.

### Library › Folders (2026-11-16)

Every folder in the rail: **Backup drives** (always shown in TV mode, with "No backup drives connected" when empty, as before) and the storefront folders **Amazon Games, Epic Games, GOG, RomM, Steam** (each under its existing rule). All games, Recent and Updates are plain rows above the divider and never fold.

**Look**

- **Panel per folder:** the heading and its rows sit in an inset panel: `surface` fill, 0.8rem corners, 0.2rem padding, 0.2rem between rows, 0.45rem between panels (none under the divider). The desktop rail now has the same panels, so the two modes read alike.
- **Heading** (unchanged parts): chevron (right when closed, down when open), mark (store monogram, RomM's "R"; none for Backup drives), name in sentence case, flag (**!**, **off**, spinner) at the right. It now stays on one line.
- **Closed:** after the flag (0.6rem), the folder's count in `text-muted`: stores and RomM their **Installed** number, Backup drives the number of drives (none when no drive is connected); no count when Installed shows **!** (Steam not installed). Open: no count.
- **Closed and holding the selected row:** the selected row's look (`selected` fill, the 0.25rem `primary` bar). With focus on it, the focus ring sits on top as on any row.

**Controller**

- Up / Down move through headings and open folders' rows as before; a closed folder is one stop. A on a heading opens or closes it (hint **A Open** / **A Close**); focus stays on the heading; nothing else on screen changes. Right from a heading goes into the grid (as Right from any rail row).
- **Arriving in the rail** (B or Left from the grid) lands on the selected row, or on the closed heading that holds it (`data-holds-sel`), so entering the rail never changes the selection.
- **Moving onto a row** selects it, as today; moving across a heading changes nothing.
- **Jumps** that select a row in a closed folder open it (and that is remembered): Settings › a drive › Show in Library, Settings › Game stores › a store's or RomM's Show in Library, Open Discover, the welcome's and Choose your systems' way into the Library, a notification card's View. **Startup and the RomM selection fallback never open a folder:** GRID opening on RomM › Discover keeps a closed RomM closed, its heading holding the selection.
- No new hints, no new words, nothing sends the player to desktop mode.

**Memory** (same list as desktop, `[ui] rail_closed`): closed ids `drives`, `amazon`, `epic`, `gog`, `romm`, `steam`; anything not listed is open; written on every open or close; kept across tab switches (Library, Downloads, Settings), pages and back, desktop and TV mode switches and restarts; a folder that goes away (store disconnected, RomM gone from the rail) keeps its state for when it comes back. This replaces "Folders stay as you left them for the session" (TV mode checklist item 183).

**States to check**

- [ ] All folders open (fresh profile): panels, chevrons down, no counts.
- [ ] Closed store folder: chevron right, Installed count; Amazon "!" and the count; Steam without its client: "!" and no count.
- [ ] Closed RomM: Connected (count), Connecting (spinner and count), Can't reach / Sign-in expired ("!" and count), Not connected ("off" and count).
- [ ] Closed Backup drives: drive count; no count with no drive connected.
- [ ] Closed folder holding the selection: selected fill and bar; B / Left from the grid lands on it; A opens it with the row still selected.
- [ ] Jumps open a closed folder; startup on Discover doesn't.
- [ ] State shared with desktop and kept across restarts.

### RomM › Installed

- Header, two rows: "**RomM** · Installed" with the mark, and "12 games" ("3 of 12 games match" while a search narrows it). Then the chips: "Signed in as **sam** · romm.home.lan" (connected) or "Not connected"; **Backup all 12…** (Backups work without RomM, decision 8); RT Select; LT Sort; Y Search.
- **The one-line banner**, between the two header rows, focusable (Up from the grid reaches it):
  - Can't reach: "**Can't reach RomM** · romm.home.lan didn't answer. Installed games still play." with **Retry** (A "Retry"). While it runs: "Retrying…", disabled ("GRID is trying RomM now.").
  - Sign-in expired: "**RomM sign-in expired** · Sign in again to browse, install and sync saves. Installed games still play." with **Sign in again…** (A "Sign in again"), which opens the sign-in page.
  - Not connected: no banner (the **off** flag says it, as for a store).
- Grid: the installed RomM rows with the Library's sort (LT) and search (Y, "Search RomM · Installed"); covers carry platform tags. RT is Select (Game backups). Hints: Y Search, LT Sort, RT Select.
- Empty: "No RomM games installed yet" + "Install games from Discover or a platform under it." (connected), or the needs-RomM line (other states: "Connect RomM in Settings › Game stores." / "Your RomM sign-in expired. Sign in again in Settings › Game stores." / "Can't reach RomM right now. GRID tries again when you press Retry."; TV keeps these pointers because the empty grid has no button). Search with no match: "No installed RomM games match "…"".

### RomM › Discover

- What Server › Discover showed, unchanged (hero and shelves; "Recently added · on RomM"). Arriving on the Library while Discover is selected focuses Discover's content (the cover last focused there, else the first), not the rail. Y searches everything. No RT.
- **While the startup probe runs:** a panel "Connecting to romm.home.lan…" with a spinner, "Discover and the platforms show when RomM answers. Installed games play now." and skeleton covers. No hints but Menu. If RomM answers, Discover fills in; if not, the selection falls back to Installed with the banner and the card "Can't reach RomM" / "romm.home.lan didn't answer. Installed games still play." (View "Retry").

### RomM › a platform

- Server's platform view, moved: header "PlayStation" and "3 games on RomM · 1 installed" (Sega Saturn: "2 games on RomM · 1 installed · plus 1 only on this computer"; while a search narrows it, "2 of 3 games match"). The first number is the rail row's count; installed games RomM doesn't have follow a "plus" and are not counted in it, as on desktop; the emulator chip ("Emulator: **DuckStation**", or "No default emulator" in warning) and the firmware chip ("Firmware: 3 of 4 files", "Firmware: 2 files needed" in warning, "Firmware: 3 files · installed", or a plain "No firmware needed"); A on either opens the system's page ("Open in Systems"). Windows and Linux have no chips. Then **Backup all 3…**, RT Select, Y Search.
- The grid merges RomM's games with the installed rows: installed covers show the installed dot (X Play), the rest X Install. Installed rows RomM doesn't have show here with a cover tag (decision 15), and never in Discover:
  - **NOT LINKED**: no RomM link (Brass Comet, Sega Saturn).
  - **NOT ON ROMM**: from another RomM server, or removed from this one (Sky Lantern, Super Nintendo). Its game page's left column starts "From another RomM server (romm.old.lan) · not on romm.home.lan".

### RomM › Emulators (2026-11-15)

The server's Emulators platform as a RomM row, as on desktop (RommStorefront README, "RomM › Emulators": same rules, data and words; TV's own focus, hints and A-says-why lines below).

- **The row:** "Emulators" with the count (emulators plus other files this computer can install; 8 in the prototype, 0 when nothing is for this computer), sorted by name among the platform rows (first, before GameCube, in the prototype). Only while RomM is Connected and the server has an Emulators platform with files; otherwise no row, and a selected row hands selection and focus to **RomM › Installed** (the existing fallback).
- **Header:** "**Emulators**" and "5 emulators · 3 other files · 3 installed" ("2 of 8 items match" while a search narrows it; "Loading…"; "On romm.home.lan"; "No builds for this computer"). Chips: "Builds for **Linux x64**", the search chip while searching, "(Y) Search". No firmware or emulator chip, no Backup all, no RT Select, no LT Sort.
- **The list** (a vertical list of rows; Right from the rail or A on the row enters it at the first row or where focus was):
  - **EMULATORS** heading, note "Named for GRID. Each installs as an emulator that Settings › Systems sets up, and RomM's newer builds show as updates." Rows: a monogram tile, the name, the line "PlayStation 2 · v2.0.2 · 38 MB", the status on the right.

    | State | Line | Right side | A | X |
    | --- | --- | --- | --- | --- |
    | Not installed | system · version · size | — | Install | — |
    | Not installed, catalog copy installed | + "GRID's catalog copy is installed. This build installs beside it as "DuckStation (server)"." | — | Install | — |
    | Queued | as above | "Queued" | Cancel | — |
    | Downloading | as above + a bar | "46%" | Cancel | — |
    | Installing (checking the program) | as above | spinner "Installing…" | dimmed, no A; A says "GRID is checking PCSX2's program. It finishes in a moment." | — |
    | Installed | as above | "Installed" (ok) | Open in Systems (the system's page; RetroArch and Pico-8 open Settings › Systems) | Uninstall |
    | Update available | "PlayStation 2 · installed v1.6.0 · v2.0.2 is on RomM" | "Update available" (warning) | Update to v2.0.2 | Uninstall |
    | Install failed (wrong processor) | + red "Couldn't install: its program is built for Linux ARM64, not Linux x64. Check the file's arch tag on RomM." | "Didn't install" (warning), warning edge | Retry | — |

  - **OTHER FILES** heading, note "These don't follow GRID's naming, so GRID installs them as plain files in your library's Emulators folder. GRID doesn't set them up for a system or check them for updates." Rows: a plain tile, the file name in mono, "<folder> folder · <size> · <why>" (desktop's whys). Not installed: A Install. Queued / Downloading / Installing as above. Installed: "Installed" (ok), line + "In your library's Emulators folder", A Uninstall.
  - **NOT FOR THIS COMPUTER** heading, one row **Other builds** · "For other computers, or older. GRID doesn't install these here." · the count and a chevron; A Open. It opens a page "**Other builds**" · "On RomM's Emulators platform. GRID installs the newest build for this computer's system and processor (Linux x64), so it doesn't install these here." with one dimmed row per build (file name, "<folder> folder · <size>", the reason on the right: "For Windows x64", "Older than v2.0.2"…). The rows take focus but have no A hint; A says "For Windows x64. GRID installs builds for Linux x64 on this computer." B Back. Hidden while searching.
- **Confirms** (Cancel focused): "**Uninstall PCSX2 v1.6.0?**" / "GRID deletes it from the emulators folder in your library. Games and saves stay. PlayStation 2 games set to it can't start until another emulator is picked." · **Uninstall** (danger); an other file: "**Uninstall xemu-v0.8.5-x86_64.AppImage?**" / "GRID deletes it from the Emulators folder in your library." Cancel during a download: "**Cancel the PCSX2 install?**" / "The download stops and its files are removed. You can install it again later." · **Cancel install**.
- **Cards:** "Pico-8 v0.2.7a6 installed from RomM" / "Set up for Pico-8 in Settings › Systems" (RetroArch: "… for its systems …"); "PCSX2 updated to v2.0.2"; "xemu-v0.8.5-x86_64.AppImage installed" / "In your library's Emulators folder"; "PCSX2 uninstalled"; "Cancelled PCSX2"; error "Couldn't install DuckStation" / "Its program is built for Linux ARM64. Nothing was set up."
- **Whole-view states:** Loading: panel with a spinner "Loading the Emulators platform from romm.home.lan…" and skeleton rows; focus stays on the rail; no hints but Menu. Can't load: panel "**Can't load the Emulators platform**" / "romm.home.lan didn't answer. Installed emulators still work." with **Retry** (A Retry). Nothing for this computer: panel "**Nothing here for this computer**" / "RomM's Emulators platform has 6 builds, all for other systems or processors. GRID installs builds for Linux x64." then the Other builds row. Search no match: "No emulators or files match "…"" · "Press (Y) to change the search."
- **Hints:** the focused row's A and X (as in the table), Y Search ("Search Emulators" keyboard), Menu. No LT, no RT.
- Emulator files are not games: never in RomM › Installed, All games, Recent, Updates or Discover, nor their counts.

## Settings

- **Rail (alphabetical):** Appearance, Backup drives, Cloud saves, **Game stores**, Library, RetroAchievements, Systems, TV mode, Updates. **Connection is gone**; Stores is renamed **Game stores** and moves to its place by name.
- **Game stores:** "Each one you connect gets a group in the Library, in this order." then one row per storefront in name order, RomM between GOG and Steam:
  - RomM row: the mark, "RomM", the line "Signed in as sam · romm.home.lan" ("Not signed in · romm.home.lan", or "Your own game server" when no server was ever stored) and the pill **Connected** / **Can't reach** / **Sign-in expired** / **Not connected** (or a spinner with "Connecting…", also while it reconnects). A opens the RomM page.

### The RomM page

A page of rows (`rowsPage`), title "RomM" with the mark; the sub line names the state. Start presses the main button where there is one.

| State | Sub line | Note | Rows | Main button (Start) |
| --- | --- | --- | --- | --- |
| Connected | "Signed in as **sam** · romm.home.lan" | — | Server, User, Sign-in ("API token · kept in the OS keyring"), Installed RomM games, **Edit sign-in…**, Show in Library, Open in browser, **Disconnect…** | none |
| Can't reach | "Signed in as **sam** · can't reach romm.home.lan" | warning: "**Can't reach RomM.** romm.home.lan: connection refused. Installed RomM games still play; browsing, installs, cloud saves and firmware downloads wait until it's back." | as connected | **Reconnect** → "Reconnecting…" (disabled: "GRID is trying RomM now.") |
| Sign-in expired | "Sign-in expired · romm.home.lan" | warning: "**Your RomM sign-in expired.** Sign in again to browse, install and sync saves. Installed games still play." | as connected | **Sign in again…** |
| Not connected | "Not signed in · romm.home.lan" ("Your own game server" when none is stored) | "Connect your RomM server to browse its games in the Library, install them with an emulator, and sync their saves and firmware. The 12 RomM games installed here still play." (the last sentence only while some are installed) | Server and User (when one is stored), Installed RomM games, Show in Library (when any), Open in browser (when a server is stored) | **Connect RomM…** |

- **Edit sign-in…** ("Server, user and token") opens the sign-in page filled in.
- **Open in browser** ("Opens romm.home.lan in your web browser, over TV mode"): card "Opening romm.home.lan in your browser…" / "It opens over TV mode. Close it to come back."
- **Disconnect…** ("Installed RomM games stay", danger) asks "**Disconnect RomM?**" / "GRID deletes your RomM sign-in from the OS keyring and stays open. Installed RomM games stay and still play. Cloud saves, firmware downloads and updates stop." with Cancel (focused) and **Disconnect**. Then card "RomM disconnected" / "Installed RomM games stay and still play." GRID stays where it is; the page shows Not connected (server and user kept for the next connect), and the Library group shows Installed only with **off** while RomM games remain.
- **Reconnect** cards: "Reconnecting to RomM…" / "romm.home.lan", then "Connected to RomM as sam" / "romm.home.lan", or "Can't reach RomM" / "romm.home.lan didn't answer. Installed games still play.", or (a 401) "RomM sign-in expired" / "Sign in again to browse, install and sync saves. Installed games still play." (View "Sign in again…").

### The RomM sign-in page (the TV RomM form)

Opened by Connect RomM…, Sign in again…, Edit sign-in…, the Installed banner, the needs-RomM panels and the welcome's RomM row. A page with the RomM mark.

- Titles: "Connect to RomM" / "Sign in to RomM again" / "Edit RomM sign-in". Sub lines: "GRID browses your RomM server, installs its games with an emulator and syncs their saves." / "RomM no longer accepts GRID's token for sam. Enter a new one." / "Change the server or the sign-in. Installed RomM games stay; GRID links them to the server's games when it connects."
- Fields (A "Edit" opens the on-screen keyboard): **Server** ("https://…"; keyboard "RomM server address", URL kind), **Sign in with** (a list: **API token** "Recommended" / Username and password), then **API token** ("Paste or type your token"; masked, opens empty) or **Username** ("Your RomM user") and **Password** ("Your password"; masked). Server and user are filled in from the stored values; the secret never is. Sign in again opens on the token field.
- Footer: "Make a token in RomM under your user settings. GRID keeps it in the OS keyring, never in a file." (password: "GRID keeps your password in the OS keyring, never in a file.") and **Connect** (Edit: **Save**) with the Menu glyph; Start presses it. Disabled until filled; A or Start then says "Enter your RomM server address first." / "Enter your RomM username first." / "Enter your API token first." ("password").
- **View pastes** the clipboard into the focused field: card "Pasted from the clipboard", or "Nothing to paste" / "Select the Server or token field first.". Hints: A Edit, View Paste, Menu Connect, B Back.
- While signing in: "Connecting…" with a spinner, disabled ("GRID is signing in to RomM.").
- Errors, in `danger` above the fields, and focus moves to the field to fix: "RomM rejected this token (401). Check it, or make a new one." (token emptied; focus on the token) / "RomM rejected the username or password (401)." / "Couldn't reach https://romm.home.lan. Check the address, and that the server is running." (focus on Server).
- Success: card "Connected to RomM as sam" / "romm.home.lan". From the RomM page: back to the page, or **Choose your systems** when setup is needed (decision 9). From the welcome: back to the welcome, whose RomM row now reads Connected.

### Cloud saves and RetroAchievements: needs RomM

While RomM isn't connected, each pane starts with a panel under its heading (warning-edged unless RomM is simply not connected). Its button is focused first. The words are desktop's: a heading, then why, then what still works.

- **Cloud saves:** "**Cloud saves need RomM**" · "<why> GRID keeps your emulator saves and states on your RomM server. These settings apply when it's connected. GOG and Epic games sync with their own store's cloud."
- **RetroAchievements:** "**Achievement progress needs RomM**" · "<why> Details › Achievements shows your progress from RomM. Signing in here still turns on achievements in RetroArch, DuckStation and Dolphin."
- `<why>`: "RomM isn't connected." / "GRID can't reach romm.home.lan right now." / "Your RomM sign-in expired." / "GRID is still connecting to romm.home.lan."
- Button (TV's own): **Set up RomM…** (not connected; opens the RomM page), **Sign in again…** (expired), **Retry** (can't reach; "Retrying…" while it runs).

- The rows below stay editable: Cloud saves' preferences apply once RomM is connected; the RetroAchievements login still goes to the emulators. Save then notifies "RetroAchievements login saved" / "Sent to 3 emulators" ("… and your RomM account" while connected).

### Appearance › Open GRID on

"Here and in desktop mode". Options: **Automatic: Discover when RomM is connected, else Library**, **Discover (when RomM is connected)**, **Library**. The stored values (`automatic`, `discover`, `library`) don't change. RomM not connected and the choice isn't Library: the line reads "Here and in desktop mode. RomM isn't connected, so GRID opens on All games."

## Where GRID opens

- **Automatic:** Library › RomM › Discover when RomM is connected at startup (the config says so; the probe runs after the shell is up), else All games.
- **Discover:** Discover when RomM is connected at startup, else All games.
- **Library:** All games.
- **Never connected** (decision 16): All games, under any setting, not the first Backup drive.
- If the startup probe then fails, the selection falls back to RomM › Installed (with the banner), or All games when the group is gone. A restart lands where the setting says, never on the row focused before it.

## The welcome (first run; replaces Connect)

A startup screen (the box fits between the bars; the top bar shows only the wordmark and the clock). Start opens the short Menu (Switch to desktop mode, Quit GRID), as on Connect before.

- **Step 1 of 2 · Welcome to GRID:** "Choose where GRID keeps your games, emulators and saves. A folder with plenty of free space is best." One field, **Library folder** ("Choose a folder"; A "Choose" opens the folder browser, Y "Use this folder"), the line "GRID makes games, emulators and saves folders inside it." and **Next**, disabled until a folder is chosen ("Choose a library folder first."). Hints: A, Menu "Menu".
- **Step 2 of 2 · Connect your games:** "Optional. Each one you connect gets a group in your Library. You can connect more later in Settings › Game stores." Five rows in name order, each with its mark, line and pill (**Connected** / **Not connected**); A "Connect" (connected: "Change"):
  - Amazon Games "Your Amazon Games library. Sign in on Amazon's page." · Epic Games "Your Epic Games library. Sign in on Epic's page." · GOG "Your GOG library. Sign in on GOG's page.": A opens the store's login window over TV mode (TVStores).
  - RomM "Your own RomM server: its games, cloud saves and firmware.": A opens the RomM sign-in page.
  - Steam "Games in Steam on this computer. A Web API key lists the rest.": A opens Steam's page (TVStores; Connect Steam with Start).
  - Connected lines: "Signed in as sam_plays", "sam · romm.home.lan", "Steam account SamPlays". A on a connected row opens a list: "Disconnect <store>" (RomM also "Edit sign-in…").
  - The button reads **Skip for now** until one is connected, then **Continue**. B goes back to step 1. Hints: A, B Back, Menu "Menu".
- **No emulator step without RomM** (plan 0.11). With RomM connected and setup needed, **Choose your systems** follows Continue; then the shell. Without RomM, Continue / Skip goes straight to the shell (Where GRID opens: a fresh profile lands on All games).
- **Python import:** step 1 shows "Filled in from your old GRID settings." with the folder filled in, and a dialog first: "**Settings imported from your old GRID**" / "Your library folder, RomM server and username are filled in. Sign in to RomM again: GRID doesn't copy tokens or passwords." (OK). Step 2's RomM row reads "From your old GRID: sam · romm.home.lan. Sign in again.", and its sign-in page has the server filled in.

## Choose your systems (after a RomM connect)

TV mode's page, with: the rows are the systems with games on RomM, plus Windows ("Proton for Windows games"); each row's line is "<n> games on RomM". From the RomM page the sub line starts "RomM is connected. ". Install <n> emulators / Skip for now (Start) as before. **From the RomM page, Install and Skip return to Settings › Game stores with the RomM row focused** (the setup card follows the installs there); from the welcome they go on to the shell. No Disconnect footer.

## Settings › Systems without RomM

- Header buttons as before: **Install recommended (n)** (your systems only), Check for updates, Add emulator. Without RomM a line: "Systems works without RomM. With RomM connected, its platforms join this list."
- **Your systems:** systems with an emulator, installed games, or (with RomM) games on RomM; no-emulator rows first, then the rest, then Windows (Proton). Row lines: "DuckStation · 3 on RomM, 2 installed" (connected) / "DuckStation · 2 installed" / "No emulator yet. Ymir is recommended. · 2 installed". Status pill: Ready, No emulator, Firmware needed, Update available, Installing…. X: Change emulator / Install <recommended>.
- **More systems** (a row with the count) opens a page "More systems" / "Systems GRID can set up from its own list. Set one up now, or when you have games for it." with the catalog systems that have an installable emulator and nothing set up (Dreamcast, Game Boy Advance, Nintendo DS, PlayStation Portable, Sega Genesis, Xbox in the prototype). A opens the system's page; X installs the recommended emulator, after which the system moves to Your systems. (Decision: the full catalog list is long on a TV, so it sits one page down.)
- Other emulators as before.

### The firmware block (a System page)

After the emulator rows, a **Firmware** heading, then one information row per expected file, then a note, then two action rows:

- File rows: the name; the line "512 KB · required" / "· optional" (key files: "Different on every console, so GRID can't check it"), plus "· checked against RomM" or "· checked by GRID" once installed; the value **Installed**, **Installed · not verified**, **Missing · required** or **Missing**.
- Notes: "GRID checks each file's size and MD5 before it goes into DuckStation's folders." / for key files "GRID checks every file it knows by size and MD5. Key files are different on every console, so they install as not verified." / PS3 "RPCS3 installs the PlayStation 3 system update file, PS3UPDAT.PUP. GRID checks it first." / no emulator "Install an emulator for Sega Saturn first. GRID puts firmware in its folders."
- **Install from RomM** — line "Downloads 2 files from romm.home.lan and checks each one". Dimmed (no A hint; A says why) with the line and reason:
  - "Needs RomM" — "Install from RomM needs RomM. Connect RomM in Settings › Game stores." (expired / unreachable: "Your RomM sign-in expired. Sign in again in Settings › Game stores." / "Can't reach RomM right now. GRID tries again when you press Retry.")
  - "Not on your RomM" — "Nintendo Switch isn't on your RomM server."
  - "Your RomM has no firmware for it" — "Your RomM has no PlayStation 3 firmware."
  - "Every file is installed" — "Every PlayStation 2 firmware file is installed."
  - "Needs an emulator first" — "Install an emulator for Sega Saturn first. GRID puts firmware in its folders."
  - Running: "Installing PlayStation 2 firmware from RomM for PCSX2…", then "PlayStation 2 firmware installed for PCSX2" / "2 files · checked against RomM".
- **Locate files…** — "Pick files on this computer or a drive. GRID installs the ones it knows." (no emulator: dimmed, "Needs an emulator first").
- **Locate PS3UPDAT.PUP…** (PlayStation 3, in place of Locate files…) — "Pick the update file on this computer or a drive".
- The page's sub line and the Systems row follow: "Firmware needed" until every required file is installed, then "Ready".

### The TV file browser in file mode (Locate files)

TV mode's folder browser page with picks: title "Locate PlayStation firmware", sub "For DuckStation. GRID checks each file and puts the ones it knows in DuckStation's folders.", the path, then a line "No files picked. Pick files with (A), or check this whole folder with (Y)." / "4 files picked: scph5501 (1).bin, scph1001.bin, BIOS pack.zip, readme.txt". Rows: Up one folder, folders (chevron), files (a tick box and the size). A opens a folder or picks / unpicks a file (hint "Open" / "Pick" / "Unpick"); picks are kept across folders. **Y "Check this folder"** checks the folder shown, three levels deep (regular files, links not followed). **Start "Check <n> files"** (hint only once something is picked; with nothing picked Start says "Pick files first, or press Y to check this folder."). B goes back. It starts on the last-used drive folder in the prototype (`/run/media/USB-1TB/bios`).

The PUP picker is the same page in single mode: no ticks, A "Choose" on a file. A wrong file: dialog "**Can't use this file**" / "scph39001.bin isn't a PlayStation 3 system update. Choose PS3UPDAT.PUP."; a PUP that doesn't match: "This isn't the PS3UPDAT.PUP GRID knows (size or MD5 differs). Download the update again." (buttons Choose another file… / OK, OK focused). A good one: "Checking PS3UPDAT.PUP…", "Installing the PS3 firmware in RPCS3…" / "This takes a minute", then "PS3 firmware installed in RPCS3" / "PS3UPDAT.PUP · size and MD5 match".

### The Locate report

After the check ("Checking 5 files…" / "PlayStation firmware" or the folder), a card "2 firmware files installed for DuckStation" / "1 not verified · 3 refused" (or the error card "No firmware installed" / "4 files refused", "Nothing matched PlayStation firmware"), and a page **Firmware check**: "Checked 5 files you picked for PlayStation · DuckStation" ("… in /run/media/USB-1TB/bios for …"). Groups, each counted:

- **Installed · n** — the known name, the line "From scph5501 (1).bin · size and MD5 match" (a renamed dump lands under its known name) or "From BIOS pack.zip · size and MD5 match" (an archive's member), pill **Installed**.
- **Installed · not verified · n** (decision 13) — "GRID can't check this file: it's different on every console", pill **Installed · not verified**.
- **Refused · n** — pill **Refused**, with the reason: "This isn't the scph1001.bin GRID knows (size or MD5 differs)", "GRID doesn't know this file for PlayStation", "Nothing in it is PlayStation firmware GRID knows" (an archive), "Over 2 GB, so GRID didn't read it".
- A folder check counts the rest, not listing them: "10 other files matched nothing GRID knows for Nintendo Switch."
- Nothing installed: warning note "Nothing was installed. The PlayStation 2 page lists the files PCSX2 needs."
- Then **Locate more files…** and **Done** (Start), which returns to the System page with the files marked.

## Details: needs RomM

- **Update** and **Link to a RomM game** (renamed from "Link to server game") are disabled while RomM isn't connected; A says "Updates need RomM. <fix>" / "Linking needs RomM. <fix>" (the fix as in the Installed empty text). An unlinked game's left column reads "**Not linked to a RomM game.** Link it to play."
- The **Saves** and **Achievements** tabs show the needs-RomM panel instead (desktop's words): "**Cloud saves need RomM**" · "<why> This game's saves stay on this computer and still work; GRID uploads and restores them when RomM is back."; "**Achievements need RomM**" · "<why> Your progress shows here from RomM; achievements still unlock in the emulator." (`<why>` as in Cloud saves above). Its button (Set up RomM… / Sign in again… / Retry) takes focus when the tab opens.
- **Backup to drive…** stays enabled without RomM (decision 8); Install and Play are unchanged (a game that isn't installed is reachable only through Discover and the platforms, which need RomM).

## What this replaces

In `TVMode/README.md` (each marked there and pointing here):
- The shell: the four tabs and the top bar's connection dot and user; the hint bar's offline notice.
- Tabs: Server › Discover, Library's **Platforms** folder, Server, and Settings' Connection pane.
- Pages: System's firmware Install ("from your server"); Link to server game; Folder browser's "Connect's library folder".
- Overlays: Menu's Reconnect and its "user and server" head.
- Startup screens: Connect (replaced by the welcome); Choose your systems' rows.
- Game backups: "Use without a server", and Server's "Backup all N…" (now in RomM › a platform).
- Decided: **Where GRID opens**, **No Home tab** (Discover is now a Library row), **RT is select mode on the Library and Server grids** (now Library only), **Leaving TV mode from the startup screens** (the welcome takes Connect's place), and the Consultant decisions on Menu › Reconnect, "Connect to a server…", the results page's offline panel and Settings › Connection's Edit connection.

In `docs/tv-mode/checklist.md` (81 items, and a note at the top): items 89 and 104 (Server header chips, arriving on Server); 121, 125, 136, 137 (top bar and hint bar); 157–175 (section 6, Server › Discover: kept, moved under Library › RomM); 176 and 191 (Library rail and empty text); 196–208 (section 8, Server platforms: moved under RomM, offline panel gone); 225, 234, 243, 244, 252 and 255 (Settings rail, Open GRID on, Connection, Systems); 326 and 331 (System page and its firmware row); 349 (folder browser's Connect use); 363–365 and 367 (Menu); 392–399 (Connect); 400–401 and 407–408 (Choose your systems' rows and where it returns); 413–417 (Where GRID opens); 437, 448, 456 (backups refused offline); 515–520 (Use without a server); 525–526 (desktop's Where GRID opens). Each is marked "Superseded (RomM storefront, TVRommStorefront)" there.

## Notes for implementers

- The prototype's states come from one `RommStatus` (plan 0.2): `connected`, `unreachable`, `expired`, `not_connected`, `busy`, `installed_count`, `in_rail`. The rail group is `rommRailGroup` (0.8); the TV Library uses the same `STOREFRONT_ORDER` as desktop.
- **Startup probe (decision for Step 1):** while the startup probe runs with `stores.romm.connected` true, the group shows Installed and Discover, Discover shows "Connecting to RomM…", and the heading flag and the RomM pill show a spinner. Platform rows appear once RomM answers. `rommRailGroup` needs to treat busy-at-startup like this, or Discover can't be where GRID opens.
- Selection fallback and focus: when the selected row goes away, select and focus RomM › Installed (or All games); never let a redraw focus a rail row by position, which would change the selection.
- **Folders (2026-11-16):** `lib.folds.drives`, `storeState` `folds` and `tvRomm.open` give way to the one shared fold store the desktop card describes (`[ui] rail_closed`). `TvRailRow`'s `holdsSel` already routes entry; it now also draws the selected look, and a closed heading gets its count. A wrapper element per folder draws the panel; it must not carry `data-nav`, so movement stays the rail's. `selectDriveRow`, `openStoreFold` and the RomM jumps open through the shared store; `openOn`'s startup selection must not open RomM.
- The Installed banner and the needs-RomM panels are focusable and reachable with Up from the grid / first in the pane; their words come from `romm/words.ts`, shared with desktop. Desktop's words won where the two cards differed (decision 17); this card now uses them, except TV's own buttons, hints and the A-says-why lines.
- Firmware rows: dimmed action rows, A says why (no A hint). The file list is `firmware_expected`; the report is `LocateReport` (installed / unverified / refused). The TV file browser's file mode is new: multi-pick with ticks, Y checks the folder, Start checks the picks; single mode for the PUP.
- RomM › Emulators: the rail key is the platform's `platform:<slug>` (`platform:emulators` in the prototype); `rommRailGroup` keeps the Emulators platform (plan 0.3 dropped it) with its own count. Source: `tvromm_emu.js` / `tvromm_emu.css`. Open questions are listed in the desktop card's README.
- Prototype artefacts, not design: the store groups keep the TVStores card's sample installs in the fresh-profile welcome (so they show **off**), and the OSK's key rows are wider than the stage at 1280×800, as in TV mode.
