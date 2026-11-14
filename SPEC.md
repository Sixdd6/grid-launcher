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

**Where GRID opens.** The view selected when the main window first appears (after a
restore, a connect, or first-run setup) follows Settings › Appearance › **Open GRID on**,
one setting shared with TV mode. **Automatic** (the default): Server › Discover when GRID
is connected to a RomM server; the Library when it is not — the server can't be reached
(the Library opens on All games), or GRID is used without a server (the Library opens on
the first connected Backup drive, when one is set up). **Server › Discover** and
**Library** force that place: forced Discover with no server opens on the Server view's
not-connected panel, and forced Library without a server still opens on the first
connected Backup drive.

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
`writing`); a game copied from this computer reads `getting details`, then `copying from
this computer`. An install from a backup drive shows "Copying from <label>" in place of
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
- A backup is one card per batch (every row one Backup click queued), never one per
  game. Queued, once the batch stops growing: "3 games added to the backup queue for
  <pack>". When every row has ended: "3 games and 1 emulator backed up to <pack>". If a
  row failed or the drive was removed, one error card instead: "<label> was removed. 4 of
  17 items were written to <pack>." (or "Couldn't backup <Title>: <reason>. …"). Adding
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
- Server (opens on Server › Discover; GRID itself opens there when connected, see Where GRID
  opens under Top Bar)
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
    network. An empty drive reads "Nothing on this drive yet. Backup games from Server
    or Library."
  - Select mode on a drive offers **Install N games** and **Remove from drive…** (two
    clicks: "Remove N from drive"); the result reads "Removed 3 games from <pack>".

  **Select mode** (Library and Server, not Discover). **Select** in the toolbar turns it
  on and reads **Done** while on. A click on a cover picks it instead of opening Details;
  Shift-click picks a range. A picked cover shows a filled check and a ring, and its hover
  bar is hidden. A bar pinned to the bottom of the grid shows "3 selected · 4.2 GB",
  Select all N / All N selected, Clear and the action: **Backup to drive…** on installed
  games and on Server. Esc or Done leaves select mode, and so does changing the rail
  entry. A Library platform row also has **Backup all N…** in its header, for its
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
  the background. A platform header has **Backup all N…** on its right, which opens the
  Backup dialog for every game of the platform, and the toolbar has Select (see Select
  mode above).

  **Offline.** In offline mode Server shows one panel in place of the rail and grid: "Not
  connected to a server", "Your Library and backup drives work without one. Connect to
  browse the server and backup games." and **Connect to a server…**.

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
  the app after first-run setup, opens on it (unless Open GRID on says Library). It has no folding header and no search;
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
  "Failed — <reason>". A game copied from this computer reads "Getting details from
  server" (its details and art still come from the server), then "Copying from this
  computer to <label> 45%" (disk sparkline), and finishes as "Copied from this computer ·
  Written to <pack>". Cancel works while it downloads and while it writes. A backup's
  game rows run first, then its emulators, then its firmware; backups and installs share
  one queue, first in, first out. When the drive is removed, the row being written fails
  ("Failed — SAMSUNG T7 was removed") and the drive's waiting rows fold into one
  Completed row, "13 more items" with "Cancelled — SAMSUNG T7 was removed · backup
  again to finish", which offers only Dismiss. A failed or cancelled backup row can be
  retried while its drive is connected. An install from a drive reads "Copying from
  <label> 45%" while it copies.

  **Free space.** Every queued or running job holds the space it will still add, and
  every "does it fit" check uses what is left: the disk's free space, less what queued
  and running jobs on that disk still hold, less a 64 MB safety margin. An install,
  update or extra content from the server, an emulator install and an install from a
  drive hold their peak in the library folder from the moment they are queued: the
  download (the server's file sizes; when one is missing, the response's size once it
  arrives) plus as much again when the download is extracted. A file already at its
  destination needs no new space. When the peak does not fit, nothing is queued and the
  error card reads "Couldn't add <Title> to the download queue: There isn't enough free
  space in the library folder for <Title> (needs 1.2 GB, 800.0 MB available)". Before a
  zip is extracted its real unpacked size (from its directory) replaces the estimate;
  when that no longer fits, the download is deleted and the row fails with the same
  reason. What a job has written stops counting against it (the disk already shows it),
  and a job that finishes, fails or is cancelled gives its space back. Installing a game
  that is already queued is ignored, never refused for space. A library move's space
  check counts what queued jobs hold too.
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
- **Appearance** (under Settings) starts with **Open GRID on**, a select with "Automatic:
  Discover when connected, else Library", "Server › Discover" and "Library", and the help
  text "Here and in TV mode" (see Where GRID opens under Top Bar). It applies the next time
  GRID opens and is stored as `open_on` (`automatic`, `discover` or `library`) under `[ui]`.
  It then chooses the theme — follow the OS, dark, or light;
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
  `card_display` (`rows` or `columns`); card display is shared with TV mode, while card
  size and game labels are the desktop's alone: TV mode has its own (`tv_card_size`,
  `tv_card_labels`; see TV mode). `[ui]` also holds TV mode's `tv_prompts`
  (`match`, `xbox`, `ps`, `nintendo` or `8bitdo`) and `tv_assignments` (a table of
  controller key → `xbox` or `nintendo`; two controllers of one model share a key).
  **Hardware acceleration** (on by default) renders
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

