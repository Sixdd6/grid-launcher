from __future__ import annotations

import os
import re
import shutil
import tempfile
import time
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any
from urllib.parse import parse_qsl, quote, urlencode, urlsplit, urlunsplit


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


def extract_zip_archive_bytes_to_directory(
    payload: bytes,
    target_root: Path,
    *,
    skip_basenames: set[str] | None = None,
    skip_extensions: set[str] | None = None,
) -> int:
    temp_zip_path: Path | None = None
    blocked_basenames = {name.casefold() for name in (skip_basenames or set()) if isinstance(name, str) and name.strip()}
    blocked_extensions = {extension.casefold() for extension in (skip_extensions or set()) if isinstance(extension, str) and extension.strip()}
    try:
        fd, temp_path = tempfile.mkstemp(prefix="rom-mate-save-", suffix=".zip")
        os.close(fd)
        temp_zip_path = Path(temp_path)
        temp_zip_path.write_bytes(payload)
        if not zipfile.is_zipfile(temp_zip_path):
            raise ValueError("Downloaded save is not a zip archive.")

        destination_root = target_root.resolve()
        extracted_count = 0
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
    finally:
        if temp_zip_path is not None and temp_zip_path.exists():
            try:
                temp_zip_path.unlink()
            except OSError:
                pass


def zip_directory_for_upload(
    directory: Path,
    safe_title: str,
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> Path:
    title = safe_title.strip() or "game"
    timestamp_iso = datetime.now().astimezone().isoformat(timespec="seconds").replace(":", "-")
    archive_name = f"{title}-{timestamp_iso}.zip"
    archive_path = Path(tempfile.gettempdir()) / archive_name
    if archive_path.exists():
        suffix = int(time.time() * 1000)
        archive_path = Path(tempfile.gettempdir()) / f"{title}-{timestamp_iso}-{suffix}.zip"

    blocked_basenames = {
        name.casefold()
        for name in (ignore_basenames or set())
        if isinstance(name, str) and name.strip()
    }
    blocked_extensions = {
        extension.casefold()
        for extension in (ignore_extensions or set())
        if isinstance(extension, str) and extension.strip()
    }

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


def ppsspp_state_upload_jobs(
    id_tokens: list[str],
    directories: list[Path],
    file_field: str,
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> list[tuple[str, dict[str, Path]]]:
    blocked_basenames = {
        name.casefold()
        for name in (ignore_basenames or set())
        if isinstance(name, str) and name.strip()
    }
    blocked_extensions = {
        extension.casefold()
        for extension in (ignore_extensions or set())
        if isinstance(extension, str) and extension.strip()
    }
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
            screenshot = state_file.with_suffix(".jpg")
            if not screenshot.exists() or not screenshot.is_file():
                screenshot = None
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
