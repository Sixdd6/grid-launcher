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


def retroarch_config_path_candidates(emulator_path_text: str) -> list[Path]:
    if not emulator_path_text:
        return []

    emulator_path = Path(emulator_path_text).expanduser()
    search_roots: list[Path] = []
    if emulator_path.is_file() or emulator_path.suffix:
        search_roots.append(emulator_path.parent)
    else:
        search_roots.append(emulator_path)

    candidates: list[Path] = []
    seen: set[str] = set()
    for root in search_roots:
        for candidate in (root / "retroarch.cfg", root / "config" / "retroarch.cfg"):
            key_value = str(candidate).casefold()
            if key_value in seen:
                continue
            seen.add(key_value)
            candidates.append(candidate)
    return candidates


def _retroarch_config_bool(value: str) -> bool:
    normalized = value.strip().casefold()
    return normalized in {"1", "true", "yes", "on"}


def retroarch_directory_settings(emulator_path_text: str) -> dict[str, object]:
    defaults: dict[str, object] = {
        "config_path": "",
        "savefile_directory": "",
        "savestate_directory": "",
        "savefiles_in_content_dir": False,
        "savestates_in_content_dir": False,
        "sort_savefiles_enable": False,
        "sort_savestates_enable": False,
        "sort_savefiles_by_content_enable": False,
        "sort_savestates_by_content_enable": False,
    }

    for candidate in retroarch_config_path_candidates(emulator_path_text):
        if not candidate.exists() or not candidate.is_file():
            continue

        parsed: dict[str, str] = {}
        try:
            raw_content = candidate.read_text(encoding="utf-8")
        except OSError:
            continue

        for raw_line in raw_content.splitlines():
            line = raw_line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            key = key.strip()
            value = value.strip()
            if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
                value = value[1:-1]
            parsed[key] = value

        if not parsed:
            continue

        defaults["config_path"] = str(candidate)
        for directory_key in ("savefile_directory", "savestate_directory"):
            raw_value = parsed.get(directory_key, "")
            if isinstance(raw_value, str) and raw_value.strip() and raw_value.strip().casefold() != "default":
                defaults[directory_key] = raw_value.strip()

        for bool_key in (
            "savefiles_in_content_dir",
            "savestates_in_content_dir",
            "sort_savefiles_enable",
            "sort_savestates_enable",
            "sort_savefiles_by_content_enable",
            "sort_savestates_by_content_enable",
        ):
            raw_value = parsed.get(bool_key, "")
            if isinstance(raw_value, str) and raw_value.strip():
                defaults[bool_key] = _retroarch_config_bool(raw_value)
        break

    return defaults


def ensure_retroarch_save_location_settings(
    emulator_path_text: str,
    *,
    enable_fullscreen: bool = False,
    username: str = "",
    retroachievements_username: str = "",
    retroachievements_token: str = "",
) -> dict[str, object]:
    settings = retroarch_directory_settings(emulator_path_text)
    config_candidates = retroarch_config_path_candidates(emulator_path_text)

    result = dict(settings)
    if not config_candidates:
        result["changed"] = False
        return result

    configured_path = settings.get("config_path", "")
    if isinstance(configured_path, str) and configured_path.strip():
        config_path = Path(configured_path.strip()).expanduser()
    else:
        config_path = config_candidates[0]

    desired_values = {
        "savefile_directory": (
            str(settings.get("savefile_directory", "")).strip() or "saves"
        ),
        "savestate_directory": (
            str(settings.get("savestate_directory", "")).strip() or "states"
        ),
        "video_windowed_fullscreen": "true",
        "audio_volume": "-18.000000",
        "discord_enable": "false",
        "pause_nonactive": "true",
        "video_vsync": "true",
        "input_menu_toggle_gamepad_combo": "2",
        "savestate_auto_save": "false",
        "savestate_auto_load": "false",
        "rgui_show_start_screen": "false",
        "menu_show_core_updater": "false",
        "sort_savefiles_enable": "false",
        "sort_savestates_enable": "false",
        "sort_savefiles_by_content_enable": "false",
        "sort_savestates_by_content_enable": "false",
        "savefiles_in_content_dir": "false",
        "savestates_in_content_dir": "false",
        "cheevos_hardcore_mode_enable": "false",
        "cheevos_visibility_lboard_start": "false",
        "cheevos_visibility_lboard_submit": "false",
        "cheevos_visibility_lboard_trackers": "false",
    }

    nick = username.strip() if isinstance(username, str) else ""
    if nick:
        desired_values["netplay_nickname"] = nick

    if enable_fullscreen:
        desired_values["video_fullscreen"] = "true"

    username = retroachievements_username.strip() if isinstance(retroachievements_username, str) else ""
    token = retroachievements_token.strip() if isinstance(retroachievements_token, str) else ""
    if username and token:
        desired_values["cheevos_enable"] = "true"
        desired_values["cheevos_username"] = username
        desired_values["cheevos_token"] = token

    created = not config_path.exists()
    try:
        raw_content = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    except OSError:
        raw_content = ""

    output_lines: list[str] = []
    seen_keys: set[str] = set()
    changed = created

    for raw_line in raw_content.splitlines():
        match = re.match(r"^\s*([A-Za-z0-9_]+)\s*=", raw_line)
        if not match:
            output_lines.append(raw_line)
            continue

        key = match.group(1)
        if key not in desired_values:
            output_lines.append(raw_line)
            continue
        if key in seen_keys:
            changed = True
            continue

        # Preserve explicit user volume preferences when already configured.
        if key == "audio_volume":
            output_lines.append(raw_line)
            seen_keys.add(key)
            continue

        replacement = f'{key} = "{desired_values[key]}"'
        if raw_line.strip() != replacement:
            changed = True
        output_lines.append(replacement)
        seen_keys.add(key)

    for key, value in desired_values.items():
        if key in seen_keys:
            continue
        output_lines.append(f'{key} = "{value}"')
        seen_keys.add(key)
        changed = True

    if changed:
        try:
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("\n".join(output_lines).rstrip() + "\n", encoding="utf-8")
        except OSError:
            result["changed"] = False
            return result

    updated = retroarch_directory_settings(emulator_path_text)
    updated["config_path"] = str(config_path)
    updated["changed"] = changed
    return updated


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


def retroarch_core_firmware_metadata(core_id: str, entries: list) -> dict | None:
    """Return the firmware metadata dict for the given core_id, or None."""
    if not core_id or not entries:
        return None
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        core_file = entry.get("core_file", "")
        if not isinstance(core_file, str) or not core_file.strip():
            continue
        if retroarch_core_id_from_file_name(core_file) == core_id:
            firmware = entry.get("firmware")
            if isinstance(firmware, dict):
                return firmware
            return None
    return None
