from __future__ import annotations

import configparser
import os
import sys
from pathlib import Path
from typing import Callable

_REGION_NAMES = ("USA", "JPN", "JAP", "EUR", "DEV")
_MEMCARD_SIZE_SUFFIXES = ("", ".59", ".123", ".251", ".507", ".1019", ".2043")
_WII_TITLE_GROUPS = ("00010000", "00010001", "00010002", "00010004", "00010005", "00010008")


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


def _launch_user_root(
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> Path | None:
    args = _split_launch_args(launch_template, split_launch_template_args)
    for index, raw_arg in enumerate(args):
        if not isinstance(raw_arg, str) or not raw_arg.strip():
            continue

        normalized_arg = raw_arg.strip()
        lowered_arg = normalized_arg.casefold()

        if lowered_arg in {"-u", "--user"} and index + 1 < len(args):
            next_arg = args[index + 1]
            if isinstance(next_arg, str) and next_arg.strip():
                cleaned = next_arg.strip().strip('"').strip("'")
                if cleaned:
                    return Path(cleaned).expanduser().resolve()
            continue

        if lowered_arg.startswith("--user="):
            _, value = normalized_arg.split("=", 1)
            cleaned = value.strip().strip('"').strip("'")
            if cleaned:
                return Path(cleaned).expanduser().resolve()

    return None


def _registry_user_root(emulator_dir: Path) -> Path | None:
    if sys.platform != "win32":
        return None

    try:
        import winreg  # type: ignore
    except ImportError:
        return None

    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Dolphin Emulator") as key:
            try:
                local_user_config, _ = winreg.QueryValueEx(key, "LocalUserConfig")
            except OSError:
                local_user_config = 0
            if int(local_user_config or 0):
                return (emulator_dir / "User").resolve()

            try:
                user_config_path, _ = winreg.QueryValueEx(key, "UserConfigPath")
            except OSError:
                user_config_path = ""
            if isinstance(user_config_path, str) and user_config_path.strip():
                return Path(user_config_path).expanduser().resolve()
    except OSError:
        return None

    return None


def dolphin_user_root_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    candidates: list[Path] = []

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    emulator_dir = Path()
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent

    launch_user_root = _launch_user_root(launch_template, split_launch_template_args)
    if launch_user_root is not None:
        candidates.append(launch_user_root)

    if emulator_dir:
        if (emulator_dir / "portable.txt").exists():
            candidates.append((emulator_dir / "User").resolve())

        registry_user_root = _registry_user_root(emulator_dir)
        if registry_user_root is not None:
            candidates.append(registry_user_root)

    if sys.platform == "win32":
        for raw_base in (os.environ.get("OneDrive", ""), os.environ.get("USERPROFILE", "")):
            if isinstance(raw_base, str) and raw_base.strip():
                candidates.append(Path(raw_base).expanduser() / "Documents" / "Dolphin Emulator")

        appdata = os.environ.get("APPDATA", "")
        if isinstance(appdata, str) and appdata.strip():
            candidates.append(Path(appdata).expanduser() / "Dolphin Emulator")

    home_path = Path.home()
    candidates.extend(
        [
            home_path / ".dolphin-emu",
            home_path / "Library" / "Application Support" / "Dolphin",
        ]
    )

    if emulator_dir:
        candidates.append((emulator_dir / "User").resolve())

    return _unique_paths([candidate.expanduser().resolve() for candidate in candidates if str(candidate)])


def dolphin_settings_path_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    return _unique_paths(
        [root / "Config" / "Dolphin.ini" for root in dolphin_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)]
    )


def _clean_path_value(value: str) -> str:
    return value.strip().strip('"').strip("'")


def _resolve_setting_path(root: Path, raw_value: str, default_value: str = "") -> str:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_value

    if not value:
        return ""

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute():
        candidate = root / candidate
    return str(candidate.resolve())


