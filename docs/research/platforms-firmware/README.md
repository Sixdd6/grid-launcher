# Platform names and firmware tables (research, 2026-11-15)

For `docs/romm-storefront-plan.md` Step 0 §0.9 and Step 7. Read-only research; nothing here
is shipped data yet.

## Premise corrections
- The bundled data files are at the repo root: `platform-emulators.json`,
  `romm-platform-cores.json`, `emulator-autoprofiles.json` (84 distinct slugs).
- GRID targets RomM 5.4.1 (`openapi.json` `info.version`). All RomM facts are from tag 5.4.1.
- `pico` is **PICO-8** in RomM; the Sega Pico is `sega-pico`. `romm-platform-cores.json`'s
  `"pico": ["picodrive"]` is a data bug: move it to `sega-pico`.
- Sega Model 3 is `model3`.
- `ps4--1` (IGDB's slug), `sg-1000` and `x68000` (folder aliases) are not RomM slugs. List them only as
  aliases of `ps4`, `sg1000` and `sharp-x68000`.

## Platform names
- RomM's slugs: `backend/utils/platform_slugs.py` `UniversalPlatformSlug` (462 in 5.4.1).
- RomM's name resolution, `backend/handler/metadata/platform_names.py`
  `resolve_platform_name()`, takes the first provider with a name, in this order: IGDB
  (`adapters/services/igdb.py`), ScreenScraper, MobyGames, RetroAchievements, LaunchBox,
  Hasheous, TGDB, Flashpoint, HLTB. If none has a name, it uses the slug in title case.
- Generate `platforms.json` by parsing those lists with `ast` and applying the same order,
  as `names.py` here does. `names.json` is its output for GRID's slugs. Never type names by hand.
- While RomM is connected, the server's `name`, `display_name` and `alternative_names` win.
- Licence: RomM is AGPL-3.0 and GRID is GPL-3.0-only. GPLv3 §13 allows combining the two.
  Slug/name pairs are largely facts. Put an attribution header in the JSON naming
  RomM 5.4.1, the source path and AGPL-3.0.

## Firmware table
- Primary source: libretro `dat/System.dat`, pinned to a commit (`2a8de027`, 2026-10-24).
  It is licensed CC BY-SA 4.0, which is one-way compatible with GPLv3; attribute it and add a
  ShareAlike notice. It has 519 rom lines with name (some with core sub-paths), size, crc, md5 and sha1.
- Map each entry to RomM slugs with RomM's key scheme `"<slug>:<file_name>"`. RomM's
  `backend/models/fixtures/known_bios_files.json` has 374 entries of the same data, already
  keyed that way.
  - Gaps: no DSi, no NAOMI set, nothing for PS3, Vita, Switch, 3DS or Wii U.
  - Four keys can never match: `odyssey-2-slash-videopac-g7000`, `super-gb`, `tvc`, `wolfenstein`.
  - Cross-check against it in a test, so GRID's verdict equals RomM's `is_verified`.
- RomM's verification rule (`models/firmware.py`): the key exists, the size equals, and **any
  one** of md5, sha1 or crc matches.
- Extra entries:
  - xemu `mcpx_1.0.bin`, md5 `d49c52a4102f6df7bcf8d0617ac475ed`.
  - Batocera's Vita PUPs (LGPL-3.0+). They pin one version each, so treat them as name-only.
  - `Complex_4627.bin`: RomM and Batocera give different md5s. Treat it as size-only, or accept either.
- Never copy DuckStation's BIOS database (CC BY-NC-ND).
- Proposed fields: `slug`, `name`, `path_hint` (for example `dc/`, `pcsx2/bios/`,
  `dolphin-emu/Sys/GC/USA/`), `size` (0 means not fixed), `md5`/`sha1`/`crc` ("" means not fixed),
  `required`, `profiles`, and `match: "name" | "content"`.
  - DuckStation, PCSX2 and Ymir identify a BIOS by its content, so any listed hash counts
    under any file name.

## No fixed checksum (name-only; installs as "Installed · not verified")
- **Switch:** `prod.keys`, `title.keys` and firmware NCAs. Switch 2 emulators: unknown.
- **Wii U (Cemu):** `keys.txt`; `otp.bin` and `seeprom.bin` (needed only for online play).
- **3DS (Azahar):** `aes_keys.txt`, `seeddb.bin`, `movable.sed`, `secure_info_A`,
  `LocalFriendCodeSeed_B`, `otp.bin`.
- **PS3 (RPCS3):** `PS3UPDAT.PUP`. The hash changes with each version, so accept any official PUP;
  RPCS3 checks the PUP's internal digests.
- **PS Vita (Vita3K):** `PSVUPDAT.PUP`, `PSP2UPDAT.PUP`. **PS4 (ShadPS4):** `sys_modules`.
- **Xbox (Xemu):** a modded flash BIOS, `xbox_hdd.qcow2`, `eeprom.bin`.
- **DS/DSi (melonDS):** `firmware.bin` (per console; 262144 B for DS), `dsi_firmware.bin`,
  `dsi_nand.bin`. `bios7.bin`, `bios9.bin` and the DSi BIOS files do have fixed hashes.

## Risks
- **Drift:** RomM adds slugs and IGDB renames platforms; System.dat changes too. Pin
  both and regenerate the tables from a script.
- **Accuracy:** never give a per-console file a hash. Don't refuse a valid BIOS only because
  its name differs when the emulator matches by content.
