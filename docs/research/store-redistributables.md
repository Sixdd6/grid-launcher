# Store redistributables: GOG (gogdl), Epic (legendary), Steam — research for GRID

Date: 2026-11-14. Research only; no repository file changed.

## Sources (exact revisions read)

- Heroic Games Launcher, `main` at `89e06ad7b89cb7514ea3ce089ff1e2ca5e49dafa` (2026-11-13)
  - `src/backend/storeManagers/gog/redist.ts` — download list + `gogdl redist` call
  - `src/backend/storeManagers/gog/setup.ts` — runs ISI / temp_executable / support_commands, then redists
  - `src/backend/storeManagers/gog/constants.ts` — `gogRedistPath = <toolsPath>/redist/gog`, `gogSupportPath = <gogdlConfig>/gog-support`
  - `src/backend/storeManagers/gog/games.ts` (install L440-480, move L795-815, update L1008-1030, L1210-1235)
  - `src/backend/downloadmanager/downloadqueue.ts` L150-180 — queues the `gog-redist` pseudo-game with the game's `dependencies`
  - `src/backend/storeManagers/legendary/setup.ts` — Epic prereq run
  - `src/backend/launcher.ts` L840-890 (`prepareWineLaunch`: `<prefix>/installed_games` tracking), L1452+ (`runWineCommand`)
  - `src/common/types/gog.ts` L494 (`GOGRedistManifest`), `src/common/types/legendary.ts` L181 (`Prerequisite`)
- heroic-gogdl `v1.4.0` (`6c6ef53f`): `gogdl/args.py` L29-43, `gogdl/cli.py` L48-51,
  `gogdl/dl/managers/dependencies.py`, `gogdl/dl/managers/v2.py` L55-100 / L140-195,
  `gogdl/dl/objects/v2.py` L13-20 / L60-75, `gogdl/constants.py` L10-11, `gogdl/dl/progressbar.py`.
- legendary `0.21.1` (`42f6bdea`): `legendary/core.py` L1660-1700, L1758-1762, L1830-1840, L1885-1888;
  `legendary/cli.py` L1100-1104, L1157-1185, L1852-1865; `legendary/models/manifest.py` L113-117.
- Live public GOG endpoints (anonymous GETs, 2026-11-14): `content-system.gog.com/dependencies/repository?generation=2`,
  its `repository_manifest`, per-depot meta, and `products/<id>/os/windows/builds?generation=2` build metas
  for 1207658924, 1495134320, 1207664643, 1207658691.
- GRID: `crates/grid-core/src/stores/gog/mod.rs` L397-430 (`manifests_dir`, `support_dir`, `tool_env`),
  `stores/offline/exec.rs` L28-36 (`SILENT_FLAGS`), `stores/amazon/install.rs` L15, `stores/steam/client.rs`.

---

## 1. GOG (gogdl)

### 1.1 Where the dependency list comes from

Three equivalent places (all are the same build-manifest field):

1. **Build manifest v2 `dependencies`** (top level of the build meta). Example, live, The Witcher 3 GOTY
   (1495134320):
   ```json
   {
     "version": 2, "baseProductId": "1495134320", "buildId": "...", "installDirectory": "...",
     "dependencies": ["DirectX","MSVC2012","MSVC2012_x64","MSVC2013","MSVC2013_x64","MSVC2019","MSVC2019_x64"],
     "scriptInterpreter": true,
     "products": [{"productId": "1495134320", "name": "...", "temp_executable": "", "temp_arguments": ""}],
     "depots": [...], "offlineDepot": {...}, "platform": "windows", "tags": [...], "clientId": "...", "clientSecret": "..."
   }
   ```
   The Witcher EE (1207658924): `"dependencies": ["MSVC2005","MSVC2005_x64","DirectX"]`, no
   `scriptInterpreter`, `products[0].temp_executable = "galaxy_the_witcher_enhanced_edition_2.1.0.15.exe"`.
   UT2004 (1207658691): no `dependencies` key at all, `scriptInterpreter: true`.
   - The field can be **absent** → treat as `[]` (gogdl does: `meta.get("dependencies") or []`).
