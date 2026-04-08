from __future__ import annotations

from pathlib import Path
from typing import Callable


def remove_game_files(
    game: dict[str, str],
    *,
    is_ps3_platform: Callable[[dict[str, str]], bool],
    ps3_link_paths_from_game: Callable[[dict[str, str]], list[Path]],
    remove_link_path: Callable[[Path], None],
    remove_rpcs3_games_yml_for_game: Callable[[dict[str, str]], None],
    is_native_executable_platform: Callable[[dict[str, str]], bool],
    candidate_extracted_dirs_for_game: Callable[[dict[str, str]], list[Path]],
    remove_directory_tree: Callable[[Path], None],
    candidate_archive_paths_for_game: Callable[[dict[str, str]], list[Path]],
) -> None:
    if is_ps3_platform(game):
        for link_path in ps3_link_paths_from_game(game):
            if not link_path.exists() and not link_path.is_symlink():
                continue
            try:
                remove_link_path(link_path)
            except OSError as error:
                raise OSError(f"Could not remove PS3 link: {link_path}\n{error}") from error

        try:
            remove_rpcs3_games_yml_for_game(game)
        except OSError as error:
            raise OSError(f"Could not update RPCS3 games.yml for uninstall:\n{error}") from error

    if is_native_executable_platform(game):
        for extracted_dir in candidate_extracted_dirs_for_game(game):
            if not extracted_dir.exists() or not extracted_dir.is_dir():
                continue
            try:
                remove_directory_tree(extracted_dir)
            except OSError as error:
                raise OSError(f"Could not remove folder: {extracted_dir}\n{error}") from error
        return

    for candidate in candidate_archive_paths_for_game(game):
        if not candidate.exists() or not candidate.is_file():
            continue
        try:
            candidate.unlink()
        except OSError as error:
            raise OSError(f"Could not remove file: {candidate}\n{error}") from error

    for extracted_dir in candidate_extracted_dirs_for_game(game):
        if not extracted_dir.exists() or not extracted_dir.is_dir():
            continue
        try:
            remove_directory_tree(extracted_dir)
        except OSError as error:
            raise OSError(f"Could not remove folder: {extracted_dir}\n{error}") from error


def uninstall_library_games(
    library_games: list[dict[str, str]],
    keys_to_remove: set[tuple[str, str]],
    *,
    game_key: Callable[[dict[str, str]], tuple[str, str]],
    library_games_without_keys: Callable[..., list[dict[str, str]]],
    cached_cover_path_keys_for_games: Callable[[list[dict[str, str]]], set[str]],
    remove_game_files: Callable[[dict[str, str]], bool],
    cleanup_cached_cover_for_game: Callable[[dict[str, str], set[str] | None], bool],
) -> tuple[list[dict[str, str]], bool]:
    if not keys_to_remove:
        return library_games, False

    matching_games = [entry for entry in library_games if game_key(entry) in keys_to_remove]
    if not matching_games:
        return library_games, False

    protected_cache_paths = cached_cover_path_keys_for_games(
        [entry for entry in library_games if game_key(entry) not in keys_to_remove]
    )
    for entry in matching_games:
        if not remove_game_files(entry):
            return library_games, False
        if not cleanup_cached_cover_for_game(entry, protected_cache_paths):
            return library_games, False

    updated_library_games = library_games_without_keys(
        library_games,
        keys_to_remove,
        game_key=game_key,
    )
    return updated_library_games, len(updated_library_games) != len(library_games)
