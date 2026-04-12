from __future__ import annotations

import json
import os
import re
from pathlib import Path
from typing import Callable


_PS3_GAME_ID_RE = re.compile(r"^[A-Z]{4}\d{5}$")
_NPWR_TROPHY_RE = re.compile(r"^NPWR\d{5}$")


def _has_ps3_game_content(directory: Path) -> bool:
    """Return True if directory contains PS3_GAME subdirectory (marks real game content)."""
    try:
        return (directory / "PS3_GAME").is_dir()
    except OSError:
        return False


def _has_game_id_descendant(directory: Path) -> bool:
    """Return True if any subdirectory of directory has a PS3 game-ID name."""
    try:
        for child in directory.rglob("*"):
            if child.is_dir() and _PS3_GAME_ID_RE.match(child.name.upper()):
                return True
    except OSError:
        pass
    return False


def ps3_link_plan_for_extracted_dir(
    extracted_dir: Path,
    emulator_root: Path,
    path_key: Callable[[Path], str],
) -> list[tuple[Path, Path, bool, str]]:
    planned: list[tuple[Path, Path, bool, str]] = []
    seen_targets: set[str] = set()

    try:
        top_entries = sorted(
            extracted_dir.iterdir(),
            key=lambda p: (
                0 if p.is_dir() else 1,
                0 if (_PS3_GAME_ID_RE.match(p.name.upper()) and _has_ps3_game_content(p)) else 1,
                p.name.casefold(),
            ),
        )
    except OSError:
        return planned

    for top_entry in top_entries:
        if not top_entry.is_dir():
            continue
        if _NPWR_TROPHY_RE.match(top_entry.name.upper()):
            target = emulator_root / "dev_hdd0" / "home" / "00000001" / "trophy" / top_entry.name
        else:
            target = emulator_root / top_entry.name
        _plan_ps3_entry(top_entry, target, planned, seen_targets, path_key)

    return planned


def _plan_ps3_entry(
    source: Path,
    target: Path,
    planned: list[tuple[Path, Path, bool, str]],
    seen_targets: set[str],
    path_key: Callable[[Path], str],
) -> None:
    target_key = path_key(target)
    if target_key in seen_targets:
        return

    name_upper = source.name.upper()

    # Game-ID directory -> create a junction here.
    if _PS3_GAME_ID_RE.match(name_upper):
        seen_targets.add(target_key)
        planned.append((source, target, True, "junction"))
        return

    # Existing real directory target should remain real; recurse into children.
    if target.exists() and target.is_dir() and not os.path.islink(str(target)):
        if source.is_dir():
            for child in sorted(source.iterdir(), key=lambda p: (0 if p.is_dir() else 1, p.name.casefold())):
                if child.is_dir():
                    _plan_ps3_entry(child, target / child.name, planned, seen_targets, path_key)
        return

    # If descendants contain a game-ID dir, create target as a real directory and recurse.
    if _has_game_id_descendant(source):
        seen_targets.add(target_key)
        planned.append((source, target, True, "mkdir"))
        if source.is_dir():
            for child in sorted(source.iterdir(), key=lambda p: (0 if p.is_dir() else 1, p.name.casefold())):
                if child.is_dir():
                    _plan_ps3_entry(child, target / child.name, planned, seen_targets, path_key)
    else:
        # No game-ID found below, link whole subtree at this target.
        seen_targets.add(target_key)
        planned.append((source, target, True, "junction"))


def ps3_game_id_from_text(value: str) -> str:
    if not isinstance(value, str):
        return ""
    match = re.search(r"([A-Z]{4}\d{5})", value.upper())
    if match is None:
        return ""
    return match.group(1)


def ps3_game_id_from_paths(paths: list[Path]) -> str:
    for path in paths:
        for part in path.parts:
            game_id = ps3_game_id_from_text(part)
            if game_id and not _NPWR_TROPHY_RE.match(game_id):
                return game_id
    return ""


def detected_ps3_game_id(extracted_dir: Path, link_targets: list[Path]) -> str:
    candidate_paths = [extracted_dir, *link_targets]
    game_id = ps3_game_id_from_paths(candidate_paths)
    if game_id:
        return game_id

    try:
        for candidate in extracted_dir.rglob("*"):
            if not candidate.is_dir():
                continue
            game_id = ps3_game_id_from_text(candidate.name)
            if game_id and _has_ps3_game_content(candidate):
                return game_id
    except OSError:
        pass
    return ""


