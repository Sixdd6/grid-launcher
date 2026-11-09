# Overview
The application is a game launcher and manager for RomM which connects to the server API specified in `openapi.json`.
It is built in Rust with Tauri 2 and a Svelte 5 frontend, and ships as a Windows installer and a self-contained AppImage on Linux.

# Top Bar
The top bar is 58px tall: the GRID logo and wordmark on the left, a centred pill
group with the four views (Library, Server, Downloads, Settings) in that order, and on
the right the app-update badge when a release notice is stored, the light / dark
toggle, a connection status dot and the server menu (Reconnect, Disconnect, Open RomM
in browser). `Ctrl+1`..`Ctrl+4` switch views from the keyboard (not while a text field
has focus or a dialog is open). Emulators are set up in Settings › Systems; there is
no Emulators view.

**Offline mode.** After "Use without a server" (First Run Setup), the connection dot is
grey (a choice, not a fault, so not the danger red) and the chip reads "Offline · no
server". Its menu has one item, **Connect to a server…**, which opens the Connect screen,
and the note "Library and backup drives work offline". Nothing is cleared until a
connect succeeds.

**Light / dark toggle.** A 28×28 icon button showing the current mode — a sun in light,
a moon in dark — whose label names the switch ("Switch to dark mode"). On the startup
screens (Connect and first-run setup) it sits in the window's top-right corner, 16px in;
in the main window it is in the top bar, left of the connection dot and username. A
click switches the theme and sets Settings › Appearance › Theme to match ("Follow
system" stays a choice there). The icon spins in (300ms) and the colours ease across
(240ms) only during the switch; with reduced motion both are instant.

Views stay mounted and switch with the `hidden` attribute, so scroll positions and
selections survive a switch. Behind the content sits blurred background art — the
cover of the last game viewed, faded toward the window background by the amount set in
Settings › Appearance.

**Surfaces.** Text never sits on the raw art. There are three levels: the art (the
canvas), **panels** and **overlays**. The top bar, each view's rail, its header and
toolbar, and its lists and forms are frosted panels, at least 86% opaque however strong
the art is, so every text stays above WCAG AA on any art in both themes; the rail and
the view are separate rounded panels with a 12px gutter, where the art shows through.
Details, dialogs and menus are opaque overlays with a shadow, over a dimming scrim. One
selected style (a tint with an inset bar on rows, the filled pill in the top bar) and one
keyboard focus ring apply everywhere.

A 28px download strip sits at the bottom of the window. It is hidden while nothing is
transferring; otherwise it reads `⬇ <title> · <percent> · <speed>` beside a small
sparkline of the current transfer's last 60 seconds (network in the primary colour, disk
in teal) and an "Open Downloads" link. Clicking anywhere on the strip opens the Downloads
view. A backup reads `Backing up <title> to <pack> · <percent> · downloading` (then
`writing`), and an install from a backup drive shows "Copying from <label>" in place of
the network rate.

**Notifications.** Every message (install results, launches, firmware, cloud saves,
errors) is a card in one stack at the bottom right, above the download strip and over
dialogs; there are no centred toasts. Each card has a Dismiss button with a bar along its
bottom edge that runs down over 10 seconds, after which the card closes itself (a few
one-time notices that need reading, such as the Python import and a library move's
outcome, stay 12–20 seconds). Pointing at a card, or focus inside it, pauses its
countdown. Errors are in the danger colour and announced as alerts. At most four cards
stack; a fifth drops the oldest. The first-run setup card sits at the top of the same
stack and counts down only once its installs are done. With reduced motion the bar
stays still, but the cards still close on time.

Every download, install and launch event shows a card once, whichever view started it
and whichever view is open:

- A game added to the download queue: "<Title> added to download queue" (an update:
  "Update for <Title> added to download queue"; PS4/Xbox 360 extra content: "Extra
  content for <Title> added to download queue"). Retrying a failed or cancelled download
  queues it again.
- The download finishing: "<Title> installed" ("<Title> updated" for an update), or an
  error card "Couldn't install <Title>: <reason>" ("Couldn't update …"). Cancelling:
  "Cancelled <Title>". A refused Install, where nothing was queued, is an error card
  "Couldn't add <Title> to the download queue: <reason>".
- An emulator or compat tool queued: "Installing <Name>…" ("Updating <Name>…" for an
  emulator update); finished: "<Name> installed" or "<Name> updated" (a Proton picked as
  the Windows default: "<Name> installed and used for Windows games"; umu-launcher:
  "umu-launcher installed"); failed: "Couldn't install <Name>: <reason>"; cancelled:
  "Cancelled <Name>". Removing an emulator: "<Name> removed".
- Uninstalling a game: "<Title> uninstalled"; removing a library row: "<Title> removed
  from the library".
- Launching: "Launching <Title>…". A refused launch is an error card "Couldn't launch
  <Title>: <reason>" (a Proton launch that needs umu-launcher also keeps the reason and
  its Install button beside the Play button). A game that exits inside its first half
  second is an error card "<Title> — Process exited immediately (…)", followed by the
  command and the first few lines the program wrote to its error output, when it wrote
  any. When that output shows a system library is missing (Linux: "error while loading
  shared libraries"), the card instead reads "<Title> — <Emulator> couldn't start:
  <library> is missing. Install it with your system's package manager (often named
  <package>)." Opening an emulator from Settings › Systems uses the same wording in its
  toast.
- A backup is one card per batch (every row one Back up click queued), never one per
  game. Queued, once the batch stops growing: "3 games added to the backup queue for
  <pack>". When every row has ended: "3 games and 1 emulator backed up to <pack>". If a
  row failed or the drive was removed, one error card instead: "<label> was removed. 4 of
  17 items were written to <pack>." (or "Couldn't back up <Title>: <reason>. …"). Adding
  a computer to a pack words its batch "Windows builds of 2 emulators added to the backup
  queue for <pack>" and "… backed up to <pack>". Backup rows show no per-row cards.
