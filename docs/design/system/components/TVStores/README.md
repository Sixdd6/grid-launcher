TV mode with game stores: the Game stores card (GOG with its offline installers, Epic Games, Amazon Games, Steam) for a controller. It is the TV mode card plus the stores, and opens on Library › GOG › Installed with a GOG install running, one queued and a Steam install that Steam runs. The words, rules, states and notifications are the **GameStores** card's; only the controls change. Everything not listed here matches TV mode.

**Status: proposed, not built.** Decisions: `docs/external-stores-plan.md`.

## Try it

Click inside the card, then use the keyboard as a controller (TV mode's README has the full map: arrows, Enter = A, Esc = B, X, Y, Q / E = LB / RB, Z = LT, M = Menu).

- **Library:** the rail's store folders are at the bottom (Amazon Games, Epic Games, GOG, Steam). Moving onto Installed or Available shows that list.
- **Menu › Prototype only** adds: **Show: a store state…** (GOG login expired or not connected, Epic loading or legendary missing or failed, Amazon connected, Steam without a key, private, key rejected, client missing, and back to the start), **Show: Settings › Stores**, **Show: GOG offline installer**, **Show: sign in to Amazon Games**.
- **GOG backups (2026-11-13):** in GOG › Installed, RT turns on Select; A picks covers, X selects all, Y opens the Backup page (Start backs up). **Backup all 3…** sits in the header and **Backup to drive…** on an installed GOG game's page. Library › Couch kit lists Marrowind Keep and Paper Lanterns after its RomM games, with a GOG tag; X on them is Install from drive.
- **RomM installer archives (2026-11-13):** Server › Windows: X on Grey Meridian (silent), Foxglove (setup window), Iron Lullaby (unrecognised) and Low Tide Radio (DLC without Pale Summit); Server › Linux: Wren Hollow (.sh). Follow them in Downloads.
- **Store pages:** Settings › Stores, then a store. Steam's Test cycles through its results (the row says which comes next).

## Library

- **A folder per store** in the rail, like Backup drives: the store's mark and name (A opens and closes it), with **Installed** and **Available** under it and their counts. The folder shows **!** when the store needs attention and **off** when it isn't connected but still has games installed. Available's count is a spinner while the owned list loads and **!** while it can't load.
- **Header (two rows, like Server's):** "GOG · Installed" and the count line; then the chips ("Signed in as sam_plays", "Cloud saves: on", or a focusable **Login expired · Sign in again**), **Install from installer…** (GOG), **Refresh** (Available) and the Y Search chip. Up from the grid reaches them.
- **Covers:** the installed dot; one top-left tag (NOT SUPPORTED, SAVE CONFLICT or UPDATE, in that order); while installing, the percentage tag and a bar along the bottom edge (teal for Steam). In All games, Recent and Updates the store games carry a store tag at the bottom left.
- **A** opens the game's page. **X** is the quick action: Play, Install… (opens the install page) or Install in Steam; none while a job runs or for a Not supported game. RT (Select) isn't offered in store views.
- **States in place of the grid** are panels with the store mark, a title, one line and focusable buttons: loading (spinner and skeleton covers), nothing installed (Open Available, Install from installer…), everything installed (Refresh), login expired (Sign in again…), not connected (Connect…), store tool missing / installing / failed (Install legendary (14 MB), Installing…, Try again), Steam without a key / rejected key (Open Settings › Stores), private game details (Refresh, Open Settings › Stores), Steam not installed (Check again).

## The game page

- **Buttons:** Play · Install… · Install in Steam · Cancel install · 45% · Installing in Steam · 52% (disabled; A on it says "Managed by Steam. Pause or cancel it in Steam.") · Not supported (disabled; A says why) · Update to 2.2 · Game settings · Verify and repair (Verify in Steam) · Uninstall (confirmed; Steam: Uninstall in Steam…, which opens Steam's window).
- **Left column:** last played, how it runs, the cloud state ("Cloud save conflict. See Cloud saves."), "Installed from an offline installer", "Installed in Steam, outside GRID", for Epic, Amazon and Steam games why they can't be backed up (the desktop's words), for a GOG game "On Couch kit" when a connected drive holds it, and for a Steam install the Managed by Steam box with its bar.
- **Tabs (LB / RB):** Overview (description, **DLC you own** rows: A installs or, confirmed, removes; facts), Media, **Cloud saves** (GOG and Epic, installed): a Synced row whose A is Sync now; on a conflict, two rows, **Keep this computer's save** and **Use the cloud save**, each confirmed; off ("Turn them on in Game settings"); the save location; "RomM cloud saves are off for store games."
- **Play:** "Launching Ashen Tide…" with "Synced cloud saves with GOG", then the Playing page ("GOG · Native"); Stop game syncs after ("Cloud saves uploaded to GOG for Ashen Tide"). A conflict opens a list first: Keep this computer's save, Use the cloud save, Don't launch. Steam: "Starting Moonlit Ferry through Steam…", and the Playing page has no Stop ("Steam runs this game. Quit it from the game; GRID notices when it closes.").
- **Install page** (the desktop's install dialog as a page): Version (Linux recommended / Windows) as radio rows, Compatibility tool and Language (lists), Install folder (the folder browser), **DLC you own** switches, the footer "22.0 GB to download · 412 GB free on /home" and **Install**, which Start presses from anywhere.
- **Game settings page:** Install folder, Launch file ("Set by GOG" / "Your choice"), Launch parameters (keyboard), Compatibility tool and Wine prefix (Windows version) or "Linux games run directly.", Cloud saves switch, Save (Start).

## Settings › Stores

- A pane between RetroAchievements and Systems: one row per store with its mark, "Signed in as …" (or "Not signed in", "SteamID64 …") and a status (Connected, Login expired, Not connected, Installed games only).
- **A store's page:** Games you own, Store tool (or a warning row: Install / Try again), License (the source link opens in desktop mode), Install folder (folder browser), Cloud saves switch, Show in Library, Install from installer… (GOG), Refresh games, **Disconnect…** (confirmed with the desktop's words). Login expired adds the warning note and **Sign in again…** as the main button (Start). Not connected shows what connecting does and **Connect GOG…** (Start); GOG keeps Install from installer….
- **Steam's page:** Steam client (found, or not installed), **Account** (a list of the accounts found in Steam's files plus **Enter a SteamID64…**, which opens the keyboard), **Web API key** (the keyboard, masked; the row shows the last four characters), **Test**, Show in Library, Disconnect…, and **Save** / **Connect Steam** (Start). Results come as notification cards with the desktop's words (works · N games owned; rejected (403); game details private, with the privacy path; not a SteamID64; no network).

## Signing in

- The store's login window opens over TV mode: GRID's title bar "Sign in to GOG — GRID", the read-only address with a lock, the store's own page inside (the card shows a placeholder with stand-ins for its fields and button), and the footer with Cancel. Focus lands on the page's first field.
- The d-pad moves between the page's fields and buttons; **Y opens the on-screen keyboard** for the focused field (A on a field does too), and Done types into the page. A physical keyboard types straight in. The hint bar reads A Select · Y Keyboard · B Cancel sign-in.
- Success closes the window, then the same cards as the desktop (connected, tool installing, games found). A network error shows inside the window; a failed sign-in closes it with an error card and **Try again** (View). B cancels silently.

## Offline GOG installer

- From the GOG header, the empty GOG Installed state, the GOG page in Settings, or the Menu. It opens TV mode's folder browser in ~/Downloads, files only; A on a file chooses it ("Reading the installer…").
- A recognised installer opens a page with the desktop's words: what it is, how it installs (unpacked, or run silently with the setup window as a fallback, which needs a mouse or keyboard over TV mode), Compatibility tool, Install folder, and **Install** / **Install DLC** (Start).
- A refusal opens a dialog with the desktop's words and **OK** / **Choose another file…**.

## Downloads and notifications

- Store rows sit in Active, Queued and Completed with a store badge and kind badge, and the desktop's lines ("Installing in Steam 67% · Managed by Steam", "Waiting for the installer's setup window · finish it there", "Failed · legendary stopped: …"). A on a row opens a list: Cancel (not for Steam rows: the list's subtitle says Steam manages them), Retry, Dismiss, Open <game>. Y clears completed rows, store rows too.
- Notification cards are the desktop's; those with an action (Play, Open Downloads, Try again) take it with View.

## RomM games with a GOG installer in their archive (2026-11-13)

The desktop's design with a controller (GameStores README). The game stays a RomM game.

- **Game page:** the left column adds "Its archive holds a **GOG Windows installer**. GRID runs it after the download." (after install: "Installed by running its **GOG installer**").
- **Downloads:** the row reads "Windows · Downloading · 45% · 38.2 MB/s", then "Windows · Running the GOG installer silently under GE-Proton9-20 · 40%" (Linux: "Unpacking the GOG installer 40%"), and "Windows · Installed · ran its GOG installer" when done. The cover shows the percentage while it runs.
- **Wizard fallback:** "Waiting for the installer's setup window · finish it there", and the card "The Foxglove installer couldn't run silently — Its setup window opened over TV mode. Finish it with a mouse or keyboard; GRID adds the game when it closes."
- **Failures:** the row fails with the desktop's words; the card is an error with **Install Pale Summit** (DLC without its base) or **Open Downloads** (unrecognised installer), taken with View. A on the failed row offers Retry and Dismiss.

## GOG games on backup drives (2026-11-13)

The desktop's design with a controller (GameStores README). Epic, Amazon and Steam games still can't be backed up.

- **Select mode (RT) in GOG › Installed:** A picks a cover (check and ring), X selects all or clears, Y opens the Backup page, RT or B leaves. The bar under the grid reads "2 selected · 21.4 GB to write". The header has **Backup all N…** and the RT Select chip. An installed GOG game's page has **Backup to drive…**.
- **Backup page:** the lede, **Drive** (radio rows with the free space), **What goes on the drive** (a row per game: "GOG · brightwater_1_3_58810.sh · 2.9 GB, and its saves" with the **Installer files** tag, or "Installed with gogdl, which keeps no installer…" with **Installed copy**), the Windows prefix note, skipped and refused games as notes, and the main button (Start): Backup N games, or, disabled, Not enough space / Nothing to backup. A dashed prototype row moves Brightwater's installer away.
- **Downloads:** Backup rows ("Copying from this computer to SAMSUNG T7 45%", then "Copied from this computer · Written to Couch kit") and the two cards.
- **A drive's grid:** its GOG games after its RomM games, with a GOG tag; the header count and the rail count include them, and a chip reads "GOG: 2 games". X is **Install from drive** (Play when installed here); A opens the game's page with "From Couch kit" in the header line, **Install from drive**, **Remove from drive…** (confirmed) and how it restores.
- **Restoring:** a store row "Copying the installer from SAMSUNG T7 30%" then "Running the installer · 60%", or "Copying from SAMSUNG T7 45%"; done "Installed from SAMSUNG T7 · ran its installer · saves restored" / "Copied from SAMSUNG T7 · saves restored", and the card "<game> installed from Couch kit" with Play.

## Inventory checklist

- [ ] Rail folders with ! / off flags, spinner and ! counts
- [ ] Two-row store header, focusable chip and buttons
- [ ] Store covers: tags, progress tag and bar, store tag in All games
- [ ] X quick action labels per state
- [ ] Every grid-area state panel (loading, empty ×2, expired, not connected, tool ×3, Steam ×4)
- [ ] Game page for installed, available, installing, Steam-installing, Not supported, offline-installed
- [ ] Cloud saves tab: synced, syncing, conflict (two confirmed rows), off, none
- [ ] Conflict list before launch; Playing page for a store game and for Steam
- [ ] Install page, Game settings page
- [ ] Settings › Stores pane; store page connected / expired / not connected; Steam page; Disconnect confirms
- [ ] Login window: loading, page, keyboard over it, network error, closing; failed sign-in card
- [ ] Folder browser for installers; recognised page (Linux, Windows, DLC); refusal dialogs
- [ ] Downloads store rows and their A list
- [ ] Prototype-only Menu entries reach every state
- [ ] RomM installer archives: game-page line, download and installer phases, wizard wait and card, done, both failures with their card actions
- [ ] GOG select mode (RT, A, X, Y, bar, header chip), Backup all N…, Backup to drive… on the game page
- [ ] Backup page: drives, Installer files / Installed copy rows, installer moved, refusals, skipped, disabled main button
- [ ] Drive grid GOG covers, counts and chip; drive game page; restore rows and card

## Decisions made by the designer — confirm with the user

The GameStores card's list applies here too. TV-only:

1. **Store folders** use the rail's existing folder rows (A opens and closes), one per store, opened by default.
2. **Y opens the on-screen keyboard** in the login window (B cancels the sign-in), because the store's page, not GRID, owns its fields.
3. **A Windows installer's own setup window** appears over TV mode and needs a mouse or keyboard; GRID says so before the install, rather than refusing Windows installers in TV mode.
4. **Steam Install** opens Steam's window over TV mode (Steam asks for the library folder); GRID only shows the progress.
5. **The Steam API key** is typed with the on-screen keyboard (masked) in TV mode; pasting is the desktop's job.
6. **CHANGED 2026-11-13 (user decision):** was "RT (Select) isn't offered in store views". Now RT is Select in GOG › Installed (for backups), matching the desktop; other store views still have none.
7. **New:** in TV mode, a Windows installer's setup window (from a RomM archive or a drive restore) opens over TV mode and needs a mouse or keyboard; the notice says so.
