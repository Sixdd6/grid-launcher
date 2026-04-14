from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.request
import zipfile
from pathlib import Path
from typing import Callable


_APP_TOOLS_DIR = Path.home() / ".rom-mate" / "tools"
_PORTABLE_7ZR_URL = "https://www.7-zip.org/a/7zr.exe"
_PORTABLE_7ZR_PATH = _APP_TOOLS_DIR / "7zr.exe"
_PORTABLE_7ZZ_URL = "https://www.7-zip.org/a/7z2600-extra.7z"
_PORTABLE_7ZZ_PATH = _APP_TOOLS_DIR / "7zz.exe"
_BUNDLED_7Z_PATH = Path(__file__).resolve().parent.parent.parent / "assets" / "tools" / "7z" / "7z.exe"

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


def _schedule_delete_on_reboot(path: Path) -> bool:
    if os.name != "nt":
        return False
    try:
        import ctypes

        MOVEFILE_DELAY_UNTIL_REBOOT = 0x4
        result = bool(ctypes.windll.kernel32.MoveFileExW(str(path), None, MOVEFILE_DELAY_UNTIL_REBOOT))
        return result
    except Exception:
        return False


def _unlink_file_with_retries(path: Path, *, attempts: int = 20, delay_seconds: float = 0.25) -> None:
    last_error: OSError | None = None
    total_attempts = max(1, int(attempts))
    for attempt_index in range(total_attempts):
        try:
            path.unlink()
            return
        except FileNotFoundError:
            return
        except OSError as error:
            last_error = error
            if attempt_index + 1 >= total_attempts:
                break
            _wait_for_extractor_processes(timeout_seconds=max(0.0, float(delay_seconds)), poll_interval=0.05)
            time.sleep(max(0.0, float(delay_seconds)))

    if last_error is not None and _schedule_delete_on_reboot(path):
        return
    if last_error is not None:
        raise last_error


def _delete_with_background_retry(path: Path, *, initial_wait_seconds: float = 5.0, attempts: int = 60, delay_seconds: float = 1.0) -> None:
    """Spawns a daemon thread to retry deletion silently after AV scanning completes."""
    import threading

    def _try() -> None:
        time.sleep(initial_wait_seconds)
        for attempt_index in range(max(1, attempts)):
            try:
                path.unlink()
                return
            except FileNotFoundError:
                return
            except OSError as error:
                time.sleep(max(0.0, delay_seconds))

    thread = threading.Thread(target=_try, daemon=True)
    thread.start()


def cleanup_install_archive(archive_path: Path) -> str:
    if not archive_path.exists() or not archive_path.is_file():
        return ""
    try:
        _unlink_file_with_retries(archive_path)
    except OSError:
        _delete_with_background_retry(archive_path)
    return ""


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


def _ensure_portable_7z() -> Path | None:
    if sys.platform != "win32":
        return None
    if _PORTABLE_7ZR_PATH.exists():
        return _PORTABLE_7ZR_PATH
    tmp_path = _PORTABLE_7ZR_PATH.with_suffix(".tmp")
    try:
        _APP_TOOLS_DIR.mkdir(parents=True, exist_ok=True)
        urllib.request.urlretrieve(_PORTABLE_7ZR_URL, tmp_path)
        tmp_path.replace(_PORTABLE_7ZR_PATH)
        return _PORTABLE_7ZR_PATH
    except Exception:
        try:
            tmp_path.unlink()
        except Exception:
            pass
        return None


