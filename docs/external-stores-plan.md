# External game stores — plan

Status: Phase 0 done 2026-11-13 (shared model on main; design approved: `docs/design/system/components/GameStores` and `TVStores`). Phase 1 (GOG with offline installers) built 2026-11-13 (`docs/stores-phase1-plan.md`). Phase 2 (Epic Games Store through legendary) built 2026-11-13 (`docs/stores-phase2-plan.md`). Phase 3 (Amazon Games through nile) built 2026-11-14 (`docs/stores-phase3-plan.md`). Phase 4 (Steam) built 2026-11-14 (`docs/stores-phase4-plan.md`). All phases are built; the real-account login tests are pending with the user (checklist below, "Waiting for the user's real-account test").

## Waiting for the user's real-account test

Everything here was built and tested against mocks and fakes only (no real account, key
or Steam client). The user's final test, store by store:

**GOG (Phase 1)**
- [ ] Sign in through GRID's login window with a real GOG account: the real page renders
  in the incognito WebKitGTK window, 2-step and captcha work, and the redirect to
  `on_login_success` is caught in Rust (the success page never loads).
- [ ] TV mode: the login page over TV mode, typing with the on-screen keyboard
  (`store_login_type`), Tab/Enter/Backspace.
- [ ] gogdl's real `auth`, a real install, update and repair, and a real `save-sync` both
  ways; Disconnect removes `<data_dir>/stores/gogdl/` and the login webview's data.

**Epic Games (Phase 2)**
- [ ] Spike 3: the Tauri login window with Epic's real sign-in page (the finish URL and
  any other localhost navigation refused; pop-ups opening in the same window; the
  authorization code reaching legendary).
- [ ] legendary's real login, owned list, install, launch (exchange code, offline retry)
  and `sync-saves`.

**Amazon Games (Phase 3)**
- [ ] Spike 5: WebKitGTK's user agent on Amazon's page, captcha and 2-step in the
  ephemeral context, the finish hop reaching `on_navigation`, and the pop-ups.
- [ ] `register`'s real lines: success, and the refusal body on stdout.
- [ ] `library sync` succeeding, and the expired-refresh line (`Failed to refresh the
  token <Response [4xx]>`).
- [ ] The real `library.json` art fields (the portrait cover question).
- [ ] Real `install`/`update`/`verify` progress, the `installed.json` version format and
  real `list-updates --json` output.
- [ ] The SDK download (the FUEL DLLs), and `auth --logout` deregistering.

**Steam (Phase 4)**
- [ ] Spike 5, the real client: `steam://install`, `uninstall`, `validate` and `-silent
  steam://rungameid/<id>` with Steam running, not running and signed out, on native and
  Flatpak Steam (and Snap if available); Steam's dialogs appearing over TV mode.
- [ ] `RunningAppID` and `apps/<id>/Running` in `registry.vdf` while a native and a
  Proton game run (the `/proc` fallback covers them until then), and the same values in
  the Windows registry (`SteamPath`, `SteamExe`, `RunningAppID`) on a Windows host.
- [ ] The manifest's flags and bytes during a real install, a pause and an update (the
  install watch's progress and its Cancelled rule), and `LastPlayed` in real manifests.
- [ ] The user's key: Test works, the private toggle gives `{"response": {}}`, a revoked
  key's status (the words carry no code either way), the real `GetOwnedGames` shape with
  `rtime_last_played`, and `GetPlayerSummaries` for a manual SteamID64.

## User decisions (2026-11-13)

- Stores: GOG, Epic, Amazon, Steam, plus installing GOG games from offline installers.
- Tooling: GRID uses Heroic's CLI tools as they are: legendary (Epic), heroic-gogdl (GOG)
  and nile (Amazon). The tools keep their own token files in GRID's data folder. This is
  a narrow, documented exception to the keyring-only rule. GRID's own secrets (the Steam
  Web API key) stay in the OS keyring. Tool output is never logged or forwarded.
