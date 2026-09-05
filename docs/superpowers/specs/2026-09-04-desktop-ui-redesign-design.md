# Desktop UI redesign — design

Date: 2026-09-04. Scope: the desktop frontend of the Rust/Tauri rewrite (`rewrite/app/src`).
Backend behaviour, IPC commands, and the E2E harness stay as they are unless a section
says otherwise. TV mode (doc 09) is out of scope and remains a later, separate redesign.

Decisions below were made with the user over mockups (`.superpowers/brainstorm/`, not
committed). Research notes on RomM v2 and on Steam / GOG Galaxy / Epic conventions informed
the options.

## 1. Goals

- Replace the modal-heavy shell with five first-class views: Library, Server, Downloads,
  Emulators, Settings. No view scrolls inside another scroll container.
- Match the polish of Steam / GOG / Epic while looking like a companion to the RomM v2
  web UI: its top pill navigation, dark purple token set, card treatment, and cover-left
  game page.
- Keep every existing behaviour (install, launch, cloud saves, firmware, updates, compat
  tools, RetroAchievements) reachable with no more clicks than today.

## 2. Decisions

| ID | Decision |
|---|---|
| D-UI-1 | Shell = fixed top bar with centred pill tabs (RomM v2 `AppNav`), logo left, server/user menu right, a download footer strip, and blurred background art behind content. |
| D-UI-2 | Library = left rail (All games, Recent, Updates, then installed platforms with counts) + one grid with a toolbar (search, sort, card size). |
| D-UI-3 | Server mirrors Library: rail of server platforms with counts, platform header (name, counts, firmware status), grid with Installed / Update badges and dimmed not-installed cards. |
| D-UI-4 | Game details stays a popup: fixed cover and action column left, header and four tabs right: Overview, Media, Saves, Files. Media opens fullscreen viewers for screenshots and video. |
| D-UI-5 | Emulators and Settings use a left category rail with one scrolling pane per category. Emulators: Installed, Add from catalog, Platform defaults, Compat tools (Linux only). Settings: Connection, Cloud saves, RetroAchievements, Updates, Appearance. |
| D-UI-6 | Downloads is a full view: Active / Queued / Completed segments with a legend; one row per transfer with a kind badge, progress, speed, ETA, a mini sparkline panel (network purple, disk teal) beside the buttons. The footer strip shows the current transfer with a tiny sparkline and opens the view. |
| D-UI-7 | Content columns for lists (Downloads, Emulators, Settings, rail panes) cap at 1100px and centre. Grids may use the full width up to 1920px. |
| D-UI-8 | Theme = RomM v2 purple tokens (§4), dark first, with a light variant. Follows the OS setting; override in Settings › Appearance. |
| D-UI-9 | Cards: hover scales 1.05 with a gradient overlay, a centred Play or Install, and a bottom action row (Details, Cloud sync, Favourite, More). Badges: installed dot top-right, UPDATE tag top-left, cloud icon bottom-right, platform chip bottom-left. Size control Small / Medium / Large per view, remembered. |
| D-UI-10 | The Files tab shows a version for PC games: the parsed version tag when the file name carries one, else the file's `last_modified` date. |

## 3. Shell

- Top bar, 58px: logo (icon + "GRID" wordmark) left; centred pill group with the five
  views; right cluster = connection status dot + server name menu (Reconnect, Disconnect,
  Open RomM in browser) and the app-update badge when a notice is stored.
- Keyboard: `Ctrl+1..5` switch views; `Ctrl+F` focuses the current view's search.
- Background art: the large cover of the last game the user viewed (opened in the
  details popup or hovered for more than 500ms), falling back to the most recently played
  installed game on startup; blurred 40px; opacity from the Settings › Appearance fade
  slider (0–60%, default 25%, stored as `ui.background_fade`); cross-fades over 360ms.
- Download footer strip, 28px, always mounted: hidden when nothing is live; otherwise
  "⬇ <title> · <percent> · <speed>" with a 60-sample sparkline and an "Open Downloads"
  link. Clicking anywhere on it opens the Downloads view.
