from __future__ import annotations

import json
import re
from pathlib import Path


def retroarch_core_list_path(base_path: Path) -> Path:
    return base_path / "retroarch-core-list.json"


def retroarch_markdown_label(value: str) -> str:
    text = value.strip()
    if not text.startswith("["):
        return text
    marker = text.find("](")
    if marker <= 1 or not text.endswith(")"):
        return text
    return text[1:marker].strip()


def retroarch_core_id_from_name(core_name: str) -> str:
    normalized_name = retroarch_markdown_label(core_name).strip().casefold()
    overrides = {
        "beetle psx": "mednafen_psx",
        "beetle psx hw": "mednafen_psx_hw",
        "beetle saturn": "mednafen_saturn",
        "beetle vb": "mednafen_vb",
        "fb neo": "fbneo",
        "fceumm": "fceumm",
        "flycast gles2": "flycast",
        "lrps2": "lrps2",
        "mame 2003-plus": "mame2003_plus",
        "mesen-s": "mesen_s",
        "mupen64plus-next": "mupen64plus_next",
        "mupen64plus-next gles2": "mupen64plus_next",
        "mupen64plus-next gles3": "mupen64plus_next",
        "parallel n64": "parallel_n64",
        "pcsx rearmed": "pcsx_rearmed",
        "snes9x 2002": "snes9x2002",
        "snes9x 2005": "snes9x2005",
        "snes9x 2005 plus": "snes9x2005_plus",
        "snes9x 2010": "snes9x2010",
        "same cdi": "same_cdi",
        "vba-m": "vbam",
        "vba next": "vba_next",
    }
    mapped = overrides.get(normalized_name)
    if mapped:
        return mapped

    sanitized: list[str] = []
    previous_underscore = False
    for character in normalized_name:
        if character.isalnum():
            sanitized.append(character)
            previous_underscore = False
            continue
        if previous_underscore:
            continue
        sanitized.append("_")
        previous_underscore = True

    return "".join(sanitized).strip("_")


def retroarch_core_id_from_file_name(core_file_name: str) -> str:
    normalized = core_file_name.strip().replace("\\", "/")
    if not normalized:
        return ""

    file_name = normalized.rsplit("/", 1)[-1].casefold()
    if file_name.endswith(".dll"):
        file_name = file_name[:-4]
    if file_name.endswith("_libretro"):
        file_name = file_name[: -len("_libretro")]
    return file_name.strip()


def normalize_retroarch_platform_key(value: str) -> str:
    normalized = value.strip().casefold()
    if not normalized:
        return ""

    normalized = normalized.replace("\\", "/")
    normalized = re.sub(r"[^a-z0-9]+", " ", normalized)
    normalized = re.sub(r"\s+", " ", normalized)
    return normalized.strip()


def retroarch_platform_tokens(value: str) -> set[str]:
    normalized = re.sub(r"[^a-z0-9]+", " ", value.strip().casefold())
    ignored_tokens = {"the", "and", "of", "for", "system"}
    return {token for token in normalized.split() if token and token not in ignored_tokens}


def all_retroarch_cores(compatibility: dict[str, list[str]]) -> list[str]:
    cores: list[str] = []
    for mapped_cores in compatibility.values():
        for core in mapped_cores:
            if core not in cores:
                cores.append(core)
    return cores


def load_retroarch_compatibility_map(path: Path) -> dict[str, list[str]]:
    compatibility: dict[str, list[str]] = {}
    if not path.exists():
        return compatibility

    try:
        raw_content = path.read_text(encoding="utf-8")
    except OSError:
        return compatibility

    try:
        entries = json.loads(raw_content)
    except json.JSONDecodeError:
        entries = None

    if isinstance(entries, list):
        for entry in entries:
            if not isinstance(entry, dict):
                continue

            core_file = entry.get("core_file", "")
            if not isinstance(core_file, str) or not core_file.strip():
                continue
            core_id = retroarch_core_id_from_file_name(core_file)
            if not core_id:
                continue

            platforms = entry.get("platforms", [])
            if not isinstance(platforms, list):
                continue
            for platform in platforms:
                if not isinstance(platform, str):
                    continue
                system_key = normalize_retroarch_platform_key(platform)
                if not system_key:
                    continue
                known = compatibility.setdefault(system_key, [])
                if core_id not in known:
                    known.append(core_id)

        return compatibility

    for line in raw_content.splitlines():
        if not line.strip().startswith("|"):
            continue
        columns = [column.strip() for column in line.split("|")]
        if len(columns) < 4:
            continue

        core_cell = columns[1]
        system_cell = columns[2]
        if not core_cell or not system_cell:
            continue
        if core_cell.casefold() == "core" or system_cell.startswith(":") or system_cell == "-":
            continue

        core_id = retroarch_core_id_from_name(core_cell)
        system_key = normalize_retroarch_platform_key(system_cell)
        if not core_id or not system_key:
            continue

        known = compatibility.setdefault(system_key, [])
        if core_id not in known:
            known.append(core_id)

    return compatibility


def retroarch_system_keys_for_platform(platform: str, compatibility: dict[str, list[str]]) -> list[str]:
    normalized = normalize_retroarch_platform_key(platform)
    if not normalized or not compatibility:
        return []

    if normalized in compatibility:
        return [normalized]
    return []


def retroarch_cores_for_platform(platform: str, compatibility: dict[str, list[str]]) -> list[str]:
    if not compatibility:
        return ["fbneo", "mame2003_plus"]

    resolved_cores: list[str] = []
    for system_key in retroarch_system_keys_for_platform(platform, compatibility):
        for core in compatibility.get(system_key, []):
            if core not in resolved_cores:
                resolved_cores.append(core)

    if resolved_cores:
        return resolved_cores
    return []


def installed_retroarch_core_ids(emulator_path_text: str) -> set[str]:
    if not emulator_path_text:
        return set()

    emulator_path = Path(emulator_path_text).expanduser()
    if not emulator_path.exists() or not emulator_path.is_file():
        return set()

    cores_dir = emulator_path.parent / "cores"
    if not cores_dir.exists() or not cores_dir.is_dir():
        return set()

    installed_core_ids: set[str] = set()
    for candidate in cores_dir.glob("*.dll"):
        if not candidate.is_file():
            continue
        core_id = retroarch_core_id_from_file_name(candidate.name)
        if core_id:
            installed_core_ids.add(core_id)
    return installed_core_ids
