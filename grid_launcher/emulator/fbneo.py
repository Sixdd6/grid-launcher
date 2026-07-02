from __future__ import annotations

import os
from pathlib import Path
from typing import Callable


def _unique_paths(paths: list[Path]) -> list[Path]:
    unique: list[Path] = []
    seen: set[str] = set()
    for candidate in paths:
        key = str(candidate).casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        unique.append(candidate)
    return unique


def _clean_path_value(value: str) -> str:
    return value.strip().strip('"').strip("'")


def _resolve_setting_path(base_root: Path, raw_value: str, default_name: str) -> str:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_name

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute():
        candidate = base_root / candidate
    return str(candidate.resolve())


def _config_path_candidates(emulator_path_text: str) -> list[Path]:
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if not path_text:
        return []

    emulator_path = Path(path_text).expanduser()
    emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
    stem = emulator_path.stem.strip() if emulator_path.suffix else emulator_dir.name.strip()

    candidates: list[Path] = []
    if emulator_dir:
        if stem:
            candidates.append((emulator_dir / "config" / f"{stem}.ini").resolve())
        candidates.append((emulator_dir / "config" / "fbneo.ini").resolve())
        candidates.append((emulator_dir / "config" / "FinalBurn Neo.ini").resolve())

    return _unique_paths(candidates)


def _read_fbneo_config(config_path: Path) -> dict[str, str]:
    if not config_path.exists() or not config_path.is_file():
        return {}

    try:
        raw_content = config_path.read_text(encoding="utf-8")
    except OSError:
        return {}

    settings: dict[str, str] = {}
    for raw_line in raw_content.splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("//") or stripped.startswith("#") or stripped.startswith(";"):
            continue

        parts = stripped.split(None, 1)
        if not parts:
            continue

        key = parts[0].strip()
        value = parts[1].strip() if len(parts) > 1 else ""
        if key:
            settings[key] = value
    return settings


def fbneo_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    del launch_template, split_launch_template_args

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    emulator_path = Path(path_text).expanduser() if path_text else Path()
    emulator_dir = (emulator_path if emulator_path.is_dir() else emulator_path.parent) if path_text else Path.cwd()
    emulator_dir = emulator_dir.resolve()

    defaults = {
        "config_path": str((emulator_dir / "config" / "fbneo.ini").resolve()),
        "base_path": str(emulator_dir),
        "eeprom_path": str((emulator_dir / "config" / "games").resolve()),
        "memcard_path": str((emulator_dir / "config" / "memcards").resolve()),
        "hiscore_path": str((emulator_dir / "support" / "hiscores").resolve()),
        "hdd_path": str((emulator_dir / "support" / "hdd").resolve()),
        "state_path": str((emulator_dir / "savestates").resolve()),
    }

    config_candidates = _config_path_candidates(emulator_path_text)
    selected_config = next((candidate for candidate in config_candidates if candidate.exists() and candidate.is_file()), None)
    if selected_config is None and config_candidates:
        selected_config = config_candidates[0]

    settings = defaults.copy()
    if selected_config is not None:
        settings["config_path"] = str(selected_config.resolve())

    config_values = _read_fbneo_config(selected_config) if selected_config is not None else {}

    settings["eeprom_path"] = _resolve_setting_path(
        emulator_dir,
        config_values.get("szAppEEPROMPath", ""),
        "config/games",
    )
    settings["hiscore_path"] = _resolve_setting_path(
        emulator_dir,
        config_values.get("szAppHiscorePath", ""),
        "support/hiscores",
    )
    settings["hdd_path"] = _resolve_setting_path(
        emulator_dir,
        config_values.get("szAppHDDPath", ""),
        "support/hdd",
    )

    return settings


def fbneo_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = fbneo_directory_settings(emulator_path_text, launch_template, split_launch_template_args)

    ordered: list[str] = []
    seen: set[str] = set()
    for key in ("eeprom_path", "memcard_path", "hiscore_path", "hdd_path"):
        value = settings.get(key, "")
        if not isinstance(value, str) or not value.strip():
            continue
        normalized = str(Path(value).expanduser().resolve())
        casefolded = normalized.casefold()
        if casefolded in seen:
            continue
        seen.add(casefolded)
        ordered.append(normalized)
    return ordered


def fbneo_state_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = fbneo_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    value = settings.get("state_path", "")
    if not isinstance(value, str) or not value.strip():
        return []
    return [str(Path(value).expanduser().resolve())]
