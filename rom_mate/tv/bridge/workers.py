from __future__ import annotations

import threading
from typing import Any, Callable

from PySide6.QtCore import QObject, QThread, Signal


class CatalogFetchWorker(QObject):
    """Fetches /api/users/me and /api/platforms in a background thread."""

    finished = Signal(object)
    error = Signal(str)

    def __init__(self, api_get: Callable[[str, dict | None], Any]) -> None:
        super().__init__()
        self._api_get = api_get

    def run(self) -> None:
        try:
            me_payload = self._api_get("/api/users/me", None)
            platforms_payload = self._api_get("/api/platforms", None)
            self.finished.emit({"me": me_payload, "platforms": platforms_payload})
        except Exception as exc:
            self.error.emit(str(exc))


class RomListFetchWorker(QObject):
    """Fetches all ROMs for a given platform_id in a background thread."""

    finished = Signal(object)   # {"platform_label": str, "games": list}
    error = Signal(object)      # {"platform_label": str, "message": str}

    def __init__(
        self,
        api_get: Callable[[str, dict | None], Any],
        platform_label: str,
        platform_id: int,
        library_games: list[dict[str, str]],
        base_url: str = "",
    ) -> None:
        super().__init__()
        self._api_get = api_get
        self._platform_label = platform_label
        self._platform_id = platform_id
        self._library_games = library_games
        self._base_url = base_url

    def run(self) -> None:
        from rom_mate.cover.utils import cover_url_from_rom_payload, screenshot_urls_from_rom_payload, resolve_cover_url, filter_to_server_host
        from rom_mate.server.catalog import fetch_platform_rom_items, games_from_rom_items
        try:
            items = fetch_platform_rom_items(self._api_get, self._platform_id)
            captured_base_url = self._base_url

            def _cover_url(payload):
                return cover_url_from_rom_payload(payload, lambda v: filter_to_server_host(resolve_cover_url(v, captured_base_url), captured_base_url))

            def _screenshot_urls(payload):
                return screenshot_urls_from_rom_payload(payload, lambda v: filter_to_server_host(resolve_cover_url(v, captured_base_url), captured_base_url))

            games, _ = games_from_rom_items(items, self._platform_label, _cover_url, _screenshot_urls)
            self.finished.emit({"platform_label": self._platform_label, "games": games})
        except Exception as exc:
            self.error.emit({"platform_label": self._platform_label, "message": str(exc)})


class FavoritesRomFetchWorker(QObject):
    """Fetches favourite ROMs in a background thread."""

    finished = Signal(object)
    error = Signal(str)

    def __init__(self, api_get: Any, base_url: str, parent: "QObject | None" = None) -> None:
        super().__init__(parent)
        self._api_get = api_get
        self._base_url = base_url

    def run(self) -> None:
        from rom_mate.cover.utils import cover_url_from_rom_payload, screenshot_urls_from_rom_payload, resolve_cover_url, filter_to_server_host
        from rom_mate.server.catalog import fetch_rom_items_by_params, games_from_rom_items
        try:
            items = fetch_rom_items_by_params(
                self._api_get,
                {"favorite": "true", "limit": "20", "with_char_index": "false", "with_filter_values": "false"},
            )
            captured_base_url = self._base_url

            def _cover_url(payload):
                return cover_url_from_rom_payload(payload, lambda v: filter_to_server_host(resolve_cover_url(v, captured_base_url), captured_base_url))

            def _screenshot_urls(payload):
                return screenshot_urls_from_rom_payload(payload, lambda v: filter_to_server_host(resolve_cover_url(v, captured_base_url), captured_base_url))

            games, _ = games_from_rom_items(items, "", _cover_url, _screenshot_urls)
            games = games[:20]
            self.finished.emit(games)
        except Exception as exc:
            self.error.emit(str(exc))


class NewAdditionsRomFetchWorker(QObject):
    """Fetches most recently added ROMs in a background thread."""

    finished = Signal(object)
    error = Signal(str)

    def __init__(self, api_get: Any, base_url: str, parent: "QObject | None" = None) -> None:
        super().__init__(parent)
        self._api_get = api_get
        self._base_url = base_url

    def run(self) -> None:
        from rom_mate.cover.utils import cover_url_from_rom_payload, screenshot_urls_from_rom_payload, resolve_cover_url, filter_to_server_host
        from rom_mate.server.catalog import fetch_rom_items_by_params, games_from_rom_items
        try:
            items = fetch_rom_items_by_params(
                self._api_get,
                {"order_by": "created_at", "order_dir": "desc", "limit": "10", "with_char_index": "false", "with_filter_values": "false"},
            )
            captured_base_url = self._base_url

            def _cover_url(payload):
                return cover_url_from_rom_payload(payload, lambda v: filter_to_server_host(resolve_cover_url(v, captured_base_url), captured_base_url))

            def _screenshot_urls(payload):
                return screenshot_urls_from_rom_payload(payload, lambda v: filter_to_server_host(resolve_cover_url(v, captured_base_url), captured_base_url))

            games, _ = games_from_rom_items(items, "", _cover_url, _screenshot_urls)
            self.finished.emit(games)
        except Exception as exc:
            self.error.emit(str(exc))


