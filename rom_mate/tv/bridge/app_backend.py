from __future__ import annotations

import threading
from pathlib import Path
from typing import Any, Callable

from PySide6.QtCore import Property, QObject, Qt, QTimer, Signal, Slot

from rom_mate.core.config import write_config_file as _write_config_file


def _is_thread_running(thread: Any) -> bool:
    """Return True only if thread is non-None and still alive/running."""
    if thread is None:
        return False
    try:
        if hasattr(thread, 'is_alive'):
            return thread.is_alive()
        return thread.isRunning()
    except RuntimeError:
        return False


class _RomMetaFetchWorker(QObject):
    """Fetches ROM detail metadata from the server in a background thread."""

    finished = Signal(object)  # {"rom_id": rom_id, "metadata": metadata_dict}

    def __init__(self, base_url: str, api_token: str, rom_id: str) -> None:
        super().__init__()
        self._base_url = base_url
        self._api_token = api_token
        self._rom_id = rom_id

    @Slot()
    def run(self) -> None:
        from rom_mate.core.api import api_get_json
        from rom_mate.server.metadata import details_metadata_from_item
        try:
            payload = api_get_json(self._base_url, self._api_token, f"/api/roms/{self._rom_id}")
            if not isinstance(payload, dict):
                self.finished.emit({"rom_id": self._rom_id, "metadata": {}})
                return
            raw = details_metadata_from_item(payload)
            filtered = {k: v for k, v in raw.items() if v and v not in ("N/A", "No description available.")}
            self.finished.emit({"rom_id": self._rom_id, "metadata": filtered})
        except Exception:
            self.finished.emit({"rom_id": self._rom_id, "metadata": {}})