def _ensure_full_7z() -> Path | None:
    if sys.platform != "win32":
        return None
    if _PORTABLE_7ZZ_PATH.exists():
        return _PORTABLE_7ZZ_PATH
    try:
        portable_7zr = _ensure_portable_7z()
        if portable_7zr is None:
            return None
        _APP_TOOLS_DIR.mkdir(parents=True, exist_ok=True)
        tmp_path = _APP_TOOLS_DIR / "7z-extra.tmp"
        try:
            urllib.request.urlretrieve(_PORTABLE_7ZZ_URL, tmp_path)
            result = subprocess.run(
                [str(portable_7zr), "x", str(tmp_path), f"-o{_APP_TOOLS_DIR}", "-y"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
                creationflags=_subprocess_creationflags(),
            )
            if result.returncode != 0:
                return None

            if not _PORTABLE_7ZZ_PATH.exists():
                x64_7zz_path = _APP_TOOLS_DIR / "x64" / "7zz.exe"
                if x64_7zz_path.exists():
                    try:
                        shutil.move(str(x64_7zz_path), str(_PORTABLE_7ZZ_PATH))
                    except OSError:
                        pass

            shutil.rmtree(_APP_TOOLS_DIR / "x64", ignore_errors=True)
            for root_filename in (
                "7za.exe",
                "7zS.sfx",
                "7zSD.sfx",
                "readme.txt",
                "History.txt",
                "License.txt",
                "7-ZipFar.dll",
                "7zS2.sfx",
                "7zS2con.sfx",
            ):
                try:
                    (_APP_TOOLS_DIR / root_filename).unlink()
                except OSError:
                    pass

            if not _PORTABLE_7ZZ_PATH.exists():
                return None
            return _PORTABLE_7ZZ_PATH
        finally:
            try:
                tmp_path.unlink()
            except OSError:
                pass
    except Exception:
        return None


def _subprocess_creationflags() -> int:
    if os.name != "nt":
        return 0
    return int(getattr(subprocess, "CREATE_NO_WINDOW", 0))


def _windows_extractor_processes_running() -> bool:
    if os.name != "nt":
        return False
    try:
        result = subprocess.run(
            ["tasklist", "/FO", "CSV", "/NH"],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=5,
            creationflags=_subprocess_creationflags(),
        )
    except (OSError, subprocess.SubprocessError):
        return False

    output = result.stdout.casefold()
    return any(name in output for name in ("7z.exe", "7za.exe", "7zz.exe", "7zr.exe", "tar.exe"))


def _wait_for_extractor_processes(*, timeout_seconds: float = 3.0, poll_interval: float = 0.15) -> None:
    if os.name != "nt":
        return
    deadline = time.monotonic() + max(0.0, float(timeout_seconds))
    while _windows_extractor_processes_running():
        if time.monotonic() >= deadline:
            return
        time.sleep(max(0.01, float(poll_interval)))


def _run_extractor_process(command: list[str], *, failure_message: str) -> None:
    result = subprocess.run(
        command,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        creationflags=_subprocess_creationflags(),
    )
    _wait_for_extractor_processes(timeout_seconds=1.0, poll_interval=0.1)
    if result.returncode != 0:
        raise OSError(str(result.stderr).strip() or failure_message)


def _try_system_7z(archive_path: Path, extracted_dir: Path) -> bool:
    for cmd in ("7z", "7za", "7zz"):
        try:
            _run_extractor_process(
                [cmd, "x", str(archive_path), f"-o{extracted_dir}", "-y"],
                failure_message=f"{cmd} extraction failed",
            )
            return True
        except FileNotFoundError:
            continue
    return False


def _extract_7z_with_fallbacks(archive_path: Path, extracted_dir: Path) -> None:
    if _BUNDLED_7Z_PATH.exists():
        try:
            _run_extractor_process(
                [str(_BUNDLED_7Z_PATH), "x", str(archive_path), f"-o{extracted_dir}", "-y"],
                failure_message="Bundled 7z extraction failed",
            )
            return
        except (FileNotFoundError, OSError):
            pass
    if _try_system_7z(archive_path, extracted_dir):
        return
    shutil.rmtree(extracted_dir, ignore_errors=True)
    extracted_dir.mkdir(parents=True, exist_ok=True)
    full_7z = _ensure_full_7z()
    if full_7z is not None:
        _run_extractor_process(
            [str(full_7z), "x", str(archive_path), f"-o{extracted_dir}", "-y"],
            failure_message="Portable 7zz extraction failed",
        )
        return
    raise OSError(
        "Cannot extract this archive: no bundled, system, or portable 7-Zip was found. "
        "On Windows, check your internet connection and try again. "
        "On Linux/Mac, install p7zip-full (apt/dnf) or p7zip (brew)."
    )


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
        if archive_path.suffix.casefold() in (".7z", ".rar"):
            if install_progress_callback is not None:
                install_progress_callback(0, 0)
            _extract_7z_with_fallbacks(archive_path, extracted_dir)
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
    except (OSError, zipfile.BadZipFile):
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
                _unlink_file_with_retries(archive_path)
            except OSError as error:
                warning_text = (
                    "Applied PS4 content, but could not delete archive:\n"
                    f"{archive_path}\n{error}"
                )
        return game, warning_text
    finally:
        shutil.rmtree(content_extract_dir, ignore_errors=True)


def apply_xenia_content_archive_without_ui(
    archive_path: Path,
    content_root: Path,
    *,
    extracted_dir_for_archive_path: Callable[[Path], Path],
    extract_archive_into_directory: Callable[[Path, Path, Callable[[int, int], None] | None], None],
    install_progress_callback: Callable[[int, int], None] | None = None,
) -> tuple[list[dict[str, object]], str]:
    """Extract archive and install all STFS packages found inside to the xenia content directory.

    Returns (results_list, warning_text). Each result is the dict from apply_xenia_content_without_ui.
    """
    from rom_mate.emulator.xenia import apply_xenia_content_without_ui as _apply_stfs

    extract_dir = extracted_dir_for_archive_path(archive_path)
    try:
        extract_archive_into_directory(archive_path, extract_dir, install_progress_callback)
    except Exception as exc:
        return [], str(exc)

    try:
        results: list[dict[str, object]] = []
        errors: list[str] = []
        for file_path in sorted(extract_dir.rglob("*")):
            if not file_path.is_file():
                continue
            result = _apply_stfs(file_path, content_root)
            if result["error"]:
                errors.append(str(result["error"]))
            else:
                results.append(result)

        if errors and not results:
            return [], "\n".join(errors)
        warning_text = "\n".join(errors) if errors else ""
        return results, warning_text
    finally:
        shutil.rmtree(str(extract_dir), ignore_errors=True)


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
        return archive_path.suffix.casefold() in {".zip", ".7z", ".rar", ".tar", ".gz", ".bz2", ".xz", ".iso"}
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
        ".xex",
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
    should_extract_archive_for_game: Callable[[dict[str, str], Path], bool],
    extract_archive_for_game: Callable[[dict[str, str], Path, Callable[[int, int], None] | None], tuple[Path, Path]],
    is_ps3_platform: Callable[[dict[str, str]], bool],
    ps3_dev_hdd0_root: Callable[[dict[str, str]], Path | None] | None = None,
    ps3_games_root: Callable[[dict[str, str]], Path | None] | None = None,
    ps3_rpcs3_data_root: Callable[[dict[str, str]], Path | None] | None = None,
    cleanup_archive_on_success: bool = True,
    install_progress_callback: Callable[[int, int], None] | None = None,
) -> tuple[dict[str, str] | None, str]:
    prepared = dict(game)
    prepared["extracted_path"] = ""
    prepared["extracted_dir"] = ""
    prepared["ps3_game_id"] = ""
    prepared["ps3_trophy_paths"] = ""
    prepared["ps4_game_id"] = ""
    if not should_extract_archive_for_game(prepared, archive_path):
        return prepared, ""

    try:
        if is_ps3_platform(prepared):
            extracted_dir = extracted_dir_for_archive_path(archive_path)
            extract_archive_into_directory(
                archive_path,
                extracted_dir,
                install_progress_callback=install_progress_callback,
            )
            has_extracted_files = any(candidate.is_file() for candidate in extracted_dir.rglob("*"))
            if not has_extracted_files:
                shutil.rmtree(extracted_dir, ignore_errors=True)
                raise OSError("Archive extracted but no ROM file was found")
            extracted_file = extracted_dir
        else:
            extracted_file, extracted_dir = extract_archive_for_game(
                prepared,
                archive_path,
                install_progress_callback,
            )
    except (OSError, zipfile.BadZipFile) as error:
        return None, str(error)

    warning_text = ""
    if cleanup_archive_on_success:
        cleanup_error = cleanup_install_archive(archive_path)
        if cleanup_error:
            warning_text = (
                f"Extracted {prepared.get('title', 'Game')}, but could not delete archive:\n{cleanup_error}"
            )

    prepared["extracted_path"] = str(extracted_file)
    prepared["extracted_dir"] = str(extracted_dir)
    if _is_ps4_platform(prepared):
        prepared["ps4_game_id"] = _detected_ps4_game_id_for_layout(extracted_dir, extracted_file, archive_path)

    if is_ps3_platform(prepared):
        from rom_mate.library.ps3_install import (
            extract_iso_to_ps3_layout,
            ps3_route_extracted_contents,
        )
        dev_hdd0_root = ps3_dev_hdd0_root(prepared) if callable(ps3_dev_hdd0_root) else None
        if dev_hdd0_root is None:
            return None, f"No PS3 VFS dev_hdd0 path configured for {prepared.get('title', 'Game')}"
        games_root = ps3_games_root(prepared) if callable(ps3_games_root) else None
        rpcs3_data_root = ps3_rpcs3_data_root(prepared) if callable(ps3_rpcs3_data_root) else None
        try:
            game_id, installed_paths = ps3_route_extracted_contents(
                extracted_dir,
                dev_hdd0_root,
                extract_iso_to_ps3_layout,
                games_root=games_root,
                rpcs3_data_root=rpcs3_data_root,
            )
            if not game_id:
                return None, f"No PS3 game ID found in archive for {prepared.get('title', 'Game')}"
            prepared["ps3_game_id"] = game_id
            game_install_dir = next(
                (
                    path for path in installed_paths
                    if path.name.upper() == game_id.upper()
                ),
                dev_hdd0_root / "game" / game_id,
            )
            prepared["extracted_path"] = str(game_install_dir)
            prepared["extracted_dir"] = str(game_install_dir)
            trophy_paths = [
                p for p in installed_paths
                if "trophy" in str(p).casefold()
            ]
            prepared["ps3_trophy_paths"] = json.dumps([str(p) for p in trophy_paths])
            shutil.rmtree(extracted_dir, ignore_errors=True)
        except OSError as error:
            return None, f"Failed to install PS3 game {prepared.get('title', 'Game')}: {error}"

    return prepared, warning_text


