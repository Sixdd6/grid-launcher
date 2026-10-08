# Dolphin auto-install and KytyPS5 `.zar` games — design

Date: 2026-10-08. Status: approved in conversation, awaiting spec review.

## Goal

1. GRID installs and launches Dolphin (GameCube / Wii) like its other catalog emulators:
   fullscreen, Vulkan, 1080p internal resolution, portable user data.
2. KytyPS5 runs PS5 games stored on RomM as `.zar` (ZArchive) files. `.pkg` games and bare
   multi-file folder games are rejected with a clear message.

## Verified external facts

These come from the upstream source, read on 2026-10-08.

**Dolphin** (`dolphin-emu/dolphin`, `Source/Core`)

- `UICommon/CommandLineParse.cpp`: `-u/--user PATH`, `-e/--exec FILE`, `-b/--batch`
  (exit when emulation stops; needs `-e`), `-v/--video_backend` (value `Vulkan`),
  `-C/--config System.Section.Key=Value`.
- `Common/Config/Config.cpp`: the `Main` system's name is `Dolphin`; `GFX` is `GFX`.
- `Core/Config/MainSettings.cpp`: `Dolphin.Display.Fullscreen`.
  `Core/Config/GraphicsSettings.cpp`: `GFX.Settings.InternalResolution` (an integer scale of
  native; 3 is the value Dolphin's UI labels "1080p").
- `UICommon/UICommon.cpp`: on Windows, Linux and macOS, `<exe dir>/portable.txt` selects
  `<exe dir>/User`. Inside an AppImage the exe dir is the read-only mount, so the AppImage
  needs `-u`. `-u` overrides every other rule on every OS.
- Official downloads: `https://dolphin-emu.org/update/latest/beta/` returns JSON naming the
  newest release (2609a today). Windows x64 is a portable `.7z`
  (`https://dl.dolphin-emu.org/releases/<rev>/dolphin-<rev>-x64.7z`). Linux is a Flatpak
  bundle only. macOS is a `.dmg` only. `dolphin-emu/dolphin` has no GitHub release assets.
- Unofficial Linux AppImage: `pkgforge-dev/Dolphin-emu-AppImage`. Stable releases are normal
  releases (tag `2609@2026-10-01_…`, asset `Dolphin_Emulator-2609-anylinux-x86_64.AppImage`).
  A rolling `nightly` release is a prerelease. Every release also carries `.zsync` files
  and `aarch64` builds.

**KytyPS5** (`KytyPS5/KytyPS5`, `src/main.cpp`, `src/common/archive.cpp`)

- `--fullscreen` is a real flag. `--screen-width N` and `--screen-height N` set the output
  size. Vulkan is the only renderer.
- `--game <path>` accepts a directory, an archive, or an ELF file. The only archive format is
  `.zar` (ZArchive, read through `zarchive/zarchivereader.h`). Kyty opens the archive in
  place and requires `eboot.bin` at its root. `.zar` is not a zip and GRID must not extract
  it.
- Kyty has no `.pkg` support.

## Decisions (user rulings, 2026-10-08)

1. Dolphin on Linux uses the unofficial `pkgforge-dev` AppImage. Flatpak is rejected: GRID
   must not manage Flatpak data folders.
2. Dolphin on Windows uses the official `.7z` (extract and run).
3. Dolphin is not offered on macOS.
4. PS5: `.zar` is the main format. The existing `.zip`/`.7z` route that extracts and ranks
   `eboot.bin` stays. `.pkg` and bare multi-file folders are rejected at install, before any
   download.
5. `.pkg` support and fixing multi-file folder downloads are out of scope.

## Design

### 1. Dolphin catalog profile

A new entry in `emulator-autoprofiles.json`:

- Name: `Dolphin (GameCube / Wii)`.
- `platform_keywords`: `gamecube`, `ngc`, `wii`. `platform_matches_keywords` blocks a
  trailing extra word, so `wii` does not match "Wii U" (Cemu keeps Wii U). A test pins this.
- `match_tokens`: the Windows exe (`Dolphin.exe`), the Linux binary names, and the AppImage
  glob (`dolphin*.appimage`).
- Source: the `direct` provider on Windows with
  `page_url = https://dolphin-emu.org/update/latest/beta/` and a `download_url_regex` that
  matches the `-x64.7z` URL under `dl.dolphin-emu.org/releases/`. The scraper falls back to
  a regex search of the whole page, which matches the URL in the JSON text. The `linux`
  platform override switches to a GitHub release source on `pkgforge-dev/Dolphin-emu-AppImage`
  with `asset_patterns` for `Dolphin_Emulator-*-anylinux-x86_64.AppImage`, no prereleases,
  and the `.zsync` files excluded. If a `direct` source cannot override into a GitHub source,
  the planner chooses the smallest change that gives one profile per-OS providers, or two
  per-OS profiles with one display name.
- `source.platforms`: `win32`, `linux`.
- `args`: `-u "%emu_dir%/User" -b -v Vulkan -C Dolphin.Display.Fullscreen=True
  -C GFX.Settings.InternalResolution=3 -e "%rom%"`.
- Save data: `save_directories` / `state_directories` / `user_data` / `firmware_directories`
  point under `User/` (`User/GC`, `User/Wii`, `User/StateSaves`, `User/Config`). The
  planner confirms the exact entries against `cloud/dirs.rs`, the Dolphin readers and
  `user_data_links.rs`.

The existing `autoconfig/dolphin.rs` already targets `<exe dir>/User/Config` and writes
`portable.txt`, so the settings writer, the Vulkan seed and the firmware hooks need no
change.

Docs: `future-plans/platform-support.md` (the "Dolphin and MAME are not part of
auto-install" lines and the Wii/GameCube row) and the 2026-09-15 "Decision 9" note now say
Dolphin is in the catalog.

### 2. `%emu_dir%` launch placeholder

- `%emu_dir%` expands to the directory that holds the entry's executable (the AppImage's
  directory for an AppImage).
- It joins `%rom%`, `%core%` and `%ps3_launch_target%` in `launch/template.rs`.
- If the executable path is blank or has no parent, launch fails with a clear error. It
  never produces a bare `-u` or `-u /User`.
- Paths with spaces stay one argument (same quoting rules as `%rom%`).

### 3. KytyPS5

- `args`: `--fullscreen --screen-width 1920 --screen-height 1080 --game "%rom%"`, with
  `legacy_args: ["--fullscreen --game \"%rom%\""]` so the startup migration moves existing
  installs to the new args.
- `.zar`: a single `.zar` file on a PS5 platform takes the Downloaded route. It is not
  extracted and `%rom%` is the `.zar` path. A test pins that `.zar` never takes the Eboot
  route.
- `.zip`/`.7z` with `eboot.bin`: unchanged.
- Rejection: on a PS5 platform, a game whose download set is a `.pkg` file, or more than one
  file (a bare folder), fails at install planning, before any download. The message names
  the reason and the accepted formats: "KytyPS5 needs a .zar archive, or a .zip/.7z that
  contains eboot.bin. .pkg files and folder games are not supported."

## Testing

- Rust unit tests:
  - Dolphin profile and catalog rows, the platform gate (win32/linux only), and keyword
    matching (GameCube and Wii yes, Wii U no).
  - Asset selection: the regex against a captured copy of the `update/latest/beta` JSON, and
    the AppImage patterns against a captured release list (stable picked, nightly and
    `.zsync` skipped).
  - `%emu_dir%` expansion, paths with spaces, and the blank-path error.
  - Kyty: `.zar` takes the Downloaded route; `.pkg` and multi-file PS5 games are rejected
    before download; the `legacy_args` migration rewrites the old Kyty args.
- Integration test in `crates/grid-core/tests/emulator_install.rs`: a Dolphin AppImage
  install lands in `emulators/Dolphin (GameCube / Wii)/` and its launch args expand `-u` to
  that directory.
- e2e: a Dolphin install through the mock catalog if the existing emulator-install stage
  supports a new row without new fixtures.
- Manual live check (user's machine): install Dolphin, launch one GameCube and one Wii game;
  install KytyPS5, launch one `.zar` game; try one `.pkg` game and see the rejection.

## Out of scope

- `.pkg` extraction or install, for PS4 or PS5.
- Downloading nested files of multi-file RomM games.
- Dolphin on macOS, and the Dolphin Flatpak.
- A KytyPS5 settings writer (Kyty takes everything it needs on the command line).
