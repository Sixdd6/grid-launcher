from __future__ import annotations

import os
import re
import xml.etree.ElementTree as ET
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


def cemu_settings_path_candidates(emulator_path_text: str) -> list[Path]:
    candidates: list[Path] = []

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            candidates.append(emulator_dir / "settings.xml")

    for env_name in ("APPDATA", "LOCALAPPDATA"):
        env_value = os.environ.get(env_name, "")
        if isinstance(env_value, str) and env_value.strip():
            candidates.append(Path(env_value).expanduser() / "Cemu" / "settings.xml")

    return _unique_paths(candidates)


def cemu_directory_settings(emulator_path_text: str) -> dict[str, str]:
    defaults = {
        "config_path": "",
        "mlc_path": "",
    }

    for candidate in cemu_settings_path_candidates(emulator_path_text):
        if not candidate.exists() or not candidate.is_file():
            continue

        try:
            root = ET.fromstring(candidate.read_text(encoding="utf-8"))
        except (ET.ParseError, OSError):
            continue

        candidate_nodes = [root, *root.findall(".//content")]
        for node in candidate_nodes:
            raw_mlc_path = node.findtext("mlc_path", default="")
            if isinstance(raw_mlc_path, str) and raw_mlc_path.strip():
                defaults["config_path"] = str(candidate)
                defaults["mlc_path"] = raw_mlc_path.strip()
                return defaults

    return defaults


def _save_root_from_mlc_path(raw_path: str) -> str:
    mlc_path = raw_path.strip().strip('"').strip("'")
    if not mlc_path:
        return ""

    normalized = re.sub(r"[\\/]+", "/", mlc_path).rstrip("/").casefold()
    if normalized.endswith("/usr/save"):
        return mlc_path.rstrip("\\/")
    return str(Path(mlc_path) / "usr" / "save")


def cemu_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    raw_mlc_paths: list[str] = []

    template = launch_template.strip() if isinstance(launch_template, str) else ""
    if template:
        try:
            args = split_launch_template_args(template)
        except ValueError:
            args = []

        for index, raw_arg in enumerate(args):
            if not isinstance(raw_arg, str) or not raw_arg.strip():
                continue
            normalized_arg = raw_arg.strip()
            lowered_arg = normalized_arg.casefold()

            if lowered_arg in {"-m", "--mlc"} and index + 1 < len(args):
                next_arg = args[index + 1]
                if isinstance(next_arg, str) and next_arg.strip():
                    raw_mlc_paths.append(next_arg.strip())
                continue

            if lowered_arg.startswith("--mlc=") or lowered_arg.startswith("-m="):
                _, value = normalized_arg.split("=", 1)
                if value.strip():
                    raw_mlc_paths.append(value.strip())

    settings = cemu_directory_settings(emulator_path_text)
    configured_mlc_path = settings.get("mlc_path", "")
    if isinstance(configured_mlc_path, str) and configured_mlc_path.strip():
        raw_mlc_paths.append(configured_mlc_path.strip())

    resolved: list[str] = []
    seen: set[str] = set()
    for raw_path in raw_mlc_paths:
        save_root = _save_root_from_mlc_path(raw_path)
        key = save_root.casefold()
        if not save_root or key in seen:
            continue
        seen.add(key)
        resolved.append(save_root)
    return resolved