2. **gogdl's saved copy**: after `download`, gogdl writes the raw meta (plus `HGLInstallLanguage`,
   `HGLdlcs`) to `$GOGDL_CONFIG_PATH/heroic_gogdl/manifests/<id>` (GRID: `svc.manifests_dir()/<id>`). Heroic
   `setup.ts` reads `dependencies` from exactly this file. v1 (legacy) manifests: the list is
   `product.depots[].redist` for depots that carry a `redist` key.
3. **`gogdl info` JSON** (`v2.py get_download_size`): `"dependencies": self.manifest.dependencies_ids` —
   same list, available before install (Heroic's download queue uses this to queue the redist download).

`goggame-<id>.info` is NOT used for redists by Heroic or gogdl.

### 1.2 The dependency repository (redist manifest)

`https://content-system.gog.com/dependencies/repository?generation=2` →
`{"repository_manifest": "<zlib JSON url>", "build_id": "60163282911088704", "generation": 2}`.
The zlib JSON is `{"depots": [...]}`; each depot:
```json
{
  "dependencyId": "openAL",
  "readableName": "Open AL (2.0.7.0)",
  "executable": {"path": "__redist/openAL/oalinst.exe", "arguments": "/S"},
  "internal": false,
  "languages": ["*"],
  "manifest": "e0b13c7ba5ef3712faf6dd2855857e45",
  "compressedSize": 0, "size": 809496, "signature": "..."
}
```
After a `gogdl redist` run, gogdl saves this repository plus `"build_id"` and `"HGLInstalled": [ids]` as
`<path>/.gogdl-redist-manifest` (Heroic type `GOGRedistManifest`). `gogdl redist --print-manifest` prints
the repository JSON to stdout and exits.

Live list (2026-11-14), the relevant rows (path → args):

| id | path | arguments | internal |
|---|---|---|---|
| ISI | `__redist/ISI/scriptinterpreter.exe` | (none) | true |
| DirectX | `__redist/DirectX/DXSETUP.exe` | `/silent` | false |
| MSVC2005 / _x64 | `__redist/MSVC2005/vcredist_x86.exe` | `/q:a /c:"VCREDI~3.EXE /q:a /c:""msiexec /i vcredist.msi /qb"" "` | false |
| MSVC2008 / _x64 | `.../vcredist_x86.exe` | `/q` | false |
| MSVC2010, 2012 | `.../vcredist_x86.exe` | `/q /norestart` | false |
| MSVC2013..2026 (+_x64) | `.../VC_redist.x86.exe` | `/install /quiet /norestart` | false |
| dotNet35, 35C, 4, 4C, 45, 46, 47, 472, dotNetCore318(_x64) | `.../*.exe` | `/q /norestart` | false |
| dotNetDesktopRuntime60/70/90(_x64) | `.../windowsdesktop-runtime-*.exe` | `/quiet /norestart` | false |
| openAL | `__redist/openAL/oalinst.exe` | `/S` | false |
| PHYSX, PHYSX_9190218 | `.../PhysX-*-SystemSoftware.exe` | `-s` | false |
| PHYSX_9100222 | `.../PhysX-9.10.0222-SystemSoftware.exe` | `/quiet` | false |
| PHYSXLEGACY | `.../PhysX-9.13.0604-SystemSoftware-Legacy.msi` | `/passive /norestart` | false |
| XNA / XNA_40 | `__redist/XNA/xnafx31_redist.msi` | `/qn` | false |
| UE4REDIST | `__redist/UE4REDIST/UE4PrereqSetup_x64.exe` | `/passive /norestart` | false |
| AdobeAir | `.../AIRRuntimeInstaller.exe` | `-silent` | true |
| Foxit | `__redist/Foxit/FoxitReader.exe` | `/VERYSILENT` | true |
| QuickTime | `__redist/QuickTime/QuickTimeInstaller.exe` | **(none — not silent)** | true |
| DOSBox*, DOSbox_S083, ScummVM, nGlide*, SuspendLauncher, DOSBOXConfigurator, language_setup | **`""` (empty)** | | true |

Each `__redist` depot contains one file at exactly `executable.path` (checked: openAL, ISI, MSVC2019_x64).
Empty-path depots (e.g. DOSBox074) contain files like `DOSBOX/DOSBox.exe` that belong in the **game folder**.

### 1.3 The two classes of dependency — and what gogdl already does

`dependencies.py` splits on `executable.path.startswith("__redist")`:
- **Game-folder deps** (path does NOT start with `__redist`, i.e. the empty-path DOSBox/ScummVM/nGlide/…):
  `gogdl download/update/repair` already fetches these **into the game folder** as part of the game
  (`DependenciesManager(..., download_game_deps_only=True)` in `v2.py` L150 / `v1.py` L201; their size is
  added to `info`'s size). GRID gets these today with no extra work. Never "run" them.
- **`__redist` deps** (the installers): `gogdl download` does NOT fetch them. They come only from a
  separate `gogdl redist` call.

### 1.4 `gogdl redist` command

```
gogdl [--auth-config-path <file>] redist --ids <ID1,ID2,...> --path <dir> [--max-workers N] [--print-manifest]
```
(alias `dependencies`). `--ids` is comma-separated. Heroic always passes `--auth-config-path` (all gogdl
calls do), but the dependency endpoints answer anonymously (`open_link?...path=/dependencies/store/` returned
200 with no token), so auth is not required; passing GRID's usual auth path is harmless.

Heroic call (`redist.ts updateRedist`):
```
gogdl --auth-config-path <...> redist --ids ISI,DirectX,MSVC2019,... --path <toolsPath>/redist/gog [--max-workers N]
```
- Downloads only `__redist`-path depots for the given ids into `<path>/__redist/<ID>/<file>`
  (one shared folder for all games — not into any prefix).
- Writes `<path>/.gogdl-redist-manifest` (repository + `build_id` + `HGLInstalled`) — even on "Nothing to do".
- Also passes `<path>/gog-support` as the support dir (unused for redists).
- Progress: the same `[PROGRESS] INFO: = Progress: <pct> <written>/<total>, Running for: hh:mm:ss, ETA: hh:mm:ss`
  / `= Downloaded: ... MiB, Written: ... MiB` / ` + Download\t- ...` / ` + Disk\t- ...` stderr lines as
  `download` — GRID's existing gogdl progress parser applies. Exit 2 + `Unable to proceed, Not enough disk space`.
- **Caveat (important):** gogdl diffs the requested ids against the previous `HGLInstalled` in that
  folder and **deletes** files of previously installed ids that are not in the new `--ids`
  (`DependenciesDiff.compare` → `deleted`). So with a shared folder, every call must pass the **union** of
  all ids still needed. Heroic does exactly that: `getRequiredRedistList()` = `['ISI']` + every
  `dependencies` of every manifest in `heroic_gogdl/manifests/`, and re-syncs when the union or the
  repository `build_id` changes (`checkForRedistUpdates`, run once when online).

### 1.5 How Heroic runs them (`gog/setup.ts`)

When: `setup(appName)` runs
- **Linux/macOS (Wine/Proton):** from `prepareWineLaunch` on **launch**, the first time the game's
  `app_name` is added to `<winePrefix>/installed_games` (a JSON array file in the prefix). That file is
  Heroic's only "already done" marker: **once per (prefix, game)**, not once per prefix for all games.
  Not at install time. After **update** and **move** it re-runs with `installRedist=false` (ISI/temp exe
  only, no redists), and only if the prefix exists.
- **Windows:** directly after **install** (`games.ts` L462-468), with redists. Each command goes through
  `powershell Start-Process -FilePath <exe> -Verb RunAs -Wait -ArgumentList ...` → **one UAC prompt per
  command**.

Order inside `setup()` (Windows-platform builds only; skip for Linux-native GOG builds):
1. Game-script step, for the base product and each installed DLC (`products[]` filtered):
   - v2 + `scriptInterpreter: true`: `<redistPath>/__redist/ISI/scriptinterpreter.exe` with
     `/VERYSILENT /DIR=<game dir, Windows path> /Language=<English name> /LANG=<same> /ProductId=<productId>
     /galaxyclient /buildId=<buildId> /versionName=<version> /lang-code=<en-US> /supportDir=<support dir,
     Windows path> /nodesktopshorctut /nodesktopshortcut` (the typo is intentional, both are passed).
   - v2 without ISI: if `products[].temp_executable` is non-empty, run
     `<supportPath>/<appName>/<productId>/<temp_executable>` with the same args minus `/supportDir`.
   - v1: each `product.support_commands[]` whose `languages` include the install language or `Neutral`:
     `<support>/<appName>/<gameID>/<executable>` with similar args.
   - cwd = game folder; `protonVerb: 'run'`.
2. Redists: for each id in the game's `dependencies` (manifest order), look it up in
   `.gogdl-redist-manifest`; skip if not found or `executable.path` is empty; run
   `<redistPath>/<executable.path> + shlex.split(executable.arguments)`, cwd = redist folder,
   `protonVerb: 'run'`, sequential (each awaited; `runWineCommand` waits for exit).
   Hack: `PHYSXLEGACY` → `msiexec /i <msi> /passive /norestart /qb`. (Heroic does **not** wrap the XNA
   `.msi` the same way — a Heroic bug; GRID should run any `.msi` via `msiexec /i`.)
3. No exit-code checking, no per-redist "installed" record; failures are only logged.

ScummVM special case: if deps include `ScummVM`, Heroic passes Unix paths instead of Wine paths for `/DIR`
and `/supportDir`.

### 1.6 What must not be run

- Empty-path deps (DOSBox*, ScummVM, nGlide*, SuspendLauncher, DOSBOXConfigurator, language_setup): files in
  the game folder, already handled by `gogdl download`.
- `ISI` must not be run as a bare redist (no args); only via the scriptInterpreter step in 1.5 when
  the manifest says `scriptInterpreter: true`. Game `dependencies` lists do not normally contain `ISI`;
  Heroic only adds it to the download union.
- `QuickTime` (no silent args — shows UI), `Foxit` (PDF reader for manuals), `AdobeAir` are `internal: true`
  extras; recommend skipping `internal: true` ids other than ISI (Heroic runs them if listed).
- Nothing in the list is "GOG Galaxy client" itself. `GalaxyCommunication.exe` (comet) is a separate Heroic
  feature, not a redist.

### 1.7 ISI / temp_executable — a gap in GRID beyond redists

GRID already passes `--support <tool>/support/<id>` so the support files (the `goggame-<id>.script`,
temp executables) land on disk, but GRID runs neither `scriptinterpreter.exe` nor `temp_executable`.
These scripts write registry keys, CD keys (UT2004 needs its key in the registry), language settings and
sometimes run their own redist steps. They are the GOG equivalent of Amazon's PostInstall. ISI is itself
a `__redist` download (`ISI` id), so it needs the same `gogdl redist` step.

### 1.8 Offline installers (Inno `setup_*.exe`)

GOG offline installers carry their own `__redist` folder and run the redists (and the game's install
script) from their own `[Run]` step; with GRID's `SILENT_FLAGS` (`/VERYSILENT /SUPPRESSMSGBOXES ...`) these run
silently. Heroic does nothing extra for sideloaded/offline installers either. Nothing extra is needed.
(Not verified against a specific installer in this research; `grid-install.log` from a real run will show
`__redist\...` executions to confirm.)

---

## 2. Epic (legendary 0.21.1)

### 2.1 Where prerequisites appear

From the Epic manifest meta (`PrereqIds`, `PrereqName`, `PrereqPath`, `PrereqArgs`; binary
`models/manifest.py` L113-117, JSON `json_manifest.py` L104-107). At most **one** prerequisite per game.

- `installed.json` → `<app>.prereq_info`:
  ```json
  "prereq_info": {
    "ids": ["<prereq id>"],
    "name": "UE4 Prerequisites (x64)",
    "path": "Engine/Extras/Redist/en-us/UE4PrereqSetup_x64.exe",
    "args": "/quiet /norestart",
    "installed": true        // only present after legendary marked it (Windows "y"/"i" answer)
  }
  ```
  or `null` (as in GRID's fixture, `epic/mod.rs` L632). `path` is **relative to the install folder** — the
  file is part of the game download; no separate fetch. (Values above are illustrative of the common UE
  case; read the real ones per game.)
- `legendary info <app> --json` → `manifest.prerequisites`: `{"ids","name","path","args"}` or `null`
  (cli.py L1852-1865). Heroic reads this one (`getInstallInfo(...).manifest.prerequisites`).

### 2.2 legendary CLI behaviour

- After install, `core.install_game()` returns `prereq_info` unless `installed` is true; cli prints
  "This game lists the following prerequisites to be installed: - <name>: <path> <args>".
- **Windows:** prompts `[y]es, [n]o, [i]gnore`; with `-y` it answers **`n`** (cli.py L1163-1164: "we don't want
  to launch anything") — nothing runs and nothing is marked. `y` runs `Popen([fullpath, args], cwd=<dir of exe>,
  shell=True)` without elevation and marks `installed`.
- **Linux:** logs "Automatic installation not available on Linux." Nothing runs.
- No `--skip-prereqs` flag exists; `-y` is the only knob. A prereq path containing `uplay`/`ubisoft` makes
  legendary flag the game as Uplay-required.

### 2.3 How Heroic runs them (`legendary/setup.ts`)

- **Linux/macOS:** `legendarySetup` from `prepareWineLaunch` on the first launch of that game in that prefix
  (same `<prefix>/installed_games` marker as GOG). Steps: `reg add HKCR\com.epicgames.launcher /f`
  (fixes FNV, Dishonored DotO), then if Windows platform and not EA/Ubisoft-managed and
  `prerequisites.path` non-empty: `runWineCommand([join(install_path, path), ...shlex.split(args)],
  wait: true /* wineserver --wait */, protonVerb: 'run')`. No exit-code handling. Then EOS overlay enable.
- **Windows:** Heroic installs with `legendary install ... -y` → prereqs are **not** run and Heroic never runs
  them itself (setup only happens on the Wine path). Users rely on redists already on the system.

---

## 3. Steam

GRID starts Steam games through the Steam client (`steam://rungameid/<id>`, `stores/steam/client.rs`).
Steam itself installs "Steamworks Common Redistributables" (app 228980, already filtered in
`stores/steam/filter.rs`) and executes each game's `installscript.vdf` / `_CommonRedist` entries on first
launch ("Performing first-time setup" / "Installing: Microsoft Visual C++ ..."), inside the game's
`compatdata/<appid>/pfx` when Proton is the compat tool, and natively on Windows. Steam records completion
in the prefix's registry / its own config, so GRID needs nothing. (Valve Steamworks docs: "Installation
Scripts" and "Redistributables"; behaviour as observed in Proton.)

---

## 4. Prompts, failure modes, durations (Proton/Wine)

- **Silent under Wine/Proton:** vcredist 2005-2026 with the listed args (2005's nested-quote args survive
  `shlex` splitting), openAL `/S`, PhysX `-s`/`/quiet`, UE4PrereqSetup `/quiet`|`/passive`, DXSETUP `/silent`.
  `/passive` shows a progress window without input — fine in a prefix.
- **.msi entries** (XNA, XNA_40, PHYSXLEGACY): must run as `msiexec /i <msi> <args>`; launching the `.msi`
  directly fails under Wine.
- **.NET Framework (dotNet35/4/45/46/47/472):** frequently hang or fail under Proton/Wine because the prefix
  has wine-mono; winetricks removes mono first. Expect long runs (minutes) or a hang — needs a timeout.
  .NET Core/Desktop Runtime installers generally work.
- **Exit codes:** 0 ok; 3010 = ok, reboot required; 1638 / 0x666 = newer version already installed (ok);
  1602 = cancelled; other = failure. Treat 0/3010/1638 as success.
- **Duration:** vcredist ~5-20 s each, openAL/PhysX ~5 s, DXSETUP 30-90 s, UE4Prereq 30-60 s, .NET 4.x
  2-10 min (or hang). A first launch of a game with 7 deps (Witcher 3) costs ~2-3 min.
- **Windows hosts:** every redist needs elevation → UAC per run (Heroic: one per command for GOG; none for
  Epic). Most redists are usually present system-wide already.
- **Downloads:** sizes per redist from the repository `size`: vcredist 14-26 MB, DirectX 103 MB, .NET 3.5
  246 MB, openAL 0.8 MB.

---

## 5. Recommendation for GRID

### GOG (Windows builds only)
- **List:** `dependencies` from `manifests_dir()/<id>` (v2) or `product.depots[].redist` (v1); missing = `[]`.
  Keep ids whose repository `executable.path` starts with `__redist/` and whose `internal` is false;
  drop empty-path ids (already in the game folder), `ISI` (handled separately), and internal extras
  (QuickTime/Foxit/AdobeAir).
- **Fetch:** one shared folder, e.g. `<gogdl tool dir>/redist`, with
  `gogdl --auth-config-path <...> redist --ids <UNION> --path <dir>` where UNION = `ISI` + the `__redist`
  ids of every installed GOG Windows game (Heroic's rule — a smaller list makes gogdl delete the others).
  Run it as part of the install job after `download` (reuse the gogdl progress parser), and again after an
  update if the list changed. Read `executable` from `<dir>/.gogdl-redist-manifest`.
- **Run (in the game's own prefix, `<game>/<PREFIX_DIR>`), before first launch or at the end of install:**
  1. if `scriptInterpreter: true`: `scriptinterpreter.exe` with the Heroic arg set (1.5) per installed
     product (base + DLCs); else each non-empty `products[].temp_executable` from
     `support_dir(id)/<productId>/`; v1: `support_commands`.
  2. each redist in manifest order: `.exe` → `<exe> + shlex(args)`; `.msi` → `msiexec /i <msi> + shlex(args)`.
  Sequential, per-item timeout (≈10 min), log exit code, never fail the install or block the launch.
- **Track:** a GRID marker in the prefix (e.g. `<prefix>/grid-redist.json` = `{ "<id>": "<repository
  build_id>" }` plus a `script:<buildId>` entry), mirroring Heroic's `installed_games`. Prefix is per
  game in GRID, so "once per prefix" = once per game; deleting the prefix resets it. Re-run the script step
  (not the redists) after updates/moves, like Heroic.
- **Windows host:** run with elevation (one UAC per item, as Heroic) or skip and say so; decide with the user.

### Epic
- **List:** `installed.json` `<app>.prereq_info` (`path`, `args`, `name`, `ids`); skip when `null`, when
  `path` is empty, when `installed` is true, or for EA/Ubisoft-managed titles (already Not supported).
- **Fetch:** nothing — the exe is inside the install folder.
- **Run:** in the game's prefix: `<install>/<path> + shlex(args)`, cwd = the exe's folder. Optionally also
  Heroic's `reg add HKCR\com.epicgames.launcher /f` (fixes some games). Keep `legendary install -y` (it runs
  nothing on Linux, answers "no" on Windows).
- **Track:** same prefix marker keyed by `prereq_info.ids` (+ game version). Optionally set
  `installed: true` is legendary-internal; GRID's own marker is enough.
- **Windows host:** Heroic runs nothing; EGL would run it elevated. Same decision as GOG.

### Steam
Nothing to do — the Steam client runs 228980 and installscript.vdf itself, in the Proton prefix.

### Offline GOG installers
Nothing to do — the Inno installer runs its own `__redist` items under `/VERYSILENT`.
