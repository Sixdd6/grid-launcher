from __future__ import annotations

import os
import re
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


def _portable_data_root(
    emulator_dir: Path,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> Path | None:
    args = _split_launch_args(launch_template, split_launch_template_args)
    portable_requested = any(
        isinstance(arg, str) and arg.strip().casefold() == "-portable"
        for arg in args
    )

    portable_ini_path = emulator_dir / "portable.ini"
    portable_txt_path = emulator_dir / "portable.txt"
    if not portable_requested and not portable_ini_path.exists() and not portable_txt_path.exists():
        return None

    portable_suffix = ""
    if portable_txt_path.exists() and portable_txt_path.is_file():
        try:
            portable_suffix = portable_txt_path.read_text(encoding="utf-8").strip()
        except OSError:
            portable_suffix = ""

    portable_root = (emulator_dir / portable_suffix).expanduser() if portable_suffix else emulator_dir
    return portable_root.resolve()


def pcsx2_data_root_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    portable_roots: list[Path] = []
    fallback_roots: list[Path] = []

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            portable_root = _portable_data_root(emulator_dir, launch_template, split_launch_template_args)
            if portable_root is not None:
                portable_roots.append(portable_root)
            fallback_roots.append(emulator_dir.resolve())

    user_roots: list[Path] = []
    for raw_base in (
        os.environ.get("OneDrive", ""),
        os.environ.get("USERPROFILE", ""),
        os.environ.get("HOME", ""),
    ):
        if isinstance(raw_base, str) and raw_base.strip():
            user_roots.append(Path(raw_base).expanduser() / "Documents" / "PCSX2")

    home_path = Path.home()
    user_roots.extend(
        [
            home_path / "Documents" / "PCSX2",
            home_path / ".config" / "PCSX2",
            home_path / "Library" / "Application Support" / "PCSX2",
        ]
    )

    xdg_config_home = os.environ.get("XDG_CONFIG_HOME", "").strip()
    if xdg_config_home:
        user_roots.append(Path(xdg_config_home).expanduser() / "PCSX2")

    return _unique_paths([*portable_roots, *user_roots, *fallback_roots])


def pcsx2_settings_path_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    return _unique_paths(
        [root / "inis" / "PCSX2.ini" for root in pcsx2_data_root_candidates(emulator_path_text, launch_template, split_launch_template_args)]
    )


def _clean_path_value(value: str) -> str:
    return value.strip().strip('"').strip("'")


def _resolve_setting_path(root: Path, raw_value: str, default_value: str) -> str:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_value

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute():
        candidate = root / candidate
    return str(candidate.resolve())


def _parse_ini_sections(raw_content: str) -> dict[str, dict[str, str]]:
    sections: dict[str, dict[str, str]] = {}
    current_section = ""

    for raw_line in raw_content.splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith(("#", ";")):
            continue

        section_match = re.match(r"^\[(.+?)\]\s*$", stripped)
        if section_match:
            current_section = section_match.group(1).strip().casefold()
            sections.setdefault(current_section, {})
            continue

        if not current_section or "=" not in raw_line:
            continue

        key, value = raw_line.split("=", 1)
        sections.setdefault(current_section, {})[key.strip().casefold()] = value.strip()

    return sections


def pcsx2_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    defaults = {
        "config_path": "",
        "data_root": "",
        "memory_cards": "",
        "savestates": "",
        "slot1_filename": "Mcd001.ps2",
        "slot2_filename": "Mcd002.ps2",
    }

    data_roots = pcsx2_data_root_candidates(emulator_path_text, launch_template, split_launch_template_args)
    settings_candidates = pcsx2_settings_path_candidates(emulator_path_text, launch_template, split_launch_template_args)

    for root, candidate in zip(data_roots, settings_candidates, strict=False):
        if not candidate.exists() or not candidate.is_file():
            continue

        try:
            raw_content = candidate.read_text(encoding="utf-8")
        except OSError:
            continue

        sections = _parse_ini_sections(raw_content)
        folder_settings = sections.get("folders", {})
        memory_card_settings = sections.get("memorycards", {})

        defaults["config_path"] = str(candidate)
        defaults["data_root"] = str(root)
        defaults["memory_cards"] = _resolve_setting_path(root, folder_settings.get("memorycards", ""), "memcards")
        defaults["savestates"] = _resolve_setting_path(root, folder_settings.get("savestates", ""), "sstates")

        slot1_filename = _clean_path_value(memory_card_settings.get("slot1_filename", ""))
        slot2_filename = _clean_path_value(memory_card_settings.get("slot2_filename", ""))
        if slot1_filename:
            defaults["slot1_filename"] = slot1_filename
        if slot2_filename:
            defaults["slot2_filename"] = slot2_filename
        return defaults

    if data_roots:
        default_root = data_roots[0]
        defaults["data_root"] = str(default_root)
        defaults["memory_cards"] = str((default_root / "memcards").resolve())
        defaults["savestates"] = str((default_root / "sstates").resolve())

    return defaults


def pcsx2_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = pcsx2_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    memory_cards_root = settings.get("memory_cards", "")
    if not isinstance(memory_cards_root, str) or not memory_cards_root.strip():
        return []

    root_path = Path(memory_cards_root.strip()).expanduser()
    raw_paths = [
        root_path / settings.get("slot1_filename", "Mcd001.ps2"),
        root_path / settings.get("slot2_filename", "Mcd002.ps2"),
        root_path,
    ]

    unique: list[str] = []
    seen: set[str] = set()
    for candidate in raw_paths:
        key = str(candidate).casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        unique.append(str(candidate))
    return unique


def pcsx2_state_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = pcsx2_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    savestates_root = settings.get("savestates", "")
    if not isinstance(savestates_root, str) or not savestates_root.strip():
        return []
    return [savestates_root.strip()]
