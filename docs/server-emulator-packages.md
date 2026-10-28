# Packaging emulators on a RomM server for GRID

GRID can install emulators straight from your RomM server, as well as from its
own catalog. For GRID to find them, know which platform each one serves, pick
the right build for the computer it runs on, and spot newer versions, the
emulators must be laid out as below.

Anything in the Emulators platform that does not follow this layout is ignored
by GRID's platform setup. It can still be installed from the Server view like
any other ROM.

## Where the files go

All emulators live in RomM's **Emulators** platform: the folder
`roms/emulators/` (or wherever your RomM library keeps that platform).

Inside it, make **one folder per platform**, named exactly like that
platform's **slug** in RomM: `ps2`, `ngc`, `pico`, `snes`. The slug is the
short id RomM shows for the platform. It is not the display name ("PlayStation
2" is `ps2`, "Pico-8" is `pico`).

An emulator that serves several platforms — RetroArch, or Dolphin (GameCube and
Wii) — goes in one shared folder named **`multi`** instead of being copied into
every platform's folder. GRID works out which platforms it serves from its own
list of emulators.

RomM shows each folder as one entry in the Emulators platform (named `ps2`,
`pico`, `multi`), with the builds as its files.

## How each build is named

Each build is one file, named:

```
<Emulator> (v<version>) (arch-<system>).<extension>
```

- **`<Emulator>`** — the emulator's name as GRID knows it, without the platform
  in brackets: `PCSX2`, `DuckStation`, `Dolphin`, `RetroArch`, `Pico-8`,
  `Xenia Canary`. Upper and lower case do not matter. A name GRID does not know
  is ignored.
- **`(v<version>)`** — the version, in RomM's version-tag style: `v` followed by
  numbers separated by dots, optionally ending in letters (`v2.0.2`, `v1.21`,
  `v0.2.7a6`). GRID compares versions number by number to find the newest.
- **`(arch-<system>)`** — the system the build runs on: `arch-linux`,
  `arch-windows` or `arch-macos`. Exactly one per file. A file without it is
  ignored. Builds are assumed to be for 64-bit x86 PCs.
- **`<extension>`** — what GRID knows how to install:
  - `.AppImage` (kept as it is)
  - `.zip`, `.7z`, `.tar.gz`, `.tgz`, `.tar.xz` (unpacked)
  - `.exe` for a portable Windows build

## Several versions

A folder may hold several versions of the same emulator. GRID installs the
newest one for the system it is running on. When a newer version is added
later, GRID offers it as an update for the one already installed.

## Example

```
roms/emulators/
  ps2/
    PCSX2 (v2.0.2) (arch-linux).AppImage
    PCSX2 (v2.0.2) (arch-windows).zip
    PCSX2 (v1.6.0) (arch-windows).zip
  pico/
    Pico-8 (v0.2.7a6) (arch-linux).zip
    Pico-8 (v0.2.7a6) (arch-windows).zip
  multi/
    RetroArch (v1.19.1) (arch-linux).AppImage
    Dolphin (v2412) (arch-windows).7z
```

On a Linux PC, GRID offers PCSX2 2.0.2 for PlayStation 2, Pico-8 0.2.7a6 for
Pico-8, and RetroArch 1.19.1 for the platforms RetroArch has a core for. The
Windows-only Dolphin build is not offered on Linux.

After adding or renaming files, rescan the Emulators platform in RomM so it
picks up the change.
