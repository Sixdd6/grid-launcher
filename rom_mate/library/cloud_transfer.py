from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any, Callable
from urllib.parse import parse_qsl, quote, urlencode, urlsplit, urlunsplit


DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES = {
    ".ds_store",
    "desktop.ini",
    "ehthumbs.db",
    "thumbs.db",
}
SUPPORTED_IMAGE_EXTENSIONS = (
    ".jpg",
    ".jpeg",
    ".png",
    ".webp",
    ".gif",
    ".bmp",
)

_BUNDLED_7Z_PATH = Path(__file__).resolve().parent.parent.parent / "assets" / "tools" / "7z" / "7z.exe"


def _normalized_blocked_basenames(values: set[str] | None = None) -> set[str]:
    blocked = set(DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES)
    blocked.update(
        name.casefold()
        for name in (values or set())
        if isinstance(name, str) and name.strip()
    )
    return blocked


def _normalized_blocked_extensions(values: set[str] | None = None) -> set[str]:
    return {
        extension.casefold()
        for extension in (values or set())
        if isinstance(extension, str) and extension.strip()
    }


def supported_image_sidecar_path(
    file_path: Path,
    *,
    blocked_basenames: set[str] | None = None,
) -> Path | None:
    blocked_names = _normalized_blocked_basenames(blocked_basenames)
    for extension in SUPPORTED_IMAGE_EXTENSIONS:
        candidate = file_path.with_suffix(extension)
        if candidate.name.casefold() in blocked_names:
            continue
        if candidate.exists() and candidate.is_file():
            return candidate
    return None


def appended_image_sidecar_path(
    file_path: Path,
    *,
    blocked_basenames: set[str] | None = None,
) -> Path | None:
    """Return the first supported image sidecar formed by appending an extension
    to the complete filename (e.g. ``game.state1`` -> ``game.state1.png``).
    Returns ``None`` if no such file exists.
    """
    blocked_names = _normalized_blocked_basenames(blocked_basenames)
    for extension in SUPPORTED_IMAGE_EXTENSIONS:
        candidate = Path(str(file_path) + extension)
        if blocked_names and candidate.name.casefold() in blocked_names:
            continue
        if candidate.exists() and candidate.is_file():
            return candidate
    return None


def session_screenshot_path(
    screenshot_directories: list[Path],
    session_window: tuple[float, float] | None,
    *,
    blocked_basenames: set[str] | None = None,
) -> Path | None:
    """Return the most recently captured screenshot from *screenshot_directories*
    whose modification time falls within *session_window*.

    Returns ``None`` if *session_window* is ``None``, if no directories are
    provided, or if no qualifying image is found.
    """
    if not screenshot_directories or session_window is None:
        return None

    window_start, window_end = session_window
    blocked_names = _normalized_blocked_basenames(blocked_basenames)

    best: Path | None = None
    best_mtime: float = -1.0

    for directory in screenshot_directories:
        try:
            candidates = list(directory.rglob("*"))
        except OSError:
            continue
        for candidate in candidates:
            try:
                if not candidate.is_file():
                    continue
                if candidate.suffix.casefold() not in SUPPORTED_IMAGE_EXTENSIONS:
                    continue
                if candidate.name.casefold() in blocked_names:
                    continue
                mtime = candidate.stat().st_mtime
            except OSError:
                continue
            if window_start <= mtime <= window_end and mtime > best_mtime:
                best = candidate
                best_mtime = mtime

    return best


def normalize_candidate_url(value: str) -> str:
    parsed = urlsplit(value)
    encoded_path = quote(parsed.path, safe="/%")
    query_items = parse_qsl(parsed.query, keep_blank_values=True)
    encoded_query = urlencode(query_items, doseq=True, quote_via=quote)
    return urlunsplit((parsed.scheme, parsed.netloc, encoded_path, encoded_query, parsed.fragment))


def state_download_candidate_paths(state_record: dict[str, Any]) -> list[str]:
    candidate_paths: list[str] = []
    for key in ("download_path", "file_path", "full_path"):
        value = state_record.get(key, "")
        if not isinstance(value, str):
            continue
        candidate = value.strip()
        if candidate:
            candidate_paths.append(candidate)
    return candidate_paths


