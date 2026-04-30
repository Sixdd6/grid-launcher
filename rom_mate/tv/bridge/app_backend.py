from __future__ import annotations

import threading
from pathlib import Path
from typing import Any

from PySide6.QtCore import Property, QObject, Signal, Slot

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

    finished = Signal(str, object)  # rom_id, metadata_dict

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
                self.finished.emit(self._rom_id, {})
                return
            raw = details_metadata_from_item(payload)
            filtered = {k: v for k, v in raw.items() if v and v not in ("N/A", "No description available.")}
            self.finished.emit(self._rom_id, filtered)
        except Exception:
            self.finished.emit(self._rom_id, {})


class AppBackend(QObject):
    """Exposes config, library games, and server data to QML."""

    _DEFAULT_EXCLUSION_LIST = ["RPCS3", "Cemu", "Dolphin", "Xemu", "Xenia", "RetroArch"]

    libraryGamesChanged = Signal()
    platformsChanged = Signal()
    serverGamesChanged = Signal(str)      # platform_label
    connectionStatusChanged = Signal(str) # status text
    switchToDesktopModeRequested = Signal()
    exclusionListChanged = Signal(object)
    homeViewTabChanged = Signal(str)
    autoSyncChanged = Signal(bool)
    overlayStateChanged = Signal()
    favoritesGamesChanged = Signal()
    newAdditionsGamesChanged = Signal()
    highlyRatedGamesChanged = Signal()
    romMetadataReady = Signal(str, str)  # rom_id, metadata_json
    romMetadataFetchStarted = Signal(str)    # rom_id
    saveConfigRequested = Signal()

    def __init__(
        self,
        config: dict[str, Any],
        image_cache_dir: Path,
        parent: QObject | None = None,
    ) -> None:
        super().__init__(parent)
        self._config = config
        self._image_cache_dir = image_cache_dir
        self._is_connected = False
        self._platforms: dict[str, int] = {}       # label -> id
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
        self._rom_meta_threads: dict[str, threading.Thread] = {}  # rom_id -> active thread
        self._rom_meta_workers: dict[str, Any] = {}
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
        _META_FIELDS = ("first_release_date", "release_year", "companies", "languages", "revision", "fanart_url", "genres", "rating")
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
            result.append(g)
        return result

    @Property(list, notify=favoritesGamesChanged)
    def favoritesGames(self) -> list:
        return self._enrich_with_local_paths(self._favorites_games)

    @Property(list, notify=newAdditionsGamesChanged)
    def newAdditionsGames(self) -> list:
        return self._enrich_with_local_paths(self._new_additions_games)

    @Property(list, notify=highlyRatedGamesChanged)
    def highlyRatedGames(self) -> list:
        return self._enrich_with_local_paths(self._highly_rated_games)

    @Property(list, notify=platformsChanged)
    def platforms(self) -> list[str]:
        return sorted(self._platforms.keys())

    @Property(bool, notify=connectionStatusChanged)
    def isConnected(self) -> bool:
        return self._is_connected

    @Property(list, notify=libraryGamesChanged)
    def tvGuideExclusionList(self) -> list[str]:
        value = self._config.get("tv_guide_button_exclusion_list")
        if not isinstance(value, list):
            return self._DEFAULT_EXCLUSION_LIST
        default_lower = {e.lower() for e in self._DEFAULT_EXCLUSION_LIST}
        extras = [e for e in value if isinstance(e, str) and e.lower() not in default_lower]
        return self._DEFAULT_EXCLUSION_LIST + extras

    @Property(list, notify=libraryGamesChanged)
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
        return self._enrich_with_local_paths(self._server_games.get(platform_label, []))

    @Slot(str)
    def loadPlatformGames(self, platform_label: str) -> None:
        platform_id = self._platforms.get(platform_label)
        if platform_id is None:
            return
        if platform_label in self._server_games:
            self.serverGamesChanged.emit(platform_label)
            return
        self._start_rom_fetch(platform_label, platform_id)

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
        self.libraryGamesChanged.emit()

    def _set_home_view_tab(self, view: str) -> None:
        if view not in ("home", "library", "server"):
            return
        self._config["tv_mode_home_view"] = view
        self._saveConfigToDisk()
        self.homeViewTabChanged.emit(view)
        self.libraryGamesChanged.emit()

    def _start_curated_rows_fetch(self) -> None:
        from rom_mate.tv.bridge.workers import (
            FavoritesRomFetchWorker,
            NewAdditionsRomFetchWorker,
            HighlyRatedRomFetchWorker,
        )
        base_url = self._config.get("server_url", "")
        if not isinstance(base_url, str):
            base_url = ""

        if not _is_thread_running(self._favorites_thread):
            worker = FavoritesRomFetchWorker(self._api_get, base_url, parent=None)
            worker.finished.connect(self._on_favorites_finished)
            worker.error.connect(self._on_favorites_error)
            worker.finished.connect(self._on_favorites_thread_done)
            worker.error.connect(self._on_favorites_thread_done)
            self._favorites_worker = worker
            t = threading.Thread(target=worker.run, daemon=True)
            self._favorites_thread = t
            t.start()

        if not _is_thread_running(self._new_additions_thread):
            worker = NewAdditionsRomFetchWorker(self._api_get, base_url, parent=None)
            worker.finished.connect(self._on_new_additions_finished)
            worker.error.connect(self._on_new_additions_error)
            worker.finished.connect(self._on_new_additions_thread_done)
            worker.error.connect(self._on_new_additions_thread_done)
            self._new_additions_worker = worker
            t = threading.Thread(target=worker.run, daemon=True)
            self._new_additions_thread = t
            t.start()

        if not _is_thread_running(self._highly_rated_thread):
            worker = HighlyRatedRomFetchWorker(self._api_get, base_url, parent=None)
            worker.finished.connect(self._on_highly_rated_finished)
            worker.error.connect(self._on_highly_rated_error)
            worker.finished.connect(self._on_highly_rated_thread_done)
            worker.error.connect(self._on_highly_rated_thread_done)
            self._highly_rated_worker = worker
            t = threading.Thread(target=worker.run, daemon=True)
            self._highly_rated_thread = t
            t.start()

    def _start_catalog_fetch(self) -> None:
        from rom_mate.tv.bridge.workers import CatalogFetchWorker
        if _is_thread_running(self._catalog_thread):
            return
        worker = CatalogFetchWorker(self._api_get)
        worker.finished.connect(self._on_catalog_finished)
        worker.error.connect(self._on_catalog_error)
        worker.finished.connect(self._on_catalog_thread_done)
        worker.error.connect(self._on_catalog_thread_done)
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
        worker.finished.connect(self._on_rom_fetch_thread_done)
        worker.error.connect(self._on_rom_fetch_thread_done)
        self._rom_workers[platform_label] = worker
        t = threading.Thread(target=worker.run, daemon=True)
        self._rom_threads[platform_label] = t
        t.start()

    def _on_rom_fetch_thread_done(self, *args: Any) -> None:
        # Remove any finished thread entries from the dict
        finished = [label for label, t in self._rom_threads.items() if not t.is_alive()]
        for label in finished:
            self._rom_threads.pop(label, None)
            self._rom_workers.pop(label, None)

    def _on_catalog_finished(self, me_payload: Any, platforms_payload: Any) -> None:
        from rom_mate.server.catalog import server_platform_ids
        from rom_mate.server.state import account_status_text
        self._platforms = server_platform_ids(platforms_payload)
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

    def _on_catalog_error(self, message: str) -> None:
        from rom_mate.server.connection import classify_connection_failure
        self._is_connected = False
        self.connectionStatusChanged.emit(f"Connection failed: {message}")

    def _on_roms_finished(self, platform_label: str, games: list) -> None:
        self._server_games[platform_label] = games
        self.serverGamesChanged.emit(platform_label)

    def _on_roms_error(self, platform_label: str, message: str) -> None:
        pass

    def _on_favorites_finished(self, games: list) -> None:
        self._favorites_games = games
        self.favoritesGamesChanged.emit()
        self.libraryGamesChanged.emit()

    def _on_favorites_error(self, message: str) -> None:
        pass

    def _on_new_additions_finished(self, games: list) -> None:
        self._new_additions_games = games
        self.newAdditionsGamesChanged.emit()
        self.libraryGamesChanged.emit()

    def _on_new_additions_error(self, message: str) -> None:
        pass

    def _on_highly_rated_finished(self, games: list) -> None:
        self._highly_rated_games = games
        self.highlyRatedGamesChanged.emit()
        self.libraryGamesChanged.emit()

    def _on_highly_rated_error(self, message: str) -> None:
        pass

    def _on_catalog_thread_done(self, *args: Any) -> None:
        self._catalog_thread = None
        self._catalog_worker = None

    def _on_favorites_thread_done(self, *args: Any) -> None:
        self._favorites_thread = None
        self._favorites_worker = None

    def _on_new_additions_thread_done(self, *args: Any) -> None:
        self._new_additions_thread = None
        self._new_additions_worker = None

    def _on_highly_rated_thread_done(self, *args: Any) -> None:
        self._highly_rated_thread = None
        self._highly_rated_worker = None

    def _on_rom_meta_finished(self, rom_id: str, metadata: dict) -> None:
        import json
        self._rom_meta_threads.pop(rom_id, None)
        w = self._rom_meta_workers.pop(rom_id, None)
        if w is not None:
            w.deleteLater()
        self.romMetadataReady.emit(rom_id, json.dumps(metadata))
        # Write metadata back to installed game record so libraryGames picks it up
        if metadata:
            installed = self._config.get("installed_games", [])
            changed = False
            _EMPTY = {"", "N/A", "No description available."}
            for game in installed:
                if not isinstance(game, dict):
                    continue
                if str(game.get("rom_id", "")).strip() == rom_id:
                    for k, v in metadata.items():
                        if v and str(v).strip() not in _EMPTY and not game.get(k):
                            game[k] = str(v)
                            changed = True
                    break
            if changed:
                self.libraryGamesChanged.emit()
                self.saveConfigRequested.emit()
