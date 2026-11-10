TV mode is GRID's second, fullscreen interface: every feature on main, driven entirely with a game controller from the couch.

**Status: proposed, not built.** Nothing here is in the app yet. It needs sign-off before it is planned.

## Try it

The card is a working prototype. Click inside it first, then use the keyboard as a controller:

| Controller | Keyboard in the preview | Does |
| --- | --- | --- |
| D-pad / left stick | Arrow keys | Move focus. Held, it repeats after 400 ms, then every 90 ms |
| A | Enter | Select |
| B | Esc or Backspace | Back. From a grid or pane, B first goes to the rail; from a tab's top level, B goes Home |
| X | X | The focused item's quick action: Play / Install on a cover, Change emulator on a system |
| Y | Y | The screen's action: Search, Use this folder, Clear |
| LB / RB | Q / E | Switch tabs (Home, Library, Server, Downloads, Settings); in Details, switch Details tabs |
| LT | Z | Sort (Library); Shift on the keyboard |
| Menu (Start) | M | The Menu panel |
| View (Back/Select) | V | Act on the newest notice that offers an action ("Open Downloads") |

A real controller also works in the preview, through the browser's Gamepad API. The prompts change to ✕ ○ □ △ when it is a PlayStation controller. Mouse hover and click work as well.

The Menu has a **Prototype only** section for states that need a trigger: first run (Connect), a rejected token, an unreachable server, an unreachable library, and a failed install.

## The shell

- **Size:** every measure is in rem. The root font is 1/54 of the screen height (20px at 1080p), so the layout looks the same at 720p, 1080p and 4K. Text is at least 0.85rem (17px at 1080p); body text is 1.05–1.1rem.
- **Top bar** (4.6rem): GRID wordmark with a TV tag, the five tabs between their LB / RB prompts, then the live download (title, bar, percent), the connection dot and user, and a clock. A pushed page swaps the tabs for a breadcrumb: "(B) Library › Starfall Circuit".
- **Hint bar** (3.6rem, bottom): what each button does right now, in the controller's own glyphs, for the focused item. The left side shows the controller's name, or the offline notice. Each hint can also be clicked.
- **Focus:** one 0.2rem `focus` ring at a 0.2rem offset. A focused cover grows to 107% with `shadow-lift` and a ring. A focused row takes the `selected` fill. Focus is never lost: every screen opens with something focused, and going back returns focus to where it was.
- **Movement:** spatial. Rails, rows and lists keep you inside them along their axis (no wrapping off the end). Moving into a rail, row or pane goes back to the item you last left there, or to its first item.
- **Background art:** the focused game's art, blurred, behind panels. The fade and blur from Settings › Appearance apply. Text sits only on `panel` or `overlay`, as on the desktop.
- **Theme:** dark by default, because TV mode is for a dark room. Settings › Appearance › Theme still applies.
- **Notifications:** the same events and wording as the desktop, as cards at the top right under the top bar (the bottom is the hint bar). At most three. They never take focus. A card with an action shows the View prompt. Every card also goes to Menu › Notifications.

## Tabs

- **Home** (opens first) — replaces Server › Discover for the couch. A hero panel shows the focused game: platform, title as a logo, played / install state, two lines of summary. Under it, rows that scroll sideways: Continue playing, New releases (last six months, on your systems), Recently added, Updates (only when there are any), Your library. A selects (Details), X plays or installs from the row.
- **Library** — the filter rail (All games, Recent, Updates, platforms; each with a count) beside a grid of large covers. Moving through the rail filters the grid at once, without pressing A. Y searches (keyboard), LT sorts. Cards carry the desktop badges: installed dot, UPDATE, MISSING, platform tag in mixed lists, and a queue / percent tag while installing.
- **Server** — the server's platforms beside the grid. The header has the emulator and firmware chips; selecting one opens that system's page. Offline, the tab says the server can't be reached, with Try again and Open Library.
- **Downloads** — Active, Queued and Completed sections, each row with the title, kind badge, platform, status line, progress bar and the network / disk sparkline. A opens the row's actions (Cancel, Retry, Dismiss, Open the game). Y clears the completed ones.
- **Settings** — the desktop's panes in the same order, plus **TV mode**: Appearance, Cloud saves, Connection, Library, RetroAchievements, Systems, TV mode, Updates. Rows are switches (A flips them), choices (A opens a list), sliders (left / right adjust while focused), text (A opens the keyboard) and actions.

## Pages (pushed over a tab; B goes back)

- **Details** — the cover and the last-played / emulator line on the left. On the right: title, header line, chips, then one row of buttons (Play / Install / Cancel install, Update, Game settings, Uninstall; Locate… and Remove from library for a missing game; Link to server game and Remove from library for an unlinked one), then the tabs, switched with LB / RB: **Overview** (summary, Related row, facts panel), **Media** (Screenshots and Videos rows; A opens the fullscreen viewer, LB / RB step through), **Saves** (Saves | States switch, Upload latest, a list where A opens Restore / Delete, each confirmed) and **Achievements** (when the server lists any).
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
- **On-screen keyboard** — slides up from the bottom over the hint bar's area: the field with a caret, four rows of keys, then Shift, symbols, Space, Delete and Done. Shortcuts: X deletes, Y adds a space, LB / RB move the caret, LT is Shift, Menu is Done, B cancels. A physical keyboard types straight in, and Enter is then Done. Masked fields (tokens, passwords) show dots. Search updates the results as you type.

## Startup screens

- **Connect** — Server, Sign in with (API token / Username and password), the token or password, Library folder (folder browser), Connect. View pastes from the clipboard. A rejected token returns here with the reason, as on the desktop.
- **Choose your systems** — the desktop rows as switches; A ticks, X changes the emulator, then Install N emulators or Skip for now. The setup card follows the installs at the top right with one segment per install.
- **Unreachable library** — the blocking dialog with Relocate the library… (folder browser) and Close without changing anything; B does nothing.

## Proposed defaults (to confirm)

- **Entering and leaving:** a TV button in the desktop top bar, F11, and a `--tv` start option; Settings › TV mode › Open GRID in (the mode it was closed in / TV mode / desktop mode). Menu › Switch to desktop mode leaves.
- **Desktop-only settings:** card size, game labels and card display stay desktop-only; TV mode has one cover size.
- **Opening an emulator** on its own from TV mode works, but the emulator's own window needs a mouse or its own controller support.

## Not in this card

- Game backups (gridpack drives) — after TV mode is done.
- How the app reads the controller (a native Rust reader, or the webview's Gamepad API) — an implementation choice for the plan.
