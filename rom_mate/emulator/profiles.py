from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Callable


def matching_platforms_for_emulator_keywords(assignable_platforms: list[str], keywords: list[str]) -> list[str]:
    if not keywords:
        return []

    def token_set(value: str) -> set[str]:
        return set(re.findall(r"[a-z0-9]+", value.casefold()))

    matches: list[str] = []
    for platform in assignable_platforms:
        platform_tokens = token_set(platform)
        if not platform_tokens:
            continue

        for keyword in keywords:
            if not isinstance(keyword, str):
                continue
            keyword_tokens = token_set(keyword.strip())
            if not keyword_tokens:
                continue
            if not keyword_tokens.issubset(platform_tokens):
                continue

            extra_tokens = platform_tokens - keyword_tokens
            keyword_has_numeric_token = any(token.isdigit() for token in keyword_tokens)
            extra_has_numeric_token = any(token.isdigit() for token in extra_tokens)
            if extra_has_numeric_token and not keyword_has_numeric_token:
                continue

            if platform not in matches:
                matches.append(platform)
            break
    return matches


def split_configured_paths(value: str) -> list[str]:
    return [
        item.strip()
        for item in re.split(r"[;\r\n]+", value)
        if isinstance(item, str) and item.strip()
    ]


def normalize_save_strategy_value(value: str) -> str:
    strategy = value.strip().casefold() if isinstance(value, str) else ""
    aliases = {
        "": "auto",
        "auto": "auto",
        "singlefile": "single_file",
        "single_file": "single_file",
        "single-file": "single_file",
        "single file": "single_file",
        "file": "single_file",
        "folder": "folder",
        "directory": "folder",
        "folder_per_game": "folder",
        "folder-per-game": "folder",
    }
    return aliases.get(strategy, "auto")


def normalize_ignore_extension_value(value: str) -> str:
    if not isinstance(value, str):
        return ""
    normalized = value.strip().casefold()
    if not normalized:
        return ""
    if "/" in normalized or "\\" in normalized:
        normalized = Path(normalized).suffix.casefold()
    if normalized.startswith("*."):
        normalized = normalized[1:]
    if not normalized.startswith("."):
        normalized = f".{normalized.lstrip('*')}"
    if not re.fullmatch(r"\.[a-z0-9]+", normalized):
        return ""
    if normalized in {".jpg", ".jpeg"}:
        return ""
    return normalized


def emulator_profile_for_entry(
    emulator: dict[str, str],
    profiles: list[dict[str, Any]],
) -> dict[str, Any] | None:
    name_value = emulator.get("name", "")
    path_value = emulator.get("path", "")
    name = name_value.strip().casefold() if isinstance(name_value, str) else ""
    executable_name = Path(path_value).name.strip().casefold() if isinstance(path_value, str) else ""
    executable_stem = Path(executable_name).stem.casefold() if executable_name else ""

    for profile in profiles:
        profile_name = profile.get("name", "")
        profile_name_folded = profile_name.strip().casefold() if isinstance(profile_name, str) else ""
        if name and profile_name_folded == name:
            return profile

        match_tokens = profile.get("match_tokens", [])
        if not isinstance(match_tokens, list):
            continue
        normalized_tokens = {
            token.strip().casefold()
            for token in match_tokens
            if isinstance(token, str) and token.strip()
        }
        if not normalized_tokens:
            continue

        if executable_name and executable_name in normalized_tokens:
            return profile
        if executable_stem and any(Path(token).stem.casefold() == executable_stem for token in normalized_tokens):
            return profile

    return None


