from __future__ import annotations

from pathlib import Path
from typing import Callable


def _text_value(payload: dict[str, str], key: str, default: str = "") -> str:
    value = payload.get(key, default)
    if not isinstance(value, str):
        return default
    return value.strip()


def build_installed_game_record(
    game: dict[str, str],
    archive_path: Path,
    *,
    resolved_cover_url: str,
    cached_cover_path: str,
) -> dict[str, str]:
    extracted_path = _text_value(game, "extracted_path")
    stored_archive_path = "" if extracted_path else str(archive_path)
    rating = _text_value(game, "rating", "N/A") or "N/A"
    description = _text_value(game, "description", "No description available.") or "No description available."

    return {
        "title": _text_value(game, "title"),
        "platform": _text_value(game, "platform"),
        "rating": rating,
        "description": description,
        "cover_url": resolved_cover_url,
        "cached_cover_path": cached_cover_path,
        "screenshot_urls": _text_value(game, "screenshot_urls"),
        "rom_id": _text_value(game, "rom_id"),
        "rom_file_name": _text_value(game, "rom_file_name"),
        "extracted_path": extracted_path,
        "extracted_dir": _text_value(game, "extracted_dir"),
        "archive_path": stored_archive_path,
        "native_launch_parameters": _text_value(game, "native_launch_parameters"),
        "ps3_links": _text_value(game, "ps3_links"),
        "ps3_game_id": _text_value(game, "ps3_game_id"),
        "ps4_game_id": _text_value(game, "ps4_game_id"),
        "ps4_content": _text_value(game, "ps4_content"),
    }


def matching_installed_emulator_games(
    library_games: list[dict[str, str]],
    emulator_path: Path,
    *,
    is_emulators_platform: Callable[[dict[str, str]], bool],
    candidate_archive_paths_for_game: Callable[[dict[str, str]], list[Path]],
    candidate_extracted_paths_for_game: Callable[[dict[str, str]], list[Path]],
    candidate_extracted_dirs_for_game: Callable[[dict[str, str]], list[Path]],
    path_key: Callable[[Path], str],
    path_within_path: Callable[[Path, Path], bool],
) -> list[dict[str, str]]:
    matches: list[dict[str, str]] = []
    target_key = path_key(emulator_path)
    for game in library_games:
        if not is_emulators_platform(game):
            continue

        file_candidates = candidate_archive_paths_for_game(game) + candidate_extracted_paths_for_game(game)
        if any(path_key(candidate) == target_key for candidate in file_candidates):
            matches.append(game)
            continue

        dir_candidates = candidate_extracted_dirs_for_game(game)
        if any(path_within_path(emulator_path, candidate) for candidate in dir_candidates):
            matches.append(game)
    return matches


def library_games_for_target(
    library_games: list[dict[str, str]],
    target: tuple[str, str],
    *,
    game_key: Callable[[dict[str, str]], tuple[str, str]],
) -> list[dict[str, str]]:
    return [entry for entry in library_games if game_key(entry) == target]


def library_games_without_keys(
    library_games: list[dict[str, str]],
    keys_to_remove: set[tuple[str, str]],
    *,
    game_key: Callable[[dict[str, str]], tuple[str, str]],
) -> list[dict[str, str]]:
    return [entry for entry in library_games if game_key(entry) not in keys_to_remove]