def dolphin_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    user_roots = dolphin_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)
    settings_candidates = dolphin_settings_path_candidates(emulator_path_text, launch_template, split_launch_template_args)

    defaults = {
        "config_path": "",
        "user_root": "",
        "gc_root": "",
        "wii_root": "",
        "state_saves": "",
        "memcard_a_path": "",
        "memcard_b_path": "",
        "gci_folder_a_path": "",
        "gci_folder_b_path": "",
        "gci_folder_a_override": "",
        "gci_folder_b_override": "",
    }

    first_existing_root: Path | None = None

    for root, candidate in zip(user_roots, settings_candidates, strict=False):
        if first_existing_root is None and root.exists() and root.is_dir():
            first_existing_root = root

        if not candidate.exists() or not candidate.is_file():
            continue

        parser = configparser.ConfigParser(interpolation=None, strict=False)
        try:
            parser.read(candidate, encoding="utf-8")
        except (configparser.Error, OSError, UnicodeDecodeError):
            continue

        core = parser["Core"] if parser.has_section("Core") else {}
        general = parser["General"] if parser.has_section("General") else {}

        defaults["config_path"] = str(candidate)
        defaults["user_root"] = str(root)
        defaults["gc_root"] = str((root / "GC").resolve())
        defaults["wii_root"] = _resolve_setting_path(root, general.get("NANDRootPath", ""), "Wii")
        defaults["state_saves"] = str((root / "StateSaves").resolve())
        defaults["memcard_a_path"] = _resolve_setting_path(root, core.get("MemcardAPath", ""))
        defaults["memcard_b_path"] = _resolve_setting_path(root, core.get("MemcardBPath", ""))
        defaults["gci_folder_a_path"] = _resolve_setting_path(root, core.get("GCIFolderAPath", ""))
        defaults["gci_folder_b_path"] = _resolve_setting_path(root, core.get("GCIFolderBPath", ""))
        defaults["gci_folder_a_override"] = _resolve_setting_path(root, core.get("GCIFolderAPathOverride", ""))
        defaults["gci_folder_b_override"] = _resolve_setting_path(root, core.get("GCIFolderBPathOverride", ""))
        return defaults

    default_root = first_existing_root or (user_roots[0] if user_roots else Path())
    if str(default_root):
        defaults["user_root"] = str(default_root)
        defaults["gc_root"] = str((default_root / "GC").resolve())
        defaults["wii_root"] = str((default_root / "Wii").resolve())
        defaults["state_saves"] = str((default_root / "StateSaves").resolve())

    return defaults


def _default_memcard_paths(gc_root: Path, slot_letter: str) -> list[Path]:
    paths: list[Path] = []
    for region in _REGION_NAMES:
        for suffix in _MEMCARD_SIZE_SUFFIXES:
            name = f"MemoryCard{slot_letter}.{region}{suffix}.raw"
            paths.append(gc_root / name)
    return paths


def _default_gci_paths(gc_root: Path, slot_letter: str) -> list[Path]:
    return [gc_root / region / f"Card {slot_letter}" for region in _REGION_NAMES]


def _configured_gci_paths(configured_path: Path) -> list[Path]:
    candidates = [configured_path]
    base_path = configured_path.parent if configured_path.name.upper() in _REGION_NAMES else configured_path
    for region in _REGION_NAMES:
        candidates.append(base_path / region)
    return _unique_paths(candidates)


def dolphin_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = dolphin_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    user_root_text = settings.get("user_root", "")
    if not isinstance(user_root_text, str) or not user_root_text.strip():
        return []

    gc_root = Path(settings.get("gc_root", "") or Path(user_root_text) / "GC").expanduser().resolve()
    wii_root = Path(settings.get("wii_root", "") or Path(user_root_text) / "Wii").expanduser().resolve()

    raw_paths: list[Path] = []

    for configured_key, slot_letter in (("memcard_a_path", "A"), ("memcard_b_path", "B")):
        configured_path = settings.get(configured_key, "")
        if isinstance(configured_path, str) and configured_path.strip():
            raw_paths.append(Path(configured_path).expanduser().resolve())
        raw_paths.extend(_default_memcard_paths(gc_root, slot_letter))

    for configured_key in ("gci_folder_a_override", "gci_folder_b_override"):
        configured_path = settings.get(configured_key, "")
        if isinstance(configured_path, str) and configured_path.strip():
            raw_paths.append(Path(configured_path).expanduser().resolve())

    for configured_key, slot_letter in (("gci_folder_a_path", "A"), ("gci_folder_b_path", "B")):
        configured_path = settings.get(configured_key, "")
        if isinstance(configured_path, str) and configured_path.strip():
            raw_paths.extend(_configured_gci_paths(Path(configured_path).expanduser().resolve()))
        raw_paths.extend(_default_gci_paths(gc_root, slot_letter))

    raw_paths.append((wii_root / "title").resolve())
    raw_paths.extend((wii_root / "title" / group).resolve() for group in _WII_TITLE_GROUPS)

    unique: list[str] = []
    seen: set[str] = set()
    for candidate in raw_paths:
        key = str(candidate).casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        unique.append(str(candidate))
    return unique


def dolphin_state_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = dolphin_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    state_root = settings.get("state_saves", "")
    if not isinstance(state_root, str) or not state_root.strip():
        return []
    return [state_root.strip()]