- An install from a backup drive shows the game's own cards as above. What it brings
  from the drive has its own card: "<Emulator> from <pack> added to download queue" (the
  emulator's queue row shows no card of its own), and "<Platform> firmware installed
  from <pack>" when the drive's firmware is in place (an error card when it fails).
Firmware, library moves, cloud saves and the Windows default keep their own cards, and a
library move's row and firmware rows in Downloads add none. Cards of one kind that arrive
within 0.4 seconds of each other (never held more than 1.5 seconds) become one summary
card, such as "5 games added to download queue", "Installing 3 emulators…" or "2 downloads
failed — see Downloads". While the first-run setup card is up it counts its emulators, so
no per-emulator queued, installed or failed cards are shown.

# Main Sections
There are several main sections to the application with buttons across the top bar to navigate between them:
- Library
- Server (opens on Server › Discover)
- Downloads
- Settings (emulators are set up in its Systems pane)

- **Library** contains a 220px filter rail — All games, Recent (played in the last 30
  days), Updates, then each installed platform, every entry with a count — beside a
  header (a title row with the rail entry's name and its game count, "12 games", over a
  toolbar: search by title, sort by Recently played / Recently installed / Title /
  Platform, card size Small / Medium / Large) and one grid of cover cards. Each rail
  entry has its own empty state. Clicking a card opens the details sub view; the hover
  Play launches directly. The card size is remembered as `ui.card_size_library`.

  **Library › Backup drives.** Under the platforms, a "Backup drives" group has one row
  per connected drive with a readable pack (a read-only drive too): a drive icon, the
  pack name and its game count; pointing at the row shows the volume label. Unplugging
  the drive drops its row (the rail falls back to All games) and closes a Details opened
  from it.
  - The header shows the pack name, "8 games · 13.3 GB · on SAMSUNG T7", and two chips:
    "Emulators: DuckStation, PCSX2" and "Firmware: PlayStation" ("none" when empty).
  - The grid is the drive's games, with search ("Search games on this drive"), sort
    (Title, Platform, Recently backed up, Largest first) and size. A game installed here
    shows Play; any other shows **Install**, which copies it from the drive with no
    network. An empty drive reads "Nothing on this drive yet. Back up games from Server
    or Library."
  - Select mode on a drive offers **Install N games** and **Remove from drive…** (two
    clicks: "Remove N from drive"); the result reads "Removed 3 games from <pack>".

  **Select mode** (Library and Server, not Discover). **Select** in the toolbar turns it
  on and reads **Done** while on. A click on a cover picks it instead of opening Details;
  Shift-click picks a range. A picked cover shows a filled check and a ring, and its hover
  bar is hidden. A bar pinned to the bottom of the grid shows "3 selected · 4.2 GB",
  Select all N / All N selected, Clear and the action: **Back up to drive…** on installed
  games and on Server. Esc or Done leaves select mode, and so does changing the rail
  entry. A Library platform row also has **Back up all N…** in its header, for its
  installed games that have a server id. Backing up needs a server: offline, both are
  disabled with the reason.

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
- **Server** mirrors Library: a rail with **Discover** first (a divider under it), then the
  server's platforms (alphabetical by name) with ROM counts. A platform shows a
  platform header (name, ROM count, installed count, a firmware chip and the platform's
  default emulator chip), a search box that filters the loaded platform list by
  title, platform or genre, and a grid of cover cards. The hover action is
  Install for a game that is not installed and Play for one that is. The card size is
  remembered as `ui.card_size_server`. Downloads and installs are queued and handled in
  the background. A platform header has **Back up all N…** on its right, which opens the
  Backup dialog for every game of the platform, and the toolbar has Select (see Select
  mode above).

  **Offline.** In offline mode Server shows one panel in place of the rail and grid: "Not
  connected to a server", "Your Library and backup drives work without one. Connect to
  browse the server and back up games." and **Connect to a server…**.

  **Header chips follow the state.** The firmware chip reads "Firmware: 3 files — no
  default emulator" until the platform has a default emulator, then "Firmware: 3 files",
  then "Firmware: 3 files · installed" ("No server firmware" when the server has none,
  "Firmware: checking…" / "Firmware: unavailable" while the status is loading or was
  refused). When the server offers firmware it is a link; so is the emulator chip, which
  names the default emulator ("No default emulator" when none). Both open Settings ›
  Systems scrolled to that platform's row, opened, where the firmware installs and the
  emulator is picked.
  Native platforms (games native to this OS, and Windows games run through Proton) and
  the Emulators shelf show neither chip, and their firmware status is never fetched.
- **Server › Discover** is the first Server rail row and the default view: a connected app, and
  the app after first-run setup, opens on it. It has no folding header and no search;
  Ctrl+F does nothing there. Its lists come from the server's `created_at` and
  `metadatum.first_release_date`, always filtered and sorted by GRID itself (RomM
  ignores an `order_by` it does not know, and lists upcoming games with future dates).
  The **preferred platforms** are the systems with a default emulator, Windows once a
  compat tool is the default (on a computer that is not Windows), and the platforms
  native to this computer, always (Linux games on Linux, Windows games on Windows).
  - **New releases:** a coverflow of the games on the preferred platforms released in
    the last six months (never in the future), newest front and centre, at most 25.
    With none, the section says so and links to Settings › Systems. The centre cover is
    flat and lit; the others tilt 52° back, recede and dim, with a soft reflection.
    The arrows, a click on a side cover, or ← / → (on the coverflow, or anywhere while
    nothing else has the keyboard) move it with a 450ms glide; it stops at both ends. A
    click on the centre cover, or Enter, opens Details. Under it: the title, the
    platform, "Released 7 Nov 2026", and Install or Play (Installing… / Queued while an
    install is live) — the same install and launch as the platform grid.
  - **Recently added:** the newest games on the server, leaving out the coverflow's, up
    to 8. A large 16:9 screenshot on the left with the platform, "Added 3 days ago"
    (today / yesterday / N days / N weeks, then the date) and Install or Play under it;
    on the right a column of small covers with their titles. A highlight frame glides
    to the next row every 10 seconds (450ms) while the screenshot crossfades (600ms),
    wrapping at the end; a thin bar inside the frame fills over the 10 seconds. Pointing
    at (or focusing inside) the section pauses it; clicking a row jumps to it; clicking
    the screenshot or the highlighted row opens Details. There are no Details buttons.
    The timer runs under reduced motion too, where the glide, crossfade and bar are off.
  - **Platform filters** run across the top of Recently added: All, then one pill per
    platform with games among the server's newest additions (the 50 newest, less the
    coverflow's games), ordered by each platform's most recent addition. The row hides
    when only one platform is there. Clicking a pill narrows the list to that platform's
    newest additions, up to 8 (fetched from the server for that platform, still leaving
    out the coverflow's games) and starts the rotation at the first row; clicking the
    pressed pill again, or All, clears it. One at a time. The previous list stays until
    the new one arrives; each platform's list is fetched once per feed load.
  - **Screenshots of another shape** (8:7, 4:3, 21:9…) are fitted inside the 16:9 frame,
    uncropped, over a blurred, darkened, enlarged copy of the same picture. A game with
    no screenshot shows its fanart, else its cover, fitted the same way.
  - **Logo, never plain text:** the title is a logo on the screenshot's bottom-left over
    a soft dark fade. A game with a ScreenScraper clear logo on the server
    (`ss_metadata.logo_path`, else `logo_url`, on the server's own host) shows it.
    Otherwise GRID draws the title in one of three logo styles (fixed per game),
    coloured from the screenshot's dominant shade: its own pixels grouped into
    4-bit-per-channel buckets, the most common bucket's average; the letters a light
    tint of it (88% lightness), the outline and shadow a deep shade (16%). The shade is
    computed in the backend from the cached image (a webview canvas cannot read an
    `asset://` image) and remembered for the session.
- **Server and Library headers fold while you scroll** (one shared behaviour). Past 32px
  of scroll the title row (title, counts, chips) folds up into the toolbar row, leaving
  the search field and Size (and Library's Sort) pinned to the top of the main panel;
  back within 8px of the top it unfolds (the gap stops flicker). The header floats over
  the grid, which reserves its unfolded height, so folding never moves a card. The fold
  takes 260ms (ease-out): the title row's height closes while it rises 8px and fades,
  the chips a beat ahead; the padding tightens from 14px to 8px; the bar turns to the
  opaque overlay surface with a soft shadow and a stronger edge. Unfolding reverses it,
  the title fading in a moment late; reduced motion makes it instant. While folded the
  search placeholder carries the hidden title's context: "Search PlayStation…" in
  Server; in Library "Search your library…", "Search recent games…", "Search games with
  updates…" or "Search <platform>…" (unfolded they read "Search title, platform or
  genre" and "Search installed games"). Switching platform or rail entry starts the grid
  at the top, unfolded. Arrow-key movement keeps the selected card clear of the folded
  bar, and moving into the first row scrolls back to the top.
- **Downloads** is a full view, capped at 1100px and centred, with three stacked
  segments in this order — Active (downloading, installing, cancelling), Queued, and
  Completed (finished, failed, or cancelled) — each with a count; the three section
  titles say enough, so there is no legend line. Each row shows the title, a kind badge (none for a base game;
  Update, Content, Emulator, Compat tool, or Firmware otherwise), the platform, the
  status/progress line, a progress bar, a 120×38 sparkline panel of the last 60 seconds
  (network in the primary colour, disk in teal, with the rate and ETA under it), and the
  action for its state: Cancel, Retry and Dismiss, or Dismiss. A live firmware row offers
  no action. The Completed segment keeps the last 50 entries.

  **Backup rows.** Each game, emulator build and firmware set of a backup is its own row
  with the kind badge **Backup**. A queued row reads "Waiting · to <pack>"; then
  "Downloading from server 45%" (network sparkline), then "Writing to <label> 70%" (disk
  sparkline); a finished row goes to Completed as "Written to <pack>"; a failed one reads
  "Failed — <reason>". Cancel works while it downloads and while it writes. A backup's
  game rows run first, then its emulators, then its firmware; backups and installs share
  one queue, first in, first out. When the drive is removed, the row being written fails
  ("Failed — SAMSUNG T7 was removed") and the drive's waiting rows fold into one
  Completed row, "13 more items" with "Cancelled — SAMSUNG T7 was removed · back up
  again to finish", which offers only Dismiss. A failed or cancelled backup row can be
  retried while its drive is connected. An install from a drive reads "Copying from
  <label> 45%" while it copies.
- **Settings** is a rail of eight panes in alphabetical order — Appearance, Backup
  drives, Cloud saves, Connection, Library, RetroAchievements, Systems, Updates — and
  opens on the first, Appearance.
  The panes: **Backup drives** (see Game backups below), **Connection** (server URL,
  user, whether a credential is stored in the OS keyring and whether the session is
  verified, Reconnect, Disconnect; offline it reads "Offline · no server", the credential
  "Not used offline", and offers only Connect to a server…), **Library** (the library folder and Change…: the new folder is typed or
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
  says so; the server-side name is never cleared), **Systems** (see Settings › Systems
  below), **Updates** (the running version, when the startup check
  ran, the release notice with its Open release and Dismiss buttons, and the note that
  the launcher only ever checks — it never downloads or installs an update), and
  **Appearance**.
- **Settings › Systems** holds every emulator choice, install, update and the Windows
  (Proton) setup (it replaced the Emulators view). Every emulator entry has a name,
  program path, launch arguments, save strategy, ignore rules and optional custom
  save/state directories.
  - **One row per system on the server** (never a native platform or the Emulators
    shelf), each with an emulator picker and a status: Ready, Update available, Firmware
    needed, Installing… or No emulator. Systems with no emulator come first, open, with
    a warning edge, the reason ("Games for this system can't start yet. PCSX2 is
    recommended.") and one-click Install of the recommended emulator; a system no
    installable emulator plays says so and points to Add an emulator manually. The rest
    follow alphabetically. The emulator shown is the one a launch would use: the
    system's default, else the first installed emulator that plays it, never one after
    an explicit None.
  - **The picker is the installer.** **Installed** lists the installed emulators that play
    the system; **Download** lists the rest GRID can install for it (the same options as
    first-run setup, server builds included), the recommended one first; the recommended
    one is marked "(recommended)" in either group. Picking an Installed emulator makes it
    the system's default; picking a Download one installs it and makes it the system's
    emulator when the install finishes — the first-run setup's waiting defaults, so a
    failed install that is retried still gets it, and a RetroArch pick becomes the
    default only when a core is on disk (a notice says so otherwise). The pick is the
    user's own: it replaces an explicit None, and setting a default by hand drops an
    install still waiting for that system. **None** stores the explicit "(none)".
  - **Install recommended (N)** in the header installs the recommended emulator for every
    system that has none (N counts systems), and hides when nothing is missing. **Check
    for updates** sits beside it.
  - **Opening a row** shows its emulator: the version ("· up to date" once checked),
    Update to <version> (only when one is out), Open (the emulator on its own, to change
    its settings) and a ⋯ menu with Settings…, Show program folder and Remove (two
    clicks: "Remove <name>? Games and saves stay"). The menu opens below the button,
    right-aligned, when it fits; near the bottom edge it opens beside the button (left,
    else right), shifted up to stay 8px inside the window. It never covers its button or
    opens upward over it, and it closes when the list scrolls, on Escape or on a click
    elsewhere. Below are the RetroArch core select, the system's firmware line ("PlayStation
    firmware · 3 files · from your server" with Install, Installing… while the pass runs,
    then Installed; each server file is checked against the size and MD5 the server lists
    before it is written; "Installing PlayStation firmware for DuckStation…" when it
    starts and "PlayStation firmware installed for DuckStation" when it ends), the PS3
    firmware line for RPCS3, and the Setup tips. RPCS3's firmware counts as installed
    when RPCS3's own dev_flash holds `sys/external/liblv2.sprx` (the file RPCS3 needs to
    boot a game); the version comes from `vsh/etc/version.txt`. GRID finds dev_flash the
    way RPCS3 does: the `portable` folder beside the program, else the program's folder,
    else RPCS3's config folder, each honouring the `/dev_flash/` entry in its vfs.yml.
    Installed firmware shows one line, "PS3 firmware 4.93 installed" ("PS3 firmware
    installed" when the version is unknown), with no Install button, and the system is
    not "Firmware needed". When it is not installed and the background download has put
    a package beside RPCS3, the line says so and Install hands the package to RPCS3's own
    installer; the page then checks again every few seconds for two minutes, and on each
    visit. With neither, the server firmware line offers the download. GRID does not
    download the package for an RPCS3 that already has firmware installed. An emulator used by several systems shows on each
    and says which others it plays.
  - **Companions.** An emulator whose catalog profile names companions (ShadPS4 and its
    Qt launcher, today the only one) shows each under its own line, marked "Companion":
    the companion's version, Update to <version> when one is out, Open and the same ⋯
    menu. An update for a companion makes the row "Update available". Installing or
    updating the emulator also installs a missing companion, so a companion that is
    still missing (removed, or a manual add) shows "<companion> is not installed" with
    a one-click Install, Installing… while its Downloads row runs. Installed entries are
    matched to profiles the way launches match them (the profile name, then the
    program's file name), not by display name alone.
  - **Update availability** is checked once per session, the first time Systems opens
    (one release request per emulator installed from a source, one at a time), and kept
    for the session; it never re-runs on a scroll or a re-render. Check for updates asks
    again. A finished update marks the emulator current.
  - **Windows** (Linux only, always shown) is a system row whose picker is the default
    Proton version: **Installed** lists system Wine, Proton installed by Steam and GRID's
    own downloads; **Download** lists the catalog's compat tools not yet installed,
    GE-Proton first and marked recommended ("Install GE-Proton (latest, recommended)" —
    the catalog's release, with no extra forge request to learn its tag). Picking a
    Download entry installs it and makes it the default when its install finishes (kept
    until then, so a retry still gets it; picking an installed one drops it). Its status
    is Installing… while that download runs, Ready once a default is set. Opening it shows
    the Proton launcher (umu-run) line — which umu-run a Proton launch uses ("system"
    with its path, "GRID-managed" with its release, or "not installed") — with Install
    (none installed) or Reinstall (GRID's copy; fetches the newest release), busy while
    its Downloads row runs, and the note that the first Windows game launched downloads
    the Steam Linux Runtime (about 200 MB). Installing a Proton tool while no umu-run is
    available also queues umu-launcher as its own Downloads row; its failure never fails
    the Proton install.
  - **Add an emulator manually** sits in the header, for an emulator GRID doesn't install;
    its form opens under the header. The settings sheet is the same for adding and
    editing (Settings… in the ⋯ menu): "Which emulator is it?" (the emulators GRID knows,
    or Something else with a name; add only), Name (Something else when adding; always
    when editing, so an entry can be renamed in place), Program (with Browse…; a path
    that matches a known emulator fills in the name and launch options when both are
    still empty), Launch options
    ("%rom% is replaced by the game file") and, under "Cloud saves (advanced)", how the
    emulator stores saves, save and save state folders, and files and file types not to
    upload. A manual add never sets a system's default; only an install the user asked
    for, the automatic install defaults, or the user's own pick do. (An emulator GRID has
    no profile for plays every system, so a launch can still fall back to it.)
  - **Other emulators** at the bottom lists the installed emulators no system uses, each
    with its emulator line and the systems it could play. A companion of a system's
    emulator is never here; one whose emulator no system uses (or that is installed on
    its own) stays here and says what it belongs to ("Companion to ShadPS4 (Playstation
    4).") instead of "Not used by any system on your server."
- **Appearance** (under Settings) chooses the theme — follow the OS, dark, or light;
  the light / dark toggle sets dark or light here too —
  turns the background art on or off, sets its fade from 0 to 100 percent with a live
  preview behind the pane (0 shows the art at full strength; each step fades it toward
  the window background; 100 is fully faded, which turns the art off, so nothing is
  fetched or built; the value reads "100% · off"), sets its blur, sets the default card size for the Library and Server
  grids, and sets **Game labels**: "Always visible" (a small label under every cover) or
  "Visible on hover" (covers only; the title shows on hover or keyboard focus), and
  **Game card display**: "Equal height" or "Equal width" (see Cards). They are stored
  under `[ui]` in `config.toml` as `theme`, `background_fade` (100 is off; a config from before 2026-11-07 stored the art's
  opacity, 0–60 with 0 off, and is converted once on load),
  `card_size_library`, `card_size_server`, `card_labels` (`always` or `hover`) and
  `card_display` (`rows` or `columns`). **Hardware acceleration** (on by default) renders
  the window on the graphics card; it applies after a restart and is stored as
  `hardware_acceleration`. Turning it off renders on the CPU, for a machine whose window
  comes up blank with it on; the environment variable `GRID_LAUNCHER_HW_ACCEL=never`
  (or `always`) overrides the setting for one start.
- **Cards** (both grids) are each the size of their artwork: never cropped, never boxed,
  no frame. **Game card display** (Settings › Appearance) picks the layout:
  - **Equal height** (the default): rows of one height, each card as wide as its art at
    that height, each full row stretched to the window's width. The row height follows
    the card size (Small 160, Medium 213, Large 267px) and moves at most 25% down or 30%
    up from it to fill a row.
  - **Equal width**: columns of one width (the card size's minimum width), each card as
    tall as its art, placed in the shortest column. Art too wide to stand 112px tall in
    one column spans two.
  A cover's shape is known once it loads; until then its card is portrait (3:4) and the
  layout settles as covers arrive. Shapes are remembered across sessions, so a platform
  seen before lays out final at once. Arrow keys move left and right in order, and up
  and down to the nearest card above or below. The cover has rounded corners and a soft
  shadow. On hover or keyboard focus it lifts, and a bar at its bottom shows Play or
  Install, Cloud sync and Details. Badges sit on the cover's corners: an installed dot
  top-right, an `UPDATE` tag top-left, a cloud icon bottom-right when the platform has
  cloud sync configured, and, in the Library only (the one grid that mixes platforms), a
  short platform name bottom-left. The details sub view shows the whole cover too.
  A card shows the server's large cover, falling back to the small one when there is no
  large one or it fails to load. Only a card drawn at most 200 device pixels wide (a
  portrait Small or Medium card on a 1x display) shows the small cover, which is about
  that wide.
  `Ctrl+F` focuses the current view's search box.

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
- **Ymir** (Sega Saturn) installs from its GitHub releases: the AVX2 build when this
  processor supports AVX2, else the SSE2 build. It runs with its own folder as its profile:
  the Saturn BIOS goes in `roms/ipl` (Ymir recognises a BIOS by its contents, so any file
  name works), and GRID sets `[System] InternalBackupRAMPerGame = true` in `Ymir.toml`
  (add-only) so each game keeps its own save, `backup/games/bup-int-<title> [<serial>].bin`,
  which cloud sync handles per game. First-run setup recommends Ymir for Sega Saturn.
- Installing a folder from the server's "Emulators" platform that follows the packaging standard (`docs/server-emulator-packages.md`) installs each emulator in it that GRID knows, from just its newest build for this system, the same way first-run setup does: under `emulators/<Name>`, registered with its server source and version. Settings › Systems' update check and **Update** for such an entry compare that version with the newest build for this system in the same folder (number by number, so 2.0.10 is newer than 2.0.9; a pre-release such as 0.2.7a6 or 2.0.0-rc.1 is older than its release, and a lone letter such as 0.2.5g is a later fix) and installs it over the same entry. A package that does not follow the standard installs as below.
- A package installed from the server's "Emulators" platform is registered as an emulator. It is downloaded and extracted under `games/Emulators/<package>` like a game, and its hidden library row is kept. GRID then picks the executable for this system (`.exe` on Windows; an AppImage, `.sh` or executable file on Linux) and matches it to a bundled emulator profile. A match gives the profile's name and arguments; no match gives the ROM title and `%rom%`. When another emulator entry already has that name, the package is added as "<Name> (server)", so a catalog entry and its update source stay unchanged. The profile's user-data folders are linked into `saves/`, the emulator is configured and gets default platforms like a catalog install, and its Downloads row shows the Emulator badge. Reinstalling the package updates the same entry and keeps the user's arguments. Removing the emulator in Settings › Systems also uninstalls its server package.

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

**Use without a server.** Under Connect, after an "or" divider, **Use without a server**
needs only the library folder (enabled once the stored folder is known, or the field is
filled). It stores the folder, drops any typed secret, remembers the choice
(`offline = true` in `config.toml`) and opens the shell on Library, on the first
connected backup drive when there is one (a drive found just after the shell opens is
picked up, unless the user has moved on by then). No request reaches any server: the next
start skips the server probe and reads no keyring entry. Library, backup drives,
Downloads, Settings and play work offline; backing up needs a server. A stored
credential stays in the keyring. **Connect to a server…** (top bar menu, Server panel,
Settings › Connection) opens the Connect screen; a connect or a Retry that succeeds
leaves offline mode. Games installed offline stay installed, and games installed from a
drive keep their server id (see Game backups).

**Choose your systems.** After the first successful connect of a fresh install (setup
never finished or skipped, no emulator configured, no game installed), Choose your systems
comes before the shell. It shows the same rows as Settings › Systems: the server's systems
that need an emulator (native PC platforms and the "Emulators" platform are left out) with
their game counts, in alphabetical order, each with a tick box, none ticked:
- Ticking a system pre-selects GRID's recommended emulator for it; its picker offers the
  other compatible emulators installable on this system, grouped **Installed** and
  **Download** like Settings › Systems, the recommended one first and marked. The
  recommendation is the
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
- A system nothing installable serves shows "No emulator available" and cannot be
  ticked; an unticked one whose recommended emulator is already configured shows
  "Installed".
- **Windows** (not on a Windows computer) picks the default Proton version for Windows
  games. Ticked, its picker lists **Installed** (system Wine, Proton installed by Steam,
  GRID's own downloads) and **Download** (the versions GRID can fetch, GE-Proton first,
  recommended and pre-selected). An installed version becomes the default at once; a
  download is installed with the emulators and becomes the default when it lands (a
  failed one that is retried still does).
- The main button counts the installs, a Proton download included: **Install 3
  emulators** (each emulator once, however many systems share it). It reads **Continue**
  when only an installed Proton version was picked, **Use selected emulators** when
  everything ticked is already installed, and is disabled ("Install emulators") while
  nothing is ticked.
- Installing opens the app at once (on Server › Discover). A card at the top of the notification
  stack follows the installs, Proton included: "Setting up 3 installs · 1 done", then
  "3 installs done" and "3 systems are ready to play." (Windows counts once its Proton
  version is the default). Its bar has one segment per install: filled when done, a
  moving fill while installing, empty while waiting, red when it failed; pointing at a
  segment names the install and its state. A failed install has Retry. Once nothing is
  still installing, its Dismiss counts down and the card closes itself after 10 seconds,
  like every notification.
- As each emulator installs, every platform it was chosen for gets it as its default
  emulator, replacing an automatic default but never an explicit "(none)". A RetroArch
  choice becomes the default only when a compatible core is actually installed; otherwise
  the platform is left without a default and the card says so. A chosen default waits
  for its own emulator, so a failed install that is retried still gets it.
- **Skip for now** opens the app with nothing installed. Either way Choose your systems
  is not shown again; changes later happen in Settings › Systems, whose **Install
  recommended (N)** replaces reopening it. Its footnote says so. **Disconnect** returns to
  Connect; it does not bring Choose your systems back once setup was finished or skipped.

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

**Backups in Details.** Under the install actions sits **Back up to drive…**, which opens
the Backup dialog for this game (offline it is disabled, and a line reads "Connect to a
server to back up games."). It is not shown for a Missing game. When connected drives
hold the game already (same server and ROM id), a line under the actions reads "On
<pack>" ("On Couch kit, Travel kit" for several). Details opened from a drive's grid adds
"From <pack>" to the header line. It offers **Install from drive** in place of Install
when the game is not installed here, and **Remove from drive…** in place of Back up
(two clicks; the second reads "Remove from <pack>").

Uninstall never removes a protected folder (a filesystem root, home, a library folder, its
top-level and platform folders, a folder that holds the game's install folder, a path with
`..`); when it keeps one, an error toast names each kept folder and the reason. This applies
to Details › Uninstall, deleting a server-package emulator, and Start fresh › Delete.

The right side carries the title, one header line (platform · release year · developer ·
genres · rating), chips for the playing state, the identification state, region and language
flags and the version, then up to four tabs:

- **Overview** — the summary, split into paragraphs at the server's own line breaks and
  kept to a readable line length, with the Related row (filtered to titles the server
  actually holds for that platform) under it; beside them, a facts panel (developer,
  companies, release, genres, game modes, players, franchises, size, version and
  content), which moves under the summary when the popup is narrow. Size is the game's
  own files' total; Version (PC platforms only) is the installed version, with the
  server's beside it when they differ ("v1.0.0 · server v1.1.0"); Content counts the PS4
  update / Xbox 360 DLC files the server holds ("1 update · 2 DLC"). Screenshots are on
  the Media tab only. There is no Files tab: firmware installs from Settings › Systems.
- **Media** — two horizontally scrolling rows, each with its count and arrows when it
  overflows: Screenshots (every screenshot, at its own shape) and Videos (the trailer and
  any server-hosted video). A row with nothing in it is left out. Clicking a tile opens a
  fullscreen viewer with arrows, Esc and a caption, which walks screenshots then videos.
  The viewer shows each item as large as the window allows at its own shape, but never
  more than twice its own resolution; the arrows sit just outside the media's edges and
  stop at the window's edge when the media is wide.
- **Saves** — the game's cloud saves or states, described below.
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

### Saves tab
The Saves tab always shows a list; there is no button that opens or closes it. When the
game syncs both saves and states, a two-sided switch on top picks which list shows: `Saves`
(`Emulator Saves` for shared save media such as Xemu and Redream VMUs) or `States`. A
"Cloud sync" action on a card opens the popup on this tab showing saves. Each list has a
title (`Cloud Saves`, `Emulator Saves` or `Cloud States`) with a button beside it to
manually upload the latest save/state for that scope, and fills in asynchronously.

The Saves/States should be listed in a vertical column with the name of the emulator used, file size, upload time and display relative 'x hours/minutes ago'. The newest is marked Latest. Each save in the list should also have right-aligned buttons: one to manually restore the save from the server, overwriting the local save (with confirmation prompt explaining the overwrite) and one to delete the save from the server (with confirmation prompt).

When the list is showing `Emulator Saves`, the entries represent shared emulator media rather than a single game's private save files, and a note above the list (and each confirmation) warns the player that restoring or deleting those entries can affect every game using that shared emulator storage.

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

# Game backups
GRID backs up games from the server to a removable drive (a USB flash drive or SSD),
with their art, details, emulators and firmware. Any computer with GRID can then install
them from that drive, with or without a network. The design is
`docs/design/system/components/GameBackups/README.md`.

## Drives
- **Detection.** GRID scans for removable drives every 2 seconds. On Linux these are the
  block-device mounts under `/run/media` and `/media`; on Windows, the drive letters
  Windows calls removable, plus fixed drives on an external bus (USB, SD, MMC, 1394),
  which is how a USB SSD reports itself, never the system drive; on macOS (best effort,
  not a release target), the volumes under `/Volumes` except the startup disk. A drive
  shows up within a few seconds of being plugged in and goes away when it is removed.
- **Identity.** A drive is identified by where it is mounted: its mount path, or its
  drive letter on Windows. A pack has no id of its own. A drive is a backup drive when
  `gridpack/gridpack.json` is on it, so GRID recognises it on any computer.
- **One pack per drive.** A game on several drives is listed under each drive, and
  Install copies it from the drive being viewed.
- **Remembered drives.** GRID keeps every drive it has set up (`[[backup.drives]]` in
  `config.toml`: mount, pack name, label, file system, size, game count, last seen), so
  Settings can list it while it is unplugged.

## Settings › Backup drives
The second pane of Settings. It lists the connected drives ("Looking for drives…" until
the first scan; "No removable drives" with "Plug in a USB flash drive or SSD" when there
are none), then the remembered drives that are not connected.
- **Each connected drive is a card:** the pack name (the volume label before setup), the
  label, the file system and size, and a status: Ready for backups, Not set up,
  Read-only, Can't read (with the reason), or Newer GRID needed (a pack written by a
  newer format is shown and never rewritten).
- **A drive that is not set up** offers **Set up for GRID…**: an inline form with the pack
  name (the volume label by default), **Computers this pack is for** (Linux and Windows,
  this computer's OS ticked; at least one), and the note that only a `gridpack` folder is
  made. On a FAT32 drive the form warns that files over 4 GB cannot go on it and suggests
  exFAT. GRID never formats a drive.
- **A set-up drive** shows "For Linux and Windows" with **Change…** (the same checkboxes,
  inline). Adding a computer queues that OS's build of every emulator already on the
  drive, which needs a server. Removing one deletes those builds; games and firmware
  stay, and it works offline. Under it: a usage bar split into Other files, GRID and Free,
  then Games, Emulators, Firmware, Last backup and the folder path, then **Show in
  Library**, **Open folder**, **Rename…** (inline) and **Remove GRID files…**. Remove asks
  again ("Delete the gridpack folder from <label>?"), says how much it frees, deletes only
  `gridpack/`, and leaves the games installed on this computer.
- **A remembered drive that is not connected** shows its name, label, file system and
  size, when it was last seen and its game count, with **Forget**.
- **Every backup also copies** holds one default for the Backup dialog: "The server's
  firmware for the systems you back up", on by default (`[backup] include_firmware`).
  Each game always takes its art and details, and each system its emulator.
- **What's in the gridpack folder** (collapsed) shows the layout below.

## The Backup dialog
Opened by Details › Back up to drive…, the select bar's Back up to drive…, and Back up
all N… (Server platform header, Library platform row). It opens over every view and over
Details.
- **Title:** "Back up Cinder Peaks", "Back up 3 games" or "Back up PlayStation · 4
  games". The lede says GRID downloads each game from the server and writes it, with its
  art and details, to the drive's gridpack folder. A backup always downloads the game
  from the server, even when it is installed here, so the drive gets the server's
  original file.
- **Drive:** one radio row per connected, set-up, writable drive: the pack name, "<label>
  · <file system>", the free space, and a usage bar where this backup's share is hatched.
  A drive without room reads "Needs 76 GB · 61.4 GB free" in the danger colour, and the
  main button reads "Not enough space". With none connected, the dialog says "No backup
  drive connected", names any connected drive that is not set up, and offers **Set up a
  drive…**, which opens Settings › Backup drives (and closes Details).
- **Also copy:** Emulators is always included, with a fixed tick: "PCSX2 for Linux and
  Windows · 120 MB", "Already on the drive" or "None needed". Each system brings its
  default emulator (else the first recommendation) for every computer the pack is for. A
  manually added emulator with no download source goes as a zip of its program folder,
  for this computer's OS only; one in a system folder, the home folder or a drive root,
  or whose folder is over 4 GB, is refused with the reason. Windows games bring the
  default Proton when the pack is for Linux, and with it umu-launcher. **Firmware** is a
  checkbox (systems and file counts), on by default from Settings, disabled with the
  reason when the drive already has it or the server has none. When the pack is for Linux
  and Windows games are being written, **Make Windows games work offline** (off by
  default) adds the Steam Linux Runtime that umu-run otherwise downloads on its first
  launch.
- **Notes:** games already on the drive (same server and ROM id) are skipped ("1 game is
  already on Couch kit and will be skipped: Moss & Marrow"). On a FAT32 drive, games with
  a file over 4 GB are listed as not possible.
- **Footer:** "11.1 GB to write", Cancel and **Back up 3 games** ("Nothing to back up"
  when there is nothing new). The bytes to write plus a safety margin must fit the free
  space.

Backups run in Downloads (see Downloads above): each item downloads into a staging
folder in the library (`<library>/.grid-staging/backup-<id>/`), then is written to the
drive. The staging folder is always deleted afterwards.

## Install from a drive
**Install** on a drive's game copies it off the drive and installs it exactly like a
server install: the same file choice and the same extract or finalize steps. Only the
bytes come from the drive. The game's art from the drive goes into the image cache first,
so its cover shows offline. The installed game keeps its server ROM id, so updates,
cloud saves and play activity work once the computer connects to that server. On a
connect to another server, such games lose their ROM id before the re-link pass (which
may link them again to that server's game), so cloud saves and play activity never
reach the wrong ROM.

Installing a game also sets up its system from the drive:
- When the system has no emulator and the drive has one for this computer, that
  emulator installs from the drive and becomes the system's emulator. Its Downloads row
  reads "Copying from <label>". It passes the same program header check as any emulator
  install.
- The drive's firmware for the system goes in place once the emulator is set (at once
  when the system already had one). Each file is checked against the size and MD5 the
  pack lists.
- A Windows game on Linux with no Proton here brings the drive's Proton, which becomes the
  default, plus the drive's umu-launcher when no umu-run is available, plus the drive's
  Steam Linux Runtime when umu has none, placed where umu looks for it so the first
  launch needs no network.

Emulators on a drive are programs: GRID installs one only when the user installs a game
that needs it. A removed drive mid-copy fails the row and deletes the partial files; the
drive's other waiting installs are cancelled.

## The gridpack folder
```
gridpack/
  gridpack.json                             the index
  games/<platform>/<file>                   a single-file game, exactly as the server gives it
  games/<platform>/<fs name>/<file>…        a multi-file or native game
  assets/<platform>/<game>/                 cover, fanart, logo, screenshot-01 … (up to 8)
  emulators/<emulator>/<version>/<os>/      the release archive GRID would download, per computer
  firmware/<platform>/<file>                the firmware files as the server serves them
```
`<platform>` is the RomM platform slug. `<os>` is `linux-x86_64` or `windows-x86_64`.
Every name is made safe for FAT, exFAT and NTFS (no reserved names, at most 255 bytes).
Two games that would share a path get ` [rom <id>]` added.

`gridpack.json` (`"format": 1`) holds the pack name, the OS list (`linux`, `windows`),
when it was created and last updated, and three arrays:
- `games`: each with its platform, its server source (server URL without userinfo, ROM
  id, and what rebuilds its details and image keys), the details Details shows, its size,
  its files (path, size, server file id, name, category, whether it is top level,
  hashes) and its asset paths;
- `emulators`: one entry per emulator per OS, with its kind (`emulator` or
  `compat_tool`, which covers Proton, umu-launcher and the Steam Linux Runtime), source,
  version, platforms, archive path and size;
- `firmware`: per platform, each file's name, path, size and MD5.

Every path is relative to `gridpack/`, so the drive works under any mount point. Unknown
keys are kept when GRID rewrites the file. The full shape is in the design README.

## Safety rules
- GRID never reads, writes or deletes anything outside `gridpack/` on a drive. The one
  delete of the whole folder is Remove GRID files…, behind a check that the target is
  exactly that folder.
- Every file is written to `<name>.gridtmp`, synced, checked, and only then renamed into
  place. `gridpack.json` is rewritten the same way after each finished item, so a drive
  pulled mid-write always holds a valid index and at most a `.gridtmp` leftover.
- Leftover `.gridtmp` files are removed the next time the drive is connected (not while
  a row is writing to it).
- A drive's `gridpack.json` is untrusted. A file over 64 MiB, one that does not parse, an
  unknown OS value or an unsafe path makes the drive "Can't read". Every path must be a
  plain relative path (no `..`, no absolute path or drive prefix, no backslash) and must
  not lead outside `gridpack/` through a link; a `gridpack` folder that is a link is
  refused.
- A pack written by a newer GRID is read-only.
- Logs and errors carry drive labels, paths and game names, never a server credential.

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
- ROM-set platforms keep their archive: the zip is the game, so it is never extracted and the emulator is given the zip. These are Arcade/MAME/FBNeo, Sega Model 2 and Model 3, Sega NAOMI and NAOMI 2, Sammy Atomiswave, Neo Geo MVS/AES, and Capcom CPS-1/2/3, matched by RomM slug (`arcade`, `model2`, `model3`, `naomi`, `naomi2`, `atomiswave`, `neogeomvs`, `neogeoaes`, `cps1`, `cps2`, `cps3`) or by platform name. Neo Geo CD and Neo Geo Pocket are not ROM sets and install as usual. DOS games also launch from their zip. A game on one of these platforms that an older GRID unpacked on install, with only the unpacked folder left, does not launch: the launch fails with "This game's zip was unpacked when it was installed, but <Platform> games launch from the zip itself. Uninstall the game and install it again." GRID never re-zips the folder; if the zip is still on disk, the game launches from it.
- RPCS3 / PS3 handling should support the game-launch requirements needed by that emulator setup.
- Theme selection should support `system`, `dark`, and `light` modes and apply consistently across the app UI.
- Settings should include an option to enable debug prints and a shortcut to open the config folder.