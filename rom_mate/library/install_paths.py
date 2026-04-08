from __future__ import annotations

from pathlib import Path
from typing import Callable


def _unique_paths(paths: list[Path]) -> list[Path]:
    unique: list[Path] = []
    seen: set[str] = set()
    for candidate in paths:
        key = str(candidate)
        if key in seen:
            continue
        seen.add(key)
        unique.append(candidate)
    return unique


def candidate_archive_paths_for_game(
    game: dict[str, str],
    platform_library_dir: Callable[[dict[str, str]], Path | None],
    archive_name_for_game: Callable[[dict[str, str]], str],
    library_path_dir: Callable[[], Path | None],
) -> list[Path]:
    candidates: list[Path] = []
    archive_path_value = game.get("archive_path", "")
    if isinstance(archive_path_value, str) and archive_path_value.strip():
        candidates.append(Path(archive_path_value).expanduser())

    platform_library_path = platform_library_dir(game)
    if platform_library_path is not None:
        candidates.append(platform_library_path / archive_name_for_game(game))

    library_path = library_path_dir()
    if library_path is not None:
        candidates.append(library_path / archive_name_for_game(game))

    return _unique_paths(candidates)


def candidate_extracted_paths_for_game(
    game: dict[str, str],
    select_extracted_launch_file: Callable[[dict[str, str], Path, Path], Path | None],
) -> list[Path]:
    candidates: list[Path] = []
    extracted_dir_value = game.get("extracted_dir", "")
    if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
        extracted_dir = Path(extracted_dir_value).expanduser()
        if extracted_dir.exists() and extracted_dir.is_dir():
            selected = select_extracted_launch_file(game, extracted_dir, Path(game.get("archive_path", "") or "archive"))
            if selected is not None:
                candidates.append(selected)

    extracted_path_value = game.get("extracted_path", "")
    if isinstance(extracted_path_value, str) and extracted_path_value.strip():
        extracted_path = Path(extracted_path_value).expanduser()
        if extracted_path.exists() and extracted_path.is_file():
            candidates.append(extracted_path)

    return _unique_paths(candidates)


def candidate_extracted_dirs_for_game(
    game: dict[str, str],
    candidate_archive_paths: list[Path],
    extracted_dir_for_archive_path: Callable[[Path], Path],
) -> list[Path]:
    candidates: list[Path] = []
    extracted_dir_value = game.get("extracted_dir", "")
    if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
        candidates.append(Path(extracted_dir_value).expanduser())

    for archive_path in candidate_archive_paths:
        candidates.append(extracted_dir_for_archive_path(archive_path))

    return _unique_paths(candidates)


def native_install_dir_for_game(
    game: dict[str, str],
    candidate_archive_paths: list[Path],
) -> Path | None:
    extracted_dir_value = game.get("extracted_dir", "")
    if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
        extracted_dir = Path(extracted_dir_value).expanduser()
        if extracted_dir.exists() and extracted_dir.is_dir():
            return extracted_dir

    extracted_path_value = game.get("extracted_path", "")
    if isinstance(extracted_path_value, str) and extracted_path_value.strip():
        extracted_path = Path(extracted_path_value).expanduser()
        if extracted_path.exists() and extracted_path.is_file():
            return extracted_path.parent

    for archive_path in candidate_archive_paths:
        if archive_path.exists() and archive_path.is_file():
            return archive_path.parent
    return None


def native_executable_candidates_for_game(
    install_dir: Path | None,
    launchable_native_game_file: Callable[[Path], bool],
) -> list[Path]:
    if install_dir is None:
        return []

    candidates = [
        candidate
        for candidate in install_dir.rglob("*")
        if candidate.is_file() and launchable_native_game_file(candidate)
    ]
    candidates.sort(key=lambda candidate: (len(candidate.parts), str(candidate).casefold()))
    return candidates


def resolved_native_executable_path_for_game(
    game: dict[str, str],
    executable_candidates: list[Path],
    launchable_native_game_file: Callable[[Path], bool],
) -> Path | None:
    selected_value = game.get("native_executable_path", "")
    selected_path_text = selected_value.strip() if isinstance(selected_value, str) else ""
    if selected_path_text:
        selected_path = Path(selected_path_text).expanduser()
        if selected_path.exists() and selected_path.is_file() and launchable_native_game_file(selected_path):
            return selected_path

    if executable_candidates:
        return executable_candidates[0]
    return None
