from __future__ import annotations

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
                for field in ("cover_url", "screenshot_urls", "rating", "description", "rom_file_name"):
                    server_value = server_game.get(field, "")
                    if not isinstance(server_value, str) or not server_value.strip():
                        continue
                    current_value = game.get(field, "")
                    if not isinstance(current_value, str) or not current_value.strip():
                        game[field] = server_value.strip()
                matched = True
                break
        if matched:
            break

    if resolved_cover_url_for_game(game):
        return

    payload = server_rom_payloads.get(normalized_rom_id)
    if not isinstance(payload, dict):
        payload = fetch_server_rom_payload(normalized_rom_id)
    if not isinstance(payload, dict):
        return

    resolved_cover = cover_url_from_rom_payload(payload)
    if resolved_cover:
        game["cover_url"] = resolved_cover

    screenshot_value = game.get("screenshot_urls", "")
    if not isinstance(screenshot_value, str) or not screenshot_value.strip():
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

    cover_value = install_game.get("cover_url", "")
    if isinstance(cover_value, str):
        current_details_game["cover_url"] = cover_value.strip()

    screenshot_value = install_game.get("screenshot_urls", "")
    if isinstance(screenshot_value, str):
        current_details_game["screenshot_urls"] = screenshot_value.strip()