def emulator_profile_for_game(
    game: dict[str, str],
    executable_path: str,
    profiles: list[dict[str, Any]],
) -> dict[str, Any]:
    title_value = game.get("title", "")
    title = title_value.strip() if isinstance(title_value, str) else ""
    executable_name = Path(executable_path).name.strip().casefold()

    for profile in profiles:
        match_tokens = profile.get("match_tokens", [])
        if executable_name and isinstance(match_tokens, list) and any(token == executable_name for token in match_tokens):
            resolved_name = profile.get("name", "Emulator")
            if profile.get("use_game_title_as_name", False):
                resolved_name = title or resolved_name
            return {
                "name": resolved_name,
                "args": profile.get("args", "%rom%"),
                "all_platforms": bool(profile.get("all_platforms", False)),
                "platform_keywords": profile.get("platform_keywords", []),
                "save_strategy": profile.get("save_strategy", "auto"),
                "ignore_files": profile.get("ignore_files", []),
                "ignore_extensions": profile.get("ignore_extensions", []),
                "save_directories": profile.get("save_directories", []),
                "state_directories": profile.get("state_directories", []),
            }

    return {
        "name": title or "Emulator",
        "args": "%rom%",
        "all_platforms": False,
        "platform_keywords": [],
        "save_strategy": "auto",
        "ignore_files": [],
        "ignore_extensions": [],
        "save_directories": [],
        "state_directories": [],
    }


def resolved_save_strategy_for_emulator(
    emulator: dict[str, str],
    save_type: str,
    *,
    emulator_profile_for_entry_fn: Callable[[dict[str, str]], dict[str, Any] | None],
) -> str:
    configured_value = emulator.get("save_strategy", "")
    configured_strategy = normalize_save_strategy_value(configured_value) if isinstance(configured_value, str) else "auto"
    if configured_strategy != "auto":
        return configured_strategy

    profile = emulator_profile_for_entry_fn(emulator)
    profile_value = profile.get("save_strategy", "") if isinstance(profile, dict) else ""
    profile_strategy = normalize_save_strategy_value(profile_value) if isinstance(profile_value, str) else "auto"
    if profile_strategy != "auto":
        return profile_strategy

    if save_type == "state":
        return "single_file"
    return "auto"


def resolved_ignore_basenames_for_emulator(
    emulator: dict[str, str],
    *,
    emulator_profile_for_entry_fn: Callable[[dict[str, str]], dict[str, Any] | None],
) -> set[str]:
    configured_value = emulator.get("ignore_files", "")
    configured_values = split_configured_paths(configured_value) if isinstance(configured_value, str) else []

    profile = emulator_profile_for_entry_fn(emulator)
    profile_values: list[str] = []
    if isinstance(profile, dict):
        raw_profile_values = profile.get("ignore_files", [])
        if isinstance(raw_profile_values, list):
            profile_values = [item.strip() for item in raw_profile_values if isinstance(item, str) and item.strip()]

    all_values = configured_values if configured_values else profile_values
    basenames: set[str] = set()
    for value in all_values:
        base_name = Path(value).name.strip().casefold()
        if base_name and Path(base_name).suffix.casefold() not in {".jpg", ".jpeg"}:
            basenames.add(base_name)
    return basenames


def resolved_ignore_extensions_for_emulator(
    emulator: dict[str, str],
    *,
    emulator_profile_for_entry_fn: Callable[[dict[str, str]], dict[str, Any] | None],
) -> set[str]:
    configured_value = emulator.get("ignore_extensions", "")
    configured_values = split_configured_paths(configured_value) if isinstance(configured_value, str) else []

    profile = emulator_profile_for_entry_fn(emulator)
    profile_values: list[str] = []
    if isinstance(profile, dict):
        raw_profile_values = profile.get("ignore_extensions", [])
        if isinstance(raw_profile_values, list):
            profile_values = [item.strip() for item in raw_profile_values if isinstance(item, str) and item.strip()]

    all_values = configured_values if configured_values else profile_values
    normalized: set[str] = set()
    for value in all_values:
        normalized_value = normalize_ignore_extension_value(value)
        if normalized_value:
            normalized.add(normalized_value)
    return normalized


def emulator_autoprofiles_path(base_path: Path) -> Path:
    return base_path / "emulator-autoprofiles.json"