- Steam needs the user's own Web API key and SteamID64. The owned library comes from
  `GetOwnedGames` (the profile's game details must be public). Install, uninstall and
  launch go through the Steam client with `steam://` links.
- Placement: each connected store gets a group in the Library rail, like Backup drives.
  The main title is the store's name, with two sub-labels: **Installed** and
  **Available**. The same applies in desktop and TV mode.
- Order: GOG with offline installers first, then Epic, Amazon and Steam.
- Store cloud saves (`gogdl save-sync`, `legendary sync-saves`) are in scope in each
  store's phase. RomM cloud saves stay off for store games.
- TV mode login: the store's login page opens over TV mode, with the on-screen keyboard.
- Steam games installed outside GRID appear automatically. Only games appear; Proton
  and the Steam runtimes are filtered out.

## User decisions (2026-11-13, later)

- **GOG installers on the RomM server.** The user zips GOG offline installers
  (`setup_*.exe` plus its `.bin` files, or a Linux `.sh`) and uploads them to RomM
  themselves. GRID does not upload them.
- **What GRID does with them.** After download, GRID checks the archive:
  - If it holds an installer, GRID runs the offline-installer flow.
  - If it holds loose game files, GRID installs them as it does today.
- **The installed game stays a RomM game.** It keeps every RomM feature: RomM cloud saves,
  play activity, update checks and drive backups. The installer is only how the files get
  onto disk.
- **Drive backups of GOG games not from RomM.** This covers games installed from a local
  installer file or through gogdl. A pack holds:
  - the installer files, when GRID has them; restoring runs the installer again
  - otherwise, the installed copy and its saves, like a copy of an installed RomM game

  This replaces the earlier "Store games are not backed up" rule, for GOG only. GRID's
  backup feature is its own and needs nothing from gogdl.
- **TV mode runs Windows installers silently only.** GRID never opens the installer's
  setup window over TV mode. If the silent install fails, GRID keeps the installer and
  the error says to finish it in desktop mode. Desktop mode stays silent-first, with the
  installer's own setup window as the fallback.

## Design

### Identity

- `GameKey` is `Rom(i64)` or `Store { source, store_id }`, where `source` is one of
  `gog`, `epic`, `amazon` or `steam`. RomM paths keep `rom_id` unchanged.
- An offline GOG install is `source = gog` with `origin: offline` in its store
  metadata. A GOG product has one identity, whether it was installed online or offline.
- Registry v11 adds three columns:
  - `source` (default `romm`)
  - `store_id`
  - `store_meta`: JSON, never a token
- v11 also adds a partial unique index on `(source, store_id)`.
- Store rows reuse the native launch columns.

### Feature matrix for store rows

| Feature | Store rows |
|---|---|
| Play activity | Off |
| RomM cloud saves | Off |
| Store cloud saves | On, for GOG and Epic |
| Drive backups | Off ("Store games are not backed up") |
| Relink and server update checks | Off |
| Each store's own update check | On |
| Last played, rail, sort, search | On |
| Details Overview, Game Settings, uninstall | On |

### Tools

- legendary, gogdl and nile are GRID-managed downloads, installed like umu-launcher.
  The version tag and the SHA-256 for each OS and architecture are pinned in code.
- A tool is installed the first time its store is connected. The tool's license and a
  source link are shown in Settings.
- Each tool's files live under `<data_dir>/stores/<tool>/`: the folder is 0700, the files
  are 0600, and Disconnect deletes them.

### Launch and sessions

- `LaunchService::launch_key(&GameKey)`.
- `GameSession.key`; `rom_id` becomes optional.
- A Watched session kind for Steam polls whether the game is still running instead of
  holding a child process.
- Epic launch arguments carry an exchange code. Those argv positions are redacted
  everywhere they could be printed.

### Logins

- A Tauri login window catches the store's redirect in Rust.
- The code and the tokens never cross IPC.

### Offline installers

- A Linux `.sh` is read as a zip; GRID takes `data/noarch/` and never runs the script.
- A Windows `setup_*.exe` runs silently under Wine/Proton, falling back to the wizard. On a
  Windows host it runs natively.
- The game is identified from `goggame-*.info`. A DLC installer attaches to its base game
  by `rootGameId`.
- macOS `.pkg` installers are refused with a reason.

### Module layout

- grid-core: `crates/grid-core/src/stores/`
  - `mod`, `tools`, `progress`, `paths`, `art`
  - one folder per source: `gog`, `offline`, `epic`, `amazon`, `steam`
- App: `stores_service.rs`, `store_auth.rs`, `commands/stores.rs`.

## Phases

0. Shared model:
   - registry v11
   - `GameKey` in launch, sessions and the queue
   - feature exclusions
   - frontend types
   - AGENTS, ARCHITECTURE and SPEC updates, and the secret-hygiene script
   - the design prototype: Library store groups, Settings › Stores, login, Details for a
     store game, the offline-installer flow, and the TV equivalents
1. GOG online, offline installers and GOG cloud saves.
2. Epic, with legendary cloud saves.
3. Amazon.
4. Steam.

Each phase ends when these pass:
- unit tests on parsers and command builders (fixtures use fake tokens only)
- e2e with mock store HTTP and a fake tool binary
- an e2e check that the log never holds the fake token
- the gate

## User decisions (2026-11-14, after the first real-account test)

- **Redistributables and setup steps run silently, right after install**, for every
  store that has them (Hotline Miami on Amazon failed without the OpenAL step it lists):
  - Amazon: fuel.json `PostInstall` steps.
  - GOG (gogdl installs): the game's setup script (GOG's scriptinterpreter or its
    `temp_executable`) for the game and each installed DLC, then the manifest's `__redist`
    dependencies fetched with `gogdl redist`. After an update or move only the script runs
    again. Offline installers already run their own.
  - Epic: legendary's `prereq_info`.
  - Steam: nothing; the Steam client runs them itself.
- They run in the game's own prefix under umu/Proton. On a Windows host each one runs
  elevated with one UAC prompt per item; in TV mode on Windows they are skipped and the
  Downloads row says to finish them in desktop mode.
- A failed or timed-out step is a warning on the Downloads row, never a failed install.
  Research notes: `docs/research/store-redistributables.md`.