def _extract_zip_with_7z(
    zip_path: Path,
    destination_root: Path,
    blocked_basenames: set[str],
    blocked_extensions: set[str],
) -> int:
    temp_dir = Path(tempfile.mkdtemp(prefix="rom-mate-save-7z-"))
    creationflags = subprocess.CREATE_NO_WINDOW if sys.platform.startswith("win") else 0
    try:
        commands: list[str] = []
        if _BUNDLED_7Z_PATH.exists():
            commands.append(str(_BUNDLED_7Z_PATH))
        commands.extend(["7z", "7za", "7zz"])

        extracted = False
        for command in commands:
            try:
                subprocess.run(
                    [command, "x", str(zip_path), f"-o{temp_dir}", "-y"],
                    check=True,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.PIPE,
                    text=True,
                    creationflags=creationflags,
                )
                extracted = True
                break
            except FileNotFoundError:
                continue
            except subprocess.CalledProcessError:
                continue

        if not extracted:
            raise OSError("No 7-Zip found to extract this archive.")

        extracted_count = 0
        for candidate in temp_dir.rglob("*"):
            if not candidate.is_file():
                continue
            if blocked_basenames and candidate.name.casefold() in blocked_basenames:
                continue
            if blocked_extensions and candidate.suffix.casefold() in blocked_extensions:
                continue

            relative_path = candidate.relative_to(temp_dir)
            destination = (destination_root / relative_path).resolve()
            try:
                destination.relative_to(destination_root)
            except ValueError:
                continue

            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(candidate, destination)
            extracted_count += 1

        return extracted_count
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


def screenshot_download_candidate_paths(screenshot_record: dict[str, Any]) -> list[str]:
    candidate_paths: list[str] = []
    for key in ("download_path", "file_path", "full_path"):
        value = screenshot_record.get(key, "")
        if not isinstance(value, str):
            continue
        candidate = value.strip()
        if candidate:
            candidate_paths.append(candidate)
    return candidate_paths


def extract_zip_archive_bytes_to_directory(
    payload: bytes,
    target_root: Path,
    *,
    skip_basenames: set[str] | None = None,
    skip_extensions: set[str] | None = None,
) -> int:
    temp_zip_path: Path | None = None
    blocked_basenames = _normalized_blocked_basenames(skip_basenames)
    blocked_extensions = _normalized_blocked_extensions(skip_extensions)
    try:
        fd, temp_path = tempfile.mkstemp(prefix="rom-mate-save-", suffix=".zip")
        os.close(fd)
        temp_zip_path = Path(temp_path)
        temp_zip_path.write_bytes(payload)
        if not zipfile.is_zipfile(temp_zip_path):
            raise ValueError("Downloaded save is not a zip archive.")

        destination_root = target_root.resolve()
        extracted_count = 0
        try:
            with zipfile.ZipFile(temp_zip_path) as archive:
                for member in archive.infolist():
                    member_name = member.filename.replace("\\", "/")
                    if not member_name or member_name.endswith("/"):
                        continue
                    relative_path = Path(member_name)
                    if relative_path.is_absolute() or any(part in {"", ".", ".."} for part in relative_path.parts):
                        continue
                    if blocked_basenames and relative_path.name.casefold() in blocked_basenames:
                        continue
                    if blocked_extensions and relative_path.suffix.casefold() in blocked_extensions:
                        continue

                    destination = (destination_root / relative_path).resolve()
                    try:
                        destination.relative_to(destination_root)
                    except ValueError:
                        continue

                    destination.parent.mkdir(parents=True, exist_ok=True)
                    with archive.open(member, "r") as source_file, destination.open("wb") as destination_file:
                        shutil.copyfileobj(source_file, destination_file)
                    extracted_count += 1
            return extracted_count
        except NotImplementedError:
            return _extract_zip_with_7z(
                temp_zip_path,
                destination_root,
                blocked_basenames,
                blocked_extensions,
            )
    finally:
        if temp_zip_path is not None and temp_zip_path.exists():
            try:
                temp_zip_path.unlink()
            except OSError:
                pass


def _temporary_archive_path(safe_title: str) -> Path:
    title = safe_title.strip() or "game"
    timestamp_iso = datetime.now().astimezone().isoformat(timespec="seconds").replace(":", "-")
    archive_name = f"{title}-{timestamp_iso}.zip"
    archive_path = Path(tempfile.gettempdir()) / archive_name
    if archive_path.exists():
        suffix = int(time.time() * 1000)
        archive_path = Path(tempfile.gettempdir()) / f"{title}-{timestamp_iso}-{suffix}.zip"
    return archive_path


