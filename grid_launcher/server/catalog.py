from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Callable

from .metadata import details_metadata_from_item
from .platform_metadata import PLATFORM_METADATA, logo_file_for_platform


_LOGO_DIR = Path(__file__).parent.parent.parent / "assets" / "retroarch-assets"


def connected_username(me_payload: Any) -> str:
    if not isinstance(me_payload, dict):
        return ""
    username = me_payload.get("username")
    if not isinstance(username, str):
        return ""
    return username.strip()


def _platform_display_name(entry: dict) -> str:
    label = entry.get("display_name") or entry.get("name") or entry.get("slug")
    if not isinstance(label, str):
        return ""

    platform_id = entry.get("id")
    base_label = label.strip()
    if base_label:
        return base_label
    if isinstance(platform_id, int):
        return f"Platform {platform_id}"
    return ""


def server_platform_ids(payload: Any) -> dict[str, int]:
    if not isinstance(payload, list):
        return {}

    platform_ids: dict[str, int] = {}
    for entry in payload:
        if not isinstance(entry, dict):
            continue

        platform_id = entry.get("id")
        base_label = _platform_display_name(entry)
        rom_count = entry.get("rom_count")
        if (
            not isinstance(platform_id, int)
            or not base_label
            or (isinstance(rom_count, int) and rom_count <= 0)
        ):
            continue

        display_label = base_label
        counter = 2
        while display_label in platform_ids:
            display_label = f"{base_label} ({counter})"
            counter += 1

        platform_ids[display_label] = platform_id

    return platform_ids


def server_platform_slug_map(payload: Any) -> dict[str, str]:
    if not isinstance(payload, list):
        return {}

    slug_map: dict[str, str] = {}
    for entry in payload:
        if not isinstance(entry, dict):
            continue

        slug = entry.get("slug")
        base_label = _platform_display_name(entry)
        rom_count = entry.get("rom_count")
        if (
            not isinstance(slug, str)
            or not slug.strip()
            or not base_label
            or (isinstance(rom_count, int) and rom_count <= 0)
        ):
            continue

        display_label = base_label
        counter = 2
        while display_label in slug_map:
            display_label = f"{base_label} ({counter})"
            counter += 1

        slug_map[display_label] = slug.strip()

    return slug_map


def server_platform_details(payload: Any) -> list[dict]:
    if not isinstance(payload, list):
        return []

    details: list[dict] = []
    used_names: set[str] = set()

    for entry in payload:
        if not isinstance(entry, dict):
            continue

        slug = entry.get("slug")
        if not isinstance(slug, str) or not slug.strip():
            continue
        slug = slug.strip()

        base_name = _platform_display_name(entry)
        if not base_name:
            continue

        rom_count = entry.get("rom_count")
        if isinstance(rom_count, int) and rom_count <= 0:
            continue

        display_name = base_name
        counter = 2
        while display_name in used_names:
            display_name = f"{base_name} ({counter})"
            counter += 1
        used_names.add(display_name)

        metadata = PLATFORM_METADATA.get(slug, {})
        manufacturer = metadata.get("manufacturer", "") if isinstance(metadata, dict) else ""
        release_year = metadata.get("release_year", "") if isinstance(metadata, dict) else ""
        player_count = metadata.get("player_count", "") if isinstance(metadata, dict) else ""
        logo_file = logo_file_for_platform(slug, base_name)
        local_logo_path = (_LOGO_DIR / logo_file).as_uri() if logo_file else ""

        details.append(
            {
                "slug": slug,
                "name": display_name,
                "rom_count": rom_count,
                "manufacturer": manufacturer if isinstance(manufacturer, str) else "",
                "release_year": release_year if isinstance(release_year, str) else "",
                "player_count": player_count if isinstance(player_count, str) else "",
                "local_logo_path": local_logo_path,
                "url_logo": entry.get("url_logo") or "",
            }
        )

    details.sort(key=lambda item: item["name"].lower())
    return details


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


def fetch_rom_items_by_params(
    api_get: Callable[[str, dict | None], Any],
    params: dict,
) -> list[dict[str, Any]]:
    response = api_get("/api/roms", params)
    if not isinstance(response, dict):
        return []
    items = response.get("items", [])
    if not isinstance(items, list):
        return []
    return [item for item in items if isinstance(item, dict)]


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


def _is_xbox360_platform_label(label: Any) -> bool:
    if not isinstance(label, str):
        return False
    normalized = "".join(ch for ch in label.lower() if ch.isalnum())
    if not normalized:
        return False
    return normalized in {"xbox360", "microsoftxbox360", "xb360"}


