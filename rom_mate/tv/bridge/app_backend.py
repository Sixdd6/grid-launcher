from __future__ import annotations

from pathlib import Path
from typing import Any

from PySide6.QtCore import Property, QObject, QThread, Signal, Slot

from rom_mate.core.config import write_config_file as _write_config_file


class AppBackend(QObject):
    """Exposes config, library games, and server data to QML."""

    _DEFAULT_EXCLUSION_LIST = ["RPCS3", "Cemu", "Dolphin", "Xemu", "Xenia"]

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
        self._catalog_thread: QThread | None = None
        self._catalog_worker: Any | None = None
        self._rom_threads: dict[str, QThread] = {}
        self._rom_workers: dict[str, Any] = {}
        self._ui_overlay_active: bool = False
        self._favorites_games: list = []
        self._new_additions_games: list = []
        self._highly_rated_games: list = []
        self._favorites_thread: QThread | None = None
        self._new_additions_thread: QThread | None = None
        self._highly_rated_thread: QThread | None = None
        self._favorites_worker: Any | None = None
        self._new_additions_worker: Any | None = None
        self._highly_rated_worker: Any | None = None

    # ------------------------------------------------------------------
    # Config sync (called by MainWindow on mode switch)
    # ------------------------------------------------------------------

    @Slot(object)
    def syncConfig(self, config: dict[str, Any]) -> None:
        self._config = config
        self.libraryGamesChanged.emit()
        self.platformsChanged.emit()
        view = self._config.get("tv_mode_home_view", "home")
        if not isinstance(view, str):
            view = "home"
        self.homeViewTabChanged.emit(view)

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @Property(list, notify=libraryGamesChanged)
    def libraryGames(self) -> list[dict[str, str]]:
        games = self._config.get("installed_games", [])
        if not isinstance(games, list):
            return []
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
        return value

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

    @Slot()
    def connectToServer(self) -> None:
        from rom_mate.server.state import credentials_present
        if not credentials_present(self._config):
            self.connectionStatusChanged.emit("No credentials configured")
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

        if self._favorites_thread is None or not self._favorites_thread.isRunning():
            worker = FavoritesRomFetchWorker(self._api_get, base_url, parent=None)
            thread = QThread(self)
            worker.moveToThread(thread)
            thread.started.connect(worker.run)
            worker.finished.connect(self._on_favorites_finished)
            worker.error.connect(self._on_favorites_error)
            worker.finished.connect(thread.quit)
            worker.error.connect(thread.quit)
            worker.finished.connect(worker.deleteLater)
            worker.error.connect(worker.deleteLater)
            thread.finished.connect(thread.deleteLater)
            thread.finished.connect(lambda: setattr(self, "_favorites_thread", None))
            thread.finished.connect(lambda: setattr(self, "_favorites_worker", None))
            self._favorites_thread = thread
            self._favorites_worker = worker
            thread.start()

        if self._new_additions_thread is None or not self._new_additions_thread.isRunning():
            worker = NewAdditionsRomFetchWorker(self._api_get, base_url, parent=None)
            thread = QThread(self)
            worker.moveToThread(thread)
            thread.started.connect(worker.run)
            worker.finished.connect(self._on_new_additions_finished)
            worker.error.connect(self._on_new_additions_error)
            worker.finished.connect(thread.quit)
            worker.error.connect(thread.quit)
            worker.finished.connect(worker.deleteLater)
            worker.error.connect(worker.deleteLater)
            thread.finished.connect(thread.deleteLater)
            thread.finished.connect(lambda: setattr(self, "_new_additions_thread", None))
            thread.finished.connect(lambda: setattr(self, "_new_additions_worker", None))
            self._new_additions_thread = thread
            self._new_additions_worker = worker
            thread.start()

        if self._highly_rated_thread is None or not self._highly_rated_thread.isRunning():
            worker = HighlyRatedRomFetchWorker(self._api_get, base_url, parent=None)
            thread = QThread(self)
            worker.moveToThread(thread)
            thread.started.connect(worker.run)
            worker.finished.connect(self._on_highly_rated_finished)
            worker.error.connect(self._on_highly_rated_error)
            worker.finished.connect(thread.quit)
            worker.error.connect(thread.quit)
            worker.finished.connect(worker.deleteLater)
            worker.error.connect(worker.deleteLater)
            thread.finished.connect(thread.deleteLater)
            thread.finished.connect(lambda: setattr(self, "_highly_rated_thread", None))
            thread.finished.connect(lambda: setattr(self, "_highly_rated_worker", None))
            self._highly_rated_thread = thread
            self._highly_rated_worker = worker
            thread.start()

    def _start_catalog_fetch(self) -> None:
        from rom_mate.tv.bridge.workers import CatalogFetchWorker
        if self._catalog_thread is not None and self._catalog_thread.isRunning():
            return
        worker = CatalogFetchWorker(self._api_get)
        thread = QThread(self)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.finished.connect(self._on_catalog_finished)
        worker.error.connect(self._on_catalog_error)
        worker.finished.connect(thread.quit)
        worker.error.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        worker.error.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        self._catalog_thread = thread
        self._catalog_worker = worker
        thread.finished.connect(lambda: setattr(self, "_catalog_thread", None))
        thread.finished.connect(lambda: setattr(self, "_catalog_worker", None))
        thread.start()

    def _start_rom_fetch(self, platform_label: str, platform_id: int) -> None:
        from rom_mate.tv.bridge.workers import RomListFetchWorker
        if platform_label in self._rom_threads and self._rom_threads[platform_label].isRunning():
            return
        library_games = self._config.get("installed_games", [])
        worker = RomListFetchWorker(self._api_get, platform_label, platform_id, library_games)
        thread = QThread(self)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.finished.connect(self._on_roms_finished)
        worker.error.connect(self._on_roms_error)
        worker.finished.connect(thread.quit)
        worker.error.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        worker.error.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(lambda: self._rom_threads.pop(platform_label, None))
        thread.finished.connect(lambda: self._rom_workers.pop(platform_label, None))
        self._rom_threads[platform_label] = thread
        self._rom_workers[platform_label] = worker
        thread.start()

    def _on_catalog_finished(self, me_payload: Any, platforms_payload: Any) -> None:
        from rom_mate.server.catalog import server_platform_ids
        from rom_mate.server.state import account_status_text
        self._platforms = server_platform_ids(platforms_payload)
        self._is_connected = bool(self._platforms)
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

    def _on_favorites_error(self, message: str) -> None:
        pass

    def _on_new_additions_finished(self, games: list) -> None:
        self._new_additions_games = games
        self.newAdditionsGamesChanged.emit()

    def _on_new_additions_error(self, message: str) -> None:
        pass

    def _on_highly_rated_finished(self, games: list) -> None:
        self._highly_rated_games = games
        self.highlyRatedGamesChanged.emit()

    def _on_highly_rated_error(self, message: str) -> None:
        pass
