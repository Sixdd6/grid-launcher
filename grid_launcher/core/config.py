from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Callable


def normalize_emulators(
    value: Any,
    normalize_save_strategy_value: Callable[[str], str],
) -> list[dict[str, str]]:
    if not isinstance(value, list):
        return []

    normalized: list[dict[str, str]] = []
    for item in value:
        if not isinstance(item, dict):
            continue
        name = item.get("name")
        path = item.get("path")
        args = item.get("args")
        save_strategy = item.get("save_strategy", "auto")
        ignore_files = item.get("ignore_files", "")
        ignore_extensions = item.get("ignore_extensions", "")
        save_paths = item.get("save_paths", "")
        state_paths = item.get("state_paths", "")
        source_id = item.get("source_id", "")
        source_provider = item.get("source_provider", "")
        source_owner = item.get("source_owner", "")
        source_repo = item.get("source_repo", "")
        source_release_tag = item.get("source_release_tag", "")
        if not isinstance(name, str) or not name.strip():
            continue
        if not isinstance(path, str):
            path = ""
        if not isinstance(args, str):
            args = "%rom%"
        if not isinstance(save_strategy, str):
            save_strategy = "auto"
        if not isinstance(ignore_files, str):
            ignore_files = ""
        if not isinstance(ignore_extensions, str):
            ignore_extensions = ""
        if not isinstance(save_paths, str):
            save_paths = ""
        if not isinstance(state_paths, str):
            state_paths = ""
        if not isinstance(source_id, str):
            source_id = ""
        if not isinstance(source_provider, str):
            source_provider = ""
        if not isinstance(source_owner, str):
            source_owner = ""
        if not isinstance(source_repo, str):
            source_repo = ""
        if not isinstance(source_release_tag, str):
            source_release_tag = ""
        normalized_entry = {
            "name": name.strip(),
            "path": path.strip(),
            "args": args.strip() or "%rom%",
            "save_strategy": normalize_save_strategy_value(save_strategy),
            "ignore_files": ignore_files.strip(),
            "ignore_extensions": ignore_extensions.strip(),
            "save_paths": save_paths.strip(),
            "state_paths": state_paths.strip(),
        }
        if source_id.strip():
            normalized_entry["source_id"] = source_id.strip()
        if source_provider.strip():
            normalized_entry["source_provider"] = source_provider.strip()
        if source_owner.strip():
            normalized_entry["source_owner"] = source_owner.strip()
        if source_repo.strip():
            normalized_entry["source_repo"] = source_repo.strip()
        if source_release_tag.strip():
            normalized_entry["source_release_tag"] = source_release_tag.strip()
        normalized.append(normalized_entry)

    normalized.sort(key=lambda emulator: emulator["name"].lower())
    return normalized


def normalize_default_emulators(value: Any) -> dict[str, str]:
    if not isinstance(value, dict):
        return {}

    normalized: dict[str, str] = {}
    for key, item in value.items():
        if isinstance(key, str) and key.strip() and isinstance(item, str):
            normalized[key.strip()] = item
    return normalized


def normalize_default_retroarch_cores(value: Any) -> dict[str, str]:
    if not isinstance(value, dict):
        return {}

    normalized: dict[str, str] = {}
    for key, item in value.items():
        if isinstance(key, str) and key.strip() and isinstance(item, str) and item.strip():
            normalized[key.strip()] = item.strip()
    return normalized