def default_emulator_autoprofiles() -> list[dict[str, Any]]:
    return [
        {
            "match_tokens": ["retroarch.exe"],
            "name": "RetroArch",
            "args": '-L "%core%" "%rom%"',
            "all_platforms": True,
            "platform_keywords": [],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": ["saves"],
            "state_directories": ["states"],
        },
        {
            "match_tokens": ["duckstation.exe"],
            "name": "DuckStation",
            "args": '-fullscreen -batch "%rom%"',
            "all_platforms": False,
            "platform_keywords": ["playstation", "ps1"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["pcsx2-qt.exe"],
            "name": "PCSX2",
            "args": '-fullscreen -batch "%rom%"',
            "all_platforms": False,
            "platform_keywords": ["playstation 2", "ps2"],
            "use_game_title_as_name": False,
            "save_strategy": "folder",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["ppssppqt.exe"],
            "name": "PPSSPP",
            "args": '--fullscreen --pause-menu-exit "%rom%"',
            "all_platforms": False,
            "platform_keywords": ["playstation portable", "psp"],
            "use_game_title_as_name": False,
            "save_strategy": "folder",
            "ignore_files": ["load_undo.ppst"],
            "ignore_extensions": [],
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["rpcs3.exe"],
            "name": "RPCS3",
            "args": "%rom%",
            "all_platforms": False,
            "platform_keywords": ["playstation 3", "ps3"],
            "use_game_title_as_name": False,
            "save_strategy": "folder",
            "save_directories": [
                "%EMULATOR_DIR%\\dev_hdd0\\home\\00000001\\savedata",
                "%APPDATA%\\rpcs3\\dev_hdd0\\home\\00000001\\savedata",
            ],
            "state_directories": [],
        },
        {
            "match_tokens": ["dolphin.exe"],
            "name": "Dolphin",
            "args": '-b -e "%rom%"',
            "all_platforms": False,
            "platform_keywords": ["gamecube", "wii"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["cemu.exe"],
            "name": "Cemu",
            "args": '-f -g "%rom%"',
            "all_platforms": False,
            "platform_keywords": ["wii u"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["azahar.exe"],
            "name": "Azahar",
            "args": '-f "%rom%"',
            "all_platforms": False,
            "platform_keywords": ["nintendo 3ds", "3ds"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["pico8.exe"],
            "name": "Pico",
            "args": '-run "%rom%"',
            "all_platforms": False,
            "platform_keywords": ["pico-8", "pico 8"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["xemu.exe"],
            "name": "Xemu",
            "args": '-full-screen -dvd_path "%rom%"',
            "all_platforms": False,
            "platform_keywords": ["xbox"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["eden.exe"],
            "name": "Eden",
            "args": '-f -g "%rom%"',
            "all_platforms": False,
            "platform_keywords": ["switch", "nintendo switch"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["yuzu.exe"],
            "name": "Yuzu",
            "args": "%rom%",
            "all_platforms": False,
            "platform_keywords": ["switch", "nintendo switch"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["ryujinx.exe"],
            "name": "Ryujinx",
            "args": "%rom%",
            "all_platforms": False,
            "platform_keywords": ["switch", "nintendo switch"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["sudachi.exe"],
            "name": "Sudachi",
            "args": "%rom%",
            "all_platforms": False,
            "platform_keywords": ["switch", "nintendo switch"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["mame.exe"],
            "name": "MAME",
            "args": "%rom%",
            "all_platforms": False,
            "platform_keywords": ["arcade", "mame", "final burn", "fbneo"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["fbneo.exe"],
            "name": "FBNeo",
            "args": "%rom%",
            "all_platforms": False,
            "platform_keywords": ["arcade", "mame", "final burn", "fbneo"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
        {
            "match_tokens": ["finalburnneo.exe"],
            "name": "FinalBurn Neo",
            "args": "%rom%",
            "all_platforms": False,
            "platform_keywords": ["arcade", "mame", "final burn", "fbneo"],
            "use_game_title_as_name": False,
            "save_strategy": "single_file",
            "save_directories": [],
            "state_directories": [],
        },
    ]


def normalize_emulator_autoprofiles(
    value: Any,
    normalize_save_strategy_value: Callable[[str], str],
    normalize_ignore_extension_value: Callable[[str], str],
) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []

    normalized: list[dict[str, Any]] = []
    for item in value:
        if not isinstance(item, dict):
            continue

        match_tokens = item.get("match_tokens", [])
        if not isinstance(match_tokens, list):
            continue
        normalized_tokens = [
            token.strip().casefold()
            for token in match_tokens
            if isinstance(token, str) and token.strip()
        ]
        if not normalized_tokens:
            continue
        primary_token = normalized_tokens[0]

        name = item.get("name", "")
        if not isinstance(name, str) or not name.strip():
            continue

        args = item.get("args", "%rom%")
        if not isinstance(args, str):
            args = "%rom%"

        all_platforms = bool(item.get("all_platforms", False))

        platform_keywords = item.get("platform_keywords", [])
        normalized_keywords: list[str] = []
        if isinstance(platform_keywords, list):
            normalized_keywords = [
                keyword.strip()
                for keyword in platform_keywords
                if isinstance(keyword, str) and keyword.strip()
            ]

        use_game_title_as_name = bool(item.get("use_game_title_as_name", False))

        save_strategy = item.get("save_strategy", "auto")
        if not isinstance(save_strategy, str):
            save_strategy = "auto"
        normalized_save_strategy = normalize_save_strategy_value(save_strategy)

        ignore_files = item.get("ignore_files", [])
        normalized_ignore_files: list[str] = []
        if isinstance(ignore_files, list):
            normalized_ignore_files = [
                file_name.strip()
                for file_name in ignore_files
                if isinstance(file_name, str) and file_name.strip()
            ]

        ignore_extensions = item.get("ignore_extensions", [])
        normalized_ignore_extensions: list[str] = []
        if isinstance(ignore_extensions, list):
            normalized_ignore_extensions = [
                normalized_extension
                for extension in ignore_extensions
                if isinstance(extension, str)
                for normalized_extension in [normalize_ignore_extension_value(extension)]
                if normalized_extension
            ]

        save_directories = item.get("save_directories", [])
        normalized_save_directories: list[str] = []
        if isinstance(save_directories, list):
            normalized_save_directories = [
                directory.strip()
                for directory in save_directories
                if isinstance(directory, str) and directory.strip()
            ]

        state_directories = item.get("state_directories", [])
        normalized_state_directories: list[str] = []
        if isinstance(state_directories, list):
            normalized_state_directories = [
                directory.strip()
                for directory in state_directories
                if isinstance(directory, str) and directory.strip()
            ]

        normalized.append(
            {
                "match_tokens": [primary_token],
                "name": name.strip(),
                "args": args.strip() or "%rom%",
                "all_platforms": all_platforms,
                "platform_keywords": normalized_keywords,
                "use_game_title_as_name": use_game_title_as_name,
                "save_strategy": normalized_save_strategy,
                "ignore_files": normalized_ignore_files,
                "ignore_extensions": normalized_ignore_extensions,
                "save_directories": normalized_save_directories,
                "state_directories": normalized_state_directories,
            }
        )

    return normalized


def load_emulator_autoprofiles(
    cached_profiles: Any,
    base_path: Path,
    normalize_save_strategy_value: Callable[[str], str],
    normalize_ignore_extension_value: Callable[[str], str],
) -> list[dict[str, Any]]:
    if isinstance(cached_profiles, list):
        return cached_profiles

    defaults = normalize_emulator_autoprofiles(
        default_emulator_autoprofiles(),
        normalize_save_strategy_value,
        normalize_ignore_extension_value,
    )
    autoprofiles_path = emulator_autoprofiles_path(base_path)
    if not autoprofiles_path.exists():
        return defaults

    try:
        parsed = json.loads(autoprofiles_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return defaults

    normalized = normalize_emulator_autoprofiles(
        parsed,
        normalize_save_strategy_value,
        normalize_ignore_extension_value,
    )
    return normalized if normalized else defaults