class AppBackend(QObject):
    """Exposes config, library games, and server data to QML."""

    _DEFAULT_EXCLUSION_LIST = ["RPCS3", "Cemu", "Dolphin", "Xemu", "Xenia", "RetroArch"]

    libraryGamesChanged = Signal()
    platformsChanged = Signal()
    serverGamesChanged = Signal(str)      # platform_label
    connectionStatusChanged = Signal(str) # status text
    switchToDesktopModeRequested = Signal()
    exclusionListChanged = Signal(object)
    exclusionDataChanged = Signal()
    homeViewTabChanged = Signal(str)
    autoSyncChanged = Signal(bool)
    overlayStateChanged = Signal()
    favoritesGamesChanged = Signal()
    newAdditionsGamesChanged = Signal()
    highlyRatedGamesChanged = Signal()
    favoriteToggleComplete = Signal(object)   # {"rom_id": str, "is_now_favorite": bool}
    romMetadataReady = Signal(object)  # {"rom_id": str, "metadata_json": str}
    romMetadataFetchStarted = Signal(str)    # rom_id
    saveConfigRequested = Signal()

    def __init__(
        self,
        config: dict[str, Any],
        image_cache_dir: Path,
        parent: QObject | None = None,
        handle_diag_fn: Callable[[str], None] | None = None,
    ) -> None:
        super().__init__(parent)
        self._config = config
        self._image_cache_dir = image_cache_dir
        self._handle_diag_fn = handle_diag_fn
        self._is_connected = False
        self._platforms: dict[str, int] = {}       # label -> id
        self._platform_details: list[dict] = []
        self._server_games: dict[str, list[dict[str, str]]] = {}  # platform_label -> games
        self._catalog_thread: threading.Thread | None = None
        self._catalog_worker: Any | None = None
        self._rom_threads: dict[str, threading.Thread] = {}
        self._rom_workers: dict[str, Any] = {}
        self._ui_overlay_active: bool = False
        self._favorites_games: list = []
        self._new_additions_games: list = []
        self._highly_rated_games: list = []
        self._favorites_thread: threading.Thread | None = None
        self._new_additions_thread: threading.Thread | None = None
        self._highly_rated_thread: threading.Thread | None = None
        self._favorites_worker: Any | None = None
        self._new_additions_worker: Any | None = None
        self._highly_rated_worker: Any | None = None
        self._saves_rom_ids: set = set()
        self._saves_thread = None
        self._saves_worker = None
        self._toggle_pending_rom_id: str = ""
        self._toggle_adding: bool = False
        self._toggle_thread: threading.Thread | None = None
        self._toggle_worker: Any | None = None
        self._rom_meta_threads: dict[str, threading.Thread] = {}  # rom_id -> active thread
        self._rom_meta_workers: dict[str, Any] = {}
        self._lib_changed_debounce_timer: QTimer | None = None
        self._catalog_fetched: bool = False
        self._catalog_server_url: str = ""

    # ------------------------------------------------------------------
    # Config sync (called by MainWindow on mode switch)
    # ------------------------------------------------------------------

    @Slot(object)
    def syncConfig(self, config: dict[str, Any]) -> None:
        self._config = config
        incoming_url = str(config.get("server_url", "")).strip()
        if incoming_url != self._catalog_server_url:
            self._catalog_fetched = False
            self._platform_details = []
            self._catalog_server_url = incoming_url
        self.libraryGamesChanged.emit()
        self.platformsChanged.emit()
        view = self._config.get("tv_mode_home_view", "home")
        if not isinstance(view, str):
            view = "home"
        self.homeViewTabChanged.emit(view)

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    def _server_metadata_index(self) -> dict[str, dict]:
        index: dict[str, dict] = {}
        all_sources: list[list] = list(self._server_games.values()) + [
            self._favorites_games,
            self._new_additions_games,
            self._highly_rated_games,
        ]
        for games in all_sources:
            if not isinstance(games, list):
                continue
            for g in games:
                if not isinstance(g, dict):
                    continue
                rid = g.get("rom_id", "")
                if rid and rid not in index:
                    index[rid] = g
        return index

    @Property(list, notify=libraryGamesChanged)
    def libraryGames(self) -> list[dict[str, str]]:
        games = self._config.get("installed_games", [])
        if not isinstance(games, list):
            return []
        _has_server_data = bool(self._server_games or self._favorites_games or self._new_additions_games or self._highly_rated_games)
        server_index = self._server_metadata_index() if _has_server_data else {}
        _favorite_rom_ids = {g.get("rom_id", "") for g in self._favorites_games if isinstance(g, dict) and g.get("rom_id")}
        _META_FIELDS = (
            "first_release_date",
            "release_year",
            "companies",
            "languages",
            "revision",
            "fanart_url",
            "cover_url",
            "genres",
            "rating",
        )
        result = []
        for g in games:
            if not isinstance(g, dict):
                continue
            if g.get("platform", "").strip().casefold() == "emulators":
                continue
            if not g.get("local_path"):
                path = self._resolve_game_path(g)
                if path:
                    g = {**g, "local_path": path}
            # Merge missing metadata from server catalog
            if server_index:
                rid = g.get("rom_id", "")
                if rid and rid in server_index:
                    server_game = server_index[rid]
                    extras = {k: server_game[k] for k in _META_FIELDS if k in server_game and not g.get(k)}
                    if extras:
                        g = {**g, **extras}
            rid = g.get("rom_id", "")
            if rid:
                g = {**g, "has_cloud_saves": "true" if rid in self._saves_rom_ids else "false"}
            g = {**g, "is_favorite": "true" if (rid and rid in _favorite_rom_ids) else "false"}
            result.append(g)
        return result

    @Property(list, notify=favoritesGamesChanged)
    def favoritesGames(self) -> list:
        return [{**g, "is_favorite": "true"} for g in self._enrich_with_local_paths(self._favorites_games)]

    @Property(list, notify=newAdditionsGamesChanged)
    def newAdditionsGames(self) -> list:
        _favorite_rom_ids = {g.get("rom_id", "") for g in self._favorites_games if isinstance(g, dict) and g.get("rom_id")}
        return [{**g, "is_favorite": "true" if g.get("rom_id") and g.get("rom_id") in _favorite_rom_ids else "false"}
            for g in self._enrich_with_local_paths(self._new_additions_games)]

    @Property(list, notify=highlyRatedGamesChanged)
    def highlyRatedGames(self) -> list:
        _favorite_rom_ids = {g.get("rom_id", "") for g in self._favorites_games if isinstance(g, dict) and g.get("rom_id")}
        return [{**g, "is_favorite": "true" if g.get("rom_id") and g.get("rom_id") in _favorite_rom_ids else "false"}
            for g in self._enrich_with_local_paths(self._highly_rated_games)]

    @Property(list, notify=platformsChanged)
    def platforms(self) -> list[str]:
        return sorted(self._platforms.keys())

    @Property(list, notify=platformsChanged)
    def platformDetails(self) -> list[dict]:
        return self._platform_details

    @Property(bool, notify=connectionStatusChanged)
    def isConnected(self) -> bool:
        return self._is_connected

    @Property(list, notify=exclusionDataChanged)
    def tvGuideExclusionList(self) -> list[str]:
        opt_outs_raw = self._config.get("tv_guide_button_default_opt_outs")
        opt_outs = {e.lower() for e in opt_outs_raw if isinstance(e, str)} if isinstance(opt_outs_raw, list) else set()
        effective_defaults = [e for e in self._DEFAULT_EXCLUSION_LIST if e.lower() not in opt_outs]

        value = self._config.get("tv_guide_button_exclusion_list")
        if not isinstance(value, list):
            return effective_defaults
        default_lower = {e.lower() for e in effective_defaults}
        extras = [e for e in value if isinstance(e, str) and e.lower() not in default_lower]
        return effective_defaults + extras

    @Property(list, notify=exclusionDataChanged)
    def availableEmulatorNames(self) -> list[str]:
        emulators = self._config.get("emulators", [])
        if not isinstance(emulators, list):
            return []
        exclusion_set = {e.lower() for e in self.tvGuideExclusionList}
        names: list[str] = []
        for e in emulators:
            if not isinstance(e, dict):
                continue
            name = e.get("name")
            if not isinstance(name, str) or not name.strip():
                continue
            if name.strip().lower() not in exclusion_set:
                names.append(name.strip())
        return sorted(names)

    @Property(str, notify=homeViewTabChanged)
    def homeViewTab(self) -> str:
        value = self._config.get("tv_mode_home_view", "home")
        if not isinstance(value, str):
            return "home"
        return value

    @Property(str, notify=libraryGamesChanged)
    def serverUrl(self) -> str:
        value = self._config.get("server_url", "")
        if not isinstance(value, str):
            return ""
        return value

    @Property(bool, notify=libraryGamesChanged)
    def isAutoSync(self) -> bool:
        return bool(self._config.get("auto_cloud_save_download_on_launch", True))

    @Property(bool, notify=overlayStateChanged)
    def uiOverlayActive(self) -> bool:
        return self._ui_overlay_active

    # ------------------------------------------------------------------
    # Slots
    # ------------------------------------------------------------------

    @Slot()
    def requestDesktopMode(self) -> None:
        self.switchToDesktopModeRequested.emit()

    @Slot(list)
    def setGuideExclusionList(self, entries: list) -> None:
        self._set_guide_exclusion_list_from_values(entries)

    @Slot("QVariant")
    def saveExclusionList(self, names: object) -> None:
        values: Any = names
        if hasattr(names, "toVariant"):
            try:
                values = names.toVariant()
            except Exception:
                values = names
        self._set_guide_exclusion_list_from_values(values)

    @Slot(str)
    def addExclusionEntry(self, name: str) -> None:
        cleaned = name.strip()
        if not cleaned:
            return
        current = self._normalized_exclusion_list()
        if cleaned.lower() in {entry.lower() for entry in current}:
            return
        current.append(cleaned)
        self._config["tv_guide_button_exclusion_list"] = current
        self._saveConfigToDisk()
        self.exclusionListChanged.emit(current)
        self.exclusionDataChanged.emit()
        self.libraryGamesChanged.emit()

    @Slot(str)
    def removeExclusionEntry(self, name: str) -> None:
        cleaned = name.strip()
        if not cleaned:
            return
        current = self._normalized_exclusion_list()
        filtered = [entry for entry in current if entry.lower() != cleaned.lower()]
        if filtered == current:
            return
        self._config["tv_guide_button_exclusion_list"] = filtered
        self._saveConfigToDisk()
        self.exclusionListChanged.emit(filtered)
        self.exclusionDataChanged.emit()
        self.libraryGamesChanged.emit()

    @Slot(str)
    def setHomeView(self, view: str) -> None:
        self._set_home_view_tab(view)

    @Slot(str)
    def setHomeViewTab(self, view: str) -> None:
        self._set_home_view_tab(view)

    @Slot(bool)
    def setAutoSync(self, enabled: bool) -> None:
        self._config["auto_cloud_save_download_on_launch"] = bool(enabled)
        self._config["auto_cloud_save_upload_on_exit"] = bool(enabled)
        self._config.pop("auto_cloud_sync", None)
        self._saveConfigToDisk()
        self.autoSyncChanged.emit(bool(enabled))
        self.libraryGamesChanged.emit()
        self.exclusionDataChanged.emit()

    @Slot(bool)
    def setUiOverlayActive(self, active: bool) -> None:
        if self._ui_overlay_active != active:
            self._ui_overlay_active = active
            self.overlayStateChanged.emit()

    @Slot(str)
    def fetchRomMetadata(self, game_json: str) -> None:
        import json
        try:
            game_dict = json.loads(game_json) if game_json else {}
        except (json.JSONDecodeError, TypeError):
            return
        if not isinstance(game_dict, dict):
            return

        rom_id = str(game_dict.get("rom_id", "")).strip()
        if not rom_id:
            return

        _COMPLETE_FIELDS = ("genres", "description", "rating", "filesize_bytes", "companies", "first_release_date")
        _EMPTY_SENTINELS = {"", "N/A", "No description available."}
        all_complete = all(
            str(game_dict.get(f, "")).strip() not in _EMPTY_SENTINELS
            for f in _COMPLETE_FIELDS
        )
        if all_complete:
            return

        base_url = str(self._config.get("server_url", "")).strip()
        api_token = str(self._config.get("api_token", "")).strip()
        if not base_url or not api_token:
            return

        existing = self._rom_meta_threads.get(rom_id)
        if existing is not None and existing.is_alive():
            return

        worker = _RomMetaFetchWorker(base_url, api_token, rom_id)
        worker.finished.connect(self._on_rom_meta_finished)
        self.romMetadataFetchStarted.emit(rom_id)
        t = threading.Thread(target=worker.run, daemon=True)
        self._rom_meta_threads[rom_id] = t
        self._rom_meta_workers[rom_id] = worker
        t.start()

    @Slot()
    def connectToServer(self) -> None:
        from rom_mate.server.state import credentials_present
        if not credentials_present(self._config):
            self.connectionStatusChanged.emit("No credentials configured")
            return
        if self._catalog_fetched:
            return
        self._start_catalog_fetch()

    @Slot(str, result=list)
    def serverGamesForPlatform(self, platform_label: str) -> list[dict[str, str]]:
        games = self._enrich_with_local_paths(self._server_games.get(platform_label, []))
        _favorite_rom_ids = {g.get("rom_id", "") for g in self._favorites_games if isinstance(g, dict) and g.get("rom_id")}
        result = []
        for g in games:
            if not isinstance(g, dict):
                result.append(g)
                continue
            rid = g.get("rom_id", "")
            if rid:
                g = {**g, "has_cloud_saves": "true" if rid in self._saves_rom_ids else "false"}
            g = {**g, "is_favorite": "true" if (rid and rid in _favorite_rom_ids) else "false"}
            result.append(g)
        return result

    @Slot(str)
    def loadPlatformGames(self, platform_label: str) -> None:
        platform_id = self._platforms.get(platform_label)
        if platform_id is None:
            return
        if platform_label in self._server_games:
            self.serverGamesChanged.emit(platform_label)
            return
        self._start_rom_fetch(platform_label, platform_id)

    @Slot(str)
    def logHandleDiag(self, label: str) -> None:
        if self._handle_diag_fn is not None:
            self._handle_diag_fn(label)

    @Slot(str, result=str)
    def getInstalledLocalPath(self, rom_id: str) -> str:
        if not rom_id:
            return ""
        installed_games = self._config.get("installed_games", [])
        if not isinstance(installed_games, list):
            return ""
        for installed in installed_games:
            if not isinstance(installed, dict):
                continue
            if str(installed.get("rom_id", "") or installed.get("id", "")) == rom_id:
                return self._resolve_game_path(installed)
        return ""

    @Slot(str)
    def toggleFavorite(self, rom_id_str: str) -> None:
        if not rom_id_str:
            return
        base_url = self._config.get("server_url", "")
        api_token = self._config.get("api_token", "")
        if not base_url or not api_token:
            return
        if _is_thread_running(self._toggle_thread):
            return
        self._toggle_pending_rom_id = rom_id_str
        from rom_mate.tv.bridge.workers import CollectionsFetchWorker
        worker = CollectionsFetchWorker(self._api_get, parent=None)
        worker.finished.connect(self._on_toggle_collections_fetched, Qt.ConnectionType.QueuedConnection)
        worker.error.connect(self._on_toggle_error, Qt.ConnectionType.QueuedConnection)
        self._toggle_worker = worker
        t = threading.Thread(target=worker.run, daemon=True)
        self._toggle_thread = t
        t.start()

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    @staticmethod
    def _resolve_game_path(game: dict) -> str:
        """Return the first non-empty file path from any known path key."""
        for key in ("local_path", "extracted_path", "extracted_dir", "archive_path"):
            val = game.get(key, "")
            if isinstance(val, str) and val.strip():
                return val.strip()
        return ""

    def _installed_local_paths(self) -> dict[str, str]:
        result: dict[str, str] = {}
        installed_games = self._config.get("installed_games", [])
        if not isinstance(installed_games, list):
            return result
        for g in installed_games:
            if not isinstance(g, dict):
                continue
            rom_id = str(g.get("rom_id", "") or g.get("id", "")).strip()
            path = self._resolve_game_path(g)
            if rom_id and path:
                result[rom_id] = path
        return result

    def _enrich_with_local_paths(self, games: list) -> list:
        local_paths = self._installed_local_paths()
        if not local_paths:
            return games
        result = []
        for game in games:
            if not isinstance(game, dict):
                result.append(game)
                continue
            rom_id = str(game.get("rom_id", "") or game.get("id", "")).strip()
            lp = local_paths.get(rom_id, "")
            if lp and not game.get("local_path"):
                result.append({**game, "local_path": lp})
            else:
                result.append(game)
        return result

    def _api_get(self, path: str, params: dict | None) -> Any:
        from rom_mate.core.api import api_get_json
        from rom_mate.server.state import server_base_url
        base_url = server_base_url(self._config)
        api_token = self._config.get("api_token", "")
        return api_get_json(base_url, api_token, path, params)

    def _api_put_multipart_text(self, path: str, fields: dict, params=None):
        from rom_mate.core.api import api_put_multipart_text_json
        from rom_mate.server.state import server_base_url
        return api_put_multipart_text_json(
            server_base_url(self._config),
            self._config.get("api_token", ""),
            path, fields, params
        )

    def _api_post_multipart_text(self, path: str, fields: dict, params=None):
        from rom_mate.core.api import api_post_multipart_text_json
        from rom_mate.server.state import server_base_url
        return api_post_multipart_text_json(
            server_base_url(self._config),
            self._config.get("api_token", ""),
            path, fields, params
        )

    def _saveConfigToDisk(self) -> None:
        config_dir = Path.home() / ".rom-mate"
        config_file = config_dir / "config.json"
        try:
            _write_config_file(config_dir, config_file, self._config)
        except OSError:
            pass

    def _normalized_exclusion_list(self) -> list[str]:
        value = self._config.get("tv_guide_button_exclusion_list")
        if not isinstance(value, list):
            return []
        normalized: list[str] = []
        for entry in value:
            cleaned = str(entry).strip()
            if cleaned:
                normalized.append(cleaned)
        return normalized

    def _set_guide_exclusion_list_from_values(self, values: Any) -> None:
        if not isinstance(values, list):
            return
        cleaned_list: list[str] = []
        for name in values:
            cleaned = str(name).strip()
            if cleaned:
                cleaned_list.append(cleaned)
        self._config["tv_guide_button_exclusion_list"] = cleaned_list
        self._saveConfigToDisk()
        self.exclusionListChanged.emit(cleaned_list)
        self.exclusionDataChanged.emit()
        self.libraryGamesChanged.emit()

    def _set_home_view_tab(self, view: str) -> None:
        if view not in ("home", "library", "server"):
            return
        self._config["tv_mode_home_view"] = view
        self._saveConfigToDisk()
        self.homeViewTabChanged.emit(view)
        self.libraryGamesChanged.emit()

    def _start_favorites_fetch(self) -> None:
        from rom_mate.tv.bridge.workers import FavoritesRomFetchWorker
        base_url = self._config.get("server_url", "")
        if not isinstance(base_url, str):
            base_url = ""

        if not _is_thread_running(self._favorites_thread):
            worker = FavoritesRomFetchWorker(self._api_get, base_url, parent=None)
            worker.finished.connect(self._on_favorites_finished)
            worker.error.connect(self._on_favorites_error)
            self._favorites_worker = worker
            t = threading.Thread(target=worker.run, daemon=True)
            self._favorites_thread = t
            t.start()

    def _start_curated_rows_fetch(self) -> None:
        from rom_mate.tv.bridge.workers import (
            NewAdditionsRomFetchWorker,
            HighlyRatedRomFetchWorker,
        )
        base_url = self._config.get("server_url", "")
        if not isinstance(base_url, str):
            base_url = ""

        self._start_favorites_fetch()

        if not _is_thread_running(self._new_additions_thread):
            worker = NewAdditionsRomFetchWorker(self._api_get, base_url, parent=None)
            worker.finished.connect(self._on_new_additions_finished)
            worker.error.connect(self._on_new_additions_error)
            self._new_additions_worker = worker
            t = threading.Thread(target=worker.run, daemon=True)
            self._new_additions_thread = t
            t.start()

        if not _is_thread_running(self._highly_rated_thread):
            worker = HighlyRatedRomFetchWorker(self._api_get, base_url, parent=None)
            worker.finished.connect(self._on_highly_rated_finished)
            worker.error.connect(self._on_highly_rated_error)
            self._highly_rated_worker = worker
            t = threading.Thread(target=worker.run, daemon=True)
            self._highly_rated_thread = t
            t.start()

    def _start_saves_fetch(self) -> None:
        from rom_mate.tv.bridge.workers import SavesBatchFetchWorker
        if _is_thread_running(self._saves_thread):
            return
        worker = SavesBatchFetchWorker(self._api_get)
        worker.finished.connect(self._on_saves_finished)
        worker.error.connect(self._on_saves_error)
        self._saves_worker = worker
        t = threading.Thread(target=worker.run, daemon=True)
        self._saves_thread = t
        t.start()

    def _start_catalog_fetch(self) -> None:
        from rom_mate.tv.bridge.workers import CatalogFetchWorker
        if _is_thread_running(self._catalog_thread):
            return
        worker = CatalogFetchWorker(self._api_get)
        worker.finished.connect(self._on_catalog_finished)
        worker.error.connect(self._on_catalog_error)
        self._catalog_worker = worker
        t = threading.Thread(target=worker.run, daemon=True)
        self._catalog_thread = t
        t.start()

    def _start_rom_fetch(self, platform_label: str, platform_id: int) -> None:
        from rom_mate.tv.bridge.workers import RomListFetchWorker
        existing = self._rom_threads.get(platform_label)
        if existing is not None and existing.is_alive():
            return
        library_games = self._config.get("installed_games", [])
        base_url = self._config.get("server_url", "")
        if not isinstance(base_url, str):
            base_url = ""
        worker = RomListFetchWorker(self._api_get, platform_label, platform_id, library_games, base_url)
        worker.finished.connect(self._on_roms_finished)
        worker.error.connect(self._on_roms_error)
        self._rom_workers[platform_label] = worker
        t = threading.Thread(target=worker.run, daemon=True)
        self._rom_threads[platform_label] = t
        t.start()

    def _on_rom_fetch_thread_done(self) -> None:
        for platform_label, thread in list(self._rom_threads.items()):
            if not _is_thread_running(thread):
                self._rom_threads.pop(platform_label, None)
                self._rom_workers.pop(platform_label, None)

    def _on_catalog_finished(self, bundle: Any) -> None:
        if not isinstance(bundle, dict):
            return
        me_payload = bundle.get("me")
        platforms_payload = bundle.get("platforms")
        from rom_mate.server.catalog import server_platform_details, server_platform_ids
        from rom_mate.server.state import account_status_text
        self._platforms = server_platform_ids(platforms_payload)
        self._platform_details = server_platform_details(platforms_payload)
        self._is_connected = bool(self._platforms)
        self._catalog_fetched = True
        self._catalog_server_url = str(self._config.get("server_url", "")).strip()
        username = ""
        if isinstance(me_payload, dict):
            username = me_payload.get("username", "")
        if isinstance(username, str) and username.strip():
            self._config["username"] = username.strip()
        status = account_status_text(self._config, self._is_connected)
        self.platformsChanged.emit()
        self.connectionStatusChanged.emit(status)
        self._start_curated_rows_fetch()
        self._start_saves_fetch()
        self._catalog_thread = None
        self._catalog_worker = None

    def _on_catalog_error(self, message: str) -> None:
        from rom_mate.server.connection import classify_connection_failure
        self._is_connected = False
        self.connectionStatusChanged.emit(f"Connection failed: {message}")
        self._catalog_thread = None
        self._catalog_worker = None

    def _on_roms_finished(self, bundle: Any) -> None:
        if not isinstance(bundle, dict):
            return
        platform_label = bundle.get("platform_label", "")
        games = bundle.get("games", [])
        self._server_games[platform_label] = games
        self.serverGamesChanged.emit(platform_label)
        self._rom_threads.pop(platform_label, None)
        self._rom_workers.pop(platform_label, None)

    def _on_roms_error(self, bundle: Any) -> None:
        if not isinstance(bundle, dict):
            return
        platform_label = bundle.get("platform_label", "")
        self._rom_threads.pop(platform_label, None)
        self._rom_workers.pop(platform_label, None)

    def _on_favorites_finished(self, games: list) -> None:
        self._favorites_games = games
        self.favoritesGamesChanged.emit()
        self.libraryGamesChanged.emit()
        self._favorites_thread = None
        self._favorites_worker = None

    def _on_favorites_error(self, message: str) -> None:
        self._favorites_thread = None
        self._favorites_worker = None

    @Slot(object)
    def _on_toggle_collections_fetched(self, result) -> None:
        rom_id_int = int(self._toggle_pending_rom_id)
        if result is None:
            # No favorites collection exists yet — create one
            self._toggle_adding = True
            from rom_mate.tv.bridge.workers import CollectionCreateWorker
            worker = CollectionCreateWorker(self._api_post_multipart_text, rom_id_int, parent=None)
            worker.finished.connect(self._on_toggle_collection_created, Qt.ConnectionType.QueuedConnection)
            worker.error.connect(self._on_toggle_error, Qt.ConnectionType.QueuedConnection)
            self._toggle_worker = worker
            t = threading.Thread(target=worker.run, daemon=True)
            self._toggle_thread = t
            t.start()
        else:
            current_ids = list(result.get("rom_ids", []))
            if rom_id_int in current_ids:
                current_ids.remove(rom_id_int)
                self._toggle_adding = False
            else:
                current_ids.append(rom_id_int)
                self._toggle_adding = True
            from rom_mate.tv.bridge.workers import CollectionUpdateWorker
            worker = CollectionUpdateWorker(self._api_put_multipart_text, result["id"], current_ids, parent=None)
            worker.finished.connect(self._on_toggle_collection_updated, Qt.ConnectionType.QueuedConnection)
            worker.error.connect(self._on_toggle_error, Qt.ConnectionType.QueuedConnection)
            self._toggle_worker = worker
            t = threading.Thread(target=worker.run, daemon=True)
            self._toggle_thread = t
            t.start()

    @Slot(object)
    def _on_toggle_collection_updated(self, updated: object) -> None:
        self.favoriteToggleComplete.emit({
            "rom_id": self._toggle_pending_rom_id,
            "is_now_favorite": self._toggle_adding,
        })
        self._start_favorites_fetch()
        self._toggle_thread = None
        self._toggle_worker = None

    @Slot(object)
    def _on_toggle_collection_created(self, created: object) -> None:
        self.favoriteToggleComplete.emit({
            "rom_id": self._toggle_pending_rom_id,
            "is_now_favorite": self._toggle_adding,
        })
        self._start_favorites_fetch()
        self._toggle_thread = None
        self._toggle_worker = None

    @Slot(str)
    def _on_toggle_error(self, message: str) -> None:
        # Silent failure — button state reverts naturally since favoriteToggleComplete is not emitted
        self._toggle_thread = None
        self._toggle_worker = None

    def _on_new_additions_finished(self, games: list) -> None:
        self._new_additions_games = games
        self.newAdditionsGamesChanged.emit()
        self.libraryGamesChanged.emit()
        self._new_additions_thread = None
        self._new_additions_worker = None

    def _on_new_additions_error(self, message: str) -> None:
        self._new_additions_thread = None
        self._new_additions_worker = None

    def _on_highly_rated_finished(self, games: list) -> None:
        self._highly_rated_games = games
        self.highlyRatedGamesChanged.emit()
        self.libraryGamesChanged.emit()
        self._highly_rated_thread = None
        self._highly_rated_worker = None

    def _on_highly_rated_error(self, message: str) -> None:
        self._highly_rated_thread = None
        self._highly_rated_worker = None

    def _on_saves_finished(self, rom_ids):
        self._saves_rom_ids = set(rom_ids)
        self.libraryGamesChanged.emit()
        self._saves_thread = None
        self._saves_worker = None

    def _on_saves_error(self, message):
        self._saves_thread = None
        self._saves_worker = None

    def _on_rom_meta_finished(self, bundle: Any) -> None:
        import json
        if not isinstance(bundle, dict):
            return
        rom_id = str(bundle.get("rom_id", "")).strip()
        metadata_obj = bundle.get("metadata", {})
        if not isinstance(metadata_obj, dict):
            metadata_obj = {}
        self._rom_meta_threads.pop(rom_id, None)
        w = self._rom_meta_workers.pop(rom_id, None)
        if w is not None:
            w.deleteLater()
        self.romMetadataReady.emit({"rom_id": rom_id, "metadata_json": json.dumps(metadata_obj)})
        # Write metadata back to installed game record so libraryGames picks it up
        if metadata_obj:
            installed = self._config.get("installed_games", [])
            changed = False
            _EMPTY = {"", "N/A", "No description available."}
            for game in installed:
                if not isinstance(game, dict):
                    continue
                if str(game.get("rom_id", "")).strip() == rom_id:
                    for k, v in metadata_obj.items():
                        if v and str(v).strip() not in _EMPTY and not game.get(k):
                            game[k] = str(v)
                            changed = True
                    break
            if changed:
                self._schedule_lib_changed()
                self.saveConfigRequested.emit()

    def _schedule_lib_changed(self) -> None:
        if self._lib_changed_debounce_timer is None:
            self._lib_changed_debounce_timer = QTimer(self)
            self._lib_changed_debounce_timer.setSingleShot(True)
            self._lib_changed_debounce_timer.timeout.connect(self._flush_lib_changed)
        self._lib_changed_debounce_timer.start(300)

    def _flush_lib_changed(self) -> None:
        self.libraryGamesChanged.emit()