def normalize_installed_games(
    value: Any,
    game_key_fn: Callable[[dict[str, str]], tuple[str, str]],
) -> list[dict[str, str]]:
    if not isinstance(value, list):
        return []

    normalized: list[dict[str, str]] = []
    seen: set[tuple[str, str]] = set()
    for item in value:
        if not isinstance(item, dict):
            continue
        title = item.get("title")
        platform = item.get("platform")
        if not isinstance(title, str) or not title.strip():
            continue
        if not isinstance(platform, str) or not platform.strip():
            continue

        rating = item.get("rating")
        description = item.get("description")
        cover_url = item.get("cover_url")
        cached_cover_path = item.get("cached_cover_path")
        screenshot_urls = item.get("screenshot_urls")
        genres = item.get("genres")
        regions = item.get("regions")
        filesize_bytes = item.get("filesize_bytes")
        rom_id = item.get("rom_id")
        ra_id = item.get("ra_id", "")
        server_updated_at = item.get("server_updated_at")
        rom_file_name = item.get("rom_file_name")
        extracted_path = item.get("extracted_path")
        extracted_dir = item.get("extracted_dir")
        archive_path = item.get("archive_path")
        native_executable_path = item.get("native_executable_path")
        native_launch_parameters = item.get("native_launch_parameters")
        native_compat_tool = item.get("native_compat_tool")
        native_wineprefix = item.get("native_wineprefix")
        native_game_dir = item.get("native_game_dir")
        included_dlc = item.get("included_dlc")
        ps3_trophy_paths = item.get("ps3_trophy_paths")
        ps3_game_id = item.get("ps3_game_id")
        ps4_game_id = item.get("ps4_game_id")
        ps4_content = item.get("ps4_content")
        normalized_game = {
            "title": title.strip(),
            "platform": platform.strip(),
            "rating": rating.strip() if isinstance(rating, str) and rating.strip() else "N/A",
            "description": description.strip() if isinstance(description, str) and description.strip() else "No description available.",
            "cover_url": cover_url.strip() if isinstance(cover_url, str) else "",
            "cached_cover_path": cached_cover_path.strip() if isinstance(cached_cover_path, str) else "",
            "screenshot_urls": screenshot_urls.strip() if isinstance(screenshot_urls, str) else "",
            "genres": genres.strip() if isinstance(genres, str) else "",
            "regions": regions.strip() if isinstance(regions, str) else "",
            "filesize_bytes": filesize_bytes.strip() if isinstance(filesize_bytes, str) else "",
            "rom_id": rom_id.strip() if isinstance(rom_id, str) else "",
            "ra_id": ra_id.strip() if isinstance(ra_id, str) else "",
            "server_updated_at": server_updated_at.strip() if isinstance(server_updated_at, str) else "",
            "rom_file_name": rom_file_name.strip() if isinstance(rom_file_name, str) else "",
            "extracted_path": extracted_path.strip() if isinstance(extracted_path, str) else "",
            "extracted_dir": extracted_dir.strip() if isinstance(extracted_dir, str) else "",
            "archive_path": archive_path.strip() if isinstance(archive_path, str) else "",
            "native_executable_path": native_executable_path.strip() if isinstance(native_executable_path, str) else "",
            "native_launch_parameters": native_launch_parameters.strip() if isinstance(native_launch_parameters, str) else "",
            "native_compat_tool": native_compat_tool.strip() if isinstance(native_compat_tool, str) else "",
            "native_wineprefix": native_wineprefix.strip() if isinstance(native_wineprefix, str) else "",
            "native_game_dir": native_game_dir.strip() if isinstance(native_game_dir, str) else "",
            "included_dlc": included_dlc.strip() if isinstance(included_dlc, str) else "",
            "ps3_trophy_paths": ps3_trophy_paths.strip() if isinstance(ps3_trophy_paths, str) else "",
            "ps3_game_id": ps3_game_id.strip().upper() if isinstance(ps3_game_id, str) else "",
            "ps4_game_id": ps4_game_id.strip().upper() if isinstance(ps4_game_id, str) else "",
            "ps4_content": ps4_content.strip() if isinstance(ps4_content, str) else "",
        }
        local_path = item.get("local_path")
        normalized_game["local_path"] = local_path.strip() if isinstance(local_path, str) else ""
        key = game_key_fn(normalized_game)
        if key in seen:
            continue
        seen.add(key)
        normalized.append(normalized_game)
    return normalized


def normalize_compat_tool_installs(value: Any) -> dict[str, dict[str, str]]:
    if not isinstance(value, dict):
        return {}

    normalized: dict[str, dict[str, str]] = {}
    for key, item in value.items():
        if not isinstance(item, dict):
            continue
        name = item.get("name", "")
        compat_tool_type = item.get("compat_tool_type", "")
        install_path = item.get("install_path", "")
        name = name.strip() if isinstance(name, str) else ""
        if not name:
            continue
        normalized[key.strip() if isinstance(key, str) else str(key)] = {
            "name": name,
            "compat_tool_type": compat_tool_type.strip() if isinstance(compat_tool_type, str) else "",
            "install_path": install_path.strip() if isinstance(install_path, str) else "",
        }
    return normalized


def merge_config_with_defaults(
    defaults: dict[str, Any],
    content: Any,
    *,
    normalize_emulators: Callable[[Any], list[dict[str, str]]],
    normalize_default_emulators: Callable[[Any], dict[str, str]],
    normalize_default_retroarch_cores: Callable[[Any], dict[str, str]],
    normalize_installed_games: Callable[[Any], list[dict[str, str]]],
    normalize_cloud_sync_state: Callable[[Any], dict[str, dict[str, Any]]],
) -> dict[str, Any]:
    if not isinstance(content, dict):
        return defaults.copy()

    merged = defaults.copy()
    for key, default_value in defaults.items():
        value = content.get(key)
        if isinstance(default_value, str) and isinstance(value, str):
            merged[key] = value
        elif isinstance(default_value, bool) and isinstance(value, bool):
            merged[key] = value
        elif isinstance(default_value, int) and not isinstance(default_value, bool) and isinstance(value, int):
            merged[key] = value
        elif key == "emulators":
            merged[key] = normalize_emulators(value)
        elif key == "default_emulators":
            merged[key] = normalize_default_emulators(value)
        elif key == "default_retroarch_cores":
            merged[key] = normalize_default_retroarch_cores(value)
        elif key == "installed_games":
            merged[key] = normalize_installed_games(value)
        elif key == "cloud_sync_state":
            merged[key] = normalize_cloud_sync_state(value)
        elif isinstance(default_value, list) and isinstance(value, list):
            merged[key] = value

    if "first_run_completed" not in content:
        merged["first_run_completed"] = bool(content)
    return merged


def serialized_config(config: dict[str, Any]) -> dict[str, Any]:
    serialized = config.copy()
    serialized["api_token"] = ""
    serialized["retroachievements_token"] = ""
    serialized["retroachievements_api_key"] = ""
    return serialized


def write_config_file(config_dir: Path, config_file: Path, config: dict[str, Any]) -> None:
    config_dir.mkdir(parents=True, exist_ok=True)
    config_file.write_text(
        json.dumps(serialized_config(config), indent=2, sort_keys=True),
        encoding="utf-8",
    )
