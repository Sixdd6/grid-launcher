from __future__ import annotations

import configparser
import os
import sys
from pathlib import Path
from typing import Callable

_SDMC_TITLE_GROUPS = ("00040000", "00040002", "0004000e", "0004008c", "00048004")
_NAND_TITLE_GROUPS = ("00040010", "00040030")
_ZERO_ID = "0" * 32


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


def _bool_value(value: str, *, default: bool = False) -> bool:
    if not isinstance(value, str):
        return default
    normalized = value.strip().casefold()
    if not normalized:
        return default
    if normalized in {"1", "true", "yes", "on"}:
        return True
    if normalized in {"0", "false", "no", "off"}:
        return False
    return default


def _resolve_setting_path(root: Path, raw_value: str, default_value: str) -> str:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_value

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute():
        candidate = root / candidate
    return str(candidate.resolve())


def azahar_user_root_candidates(
    emulator_path_text: str,
    launch_template: str = "",
    split_launch_template_args: Callable[[str], list[str]] | None = None,
) -> list[Path]:
    del launch_template, split_launch_template_args

    candidates: list[Path] = []
    emulator_dir = Path()

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            portable_root = (emulator_dir / "user").resolve()
            if portable_root.exists() and portable_root.is_dir():
                candidates.append(portable_root)

    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA", "")
        if isinstance(appdata, str) and appdata.strip():
            candidates.append((Path(appdata).expanduser() / "Azahar").resolve())
    else:
        xdg_data_home = os.environ.get("XDG_DATA_HOME", "")
        if isinstance(xdg_data_home, str) and xdg_data_home.strip():
            candidates.append((Path(xdg_data_home).expanduser() / "Azahar").resolve())

        home_path = Path.home()
        candidates.extend(
            [
                home_path / ".local" / "share" / "Azahar",
                home_path / "Library" / "Application Support" / "Azahar",
            ]
        )

    if emulator_dir:
        candidates.append((emulator_dir / "user").resolve())

    return _unique_paths([candidate.expanduser().resolve() for candidate in candidates if str(candidate)])


def azahar_settings_path_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    return _unique_paths(
        [root / "config" / "qt-config.ini" for root in azahar_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)]
    )


def azahar_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    user_roots = azahar_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)
    settings_candidates = azahar_settings_path_candidates(emulator_path_text, launch_template, split_launch_template_args)

    defaults = {
        "config_path": "",
        "user_root": "",
        "nand_root": "",
        "sdmc_root": "",
        "states_root": "",
        "use_custom_storage": "false",
        "use_virtual_sd": "true",
    }

    first_existing_root: Path | None = None

    for root, candidate in zip(user_roots, settings_candidates, strict=False):
        if first_existing_root is None and root.exists() and root.is_dir():
            first_existing_root = root.resolve()

        if not candidate.exists() or not candidate.is_file():
            continue

        parser = configparser.ConfigParser(interpolation=None, strict=False)
        try:
            parser.read(candidate, encoding="utf-8")
        except (configparser.Error, OSError, UnicodeDecodeError):
            continue

        storage = parser["Data Storage"] if parser.has_section("Data Storage") else {}
        use_custom_storage = _bool_value(storage.get("use_custom_storage", "false"))
        use_virtual_sd = _bool_value(storage.get("use_virtual_sd", "true"), default=True)

        defaults["config_path"] = str(candidate.resolve())
        defaults["user_root"] = str(root.resolve())
        defaults["states_root"] = str((root / "states").resolve())
        defaults["use_custom_storage"] = "true" if use_custom_storage else "false"
        defaults["use_virtual_sd"] = "true" if use_virtual_sd else "false"

        if use_custom_storage:
            defaults["nand_root"] = _resolve_setting_path(root, storage.get("nand_directory", ""), "nand")
            defaults["sdmc_root"] = _resolve_setting_path(root, storage.get("sdmc_directory", ""), "sdmc")
        else:
            defaults["nand_root"] = str((root / "nand").resolve())
            defaults["sdmc_root"] = str((root / "sdmc").resolve())
        return defaults

    default_root = first_existing_root or (user_roots[0] if user_roots else Path())
    if str(default_root):
        defaults["user_root"] = str(default_root.resolve())
        defaults["nand_root"] = str((default_root / "nand").resolve())
        defaults["sdmc_root"] = str((default_root / "sdmc").resolve())
        defaults["states_root"] = str((default_root / "states").resolve())

    return defaults


def _existing_sdmc_title_roots(sdmc_root: Path) -> list[Path]:
    container_root = sdmc_root / "Nintendo 3DS"
    discovered: list[Path] = []

    if container_root.exists() and container_root.is_dir():
        for system_dir in sorted(container_root.iterdir(), key=lambda item: item.name):
            if not system_dir.is_dir():
                continue
            for storage_dir in sorted(system_dir.iterdir(), key=lambda item: item.name):
                if not storage_dir.is_dir():
                    continue
                title_root = storage_dir / "title"
                for group in _SDMC_TITLE_GROUPS:
                    candidate = (title_root / group).resolve()
                    if candidate.exists() and candidate.is_dir():
                        discovered.append(candidate)

    if discovered:
        return _unique_paths(discovered)

    default_title_root = container_root / _ZERO_ID / _ZERO_ID / "title"
    return _unique_paths([(default_title_root / group).resolve() for group in _SDMC_TITLE_GROUPS])


def _existing_nand_title_roots(nand_root: Path) -> list[Path]:
    discovered: list[Path] = []
    title_containers: list[Path] = []

    direct_title_root = nand_root / "title"
    if direct_title_root.exists() and direct_title_root.is_dir():
        title_containers.append(direct_title_root)

    if nand_root.exists() and nand_root.is_dir():
        for child in sorted(nand_root.iterdir(), key=lambda item: item.name):
            if not child.is_dir():
                continue
            title_root = child / "title"
            if title_root.exists() and title_root.is_dir():
                title_containers.append(title_root)

    for title_root in title_containers:
        for group in _NAND_TITLE_GROUPS:
            candidate = (title_root / group).resolve()
            if candidate.exists() and candidate.is_dir():
                discovered.append(candidate)

    if discovered:
        return _unique_paths(discovered)

    fallbacks = [(nand_root / _ZERO_ID / "title" / group).resolve() for group in _NAND_TITLE_GROUPS]
    fallbacks.extend((nand_root / "title" / group).resolve() for group in _NAND_TITLE_GROUPS)
    return _unique_paths(fallbacks)


def azahar_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = azahar_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    raw_paths: list[Path] = []

    sdmc_root_text = settings.get("sdmc_root", "")
    if settings.get("use_virtual_sd", "true") != "false" and isinstance(sdmc_root_text, str) and sdmc_root_text.strip():
        raw_paths.extend(_existing_sdmc_title_roots(Path(sdmc_root_text).expanduser().resolve()))

    nand_root_text = settings.get("nand_root", "")
    if isinstance(nand_root_text, str) and nand_root_text.strip():
        raw_paths.extend(_existing_nand_title_roots(Path(nand_root_text).expanduser().resolve()))

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


def azahar_state_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = azahar_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    states_root = settings.get("states_root", "")
    if not isinstance(states_root, str) or not states_root.strip():
        return []
    return [states_root.strip()]
