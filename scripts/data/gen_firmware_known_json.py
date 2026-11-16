#!/usr/bin/env python3
"""Generates the repo root's firmware-known.json: the firmware files GRID can
check and install without RomM (romm-storefront-plan 0.9, Locate files).

Usage (the inputs are parsed as text, never executed):

    python3 -I scripts/data/gen_firmware_known_json.py \\
        <libretro System.dat at 2a8de027> <romm-5.4.1 checkout> <grid-repo>

Sources:
- libretro-database `dat/System.dat` at commit
  2a8de0276fd7d3095f2501c3de3799a14c37e910 (CC BY-SA 4.0): every rom line's
  name, size, crc, md5 and sha1. A name with a folder (`dc/dc_boot.bin`)
  becomes `name` `dc_boot.bin` and `path_hint` `dc/`.
- Each entry is keyed `<slug>:<name>` the way RomM 5.4.1 keys
  `backend/models/fixtures/known_bios_files.json` (AGPL-3.0): the slug is
  RomM's slug for the same name and md5 when RomM has the file, else the
  System.dat section's slug (SECTION_SLUGS). RomM keys that name no RomM
  platform (`odyssey-2-slash-videopac-g7000`) are mapped to the real slug.
- RomM's own entries for GRID's catalog slugs that System.dat lacks (another
  md5 of a known name, `dc_flash.bin`, the Xbox files) are added with
  `source` `romm`, so GRID's verdict equals RomM's `is_verified`.
- NAME_ONLY: per-console files with no fixed checksum (keys, system updates,
  console firmware). They carry no md5, install as "Installed · not verified"
  and are never given a hash (research: docs/research/platforms-firmware).

Policy (GRID's own, not copied from anywhere): REQUIRED names the files a
system cannot start without; files sharing a `group` satisfy each other
(any one of a region's BIOS). CONTENT marks systems whose emulators identify
a BIOS by its content (DuckStation, PCSX2, Ymir): their BIOS dumps are
`match` `content` and form one group. DuckStation's BIOS database is never
used.
"""
import json
import os
import re
import sys

DAT, ROMM, GRID = sys.argv[1], sys.argv[2], sys.argv[3]
DAT_COMMIT = "2a8de0276fd7d3095f2501c3de3799a14c37e910"

# System.dat sections RomM's table does not cover (or covers only in part):
# the RomM slug for the section's files. A section absent here takes the
# slug RomM uses for most of its files.
SECTION_SLUGS = {
    "Commodore - C128": "c128",
    "Dinothawr": None,  # a game's own data, not a system's firmware
    "DOS": "dos",
    "Elektronika - BK-0010/BK-0011(M)": "bk",
    "EPOCH/YENO Super Cassette Vision": "epoch-super-cassette-vision",
    "Galaksija": "galaksija",
    "NEC - PC-8801": "pc-8800-series",
    "Nintendo - Nintendo DSi": "nintendo-dsi",
    "Philips - CD-i": "philips-cd-i",
    "Palm OS": "palm-os",
    "Texas Instruments TI-83": "ti-83",
    "Vircon32": None,  # no RomM platform
    "ScummVM": "scummvm",
    "Arcade": "arcade",
    "Sega - Dreamcast-based Arcade": "arcade",
    "Magnavox - Odyssey2": "odyssey-2",
}
# RomM keys that can never match a RomM platform, and the slug they mean.
DEAD_KEYS = {"odyssey-2-slash-videopac-g7000": "odyssey-2"}

# Content-matched BIOS dumps: slug -> the BIOS size that marks a dump.
CONTENT = {"psx": 524288, "ps2": 4194304, "saturn": 524288}
# Saturn files of BIOS size that are not BIOS dumps (cartridge ROMs).
NOT_BIOS = {("saturn", "mpr-18811-mx.ic1"), ("saturn", "mpr-19367-mx.ic1")}

