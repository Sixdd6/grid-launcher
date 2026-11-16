#!/usr/bin/env python3
"""Generates the repo root's platforms.json: GRID's own platform catalog.

Usage (nothing is imported from the RomM checkout; its lists are parsed):

    python3 -I scripts/data/gen_platforms_json.py <romm-5.4.1-checkout> <grid-repo>

Each platform's `name` is RomM's default name for its slug, resolved the way
RomM 5.4.1 resolves it (`backend/handler/metadata/platform_names.py`
`resolve_platform_name`): the first provider list with a name, in the order
IGDB (or its platform-versions list), ScreenScraper, MobyGames,
RetroAchievements, LaunchBox, Hasheous, TGDB, Flashpoint, HLTB; else the slug
in title case. The lists are read with `ast`, never executed. No name is
typed by hand. `aliases` are the other providers' names for the slug, minus
any name another catalog platform also goes by.

Which slugs: every slug of platform-emulators.json and
romm-platform-cores.json, plus the systems GRID has an emulator profile for
but no slug file names (PICO-8 `pico`, Sega Model 3 `model3`) and the Sega
Pico (`sega-pico`). `slug_aliases` are slugs RomM does not have that older
data used for the same platform (IGDB's `ps4--1`, the folder names `sg-1000`
and `x68000`).
"""
import ast
import json
import os
import sys

ROMM, GRID = sys.argv[1], sys.argv[2]
BACKEND = os.path.join(ROMM, "backend")

PROVIDERS = [
    ("igdb", "adapters/services/igdb.py", "IGDB_PLATFORM_LIST"),
    ("ss", "handler/metadata/ss_handler.py", "SCREENSAVER_PLATFORM_LIST"),
    ("moby", "handler/metadata/moby_handler.py", "MOBYGAMES_PLATFORM_LIST"),
    ("ra", "handler/metadata/ra_handler.py", "RA_PLATFORM_LIST"),
    ("lb", "handler/metadata/launchbox_handler/platforms.py", "LAUNCHBOX_PLATFORM_LIST"),
    ("hasheous", "handler/metadata/hasheous_handler.py", "HASHEOUS_PLATFORM_LIST"),
    ("tgdb", "handler/metadata/tgdb_handler.py", "TGDB_PLATFORM_LIST"),
    ("fp", "handler/metadata/flashpoint_handler.py", "FLASHPOINT_PLATFORM_LIST"),
    ("hltb", "handler/metadata/hltb_handler.py", "HLTB_PLATFORM_LIST"),
]

EXTRA_SLUGS = ["pico", "model3", "sega-pico"]
SLUG_ALIASES = {"ps4": ["ps4--1"], "sg1000": ["sg-1000"], "sharp-x68000": ["x68000"]}


def read(rel):
    with open(os.path.join(BACKEND, rel), encoding="utf-8") as f:
        return f.read()


def universal_slugs():
    out = {}
    for node in ast.walk(ast.parse(read("utils/platform_slugs.py"))):
        if isinstance(node, ast.ClassDef) and node.name == "UniversalPlatformSlug":
            for stmt in node.body:
                if isinstance(stmt, ast.Assign):
                    out[stmt.targets[0].id] = stmt.value.value
    return out


UPS = universal_slugs()


def load(rel, var):
    for node in ast.parse(read(rel)).body:
        if isinstance(node, ast.AnnAssign) and getattr(node.target, "id", None) == var:
            out = {}
            for key, value in zip(node.value.keys, node.value.values):
                if isinstance(key, ast.Attribute):
                    slug = UPS[key.attr]
                elif isinstance(key, ast.Constant):
                    slug = key.value
                else:
                    continue
                entry = {}
                if isinstance(value, ast.Dict):
                    for k, v in zip(value.keys, value.values):
                        if not isinstance(k, ast.Constant):
                            continue
                        try:
                            entry[k.value] = ast.literal_eval(v)
                        except ValueError:
                            entry[k.value] = None
                out[slug] = entry
            return out
    raise SystemExit(f"{var} not found in {rel}")


LISTS = {name: load(rel, var) for name, rel, var in PROVIDERS}
VERSIONS = load("adapters/services/igdb.py", "IGDB_PLATFORM_VERSIONS")


def provider_names(slug):
    names = []
    for name, _, _ in PROVIDERS:
        if name == "igdb" and slug not in LISTS["igdb"] and slug in VERSIONS:
            found = VERSIONS[slug].get("name")
        else:
            found = (LISTS[name].get(slug) or {}).get("name")
        if found:
            names.append(found)
    return names


def resolve(slug):
    names = provider_names(slug)
    return names[0] if names else slug.replace("-", " ").title()


def main():
    with open(os.path.join(GRID, "platform-emulators.json"), encoding="utf-8") as f:
        recommended = json.load(f)["recommended"]
    with open(os.path.join(GRID, "romm-platform-cores.json"), encoding="utf-8") as f:
        cores = json.load(f)
    aliased = {a for v in SLUG_ALIASES.values() for a in v}
    slugs = sorted((set(recommended) | set(cores) | set(EXTRA_SLUGS)) - aliased)
    universal = set(UPS.values())
    missing = [s for s in slugs if s not in universal]
    if missing:
        raise SystemExit(f"not RomM 5.4.1 slugs: {missing}")

    rows = []
    for slug in slugs:
        name = resolve(slug)
        others = []
        for alt in provider_names(slug):
            if alt != name and alt not in others:
                others.append(alt)
        rows.append({"slug": slug, "name": name, "aliases": others})

    # An alias another platform also goes by (as its name or an alias) would
    # send a config key to the wrong slug: drop it from every platform.
    seen = {}
    for row in rows:
        for n in [row["name"], *row["aliases"]]:
            seen.setdefault(n.casefold(), set()).add(row["slug"])
    for row in rows:
        row["aliases"] = [a for a in row["aliases"] if len(seen[a.casefold()]) == 1]
        if row["slug"] in SLUG_ALIASES:
            row["slug_aliases"] = SLUG_ALIASES[row["slug"]]
    names = [row["name"].casefold() for row in rows]
    if len(set(names)) != len(names):
        raise SystemExit("two catalog platforms resolve to one name")

    out = {
        "_comment": (
            "GRID's platform catalog (romm-storefront-plan 0.9), generated by "
            "scripts/data/gen_platforms_json.py; do not edit by hand. Each name is "
            "RomM 5.4.1's default platform name for the slug, resolved as RomM's "
            "backend/handler/metadata/platform_names.py resolve_platform_name does from "
            "the provider lists in backend/adapters/services/igdb.py and "
            "backend/handler/metadata/*.py. Source: RomM "
            "(https://github.com/rommapp/romm), tag 5.4.1, licensed AGPL-3.0. "
            "aliases are the other providers' names; slug_aliases are slugs older "
            "data used for the same platform."
        ),
        "source": {
            "project": "RomM",
            "url": "https://github.com/rommapp/romm",
            "version": "5.4.1",
            "paths": [
                "backend/utils/platform_slugs.py",
                "backend/handler/metadata/platform_names.py",
            ],
            "license": "AGPL-3.0",
        },
        "platforms": rows,
    }
    with open(os.path.join(GRID, "platforms.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"{len(rows)} platforms")


main()