def ps3_link_paths_from_game(game: dict[str, str]) -> list[Path]:
    raw_value = game.get("ps3_links", "")
    if not isinstance(raw_value, str) or not raw_value.strip():
        return []

    try:
        parsed = json.loads(raw_value)
    except json.JSONDecodeError:
        return []

    if not isinstance(parsed, list):
        return []

    paths: list[Path] = []
    for item in parsed:
        if not isinstance(item, str):
            continue
        item_text = item.strip()
        if item_text:
            paths.append(Path(item_text).expanduser())
    return paths


def ps3_games_yml_path_for_game(
    game: dict[str, str],
    ps3_emulator_root_for_game: Callable[[dict[str, str]], Path | None],
) -> Path | None:
    emulator_root = ps3_emulator_root_for_game(game)
    if emulator_root is None:
        return None
    return emulator_root / "config" / "games.yml"


def ps3_games_yml_paths_for_game(
    game: dict[str, str],
    configured_path: Path | None,
    link_paths: list[Path],
    path_key: Callable[[Path], str],
) -> list[Path]:
    candidates: list[Path] = []
    if configured_path is not None:
        candidates.append(configured_path)

    for link_path in link_paths:
        current = link_path.parent if link_path.name else link_path
        for parent in [current, *current.parents]:
            games_yml_path = parent / "config" / "games.yml"
            if games_yml_path.exists() and games_yml_path.is_file():
                candidates.append(games_yml_path)
                break

    unique: list[Path] = []
    seen: set[str] = set()
    for candidate in candidates:
        key = path_key(candidate)
        if key in seen:
            continue
        seen.add(key)
        unique.append(candidate)
    return unique


def ps3_games_yml_install_path(
    game_id: str,
    link_paths: list[Path],
    extracted_dir: Path,
    emulator_root: Path,
) -> str:
    target_id = game_id.strip().upper()
    for link_path in link_paths:
        parts = list(link_path.parts)
        for index, part in enumerate(parts):
            if part.strip().upper() != target_id:
                continue
            candidate = Path(*parts[: index + 1])
            install_path = candidate.as_posix().rstrip("/")
            return f"{install_path}/" if install_path else ""

    default_candidate = emulator_root / "games" / target_id
    if default_candidate.exists() or not extracted_dir.exists():
        install_path = default_candidate.as_posix().rstrip("/")
        return f"{install_path}/"

    extracted_candidate = extracted_dir / target_id
    install_path = extracted_candidate.as_posix().rstrip("/")
    return f"{install_path}/"


def upsert_rpcs3_games_yml_entry(games_yml_path: Path, game_id: str, install_path: str) -> None:
    entry = f"{game_id}: {install_path}"
    lines: list[str] = []
    if games_yml_path.exists() and games_yml_path.is_file():
        try:
            lines = games_yml_path.read_text(encoding="utf-8").splitlines()
        except OSError as error:
            raise OSError(f"Could not read RPCS3 games file: {games_yml_path}\n{error}") from error

    updated_lines: list[str] = []
    replaced = False
    for line in lines:
        match = re.match(r"^\s*([A-Za-z]{4}\d{5})\s*:\s*.*$", line)
        if match and match.group(1).upper() == game_id:
            if not replaced:
                updated_lines.append(entry)
                replaced = True
            continue
        updated_lines.append(line)

    if not replaced:
        updated_lines.append(entry)

    games_yml_path.parent.mkdir(parents=True, exist_ok=True)
    output_text = "\n".join(updated_lines)
    if output_text:
        if not output_text.endswith("\n"):
            output_text = f"{output_text}\n"
    else:
        output_text = f"{entry}\n"
    try:
        games_yml_path.write_text(output_text, encoding="utf-8")
    except OSError as error:
        raise OSError(f"Could not write RPCS3 games file: {games_yml_path}\n{error}") from error


def ps3_game_ids_for_game(
    game: dict[str, str],
    ps3_game_id_from_text: Callable[[str], str],
    ps3_game_id_for_game: Callable[[dict[str, str]], str],
    link_paths: list[Path],
) -> set[str]:
    game_ids: set[str] = set()

    for key in ("ps3_game_id", "rom_file_name", "title"):
        value = game.get(key, "")
        if not isinstance(value, str):
            continue
        game_id = ps3_game_id_from_text(value)
        if game_id:
            game_ids.add(game_id)

    for link_path in link_paths:
        for part in link_path.parts:
            game_id = ps3_game_id_from_text(part)
            if game_id:
                game_ids.add(game_id)

    fallback = ps3_game_id_for_game(game).strip().upper()
    if ps3_game_id_from_text(fallback):
        game_ids.add(fallback)

    return game_ids


