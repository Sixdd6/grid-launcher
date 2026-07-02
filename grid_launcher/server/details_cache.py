from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Callable
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlsplit


def rom_file_name_from_payload(payload: dict[str, Any]) -> str:
    candidate_fields = (
        "fs_name",
        "file_name",
        "filename",
        "rom_file_name",
        "download_path",
        "file_path",
        "full_path",
        "path",
        "url",
    )
    candidates: list[str] = []
    # Highest priority: fs_name + fs_extension combined (RomM stores these separately)
    fs_name_raw = payload.get("fs_name", "")
    fs_ext_raw = payload.get("fs_extension", "")
    fs_name = fs_name_raw.strip() if isinstance(fs_name_raw, str) else ""
    fs_ext = fs_ext_raw.strip().lstrip(".") if isinstance(fs_ext_raw, str) else ""
    if fs_name and fs_ext:
        candidates.append(f"{fs_name}.{fs_ext}")
    elif fs_name and not fs_ext:
        # Folder-backed ROM: use the first nested file when it has a real archive suffix.
        files_val = payload.get("files")
        if isinstance(files_val, list) and files_val:
            first_file = files_val[0]
            if isinstance(first_file, dict):
                first_file_name = str(first_file.get("file_name", "")).strip()
                if first_file_name and Path(first_file_name).suffix:
                    candidates.append(first_file_name)

    for field in candidate_fields:
        value = payload.get(field, "")
        if not isinstance(value, str):
            continue
        candidate = value.strip().replace("\\", "/")
        if field == "url":
            candidate = urlsplit(candidate).path
        candidate = candidate.strip().lstrip("/")
        if candidate:
            candidates.append(candidate)

    if not candidates:
        return ""

    with_suffix = [candidate for candidate in candidates if Path(candidate.split("/")[-1]).suffix]
    return with_suffix[0] if with_suffix else candidates[0]


def fetch_server_rom_payload(
    rom_id: str,
    server_rom_payloads: dict[str, dict[str, Any]],
    *,
    api_get: Callable[[str], Any],
    force_refresh: bool = False,
) -> dict[str, Any] | None:
    rom_id_key = rom_id.strip()
    if not rom_id_key:
        return None

    cached_payload = server_rom_payloads.get(rom_id_key)
    if not force_refresh and isinstance(cached_payload, dict):
        return cached_payload

    rom_id_path = quote(rom_id_key, safe="")
    try:
        payload = api_get(f"/api/roms/{rom_id_path}")
    except (HTTPError, URLError, ValueError, json.JSONDecodeError):
        return None

    detail_payload: dict[str, Any] | None = None
    if isinstance(payload, dict):
        if any(key in payload for key in ("fs_name", "file_name", "filename", "rom_file_name")):
            detail_payload = payload
        else:
            for nested_key in ("item", "rom", "data"):
                nested = payload.get(nested_key)
                if isinstance(nested, dict):
                    detail_payload = nested
                    break

    if detail_payload is None:
        return None

    server_rom_payloads[rom_id_key] = detail_payload
    return detail_payload


def details_rom_id_cache_key(
    game: dict[str, str] | None,
    *,
    game_key: Callable[[dict[str, str]], tuple[str, str]],
) -> str:
    if game is None:
        return ""
    title, platform = game_key(game)
    if not title or not platform:
        return ""
    return f"{title}::{platform}"


def details_rom_id_cache(raw: Any) -> dict[str, str]:
    if not isinstance(raw, dict):
        return {}
    normalized: dict[str, str] = {}
    for key, value in raw.items():
        if not isinstance(key, str) or not isinstance(value, str):
            continue
        if not key.strip() or not value.strip():
            continue
        normalized[key.strip()] = value.strip()
    return normalized


def cache_rom_id_for_details_game(
    config: dict[str, Any],
    game: dict[str, str],
    rom_id: str,
    *,
    details_rom_id_cache_key: Callable[[dict[str, str] | None], str],
    details_rom_id_cache: Callable[[], dict[str, str]],
) -> bool:
    cache_key = details_rom_id_cache_key(game)
    if not cache_key or not rom_id.strip():
        return False
    cache = details_rom_id_cache()
    cache[cache_key] = rom_id.strip()
    config["details_rom_id_cache"] = cache
    return True


def clear_cached_rom_id_for_details_game(
    config: dict[str, Any],
    game: dict[str, str] | None,
    *,
    details_rom_id_cache_key: Callable[[dict[str, str] | None], str],
    details_rom_id_cache: Callable[[], dict[str, str]],
) -> bool:
    cache_key = details_rom_id_cache_key(game)
    if not cache_key:
        return False
    cache = details_rom_id_cache()
    if cache_key not in cache:
        return False
    cache.pop(cache_key, None)
    if cache:
        config["details_rom_id_cache"] = cache
    else:
        config.pop("details_rom_id_cache", None)
    return True


def resolved_rom_file_name_for_game(
    game: dict[str, str],
    rom_id: str,
    server_rom_payloads: dict[str, dict[str, Any]],
    *,
    fetch_server_rom_payload: Callable[[str, bool], dict[str, Any] | None],
    rom_file_name_from_payload: Callable[[dict[str, Any]], str],
) -> str:
    current_value = game.get("rom_file_name", "")
    current_name = current_value.strip().replace("\\", "/").lstrip("/") if isinstance(current_value, str) else ""
    if current_name and Path(current_name.split("/")[-1]).suffix:
        return current_name

    fallback_name = current_name if current_name else ""

    payload = server_rom_payloads.get(rom_id)
    if not isinstance(payload, dict):
        payload = fetch_server_rom_payload(rom_id, False)
    if isinstance(payload, dict):
        payload_name = rom_file_name_from_payload(payload)
        if payload_name and Path(payload_name.split("/")[-1]).suffix:
            return payload_name
        if payload_name and not fallback_name:
            fallback_name = payload_name

    payload = fetch_server_rom_payload(rom_id, True)
    if isinstance(payload, dict):
        payload_name = rom_file_name_from_payload(payload)
        if payload_name and Path(payload_name.split("/")[-1]).suffix:
            return payload_name
        if payload_name and not fallback_name:
            fallback_name = payload_name

    return fallback_name


def resolve_rom_id_for_game(
    game: dict[str, str],
    server_games_by_platform: dict[str, list[dict[str, str]]],
    *,
    game_key: Callable[[dict[str, str]], tuple[str, str]],
    details_rom_id_cache_key: Callable[[dict[str, str] | None], str],
    details_rom_id_cache: Callable[[], dict[str, str]],
) -> str:
    direct = str(game.get("rom_id", "")).strip()
    if direct:
        return direct

    cache_key = details_rom_id_cache_key(game)
    cache = details_rom_id_cache()
    cached = str(cache.get(cache_key, "")).strip()
    if cached:
        return cached

    target = game_key(game)
    for games in server_games_by_platform.values():
        for server_game in games:
            if game_key(server_game) != target:
                continue
            server_rom_id = str(server_game.get("rom_id", "")).strip()
            if server_rom_id:
                return server_rom_id
    return ""