- The current app-update banner becomes a badge on the user menu plus an entry under
  Settings › Updates; the banner strip is removed.
- Views stay mounted and switch with `hidden`, as Library/Server do today, so scroll
  positions and selections survive switching.

## 4. Theme tokens

Adopted from RomM v2 `tokens/index.ts`, defined once in `app.css` as CSS variables:

- Dark: `--bg #07070f`, `--surface rgba(255,255,255,.07)`, `--surface-2 #14141f`,
  `--border #22223a`, `--text #ffffff`, `--text-muted #9a9ab0`.
- Light: `--bg #f5f5fa`, `--surface rgba(0,0,0,.035)`, `--text #111117`; primary darkens
  to `#553E98`.
- Primary `#8B74E8` (hover `#A18FFF`, pressed `#6043C8`), secondary `#9E8CD6`, accent
  `#E1A38D`, favourite `#FF4F6B`, success `#4ADE80`, warning `#FBBF24`, danger `#FF5050`,
  info `#93C5FD`, disk-graph teal `#2dd4bf`.
- Type: Segoe UI / system-ui / Inter; base 13px; titles 15–20px semibold.
- Spacing on a 4px scale; radii 4 (controls) / 6 (chips) / 8 (rows) / 14 (cards) /
  100 (pills). Motion 150 / 220 / 360ms.
- Theme resolution: `prefers-color-scheme` unless Settings › Appearance overrides (stored
  in config as `ui.theme = "system" | "dark" | "light"`).

## 5. Library view

- Rail (220px): All games (count), Recent (played in the last 30 days), Updates (count of
  rows with a newer server version), then "PLATFORMS" with each installed platform and
  its count. Selection persists per session.
- Toolbar: search (title contains), sort (Recently played, Recently installed, Title,
  Platform), card size. Empty state per rail item.
- Grid: `repeat(auto-fill, minmax(<size>, 1fr))` with sizes 120 / 160 / 200px; cover
  fixed 3:4 frame; the cover is fitted inside it over a blurred, dimmed copy of itself
  (user decision 2026-09-05, replaces the image-ratio rule); title under the card, one
  line, ellipsis.
- Card click opens the details popup; the hover Play launches directly.

## 6. Server view

- Rail: server platforms with ROM counts (RomM `/api/platforms`).
- Platform header: display name, ROM count, installed count, firmware status chip with
  an Install action when the server offers firmware, and the platform's default emulator
  chip linking to Emulators › Platform defaults.
- Grid: the platform's ROMs; installed cards carry the installed dot, updatable cards the
  UPDATE tag, not-installed cards render at 60% until hover. Hover primary action is
  Install for not-installed, Play for installed.
- Search searches the platform's list client-side (RomM search endpoint is a later
  addition).

## 7. Game details popup

Dialog 1040×680 max, centred over a dimmed, blurred shell; Esc and ✕ close.

- Left column (240px): cover (`path_cover_large`), then Play / Install (primary), Update
  when available, cloud status button (Synced 2h ago / Sync now / Not configured),
  gear menu (native game settings, emulator override, remove), play time and the
  emulator + core that will launch.
- Right header: title, platform, first release date, developer, genres, rating
  (`igdb_metadata.total_rating`), region / language flags, verification state.
- Tabs (URL-less; last tab remembered per session):
  1. **Overview**: `summary`, metadata grid (developer, publisher, release, genres,
     game modes, player count, franchises), screenshot strip (first six of
     `merged_screenshots`), Related row (`igdb_metadata.similar_games`, remakes,
     remasters, dlcs, expansions) filtered to titles present on the server.
  2. **Media**: screenshot gallery (`merged_screenshots`, user screenshots), video
     (`youtube_video_id` embedded, `path_video` when the server hosts one). Click opens
     a fullscreen viewer with arrows, Esc, and a caption.
  3. **Saves**: user saves and states (`user_saves`, `user_states`) with timestamps and
     sizes, last cloud sync, Upload / Download / Sync now, and the cloud scope warning
     the current details view shows.
  4. **Files**: `files[]` with name, size, `last_modified`; installed version vs server
     version per D-UI-10 with the Update button; PS4 / Xbox 360 content rows; firmware
     row for the platform. Native rows keep the merge-path confirm text.
