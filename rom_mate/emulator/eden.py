from __future__ import annotations

import configparser
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


def _app_name_candidates(emulator_path_text: str) -> list[str]:
    names: list[str] = []

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        stem = Path(path_text).stem.strip()
        if stem:
            names.extend([stem, stem.casefold(), stem.title()])

    names.extend(["Eden", "eden", "yuzu", "Yuzu", "suyu", "Suyu"])

    ordered: list[str] = []
    seen: set[str] = set()
    for name in names:
        key = name.casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        ordered.append(name)
    return ordered


def eden_user_root_candidates(
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

    app_names = _app_name_candidates(emulator_path_text)
    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA", "")
        if isinstance(appdata, str) and appdata.strip():
            base = Path(appdata).expanduser()
            candidates.extend((base / name).resolve() for name in app_names)
    else:
        xdg_data_home = os.environ.get("XDG_DATA_HOME", "")
        if isinstance(xdg_data_home, str) and xdg_data_home.strip():
            base = Path(xdg_data_home).expanduser()
            candidates.extend((base / name).resolve() for name in app_names)

        home_path = Path.home()
        for name in app_names:
            candidates.extend(
                [
                    (home_path / ".local" / "share" / name).resolve(),
                    (home_path / "Library" / "Application Support" / name).resolve(),
                ]
            )

    if emulator_dir:
        candidates.append((emulator_dir / "user").resolve())

    return _unique_paths([candidate.expanduser().resolve() for candidate in candidates if str(candidate)])


def eden_settings_path_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    return _unique_paths(
        [root / "config" / "qt-config.ini" for root in eden_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)]
    )


def eden_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    user_roots = eden_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)
    settings_candidates = eden_settings_path_candidates(emulator_path_text, launch_template, split_launch_template_args)

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


def _existing_user_save_roots(nand_root: Path) -> list[Path]:
    save_root = nand_root / "user" / "save" / "0000000000000000"
    discovered: list[Path] = []

    if save_root.exists() and save_root.is_dir():
        for user_root in sorted(save_root.iterdir(), key=lambda item: item.name):
            if not user_root.is_dir():
                continue
            if any(child.is_dir() for child in user_root.iterdir()):
                discovered.append(user_root.resolve())

    if discovered:
        return _unique_paths(discovered)

    return _unique_paths([save_root.resolve()])


def eden_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = eden_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    nand_root_text = settings.get("nand_root", "")
    if not isinstance(nand_root_text, str) or not nand_root_text.strip():
        return []

    raw_paths = _existing_user_save_roots(Path(nand_root_text).expanduser().resolve())
    unique: list[str] = []
    seen: set[str] = set()
    for candidate in raw_paths:
        normalized = str(candidate.resolve())
        key = normalized.casefold()
        if key in seen:
            continue
        seen.add(key)
        unique.append(normalized)
    return unique
