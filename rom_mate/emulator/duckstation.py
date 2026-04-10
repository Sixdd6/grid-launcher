from __future__ import annotations

import os
import re
from pathlib import Path


def duckstation_config_path_candidates(emulator_path_text: str) -> list[Path]:
    if not emulator_path_text:
        return []

    emulator_path = Path(emulator_path_text).expanduser()
    search_roots: list[Path] = []
    if emulator_path.is_file() or emulator_path.suffix:
        search_roots.append(emulator_path.parent)
    else:
        search_roots.append(emulator_path)

    local_app_data = os.environ.get("LOCALAPPDATA", "").strip()
    if local_app_data:
        search_roots.append(Path(local_app_data) / "DuckStation")

    user_profile = Path.home()
    search_roots.extend(
        [
            user_profile / "Documents" / "DuckStation",
            user_profile / ".local" / "share" / "duckstation",
            user_profile / ".config" / "duckstation",
            user_profile / "Library" / "Application Support" / "DuckStation",
        ]
    )

    xdg_data_home = os.environ.get("XDG_DATA_HOME", "").strip()
    if xdg_data_home:
        search_roots.append(Path(xdg_data_home) / "duckstation")

    xdg_config_home = os.environ.get("XDG_CONFIG_HOME", "").strip()
    if xdg_config_home:
        search_roots.append(Path(xdg_config_home) / "duckstation")

    candidates: list[Path] = []
    seen: set[str] = set()
    for root in search_roots:
        candidate = root / "settings.ini"
        key_value = str(candidate).casefold()
        if key_value in seen:
            continue
        seen.add(key_value)
        candidates.append(candidate)
    return candidates


def _duckstation_config_bool(value: str) -> bool:
    return value.strip().casefold() in {"1", "true", "yes", "on"}


def _ensure_duckstation_section_values(
    raw_content: str,
    section_name: str,
    desired_values: dict[str, str],
) -> tuple[str, bool]:
    if not desired_values:
        return raw_content, False

    lines = raw_content.splitlines()
    output_lines: list[str] = []
    changed = False
    target_key = section_name.casefold()
    in_target = False
    section_found = False
    seen_keys: set[str] = set()

    def flush_missing_keys() -> None:
        nonlocal changed
        for key, value in desired_values.items():
            if key in seen_keys:
                continue
            output_lines.append(f"{key} = {value}")
            seen_keys.add(key)
            changed = True

    for raw_line in lines:
        stripped = raw_line.strip()
        section_match = re.match(r"^\[(.+?)\]\s*$", stripped)
        if section_match:
            if in_target:
                flush_missing_keys()
            current_section = section_match.group(1).strip()
            in_target = current_section.casefold() == target_key
            if in_target:
                section_found = True
            output_lines.append(raw_line)
            continue

        if in_target:
            key_match = re.match(r"^\s*([A-Za-z0-9_]+)\s*=", raw_line)
            if key_match:
                key = key_match.group(1)
                if key in desired_values:
                    if key in seen_keys:
                        changed = True
                        continue
                    replacement = f"{key} = {desired_values[key]}"
                    if raw_line.strip() != replacement:
                        changed = True
                    output_lines.append(replacement)
                    seen_keys.add(key)
                    continue

        output_lines.append(raw_line)

    if in_target:
        flush_missing_keys()

    if not section_found:
        if output_lines and output_lines[-1].strip():
            output_lines.append("")
        output_lines.append(f"[{section_name}]")
        for key, value in desired_values.items():
            output_lines.append(f"{key} = {value}")
        changed = True

    return "\n".join(output_lines).rstrip() + "\n", changed


def duckstation_memory_card_settings(emulator_path_text: str) -> dict[str, object]:
    defaults: dict[str, object] = {
        "config_path": "",
        "directory": "memcards",
        "card1_type": "PerGameTitle",
        "card2_type": "None",
        "use_playlist_title": True,
    }

    for candidate in duckstation_config_path_candidates(emulator_path_text):
        if not candidate.exists() or not candidate.is_file():
            continue

        try:
            raw_content = candidate.read_text(encoding="utf-8")
        except OSError:
            continue

        current_section = ""
        parsed_any = False
        for raw_line in raw_content.splitlines():
            stripped = raw_line.strip()
            if not stripped or stripped.startswith(("#", ";")):
                continue

            section_match = re.match(r"^\[(.+?)\]\s*$", stripped)
            if section_match:
                current_section = section_match.group(1).strip().casefold()
                continue

            if current_section != "memorycards" or "=" not in raw_line:
                continue

            key, value = raw_line.split("=", 1)
            key = key.strip()
            value = value.strip()
            parsed_any = True

            if key == "Directory" and value:
                defaults["directory"] = value
            elif key == "Card1Type" and value:
                defaults["card1_type"] = value
            elif key == "Card2Type" and value:
                defaults["card2_type"] = value
            elif key == "UsePlaylistTitle" and value:
                defaults["use_playlist_title"] = _duckstation_config_bool(value)

        if parsed_any:
            defaults["config_path"] = str(candidate)
            break

    return defaults


def ensure_duckstation_memory_card_settings(
    emulator_path_text: str,
    *,
    enable_fullscreen: bool = False,
    retroachievements_username: str = "",
    retroachievements_token: str = "",
) -> dict[str, object]:
    settings = duckstation_memory_card_settings(emulator_path_text)
    config_candidates = duckstation_config_path_candidates(emulator_path_text)

    result = dict(settings)
    if not config_candidates:
        result["changed"] = False
        return result

    configured_path = settings.get("config_path", "")
    if isinstance(configured_path, str) and configured_path.strip():
        config_path = Path(configured_path.strip()).expanduser()
    else:
        config_path = config_candidates[0]

    current_card1_type = str(settings.get("card1_type", "")).strip()
    current_card2_type = str(settings.get("card2_type", "")).strip()
    per_game_types = {"PerGame", "PerGameTitle", "PerGameFileTitle"}

    desired_values = {
        "Directory": str(settings.get("directory", "")).strip() or "memcards",
        "Card1Type": current_card1_type if current_card1_type in per_game_types else "PerGameTitle",
        "Card2Type": current_card2_type if current_card2_type in (*per_game_types, "None") else "None",
        "UsePlaylistTitle": "true",
    }

    created = not config_path.exists()
    try:
        raw_content = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    except OSError:
        raw_content = ""

    changed = created
    updated_content, section_changed = _ensure_duckstation_section_values(raw_content, "MemoryCards", desired_values)
    changed = changed or section_changed

    if enable_fullscreen:
        updated_content, section_changed = _ensure_duckstation_section_values(
            updated_content,
            "Main",
            {"StartFullscreen": "true"},
        )
        changed = changed or section_changed

    username = retroachievements_username.strip() if isinstance(retroachievements_username, str) else ""
    token = retroachievements_token.strip() if isinstance(retroachievements_token, str) else ""
    if username and token:
        updated_content, section_changed = _ensure_duckstation_section_values(
            updated_content,
            "Achievements",
            {
                "Enabled": "true",
                "Username": username,
                "Token": token,
            },
        )
        changed = changed or section_changed

    if changed:
        try:
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(updated_content, encoding="utf-8")
        except OSError:
            result["changed"] = False
            return result

    updated = duckstation_memory_card_settings(emulator_path_text)
    updated["config_path"] = str(config_path)
    updated["changed"] = changed
    return updated