def merge_archive_into_directory(
    archive_path: Path,
    target_dir: Path,
    temp_dir: Path,
    install_progress_callback: Callable[[int, int], None] | None = None,
) -> None:
    """Extract archive into temp_dir (same filesystem as target_dir), then merge into target_dir.

    Files already present in target_dir that are NOT in the archive are left untouched.
    The temp_dir is always removed on completion, success or failure.

    Args:
        archive_path: Path to the archive file (.zip, .7z, .tar, etc.)
        target_dir: Existing game install directory to merge updates into.
        temp_dir: Temporary extraction directory. Must be on the same filesystem as target_dir
                  so that file moves are atomic and fast. Caller is responsible for choosing
                  a suitable location (e.g. target_dir.parent / (target_dir.name + "-temp")).
                  Must not already exist; will be created and deleted by this function.
    """
    if temp_dir.exists():
        shutil.rmtree(temp_dir, ignore_errors=True)
    try:
        extract_archive_into_directory(archive_path, temp_dir, install_progress_callback)
        _merge_tree(temp_dir, target_dir)
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


def prepare_native_game_update_without_ui(
    installed_game: dict[str, str],
    update_game: dict[str, str],
    archive_path: Path,
    *,
    temp_dir_for_game: Callable[[dict[str, str]], Path],
    select_extracted_launch_file: Callable[[dict[str, str], Path, Path], Path | None],
    install_progress_callback: Callable[[int, int], None] | None = None,
) -> tuple[dict[str, str] | None, str]:
    """Merge a new archive into an existing native game install directory.

    Preserves all files in the install directory that are not present in the new archive
    (saves, configs, keybindings, etc.). Only files delivered by the new archive are
    updated or added.

    Args:
        installed_game: The currently installed game record (has extracted_dir, extracted_path, etc.)
        update_game: The server game dict for the new version (has rom_file_name, server_updated_at, etc.)
        archive_path: Path to the downloaded update archive.
        temp_dir_for_game: Callable that returns the temp extraction directory for the game.
                           Should return a path on the same filesystem as extracted_dir.
        select_extracted_launch_file: Callable to detect the primary executable from extracted files.
        install_progress_callback: Optional progress callback (bytes_done, bytes_total).

    Returns:
        (prepared_game_dict, warning_text) on success, or (None, error_string) on failure.
    """
    prepared = dict(installed_game)

    # Merge server-side metadata from the update into the prepared record.
    for field in (
        "rom_id",
        "rom_file_name",
        "server_updated_at",
        "description",
        "rating",
        "genres",
        "regions",
        "filesize_bytes",
        "screenshot_urls",
        "ra_id",
    ):
        value = update_game.get(field, "")
        if value:
            prepared[field] = value

    extracted_dir_text = prepared.get("extracted_dir", "")
    if not isinstance(extracted_dir_text, str) or not extracted_dir_text.strip():
        return None, "Installed game directory not found - reinstall the game and try again."

    extracted_dir = Path(extracted_dir_text.strip())
    if not extracted_dir.exists() or not extracted_dir.is_dir():
        return None, f"Installed game directory does not exist: {extracted_dir}"

    temp_dir = temp_dir_for_game(prepared)

    try:
        merge_archive_into_directory(
            archive_path,
            extracted_dir,
            temp_dir,
            install_progress_callback,
        )
    except (OSError, zipfile.BadZipFile) as error:
        return None, str(error)

    # Re-detect the launch file in case the executable name changed.
    new_launch_file = select_extracted_launch_file(prepared, extracted_dir, archive_path)
    if new_launch_file is not None:
        # Respect manual executable overrides when present.
        manual_exe = prepared.get("native_executable_path", "").strip()
        if not manual_exe:
            prepared["extracted_path"] = str(new_launch_file)

    warning_text = ""
    if archive_path.exists() and archive_path.is_file():
        try:
            _unlink_file_with_retries(archive_path)
        except OSError as error:
            warning_text = (
                f"Updated {prepared.get('title', 'Game')}, but could not delete archive:\n"
                f"{archive_path}\n{error}"
            )

    return prepared, warning_text