def _unique_existing_files(files: list[Path]) -> list[Path]:
    unique: list[Path] = []
    seen: set[str] = set()
    for file_path in files:
        if not isinstance(file_path, Path) or not file_path.exists() or not file_path.is_file():
            continue
        key_value = str(file_path).casefold()
        if key_value in seen:
            continue
        seen.add(key_value)
        unique.append(file_path)
    return unique


def zip_selected_files_for_upload(
    files: list[Path],
    safe_title: str,
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> Path:
    selected_files = _unique_existing_files(files)
    if not selected_files:
        raise ValueError("No files were provided to archive for upload.")

    archive_path = _temporary_archive_path(safe_title)
    blocked_basenames = _normalized_blocked_basenames(ignore_basenames)
    blocked_extensions = _normalized_blocked_extensions(ignore_extensions)

    common_root = selected_files[0].parent
    try:
        common_root = Path(os.path.commonpath([str(path.parent) for path in selected_files]))
    except ValueError:
        common_root = selected_files[0].parent

    try:
        with zipfile.ZipFile(archive_path, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
            for file_path in sorted(selected_files, key=lambda item: str(item).casefold()):
                if blocked_basenames and file_path.name.casefold() in blocked_basenames:
                    continue
                if blocked_extensions and file_path.suffix.casefold() in blocked_extensions:
                    continue
                try:
                    relative_path = file_path.relative_to(common_root)
                except ValueError:
                    relative_path = Path(file_path.name)
                archive.write(file_path, relative_path.as_posix())
        return archive_path
    except OSError:
        if archive_path.exists():
            try:
                archive_path.unlink()
            except OSError:
                pass
        raise


def _grouped_upload_key(file_path: Path, file_field: str) -> str:
    if file_field == "stateFile":
        return file_path.name.strip().casefold()
    stem = file_path.stem.strip().casefold()
    return stem or file_path.name.strip().casefold()


def grouped_file_upload_jobs(
    files: list[Path],
    file_field: str,
    archive_builder: Callable[[list[Path]], Path],
) -> tuple[list[tuple[str, dict[str, Path]]], list[Path]]:
    selected_files = _unique_existing_files(files)
    if not selected_files:
        return [], []

    grouped_files: dict[str, list[Path]] = {}
    ordered_keys: list[str] = []
    for file_path in selected_files:
        key_value = _grouped_upload_key(file_path, file_field)
        if key_value not in grouped_files:
            grouped_files[key_value] = []
            ordered_keys.append(key_value)
        grouped_files[key_value].append(file_path)

    upload_jobs: list[tuple[str, dict[str, Path]]] = []
    temporary_archives: list[Path] = []
    for key_value in ordered_keys:
        group = grouped_files.get(key_value, [])
        if not group:
            continue
        if len(group) == 1:
            file_path = group[0]
            upload_jobs.append((file_path.name, {file_field: file_path}))
            continue

        archive_path = archive_builder(group)
        temporary_archives.append(archive_path)
        display_name = group[0].stem or archive_path.stem or group[0].name
        upload_jobs.append((display_name, {file_field: archive_path}))

    return upload_jobs, temporary_archives


def zip_directory_for_upload(
    directory: Path,
    safe_title: str,
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> Path:
    archive_path = _temporary_archive_path(safe_title)
    blocked_basenames = _normalized_blocked_basenames(ignore_basenames)
    blocked_extensions = _normalized_blocked_extensions(ignore_extensions)

    try:
        with zipfile.ZipFile(archive_path, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
            for candidate in directory.rglob("*"):
                if not candidate.is_file():
                    continue
                if blocked_basenames and candidate.name.casefold() in blocked_basenames:
                    continue
                if blocked_extensions and candidate.suffix.casefold() in blocked_extensions:
                    continue
                relative_path = candidate.relative_to(directory)
                archive_member_name = f"{directory.name}/{relative_path.as_posix()}"
                archive.write(candidate, archive_member_name)
        return archive_path
    except OSError:
        if archive_path.exists():
            try:
                archive_path.unlink()
            except OSError:
                pass
        raise


def zip_native_save_dirs_for_upload(
    dir_map: list[tuple[str, Path]],
    safe_title: str,
) -> tuple[Path, int, dict[str, str]]:
    """Create a combined zip archive from multiple native save directories.

    Returns ``(archive_path, total_files, manifest)`` where *manifest* maps
    ``str(idx)`` to the raw (unexpanded) path string for each directory whose
    contents were successfully iterated. Directories that raise an
    ``OSError`` during traversal are silently skipped. Individual files that
    raise an ``OSError`` during write are silently skipped.

    The archive always contains ``_rom_mate_dirs.json`` even when no files
    were added. If the underlying ``ZipFile`` creation itself raises
    ``OSError`` the temp file is cleaned up and the exception is re-raised.
    """
    import json

    archive_path = _temporary_archive_path(safe_title)
    total_files = 0
    manifest: dict[str, str] = {}

    try:
        with zipfile.ZipFile(archive_path, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
            for idx, (raw_path, directory) in enumerate(dir_map):
                prefix = f"{idx}/"
                try:
                    candidates = sorted(directory.rglob("*"), key=lambda p: str(p).casefold())
                except OSError:
                    continue
                manifest[str(idx)] = raw_path
                for candidate in candidates:
                    if not candidate.is_file():
                        continue
                    try:
                        relative = candidate.relative_to(directory)
                    except ValueError:
                        relative = Path(candidate.name)
                    archive_member = prefix + relative.as_posix()
                    try:
                        archive.write(candidate, archive_member)
                        total_files += 1
                    except OSError:
                        pass
            manifest_bytes = json.dumps(manifest, indent=2).encode("utf-8")
            archive.writestr("_rom_mate_dirs.json", manifest_bytes)
    except OSError:
        archive_path.unlink(missing_ok=True)
        raise

    return archive_path, total_files, manifest


def resolve_native_save_dir(raw_path: str, windows_documents: "Path | None") -> "Path":
    """Expand a raw env-var save path, correcting for Windows Documents folder redirection.

    On Windows the Documents folder may be redirected to a network share or
    non-default location.  ``%USERPROFILE%\\Documents`` then resolves to the
    wrong drive.  This function detects that situation and replaces the
    ``%USERPROFILE%\\Documents`` prefix with the Shell-resolved path supplied
    in *windows_documents*.

    Pass ``pcsx2_windows_documents_folder()`` as *windows_documents*.
    Pass ``None`` on non-Windows (no adjustment is made).
    """
    import os

    expanded = Path(os.path.expandvars(raw_path))
    if windows_documents is None:
        return expanded

    from pathlib import PureWindowsPath

    docs_via_env = PureWindowsPath(os.path.expandvars("%USERPROFILE%")) / "Documents"

    # If Shell Documents matches env-expanded Documents, no redirection — return as-is.
    if (
        str(windows_documents).casefold().rstrip("/\\")
        == str(docs_via_env).casefold().rstrip("/\\")
    ):
        return expanded

    # Check whether the expanded path is rooted under %USERPROFILE%\Documents.
    expanded_cf = str(PureWindowsPath(os.path.expandvars(raw_path))).casefold()
    docs_prefix = str(docs_via_env).rstrip("/\\").casefold()

    if expanded_cf == docs_prefix:
        return windows_documents

    if expanded_cf.startswith(docs_prefix + "\\") or expanded_cf.startswith(
        docs_prefix + "/"
    ):
        suffix = str(PureWindowsPath(os.path.expandvars(raw_path)))[len(str(docs_via_env)) :].lstrip("/\\")
        return Path(str(windows_documents) + "\\" + suffix)

    return expanded


def normalize_manual_save_path(folder: str) -> str:
    """Replace hardcoded user-profile path prefixes with env-var equivalents.

    Converts paths like ``C:\\Users\\Sam\\AppData\\Roaming\\...`` to
    ``%APPDATA%\\...`` so saves are portable across Windows reinstalls or
    username changes. Forward slashes (as returned by QFileDialog) are
    normalised to backslashes. On non-Windows, or when no known prefix
    matches, the path is returned unchanged.
    """
    from pathlib import PureWindowsPath

    folder_str = str(PureWindowsPath(folder))

    userprofile = os.path.expandvars("%USERPROFILE%")

    candidates: list[tuple[str, str]] = [
        ("%APPDATA%", os.path.expandvars("%APPDATA%")),
        ("%LOCALAPPDATA%", os.path.expandvars("%LOCALAPPDATA%")),
        (
            r"%USERPROFILE%\AppData\LocalLow",
            str(PureWindowsPath(userprofile) / "AppData" / "LocalLow"),
        ),
        (
            r"%USERPROFILE%\Documents",
            str(PureWindowsPath(userprofile) / "Documents"),
        ),
        ("%USERPROFILE%", userprofile),
    ]

    for env_var, expanded in candidates:
        if expanded.startswith("%"):
            continue
        prefix = expanded.rstrip("\\")
        if not folder_str.lower().startswith(prefix.lower()):
            continue
        after = folder_str[len(prefix) :]
        if after and after[0] not in "\\/":
            continue
        remainder = after.lstrip("\\/")
        if remainder:
            return env_var + "\\" + remainder
        return env_var

    return folder


def ppsspp_state_upload_jobs(
    id_tokens: list[str],
    directories: list[Path],
    file_field: str,
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> list[tuple[str, dict[str, Path]]]:
    blocked_basenames = _normalized_blocked_basenames(ignore_basenames)
    blocked_extensions = _normalized_blocked_extensions(ignore_extensions)
    candidates: list[tuple[Path, Path | None]] = []

    for directory in directories:
        if not directory.exists() or not directory.is_dir():
            continue
        for state_file in directory.glob("*.ppst"):
            if not state_file.is_file():
                continue
            if blocked_basenames and state_file.name.casefold() in blocked_basenames:
                continue
            if blocked_extensions and state_file.suffix.casefold() in blocked_extensions:
                continue
            normalized_name = re.sub(r"[^A-Z0-9]+", "", state_file.name.upper())
            if id_tokens and not any(token in normalized_name for token in id_tokens):
                continue
            screenshot = supported_image_sidecar_path(
                state_file,
                blocked_basenames=blocked_basenames,
            )
            candidates.append((state_file, screenshot))

    candidates.sort(key=lambda item: item[0].stat().st_mtime if item[0].exists() else 0, reverse=True)

    jobs: list[tuple[str, dict[str, Path]]] = []
    seen: set[str] = set()
    for state_file, screenshot in candidates:
        key_value = str(state_file).casefold()
        if key_value in seen:
            continue
        seen.add(key_value)

        files: dict[str, Path] = {file_field: state_file}
        if screenshot is not None:
            files["screenshotFile"] = screenshot
        jobs.append((state_file.name, files))
    return jobs


def retroarch_state_upload_jobs(
    files: list[Path],
    file_field: str,
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> tuple[list[tuple[str, dict[str, Path]]], list[Path]]:
    selected_files = _unique_existing_files(files)
    blocked_basenames = _normalized_blocked_basenames(ignore_basenames)
    blocked_extensions = _normalized_blocked_extensions(ignore_extensions)

    jobs: list[tuple[str, dict[str, Path]]] = []
    for state_file in selected_files:
        if blocked_basenames and state_file.name.casefold() in blocked_basenames:
            continue
        if blocked_extensions and state_file.suffix.casefold() in blocked_extensions:
            continue

        screenshot = appended_image_sidecar_path(
            state_file,
            blocked_basenames=ignore_basenames,
        )
        payload: dict[str, Path] = {file_field: state_file}
        if screenshot is not None:
            payload["screenshotFile"] = screenshot
        jobs.append((state_file.name, payload))

    return jobs, []


def filter_upload_jobs_by_session_window(
    upload_jobs: list[tuple[str, dict[str, Path]]],
    session_window: tuple[float, float] | None,
) -> list[tuple[str, dict[str, Path]]]:
    if session_window is None:
        return upload_jobs

    window_start, window_end = session_window
    filtered_jobs: list[tuple[str, dict[str, Path]]] = []
    for display_name, files_payload in upload_jobs:
        for path in files_payload.values():
            if not isinstance(path, Path) or not path.exists():
                continue
            try:
                path_mtime = float(path.stat().st_mtime)
            except OSError:
                continue
            if window_start <= path_mtime <= window_end:
                filtered_jobs.append((display_name, files_payload))
                break
    return filtered_jobs


def cleanup_temporary_paths(paths: list[Path]) -> None:
    for path in paths:
        if not path.exists():
            continue
        try:
            path.unlink()
        except OSError:
            continue


def uploaded_kind_label(save_type: str) -> str:
    return "save files" if save_type == "save" else "save states"


def should_skip_known_latest(last_downloaded_id: str, current_id: str, local_latest_mtime: float) -> bool:
    return bool(last_downloaded_id and last_downloaded_id == current_id and local_latest_mtime > 0)


def is_local_newer_than_server(local_latest_mtime: float, server_latest_timestamp: float) -> bool:
    return local_latest_mtime > 0 and local_latest_mtime > (server_latest_timestamp + 1.0)
