from __future__ import annotations

from pathlib import Path
from typing import Protocol

from PySide6.QtGui import QPixmap
from PySide6.QtWidgets import QLabel


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


def cached_cover_for_game(window: CoverManagerWindowProtocol, game: dict[str, str]) -> QPixmap | None:
    cached_cover_path = window._cached_cover_path_from_game(game)
    if cached_cover_path is None or not cached_cover_path.exists() or not cached_cover_path.is_file():
        return None

    cache_key = window._cached_cover_cache_key(cached_cover_path)
    if cache_key in window.cover_cache:
        return window.cover_cache[cache_key]

    pixmap = QPixmap(str(cached_cover_path))
    loaded = pixmap if not pixmap.isNull() else None
    window.cover_cache[cache_key] = loaded
    return loaded


def queue_game_cover_load(window: CoverManagerWindowProtocol, game: dict[str, str], label: QLabel) -> None:
    cached_cover = cached_cover_for_game(window, game)
    if cached_cover is not None:
        window._apply_cover_to_label(label, cached_cover)
        return

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
    cached_path_key = window._path_key(cached_cover_path)
    if protected_cache_paths is not None and cached_path_key in protected_cache_paths:
        return True, cached_cover_path

    if not cached_cover_path.exists() or not cached_cover_path.is_file():
        return True, cached_cover_path

    cached_cover_path.unlink()
    return True, cached_cover_path
