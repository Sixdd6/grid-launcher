from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import time
import zipfile
from pathlib import Path
from typing import Callable

try:
    import py7zr
    from py7zr.exceptions import Bad7zFile
except ImportError:  # pragma: no cover - dependency is declared in requirements.
    py7zr = None

    class Bad7zFile(Exception):
        pass


_PS4_GAME_ID_PATTERN = re.compile(r"^[A-Z]{4}\d{5}$")


def _is_ps4_platform(game: dict[str, str]) -> bool:
    platform_value = game.get("platform", "")
    platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
    normalized = re.sub(r"[^a-z0-9]+", " ", platform).strip()
    compact = normalized.replace(" ", "")
    tokens = set(normalized.split())

    if not normalized:
        return False
    if normalized in {"playstation 4", "ps4"}:
        return True
    if "ps4" in tokens:
        return True
    return "playstation4" in compact


def _ps4_game_id_from_text(value: str) -> str:
    cleaned = re.sub(r"[^A-Za-z0-9]", "", value or "").upper()
    if _PS4_GAME_ID_PATTERN.fullmatch(cleaned):
        return cleaned
    return ""


def _select_ps4_launch_file(extracted_dir: Path, pool: list[Path]) -> Path | None:
    eboot_candidates = [candidate for candidate in pool if candidate.name.casefold() == "eboot.bin"]
    if not eboot_candidates:
        return None

    top_level_ids: set[str] = set()
    try:
        for child in extracted_dir.iterdir():
            if not child.is_dir():
                continue
            game_id = _ps4_game_id_from_text(child.name)
            if game_id:
                top_level_ids.add(game_id)
    except OSError:
        top_level_ids = set()

    def _candidate_sort_key(candidate: Path) -> tuple[int, int, str]:
        try:
            relative_parts = [part for part in candidate.relative_to(extracted_dir).parts]
        except ValueError:
            relative_parts = list(candidate.parts)

        relative_ids = {
            game_id
            for game_id in (_ps4_game_id_from_text(part) for part in relative_parts[:-1])
            if game_id
        }
        top_level_match_rank = 0 if top_level_ids and relative_ids.intersection(top_level_ids) else 1
        return (top_level_match_rank, len(relative_parts), str(candidate).casefold())

    eboot_candidates.sort(key=_candidate_sort_key)
    return eboot_candidates[0]


def _detected_ps4_game_id_for_layout(extracted_dir: Path, launch_file: Path, archive_path: Path) -> str:
    try:
        relative_parts = launch_file.relative_to(extracted_dir).parts
    except ValueError:
        relative_parts = tuple()

    for part in relative_parts[:-1]:
        game_id = _ps4_game_id_from_text(part)
        if game_id:
            return game_id

    try:
        for child in extracted_dir.iterdir():
            if not child.is_dir():
                continue
            game_id = _ps4_game_id_from_text(child.name)
            if game_id:
                return game_id
    except OSError:
        pass

    for parent in launch_file.parents:
        game_id = _ps4_game_id_from_text(parent.name)
        if game_id:
            return game_id
        if parent == extracted_dir:
            break

    return _ps4_game_id_from_text(archive_path.stem)


def _ps4_title_id_roots(directory: Path) -> list[Path]:
    roots: list[Path] = []
    try:
        for child in directory.iterdir():
            if not child.is_dir():
                continue
            if _ps4_game_id_from_text(child.name):
                roots.append(child)
    except OSError:
        return []
    roots.sort(key=lambda candidate: candidate.name.casefold())
    return roots


def _detected_ps4_game_id_from_installed_game(game: dict[str, str]) -> str:
    explicit = _ps4_game_id_from_text(game.get("ps4_game_id", ""))
    if explicit:
        return explicit

    extracted_path_text = game.get("extracted_path", "")
    if isinstance(extracted_path_text, str) and extracted_path_text.strip():
        extracted_path = Path(extracted_path_text.strip())
        for parent in extracted_path.parents:
            game_id = _ps4_game_id_from_text(parent.name)
            if game_id:
                return game_id

    extracted_dir_text = game.get("extracted_dir", "")
    if isinstance(extracted_dir_text, str) and extracted_dir_text.strip():
        extracted_dir = Path(extracted_dir_text.strip())
        roots = _ps4_title_id_roots(extracted_dir)
        if roots:
            return roots[0].name.upper()

    return ""


