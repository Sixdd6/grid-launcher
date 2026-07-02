from __future__ import annotations

import json
from typing import Any, Callable


def archive_name_for_game(
    game: dict[str, str],
    sanitize_path_component: Callable[[str, str], str],
) -> str:
    rom_file_name_value = game.get("rom_file_name", "")
    if isinstance(rom_file_name_value, str):
        rom_file_name = rom_file_name_value.strip().replace("\\", "/").split("/")[-1]
        if rom_file_name:
            return rom_file_name

    title_value = game.get("title", "Game")
    platform_value = game.get("platform", "Platform")
    title = title_value if isinstance(title_value, str) else "Game"
    platform = platform_value if isinstance(platform_value, str) else "Platform"
    safe_title = sanitize_path_component(title, "game")
    safe_platform = sanitize_path_component(platform, "platform")
    return f"{safe_title}-{safe_platform}.zip"


def server_content_file_name_for_game(game: dict[str, str]) -> str:
    rom_file_name_value = game.get("rom_file_name", "")
    if isinstance(rom_file_name_value, str):
        rom_file_name = rom_file_name_value.strip().replace("\\", "/").lstrip("/")
        if rom_file_name:
            return rom_file_name
    return ""


def hydrate_install_game_metadata(
    game: dict[str, str],
    rom_id: str,
    *,
    server_games_by_platform: dict[str, list[dict[str, str]]],
    server_rom_payloads: dict[str, dict[str, Any]],
    game_key: Callable[[dict[str, str]], tuple[str, str]],
    rom_id_key: Callable[[dict[str, str]], str],
    fetch_server_rom_payload: Callable[[str], dict[str, Any] | None],
    resolved_cover_url_for_game: Callable[[dict[str, str]], str],
    cover_url_from_rom_payload: Callable[[dict[str, Any]], str],
    screenshot_urls_from_rom_payload: Callable[[dict[str, Any]], list[str]],
) -> None:
    normalized_rom_id = rom_id.strip()
    if not normalized_rom_id:
        return

    target_key = game_key(game)
    matched = False
    for games in server_games_by_platform.values():
        for server_game in games:
            if rom_id_key(server_game) == normalized_rom_id.casefold() or game_key(server_game) == target_key:
                for field in (
                    "cover_url",
                    "screenshot_urls",
                    "rating",
                    "description",
                    "genres",
                    "regions",
                    "release_year",
                    "filesize_bytes",
                    "rom_file_name",
                    "rom_nested_file_name",
                    "rom_base_file_id",
                    "ps4_has_update",
                    "ps4_has_dlc",
                    "ps4_file_ids_by_category",
                    "revision",
                    "languages",
                    "tags",
                    "fanart_url",
                    "companies",
                    "first_release_date",
                ):
                    server_value = server_game.get(field, "")
                    if not isinstance(server_value, str) or not server_value.strip():
                        continue
                    current_value = game.get(field, "")
                    if field == "rating" and isinstance(current_value, str) and current_value.strip().casefold() == "n/a":
                        current_value = ""
                    if (
                        field == "description"
                        and isinstance(current_value, str)
                        and current_value.strip().casefold() == "no description available."
                    ):
                        current_value = ""
                    if not isinstance(current_value, str) or not current_value.strip():
                        game[field] = server_value.strip()
                matched = True
                break
        if matched:
            break

    payload = server_rom_payloads.get(normalized_rom_id)
    if not isinstance(payload, dict):
        payload = fetch_server_rom_payload(normalized_rom_id)
    if not isinstance(payload, dict):
        return

    if not resolved_cover_url_for_game(game):
        resolved_cover = cover_url_from_rom_payload(payload)
        if resolved_cover:
            game["cover_url"] = resolved_cover

    screenshots = screenshot_urls_from_rom_payload(payload)
    if screenshots:
        game["screenshot_urls"] = "\n".join(screenshots)


def sync_install_metadata_to_details_game(
    current_details_game: dict[str, str] | None,
    install_game: dict[str, str],
    *,
    game_key: Callable[[dict[str, str]], tuple[str, str]],
) -> None:
    if current_details_game is None or game_key(current_details_game) != game_key(install_game):
        return

    current_details_game["rom_id"] = install_game.get("rom_id", "")
    current_details_game["rom_file_name"] = install_game.get("rom_file_name", "")
    current_details_game["rom_base_file_id"] = install_game.get("rom_base_file_id", "")

    cover_value = install_game.get("cover_url", "")
    if isinstance(cover_value, str):
        current_details_game["cover_url"] = cover_value.strip()

    screenshot_value = install_game.get("screenshot_urls", "")
    if isinstance(screenshot_value, str):
        current_details_game["screenshot_urls"] = screenshot_value.strip()

    for field in ("rating", "description", "genres", "regions", "release_year", "filesize_bytes", "revision", "languages", "tags", "fanart_url", "companies", "first_release_date"):
        field_value = install_game.get(field, "")
        if isinstance(field_value, str):
            current_details_game[field] = field_value.strip()

    for field in ("ps4_has_update", "ps4_has_dlc", "ps4_file_ids_by_category"):
        field_value = install_game.get(field, "")
        if isinstance(field_value, str):
            current_details_game[field] = field_value.strip()


def parse_windows_game_json(data: bytes) -> dict[str, str]:
    try:
        game_json = json.loads(data)
    except (ValueError, UnicodeDecodeError):
        return {}

    if not isinstance(game_json, dict):
        return {}

    version_value = game_json.get("version")
    revision = "" if version_value is None else str(version_value)

    first_release_date = ""
    year_val = game_json.get("year") or game_json.get("release_year", "")
    try:
        first_release_date = str(int(year_val))
    except (TypeError, ValueError):
        first_release_date = ""

    tags = ""
    tags_value = game_json.get("tags")
    if isinstance(tags_value, list) and tags_value and all(isinstance(item, str) for item in tags_value):
        tags = ", ".join(tags_value)

    included_dlc = "[]"
    dlc_value = game_json.get("included_dlc")
    if isinstance(dlc_value, list):
        included_dlc = json.dumps(dlc_value)

    name = ""
    name_value = game_json.get("name")
    if name_value is not None:
        name = str(name_value)

    return {
        "revision": revision,
        "first_release_date": first_release_date,
        "tags": tags,
        "included_dlc": included_dlc,
        "name": name,
    }


def apply_windows_game_json_to_game(game: dict[str, str], parsed: dict[str, str]) -> None:
    if not parsed:
        return

    for field in ("revision", "first_release_date", "tags"):
        parsed_value = parsed.get(field, "")
        if not parsed_value:
            continue
        current_value = game.get(field, "")
        if not isinstance(current_value, str) or not current_value.strip():
            game[field] = parsed_value

    game["included_dlc"] = parsed.get("included_dlc", "[]")
