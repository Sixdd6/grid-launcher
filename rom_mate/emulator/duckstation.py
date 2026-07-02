from __future__ import annotations

import os
import re
from pathlib import Path

from rom_mate.core.path import xdg_config_home, xdg_data_home


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

    search_roots.append(xdg_data_home() / "duckstation")
    search_roots.append(xdg_config_home() / "duckstation")

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


def _duckstation_section_has_key(raw_content: str, section_name: str, key_name: str) -> bool:
    target_section = section_name.casefold()
    target_key = key_name.casefold()
    in_target = False

    for raw_line in raw_content.splitlines():
        stripped = raw_line.strip()
        section_match = re.match(r"^\[(.+?)\]\s*$", stripped)
        if section_match:
            in_target = section_match.group(1).strip().casefold() == target_section
            continue
        if not in_target:
            continue

        key_match = re.match(r"^\s*([A-Za-z0-9_]+)\s*=", raw_line)
        if key_match and key_match.group(1).casefold() == target_key:
            return True

    return False


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
) -> dict[str, object]:
    _emulator_dir: Path | None = None
    if isinstance(emulator_path_text, str) and emulator_path_text.strip():
        _emulator_path = Path(emulator_path_text.strip()).expanduser()
        _emulator_dir = _emulator_path if _emulator_path.is_dir() else _emulator_path.parent
        _portable_txt = _emulator_dir / "portable.txt"
        if not _portable_txt.exists():
            try:
                _portable_txt.write_text("", encoding="utf-8")
            except OSError:
                pass

    settings = duckstation_memory_card_settings(emulator_path_text)
    config_candidates = duckstation_config_path_candidates(emulator_path_text)

    result = dict(settings)
    if not config_candidates:
        result["changed"] = False
        return result

    if _emulator_dir is not None:
        config_path = _emulator_dir / "settings.ini"
    else:
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

    updated_content, section_changed = _ensure_duckstation_section_values(
        updated_content,
        "Main",
        {
            "InhibitScreensaver": "true",
            "SetupWizardIncomplete": "false",
            **({} if _duckstation_section_has_key(raw_content, "Main", "ConfirmPowerOff") else {"ConfirmPowerOff": "false"}),
        },
    )
    changed = changed or section_changed

    updated_content, section_changed = _ensure_duckstation_section_values(
        updated_content,
        "Display",
        {
            "FullscreenMode": "Borderless Windowed",
            **({} if _duckstation_section_has_key(raw_content, "Display", "Scaling") else {"Scaling": "Lanczos"}),
            **({} if _duckstation_section_has_key(raw_content, "Display", "Scaling24Bit") else {"Scaling24Bit": "Lanczos"}),
        },
    )
    changed = changed or section_changed

    # Auto-update disabled (always force)
    updated_content, section_changed = _ensure_duckstation_section_values(
        updated_content,
        "AutoUpdater",
        {"CheckAtStartup": "false"},
    )
    changed = changed or section_changed

    # GPU settings (only if not already set by user)
    gpu_defaults = {}
    for key, value in (
        ("ResolutionScale", "4"),
        ("PGXPEnable", "true"),
        ("PGXPColorCorrection", "true"),
        ("TextureFilter", "Scale2x"),
        ("SpriteTextureFilter", "Scale2x"),
        ("DitheringMode", "TrueColorFull"),
        ("LineDetectMode", "BasicTriangles"),
        ("DownsampleMode", "Box"),
        ("DownsampleScale", "2"),
    ):
        if not _duckstation_section_has_key(raw_content, "GPU", key):
            gpu_defaults[key] = value
    if gpu_defaults:
        updated_content, section_changed = _ensure_duckstation_section_values(
            updated_content, "GPU", gpu_defaults,
        )
        changed = changed or section_changed

    # Audio volume 60% (only if not already set by user)
    updated_content, section_changed = _ensure_duckstation_section_values(
        updated_content,
        "Audio",
        {**({} if _duckstation_section_has_key(raw_content, "Audio", "OutputVolume") else {"OutputVolume": "60"})},
    )
    changed = changed or section_changed

    # Pause menu hotkey on guide button (only if not already set by user)
    updated_content, section_changed = _ensure_duckstation_section_values(
        updated_content,
        "Hotkeys",
        {**({} if _duckstation_section_has_key(raw_content, "Hotkeys", "OpenPauseMenu") else {"OpenPauseMenu": "SDL-0/Guide"})},
    )
    changed = changed or section_changed

    # Default SDL controller mapping for Pad1 (only if not already configured)
    if not _duckstation_section_has_key(raw_content, "Pad1", "Type"):
        pad1_values = {
            "Type": "AnalogController",
            "Up": "SDL-0/DPadUp",
            "Down": "SDL-0/DPadDown",
            "Left": "SDL-0/DPadLeft",
            "Right": "SDL-0/DPadRight",
            "Triangle": "SDL-0/Y",
            "Circle": "SDL-0/B",
            "Cross": "SDL-0/A",
            "Square": "SDL-0/X",
            "L1": "SDL-0/LeftShoulder",
            "R1": "SDL-0/RightShoulder",
            "L2": "SDL-0/+LeftTrigger",
            "R2": "SDL-0/+RightTrigger",
            "L3": "SDL-0/LeftStick",
            "R3": "SDL-0/RightStick",
            "Select": "SDL-0/Back",
            "Start": "SDL-0/Start",
            "LLeft": "SDL-0/-LeftX",
            "LRight": "SDL-0/+LeftX",
            "LUp": "SDL-0/-LeftY",
            "LDown": "SDL-0/+LeftY",
            "RLeft": "SDL-0/-RightX",
            "RRight": "SDL-0/+RightX",
            "RUp": "SDL-0/-RightY",
            "RDown": "SDL-0/+RightY",
            "LargeMotor": "SDL-0/LargeMotor",
            "SmallMotor": "SDL-0/SmallMotor",
        }
        updated_content, section_changed = _ensure_duckstation_section_values(
            updated_content, "Pad1", pad1_values,
        )
        changed = changed or section_changed

    if enable_fullscreen:
        updated_content, section_changed = _ensure_duckstation_section_values(
            updated_content,
            "Main",
            {"StartFullscreen": "true"},
        )
        changed = changed or section_changed

    updated_content, section_changed = _ensure_duckstation_section_values(
        updated_content,
        "Cheevos",
        {
            "Enabled": "true",
            "ChallengeMode": "false",
            "LeaderboardNotifications": "false",
            "LeaderboardTrackers": "false",
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