- All strings that E2E asserts today (update toast, confirm text, launch errors) stay
  verbatim.

## 8. Downloads view

- Segments Active (live), Queued, Completed (terminal, dismissable), each with a count;
  a legend line beside them: "Active: downloading or installing · Queued: waiting for a
  slot · Completed: finished, failed, or cancelled".
- Row: title + kind badge (base none / Update / Content / Emulator / Compat tool /
  Firmware), detail line (existing `entryDetail` text), progress bar, then the sparkline
  panel (120×38: network in primary, disk in teal, 60 one-second samples), then the
  action buttons from `actionFor`.
- Sampling: the downloads store keeps a ring buffer per entry fed from `downloaded_bytes`
  and `install_processed_bytes` deltas on each progress event, sampled once per second.
  No new IPC.
- Completed keeps the last 50 entries.

## 9. Emulators view

Rail: Installed, Add from catalog, Platform defaults, Compat tools (hidden on Windows).

- Installed: rows with name, path, source, Edit / Remove, the RPCS3 firmware note and
  button. Edit opens the manual form inline as a sheet on the right of the pane.
- Add from catalog: search box, catalog rows with provider and Install / Installed; a
  "Manual" button opens the manual form.
- Platform defaults: the card list shipped on 2026-09-04 (emulator select, core select,
  remembered "(none)").
- Compat tools: the current `CompatTools` content.

## 10. Settings view

Rail: Connection (server URL, token status, reconnect, disconnect), Cloud saves (current
cloud settings form), RetroAchievements (current form), Updates (app version, last check,
release link, "check-only" note), Appearance (theme, card size defaults, background art
on/off, background fade slider 0–60% with a live preview behind the settings pane).

## 11. Test ids and E2E

- Test ids that survive unchanged: `nav-library`, `nav-server`, `nav-downloads`,
  `platform-btn-<id>`, `game-card-<id>`, `library-card-<id>`, `library-update-badge-<id>`,
  `details-*`, `download-row-<id>`, `download-detail-<id>`, `download-kind-<id>`,
  `downloads-footer`, `emulator-*`, `emu-*`, `default-select-<id>`, `default-core-<id>`,
  `compat-*`, `ra-*`, `cloud-settings-*`, `app-update-*`.
- Renamed: `emulators-open` → `nav-emulators` (the pill tab); `emulators-panel` → the
  Emulators view root; `emulators-close` is removed (navigation replaces it);
  `downloads-drawer` → the Downloads view root. Specs are updated in the same task that
  renames an id; every E2E group must pass at the end of each plan.
- New ids: `nav-settings`, `emu-nav-<page>`, `settings-nav-<page>`, `library-rail-<key>`,
  `server-rail-<id>`, `details-tab-<name>`, `media-viewer`, `downloads-seg-<name>`,
  `download-graph-<id>`, `theme-select`.

## 12. Delivery

Five plans, in order, each ending with the full gate and a merge:

1. **Shell and theme**: tokens, top bar, pill navigation, view scaffolding (all five
   views mounted), footer strip, background art, Settings › Appearance with theme
   override. Existing Library/Server/Emulators content moves into the new frames
   unchanged.
2. **Library and Server views**: rails, toolbars, grids, card treatment and badges,
   platform header.
3. **Details popup**: cover-left layout, four tabs, media viewers, version display rule.
4. **Downloads view**: segments, rows with sparklines, footer sparkline, completed
   history.
5. **Emulators and Settings views**: category rails, inline edit sheet, remaining
   settings pages; removal of the old modal and its ids.

SPEC.md (desktop UI sections) and `rewrite/README.md` (manual checklist) are updated in each plan for the sections it changes.

## 13. Out of scope

TV mode; RomM collections and search endpoints; achievements tab; notes; controller
navigation in the desktop shell; core downloads.
