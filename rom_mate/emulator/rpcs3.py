from __future__ import annotations

import configparser
import os
import re
from pathlib import Path
from typing import Callable

_USER_ID_PATTERN = re.compile(r"^\d{8}$")


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


def _is_valid_user_id(value: str) -> bool:
    return bool(_USER_ID_PATTERN.fullmatch(value)) and value != "00000000"


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


def _launch_user_id(
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> str:
    args = _split_launch_args(launch_template, split_launch_template_args)
    for index, raw_arg in enumerate(args):
        if not isinstance(raw_arg, str) or not raw_arg.strip():
            continue

        normalized_arg = raw_arg.strip()
        lowered_arg = normalized_arg.casefold()

        if lowered_arg == "--user-id" and index + 1 < len(args):
            next_arg = args[index + 1]
            if isinstance(next_arg, str):
                candidate = next_arg.strip()
                if _is_valid_user_id(candidate):
                    return candidate
            continue

        if lowered_arg.startswith("--user-id="):
            _, candidate = normalized_arg.split("=", 1)
            candidate = candidate.strip()
            if _is_valid_user_id(candidate):
                return candidate

    return ""


def _clean_path_value(value: str) -> str:
    cleaned = value.strip()
    if cleaned and cleaned[0] not in {'"', "'"}:
        cleaned = re.split(r"\s+#", cleaned, maxsplit=1)[0].strip()
    return cleaned.strip().strip('"').strip("'")


def _yaml_scalar_value(raw_content: str, key: str) -> str:
    pattern = re.compile(rf'^\s*["\']?{re.escape(key)}["\']?\s*:\s*(.+?)\s*$', re.IGNORECASE | re.MULTILINE)
    match = pattern.search(raw_content)
    if not match:
        return ""

    raw_value = match.group(1).strip()
    if raw_value in {"", "{}", "[]", "|", ">"}:
        return ""
    return _clean_path_value(raw_value)


def _resolve_rpcs3_path(base_root: Path, raw_value: str, default_value: str = "") -> Path:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_value

    if not value:
        return base_root.resolve()

    emulator_dir_value = str(base_root.resolve()).replace("\\", "/")
    if not emulator_dir_value.endswith("/"):
        emulator_dir_value += "/"

    expanded = os.path.expandvars(value.replace("$(EmulatorDir)", emulator_dir_value))
    candidate = Path(expanded).expanduser()
    if not candidate.is_absolute():
        candidate = base_root / candidate
    return candidate.resolve()


def rpcs3_data_root_candidates(emulator_path_text: str) -> list[Path]:
    candidates: list[Path] = []

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            portable_dir = emulator_dir / "portable"
            if portable_dir.exists() and portable_dir.is_dir():
                candidates.append(portable_dir.resolve())
            candidates.append(emulator_dir.resolve())

    config_env = os.environ.get("RPCS3_CONFIG_DIR", "")
    if isinstance(config_env, str) and config_env.strip():
        candidates.insert(1 if candidates else 0, Path(config_env).expanduser().resolve())

    home_path = Path.home()
    candidates.extend(
        [
            home_path / ".config" / "rpcs3",
            home_path / "Library" / "Application Support" / "rpcs3",
        ]
    )

    return _unique_paths(candidates)


def _vfs_path_candidates_for_root(data_root: Path) -> list[Path]:
    return _unique_paths([data_root / "config" / "vfs.yml", data_root / "vfs.yml"])


def _persistent_settings_path_candidates_for_root(data_root: Path) -> list[Path]:
    return _unique_paths([data_root / "GuiConfigs" / "persistent_settings.dat"])


def _persistent_active_user(data_root: Path) -> tuple[str, str]:
    for candidate in _persistent_settings_path_candidates_for_root(data_root):
        if not candidate.exists() or not candidate.is_file():
            continue

        parser = configparser.ConfigParser()
        try:
            parser.read(candidate, encoding="utf-8")
        except (configparser.Error, OSError, UnicodeDecodeError):
            continue

        active_user = parser.get("Users", "active_user", fallback="").strip()
        if _is_valid_user_id(active_user):
            return active_user, str(candidate)
        return "", str(candidate)

    return "", ""


def rpcs3_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    launch_user = _launch_user_id(launch_template, split_launch_template_args)
    defaults = {
        "config_path": "",
        "persistent_settings_path": "",
        "data_root": "",
        "dev_hdd0": "",
        "current_user": launch_user or "00000001",
    }

    for data_root in rpcs3_data_root_candidates(emulator_path_text):
        settings = defaults.copy()
        settings["data_root"] = str(data_root)

        persistent_user, persistent_settings_path = _persistent_active_user(data_root)
        if persistent_settings_path:
            settings["persistent_settings_path"] = persistent_settings_path
        if not launch_user and persistent_user:
            settings["current_user"] = persistent_user

        emulator_root = data_root.resolve()
        dev_hdd0_root = emulator_root / "dev_hdd0"

        for candidate in _vfs_path_candidates_for_root(data_root):
            if not candidate.exists() or not candidate.is_file():
                continue

            try:
                raw_content = candidate.read_text(encoding="utf-8")
            except OSError:
                continue

            settings["config_path"] = str(candidate)
            raw_emulator_root = _yaml_scalar_value(raw_content, "$(EmulatorDir)")
            emulator_root = _resolve_rpcs3_path(data_root, raw_emulator_root)
            raw_dev_hdd0 = _yaml_scalar_value(raw_content, "/dev_hdd0/")
            dev_hdd0_root = _resolve_rpcs3_path(emulator_root, raw_dev_hdd0, "$(EmulatorDir)dev_hdd0/")
            break

        settings["dev_hdd0"] = str(dev_hdd0_root)
        return settings

    return defaults


def rpcs3_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = rpcs3_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    dev_hdd0_path = settings.get("dev_hdd0", "")
    if not isinstance(dev_hdd0_path, str) or not dev_hdd0_path.strip():
        return []

    home_root = Path(dev_hdd0_path.strip()).expanduser() / "home"
    current_user = settings.get("current_user", "")

    raw_paths: list[Path] = []
    if isinstance(current_user, str) and _is_valid_user_id(current_user):
        raw_paths.append(home_root / current_user / "savedata")

    if home_root.exists() and home_root.is_dir():
        for child in sorted(home_root.iterdir(), key=lambda item: item.name):
            if not child.is_dir() or not _is_valid_user_id(child.name):
                continue
            raw_paths.append(child / "savedata")

    raw_paths.append(home_root / "00000001" / "savedata")

    unique: list[str] = []
    seen: set[str] = set()
    for candidate in raw_paths:
        resolved = candidate.resolve()
        key = str(resolved).casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        unique.append(str(resolved))
    return unique