# slug -> groups; a system needs one file of each group.
REQUIRED = {
    "3do": [["goldstar.bin", "panafz1.bin", "panafz10.bin", "panafz10-norsa.bin",
             "panafz10e-anvil.bin", "panafz10e-anvil-norsa.bin", "panafz1j.bin",
             "panafz1j-norsa.bin", "sanyotry.bin"]],
    "atari5200": [["5200.rom"]],
    "atari-st": [["tos.img"]],
    "lynx": [["lynxboot.img"]],
    "colecovision": [["colecovision.rom", "coleco.rom", "BIOS.col", "bioscv.rom"]],
    "intellivision": [["exec.bin"], ["grom.bin"]],
    "odyssey-2": [["o2rom.bin"]],
    "fairchild-channel-f": [["sl31253.bin"], ["sl31254.bin"]],
    "fds": [["disksys.rom"]],
    "pc-fx": [["pcfx.rom", "pcfxbios.bin", "pcfxv101.bin", "pcfxga.rom"]],
    "segacd": [["bios_CD_U.bin", "bios_CD_E.bin", "bios_CD_J.bin"]],
    "dc": [["dc_boot.bin", "boot.bin"]],
    "satellaview": [["BS-X.bin", "BS-X (En).bin", "BS-X (En) (DRM-Free).bin"]],
    "sufami-turbo": [["STBIOS.bin"]],
    "sharp-x68000": [["iplrom.dat", "iplromxv.dat", "iplromco.dat", "iplrom30.dat"], ["cgrom.dat"]],
    "xbox": [["mcpx_1.0.bin"], ["Complex_4627.bin"]],
    "switch": [["prod.keys"]],
    "ps3": [["PS3UPDAT.PUP"]],
    "psvita": [["PSVUPDAT.PUP", "PSP2UPDAT.PUP"]],
}

# Per-console files: name-only, no checksum ever. (slug, name, size or 0).
NAME_ONLY = [
    ("switch", "prod.keys", 0),
    ("switch", "title.keys", 0),
    ("wiiu", "keys.txt", 0),
    ("wiiu", "otp.bin", 0),
    ("wiiu", "seeprom.bin", 0),
    ("3ds", "aes_keys.txt", 0),
    ("3ds", "seeddb.bin", 0),
    ("3ds", "movable.sed", 0),
    ("3ds", "secure_info_A", 0),
    ("3ds", "LocalFriendCodeSeed_B", 0),
    ("3ds", "otp.bin", 0),
    ("ps3", "PS3UPDAT.PUP", 0),
    ("psvita", "PSVUPDAT.PUP", 0),
    ("psvita", "PSP2UPDAT.PUP", 0),
    ("xbox", "eeprom.bin", 0),
    ("nds", "firmware.bin", 262144),
    ("nintendo-dsi", "dsi_firmware.bin", 0),
    ("nintendo-dsi", "dsi_nand.bin", 0),
]
# Files RomM lists with a hash that GRID keeps name-only (per console).
NAME_ONLY_KEYS = {(s, n.lower()) for s, n, _ in NAME_ONLY}
# Complex_4627.bin: RomM and Batocera give different md5s, so only its
# size is checked (research, "Extra entries").
SIZE_ONLY = {("xbox", "complex_4627.bin")}

ROM = re.compile(
    r'\s*rom \( name (".*?"|\S+)(?: size (\d+))?(?: crc (\w+))? md5 (\w+)(?: sha1 (\w+))? \)'
)


def parse_dat():
    rows, section = [], None
    with open(DAT, encoding="utf-8") as f:
        for line in f:
            m = re.match(r'\s*comment "(.*)"', line)
            if m:
                section = m.group(1)
                continue
            m = ROM.match(line)
            if m:
                full = m.group(1).strip('"')
                rows.append({
                    "section": section,
                    "name": full.rsplit("/", 1)[-1],
                    "path_hint": full.rsplit("/", 1)[0] + "/" if "/" in full else "",
                    "size": int(m.group(2) or 0),
                    "crc": (m.group(3) or "").lower(),
                    "md5": m.group(4).lower(),
                    "sha1": (m.group(5) or "").lower(),
                })
            elif "rom (" in line:
                raise SystemExit(f"unparsed System.dat line: {line.strip()}")
    return rows


