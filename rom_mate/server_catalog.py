from __future__ import annotations

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
        summary = item.get("summary")
        cover_url = cover_url_from_payload(item)
        screenshot_urls = screenshot_urls_from_payload(item)
        games.append(
            {
                "title": title.strip(),
                "platform": platform_name.strip() if isinstance(platform_name, str) and platform_name.strip() else platform_label,
                "rating": "N/A",
                "description": summary.strip() if isinstance(summary, str) and summary.strip() else "No description available.",
                "cover_url": cover_url,
                "screenshot_urls": "\n".join(screenshot_urls),
                "rom_id": rom_id,
                "rom_file_name": item.get("fs_name", "").strip() if isinstance(item.get("fs_name", ""), str) else "",
            }
        )

    return games, rom_payloads
