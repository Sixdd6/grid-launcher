from __future__ import annotations

import io
import time
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any, Callable

from .cloud_transfer import extract_zip_archive_bytes_to_directory


def save_record_timestamp(record: dict[str, Any]) -> float:
    for key in ("updated_at", "created_at"):
        value = record.get(key)
        if not isinstance(value, str):
            continue
        text = value.strip()
        if not text:
            continue
        if text.endswith("Z"):
            text = f"{text[:-1]}+00:00"
        try:
            return datetime.fromisoformat(text).timestamp()
        except ValueError:
            continue
    return 0.0


def relative_timestamp_text(timestamp: float, *, now: float | None = None) -> str:
    if not timestamp:
        return "Unknown"

    current_time = time.time() if now is None else now
    elapsed_seconds = max(0, int(current_time - timestamp))
    if elapsed_seconds < 30:
        return "just now"
    if elapsed_seconds < 90:
        return "1 minute ago"

    ranges = (
        (86_400, 3_600, "hour"),
        (3_600, 60, "minute"),
    )
    for threshold, unit_seconds, label in ranges:
        if elapsed_seconds < threshold:
            value = max(1, elapsed_seconds // unit_seconds)
            suffix = "" if value == 1 else "s"
            return f"{value} {label}{suffix} ago"

    days = elapsed_seconds // 86_400
    if days < 7:
        suffix = "" if days == 1 else "s"
        return f"{days} day{suffix} ago"

    weeks = max(1, days // 7)
    suffix = "" if weeks == 1 else "s"
    return f"{weeks} week{suffix} ago"


def sort_server_records_by_recency(
    records: list[dict[str, Any]],
    timestamp_fn: Callable[[dict[str, Any]], float],
) -> list[dict[str, Any]]:
    def _id_rank(record: dict[str, Any]) -> int:
        value = record.get("id", 0)
        try:
            return int(value)
        except (TypeError, ValueError):
            return 0

    return sorted(
        [item for item in records if isinstance(item, dict)],
        key=lambda item: (timestamp_fn(item), _id_rank(item)),
        reverse=True,
    )


def server_records_from_payload(payload: Any, *, id_key: str = "id") -> list[dict[str, Any]]:
    if not isinstance(payload, list):
        return []

    records: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    for item in payload:
        if not isinstance(item, dict):
            continue
        record_id = str(item.get(id_key, "")).strip()
        if not record_id or record_id in seen_ids:
            continue
        seen_ids.add(record_id)
        records.append(item)
    return records


def latest_server_record(
    records: list[dict[str, Any]],
    emulator_name: str,
    timestamp_fn: Callable[[dict[str, Any]], float],
) -> dict[str, Any] | None:
    if not records:
        return None

    emulator_key = emulator_name.strip().casefold()
    emulator_records = [
        item
        for item in records
        if isinstance(item.get("emulator"), str)
        and item.get("emulator", "").strip().casefold() == emulator_key
    ]
    selection = sort_server_records_by_recency(list(emulator_records if emulator_records else records), timestamp_fn)
    return selection[0] if selection else None


def preferred_restore_target_path(
    directories: list[Path],
    record_file_name: str,
    candidate_paths: list[Path],
    fallback_name: str,
) -> Path | None:
    if not directories:
        return None

    normalized_record_name = Path(record_file_name).name.strip() if isinstance(record_file_name, str) else ""
    normalized_fallback_name = Path(fallback_name).name.strip() if isinstance(fallback_name, str) else ""

    for preferred_name in (normalized_record_name, normalized_fallback_name):
        if not preferred_name:
            continue
        for candidate in candidate_paths:
            if candidate.name.strip().casefold() == preferred_name.casefold():
                return candidate

    if normalized_record_name and candidate_paths:
        return candidate_paths[0].parent / normalized_record_name
    if candidate_paths:
        return candidate_paths[0]
    if normalized_record_name:
        return directories[0] / normalized_record_name
    if normalized_fallback_name:
        return directories[0] / normalized_fallback_name
    return None


def _payload_is_zip_archive(payload: bytes) -> bool:
    if not payload:
        return False
    return zipfile.is_zipfile(io.BytesIO(payload))


def restore_single_save_payload(
    directories: list[Path],
    save_record: dict[str, Any],
    payload: bytes,
    candidate_paths: list[Path],
    fallback_name: str,
    *,
    skip_basenames: set[str] | None = None,
    skip_extensions: set[str] | None = None,
) -> Path | None:
    if not payload or not directories:
        return None

    file_name_value = save_record.get("file_name", "")
    file_name = Path(file_name_value).name if isinstance(file_name_value, str) else ""
    target_path = preferred_restore_target_path(directories, file_name, candidate_paths, fallback_name)
    if target_path is None:
        return None

    target_path.parent.mkdir(parents=True, exist_ok=True)
    if _payload_is_zip_archive(payload):
        extracted_count = extract_zip_archive_bytes_to_directory(
            payload,
            target_path.parent,
            skip_basenames=skip_basenames,
            skip_extensions=skip_extensions,
        )
        return target_path.parent if extracted_count > 0 else None

    target_path.write_bytes(payload)
    return target_path


def restore_single_state_payload(
    directories: list[Path],
    state_record: dict[str, Any],
    payload: bytes,
    candidate_paths: list[Path],
    fallback_name: str,
    *,
    skip_basenames: set[str] | None = None,
    skip_extensions: set[str] | None = None,
) -> Path | None:
    if not payload or not directories:
        return None

    file_name_value = state_record.get("file_name", "")
    file_name = Path(file_name_value).name if isinstance(file_name_value, str) else ""
    target_path = preferred_restore_target_path(directories, file_name, candidate_paths, fallback_name)
    if target_path is None:
        return None

    target_path.parent.mkdir(parents=True, exist_ok=True)
    if _payload_is_zip_archive(payload):
        extracted_count = extract_zip_archive_bytes_to_directory(
            payload,
            target_path.parent,
            skip_basenames=skip_basenames,
            skip_extensions=skip_extensions,
        )
        return target_path.parent if extracted_count > 0 else None

    target_path.write_bytes(payload)
    return target_path
