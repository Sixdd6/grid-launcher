from __future__ import annotations

import hashlib
import re
from pathlib import Path
from typing import Any, Callable
from urllib.parse import parse_qsl, quote, urlencode, urlsplit, urlunsplit


def resolve_cover_url(value: Any, base_url: str) -> str:
    if not isinstance(value, str) or not value.strip():
        return ""
    candidate = value.strip()
    if not (candidate.startswith("http://") or candidate.startswith("https://")):
        if not base_url:
            return ""
        if candidate.startswith("/"):
            candidate = f"{base_url}{candidate}"
        else:
            candidate = f"{base_url}/{candidate}"

    split = urlsplit(candidate)
    safe_path = quote(split.path, safe="/%._-~")
    query_items = parse_qsl(split.query, keep_blank_values=True)
    safe_query = urlencode(query_items, doseq=True)
    return urlunsplit((split.scheme, split.netloc, safe_path, safe_query, split.fragment))


def cover_url_from_rom_payload(payload: dict[str, Any], resolver: Callable[[Any], str]) -> str:
    def resolve_cover_value(value: Any) -> str:
        if isinstance(value, str):
            return resolver(value)
        if isinstance(value, dict):
            for key in ("url", "path", "image", "src", "download_path", "file_path", "full_path"):
                candidate = value.get(key)
                if isinstance(candidate, str):
                    resolved = resolver(candidate)
                    if resolved:
                        return resolved
        return ""

    for key in (
        "url_cover",
        "path_cover_large",
        "path_cover_small",
        "cover_url",
        "cover_image",
        "cover_path",
        "image_url",
    ):
        value = payload.get(key)
        resolved = resolve_cover_value(value)
        if resolved:
            return resolved

    return ""


def screenshot_urls_from_rom_payload(payload: dict[str, Any], resolver: Callable[[Any], str]) -> list[str]:
    urls: list[str] = []

    def append_url(value: Any) -> None:
        if isinstance(value, str):
            resolved = resolver(value)
            if resolved and resolved not in urls:
                urls.append(resolved)
            return
        if isinstance(value, dict):
            for key in ("url", "path", "image", "src"):
                candidate = value.get(key)
                if isinstance(candidate, str):
                    resolved = resolver(candidate)
                    if resolved and resolved not in urls:
                        urls.append(resolved)
                        return

    merged_screenshots = payload.get("merged_screenshots")
    if isinstance(merged_screenshots, list):
        for item in merged_screenshots:
            append_url(item)

    user_screenshots = payload.get("user_screenshots")
    if isinstance(user_screenshots, list):
        for item in user_screenshots:
            if not isinstance(item, dict):
                continue
            for key in ("download_path", "file_path", "full_path"):
                append_url(item.get(key))

    gamelist_metadata = payload.get("gamelist_metadata")
    if isinstance(gamelist_metadata, dict):
        for key in ("screenshot_url", "title_screen_url", "image_url"):
            append_url(gamelist_metadata.get(key))

    ss_metadata = payload.get("ss_metadata")
    if isinstance(ss_metadata, dict):
        for key in ("screenshot_url", "title_screen_url", "fanart_url"):
            append_url(ss_metadata.get(key))

    launchbox_metadata = payload.get("launchbox_metadata")
    if isinstance(launchbox_metadata, dict):
        images = launchbox_metadata.get("images")
        if isinstance(images, list):
            for image in images:
                if not isinstance(image, dict):
                    continue
                append_url(image.get("url"))

    for key in ("url_screenshots", "path_screenshots", "screenshots", "images"):
        value = payload.get(key)
        if isinstance(value, list):
            for item in value:
                append_url(item)
        else:
            append_url(value)

    for key in ("url_screenshot", "path_screenshot"):
        append_url(payload.get(key))

    return urls


def screenshot_urls_from_game(raw: Any) -> list[str]:
    if not isinstance(raw, str) or not raw.strip():
        return []
    unique: list[str] = []
    for item in raw.splitlines():
        value = item.strip()
        if value and value not in unique:
            unique.append(value)
    return unique


def cached_cover_path_from_game(game: dict[str, str]) -> Path | None:
    cached_cover_value = game.get("cached_cover_path", "")
    if not isinstance(cached_cover_value, str) or not cached_cover_value.strip():
        return None
    return Path(cached_cover_value.strip()).expanduser()


def cached_cover_cache_key(cached_cover_path: Path, path_key: Callable[[Path], str]) -> str:
    return f"file:{path_key(cached_cover_path)}"


def installed_cover_cache_key(game: dict[str, str]) -> str:
    rom_id_value = game.get("rom_id", "")
    title_value = game.get("title", "")
    platform_value = game.get("platform", "")

    rom_id = rom_id_value.strip() if isinstance(rom_id_value, str) else ""
    title = title_value.strip() if isinstance(title_value, str) else ""
    platform = platform_value.strip() if isinstance(platform_value, str) else ""

    basis = rom_id or f"{title}|{platform}"
    digest = hashlib.sha1(basis.encode("utf-8", errors="ignore")).hexdigest()[:12]
    safe_title = re.sub(r"[^A-Za-z0-9._-]+", "_", title).strip("_.-") or "game"
    return f"{safe_title[:48]}-{digest}"


def cover_cache_extension_from_payload(cover_url: str, payload: bytes, content_type: str = "") -> str:
    normalized_content_type = content_type.strip().casefold().split(";", 1)[0]
    mime_extensions = {
        "image/jpeg": ".jpg",
        "image/jpg": ".jpg",
        "image/png": ".png",
        "image/webp": ".webp",
        "image/gif": ".gif",
        "image/bmp": ".bmp",
        "image/x-ms-bmp": ".bmp",
        "image/tiff": ".tiff",
        "image/x-icon": ".ico",
        "image/vnd.microsoft.icon": ".ico",
        "image/svg+xml": ".svg",
    }
    mapped_extension = mime_extensions.get(normalized_content_type)
    if mapped_extension:
        return mapped_extension

    if payload.startswith(b"\x89PNG\r\n\x1a\n"):
        return ".png"
    if payload.startswith(b"\xff\xd8\xff"):
        return ".jpg"
    if payload.startswith((b"GIF87a", b"GIF89a")):
        return ".gif"
    if payload.startswith(b"BM"):
        return ".bmp"
    if payload.startswith((b"II*\x00", b"MM\x00*")):
        return ".tiff"
    if payload.startswith(b"\x00\x00\x01\x00"):
        return ".ico"
    if len(payload) >= 12 and payload.startswith(b"RIFF") and payload[8:12] == b"WEBP":
        return ".webp"

    preview = payload[:256].lstrip()
    if preview.startswith(b"<svg") or preview.startswith(b"<?xml") and b"<svg" in preview.casefold():
        return ".svg"

    parsed = urlsplit(cover_url)
    suffix = Path(parsed.path).suffix.lower()
    valid_extensions = {
        ".jpg",
        ".jpeg",
        ".png",
        ".webp",
        ".gif",
        ".bmp",
        ".tif",
        ".tiff",
        ".ico",
        ".svg",
        ".avif",
        ".heic",
        ".heif",
    }
    if suffix in valid_extensions:
        return suffix
    return ".img"
