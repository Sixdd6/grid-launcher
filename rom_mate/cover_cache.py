from __future__ import annotations

from typing import Any, Protocol
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from PySide6.QtGui import QPixmap


class CoverCacheWindowProtocol(Protocol):
    cover_cache: dict[str, QPixmap | None]
    current_details_game: dict[str, str] | None
    details_cover_label: Any

    def _cached_cover_path_from_game(self, game: dict[str, str]):
        ...

    def _resolved_cover_url_for_game(self, game: dict[str, str]) -> str:
        ...

    def _cover_cache_extension_from_payload(self, cover_url: str, payload: bytes, content_type: str = "") -> str:
        ...

    def _installed_cover_cache_key(self, game: dict[str, str]) -> str:
        ...

    def _image_cache_dir(self):
        ...

    def _cached_cover_cache_key(self, cached_cover_path):
        ...

    def _auth_headers(self) -> dict[str, str]:
        ...

    def _games_match_identity(self, left: dict[str, str], right: dict[str, str]) -> bool:
        ...


def _write_cover_payload(window: CoverCacheWindowProtocol, game: dict[str, str], cover_url: str, payload: bytes, content_type: str = "") -> str:
    if not payload:
        return ""

    parsed = QPixmap()
    if not parsed.loadFromData(payload):
        return ""

    extension = window._cover_cache_extension_from_payload(cover_url, payload, content_type)
    cache_file_name = f"{window._installed_cover_cache_key(game)}{extension}"
    cache_file = window._image_cache_dir() / cache_file_name

    try:
        window._image_cache_dir().mkdir(parents=True, exist_ok=True)
        cache_file.write_bytes(payload)
    except OSError:
        return ""

    window.cover_cache[cover_url] = parsed
    window.cover_cache[window._cached_cover_cache_key(cache_file)] = parsed
    return str(cache_file)


def _save_fallback_cached_pixmap(window: CoverCacheWindowProtocol, game: dict[str, str], cover_url: str) -> str:
    cached_pixmap = window.cover_cache.get(cover_url)
    if cached_pixmap is None or cached_pixmap.isNull():
        if (
            window.current_details_game is not None
            and window._games_match_identity(window.current_details_game, game)
            and window.details_cover_label is not None
        ):
            label_pixmap = window.details_cover_label.pixmap()
            if label_pixmap is not None and not label_pixmap.isNull():
                cached_pixmap = label_pixmap
                window.cover_cache[cover_url] = cached_pixmap
        if cached_pixmap is None or cached_pixmap.isNull():
            return ""

    cache_file_name = f"{window._installed_cover_cache_key(game)}.png"
    cache_file = window._image_cache_dir() / cache_file_name
    try:
        window._image_cache_dir().mkdir(parents=True, exist_ok=True)
        if not cached_pixmap.save(str(cache_file), "PNG"):
            return ""
    except OSError:
        return ""

    window.cover_cache[window._cached_cover_cache_key(cache_file)] = cached_pixmap
    return str(cache_file)


def cache_cover_image_for_game(window: CoverCacheWindowProtocol, game: dict[str, str]) -> str:
    existing_cached_path = window._cached_cover_path_from_game(game)
    if existing_cached_path is not None and existing_cached_path.exists() and existing_cached_path.is_file():
        return str(existing_cached_path)

    cover_url = window._resolved_cover_url_for_game(game)
    if not cover_url:
        return ""

    request_headers: dict[str, str] = {"Accept": "image/*"}
    authorization = window._auth_headers().get("Authorization", "").strip()
    if authorization:
        request_headers["Authorization"] = authorization

    try:
        request = Request(cover_url, headers=request_headers, method="GET")
        with urlopen(request, timeout=30) as response:
            payload = response.read()
            response_content_type = response.headers.get("Content-Type", "")
        cached_path = _write_cover_payload(window, game, cover_url, payload, response_content_type)
        if cached_path:
            return cached_path
    except (HTTPError, URLError, OSError, ValueError, OverflowError):
        pass

    return _save_fallback_cached_pixmap(window, game, cover_url)
