from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable


def normalize_cloud_sync_state(value: Any) -> dict[str, dict[str, Any]]:
    if not isinstance(value, dict):
        return {}

    normalized: dict[str, dict[str, Any]] = {}
    for raw_key, raw_state in value.items():
        if not isinstance(raw_key, str) or not raw_key.strip() or not isinstance(raw_state, dict):
            continue

        key = raw_key.strip()
        state: dict[str, Any] = {}

        last_downloaded_save_id = raw_state.get("last_downloaded_save_id", "")
        if isinstance(last_downloaded_save_id, str) and last_downloaded_save_id.strip():
            state["last_downloaded_save_id"] = last_downloaded_save_id.strip()

        last_server_timestamp = raw_state.get("last_server_timestamp", 0)
        if isinstance(last_server_timestamp, (int, float)):
            state["last_server_timestamp"] = float(last_server_timestamp)

        last_uploaded_local_mtime = raw_state.get("last_uploaded_local_mtime", 0)
        if isinstance(last_uploaded_local_mtime, (int, float)):
            state["last_uploaded_local_mtime"] = float(last_uploaded_local_mtime)

        last_uploaded_at = raw_state.get("last_uploaded_at", "")
        if isinstance(last_uploaded_at, str) and last_uploaded_at.strip():
            state["last_uploaded_at"] = last_uploaded_at.strip()

        last_downloaded_state_id = raw_state.get("last_downloaded_state_id", "")
        if isinstance(last_downloaded_state_id, str) and last_downloaded_state_id.strip():
            state["last_downloaded_state_id"] = last_downloaded_state_id.strip()

        last_uploaded_save_mtime = raw_state.get("last_uploaded_save_mtime", 0)
        if isinstance(last_uploaded_save_mtime, (int, float)):
            state["last_uploaded_save_mtime"] = float(last_uploaded_save_mtime)

        last_uploaded_state_mtime = raw_state.get("last_uploaded_state_mtime", 0)
        if isinstance(last_uploaded_state_mtime, (int, float)):
            state["last_uploaded_state_mtime"] = float(last_uploaded_state_mtime)

        last_session_started_at = raw_state.get("last_session_started_at", 0)
        if isinstance(last_session_started_at, (int, float)):
            state["last_session_started_at"] = float(last_session_started_at)

        last_session_ended_at = raw_state.get("last_session_ended_at", 0)
        if isinstance(last_session_ended_at, (int, float)):
            state["last_session_ended_at"] = float(last_session_ended_at)

        if state:
            normalized[key] = state

    return normalized


def cloud_sync_state(state_value: Any) -> dict[str, dict[str, Any]]:
    return normalize_cloud_sync_state(state_value)


def cloud_sync_state_key(
    game: dict[str, str],
    rom_id_key: Callable[[dict[str, str]], str],
    game_key: Callable[[dict[str, str]], tuple[str, str]],
) -> str:
    rom_id = rom_id_key(game)
    if rom_id:
        return f"rom:{rom_id}"
    title, platform = game_key(game)
    if not title and not platform:
        return ""
    return f"name:{title}::{platform}"


def cloud_sync_state_for_game(
    state_map: dict[str, dict[str, Any]],
    game: dict[str, str],
    rom_id_key: Callable[[dict[str, str]], str],
    game_key_fn: Callable[[dict[str, str]], tuple[str, str]],
) -> dict[str, Any]:
    key = cloud_sync_state_key(game, rom_id_key, game_key_fn)
    if not key:
        return {}
    return dict(state_map.get(key, {}))


def update_cloud_sync_state_for_game(
    state_map: dict[str, dict[str, Any]],
    game: dict[str, str],
    updates: dict[str, Any],
    rom_id_key: Callable[[dict[str, str]], str],
    game_key_fn: Callable[[dict[str, str]], tuple[str, str]],
) -> dict[str, dict[str, Any]]:
    key = cloud_sync_state_key(game, rom_id_key, game_key_fn)
    if not key or not isinstance(updates, dict) or not updates:
        return state_map

    existing = state_map.get(key, {})
    if not isinstance(existing, dict):
        existing = {}
    merged = existing.copy()
    merged.update(updates)

    updated_map = dict(state_map)
    updated_map[key] = merged
    return updated_map


