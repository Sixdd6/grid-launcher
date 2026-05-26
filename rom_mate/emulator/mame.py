from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Callable

from rom_mate.core.path import xdg_config_home


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


def _split_launch_args(
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> list[str]:
    template = launch_template.strip() if isinstance(launch_template, str) else ""
    if not template or not callable(split_launch_template_args):
        return []

    try:
        return split_launch_template_args(template)
    except ValueError:
        return []


def _consume_arg_value(args: list[str], start_index: int) -> tuple[str, int]:
    if start_index >= len(args):
        return "", start_index

    token = args[start_index].strip()
    if not token:
        return "", start_index

    quote = token[0] if token[0] in {'"', "'"} else ""
    if quote and (len(token) == 1 or not token.endswith(quote)):
        parts = [token]
        index = start_index + 1
        while index < len(args):
            parts.append(args[index])
            if args[index].strip().endswith(quote):
                break
            index += 1
        token = " ".join(parts)
        return token.strip().strip('"').strip("'"), index

    return token.strip().strip('"').strip("'"), start_index


def _resolve_setting_path(base_root: Path, raw_value: str, default_name: str) -> str:
    value = raw_value.strip().strip('"').strip("'") if isinstance(raw_value, str) else ""
    if not value:
        value = default_name

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute():
        candidate = base_root / candidate
    return str(candidate.resolve())


def _launch_path_overrides(
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> dict[str, str]:
    args = _split_launch_args(launch_template, split_launch_template_args)
    if not args:
        return {}

    overrides: dict[str, str] = {}
    supported_options = {
        "inipath",
        "cfg_directory",
        "nvram_directory",
        "state_directory",
        "diff_directory",
        "memcard_directory",
    }

    index = 0
    while index < len(args):
        raw_arg = args[index]
        index += 1
        if not isinstance(raw_arg, str) or not raw_arg.strip():
            continue

        normalized_arg = raw_arg.strip()
        if not normalized_arg.startswith("-"):
            continue

        option_text = normalized_arg.lstrip("-")
        value = ""
        if "=" in option_text:
            option_text, _, value = option_text.partition("=")
            value = value.strip().strip('"').strip("'")
        elif index < len(args):
            next_token = args[index].strip() if isinstance(args[index], str) else ""
            if next_token and not next_token.startswith("-"):
                value, consumed_index = _consume_arg_value(args, index)
                index = consumed_index + 1

        option_name = option_text.replace("-", "_").casefold()
        if option_name in supported_options and value:
            overrides[option_name] = value

    return overrides


def _ini_path_candidates(
    base_root: Path,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> list[Path]:
    overrides = _launch_path_overrides(launch_template, split_launch_template_args)
    raw_inipath = overrides.get("inipath", "")

    directories: list[Path] = []
    if raw_inipath:
        for part in raw_inipath.split(";"):
            trimmed = part.strip()
            if not trimmed:
                continue
            candidate = Path(os.path.expandvars(trimmed)).expanduser()
            if not candidate.is_absolute():
                candidate = base_root / candidate
            directories.append(candidate.resolve())
    else:
        directories.extend(
            [
                base_root.resolve(),
                (base_root / "ini").resolve(),
                (base_root / "ini" / "presets").resolve(),
            ]
        )
        if sys.platform != "win32":
            directories.append((Path.home() / ".mame").resolve())
            xdg_config = xdg_config_home()
            if xdg_config is not None:
                directories.append((xdg_config / "mame").resolve())

    candidates = [directory / "mame.ini" for directory in directories]
    return _unique_paths(candidates)


def _read_mame_ini_settings(ini_path: Path) -> dict[str, str]:
    if not ini_path.exists() or not ini_path.is_file():
        return {}

    try:
        raw_content = ini_path.read_text(encoding="utf-8")
    except OSError:
        return {}

    settings: dict[str, str] = {}
    for raw_line in raw_content.splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("#") or stripped.startswith(";"):
            continue

        parts = stripped.split(None, 1)
        if not parts:
            continue

        key = parts[0].strip().casefold()
        value = parts[1].strip() if len(parts) > 1 else ""
        if key:
            settings[key] = value
    return settings


def mame_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    emulator_path = Path(path_text).expanduser() if path_text else Path()
    base_root = (emulator_path if emulator_path.is_dir() else emulator_path.parent) if path_text else Path.cwd()
    base_root = base_root.resolve()

    defaults = {
        "ini_path": str((base_root / "mame.ini").resolve()),
        "base_path": str(base_root),
        "cfg_directory": str((base_root / "cfg").resolve()),
        "nvram_directory": str((base_root / "nvram").resolve()),
        "memcard_directory": str((base_root / "memcard").resolve()),
        "diff_directory": str((base_root / "diff").resolve()),
        "state_directory": str((base_root / "sta").resolve()),
    }

    ini_candidates = _ini_path_candidates(base_root, launch_template, split_launch_template_args)
    selected_ini = next((candidate for candidate in ini_candidates if candidate.exists() and candidate.is_file()), None)
    if selected_ini is None and ini_candidates:
        selected_ini = ini_candidates[0]

    ini_settings = _read_mame_ini_settings(selected_ini) if selected_ini is not None else {}
    launch_overrides = _launch_path_overrides(launch_template, split_launch_template_args)

    settings = defaults.copy()
    if selected_ini is not None:
        settings["ini_path"] = str(selected_ini.resolve())

    for option_name, default_name in {
        "cfg_directory": "cfg",
        "nvram_directory": "nvram",
        "memcard_directory": "memcard",
        "diff_directory": "diff",
        "state_directory": "sta",
    }.items():
        raw_value = launch_overrides.get(option_name, ini_settings.get(option_name, ""))
        settings[option_name] = _resolve_setting_path(base_root, raw_value, default_name)

    return settings


def mame_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = mame_directory_settings(emulator_path_text, launch_template, split_launch_template_args)

    ordered: list[str] = []
    seen: set[str] = set()
    for key in ("nvram_directory", "memcard_directory", "diff_directory"):
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


def mame_state_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = mame_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    value = settings.get("state_directory", "")
    if not isinstance(value, str) or not value.strip():
        return []
    return [str(Path(value).expanduser().resolve())]