**A system with no emulator.** When the game isn't installed and its system has no
emulator (and isn't set to None on purpose), a line under Install reads "No emulator for
<system>. Install installs <rec> too." when GRID can install a recommended emulator for
it, and Install then queues the game and installs that emulator too (the same install as
Settings › Systems' per-system button). While that emulator installs the line reads
"<rec> is installing." and Install brings only the game. With no recommendation it reads
"No emulator for <system>. Add one in Settings › Systems." The line replaces the
button's hover reason. TV mode's Details does the same, and there the game's queued card
adds "<rec> installs with it".

**Backups in Details.** Under the install actions sits **Backup to drive…**, which opens
the Backup dialog for this game (offline it is disabled, and a line reads "Connect to a
server to backup games."). It is not shown for a Missing game. When connected drives
hold the game already (same server and ROM id), a line under the actions reads "On
<pack>" ("On Couch kit, Travel kit" for several). Details opened from a drive's grid adds
"From <pack>" to the header line. It offers **Install from drive** in place of Install
when the game is not installed here, and **Remove from drive…** in place of Backup
(two clicks; the second reads "Remove from <pack>"). A drive game backed up from a
computer's install adds "A copy of an installed game · v1.1", with "(the server had newer
files)" when it was out of date at backup time (no version part when the copy has no
version tag). A Windows one adds "Saves from its Wine prefix are included" when its saves
were copied, else "Its Wine prefix wasn't copied, so saves kept there aren't on the
drive". A downloaded Windows game whose saves were copied adds the first line alone.

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
  The trailer plays inside GRID, in YouTube's own player (with its own controls), framed
  by a page on GRID's loopback media server, because YouTube refuses an embed straight
  from the app's page on Linux and macOS. "Watch on YouTube" sits under the player at all
  times and opens the trailer in the browser. Nothing overlaps the player (YouTube's
  rules): it keeps clear of the notification cards' column, the arrows sit beside it, and
  it is at least 480×270. When YouTube refuses the trailer (errors 2, 5, 100, 101, 150,
  153), its player script doesn't load, or it isn't ready within about 10 s, the poster
  comes back with "Watch on YouTube" and "This trailer can't play in GRID.". Keyboard
  focus stays with the viewer: a click in the player hands it back, so Esc and the arrows
  keep working.
  Every video player (the trailer and a server video, here and in TV mode) has its own
  volume, separate from the system volume: it opens at the stored level
  (`[ui] video_volume`, 0–100, default 50), and a change made with the player's own
  controls is stored, about 500 ms after the last change, for every later video. TV mode's viewer
  (TVMode README § Decided, 2026-11-10) plays videos too: A plays and pauses, LB / RB seek
  10 s, LT / RT step the volume by 10 with a "Volume N%" readout beside the player, Left /
  Right page and B closes.
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
GRID backs up games to a removable drive (a USB flash drive or SSD), with their art,
details, emulators and firmware. A game installed on this computer is copied from the
library; any other game downloads from the server. Any computer with GRID can then install
them from that drive, with or without a network. The design is
`docs/design/system/components/GameBackups/README.md`.

Epic, Amazon and Steam games (see "Game stores") are not backed up. The Backup dialog
shows them as ineligible with the reason "Store games are not backed up". GOG games are
backed up: the pack holds the installer files when GRID still finds the installer it
installed from (restoring runs it again), otherwise the installed copy and its saves.

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
  (GRID is what the `gridpack` folder takes on the drive, art and files a backup is still
  writing included; the bar's total leaves out space the file system keeps for root, such as
  ext4's 5%), then Games, Emulators, Firmware, Last backup and the folder path, then **Show in
  Library**, **Open folder**, **Rename…** (inline) and **Remove GRID files…**. Remove asks
  again ("Delete the gridpack folder from <label>?"), says how much it frees, deletes only
  `gridpack/`, and leaves the games installed on this computer.
- **A remembered drive that is not connected** shows its name, label, file system and
  size, when it was last seen and its game count, with **Forget**.
- **Every backup also copies** holds one default for the Backup dialog: "The server's
  firmware for the systems you backup", on by default (`[backup] include_firmware`).
  Each game always takes its art and details, and each system its emulator.
- **What's in the gridpack folder** (collapsed) shows the layout below.

## The Backup dialog
Opened by Details › Backup to drive…, the select bar's Backup to drive…, and Backup
all N… (Server platform header, Library platform row). It opens over every view and over
Details.
- **Title:** "Backup Cinder Peaks", "Backup 3 games" or "Backup PlayStation · 4
  games". The lede reads "GRID copies games installed on this computer and downloads the
  rest from <host>, then writes each one, with its art and details, to the drive's
  gridpack folder." When no game is copied from this computer it reads "GRID downloads
  each game from <host> and writes it, with its art and details, to the drive's gridpack
  folder."
- **Drive:** one radio row per connected, set-up, writable drive: the pack name, "<label>
  · <file system>", the space left for this backup, and a usage bar where this backup's
  share is hatched. The space left is the drive's free space less what queued and running
  backups to the drive still hold (see Free space under Downloads); rows for games this
  backup would write again are not counted twice. While some space is held the row reads
  "60.2 GB available", with "1.2 GB set aside for queued backups" under the bar, which
  shows the held share in a muted disk colour. A drive without room reads "Needs 76 GB ·
  61.4 GB free" ("… available" while space is held) in the danger colour, and the main
  button reads "Not enough space". With none connected, the dialog says "No backup
  drive connected", names any connected drive that is not set up, and offers **Set up a
  drive…**, which opens Settings › Backup drives (and closes Details).
- **Source** sits between Drive and Also copy. It shows only when at least one requested
  game can be copied from this computer.
  - **Copy installed games from this computer** is a checkbox, ticked by default. Its note
    counts the sources: "4 games from this computer (12.3 GB) · 2 downloaded · 2 to
    choose". Unticked, the note reads "Every game downloads from <host>", and the rows
    and the out-of-date choice are hidden.
  - **One row per game being written**, with its platform and a tag: **From this
    computer** (drive icon) or **Download** (download icon). A game that is installed but
    downloads says why: "Its files weren't found on this computer" (Missing), "Installed
    outside the library folder" (a native game moved with Locate…, a PS3 game in RPCS3's
    `dev_hdd0`, a launch file outside the game's folder), "Its install contains links",
    "Its install has a file name Windows can't use" (on a pack for Windows), "Its install
    has a file name that isn't valid text", "Its install contains a file GRID can't
    copy" (a FIFO, socket or device file), "Its files couldn't be read on this computer",
    "Xbox 360 games always download from the server", "Emulator packages always download
    from the server", "It was installed from another server", "It isn't linked to this
    server's game" (a row with no ROM id), or "It's running right now; close it to copy
    it". A game that is not installed has no note. With more than 6 games the rows sit in
    a collapsed **Show each game** (with the count).
  - **Which installs are copied.** The install must be for this ROM id and this server,
    present, and sit directly in the library's platform folder for its platform
    (`games/<Platform>/`, or the legacy `<library>/<Platform>/`), with its launch files
    inside it. Its tree is scanned when the dialog plans; the scan is reused for a minute.
  - **What is copied.** An install that is exactly the server's files (a single file
    installed as downloaded, or a multi-file game whose files are all there with the
    server's names and sizes) goes on the drive as those files, the same as a download.
    This applies only when the game is not out of date. Any other install (an extracted
    game, a PS4 tree, a native Linux or Windows game) goes as an uncompressed tar in parts
    (see The gridpack folder). A native game's Wine prefix is never copied as a whole;
    only the save folders GRID knows about are (see Saves of a Windows game). When a
    Windows game is copied, the section says "Windows games are copied without their Wine
    prefix. The save folders GRID knows about are copied from it."
  - **Out of date.** When the server has newer files for a game that can be copied, its
    tag reads **Choose below** ("the server has newer files"). A block titled "The server
    has newer files" ("… for 2 games" with more than one) says "Backup the copy installed
    here, or download the newer version." Each game reads "<Game>: the server has newer
    files (v1.2; this computer has v1.1)" ("an older copy" when the install has no version
    tag), with two radios, **Use this computer's copy** and **Download v1.2 from the
    server** ("Download the newer version from the server" when the server's file has no
    version tag). With more than one game, two buttons set every choice: **Use this
    computer's copies** and **Download all**. A pick changes the row's note to "an older
    copy than the server's" or "the newer version". Out-of-date comes from the update
    check when one has finished this session, else from each game's details (4 at a
    time); a game whose details fail to load counts as up to date.
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
  already on Couch kit and will be skipped: Moss & Marrow"). On a FAT32 drive, games that
  download and have a file over 4 GB are listed as not possible. A game copied from this
  computer is split into parts, so the limit does not apply to it.