def partition_active_game_sessions(active_game_sessions: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    remaining: list[dict[str, Any]] = []
    finished: list[dict[str, Any]] = []
    for session in active_game_sessions:
        process = session.get("process")
        poll = getattr(process, "poll", None)
        if not callable(poll):
            continue
        if poll() is None:
            remaining.append(session)
        else:
            finished.append(session)
    return remaining, finished


def session_cloud_sync_updates(session: dict[str, Any], ended_at: float) -> tuple[dict[str, str] | None, dict[str, Any]]:
    game = session.get("game")
    if not isinstance(game, dict):
        return None, {}

    started_raw = session.get("started_at", 0)
    try:
        started_at = float(started_raw)
    except (TypeError, ValueError):
        started_at = 0.0

    if started_at <= 0:
        return game, {}

    return game, {
        "last_session_started_at": started_at,
        "last_session_ended_at": ended_at,
    }


def auto_cloud_upload_plan(
    sync_state: dict[str, Any],
    local_latest_save_mtime: float = 0.0,
    local_latest_state_mtime: float = 0.0,
    include_state_upload: bool = True,
) -> tuple[list[str], dict[str, float]]:
    upload_types: list[str] = []
    latest_mtimes: dict[str, float] = {}

    if local_latest_save_mtime > 0:
        previous_save_mtime_raw = sync_state.get("last_uploaded_save_mtime", sync_state.get("last_uploaded_local_mtime", 0))
        try:
            previous_save_mtime = float(previous_save_mtime_raw)
        except (TypeError, ValueError):
            previous_save_mtime = 0.0
        if local_latest_save_mtime > (previous_save_mtime + 1.0):
            upload_types.append("save")
            latest_mtimes["save"] = local_latest_save_mtime

    if include_state_upload and local_latest_state_mtime > 0:
        previous_state_mtime_raw = sync_state.get("last_uploaded_state_mtime", 0)
        try:
            previous_state_mtime = float(previous_state_mtime_raw)
        except (TypeError, ValueError):
            previous_state_mtime = 0.0
        if local_latest_state_mtime > (previous_state_mtime + 1.0):
            upload_types.append("state")
            latest_mtimes["state"] = local_latest_state_mtime

    return upload_types, latest_mtimes


def summarize_auto_cloud_upload_result(
    result: dict[str, Any],
    uploaded_at: str,
) -> tuple[dict[str, Any], list[str], bool, bool]:
    per_type_raw = result.get("per_type", {})
    local_latest_mtimes_raw = result.get("local_latest_mtimes", {})
    per_type = per_type_raw if isinstance(per_type_raw, dict) else {}
    local_latest_mtimes = local_latest_mtimes_raw if isinstance(local_latest_mtimes_raw, dict) else {}

    updates: dict[str, Any] = {}
    debug_segments: list[str] = []
    any_uploaded = False
    any_failed = False

    for save_type in ("save", "state"):
        raw_entry = per_type.get(save_type, {})
        entry = raw_entry if isinstance(raw_entry, dict) else {}

        try:
            uploaded = int(entry.get("uploaded_count", 0))
        except (TypeError, ValueError):
            uploaded = 0
        try:
            total = int(entry.get("total_count", 0))
        except (TypeError, ValueError):
            total = 0

        failed_raw = entry.get("failed_files", [])
        failed = [str(item) for item in failed_raw] if isinstance(failed_raw, list) else []

        if total <= 0 and uploaded <= 0 and not failed:
            continue

        any_uploaded = any_uploaded or uploaded > 0
        any_failed = any_failed or bool(failed)

        if uploaded > 0:
            latest_raw = local_latest_mtimes.get(save_type, 0)
            try:
                latest_mtime = float(latest_raw)
            except (TypeError, ValueError):
                latest_mtime = 0.0

            if save_type == "save":
                updates["last_uploaded_save_mtime"] = latest_mtime
                updates["last_uploaded_local_mtime"] = latest_mtime
            else:
                updates["last_uploaded_state_mtime"] = latest_mtime

        debug_segments.append(f"{save_type}={uploaded}/{max(total, uploaded)} failed={failed[:3]}")

    if any_uploaded and isinstance(uploaded_at, str) and uploaded_at.strip():
        updates["last_uploaded_at"] = uploaded_at.strip()

    return updates, debug_segments, any_uploaded, any_failed


def session_window_for_state_upload(
    active_game_sessions: list[dict[str, Any]],
    game: dict[str, str],
    games_match_identity: Callable[[dict[str, str], dict[str, str]], bool],
    sync_state: dict[str, Any],
    now: float,
) -> tuple[float, float] | None:
    for session in reversed(active_game_sessions):
        session_game = session.get("game")
        if not isinstance(session_game, dict):
            continue
        if not games_match_identity(session_game, game):
            continue
        started_raw = session.get("started_at", 0)
        try:
            started_at = float(started_raw)
        except (TypeError, ValueError):
            started_at = 0.0
        if started_at <= 0:
            continue
        return max(0.0, started_at - 2.0), now + 30.0

    started_raw = sync_state.get("last_session_started_at", 0)
    ended_raw = sync_state.get("last_session_ended_at", 0)
    try:
        started_at = float(started_raw)
    except (TypeError, ValueError):
        started_at = 0.0
    try:
        ended_at = float(ended_raw)
    except (TypeError, ValueError):
        ended_at = 0.0

    if started_at <= 0:
        return None
    if ended_at <= 0:
        ended_at = started_at
    if ended_at < started_at:
        ended_at = started_at
    return max(0.0, started_at - 2.0), ended_at + 30.0


def filter_files_by_mtime_window(files: list[Path], start_time: float, end_time: float) -> list[Path]:
    filtered: list[Path] = []
    for candidate in files:
        try:
            candidate_mtime = float(candidate.stat().st_mtime)
        except (OSError, ValueError):
            continue
        if start_time <= candidate_mtime <= end_time:
            filtered.append(candidate)
    return filtered


def filter_directories_by_mtime_window(
    directories: list[Path],
    start_time: float,
    end_time: float,
    latest_file_mtime_under_path: Callable[..., float],
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> list[Path]:
    filtered: list[Path] = []
    for directory in directories:
        latest_mtime = latest_file_mtime_under_path(
            directory,
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )
        if start_time <= latest_mtime <= end_time:
            filtered.append(directory)
    return filtered


def session_filtered_file_candidates(files: list[Path], session_window: tuple[float, float] | None) -> list[Path]:
    if session_window is None:
        return files
    filtered = filter_files_by_mtime_window(files, session_window[0], session_window[1])
    return filtered if filtered else files


def session_filtered_directory_candidates(
    directories: list[Path],
    session_window: tuple[float, float] | None,
    latest_file_mtime_under_path: Callable[..., float],
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> list[Path]:
    if session_window is None:
        return directories
    filtered = filter_directories_by_mtime_window(
        directories,
        session_window[0],
        session_window[1],
        latest_file_mtime_under_path,
        ignore_basenames=ignore_basenames,
        ignore_extensions=ignore_extensions,
    )
    return filtered if filtered else directories


def _unique_casefold_paths(paths: list[Path]) -> list[Path]:
    unique: list[Path] = []
    seen: set[str] = set()
    for path in paths:
        key_value = str(path).casefold()
        if key_value in seen:
            continue
        seen.add(key_value)
        unique.append(path)
    return unique


def cloud_sync_directory_candidates_for_game(
    game: dict[str, str],
    directories: list[Path],
    game_save_match_tokens: Callable[[dict[str, str]], set[str]],
    latest_file_mtime_under_path: Callable[..., float],
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> list[Path]:
    tokens = game_save_match_tokens(game)
    candidates: list[Path] = []
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

    for directory in directories:
        if not directory.exists() or not directory.is_dir():
            continue
        for child in directory.iterdir():
            if not child.is_dir():
                continue
            if not any(
                candidate.is_file()
                and (not blocked_basenames or candidate.name.casefold() not in blocked_basenames)
                and (not blocked_extensions or candidate.suffix.casefold() not in blocked_extensions)
                for candidate in child.rglob("*")
            ):
                continue

            normalized_name = re.sub(r"[^a-z0-9]+", "", child.name.casefold())
            normalized_relative = re.sub(r"[^a-z0-9]+", "", str(child.relative_to(directory)).casefold())
            if tokens and not any(token in normalized_name or token in normalized_relative for token in tokens):
                continue
            candidates.append(child)

    candidates.sort(
        key=lambda item: latest_file_mtime_under_path(
            item,
            ignore_basenames=blocked_basenames,
            ignore_extensions=blocked_extensions,
        ),
        reverse=True,
    )
    return _unique_casefold_paths(candidates)


def cloud_sync_candidates_for_game(
    game: dict[str, str],
    directories: list[Path],
    save_type: str,
    game_save_match_tokens: Callable[[dict[str, str]], set[str]],
    is_state_file_candidate: Callable[[Path], bool],
    *,
    ignore_basenames: set[str] | None = None,
    ignore_extensions: set[str] | None = None,
) -> list[Path]:
    if save_type not in {"save", "state"}:
        return []

    tokens = game_save_match_tokens(game)
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
    candidates: list[Path] = []
    state_candidates: list[Path] = []

    for directory in directories:
        for candidate in directory.rglob("*"):
            if not candidate.is_file():
                continue
            if blocked_basenames and candidate.name.casefold() in blocked_basenames:
                continue
            if blocked_extensions and candidate.suffix.casefold() in blocked_extensions:
                continue

            candidate_name = candidate.name.casefold()
            candidate_stem_compact = re.sub(r"[^a-z0-9]+", "", candidate.stem.casefold())
            if save_type == "save" and tokens and not any(
                token in candidate_name or (token in candidate_stem_compact and token) for token in tokens
            ):
                continue
            candidates.append(candidate)
            if save_type == "state" and is_state_file_candidate(candidate):
                state_candidates.append(candidate)

    if save_type == "state" and state_candidates:
        candidates = state_candidates

    candidates.sort(key=lambda item: item.stat().st_mtime if item.exists() else 0, reverse=True)
    return _unique_casefold_paths(candidates)
