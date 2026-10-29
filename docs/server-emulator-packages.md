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
<Emulator> (v<version>) (arch-<system>-<cpu>).<extension>
```

The two tags may come in either order, but both must come after the emulator's
name.

- **`<Emulator>`** — the emulator's name as GRID knows it, without the platform
  in brackets: `PCSX2`, `DuckStation`, `Dolphin`, `RetroArch`, `Pico-8`,
  `Xenia Canary`. Upper and lower case do not matter. A name GRID does not know
  is ignored.
- **`(v<version>)`** — the version, in RomM's version-tag style: `v` followed by
  numbers separated by dots, optionally ending in letters (`v2.0.2`, `v1.21`,
  `v0.2.5g`, `v0.2.7a6`, `v2.0.0-rc.1`). GRID compares versions number by
  number to find the newest. Letters after a number follow the usual
  convention:
  - a pre-release comes **before** its release: `a6` or `alpha` (alpha), `b2`
    or `beta`, `rc1` (release candidate), `dev`. So `0.2.7a6` is older than
    `0.2.7`.
  - a single letter on its own is a **later** fix: `0.2.5g` is newer than
    `0.2.5`.
- **`(arch-<system>-<cpu>)`** — what the build runs on. Exactly one per file;
  a file without it, or with only the system (`arch-linux`), is ignored.
  - `<system>`: `linux`, `windows` or `macos`.
  - `<cpu>`:
    - `x64` — 64-bit Intel and AMD PCs, and Intel Macs (`x86_64` and `amd64`
      are read too)
    - `arm64` — 64-bit ARM: Apple-chip Macs, 64-bit Raspberry Pi OS, ARM
      Windows laptops (`aarch64` is read too)
    - `armhf` — 32-bit ARM: 32-bit Raspberry Pi OS
    - `universal` — macOS only: one build for both Intel and Apple-chip Macs

  For example: `arch-linux-x64`, `arch-windows-x64`, `arch-linux-armhf`,
  `arch-macos-universal`.
- **`<extension>`** — what GRID knows how to install:
  - `.AppImage` (kept as it is)
  - `.zip`, `.7z`, `.tar.gz`, `.tgz`, `.tar.xz` (unpacked)
  - `.exe` for a portable Windows build

## Several versions

A folder may hold several versions of the same emulator. GRID installs the
newest one for the system and processor it is running on. A Mac takes a
`universal` build when there is no newer one for its own processor; at the
same version, the build for its own processor wins. When a newer version is added
later, GRID offers it as an update for the one already installed.

## A build for the wrong processor

GRID reads the program inside each build before it registers it. A build whose
program is for another processor or system — a Raspberry Pi zip tagged
`arch-linux-x64`, say — fails to install, and the Downloads row says what the
program is built for and which file's tag to check. Nothing is registered.

## Example

```
roms/emulators/
  ps2/
    PCSX2 (v2.0.2) (arch-linux-x64).AppImage
    PCSX2 (v2.0.2) (arch-windows-x64).zip
    PCSX2 (v1.6.0) (arch-windows-x64).zip
  pico/
    Pico-8 (v0.2.7a6) (arch-linux-x64).zip
    Pico-8 (v0.2.7a6) (arch-linux-armhf).zip
    Pico-8 (v0.2.7a6) (arch-windows-x64).zip
  multi/
    RetroArch (v1.19.1) (arch-linux-x64).AppImage
    Dolphin (v2412) (arch-windows-x64).7z
    Dolphin (v2412) (arch-macos-universal).7z
```

On a Linux PC, GRID offers PCSX2 2.0.2 for PlayStation 2, the x64 Pico-8
0.2.7a6 for Pico-8, and RetroArch 1.19.1 for the platforms RetroArch has a core
for. A 32-bit Raspberry Pi gets the `armhf` Pico-8 instead. The Windows and
macOS Dolphin builds are not offered on Linux.

After adding or renaming files, rescan the Emulators platform in RomM so it
picks up the change.
