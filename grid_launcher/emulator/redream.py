from __future__ import annotations

import os
import sys
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


def _emulator_dir(emulator_path_text: str) -> Path:
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if not path_text:
        return Path()

    emulator_path = Path(path_text).expanduser()
    return emulator_path if emulator_path.is_dir() else emulator_path.parent


def _portable_data_marker_exists(root: Path) -> bool:
    if not str(root):
        return False

    for file_name in ("redream.cfg", "flash.bin", "vmu0.bin", "vmu1.bin", "vmu2.bin", "vmu3.bin"):
        if (root / file_name).exists():
            return True

    for pattern in ("*.sav", "*.png"):
        if any(root.glob(pattern)):
            return True

    return False


def _default_user_root() -> Path:
    home_path = Path.home()
    if sys.platform == "darwin":
        return (home_path / "Library" / "Application Support" / "redream").resolve()

    xdg_data_home = os.environ.get("XDG_DATA_HOME", "").strip()
    if xdg_data_home:
        return (Path(xdg_data_home).expanduser() / "redream").resolve()

    return (home_path / ".local" / "share" / "redream").resolve()


def redream_data_root_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    del launch_template
    del split_launch_template_args

    emulator_dir = _emulator_dir(emulator_path_text)
    candidates: list[Path] = []

    if str(emulator_dir) and _portable_data_marker_exists(emulator_dir):
        candidates.append(emulator_dir.resolve())

    default_user_root = _default_user_root()
    if default_user_root.exists() or sys.platform == "darwin":
        candidates.append(default_user_root)

    if str(emulator_dir):
        candidates.append(emulator_dir.resolve())

    return _unique_paths([candidate.expanduser().resolve() for candidate in candidates if str(candidate)])


def redream_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    emulator_dir = _emulator_dir(emulator_path_text)
    data_roots = redream_data_root_candidates(emulator_path_text, launch_template, split_launch_template_args)

    defaults = {
        "config_path": "",
        "data_root": "",
        "portable": "false",
    }

    for data_root in data_roots:
        settings = defaults.copy()
        settings["data_root"] = str(data_root.resolve())
        settings["config_path"] = str((data_root / "redream.cfg").resolve())
        settings["portable"] = "true" if str(emulator_dir) and data_root.resolve() == emulator_dir.resolve() else "false"

        if (data_root / "redream.cfg").exists() or (data_root.exists() and data_root.is_dir()):
            return settings

        if not defaults["data_root"]:
            defaults = settings

    return defaults


def redream_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = redream_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    data_root_text = settings.get("data_root", "")
    if not isinstance(data_root_text, str) or not data_root_text.strip():
        return []

    data_root = Path(data_root_text).expanduser()
    if not data_root.exists() or not data_root.is_dir():
        return []

    vmu_paths = [
        data_root / f"vmu{index}.bin"
        for index in range(4)
        if (data_root / f"vmu{index}.bin").exists() and (data_root / f"vmu{index}.bin").is_file()
    ]
    return [str(path) for path in _unique_paths(vmu_paths)]


def redream_state_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = redream_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    data_root_text = settings.get("data_root", "")
    if not isinstance(data_root_text, str) or not data_root_text.strip():
        return []

    data_root = Path(data_root_text).expanduser()
    if not data_root.exists() or not data_root.is_dir():
        return []

    candidates: list[Path] = []
    states_root = data_root / "states"
    if states_root.exists() and states_root.is_dir():
        candidates.append(states_root.resolve())
    candidates.append(data_root.resolve())
    return [str(path) for path in _unique_paths(candidates)]


def ensure_redream_settings(emulator_path_text: str) -> dict:
    settings = redream_directory_settings(emulator_path_text, "", lambda s: [])
    config_path_text = settings.get("config_path", "")
    if not isinstance(config_path_text, str) or not config_path_text.strip():
        return {"config_path": None, "changed": False}

    config_path = Path(config_path_text)
    desired = {"mode": "fullscreen", "volume": "40"}

    try:
        existing_lines = config_path.read_text(encoding="utf-8").splitlines() if config_path.exists() else []
    except OSError:
        return {"config_path": None, "changed": False}

    parsed: dict[str, str] = {}
    for line in existing_lines:
        if "=" in line:
            key, _, value = line.partition("=")
            parsed[key.strip()] = value.strip()

    changed = False
    for key, value in desired.items():
        if parsed.get(key) != value:
            parsed[key] = value
            changed = True

    if not changed:
        return {"config_path": str(config_path), "changed": False}

    output_lines: list[str] = []
    written_keys: set[str] = set()
    for line in existing_lines:
        if "=" in line:
            key, _, _ = line.partition("=")
            key = key.strip()
            if key in desired:
                output_lines.append(f"{key}={desired[key]}")
                written_keys.add(key)
                continue
        output_lines.append(line)
    for key, value in desired.items():
        if key not in written_keys:
            output_lines.append(f"{key}={value}")

    try:
        config_path.parent.mkdir(parents=True, exist_ok=True)
        config_path.write_text("\n".join(output_lines) + "\n", encoding="utf-8")
    except OSError:
        return {"config_path": None, "changed": False}

    return {"config_path": str(config_path), "changed": True}
