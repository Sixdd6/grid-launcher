from __future__ import annotations

from pathlib import Path
from typing import Protocol

from PySide6.QtCore import QUrl
from PySide6.QtGui import QPixmap
from PySide6.QtWidgets import QLabel


MAX_CACHED_COVER_BYTES = 20 * 1024 * 1024


class CoverManagerWindowProtocol(Protocol):
    cover_cache: dict[str, QPixmap | None]

    def _cached_cover_path_from_game(self, game: dict[str, str]) -> Path | None:
        ...

    def _cached_cover_cache_key(self, cached_cover_path: Path) -> str:
        ...

    def _resolved_cover_url_for_game(self, game: dict[str, str]) -> str:
        ...

    def _apply_cover_to_label(self, label: QLabel, pixmap: QPixmap | None) -> None:
        ...

    def _queue_cover_load(self, cover_url: str, label: QLabel) -> None:
        ...

    def _path_key(self, path: Path) -> str:
        ...


def _discard_unreadable_cached_cover(
    window: CoverManagerWindowProtocol,
    cached_cover_path: Path,
    cache_key: str,
) -> None:
    window.cover_cache[cache_key] = None
    try:
        if cached_cover_path.exists() and cached_cover_path.is_file():
            cached_cover_path.unlink()
    except (OSError, ValueError):
        pass


def _cached_cover_url(cached_cover_path: Path) -> str:
    return QUrl.fromLocalFile(str(cached_cover_path)).toString()


def _cached_cover_file_looks_safe(cached_cover_path: Path) -> bool:
    try:
        file_size = cached_cover_path.stat().st_size
    except (OSError, ValueError):
        return False
    return 0 < file_size <= MAX_CACHED_COVER_BYTES


def cached_cover_for_game(window: CoverManagerWindowProtocol, game: dict[str, str]) -> QPixmap | None:
    cached_cover_path = window._cached_cover_path_from_game(game)
    if cached_cover_path is None:
        return None

    try:
        if not cached_cover_path.exists() or not cached_cover_path.is_file():
            return None
    except (OSError, ValueError):
        return None

    cache_key = window._cached_cover_cache_key(cached_cover_path)
    if cache_key in window.cover_cache:
        return window.cover_cache[cache_key]

    local_cover_url = _cached_cover_url(cached_cover_path)
    if local_cover_url in window.cover_cache:
        local_cached = window.cover_cache[local_cover_url]
        window.cover_cache[cache_key] = local_cached
        return local_cached

    return None


def queue_game_cover_load(window: CoverManagerWindowProtocol, game: dict[str, str], label: QLabel) -> None:
    cached_cover = cached_cover_for_game(window, game)
    if cached_cover is not None:
        window._apply_cover_to_label(label, cached_cover)
        return

    cached_cover_path = window._cached_cover_path_from_game(game)
    if cached_cover_path is not None:
        try:
            cached_exists = cached_cover_path.exists() and cached_cover_path.is_file()
        except (OSError, ValueError):
            cached_exists = False
        if cached_exists:
            cache_key = window._cached_cover_cache_key(cached_cover_path)
            if _cached_cover_file_looks_safe(cached_cover_path):
                window._queue_cover_load(_cached_cover_url(cached_cover_path), label)
            else:
                _discard_unreadable_cached_cover(window, cached_cover_path, cache_key)

    cover_url = window._resolved_cover_url_for_game(game)
    if cover_url:
        window._queue_cover_load(cover_url, label)


def cached_cover_path_keys_for_games(window: CoverManagerWindowProtocol, games: list[dict[str, str]]) -> set[str]:
    keys: set[str] = set()
    for game in games:
        cached_cover_path = window._cached_cover_path_from_game(game)
        if cached_cover_path is None:
            continue
        keys.add(window._path_key(cached_cover_path))
    return keys


def cleanup_cached_cover_for_game(
    window: CoverManagerWindowProtocol,
    game: dict[str, str],
    protected_cache_paths: set[str] | None = None,
) -> tuple[bool, Path | None]:
    cached_cover_path = window._cached_cover_path_from_game(game)
    if cached_cover_path is None:
        return True, None

    window.cover_cache.pop(window._cached_cover_cache_key(cached_cover_path), None)
    window.cover_cache.pop(_cached_cover_url(cached_cover_path), None)
    cached_path_key = window._path_key(cached_cover_path)
    if protected_cache_paths is not None and cached_path_key in protected_cache_paths:
        return True, cached_cover_path

    if not cached_cover_path.exists() or not cached_cover_path.is_file():
        return True, cached_cover_path

    cached_cover_path.unlink()
    return True, cached_cover_path
