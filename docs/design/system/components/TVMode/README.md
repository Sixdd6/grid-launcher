TV mode is GRID's second, fullscreen interface: every feature on main, driven entirely with a game controller from the couch.

**Status: proposed, not built.** Nothing here is in the app yet. It needs sign-off before it is planned.

## Try it

The card is a working prototype. Click inside it first, then use the keyboard as a controller:

| Controller | Keyboard in the preview | Does |
| --- | --- | --- |
| D-pad / left stick | Arrow keys | Move focus. Held, it repeats after 400 ms, then every 90 ms |
| (held, on the on-screen keyboard) | (held) | A, X, Y, LB and RB repeat after 500 ms, 20 times a second |
| A | Enter | Select |
| B | Esc or Backspace | Back. From a grid or pane, B first goes to the rail; from a tab's top level, B goes Home |
| X | X | The focused item's quick action: Play / Install on a cover, Change emulator on a system |
| Y | Y | The screen's action: Search, Use this folder, Clear |
| LB / RB | Q / E | Switch tabs (Home, Library, Server, Downloads, Settings); in Details, switch Details tabs |
| LT | Z | Sort (Library); Shift on the keyboard |
| RT | R | Select mode on the Library and Server grids (Game backups) |
| Menu (Start) | M | The Menu panel |
| View (Back/Select) | V | Act on the newest notice that offers an action ("Open Downloads") |
| Right stick | Page Up / Page Down | Scroll what you're reading (a Details tab, a list) without moving focus |

A real controller also works in the preview, through the browser's Gamepad API. The prompts change to ✕ ○ □ △ when it is a PlayStation controller. Mouse hover and click work as well.

The Menu has a **Prototype only** section for states that need a trigger: first run (Connect), a rejected token, an unreachable server, an unreachable library, unplugging and plugging in the Couch kit drive (also mid-backup), and a failed install.

## The shell

