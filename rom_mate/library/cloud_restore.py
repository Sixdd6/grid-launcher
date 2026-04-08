from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Any, Callable


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
    selection = list(emulator_records if emulator_records else records)

    def _id_rank(record: dict[str, Any]) -> int:
        value = record.get("id", 0)
        try:
            return int(value)
        except (TypeError, ValueError):
            return 0

    selection.sort(key=lambda item: (timestamp_fn(item), _id_rank(item)), reverse=True)
    return selection[0] if selection else None


def restore_single_save_payload(
    directories: list[Path],
    save_record: dict[str, Any],
    payload: bytes,
    candidate_paths: list[Path],
    fallback_name: str,
) -> Path | None:
    if not payload or not directories:
        return None

    if candidate_paths:
        target_path = candidate_paths[0]
    else:
        file_name_value = save_record.get("file_name", "")
        file_name = Path(file_name_value).name if isinstance(file_name_value, str) else ""
        if not file_name:
            file_name = fallback_name
        target_path = directories[0] / file_name

    target_path.parent.mkdir(parents=True, exist_ok=True)
    target_path.write_bytes(payload)
    return target_path


def restore_single_state_payload(
    directories: list[Path],
    state_record: dict[str, Any],
    payload: bytes,
    candidate_paths: list[Path],
    fallback_name: str,
) -> Path | None:
    if not payload or not directories:
        return None

    file_name_value = state_record.get("file_name", "")
    file_name = Path(file_name_value).name if isinstance(file_name_value, str) else ""
    if file_name:
        target_path = directories[0] / file_name
        for directory in directories:
            candidate = directory / file_name
            if candidate.exists() and candidate.is_file():
                target_path = candidate
                break
    elif candidate_paths:
        target_path = candidate_paths[0]
    else:
        target_path = directories[0] / fallback_name

    target_path.parent.mkdir(parents=True, exist_ok=True)
    target_path.write_bytes(payload)
    return target_path