def main():
    with open(os.path.join(ROMM, "backend/models/fixtures/known_bios_files.json"), encoding="utf-8") as f:
        romm = json.load(f)
    with open(os.path.join(GRID, "platforms.json"), encoding="utf-8") as f:
        catalog = {p["slug"] for p in json.load(f)["platforms"]}

    romm_by = {}
    for key, v in romm.items():
        slug, name = key.split(":", 1)
        slug = DEAD_KEYS.get(slug, slug)
        romm_by.setdefault((name.lower(), v["md5"].lower()), set()).add(slug)

    rows = parse_dat()
    majority = {}
    for r in rows:
        for slug in romm_by.get((r["name"].lower(), r["md5"]), ()):
            majority.setdefault(r["section"], {}).setdefault(slug, 0)
            majority[r["section"]][slug] += 1

    entries = []
    for r in rows:
        slugs = sorted(romm_by.get((r["name"].lower(), r["md5"]), ()))
        if not slugs:
            if r["section"] in SECTION_SLUGS:
                slug = SECTION_SLUGS[r["section"]]
            elif r["section"] in majority:
                slug = max(majority[r["section"]].items(), key=lambda kv: kv[1])[0]
            else:
                raise SystemExit(f"no slug for System.dat section {r['section']!r}")
            slugs = [slug] if slug else []
        for slug in slugs:
            entries.append({"slug": slug, **{k: r[k] for k in ("name", "path_hint", "size", "md5", "sha1", "crc")},
                            "source": "libretro"})

    have = {(e["slug"], e["name"].lower(), e["md5"]) for e in entries}
    for key, v in romm.items():
        slug, name = key.split(":", 1)
        slug = DEAD_KEYS.get(slug, slug)
        if slug not in catalog or (slug, name.lower(), v["md5"].lower()) in have:
            continue
        entries.append({"slug": slug, "name": name, "path_hint": "", "size": int(v["size"]),
                        "md5": v["md5"].lower(), "sha1": v["sha1"].lower(), "crc": v["crc"].lower(),
                        "source": "romm"})

    # Name-only files: drop any hashed copy, then one entry without a hash.
    entries = [e for e in entries if (e["slug"], e["name"].lower()) not in NAME_ONLY_KEYS]
    for slug, name, size in NAME_ONLY:
        entries.append({"slug": slug, "name": name, "path_hint": "", "size": size,
                        "md5": "", "sha1": "", "crc": "", "source": "grid"})
    for e in entries:
        if (e["slug"], e["name"].lower()) in SIZE_ONLY:
            e["md5"] = e["sha1"] = e["crc"] = ""
    # One size-only entry is enough for a name.
    seen, unique = set(), []
    for e in entries:
        k = (e["slug"], e["name"].lower(), e["md5"], e["path_hint"])
        if k not in seen:
            seen.add(k)
            unique.append(e)
    entries = unique

    names_by_slug = {}
    for e in entries:
        names_by_slug.setdefault(e["slug"], set()).add(e["name"])
    group_of = {}
    for slug, groups in REQUIRED.items():
        for i, group in enumerate(groups):
            for name in group:
                if name not in names_by_slug.get(slug, set()):
                    raise SystemExit(f"REQUIRED names {slug}:{name}, which is not in the table")
                group_of[(slug, name)] = f"{slug}-{i + 1}"
    for e in entries:
        content = (
            e["slug"] in CONTENT
            and e["size"] == CONTENT[e["slug"]]
            and (e["slug"], e["name"]) not in NOT_BIOS
            and e["md5"] != ""
        )
        e["match"] = "content" if content else "name"
        group = f"{e['slug']}-bios" if content else group_of.get((e["slug"], e["name"]), "")
        e["required"] = group != ""
        e["group"] = group
        e["profiles"] = []

    entries.sort(key=lambda e: (e["slug"], e["name"].lower(), e["path_hint"], e["md5"]))
    order = ["slug", "name", "path_hint", "size", "md5", "sha1", "crc", "required", "group",
             "match", "profiles", "source"]
    out = {
        "_comment": (
            "Firmware files GRID can check and install without RomM (romm-storefront-plan "
            "0.9), generated by scripts/data/gen_firmware_known_json.py; do not edit by hand. "
            "Hashes from libretro-database dat/System.dat at commit " + DAT_COMMIT + ", "
            "(c) libretro, licensed CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/). "
            "ShareAlike: this file is an adaptation of System.dat and is shared under CC BY-SA 4.0. "
            "Entries keyed <slug>:<name> as RomM 5.4.1 keys backend/models/fixtures/"
            "known_bios_files.json (RomM, https://github.com/rommapp/romm, AGPL-3.0); entries "
            "with source romm come from that file. Entries with an empty md5 are per-console "
            "files with no fixed checksum: they install as not verified. required/group/match "
            "are GRID's own policy."
        ),
        "sources": [
            {"project": "libretro-database", "path": "dat/System.dat", "commit": DAT_COMMIT,
             "license": "CC BY-SA 4.0"},
            {"project": "RomM", "path": "backend/models/fixtures/known_bios_files.json",
             "version": "5.4.1", "license": "AGPL-3.0"},
        ],
        "files": [{k: e[k] for k in order} for e in entries],
    }
    with open(os.path.join(GRID, "firmware-known.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1, ensure_ascii=False)
        f.write("\n")
    print(f"{len(entries)} files, {len(names_by_slug)} slugs")


main()
