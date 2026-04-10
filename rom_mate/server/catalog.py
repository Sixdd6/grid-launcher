from __future__ import annotations

import json
from typing import Any, Callable


def connected_username(me_payload: Any) -> str:
    if not isinstance(me_payload, dict):
        return ""
    username = me_payload.get("username")
    if not isinstance(username, str):
        return ""
    return username.strip()


def server_platform_ids(payload: Any) -> dict[str, int]:
    if not isinstance(payload, list):
        return {}

    platform_ids: dict[str, int] = {}
    for entry in payload:
        if not isinstance(entry, dict):
            continue

        platform_id = entry.get("id")
        label = entry.get("display_name") or entry.get("name") or entry.get("slug")
        rom_count = entry.get("rom_count")
        if (
            not isinstance(platform_id, int)
            or not isinstance(label, str)
            or (isinstance(rom_count, int) and rom_count <= 0)
        ):
            continue

        base_label = label.strip() or f"Platform {platform_id}"
        display_label = base_label
        counter = 2
        while display_label in platform_ids:
            display_label = f"{base_label} ({counter})"
            counter += 1

        platform_ids[display_label] = platform_id

    return platform_ids


def fetch_platform_rom_items(
    api_get: Callable[[str, dict[str, Any] | None], Any],
    platform_id: int,
    page_size: int = 200,
) -> list[dict[str, Any]]:
    all_items: list[dict[str, Any]] = []
    offset = 0

    while True:
        payload = api_get(
            "/api/roms",
            {
                "platform_ids": [platform_id],
                "limit": page_size,
                "offset": offset,
                "with_char_index": "false",
                "with_filter_values": "false",
            },
        )
        if not isinstance(payload, dict):
            break

        page_items = payload.get("items")
        if not isinstance(page_items, list) or not page_items:
            break

        for item in page_items:
            if isinstance(item, dict):
                all_items.append(item)

        total = payload.get("total")
        if isinstance(total, int) and len(all_items) >= total:
            break

        if len(page_items) < page_size:
            break

        offset += page_size

    return all_items


def _is_ps4_platform_label(label: Any) -> bool:
    if not isinstance(label, str):
        return False
    normalized = "".join(ch for ch in label.lower() if ch.isalnum())
    if not normalized:
        return False
    return normalized in {"ps4", "playstation4", "sonyplaystation4"}


def _is_ps4_rom_item(item: dict[str, Any], fallback_platform_label: str) -> bool:
    candidates = (
        item.get("platform_fs_slug"),
        item.get("platform_slug"),
        item.get("platform_display_name"),
        fallback_platform_label,
    )
    return any(_is_ps4_platform_label(candidate) for candidate in candidates)


def _ps4_file_ids_by_category(item: dict[str, Any]) -> dict[str, list[int]]:
    files = item.get("files")
    if not isinstance(files, list):
        return {}

    file_ids_by_category: dict[str, list[int]] = {}
    for file_entry in files:
        if not isinstance(file_entry, dict):
            continue

        file_id = file_entry.get("id")
        if not isinstance(file_id, int):
            continue

        raw_category = file_entry.get("category")
        category = raw_category.strip().lower() if isinstance(raw_category, str) and raw_category.strip() else "game"
        file_ids_by_category.setdefault(category, []).append(file_id)

    return file_ids_by_category


def _rom_updated_at_text(item: dict[str, Any]) -> str:
    updated_at = item.get("updated_at")
    if not isinstance(updated_at, str):
        return ""
    return updated_at.strip()


def games_from_rom_items(
    all_items: list[dict[str, Any]],
    platform_label: str,
    cover_url_from_payload: Callable[[dict[str, Any]], str],
    screenshot_urls_from_payload: Callable[[dict[str, Any]], list[str]],
) -> tuple[list[dict[str, str]], dict[str, dict[str, Any]]]:
    games: list[dict[str, str]] = []
    rom_payloads: dict[str, dict[str, Any]] = {}

    for item in all_items:
        title = item.get("name") or item.get("fs_name_no_ext")
        if not isinstance(title, str) or not title.strip():
            continue

        rom_id = str(item.get("id", "")).strip()
        if rom_id:
            rom_payloads[rom_id] = item

        platform_name = item.get("platform_display_name")
        resolved_platform = platform_name.strip() if isinstance(platform_name, str) and platform_name.strip() else platform_label
        ps4_file_ids_by_category = {}
        if _is_ps4_rom_item(item, resolved_platform):
            ps4_file_ids_by_category = _ps4_file_ids_by_category(item)

        summary = item.get("summary")
        cover_url = cover_url_from_payload(item)
        screenshot_urls = screenshot_urls_from_payload(item)
        ra_id = item.get("ra_id")
        games.append(
            {
                "title": title.strip(),
                "platform": resolved_platform,
                "rating": "N/A",
                "description": summary.strip() if isinstance(summary, str) and summary.strip() else "No description available.",
                "cover_url": cover_url,
                "screenshot_urls": "\n".join(screenshot_urls),
                "rom_id": rom_id,
                "server_updated_at": _rom_updated_at_text(item),
                "rom_file_name": item.get("fs_name", "").strip() if isinstance(item.get("fs_name", ""), str) else "",
                "ra_id": str(ra_id) if ra_id is not None else "",
                "ps4_has_update": "true" if "update" in ps4_file_ids_by_category else "false",
                "ps4_has_dlc": "true" if "dlc" in ps4_file_ids_by_category else "false",
                "ps4_file_ids_by_category": json.dumps(ps4_file_ids_by_category, separators=(",", ":"), sort_keys=True),
            }
        )

    return games, rom_payloads