def _read_ps4_content_entries(value: str) -> list[dict[str, str]]:
    if not isinstance(value, str) or not value.strip():
        return []
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError:
        return []
    if not isinstance(parsed, list):
        return []

    entries: list[dict[str, str]] = []
    for item in parsed:
        if not isinstance(item, dict):
            continue
        normalized: dict[str, str] = {}
        for key, item_value in item.items():
            if not isinstance(key, str):
                continue
            if isinstance(item_value, str):
                normalized[key] = item_value.strip()
            else:
                normalized[key] = str(item_value)
        if normalized:
            entries.append(normalized)
    return entries


def _write_ps4_content_entries(entries: list[dict[str, str]]) -> str:
    return json.dumps(entries, separators=(",", ":"))


def _merge_tree(source_dir: Path, destination_dir: Path) -> None:
    destination_dir.mkdir(parents=True, exist_ok=True)
    for source in source_dir.rglob("*"):
        relative_path = source.relative_to(source_dir)
        target = destination_dir / relative_path
        if source.is_dir():
            target.mkdir(parents=True, exist_ok=True)
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)


def extract_archive_into_directory(
    archive_path: Path,
    extracted_dir: Path,
    install_progress_callback: Callable[[int, int], None] | None = None,
) -> None:
    if extracted_dir.exists():
        if extracted_dir.is_dir():
            shutil.rmtree(extracted_dir, ignore_errors=True)
        else:
            try:
                extracted_dir.unlink()
            except OSError:
                pass
    extracted_dir.mkdir(parents=True, exist_ok=True)

    try:
        if archive_path.suffix.casefold() == ".7z":
            if py7zr is None:
                raise OSError("Cannot extract .7z archives because py7zr is not installed")
            if install_progress_callback is not None:
                install_progress_callback(0, 0)
            with py7zr.SevenZipFile(archive_path, mode="r") as archive:
                archive.extractall(path=extracted_dir)
            if install_progress_callback is not None:
                installed_bytes = directory_total_file_bytes(extracted_dir)
                install_progress_callback(installed_bytes, installed_bytes)
        elif zipfile.is_zipfile(archive_path):
            with zipfile.ZipFile(archive_path) as archive:
                members = archive.infolist()
                total_install_bytes = sum(max(0, int(member.file_size)) for member in members if not member.is_dir())
                installed_bytes = 0
                if install_progress_callback is not None:
                    install_progress_callback(installed_bytes, total_install_bytes)
                for member in members:
                    archive.extract(member, extracted_dir)
                    if member.is_dir():
                        continue
                    installed_bytes += max(0, int(member.file_size))
                    if install_progress_callback is not None:
                        install_progress_callback(installed_bytes, total_install_bytes)
        else:
            total_install_bytes = tar_archive_total_install_bytes(archive_path)
            if install_progress_callback is not None:
                install_progress_callback(0, total_install_bytes)
            process = subprocess.Popen(
                ["tar", "-xf", str(archive_path), "-C", str(extracted_dir)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
            )
            while process.poll() is None:
                if install_progress_callback is not None:
                    installed_bytes = directory_total_file_bytes(extracted_dir)
                    install_progress_callback(min(installed_bytes, total_install_bytes), total_install_bytes)
                time.sleep(0.15)
            stderr_text = ""
            if process.stderr is not None:
                stderr_text = process.stderr.read().strip()
                process.stderr.close()
            if process.returncode != 0:
                raise OSError(stderr_text or "Unknown extraction error")
            if install_progress_callback is not None:
                installed_bytes = directory_total_file_bytes(extracted_dir)
                resolved_total = max(total_install_bytes, installed_bytes)
                install_progress_callback(installed_bytes, resolved_total)
    except (OSError, zipfile.BadZipFile, Bad7zFile):
        shutil.rmtree(extracted_dir, ignore_errors=True)
        raise


def apply_ps4_content_archive_without_ui(
    installed_game: dict[str, str],
    archive_path: Path,
    *,
    content_kind: str = "content",
    extracted_dir_for_archive_path: Callable[[Path], Path],
    extract_archive_into_directory: Callable[[Path, Path, Callable[[int, int], None] | None], None],
    install_progress_callback: Callable[[int, int], None] | None = None,
) -> tuple[dict[str, str] | None, str]:
    game = dict(installed_game)
    if not _is_ps4_platform(game):
        return None, "PS4 content apply is only supported for PS4 games"

    expected_game_id = _detected_ps4_game_id_from_installed_game(game)
    if not expected_game_id:
        return None, "Installed PS4 game is missing a detectable title ID"

    extracted_dir_text = game.get("extracted_dir", "")
    extracted_dir_value = extracted_dir_text.strip() if isinstance(extracted_dir_text, str) else ""
    if not extracted_dir_value:
        return None, "Installed PS4 game is missing an extracted install directory"

    installed_root = Path(extracted_dir_value)
    if not installed_root.exists() or not installed_root.is_dir():
        return None, f"Installed PS4 directory does not exist: {installed_root}"

    target_title_dir = installed_root / expected_game_id
    if not target_title_dir.exists() or not target_title_dir.is_dir():
        return None, f"Installed PS4 title directory was not found: {target_title_dir}"

    content_extract_dir = extracted_dir_for_archive_path(archive_path)
    try:
        extract_archive_into_directory(
            archive_path,
            content_extract_dir,
            install_progress_callback,
        )
    except (OSError, zipfile.BadZipFile) as error:
        return None, str(error)

    try:
        content_roots = _ps4_title_id_roots(content_extract_dir)
        if not content_roots:
            return None, "PS4 content archive must include a title-ID root folder"

        matching_roots = [candidate for candidate in content_roots if candidate.name.upper() == expected_game_id]
        if not matching_roots:
            detected_ids = ", ".join(candidate.name.upper() for candidate in content_roots)
            return None, (
                "PS4 content title ID mismatch: "
                f"expected {expected_game_id}, archive contains {detected_ids or 'unknown'}"
            )

        source_title_dir = matching_roots[0]
        try:
            _merge_tree(source_title_dir, target_title_dir)
        except OSError as error:
            return None, f"Failed to merge PS4 content into installed game: {error}"

        metadata_entries = _read_ps4_content_entries(game.get("ps4_content", ""))
        metadata_entries.append(
            {
                "kind": content_kind.strip().lower() or "content",
                "title_id": expected_game_id,
                "archive_name": archive_path.name,
                "applied_at": str(int(time.time())),
            }
        )
        game["ps4_content"] = _write_ps4_content_entries(metadata_entries)
        game["ps4_game_id"] = expected_game_id

        warning_text = ""
        if archive_path.exists() and archive_path.is_file():
            try:
                archive_path.unlink()
            except OSError as error:
                warning_text = (
                    "Applied PS4 content, but could not delete archive:\n"
                    f"{archive_path}\n{error}"
                )
        return game, warning_text
    finally:
        shutil.rmtree(content_extract_dir, ignore_errors=True)


def should_extract_archive_for_game(
    game: dict[str, str],
    archive_path: Path,
    *,
    is_native_executable_platform: Callable[[dict[str, str]], bool],
    is_arcade_platform: Callable[[dict[str, str]], bool],
    is_ps3_platform: Callable[[dict[str, str]], bool],
) -> bool:
    if is_native_executable_platform(game):
        return True
    if is_arcade_platform(game):
        return False
    if is_ps3_platform(game):
        return archive_path.suffix.casefold() in {".zip", ".7z", ".rar", ".tar", ".gz", ".bz2", ".xz"}
    return archive_path.suffix.casefold() in {".7z", ".zip"}


def extracted_dir_for_archive_path(archive_path: Path) -> Path:
    extracted_name = archive_path.stem or archive_path.name
    extracted_dir = archive_path.parent / extracted_name
    if extracted_dir == archive_path or (extracted_dir.exists() and extracted_dir.is_file()):
        return archive_path.parent / f"{extracted_name}_extracted"
    return extracted_dir


def select_extracted_launch_file(
    game: dict[str, str],
    extracted_dir: Path,
    archive_path: Path,
    *,
    is_ps3_platform: Callable[[dict[str, str]], bool],
) -> Path | None:
    files = [candidate for candidate in extracted_dir.rglob("*") if candidate.is_file()]
    if not files:
        return None

    archive_suffixes = {".zip", ".7z", ".rar", ".tar", ".gz", ".bz2", ".xz"}
    non_archive_files = [candidate for candidate in files if candidate.suffix.casefold() not in archive_suffixes]
    pool = non_archive_files if non_archive_files else files

    preferred_extensions = [
        ".m3u",
        ".cue",
        ".chd",
        ".iso",
        ".bin",
        ".pbp",
        ".cso",
        ".img",
        ".ccd",
        ".nrg",
        ".mdf",
        ".gdi",
        ".rvz",
        ".gcz",
        ".wbfs",
        ".gcm",
        ".dol",
        ".elf",
        ".nes",
        ".fds",
        ".sfc",
        ".smc",
        ".gba",
        ".gb",
        ".gbc",
        ".n64",
        ".z64",
        ".v64",
        ".nds",
        ".3ds",
        ".cia",
        ".xci",
        ".nsp",
        ".gen",
        ".smd",
        ".md",
        ".32x",
        ".sms",
        ".gg",
        ".pce",
        ".sgx",
        ".a26",
        ".a52",
        ".a78",
        ".lnx",
        ".ws",
        ".wsc",
        ".ngp",
        ".ngc",
        ".jag",
        ".rom",
    ]
    if is_ps3_platform(game):
        preferred_extensions = [".pkg", *preferred_extensions]
    extension_priority = {extension: index for index, extension in enumerate(preferred_extensions)}
    support_extensions = {
        ".txt",
        ".nfo",
        ".diz",
        ".log",
        ".json",
        ".xml",
        ".ini",
        ".cfg",
        ".conf",
        ".url",
        ".pdf",
        ".html",
        ".htm",
        ".png",
        ".jpg",
        ".jpeg",
        ".gif",
        ".bmp",
        ".webp",
        ".svg",
        ".ico",
        ".dll",
        ".so",
        ".dylib",
        ".py",
        ".lua",
        ".js",
        ".css",
        ".db",
        ".sqlite",
        ".tmp",
        ".cache",
        ".sav",
        ".srm",
        ".state",
        ".states",
        ".cht",
        ".slangp",
        ".slang",
        ".glsl",
        ".vert",
        ".frag",
    }
    support_directories = {
        "__macosx",
        "glcache",
        "cache",
        "caches",
        "shadercache",
        "shaders",
        "docs",
        "doc",
        "manual",
        "manuals",
        "readme",
        "licenses",
        "license",
        "resources",
    }

    archive_stem = archive_path.stem.casefold()

    if _is_ps4_platform(game):
        ps4_launch_file = _select_ps4_launch_file(extracted_dir, pool)
        if ps4_launch_file is not None:
            return ps4_launch_file

    def _candidate_sort_key(candidate: Path) -> tuple[int, int, int, int, str]:
        try:
            relative_parts = [part.casefold() for part in candidate.relative_to(extracted_dir).parts]
        except ValueError:
            relative_parts = [part.casefold() for part in candidate.parts]

        suffix = candidate.suffix.casefold()
        support_dir_penalty = 1 if any(part in support_directories for part in relative_parts[:-1]) else 0
        support_ext_penalty = 1 if suffix in support_extensions else 0
        extension_rank = extension_priority.get(suffix, len(extension_priority) + 10)
        stem = candidate.stem.casefold()
        stem_rank = 0 if stem == archive_stem else 1
        return (
            support_dir_penalty + support_ext_penalty,
            extension_rank,
            stem_rank,
            len(relative_parts),
            str(candidate).casefold(),
        )

    playable_candidates = [candidate for candidate in pool if candidate.suffix.casefold() in extension_priority]
    if playable_candidates:
        playable_candidates.sort(key=_candidate_sort_key)
        return playable_candidates[0]

    non_support_candidates = [candidate for candidate in pool if _candidate_sort_key(candidate)[0] == 0]
    selection_pool = non_support_candidates if non_support_candidates else pool

    stem_matches = [candidate for candidate in selection_pool if candidate.stem.casefold() == archive_stem]
    if stem_matches:
        stem_matches.sort(key=_candidate_sort_key)
        return stem_matches[0]

    selection_pool.sort(key=_candidate_sort_key)
    return selection_pool[0]


def directory_total_file_bytes(directory: Path) -> int:
    total = 0
    if not directory.exists() or not directory.is_dir():
        return 0
    for root, _, files in os.walk(directory):
        root_path = Path(root)
        for name in files:
            candidate = root_path / name
            try:
                if candidate.exists() and candidate.is_file():
                    total += max(0, int(candidate.stat().st_size))
            except OSError:
                continue
    return total


def tar_listing_line_size(line: str) -> int:
    parts = line.split()
    if len(parts) < 4:
        return 0
    for index, token in enumerate(parts[:-1]):
        if not token.isdigit():
            continue
        next_token = parts[index + 1] if index + 1 < len(parts) else ""
        if re.fullmatch(r"\d{4}-\d{2}-\d{2}", next_token):
            return max(0, int(token))
        if re.fullmatch(r"[A-Za-z]{3}", next_token):
            return max(0, int(token))
    return 0


def tar_archive_total_install_bytes(archive_path: Path) -> int:
    try:
        result = subprocess.run(
            ["tar", "-tvf", str(archive_path)],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return 0
    if result.returncode != 0:
        return 0
    total = 0
    for raw_line in result.stdout.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("tar:"):
            continue
        size = tar_listing_line_size(line)
        if size > 0:
            total += size
    return total


def extract_archive_for_game(
    game: dict[str, str],
    archive_path: Path,
    *,
    extracted_dir_for_archive_path: Callable[[Path], Path],
    select_extracted_launch_file: Callable[[dict[str, str], Path, Path], Path | None],
    install_progress_callback: Callable[[int, int], None] | None = None,
) -> tuple[Path, Path]:
    extracted_dir = extracted_dir_for_archive_path(archive_path)
    extract_archive_into_directory(
        archive_path,
        extracted_dir,
        install_progress_callback=install_progress_callback,
    )

    launch_file = select_extracted_launch_file(game, extracted_dir, archive_path)
    if launch_file is None:
        shutil.rmtree(extracted_dir, ignore_errors=True)
        raise OSError("Archive extracted but no ROM file was found")

    return launch_file, extracted_dir


def prepare_installed_game_without_ui(
    game: dict[str, str],
    archive_path: Path,
    *,
    configure_ps3_links: bool,
    should_extract_archive_for_game: Callable[[dict[str, str], Path], bool],
    extract_archive_for_game: Callable[[dict[str, str], Path, Callable[[int, int], None] | None], tuple[Path, Path]],
    is_ps3_platform: Callable[[dict[str, str]], bool],
    configure_ps3_install_links: Callable[[dict[str, str], Path], list[Path]],
    update_rpcs3_games_yml_for_install: Callable[[dict[str, str], Path, list[Path]], str],
    install_progress_callback: Callable[[int, int], None] | None = None,
) -> tuple[dict[str, str] | None, str]:
    prepared = dict(game)
    prepared["extracted_path"] = ""
    prepared["extracted_dir"] = ""
    prepared["ps3_links"] = ""
    prepared["ps3_game_id"] = ""
    prepared["ps4_game_id"] = ""
    if not should_extract_archive_for_game(prepared, archive_path):
        return prepared, ""

    try:
        extracted_file, extracted_dir = extract_archive_for_game(
            prepared,
            archive_path,
            install_progress_callback,
        )
    except (OSError, zipfile.BadZipFile) as error:
        return None, str(error)

    warning_text = ""
    if archive_path.exists() and archive_path.is_file():
        try:
            archive_path.unlink()
        except OSError as error:
            warning_text = (
                f"Extracted {prepared.get('title', 'Game')}, but could not delete archive:\n{archive_path}\n{error}"
            )

    prepared["extracted_path"] = str(extracted_file)
    prepared["extracted_dir"] = str(extracted_dir)
    if _is_ps4_platform(prepared):
        prepared["ps4_game_id"] = _detected_ps4_game_id_for_layout(extracted_dir, extracted_file, archive_path)

    if is_ps3_platform(prepared) and configure_ps3_links:
        try:
            ps3_links = configure_ps3_install_links(prepared, extracted_dir)
            prepared["ps3_links"] = json.dumps([str(path) for path in ps3_links])
            prepared["ps3_game_id"] = update_rpcs3_games_yml_for_install(prepared, extracted_dir, ps3_links)
        except OSError as error:
            return None, f"Failed to prepare PS3 symlink layout for {prepared.get('title', 'Game')}: {error}"

    return prepared, warning_text