- **Footer:** "11.1 GB to write · 3.2 GB to download", Cancel and **Backup 3 games**. The
  second half shows only when some game is copied from this computer and something
  downloads; emulators and firmware count as downloads. A copied game counts the exact
  size of its archive. The button reads "Choose for 2 games" (disabled) while an
  out-of-date game has no pick, "Nothing to backup" when there is nothing new, and "Not
  enough space" when it does not fit. The bytes to write plus a safety margin must fit
  the space left. `start_backup` refuses a plan that still waits on a pick ("Choose
  which copy to backup for 2 games"). It then checks the space again and holds it in
  one step, so two quick starts cannot both pass: when the backup no longer fits, nothing
  is queued ("SAMSUNG T7 doesn't have enough free space (needs 76 GB, 61.4 GB
  available)"). Each row holds its bytes on the drive and, unless it is copied from this
  computer, its staging space in the library folder. What a row has written stops
  counting; its staging space is given back once staging is deleted, and the rest when
  the row ends.

Backups run in Downloads (see Downloads above): each item downloads into a staging
folder in the library (`<library>/.grid-staging/backup-<id>/`), then is written to the
drive. The staging folder is always deleted afterwards. A game copied from this computer
puts only its art in staging; its files are written straight from the library to the
drive.

When the row starts, GRID judges the install again. A game no longer eligible fails the
row ("<Game> is no longer installed on this computer; backup again", or "<Game> can't be
copied from this computer: <reason>; backup again"); it never downloads instead. A file
whose length changed since the scan fails the row ("<file> changed while it was being
copied; close the game and backup again").

While a game copied from this computer has a live row (queued, getting details or
copying):
- Uninstall, Update and Locate… on that game are refused ("This game is being backed up.
  Wait for it to finish.").
- The library folder can't be moved while the copy is being written.
- A game that is running is not copied: the plan lists it as downloading ("It's running
  right now; close it to copy it"), and a queued copy of a game launched since fails with
  that reason. Only games GRID launched are detected.

## Install from a drive
**Install** on a drive's game copies it off the drive and installs it exactly like a
server install: the same file choice and the same extract or finalize steps. Only the
bytes come from the drive. The game's art from the drive goes into the image cache first,
so its cover shows offline. The installed game keeps its server ROM id, so updates,
cloud saves and play activity work once the computer connects to that server. On a
connect to another server, such games lose their ROM id before the re-link pass (which
may link them again to that server's game), so cloud saves and play activity never
reach the wrong ROM.

**A copy of an installed game** (a game with an `installed` block) is restored, not
planned like a server install. A copy written as the server's files has no such block
and installs like any other drive game.
- **Before it is queued,** GRID refuses a copy it cannot restore ("This backup needs a
  newer GRID": an unknown `archive` or `layout`), a pack path that is not safe, and a
  library without room for the tree (`tree_bytes`) after what queued jobs hold and the
  safety margin ("There isn't enough free space in the library folder for <Game> (needs
  X, Y available)"). The restore holds that space until it ends.
- **Where it goes:** always the modern platform folder, `games/<Platform>/`. An
  `extracted` copy goes to `<platform>/<copy name>`; a `native` or `multi` copy to
  `<platform>/<game name>`, where a server install puts it; a `file` copy to
  `<platform>/<file name>`.
- **The copy:** the parts are read off the drive in order into
  `<platform folder>/.grid-restore-<rom id>`, then moved into place. The Downloads row
  reads "Copying from <label> 45%". A folder already at the destination is replaced only
  when no installed game uses it ("<path> is already used by another game"). A removed
  drive fails the row ("<label> was removed"); a part with the wrong size or MD5, a
  missing part or a refused entry fails it as "The backup of <Game> on <label> is
  damaged". The temporary folder is removed on any failure or cancel.
- **The row:** the launch file must be inside the tree ("The backup of <Game> has no
  launch file at <path>"). Launch parameters, working folder, PS4 ids, included DLC and
  the RetroAchievements id come from the copy. A native game that kept its archive
  inside its folder gets that archive back as its kept archive. A native game restored
  on Linux or macOS gets a new `prefix/` as its Wine prefix (holding only the saves the
  backup copied, see below), and the default compat tool. The ROM file name and the
  server timestamp are the copy's, so a copy that was out of date at backup time shows
  Update once the computer is connected to that server.

**Saves of a Windows game.** A backup of a Windows game records every save folder GRID
knows for it, the list cloud saves use (PCGamingWiki's and the user's own, less the ones
the user removed), read from the settings with no lookup. The folders are stored as
Windows paths (`%APPDATA%\Foo`, `%USERPROFILE%\Documents\My Games\Foo`); a folder the
user added inside the game's Wine prefix is turned into that form (a folder elsewhere on
drive C becomes `%SYSTEMDRIVE%\…`), and one outside the prefix is left out. The folders
are recorded even when none of them exist.
- **Which user folder a prefix uses.** A Windows save path such as `%APPDATA%\Foo`
  lives under `drive_c/users/<user>/` in a Wine prefix. Proton (run through umu-run)
  names that user `steamuser`; plain Wine uses this computer's user name. GRID uses
  `steamuser` when the prefix has that folder, else this computer's user name when the
  prefix has that one. A prefix with neither (a new one, such as right after an install
  from a drive) takes the user of the compat tool that will run the game, picked the way
  the launcher picks it (the game's own tool, else the Windows default in Settings): a
  Proton tool gives `steamuser`, `wine` or no tool gives this computer's user name. Cloud
  saves and backups follow the same rule.
- **Copy.** When the game is installed on this computer (Linux or macOS) with a Wine
  prefix, the folders that exist in the prefix are copied to the drive as one
  uncompressed tar in parts, checked like every other file. This happens whether the
  game itself is copied from this computer or downloaded, with no checkbox. Nothing else
  in the prefix is copied. A folder holding a link, or a name the drive's systems can't
  hold, is left out with a warning ("Saves in <folder> weren't copied: it contains
  links"). While the game is running its saves are not copied; the backup goes ahead
  and the row warns "Its saves weren't copied because it's running". A failed saves
  write (other than the drive going away or a cancel) is a warning, and the game is
  still on the drive with its folders recorded.
- **Restore.** Installing the game from the drive, as a copy or as downloaded files,
  puts the saves back once the game is installed. On Linux or macOS they go into the
  game's new Wine prefix, under the Windows user folder that prefix uses (see Which user
  folder a prefix uses); on Windows into the real folders. A folder that already holds files is never written into: it is skipped and
  the row warns "Saves for <folder> weren't restored: <path> already has files". The
  recorded folders are also given to cloud saves for that game (as its PCGamingWiki
  list when it has none yet, else added to its own list; nothing there is replaced or
  removed), so cloud saves know them without a lookup. A saves problem never fails the
  install; it is a warning on the row.

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
  games/<platform>/<name>/installed.tar.001…  a copy of a computer's install, in parts
  assets/<platform>/<game>/                 cover, fanart, logo, screenshot-01 … (up to 8)
  saves/<platform>/<game>/saves.tar.001…    a Windows game's saves from its Wine prefix, in parts
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
  hashes) and its asset paths. A game file's hashes are RomM's, kept for identification
  only: for an archive they describe its contents, so nothing checks a file against them.
  A game copied from a computer's install has an `installed` block and may have no
  `files` (see below). A Windows game may have a `saves` block (see below);
- `emulators`: one entry per emulator per OS, with its kind (`emulator` or
  `compat_tool`, which covers Proton, umu-launcher and the Steam Linux Runtime), source,
  version, platforms, archive path and size;
- `firmware`: per platform, each file's name, path, size and MD5. RomM serves firmware
  as is, so this MD5 is the file's own and an install from the drive checks it.

Every path is relative to `gridpack/`, so the drive works under any mount point. Unknown
keys are kept when GRID rewrites the file. The full shape is in the design README.

**Copies of an installed game.** An install that is exactly the server's files is
written as those files, with no `installed` block, the same as a download. Any other
install is one uncompressed GNU tar, cut into `installed.tar.001`, `.002`, … parts of at
most 4095 MiB on every drive, so each part fits FAT32. The folder is named after the
install's folder (a file's name without its extension), else the server file name. The
`installed` block holds:
- `archive` (`tar`) and `layout` (`extracted`, `native`, `multi` or `file`);
- `name`, the folder or file name in the platform folder;
- `parts`, in order, each with its path, size and the MD5 of the bytes GRID wrote; the
  game's `size` is their sum;
- `tree_bytes` (the space a restore needs) and `tree_files`;
- `launch`, the launch file relative to the tree;
- `rom_file_name`, `server_updated_at` and `version` of the installed copy, and
  `outdated` when the server had newer files at backup time;
- `from_os`, the OS the copy was made on (an unknown value reads as none);
- `row`: the native executable and a kept archive as paths inside the tree, the launch
  parameters and source, the working folder (only when relative), PS4 ids, included DLC
  and the RetroAchievements id. Never an absolute path.

A native game's Wine prefix (`prefix/`, and a recorded prefix inside the game's folder)
is left out. An unknown `archive` or `layout` keeps the pack readable; Install says
"This backup needs a newer GRID". The parts rebuild without GRID:
`cat installed.tar.* | tar x`.

**Saves of a Windows game** (see Install from a drive). The optional `saves` block holds:
- `folders`: the save folders GRID knew at backup time, as Windows paths
  (`%APPDATA%\Foo`), recorded even when none existed;
- `parts`: the saves copied out of the game's Wine prefix, one uncompressed GNU tar in
  `saves.tar.001`, … parts written like an installed copy's, with the folder at position
  `i` of `folders` under `i/` (`0/slot1.sav`). Absent when nothing was copied;
- `tree_bytes` and `tree_files` of the copied saves.

A pack written before `saves` existed reads as having none. The saves count toward the
pack's share of the drive, and removing the game removes them.

## Safety rules
- GRID never reads, writes or deletes anything outside `gridpack/` on a drive. The one
  delete of the whole folder is Remove GRID files…, behind a check that the target is
  exactly that folder.
- Every file is written to `<name>.gridtmp`, synced, checked, and only then renamed into
  place. The check is against the bytes GRID copied (from staging, or from the library
  for a game copied from this computer): the file's size, then the file is read back from
  the drive (past the system's cache where the OS allows) and its MD5 must match the MD5
  of the bytes copied. The server's hashes are never used for this, because RomM
  hashes an archive's decompressed contents, not the archive. The read-back adds about a
  third to a half to each file's write time. `gridpack.json` is rewritten the same way after each finished item, so a drive
  pulled mid-write always holds a valid index and at most a `.gridtmp` leftover.
- Each part of an installed copy follows the same rule: it is written to
  `<part>.gridtmp`, hashed as it is written, synced and read back from the drive. Only
  when every part is checked are they all renamed. On any failure the parts written are
  removed (unless the drive is gone).
- On restore, every part's size and MD5 is checked, the last part's padding included.
  Only regular files and folders are accepted. Links (symbolic or hard), devices, FIFOs
  and any entry name that is not a plain relative path (absolute, `..`, `.`, an empty
  part) are refused, and the restore fails. Unix permission bits are kept, masked with
  `0o777`.
- Leftover `.gridtmp` files are removed the next time the drive is connected (not while
  a row is writing to it).
- A drive's `gridpack.json` is untrusted. A file over 64 MiB, one that does not parse, an
  unknown OS value or an unsafe path makes the drive "Can't read". Every path must be a
  plain relative path (no `..`, no absolute path or drive prefix, no backslash) and must
  not lead outside `gridpack/` through a link; a `gridpack` folder that is a link is
  refused.
- A pack written by a newer GRID is read-only.
- Logs and errors carry drive labels, paths and game names, never a server credential.

# TV mode
TV mode is GRID's second interface: fullscreen, for a TV, and driven entirely with a game
controller. It reaches every feature of the desktop app. The design, with every word and
state, is `docs/design/system/components/TVMode/README.md` (its **Decided** and
**Consultant decisions** sections are binding); `docs/tv-mode/checklist.md` lists each
testable statement.

## Entering and leaving
- The desktop top bar's TV button (a 28×28 icon button left of the light / dark toggle,
  "Switch to TV mode") opens TV mode on the same view; starting GRID with `--tv` starts
  in it. Menu › Switch to desktop mode, also in Settings › TV mode, leaves it, on the same
  view. There is no other way in or out, and the mode is never saved: without `--tv` GRID
  starts in desktop mode.
- TV mode makes the window fullscreen through the window itself (never the HTML
  Fullscreen API). Leaving restores the desktop window, and the window geometry GRID saves
  is always the desktop one, so a desktop start never opens fullscreen.
- Where GRID opens follows the same rule as the desktop (Where GRID opens, under Top Bar):
  Server › Discover when connected, else the Library (on the first connected Backup drive
  without a server). A "Welcome to TV mode" card shows the first time TV mode opens
  (`[ui] tv_welcomed`); Menu › Controls shows it again.

## Controllers
- GRID reads controllers natively (a Rust reader, only while TV mode is on). Input
  reaches GRID only while its window is focused, so a game in front gets the controller.
  While a game runs and GRID's window is focused, presses reach the Playing page (Back to
  the game, Stop game). When the last game closes GRID comes back to the front and
  listens again. The Home / Guide button does nothing.
- **Button assignments** (Settings › TV mode): Xbox style (bottom selects, right goes
  back) or Nintendo style (right selects, bottom goes back), per controller and kept per
  controller. A controller starts on its own layout's style. With several controllers,
  each uses its own, and the prompts follow the one pressed last.
- **Button prompts** match the controller (Xbox, PlayStation, Nintendo, 8BitDo Pro sets,
  Kenney's CC0 glyphs) or a set picked by hand. With no controller the hint bar reads
  "No controller · the keyboard works too".
- Buttons: D-pad / left stick move (held: repeat after 400 ms, then every 90 ms); A
  selects; B goes back; X is the focused item's quick action; Y the screen's action; LB /
  RB switch tabs (Details tabs on Details); LT sorts; RT is select mode; Start presses a
  page's main button where it has one, else opens the Menu; View acts on the newest notice
  that offers an action; the right stick scrolls what you read. A keyboard and mouse work
  too (Enter / Space A, Esc / Backspace B, Q / E LB / RB, Z LT, R RT, M Menu, V View;
  pointing focuses, a click is A).

## The shell
- **Size.** Every measure is in rem; the root font is 1/54 of the screen height, capped at
  1/80 of its width, so the layout is the same at 720p, 1080p and 4K. No text is smaller
  than 0.85rem.
- **Three bands:** the top bar (4.6rem: wordmark with a TV tag, the four tabs between
  their LB / RB prompts, or a breadcrumb on a pushed page, then the live download, the
  connection and a clock), the stage, and the hint bar (3.6rem: the controller's name or
  the offline notice, then what each button does now). The window itself never scrolls.
- **Focus** is a 0.2rem ring; a focused cover grows to 107%. Focus is never lost, and
  going back returns it to where it was. Movement is spatial, inside a rail, row or list
  along its axis; Right from a rail is A. Scrolling follows fixed rules (a Discover shelf
  brings its title to the top; a sideways row keeps the cover 1.2rem inside; everything
  else 1.2rem inside its area), and nothing focused is ever cut by a scrolling area.
- **Background art, theme, notifications** as on the desktop, with TV's sizes: Follow
  system shows dark; the cards sit at the top right, at most three, never take focus, and
  every card goes to Menu › Notifications (the whole session).
- **TV grid settings** (Settings › Appearance, after Theme): **Game card display**
  ("Here and in desktop mode", the shared `card_display`), then TV mode's own **Card size**
  ("Every grid in TV mode": Small, Medium (default), Large; one size for every TV grid,
  cover widths 8.5, 10 and 12.5rem; Discover's shelves and Details keep theirs) and **Game
  labels** ("The title under each cover": Always visible (default), or Visible on focus,
  where covers show no title line and the focused cover's title floats under it as a pill
  without moving the layout). They are stored as `[ui]` `tv_card_size` (`small`, `medium`,
  `large`) and `tv_card_labels` (`always`, `focus`), apart from the desktop's
  `card_size_library`, `card_size_server` and `card_labels`: changing one mode never
  changes the other. TV mode never sends the player to desktop mode for them (2026-11-14).

## Screens
- **Tabs:** Library, Server (Discover first, then the platforms), Downloads, Settings (the
  desktop's panes plus TV mode). **Pages** pushed over a tab (B goes back): Details and its
  tabs (Overview, Media, Saves, Achievements), Playing, System, the emulator form, Game
  settings, Link to server game, the folder browser (every path in TV mode), Search
  everything, Notifications, the Backup page and the drive pages. **Overlays:** the Menu,
  lists, confirmations (Cancel focused; a destructive action always asks) and the
  on-screen keyboard.
- **Startup:** Connect (with View to paste and Use without a server), Choose your systems
  (Start skips) with the setup card, and the unreachable-library dialog. On Connect and
  the unreachable-library dialog Start opens a short Menu: Switch to desktop mode and
  Quit GRID.
- **Game backups** are the Game backups design with a controller's controls: select mode
  on RT, the Backup page as a page whose main button Start presses, Backup drives rows in
  the Library rail, and Settings › Backup drives with a page per drive.

# Game stores
Status: all four stores are built. Phase 1, GOG with offline installers (2026-11-13);
Phase 2, Epic Games Store through legendary (2026-11-13); Phase 3, Amazon Games through
nile (2026-11-14); Phase 4, Steam through the Steam client and the Steam Web API
(2026-11-14). The login and real-account checks wait for the user's own test
(`docs/external-stores-plan.md`, "Waiting for the user's real-account test"). The plan is
`docs/external-stores-plan.md`, the work plans `docs/stores-phase1-plan.md`,
`docs/stores-phase2-plan.md`, `docs/stores-phase3-plan.md` and
`docs/stores-phase4-plan.md`. The approved design is
`docs/design/system/components/GameStores/README.md` and
`docs/design/system/components/TVStores/README.md`.

GRID installs and launches games from external stores beside RomM games.

User decisions (2026-11-13):
- **Stores.** GOG, Epic, Amazon and Steam, plus installing GOG games from offline
  installers. They ship in this order: GOG with offline installers, then Epic, Amazon,
  Steam.
- **Tools.** GRID uses Heroic's CLI tools as they are: legendary (Epic), heroic-gogdl
  (GOG) and nile (Amazon). They are GRID-managed downloads with a pinned version and
  SHA-256 per OS and architecture, installed the first time their store is connected, before the sign-in (each tool
  redeems its store's login code), so Connect never fails for a missing tool.
  Settings shows each tool's license and a source link.
- **Tokens.** The tools keep their own token files under `<data_dir>/stores/<tool>/`
  (folder 0700, files 0600). Disconnect deletes them. This is a narrow, documented
  exception to the keyring-only rule (AGENTS.md "Secrets"). GRID's own secrets, the
  Steam Web API key included, stay in the OS keyring. Tool output is never logged or
  forwarded.
- **Steam.** Steam needs the user's own Web API key and SteamID64. The owned library comes
  from `GetOwnedGames`, so the profile's game details must be public. Install, uninstall
  and launch go through the Steam client with `steam://` links. Steam games installed
  outside GRID appear automatically. Only games appear: Proton and the Steam runtimes are
  filtered out.
- **Placement.** Each connected store gets a group in the Library rail, like Backup
  drives. The group's title is the store's name, with two sub-labels: **Installed** and
  **Available**. Desktop and TV mode work the same way.
- **Store cloud saves.** `gogdl save-sync` (GOG) and `legendary sync-saves` (Epic) are in
  scope, each in its store's phase. RomM cloud saves stay off for store games.
- **Logins.** A store's login page opens in a GRID window that catches the store's
  redirect in Rust; the code and the tokens never cross IPC. In TV mode the login page
  opens over TV mode, with the on-screen keyboard.
- **Offline installers.** A Linux `.sh` installer is read as a zip, and GRID takes
  `data/noarch/` from it without running the script. A Windows `setup_*.exe` runs
  silently under Wine/Proton (natively on a Windows host). When the silent run fails (a
  non-zero exit, no progress for 120 s, or its Inno Setup log says it rolled back), GRID
  runs it once more with milder silent flags, which still need no input. In desktop mode
  the installer's own wizard is the last fallback.
  The game is identified from `goggame-*.info`; a DLC installer attaches to its base game
  by `rootGameId`. A macOS `.pkg` is refused with a reason. An offline install is the
  same GOG product as an online one. TV mode runs Windows installers silently only and
  never sends the user to desktop mode: when both silent runs fail, the installer is kept,
  and the Downloads row and the card give the reason read from the installer's log in
  GRID's words (not enough disk space; a file in use or access denied; the installer's
  files missing or damaged; the installer cancelled itself; else "the installer stopped
  without saying why"). The card offers Retry, which runs both silent attempts again
  without downloading again. The row offers Retry and Delete installer. GRID never shows
  or logs the log's own text, or the installer's output.
- **Installers on the RomM server.** The user zips GOG offline installers and uploads
  them to RomM; GRID never uploads them. When a downloaded RomM archive holds an
  installer instead of loose game files, GRID runs the offline-installer flow. The game
  stays a RomM game, with every RomM feature. Once installed, Details says "Installed by
  running its GOG installer". Before the download GRID can't know what the archive
  holds, so Details says nothing about an installer then.
- **GOG backups.** GOG games that did not come from RomM can be backed up (see "Game
  backups"). Epic, Amazon and Steam games cannot.

Identity: a store game is keyed by its store and the store's product id (`gog:1207664643`);
a RomM game keeps its rom id (`romm:123`). Store rows carry the platform names "GOG",
"Epic Games", "Amazon Games" and "Steam".

What store games get:

| Feature | Store games |
|---|---|
| Play activity | Off |
| RomM cloud saves | Off |
| Store cloud saves | On, for GOG and Epic (Amazon has none) |
| Drive backups | GOG on; Epic, Amazon, Steam off ("Store games are not backed up") |
| Relink and server update checks | Off |
| Each store's own update check | On (Steam updates its games itself) |
| Last played, rail, sort, search | On |
| Details Overview, Game Settings, uninstall | On (Steam: no Game Settings; Steam uninstalls) |

GOG, Epic and Amazon games launch like native games, from their install folder (with
Wine/Proton for a Windows build). A Windows GOG game's Wine prefix is `prefix/` in its
install folder, however it was installed (gogdl, an offline installer, a backup drive),
so uninstalling the game removes it and a backup leaves it out. A DLC installer runs in
its base game's prefix. A Windows GOG game installed through gogdl gets its setup steps
right after the download, silently, in its prefix: first GOG's setup script for the game
and each installed DLC (GOG's script interpreter, or the game's own setup program), which
writes the registry keys, CD keys and language settings the game expects; then the
redistributables its build lists (Visual C++, DirectX, OpenAL, .NET and the like),
downloaded once by gogdl into a folder all GOG games share. Game-folder items (DOSBox,
ScummVM) are never run, and neither are QuickTime, Foxit or Adobe AIR. After an update
only the setup script runs again; a DLC add runs the new DLC's script; a repair runs
none. A Linux build, an offline installer (it runs its own) and a backup-drive restore
have none. Skipping what is already installed, the one Windows permission prompt, the
warnings and Retry ("Setup" row) work as for Amazon below; when gogdl can't download the
redistributables, the row reads "Couldn't run its setup steps: GRID couldn't download
them (…)" and Retry tries again. Store art (covers, screenshots) is fetched from the store's own
image hosts without any credential and cached under `<data_dir>/stores/art/`. An Epic launch carries a one-time exchange code, which
is never printed in a log, an error or a warning. A Steam game's session is watched
rather than spawned: Steam starts the game and GRID sees it running until Steam reports
it closed.

Store screens (2026-11-13, matching the approved design):
- **Save conflicts.** Both sides show when they changed and their size: this computer's
  files and bytes from its save folders, the cloud's file count and bytes from GOG's cloud
  listing (the conflict prompt shows the bytes only, the Cloud saves tab both). A size the
  listing doesn't give is left out, never guessed.
- **Offline installer space.** The installer card reads "Needs <size> · <free> free": about
  1.4 times the installer's size, and the free space on the volume it installs to (the new
  game's folder, the installed game's for an update, the base game's for a DLC). Picking
  another install folder reads that folder's volume again. It doesn't block the install.
- **Game Settings › Executable** (store games only; RomM native games keep their own
  Game Settings) is a list of the game's executables, found up to four path parts deep in
  its folder: `.exe` files for a Windows build, `.sh` scripts and ELF binaries for a Linux
  build. The Wine prefix, uninstallers (`unins*`), installers (`setup*`), crash reporters
  and redistributables are left out. A file the user picked before that isn't in the list
  stays in it, first. There is no file chooser. Picking another one makes it the user's
  choice; saving with the list untouched pins nothing.
- **Epic Games** (Phase 2). legendary is installed when Epic is connected, before the
  sign-in code is redeemed (legendary redeems it). Every Epic game is its Windows
  version, launched under Wine/Proton. Titles another launcher manages (the EA app,
  Ubisoft Connect) are listed with a NOT SUPPORTED tag; Details names the launcher and
  GRID can't install them. Covers are the portrait images, like GOG's. The install dialog
  has no Version or Language rows. An update or a Verify and repair also runs each
  installed DLC, and a repair's result shows as "Verified · N files repaired". Signed
  out, a game that may run offline still launches offline; signed in, when Epic can't be
  reached, GRID retries such a game once offline. Epic cloud saves always sync to the save
  folder GRID resolves from the game's cloud folder; before a download replaces the local
  save, a copy goes to `<data_dir>/stores/save-backups/epic/<game>/` (the newest five
  kept). Epic games are never backed up to a drive. A game's prerequisite (the one
  installer legendary lists for it, such as Unreal Engine's prerequisites) runs silently
  as the last step of its install, and after an update or a Verify and repair only when
  it has changed since it last worked (never after a DLC install). It runs from its own
  folder with the arguments Epic gives it, exactly like Amazon's `PostInstall` steps: in
  the game's prefix under its compat tool on Linux, in one elevated run (one permission
  prompt) on Windows, skipped when what it installs is already there, with "Running the
  installer silently" on the row, a failure or a declined prompt as a warning, and Retry
  running only it (the "Setup" row; no legendary call, no login). Nothing runs for a game
  that lists none, one legendary already marked installed, or a title another launcher
  manages. GRID never changes legendary's own record of it.
- **Amazon Games** (Phase 3, 2026-11-14). nile makes the sign-in URL, so on Connect GRID
  installs nile first ("Installing nile (11 MB) for Amazon Games…"), then opens the login
  window. The sign-in code goes to nile on its command line for the one short `register`
  run (nile has no other way to take it); it is never logged or forwarded. Every Amazon
  game is its Windows version, launched under Wine/Proton with nile's FUEL environment;
  after a Disconnect, installed games still launch, as they are. Amazon games have no
  cloud saves (no Cloud saves chip, tab, row or switch, and never a SAVE CONFLICT) and no
  DLC. nile's versions are opaque ids, so no version is shown: the button reads
  "Update", the cover keeps its UPDATE tag, the card reads "<game>: an update is
  available", and Overview and the Details header leave the version out. A verify reads
  "Verified" with no count. A game's fuel.json `PostInstall` steps (redistributables
  such as OpenAL and DirectX) run silently after its install, and after an update or a
  verify when the list has changed since it last worked. Steps whose redistributable is
  already installed are skipped. On Linux they run under the game's compat tool in its
  own prefix, with no prompt. On Windows all of one job's steps run in one elevated
  runner, so there is one permission prompt, in TV mode too. While they run, the row
  reads "Running the installer silently". A step that fails, or a declined prompt, is a
  warning on the Downloads row ("Couldn't run oalinst.exe: …"), never a failed install.
  A warning from the steps (a declined prompt, a failed or timed-out step, no compat
  tool, a cancel) offers Retry, which runs only the missing steps: a small "Setup"
  Downloads row with no download, no login and no nile call, reading "Running the
  installer silently · N%" and then "Setup steps done" (or "Setup steps didn't all run ·
  <warnings>", with Retry again). It is refused while the game runs or has another job.
  Epic uses the same job for its prerequisite, and GOG for its setup script and redistributables. A cancel stops the steps and leaves the game
  installed. A
  "Sign in again" that is cancelled leaves the store expired (nile's old login is deleted
  before a new sign-in). On a Linux system older than glibc 2.35, nile can't run, and
  Settings and the store panel say "nile can't run on this computer: it needs a newer
  Linux system (glibc 2.35 or later)." Amazon games are never backed up to a drive.
- **Steam** (Phase 4, 2026-11-14). Steam has no tool and no login window: it connects in
  Settings › Stores › Steam with a SteamID64 (picked from the accounts signed in to this
  computer's Steam, by persona name only, or typed) and the user's own Web API key.
  Connect needs a key Steam answers for. The key lives only in the OS keyring; Settings
  shows a saved key as dots only, never any part of it, and the key field is empty after
  a Save. Test asks Steam once (`GetOwnedGames`) and says so: "The key works · N games
  owned", a rejected key ("Steam rejected the key…", with no status code), private game
  details (Available stays empty, installed games still show), a SteamID64 of the wrong
  form (refused before any request), or no network. Remove key keeps Steam connected
  with its installed games only; Disconnect deletes the key, the SteamID64 and GRID's
  Steam rows, never a game. The client line reads "Steam client found · <path>", or for
  a Flatpak or Snap Steam its package ("Flatpak (com.valvesoftware.Steam)"); with no
  client, Steam's panels say to install Steam. Steam installs, updates, verifies,
  uninstalls and runs its own games: GRID opens `steam://` links and Steam's own window
  does the rest. While Steam installs, the Downloads row reads "Installing in Steam N% ·
  Managed by Steam" with Cancel disabled (it takes no download slot and is never retried)
  and ends "Installed by Steam" or Cancelled. Games installed in Steam outside GRID
  appear by themselves within the 15 s background scan, and Details says "Installed in
  Steam, outside GRID". Only games and demos show: Proton, the Steam runtimes,
  redistributables and owned software are filtered out everywhere. An Available Steam
  game shows no size, and no Steam game shows Genres. Play starts the game through Steam
  and GRID watches the session (Steam's running app, else the game's process); it ends
  when Steam reports the game closed, or silently when Steam never starts it within
  3 minutes. GRID can't stop a Steam game ("Running in Steam"). Steam games have no Game
  Settings, no store cloud saves (Steam Cloud is Steam's), no updates in Library ›
  Updates, and are never backed up to a drive.
- **Play on installed cards.** Every "<game> installed" card for a RomM install has Play,
  which launches the game the normal way (TV mode opens Playing). A card that sums several
  installs has none. Store install cards already have Play.

# Cloud Save Synchronization
The application now includes cloud save and state synchronization for emulator-based games using the RomM saves/states API.

Store games (see "Game stores") never use RomM cloud saves: no manual action, no restore
before launch and no upload on exit.

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
Every finished game session of 30 seconds or more is sent to RomM as a play session (start, end, duration). It is always on; there is no setting. Sessions without a server ROM id, store games (see "Game stores") and Emulators-platform entries are not sent. While the server is unreachable, sessions wait in a local queue and go out after the next successful connect or restore. RomM's "last played" for the game follows the session end; the app never changes the game's play status on the server.

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