def _is_xbox360_rom_item(item: dict[str, Any], fallback_platform_label: str) -> bool:
    candidates = (
        item.get("platform_fs_slug"),
        item.get("platform_slug"),
        item.get("platform_display_name"),
        fallback_platform_label,
    )
    return any(_is_xbox360_platform_label(candidate) for candidate in candidates)


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

        details_metadata = details_metadata_from_item(item)

        rom_id = str(item.get("id", "")).strip()
        if rom_id:
            rom_payloads[rom_id] = item

        platform_name = item.get("platform_display_name")
        resolved_platform = platform_name.strip() if isinstance(platform_name, str) and platform_name.strip() else platform_label
        ps4_file_ids_by_category = {}
        if _is_ps4_rom_item(item, resolved_platform):
            ps4_file_ids_by_category = _ps4_file_ids_by_category(item)
        xbox360_file_ids_by_category = {}
        if _is_xbox360_rom_item(item, resolved_platform):
            xbox360_file_ids_by_category = _ps4_file_ids_by_category(item)

        cover_url = cover_url_from_payload(item)
        screenshot_urls = screenshot_urls_from_payload(item)
        ra_id = item.get("ra_id")
        fs_name_val = item.get("fs_name", "")
        fs_ext_val = item.get("fs_extension", "")
        fs_name_str = fs_name_val.strip() if isinstance(fs_name_val, str) else ""
        fs_ext_str = fs_ext_val.strip().lstrip(".") if isinstance(fs_ext_val, str) else ""
        if fs_name_str and fs_ext_str and not Path(fs_name_str).suffix:
            rom_file_name = f"{fs_name_str}.{fs_ext_str}"
        else:
            rom_file_name = fs_name_str
        rom_nested_file_name = ""
        # Folder-backed ROMs can expose the actual archive filename under files[0].file_name.
        if not fs_ext_str:
            files_val = item.get("files")
            if isinstance(files_val, list) and files_val:
                first_file_name = files_val[0].get("file_name", "") if isinstance(files_val[0], dict) else ""
                first_file_name = first_file_name.strip() if isinstance(first_file_name, str) else ""
                if first_file_name and Path(first_file_name).suffix:
                    rom_nested_file_name = first_file_name
        rom_base_file_id = ""
        files_val = item.get("files")
        if isinstance(files_val, list) and len(files_val) > 1:
            for file_entry in files_val:
                if not isinstance(file_entry, dict):
                    continue
                raw_cat = file_entry.get("category")
                cat = raw_cat.strip().lower() if isinstance(raw_cat, str) else ""
                if cat in ("", "game"):
                    file_id = file_entry.get("id")
                    if isinstance(file_id, int):
                        rom_base_file_id = str(file_id)
                        break
        games.append(
            {
                "title": title.strip(),
                "platform": resolved_platform,
                "rating": details_metadata.get("rating", ""),
                "description": details_metadata.get("description", ""),
                "genres": details_metadata.get("genres", ""),
                "regions": details_metadata.get("regions", ""),
                "release_year": details_metadata.get("release_year", ""),
                "filesize_bytes": details_metadata.get("filesize_bytes", ""),
                "revision": details_metadata.get("revision", ""),
                "languages": details_metadata.get("languages", ""),
                "tags": details_metadata.get("tags", ""),
                "fanart_url": details_metadata.get("fanart_url", ""),
                "companies": details_metadata.get("companies", ""),
                "first_release_date": details_metadata.get("first_release_date", ""),
                "cover_url": cover_url,
                "screenshot_urls": "\n".join(screenshot_urls),
                "rom_id": rom_id,
                "server_updated_at": _rom_updated_at_text(item),
                "rom_file_name": rom_file_name,
                "rom_nested_file_name": rom_nested_file_name,
                "rom_base_file_id": rom_base_file_id,
                "ra_id": str(ra_id) if ra_id is not None else "",
                "ps4_has_update": "true" if "update" in ps4_file_ids_by_category else "false",
                "ps4_has_dlc": "true" if "dlc" in ps4_file_ids_by_category else "false",
                "ps4_file_ids_by_category": json.dumps(ps4_file_ids_by_category, separators=(",", ":"), sort_keys=True),
                "xbox360_has_update": "true" if "update" in xbox360_file_ids_by_category else "false",
                "xbox360_has_dlc": "true" if "dlc" in xbox360_file_ids_by_category else "false",
                "xbox360_file_ids_by_category": json.dumps(xbox360_file_ids_by_category, separators=(",", ":"), sort_keys=True),
            }
        )

    return games, rom_payloads