- **Size:** every measure is in rem. The root font is 1/54 of the screen height (20px at 1080p), so the layout looks the same at 720p, 1080p and 4K. Text is at least 0.85rem (17px at 1080p); body text is 1.05–1.1rem.
- **Top bar** (4.6rem): GRID wordmark with a TV tag, the five tabs between their LB / RB prompts, then the live download (title, bar, percent), the connection dot and user, and a clock. A pushed page swaps the tabs for a breadcrumb: "(B) Library › Starfall Circuit".
- **Hint bar** (3.6rem, bottom): what each button does right now, in the controller's own glyphs, for the focused item. The left side shows the controller's name, or the offline notice. Each hint can also be clicked.
- **Focus:** one 0.2rem `focus` ring at a 0.2rem offset. A focused cover grows to 107% with `shadow-lift` and a ring. A focused row takes the `selected` fill. Focus is never lost: every screen opens with something focused, and going back returns focus to where it was. A change never moves the player: in Settings only the pane redraws (never the rail), and when the focused control goes away (Install recommended once nothing is missing) focus moves to the control now in its place in the same panel. While Install recommended runs, the button reads "Installing N emulators…" and can't be pressed again.
- **Movement:** spatial. Rails, rows and lists keep you inside them along their axis (no wrapping off the end). Right from a rail does what A does: into the grid or pane beside it. Moving into a rail always lands on its selected row (the platform, filter or pane you're in), even after a redraw. Moving into a row, pane or button group goes back to the item you last left there, or to its first item. Off a grid's edge, focus goes only to what sits straight past that edge: Up from the top row reaches Server's header chips, never the rail, and with nothing there (the Library) it stays put; Left from the first column goes to the rail. Down from a list's last item carries on to the buttons under it (never sideways into another panel).
- **Scrolling** follows fixed rules, never the browser's "just enough":
  - On Home, moving into a shelf brings its title to the top of the rows area, so a shelf is always seen whole with its title; the first shelf sits at the very top. The hero has a fixed height, so the rows area never changes size.
  - A row that scrolls sideways keeps the focused cover a full row margin (2.4rem) inside its edges.
  - Everywhere else (grids, lists, panes, menus), the focused item stays 1.2rem inside the scrolling area, room for its ring and lift; an item taller than the area lines up with its top. The first and last items scroll all the way to that end, so a heading above or a note below is never stranded.
  - Rows, grids and Details tabs leave room around every cover for its 7% growth and ring, so nothing focused is ever cut, and covers still line up with the titles and headers beside them.
  - A redraw (an install starting, coming back from Details, switching tabs and back) keeps every scroll position.
  - Scrolls glide; with reduced motion, and when a screen first appears, they jump.
- **Background art:** the focused game's art, blurred, behind panels. The fade and blur from Settings › Appearance apply. Text sits only on `panel` or `overlay`, as on the desktop.
- **Theme:** dark by default, because TV mode is for a dark room. Settings › Appearance › Theme still applies.
- **Notifications:** the same events and wording as the desktop, as cards at the top right under the top bar (the bottom is the hint bar). At most three. They never take focus. A card with an action shows the View prompt. Every card also goes to Menu › Notifications.

## Tabs

- **Home** (opens first) — replaces Server › Discover for the couch. A hero panel shows the focused game: platform, title as a logo, played / install state, two lines of summary. Under it, rows that scroll sideways: Continue playing, New releases (last six months, on your systems), Recently added, Updates (only when there are any), Your library. A selects (Details), X plays or installs from the row.
- **Library** — the filter rail (All games, Recent, Updates, then two folders: **Platforms**, with each installed platform, and **Backup drives**; rows carry their game counts) beside a grid of large covers. Moving through the rail filters the grid at once, without pressing A; A or Right then moves into the grid. A folder's header row opens and closes it with A (its chevron turns; Right still goes to the grid). A closed folder shows no rows; the grid's header still names the filter. Folders stay as you left them for the session. **Backup drives** has one row per connected, set-up drive (see Game backups below), or "No Backup drives connected". The same holds for every rail (Server, Settings). Y searches (keyboard), LT sorts. Cards carry the desktop badges: installed dot, UPDATE, MISSING, platform tag in mixed lists, and a queue / percent tag while installing. When an install finishes, the tag goes and the installed dot comes on every cover of that game at once, wherever focus is.
- **Server** — the server's platforms beside the grid, each rail row with its game count. The header reads "3 games · 1 installed", or "2 of 3 games match · 1 installed" while a search narrows the grid; every count is the number of games the grid shows. The header has the emulator and firmware chips; selecting one opens that system's page. Offline, the tab says the server can't be reached, with Try again and Open Library.
- **Downloads** — Active, Queued and Completed sections, each row with the title, kind badge, platform, status line, progress bar and the network / disk sparkline. A opens the row's actions (Cancel, Retry, Dismiss, Open the game). Y clears the completed ones.
- **Settings** — the desktop's panes in the same order, plus **TV mode**: Appearance, Cloud saves, Connection, Library, RetroAchievements, Systems, TV mode, Updates. Rows are switches (A flips them), choices (A opens a list), sliders (left / right adjust while focused), text (A opens the keyboard) and actions.

## Pages (pushed over a tab; B goes back)

- **Details** — the cover and the last-played / emulator line on the left. On the right: title, header line, chips, then one row of buttons (Play / Install / Cancel install, Update, Game settings, Uninstall; Locate… and Remove from library for a missing game; Link to server game and Remove from library for an unlinked one), then the tabs, switched with LB / RB (switching focuses the new tab's first item, at the top of the tab; a tab with nothing to focus puts focus on the first button): **Overview** (summary, Related row, facts panel: Down from the buttons focuses the summary, and Down / Up read through a long one before moving on; Up from Related always returns to the summary; Up into the buttons scrolls the tab back to its top; the right stick scrolls the summary, Related row and facts panel together, as one page, from anywhere in Details, without moving focus), **Media** (Screenshots and Videos rows; A opens the fullscreen viewer, LB / RB step through), **Saves** (Saves | States switch, Upload latest, a list where A opens Restore / Delete, each confirmed; a deleted entry leaves the list and focus moves to the one now in its place, the newest left is marked Latest, an upload appears at the top as Latest, and an empty list says how saves get there) and **Achievements** (when the server lists any).
- **Playing** — after Play, "Now playing" with the session time, Back to the game, and Stop game (confirmed). While a game runs, the game owns the controller and GRID ignores it. GRID listens again when the game closes, then shows the save upload notices.
- **System** (from Settings › Systems or a Server chip) — Emulator (a list with Installed and Download groups; picking a Download one installs it), version, Update to …, RetroArch core, firmware Install, Open the emulator, Emulator settings, Remove, setup tips. **Windows** has the Proton version, the umu-run line and Reinstall.
- **Add an emulator manually / Emulator settings** — the desktop's settings sheet as rows: which emulator, name, program (folder browser), launch options, cloud-save strategy.
- **Game settings** (native games) — launch file (a list; "Set by game.json" when it came from there), launch parameters, Reload launch info from server.
- **Link to server game** — the platform's server games as a list, Y to search.
- **Folder browser** — one page for every path: Locate…, Change library folder, Relocate the library, Program, Connect's library folder. A opens a folder; Y uses the folder shown; Up one folder is the first row.
- **Search everything** (Y on Home) — the keyboard, then a results grid.
- **Notifications** (from the Menu) — the session's notices, newest first.

## Overlays

- **Menu** (Menu button) — a panel from the right: clock, user and server, then Home, Downloads (with the active count), Notifications, Settings, Reconnect, Switch to desktop mode, Quit GRID.
- **Lists** — every choice is a centred list with a check on the current value; group headings (Installed / Download) are not focusable.
- **Confirmations** — a destructive action always asks first, in a dialog whose Cancel is focused. (The desktop's two-click confirm doesn't work with a controller: a double press of A would confirm by accident.)
- **On-screen keyboard** — slides up from the bottom over the hint bar's area: the field with a caret, four rows of keys, then Shift, symbols, Space, Delete and Done. Shortcuts: X deletes, Y adds a space, LB / RB move the caret, LT is Shift, Menu is Done, B cancels. Held typing buttons repeat like a keyboard key, after 500 ms and then 20 times a second: A on a letter, number, symbol, Space or Delete key, X (delete), Y (space) and LB / RB (caret), so a held X clears a search quickly. A held on Shift, symbols or Done doesn't repeat. A physical keyboard types straight in, and Enter is then Done. Masked fields (tokens, passwords) show dots. Search updates the results as you type.

## Startup screens

The box always fits between the top bar and the hint bar, so the screen itself never scrolls. A list longer than the space (a server with many systems) scrolls inside the box, under its title and above its buttons, which never leave view. The top bar already carries the GRID wordmark, so the box doesn't repeat it.

- **Connect** — Server, Sign in with (API token / Username and password), the token or password, Library folder (folder browser), Connect. View pastes from the clipboard. A rejected token returns here with the reason, as on the desktop.
- **Choose your systems** — the desktop rows as switches; A ticks, X changes the emulator, then Install N emulators or Skip for now. Down from the last system reaches the buttons; Menu (Start) skips from anywhere on the page, so a long list never has to be scrolled to skip. The setup card follows the installs, pinned to the top of the notification stack: one segment per install (each emulator once), empty while waiting; its first half fills while the emulator downloads and its second half while it installs, full when done (red if it failed). A line under the bar names the step: "DuckStation · downloading 62%", then "DuckStation · installing…". It has no countdown while anything is still installing; once the last install is done it says how many systems are ready and closes after 10 seconds.
- **Unreachable library** — the blocking dialog with Relocate the library… (folder browser) and Close without changing anything; B does nothing.

## Game backups

TV mode carries the whole Game backups design (the Game backups card, built on `feat/game-backups`): the same drives, words, rules, states and notifications. Only the controls change for a controller. Everything not listed here matches that card.

- **Library › Backup drives:** one row per connected, set-up drive, with its game count. Its grid is the drive's games with platform tags (search and sort as usual); the header reads "Couch kit", "8 games · 13.3 GB · on SAMSUNG T7", with the emulators and firmware chips. X on a cover is Play when the game is installed, else **Install from drive**, which copies it from the drive with no network ("Copying from SAMSUNG T7" in Downloads). When the game's system has no emulator, it also brings one from the drive, but only an emulator that plays that platform (the app's `platform-emulators.json` and emulator profiles decide); with none on the drive, the notice says the drive has no emulator for that system.
- **Details from a drive:** "From Couch kit" leads the header line; the buttons are Install from drive (or Play) and **Remove from drive…** (confirmed); the left column says where the copy is.
- **Details › Backup to drive…** sits after Uninstall; the left column adds "On Couch kit" when a connected drive already has the game. Offline it is disabled, and A on it says "Backing up needs a server."
- **Select mode (RT)** on the Library and Server grids, the desktop's Select / Done. A picks a cover instead of opening Details (a filled check top left, a primary ring, slightly smaller), X selects all or clears, Y acts, RT or B leaves. A bar pinned under the grid reads "3 selected · 4.2 GB" with those buttons. Y is **Backup to drive…**; on a drive it opens a list with Install N games and Remove N from drive…. The desktop's Shift-click range has no controller equivalent: Select all covers it.
- **A whole platform:** **Backup all N…** in a Server platform header; on a Library platform row, X is "Backup all N…" (installed games only).
- **The Backup page** is the desktop's backup dialog as a page (it is long, and a page scrolls like every list here). Title ("Backup 3 games", "Backup Cinder Peaks", "Backup PlayStation · 3 games"), the lede, then **Drive** (one radio row per connected, set-up drive with the space left after this backup and a bar where this backup's share is hatched; "Needs 76 GB · 61.4 GB free" in the danger colour; with none connected it says so, names a connected drive that isn't set up, and offers Set up a drive…), **Source** (Copy installed games from this computer with its count note; a warning-edged row per out-of-date game whose choice opens a list, Use this computer's copy or Download v1.2; the bulk rows Use this computer's copies and Download all; Show each game with each game's tag: From this computer, Download or Choose below), **Also copy** (Emulators, always, with what goes or "Already on the drive"; Firmware, from the default; Make Windows games work offline when it applies), the skipped-games note, and the footer: "4.2 GB to write · 1.4 GB to download" and the main button. It opens on its first row with the title in view. **Start presses the main button** from anywhere on the page; the button reads Backup N games, or, disabled, Choose for N games, Not enough space or Nothing to backup, and Start (or A) on it then says why.
- **Downloads:** every game, emulator and firmware row of a backup carries the **Backup** badge and reads "Downloading from server 45%" or "Copying from this computer to SAMSUNG T7 45%", then "Writing to SAMSUNG T7 70%", "Waiting · to Couch kit" while queued, and "Copied from this computer · Written to Couch kit" when done. One card per backup: "3 games added to the backup queue for Couch kit", then "3 games and 1 emulator backed up to Couch kit".
- **A drive removed mid-backup:** the row being written fails ("Failed — SAMSUNG T7 was removed"), the drive's remaining queue becomes one Completed row ("5 more items · Cancelled — SAMSUNG T7 was removed · backup again to finish"), one error card says how many items were written, and the drive leaves the Library rail.
- **Settings › Backup drives** (second in the rail): Connected (set up or not), Not connected (remembered), then **Every backup also copies › Firmware**. A drive opens its page: For Linux and Windows (Change… opens the two switches, at least one stays on, with the notices the desktop gives), the space bar (other files, GRID, free), games, emulators, firmware, last backup, folder, Show in Library, Open folder (in desktop mode), Rename… (keyboard) and **Remove GRID files…**, which asks twice. A drive that isn't set up opens **Set up for GRID**: pack name, the two computers, the FAT32 warning and the note that only a gridpack folder is made; Start sets it up. A remembered drive offers Forget.
- **Use without a server:** on Connect, after "or", beside Connect. It needs only the library folder, and opens the Library on the first connected Backup drive. The top bar reads "Offline · no server" (grey dot), the hint bar says the Library and Backup drives work without one, and Server shows "Not connected to a server" with **Connect to a server…**.

## Decided

- **Entering and leaving:** a TV button in the desktop top bar opens TV mode, and so does starting GRID with `--tv`. Menu › Switch to desktop mode (also in Settings › TV mode) leaves. No other way in or out (2026-11-10).
- **RT is select mode** on the Library and Server grids (2026-11-10).
- **Start presses a page's main button** where a page has one: Skip for now on Choose your systems, Backup N games on the Backup page, Set up for GRID; while the button is disabled, Start says why (2026-11-10). Elsewhere Start opens the Menu.

## Proposed defaults (to confirm)

- **Desktop-only settings:** card size, game labels and card display stay desktop-only; TV mode has one cover size.
- **Opening an emulator** on its own from TV mode works, but the emulator's own window needs a mouse or its own controller support.

## Not in this card

- How the app reads the controller (a native Rust reader, or the webview's Gamepad API) — an implementation choice for the plan.