def remove_rpcs3_games_yml_entries(
    games_yml_path: Path,
    game_ids: set[str],
    ps3_game_id_from_text: Callable[[str], str],
) -> None:
    target_ids = {game_id.strip().upper() for game_id in game_ids if ps3_game_id_from_text(game_id.strip().upper())}
    if not target_ids:
        return

    if not games_yml_path.exists() or not games_yml_path.is_file():
        return

    try:
        lines = games_yml_path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise OSError(f"Could not read RPCS3 games file: {games_yml_path}\n{error}") from error

    filtered_lines: list[str] = []
    for line in lines:
        match = re.match(r"^\s*([A-Za-z]{4}\d{5})\s*:\s*(.*)$", line)
        if match is None:
            filtered_lines.append(line)
            continue

        entry_id = match.group(1).upper()
        entry_path = match.group(2).upper()
        path_ids = {path_match.group(1) for path_match in re.finditer(r"\b([A-Z]{4}\d{5})\b", entry_path)}
        if entry_id in target_ids or path_ids.intersection(target_ids):
            continue
        filtered_lines.append(line)

    if filtered_lines == lines:
        return

    output_text = "\n".join(filtered_lines)
    if output_text and not output_text.endswith("\n"):
        output_text = f"{output_text}\n"
    try:
        games_yml_path.write_text(output_text, encoding="utf-8")
    except OSError as error:
        raise OSError(f"Could not write RPCS3 games file: {games_yml_path}\n{error}") from error


def update_rpcs3_games_yml_for_install(
    game: dict[str, str],
    extracted_dir: Path,
    link_paths: list[Path],
    *,
    detected_ps3_game_id: Callable[[Path, list[Path]], str],
    ps3_game_id_from_text: Callable[[str], str],
    ps3_emulator_root_for_game: Callable[[dict[str, str]], Path | None],
    ps3_games_yml_install_path: Callable[[str, list[Path], Path, Path], str],
    ps3_games_yml_path_for_game: Callable[[dict[str, str]], Path | None],
    upsert_rpcs3_games_yml_entry: Callable[[Path, str, str], None],
) -> str:
    game_id = detected_ps3_game_id(extracted_dir, link_paths).strip().upper()
    if not ps3_game_id_from_text(game_id):
        raise OSError("No valid PS3 game ID was found for this title.")

    emulator_root = ps3_emulator_root_for_game(game)
    if emulator_root is None:
        raise OSError("No default PS3 emulator path is configured for PlayStation 3 installs.")

    install_path = ps3_games_yml_install_path(game_id, link_paths, extracted_dir, emulator_root)
    games_yml_path = ps3_games_yml_path_for_game(game)
    if games_yml_path is None:
        raise OSError("Could not resolve RPCS3 config path for PlayStation 3 installs.")

    upsert_rpcs3_games_yml_entry(games_yml_path, game_id, install_path)
    return game_id


def configure_ps3_install_links(
    extracted_dir: Path,
    emulator_root: Path | None,
    ps3_link_plan_for_extracted_dir: Callable[[Path, Path], list[tuple[Path, Path, bool, str]]],
    create_link_path: Callable[[Path, Path, bool], None],
    remove_link_path: Callable[[Path], None],
) -> list[Path]:
    if emulator_root is None:
        raise OSError("No default PS3 emulator path is configured for PlayStation 3 installs.")

    link_plan = ps3_link_plan_for_extracted_dir(extracted_dir, emulator_root)
    if not link_plan:
        raise OSError("No PS3 directory branches could be mapped into the emulator directory.")

    created_links: list[Path] = []
    try:
        for source_path, link_target, is_directory, link_type in link_plan:
            if link_type == "mkdir":
                link_target.mkdir(parents=True, exist_ok=True)
                created_links.append(link_target)
                continue

            create_link_path(source_path, link_target, is_directory)
            created_links.append(link_target)
    except OSError:
        for link_target in reversed(created_links):
            try:
                remove_link_path(link_target)
            except OSError:
                pass
        raise
    return created_links