class HighlyRatedRomFetchWorker(QObject):
    """Fetches highly rated ROMs (>= 4.0/5) in a background thread."""

    finished = Signal(object)
    error = Signal(str)

    def __init__(self, api_get: Any, base_url: str, parent: "QObject | None" = None) -> None:
        super().__init__(parent)
        self._api_get = api_get
        self._base_url = base_url

    def run(self) -> None:
        from rom_mate.cover.utils import cover_url_from_rom_payload, screenshot_urls_from_rom_payload, resolve_cover_url, filter_to_server_host
        from rom_mate.server.catalog import fetch_rom_items_by_params, games_from_rom_items
        from rom_mate.server.metadata import normalize_rating_to_five
        try:
            items = fetch_rom_items_by_params(
                self._api_get,
                {"limit": "500", "with_char_index": "false", "with_filter_values": "false"},
            )
            captured_base_url = self._base_url

            def _cover_url(payload):
                return cover_url_from_rom_payload(payload, lambda v: filter_to_server_host(resolve_cover_url(v, captured_base_url), captured_base_url))

            def _screenshot_urls(payload):
                return screenshot_urls_from_rom_payload(payload, lambda v: filter_to_server_host(resolve_cover_url(v, captured_base_url), captured_base_url))

            games, _ = games_from_rom_items(items, "", _cover_url, _screenshot_urls)
            filtered = [
                game for game in games
                if (r := normalize_rating_to_five(game.get("rating", ""))) is not None and r >= 4.0
            ]
            filtered.sort(key=lambda g: normalize_rating_to_five(g.get("rating", "")) or 0.0, reverse=True)
            filtered = filtered[:20]
            self.finished.emit(filtered)
        except Exception as exc:
            self.error.emit(str(exc))


class SavesBatchFetchWorker(QObject):
    finished = Signal(object)
    error = Signal(str)

    def __init__(self, api_get):
        super().__init__()
        self._api_get = api_get

    def run(self):
        try:
            payload = self._api_get("/api/saves", None)
            rom_ids = set()
            if isinstance(payload, list):
                for item in payload:
                    if isinstance(item, dict) and item.get("rom_id") is not None:
                        rom_ids.add(str(item["rom_id"]))
            self.finished.emit(list(rom_ids))
        except Exception as exc:
            self.error.emit(str(exc))


class CollectionsFetchWorker(QObject):
    finished = Signal(object)
    error = Signal(str)

    def __init__(self, api_get: Any, parent: "QObject | None" = None) -> None:
        super().__init__(parent)
        self._api_get = api_get

    def run(self) -> None:
        try:
            payload = self._api_get("/api/collections", None)
            if not isinstance(payload, list):
                self.finished.emit(None)
                return
            for item in payload:
                if not isinstance(item, dict):
                    continue
                if item.get("is_favorite") is True:
                    collection_id = item.get("id")
                    rom_ids = item.get("rom_ids")
                    if not isinstance(rom_ids, list):
                        rom_ids = []
                    self.finished.emit({"id": collection_id, "rom_ids": rom_ids})
                    return
            self.finished.emit(None)
        except Exception as exc:
            self.error.emit(str(exc))


class CollectionUpdateWorker(QObject):
    finished = Signal(object)
    error = Signal(str)

    def __init__(
        self,
        api_put_text: Any,
        collection_id: int,
        new_rom_ids: list,
        parent: "QObject | None" = None,
    ) -> None:
        super().__init__(parent)
        self._api_put_text = api_put_text
        self._collection_id = collection_id
        self._new_rom_ids = new_rom_ids

    def run(self) -> None:
        import json

        try:
            payload = self._api_put_text(
                f"/api/collections/{self._collection_id}",
                {"rom_ids": json.dumps(self._new_rom_ids)},
            )
            self.finished.emit(payload)
        except Exception as exc:
            self.error.emit(str(exc))


class CollectionCreateWorker(QObject):
    finished = Signal(object)
    error = Signal(str)

    def __init__(self, api_post_text: Any, first_rom_id: int, parent: "QObject | None" = None) -> None:
        super().__init__(parent)
        self._api_post_text = api_post_text
        self._first_rom_id = first_rom_id

    def run(self) -> None:
        import json

        try:
            payload = self._api_post_text(
                "/api/collections",
                {"name": "Favorites", "rom_ids": json.dumps([self._first_rom_id])},
                params={"is_favorite": "true"},
            )
            self.finished.emit(payload)
        except Exception as exc:
            self.error.emit(str(exc))
