from __future__ import annotations

import json
import os
import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication

_app = QApplication.instance() or QApplication(sys.argv)


class TestAppBackendConfigDefaults(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_tv_guide_exclusion_list_empty_when_key_missing(self):
        backend = self._make_backend({})
        self.assertEqual(backend.tvGuideExclusionList, [])

    def test_tv_guide_exclusion_list_returns_config_entries(self):
        backend = self._make_backend({"tv_guide_button_exclusion_list": ["RPCS3", "Cemu"]})
        self.assertEqual(backend.tvGuideExclusionList, ["RPCS3", "Cemu"])

    def test_tv_guide_exclusion_list_invalid_type_returns_empty(self):
        backend = self._make_backend({"tv_guide_button_exclusion_list": "bad"})
        self.assertEqual(backend.tvGuideExclusionList, [])

    def test_tv_guide_exclusion_list_empty_list_returns_empty(self):
        backend = self._make_backend({"tv_guide_button_exclusion_list": []})
        self.assertEqual(backend.tvGuideExclusionList, [])

    def test_home_view_default(self):
        backend = self._make_backend({})
        self.assertEqual(backend.homeViewTab, "home")

    def test_home_view_from_config(self):
        backend = self._make_backend({"tv_mode_home_view": "library"})
        self.assertEqual(backend.homeViewTab, "library")

    def test_home_view_invalid_type_returns_default(self):
        backend = self._make_backend({"tv_mode_home_view": 42})
        self.assertEqual(backend.homeViewTab, "home")

    def test_library_games_empty_when_no_installed_games(self):
        backend = self._make_backend({})
        self.assertEqual(backend.libraryGames, [])

    def test_library_games_returns_installed_games(self):
        games = [{"title": "Doom", "platform": "PC"}]
        backend = self._make_backend({"installed_games": games})
        self.assertEqual(backend.libraryGames, [{"title": "Doom", "platform": "PC", "is_favorite": "false"}])

    def test_library_games_excludes_emulator_platform_entries(self):
        games = [
            {"title": "Doom", "platform": "PC"},
            {"title": "RetroArch", "platform": "emulators"},
            {"title": "Dolphin", "platform": "Emulators"},
        ]
        backend = self._make_backend({"installed_games": games})
        result = backend.libraryGames
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["title"], "Doom")

    def test_library_games_invalid_type_returns_empty(self):
        backend = self._make_backend({"installed_games": "bad"})
        self.assertEqual(backend.libraryGames, [])


class TestAppBackendSetGuideExclusionList(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    @patch("grid_launcher.tv.bridge.app_backend._write_config_file")
    def test_set_guide_exclusion_list_updates_config(self, mock_write):
        backend = self._make_backend({})
        backend.setGuideExclusionList(["RPCS3", "Dolphin"])
        self.assertEqual(backend._config["tv_guide_button_exclusion_list"], ["RPCS3", "Dolphin"])

    @patch("grid_launcher.tv.bridge.app_backend._write_config_file")
    def test_set_guide_exclusion_list_persists(self, mock_write):
        backend = self._make_backend({})
        backend.setGuideExclusionList(["RPCS3"])
        self.assertTrue(mock_write.called)

    @patch("grid_launcher.tv.bridge.app_backend._write_config_file")
    def test_set_home_view_valid(self, mock_write):
        backend = self._make_backend({})
        backend.setHomeView("server")
        self.assertEqual(backend._config["tv_mode_home_view"], "server")

    @patch("grid_launcher.tv.bridge.app_backend._write_config_file")
    def test_set_home_view_invalid_ignored(self, mock_write):
        backend = self._make_backend({"tv_mode_home_view": "home"})
        backend.setHomeView("invalid_value")
        self.assertEqual(backend._config.get("tv_mode_home_view"), "home")
        mock_write.assert_not_called()


class TestAppBackendSyncConfig(unittest.TestCase):
    def test_sync_config_updates_library_games(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend
        backend = AppBackend({}, Path("/tmp/covers"))
        new_config = {"installed_games": [{"title": "Zelda", "platform": "N64"}]}
        backend.syncConfig(new_config)
        self.assertEqual(backend.libraryGames, [{"title": "Zelda", "platform": "N64", "is_favorite": "false"}])


class TestAppBackendConnectToServer(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_connect_to_server_emits_status_when_no_credentials(self):
        backend = self._make_backend({"server_url": "", "api_token": ""})
        emitted = []
        backend.connectionStatusChanged.connect(lambda s: emitted.append(s))
        backend.connectToServer()
        self.assertEqual(len(emitted), 1)
        self.assertIn("No credentials", emitted[0])

    def test_connect_to_server_skips_fetch_when_already_fetched(self):
        backend = self._make_backend({"server_url": "http://server", "api_token": "tok"})
        backend._catalog_fetched = True
        with patch.object(backend, "_start_catalog_fetch") as mock_fetch:
            backend.connectToServer()
        mock_fetch.assert_not_called()

    def test_sync_config_resets_catalog_fetched_when_url_differs(self):
        backend = self._make_backend({})
        backend._catalog_fetched = True
        backend._catalog_server_url = "http://old"
        backend.syncConfig({"server_url": "http://new"})
        self.assertFalse(backend._catalog_fetched)

    def test_sync_config_preserves_catalog_fetched_when_url_same(self):
        backend = self._make_backend({})
        backend._catalog_fetched = True
        backend._catalog_server_url = "http://same"
        backend.syncConfig({"server_url": "http://same"})
        self.assertTrue(backend._catalog_fetched)


class TestAppBackendSettings(unittest.TestCase):
    def setUp(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend

        self._write_patcher = patch("grid_launcher.tv.bridge.app_backend._write_config_file")
        self.mock_write = self._write_patcher.start()
        self.addCleanup(self._write_patcher.stop)

        self.backend = AppBackend({}, Path("/tmp/covers"))

    def test_add_exclusion_entry_appends(self):
        emitted = []
        self.backend.exclusionListChanged.connect(lambda values: emitted.append(values))

        self.backend.addExclusionEntry("Xemu")

        self.assertIn("Xemu", self.backend._config["tv_guide_button_exclusion_list"])
        self.assertEqual(emitted[-1], ["Xemu"])
        self.assertTrue(self.mock_write.called)

    def test_add_exclusion_entry_deduplicates(self):
        self.backend.addExclusionEntry("Xemu")
        self.backend.addExclusionEntry("xEmU")

        self.assertEqual(self.backend._config["tv_guide_button_exclusion_list"], ["Xemu"])

    def test_remove_exclusion_entry_removes(self):
        emitted = []
        self.backend.exclusionListChanged.connect(lambda values: emitted.append(values))

        self.backend.addExclusionEntry("Xemu")
        self.backend.removeExclusionEntry("xemu")

        self.assertEqual(self.backend._config["tv_guide_button_exclusion_list"], [])
        self.assertEqual(emitted[-1], [])

    def test_set_auto_sync_true(self):
        emitted = []
        self.backend.autoSyncChanged.connect(lambda value: emitted.append(value))

        self.backend.setAutoSync(True)

        self.assertTrue(self.backend._config["auto_cloud_save_download_on_launch"])
        self.assertTrue(self.backend._config["auto_cloud_save_upload_on_exit"])
        self.assertEqual(emitted[-1], True)

    def test_server_url_property(self):
        self.backend.syncConfig({"server_url": "http://myserver"})
        self.assertEqual(self.backend.serverUrl, "http://myserver")

    def test_is_auto_sync_defaults_true(self):
        self.assertTrue(self.backend.isAutoSync)

    def test_set_auto_sync_false(self):
        emitted = []
        self.backend.autoSyncChanged.connect(lambda value: emitted.append(value))

        self.backend.setAutoSync(False)

        self.assertFalse(self.backend._config["auto_cloud_save_download_on_launch"])
        self.assertFalse(self.backend._config["auto_cloud_save_upload_on_exit"])
        self.assertEqual(emitted[-1], False)

    def test_set_home_view_tab_valid(self):
        emitted = []
        self.backend.homeViewTabChanged.connect(lambda value: emitted.append(value))

        self.backend.setHomeViewTab("library")

        self.assertEqual(self.backend._config["tv_mode_home_view"], "library")
        self.assertEqual(emitted[-1], "library")

    def test_set_home_view_tab_invalid(self):
        emitted = []
        self.backend.homeViewTabChanged.connect(lambda value: emitted.append(value))
        self.backend._config["tv_mode_home_view"] = "home"

        self.backend.setHomeViewTab("invalid")

        self.assertEqual(self.backend._config["tv_mode_home_view"], "home")
        self.assertEqual(emitted, [])


class TestAppBackendOverlayState(unittest.TestCase):
    def test_ui_overlay_active_defaults_false(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        self.assertFalse(backend.uiOverlayActive)

    def test_set_ui_overlay_active_true(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        backend.setUiOverlayActive(True)
        self.assertTrue(backend.uiOverlayActive)

    def test_set_ui_overlay_active_false(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        backend.setUiOverlayActive(True)
        backend.setUiOverlayActive(False)
        self.assertFalse(backend.uiOverlayActive)

    def test_set_ui_overlay_active_emits_signal_on_change(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        fired = []
        backend.overlayStateChanged.connect(lambda: fired.append(1))
        backend.setUiOverlayActive(True)
        self.assertEqual(len(fired), 1)

    def test_set_ui_overlay_active_no_emit_if_unchanged(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        fired = []
        backend.overlayStateChanged.connect(lambda: fired.append(1))
        backend.setUiOverlayActive(False)
        self.assertEqual(len(fired), 0)


class TestAppBackendCuratedRows(unittest.TestCase):
    def _make_backend(self, config: dict = None) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config or {}, Path("/tmp/covers"))

    def test_favorites_games_initially_empty(self):
        backend = self._make_backend()
        self.assertEqual(backend.favoritesGames, [])

    def test_new_additions_games_initially_empty(self):
        backend = self._make_backend()
        self.assertEqual(backend.newAdditionsGames, [])

    def test_highly_rated_games_initially_empty(self):
        backend = self._make_backend()
        self.assertEqual(backend.highlyRatedGames, [])

    def test_on_favorites_finished_updates_list(self):
        backend = self._make_backend()
        backend._on_favorites_finished([{"title": "Tetris"}])
        self.assertEqual(backend.favoritesGames, [{"title": "Tetris", "is_favorite": "true"}])

    def test_on_new_additions_finished_updates_list(self):
        backend = self._make_backend()
        backend._on_new_additions_finished([{"title": "Doom"}])
        self.assertEqual(backend.newAdditionsGames, [{"title": "Doom", "is_favorite": "false"}])

    def test_on_highly_rated_finished_updates_list(self):
        backend = self._make_backend()
        backend._on_highly_rated_finished([{"title": "Half-Life"}])
        self.assertEqual(backend.highlyRatedGames, [{"title": "Half-Life", "is_favorite": "false"}])

    def test_favorites_finished_emits_library_games_changed(self):
        backend = self._make_backend()
        count = [0]
        backend.libraryGamesChanged.connect(lambda: count.__setitem__(0, count[0] + 1))
        backend._on_favorites_finished([{"title": "Tetris"}])
        self.assertEqual(count[0], 1)

    def test_new_additions_finished_emits_library_games_changed(self):
        backend = self._make_backend()
        count = [0]
        backend.libraryGamesChanged.connect(lambda: count.__setitem__(0, count[0] + 1))
        backend._on_new_additions_finished([{"title": "Doom"}])
        self.assertEqual(count[0], 1)

    def test_highly_rated_finished_emits_library_games_changed(self):
        backend = self._make_backend()
        count = [0]
        backend.libraryGamesChanged.connect(lambda: count.__setitem__(0, count[0] + 1))
        backend._on_highly_rated_finished([{"title": "Half-Life"}])
        self.assertEqual(count[0], 1)

    def test_catalog_finish_triggers_curated_fetch(self):
                from unittest.mock import patch
                from grid_launcher.tv.bridge.app_backend import AppBackend

                backend = AppBackend({}, Path("/tmp/covers"))
                with patch("grid_launcher.server.catalog.server_platform_ids", return_value={}), \
                         patch("grid_launcher.server.state.account_status_text", return_value="Connected"), \
                         patch.object(backend, "_start_curated_rows_fetch") as mock_curated, \
                         patch.object(backend, "_start_saves_fetch"):
                        backend._on_catalog_finished({"me": {}, "platforms": {}})
                        mock_curated.assert_called_once()

    def test_platform_details_empty_before_catalog_fetch(self):
        backend = self._make_backend()
        self.assertEqual(backend.platformDetails, [])

    def test_platform_details_populated_on_catalog_finished(self):
        backend = self._make_backend()
        expected_details = [{
            "slug": "snes",
            "name": "SNES",
            "rom_count": 5,
            "manufacturer": "Nintendo",
            "release_year": "1990",
            "player_count": "1-2 players",
        }]

        with patch("grid_launcher.server.catalog.server_platform_ids", return_value={"SNES": 42}), \
             patch("grid_launcher.server.catalog.server_platform_details", return_value=expected_details), \
             patch("grid_launcher.server.state.account_status_text", return_value="Connected"), \
               patch.object(backend, "_start_curated_rows_fetch"), \
               patch.object(backend, "_start_saves_fetch"):
            backend._on_catalog_finished({"me": {}, "platforms": {}})

        self.assertEqual(backend.platformDetails, expected_details)

    def test_platform_details_reset_on_url_change(self):
        backend = self._make_backend({"server_url": "http://old"})
        backend._catalog_server_url = "http://old"
        backend._platform_details = [{
            "slug": "snes",
            "name": "SNES",
            "rom_count": 5,
            "manufacturer": "Nintendo",
            "release_year": "1990",
            "player_count": "1-2 players",
        }]

        backend.syncConfig({"server_url": "http://new"})

        self.assertEqual(backend.platformDetails, [])

    def test_platform_details_notifies_platforms_changed(self):
        backend = self._make_backend()
        emissions = []
        expected_details = [{
            "slug": "snes",
            "name": "SNES",
            "rom_count": 5,
            "manufacturer": "Nintendo",
            "release_year": "1990",
            "player_count": "1-2 players",
        }]
        backend.platformsChanged.connect(lambda: emissions.append(1))

        with patch("grid_launcher.server.catalog.server_platform_ids", return_value={"SNES": 42}), \
             patch("grid_launcher.server.catalog.server_platform_details", return_value=expected_details), \
             patch("grid_launcher.server.state.account_status_text", return_value="Connected"), \
             patch.object(backend, "_start_curated_rows_fetch"), \
             patch.object(backend, "_start_saves_fetch"):
            backend._on_catalog_finished({"me": {}, "platforms": {}})

        self.assertGreaterEqual(len(emissions), 1)


class TestAppBackendSavesFetch(unittest.TestCase):
    def setUp(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend

        self.backend = AppBackend({}, Path("/tmp/covers"))

    def test_saves_rom_ids_initially_empty(self):
        self.assertEqual(self.backend._saves_rom_ids, set())

    def test_on_saves_finished_stores_rom_ids(self):
        self.backend._on_saves_finished(["1", "2"])
        self.assertEqual(self.backend._saves_rom_ids, {"1", "2"})

    def test_on_saves_finished_emits_library_games_changed(self):
        emitted = []
        self.backend.libraryGamesChanged.connect(lambda: emitted.append(True))
        self.backend._on_saves_finished(["1"])
        self.assertEqual(len(emitted), 1)

    def test_library_games_annotates_has_cloud_saves_true(self):
        self.backend._saves_rom_ids = {"42"}
        self.backend._config["installed_games"] = [
            {"title": "Game", "platform": "PS2", "rom_id": "42", "local_path": "/games/game.iso"}
        ]
        result = self.backend.libraryGames
        self.assertEqual(result[0]["has_cloud_saves"], "true")

    def test_library_games_annotates_has_cloud_saves_false_when_not_in_set(self):
        self.backend._saves_rom_ids = set()
        self.backend._config["installed_games"] = [
            {"title": "Game", "platform": "PS2", "rom_id": "42", "local_path": "/games/game.iso"}
        ]
        result = self.backend.libraryGames
        self.assertEqual(result[0]["has_cloud_saves"], "false")

    def test_library_games_no_has_cloud_saves_when_no_rom_id(self):
        self.backend._config["installed_games"] = [
            {"title": "Game", "platform": "PS2", "local_path": "/games/game.iso"}
        ]
        result = self.backend.libraryGames
        self.assertNotIn("has_cloud_saves", result[0])

    def test_catalog_finish_triggers_saves_fetch(self):
        me_payload = {}
        platforms_payload = {}
        with patch.object(self.backend, "_start_saves_fetch") as mock_saves, \
             patch.object(self.backend, "_start_curated_rows_fetch"), \
             patch("grid_launcher.server.catalog.server_platform_ids", return_value={}), \
             patch("grid_launcher.server.state.account_status_text", return_value="Connected"):
            self.backend._on_catalog_finished({"me": me_payload, "platforms": platforms_payload})
            mock_saves.assert_called_once()

    def test_start_saves_fetch_skips_when_thread_running(self):
        mock_thread = MagicMock()
        mock_thread.is_alive.return_value = True
        self.backend._saves_thread = mock_thread
        before = self.backend._saves_worker
        self.backend._start_saves_fetch()
        self.assertIsNone(before)
        self.assertIsNone(self.backend._saves_worker)


class TestAppBackendToggleFavorite(unittest.TestCase):
    def setUp(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend

        self.backend = AppBackend({}, Path("/tmp/covers"))

    def test_toggle_favorite_skips_when_no_credentials(self):
        self.backend.toggleFavorite("5")
        self.assertIsNone(self.backend._toggle_thread)

    def test_toggle_favorite_skips_when_thread_already_running(self):
        self.backend._config["server_url"] = "http://server"
        self.backend._config["api_token"] = "token"
        existing_thread = MagicMock()
        existing_thread.is_alive.return_value = True
        self.backend._toggle_thread = existing_thread

        self.backend.toggleFavorite("5")

        self.assertIs(self.backend._toggle_thread, existing_thread)

    @patch("grid_launcher.tv.bridge.app_backend.threading.Thread")
    @patch("grid_launcher.tv.bridge.workers.CollectionUpdateWorker")
    def test_toggle_favorite_adds_rom_to_existing_collection(self, mock_update_worker, mock_thread_cls):
        fake_thread = MagicMock()
        mock_thread_cls.return_value = fake_thread
        self.backend._toggle_pending_rom_id = "5"

        self.backend._on_toggle_collections_fetched({"id": 10, "rom_ids": [1, 2, 3]})

        args, kwargs = mock_update_worker.call_args
        self.assertEqual(args[1], 10)
        self.assertEqual(args[2], [1, 2, 3, 5])
        self.assertEqual(kwargs, {"parent": None})
        self.assertTrue(self.backend._toggle_adding)
        fake_thread.start.assert_called_once()

    @patch("grid_launcher.tv.bridge.app_backend.threading.Thread")
    @patch("grid_launcher.tv.bridge.workers.CollectionUpdateWorker")
    def test_toggle_favorite_removes_rom_from_existing_collection(self, mock_update_worker, mock_thread_cls):
        fake_thread = MagicMock()
        mock_thread_cls.return_value = fake_thread
        self.backend._toggle_pending_rom_id = "5"

        self.backend._on_toggle_collections_fetched({"id": 10, "rom_ids": [1, 2, 5]})

        args, kwargs = mock_update_worker.call_args
        self.assertEqual(args[1], 10)
        self.assertEqual(args[2], [1, 2])
        self.assertEqual(kwargs, {"parent": None})
        self.assertFalse(self.backend._toggle_adding)
        fake_thread.start.assert_called_once()

    @patch("grid_launcher.tv.bridge.app_backend.threading.Thread")
    @patch("grid_launcher.tv.bridge.workers.CollectionUpdateWorker")
    @patch("grid_launcher.tv.bridge.workers.CollectionCreateWorker")
    def test_toggle_favorite_creates_collection_when_none_exists(
        self,
        mock_create_worker,
        mock_update_worker,
        mock_thread_cls,
    ):
        fake_thread = MagicMock()
        mock_thread_cls.return_value = fake_thread
        self.backend._toggle_pending_rom_id = "7"

        self.backend._on_toggle_collections_fetched(None)

        mock_create_worker.assert_called_once()
        create_args, create_kwargs = mock_create_worker.call_args
        self.assertEqual(create_args[1], 7)
        self.assertEqual(create_kwargs, {"parent": None})
        mock_update_worker.assert_not_called()
        self.assertTrue(self.backend._toggle_adding)
        fake_thread.start.assert_called_once()

    def test_toggle_favorite_emits_signal_on_update_success(self):
        self.backend._toggle_pending_rom_id = "7"
        self.backend._toggle_adding = True
        emitted = []
        self.backend.favoriteToggleComplete.connect(lambda payload: emitted.append(payload))

        with patch.object(self.backend, "_start_favorites_fetch"):
            self.backend._on_toggle_collection_updated({})

        self.assertEqual(emitted, [{"rom_id": "7", "is_now_favorite": True}])

    def test_toggle_favorite_emits_signal_on_create_success(self):
        self.backend._toggle_pending_rom_id = "7"
        self.backend._toggle_adding = True
        emitted = []
        self.backend.favoriteToggleComplete.connect(lambda payload: emitted.append(payload))

        with patch.object(self.backend, "_start_favorites_fetch"):
            self.backend._on_toggle_collection_created({})

        self.assertEqual(emitted, [{"rom_id": "7", "is_now_favorite": True}])

    def test_toggle_favorite_triggers_favorites_refresh_on_update(self):
        self.backend._toggle_pending_rom_id = "7"
        self.backend._toggle_adding = True

        with patch.object(self.backend, "_start_favorites_fetch") as mock_refresh:
            self.backend._on_toggle_collection_updated({})

        mock_refresh.assert_called_once()


class TestAppBackendAvailableEmulatorNames(unittest.TestCase):
    def _make_backend(self, config):
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_returns_sorted_names_excluding_already_excluded(self):
        config = {
            "emulators": [
                {"name": "Dolphin"},
                {"name": "RPCS3"},
                {"name": "Cemu"},
            ],
            "tv_guide_button_exclusion_list": ["RPCS3"],
        }
        backend = self._make_backend(config)
        self.assertEqual(backend.availableEmulatorNames, ["Cemu", "Dolphin"])

    def test_empty_when_emulators_key_missing(self):
        backend = self._make_backend({})
        self.assertEqual(backend.availableEmulatorNames, [])

    def test_empty_when_emulators_is_empty_list(self):
        backend = self._make_backend({"emulators": []})
        self.assertEqual(backend.availableEmulatorNames, [])

    def test_case_insensitive_exclusion_filter(self):
        config = {
            "emulators": [{"name": "RPCS3"}],
            "tv_guide_button_exclusion_list": ["rpcs3"],
        }
        backend = self._make_backend(config)
        self.assertEqual(backend.availableEmulatorNames, [])

    def test_no_exclusions_when_config_key_missing(self):
        config = {
            "emulators": [
                {"name": "RetroArch"},
                {"name": "PCSX2"},
                {"name": "RPCS3"},
            ],
        }
        backend = self._make_backend(config)
        self.assertEqual(backend.availableEmulatorNames, ["PCSX2", "RPCS3", "RetroArch"])

    def test_skips_invalid_emulator_entries(self):
        config = {
            "emulators": [
                {"name": "RetroArch"},
                "not_a_dict",
                {"no_name_key": True},
                {"name": "   "},   # blank name
            ],
            "tv_guide_button_exclusion_list": [],
        }
        backend = self._make_backend(config)
        self.assertEqual(backend.availableEmulatorNames, ["RetroArch"])


class TestAppBackendGetInstalledLocalPath(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_returns_local_path_when_game_is_installed(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "42", "local_path": "/games/mygame.zip"}
            ]
        })
        result = backend.getInstalledLocalPath("42")
        self.assertEqual(result, "/games/mygame.zip")

    def test_returns_empty_string_when_not_installed(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "7", "local_path": "/games/other.zip"}
            ]
        })
        result = backend.getInstalledLocalPath("42")
        self.assertEqual(result, "")

    def test_returns_empty_string_when_rom_id_is_empty(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "42", "local_path": "/games/mygame.zip"}
            ]
        })
        result = backend.getInstalledLocalPath("")
        self.assertEqual(result, "")

    def test_returns_empty_string_when_installed_games_empty(self):
        backend = self._make_backend({"installed_games": []})
        result = backend.getInstalledLocalPath("42")
        self.assertEqual(result, "")

    def test_returns_empty_string_when_local_path_missing(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "42"}
            ]
        })
        result = backend.getInstalledLocalPath("42")
        self.assertEqual(result, "")

    def test_returns_archive_path_when_no_local_path(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "42", "archive_path": "/games/mygame.zip"}
            ]
        })
        result = backend.getInstalledLocalPath("42")
        self.assertEqual(result, "/games/mygame.zip")

    def test_returns_extracted_path_when_no_local_path(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "42", "extracted_path": "/games/mygame/"}
            ]
        })
        result = backend.getInstalledLocalPath("42")
        self.assertEqual(result, "/games/mygame/")


class TestAppBackendEnrichWithLocalPaths(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_server_games_for_platform_enriches_installed_game(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "42", "local_path": "/games/mygame.zip"}
            ]
        })
        backend._server_games["PS2"] = [{"rom_id": "42", "title": "My Game"}]
        result = backend.serverGamesForPlatform("PS2")
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].get("local_path"), "/games/mygame.zip")

    def test_server_games_for_platform_leaves_uninstalled_game_unchanged(self):
        backend = self._make_backend({"installed_games": []})
        backend._server_games["PS2"] = [{"rom_id": "99", "title": "Other Game"}]
        result = backend.serverGamesForPlatform("PS2")
        self.assertEqual(len(result), 1)
        self.assertNotIn("local_path", result[0])

    def test_favorites_games_enriches_installed_game(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "7", "local_path": "/games/fav.zip"}
            ]
        })
        backend._favorites_games = [{"rom_id": "7", "title": "Fav Game"}]
        result = backend.favoritesGames
        self.assertEqual(result[0].get("local_path"), "/games/fav.zip")

    def test_new_additions_games_enriches_installed_game(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "5", "local_path": "/games/new.zip"}
            ]
        })
        backend._new_additions_games = [{"rom_id": "5", "title": "New Game"}]
        result = backend.newAdditionsGames
        self.assertEqual(result[0].get("local_path"), "/games/new.zip")

    def test_highly_rated_games_enriches_installed_game(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "3", "local_path": "/games/rated.zip"}
            ]
        })
        backend._highly_rated_games = [{"rom_id": "3", "title": "Rated Game"}]
        result = backend.highlyRatedGames
        self.assertEqual(result[0].get("local_path"), "/games/rated.zip")

    def test_enrichment_does_not_overwrite_existing_local_path(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "42", "local_path": "/games/new_path.zip"}
            ]
        })
        backend._server_games["PS2"] = [
            {"rom_id": "42", "title": "My Game", "local_path": "/games/original.zip"}
        ]
        result = backend.serverGamesForPlatform("PS2")
        self.assertEqual(result[0].get("local_path"), "/games/original.zip")

    def test_server_games_enriched_with_archive_path_for_desktop_install(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "42", "archive_path": "/games/mygame.zip"}
            ]
        })
        backend._server_games["PS2"] = [{"rom_id": "42", "title": "My Game"}]
        result = backend.serverGamesForPlatform("PS2")
        self.assertEqual(result[0].get("local_path"), "/games/mygame.zip")


class TestAppBackendInstallStateCrossNotification(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_highly_rated_games_de_enriches_after_install_state_change(self):
        backend = self._make_backend({})
        backend._highly_rated_games = [{"rom_id": "10", "title": "Game"}]
        backend._config["installed_games"] = [{"rom_id": "10", "local_path": "/games/game.iso"}]

        self.assertEqual(backend.highlyRatedGames[0]["local_path"], "/games/game.iso")

        backend._config["installed_games"] = []

        self.assertFalse(backend.highlyRatedGames[0].get("local_path"))

    def test_favorites_games_de_enriches_after_install_state_change(self):
        backend = self._make_backend({})
        backend._favorites_games = [{"rom_id": "10", "title": "Game"}]
        backend._config["installed_games"] = [{"rom_id": "10", "local_path": "/games/game.iso"}]

        self.assertEqual(backend.favoritesGames[0]["local_path"], "/games/game.iso")

        backend._config["installed_games"] = []

        self.assertFalse(backend.favoritesGames[0].get("local_path"))

    def test_new_additions_games_de_enriches_after_install_state_change(self):
        backend = self._make_backend({})
        backend._new_additions_games = [{"rom_id": "10", "title": "Game"}]
        backend._config["installed_games"] = [{"rom_id": "10", "local_path": "/games/game.iso"}]

        self.assertEqual(backend.newAdditionsGames[0]["local_path"], "/games/game.iso")

        backend._config["installed_games"] = []

        self.assertFalse(backend.newAdditionsGames[0].get("local_path"))


class TestAppBackendFetchRomMetadata(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_fetch_rom_metadata_skips_when_no_rom_id(self):
        backend = self._make_backend({"server_url": "http://server", "api_token": "tok"})
        backend.fetchRomMetadata(json.dumps({"title": "Game"}))
        self.assertFalse(backend._rom_meta_threads)

    def test_fetch_rom_metadata_skips_when_no_credentials(self):
        backend = self._make_backend({"server_url": ""})
        backend.fetchRomMetadata(json.dumps({"rom_id": "42", "title": "Game"}))
        self.assertFalse(backend._rom_meta_threads)

    def test_fetch_rom_metadata_skips_when_metadata_complete(self):
        backend = self._make_backend({"server_url": "http://server", "api_token": "tok"})
        game = {
            "rom_id": "42",
            "genres": "RPG",
            "description": "A great game.",
            "rating": "4.5",
            "filesize_bytes": "1024",
            "companies": "Nintendo",
            "first_release_date": "1996-02-21",
        }
        backend.fetchRomMetadata(json.dumps(game))
        self.assertFalse(backend._rom_meta_threads)

    def test_fetch_rom_metadata_runs_when_first_release_date_missing(self):
        import threading
        from unittest.mock import patch
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(
            {"server_url": "http://server", "api_token": "tok"},
            Path("/tmp/covers"),
        )
        # All original 5 fields are complete but first_release_date is absent
        game = {
            "rom_id": "55",
            "genres": "RPG",
            "description": "A great game.",
            "rating": "4.5",
            "filesize_bytes": "1024",
            "companies": "Nintendo",
        }
        with patch("grid_launcher.core.api.api_get_json", return_value={}), \
             patch("grid_launcher.server.metadata.details_metadata_from_item", return_value={}):
            backend.fetchRomMetadata(json.dumps(game))

        t = backend._rom_meta_threads.get("55")
        self.assertIsNotNone(t)
        self.assertIsInstance(t, threading.Thread)
        if t is not None:
            t.join(timeout=2.0)

    def test_fetch_rom_metadata_emits_romMetadataReady(self):
        from unittest.mock import patch
        from grid_launcher.tv.bridge.app_backend import AppBackend, _RomMetaFetchWorker

        backend = AppBackend(
            {"server_url": "http://server", "api_token": "tok"},
            Path("/tmp/covers"),
        )

        fake_api_payload = {"igdb_metadata": {"genres": ["RPG"]}}
        with patch("grid_launcher.core.api.api_get_json", return_value=fake_api_payload), \
             patch("grid_launcher.server.metadata.details_metadata_from_item", return_value={"genres": "RPG", "description": ""}):
            worker = _RomMetaFetchWorker("http://server", "tok", "42")
            worker_results = []
            worker.finished.connect(lambda payload: worker_results.append(payload))
            worker.run()

        self.assertEqual(len(worker_results), 1)
        self.assertEqual(worker_results[0]["rom_id"], "42")
        self.assertIn("genres", worker_results[0]["metadata"])
        self.assertEqual(worker_results[0]["metadata"]["genres"], "RPG")
        self.assertNotIn("description", worker_results[0]["metadata"])

        emitted = []
        backend.romMetadataReady.connect(lambda payload: emitted.append(payload))
        backend._on_rom_meta_finished({"rom_id": "42", "metadata": {"genres": "RPG"}})
        self.assertEqual(len(emitted), 1)
        self.assertEqual(emitted[0]["rom_id"], "42")
        self.assertEqual(json.loads(emitted[0]["metadata_json"])["genres"], "RPG")

    def test_fetch_rom_metadata_stores_threading_thread(self):
        import threading
        from unittest.mock import patch
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(
            {"server_url": "http://server", "api_token": "tok"},
            Path("/tmp/covers"),
        )
        game = {"rom_id": "99", "genres": "", "description": "", "rating": "", "filesize_bytes": "", "companies": ""}
        with patch("grid_launcher.core.api.api_get_json", return_value={}), \
             patch("grid_launcher.server.metadata.details_metadata_from_item", return_value={}):
            backend.fetchRomMetadata(json.dumps(game))

        t = backend._rom_meta_threads.get("99")
        self.assertIsNotNone(t)
        self.assertIsInstance(t, threading.Thread)
        if t is not None:
            t.join(timeout=2.0)

    def test_fetch_rom_metadata_dedup_skips_when_alive(self):
        import threading
        from unittest.mock import MagicMock
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(
            {"server_url": "http://server", "api_token": "tok"},
            Path("/tmp/covers"),
        )
        game = {"rom_id": "77", "genres": "", "description": "", "rating": "", "filesize_bytes": "", "companies": ""}

        # Simulate an already-alive thread for this rom_id
        fake_thread = MagicMock(spec=threading.Thread)
        fake_thread.is_alive.return_value = True
        backend._rom_meta_threads["77"] = fake_thread

        with patch("grid_launcher.tv.bridge.app_backend.threading") as mock_threading:
            backend.fetchRomMetadata(json.dumps(game))
            mock_threading.Thread.assert_not_called()

    def test_rom_metadata_fetch_started_signal_emitted(self):
        import threading
        from unittest.mock import patch
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(
            {"server_url": "http://server", "api_token": "tok"},
            Path("/tmp/covers"),
        )
        # first_release_date is missing so the fetch should proceed
        game = {
            "rom_id": "88",
            "genres": "RPG",
            "description": "A great game.",
            "rating": "4.5",
            "filesize_bytes": "1024",
            "companies": "Nintendo",
        }
        emitted_ids: list[str] = []
        backend.romMetadataFetchStarted.connect(lambda rid: emitted_ids.append(rid))

        with patch("grid_launcher.core.api.api_get_json", return_value={}), \
             patch("grid_launcher.server.metadata.details_metadata_from_item", return_value={}):
            backend.fetchRomMetadata(json.dumps(game))

        t = backend._rom_meta_threads.get("88")
        if t is not None:
            t.join(timeout=2.0)

        self.assertEqual(emitted_ids, ["88"])

    def test_on_rom_meta_finished_removes_worker_and_calls_delete_later(self):
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(
            {"server_url": "http://server", "api_token": "tok"},
            Path("/tmp/covers"),
        )
        mock_worker = MagicMock()
        backend._rom_meta_workers["42"] = mock_worker
        backend._on_rom_meta_finished({"rom_id": "42", "metadata": {}})
        self.assertNotIn("42", backend._rom_meta_workers)
        mock_worker.deleteLater.assert_called_once()


class TestAppBackendStartRomFetch(unittest.TestCase):
    def _make_backend(self, config: dict = None) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config or {}, Path("/tmp/covers"))

    def test_start_rom_fetch_stores_threading_thread(self):
        import threading
        from unittest.mock import MagicMock, patch
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(
            {"server_url": "http://server", "installed_games": []},
            Path("/tmp/covers"),
        )
        backend._platforms = {"PS2": 2}

        fake_thread = MagicMock(spec=threading.Thread)
        fake_thread.is_alive.return_value = False

        with patch("grid_launcher.tv.bridge.app_backend.threading") as mock_threading, \
             patch("grid_launcher.tv.bridge.workers.RomListFetchWorker") as _mock_worker_cls:
            mock_threading.Thread.return_value = fake_thread
            backend._start_rom_fetch("PS2", 2)

        self.assertIn("PS2", backend._rom_threads)
        self.assertIs(backend._rom_threads["PS2"], fake_thread)
        fake_thread.start.assert_called_once()

    def test_start_rom_fetch_skips_when_thread_alive(self):
        import threading
        from unittest.mock import MagicMock, patch
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend(
            {"server_url": "http://server", "installed_games": []},
            Path("/tmp/covers"),
        )
        fake_thread = MagicMock(spec=threading.Thread)
        fake_thread.is_alive.return_value = True
        backend._rom_threads["PS2"] = fake_thread

        with patch("grid_launcher.tv.bridge.app_backend.threading") as mock_threading:
            backend._start_rom_fetch("PS2", 2)
            mock_threading.Thread.assert_not_called()

    def test_on_rom_fetch_thread_done_removes_finished_entries(self):
        import threading
        from unittest.mock import MagicMock
        from grid_launcher.tv.bridge.app_backend import AppBackend

        backend = AppBackend({}, Path("/tmp/covers"))

        alive_thread = MagicMock(spec=threading.Thread)
        alive_thread.is_alive.return_value = True
        dead_thread = MagicMock(spec=threading.Thread)
        dead_thread.is_alive.return_value = False

        backend._rom_threads["SNES"] = alive_thread
        backend._rom_threads["GBA"] = dead_thread
        backend._rom_workers["SNES"] = object()
        backend._rom_workers["GBA"] = object()

        backend._on_rom_fetch_thread_done()

        self.assertIn("SNES", backend._rom_threads)
        self.assertNotIn("GBA", backend._rom_threads)
        self.assertNotIn("GBA", backend._rom_workers)


class TestAppBackendLibraryGamesServerMetadataMerge(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_library_games_merges_genres_and_rating_from_server(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "10", "title": "Sonic", "platform": "Genesis", "local_path": "/games/sonic.zip"}
            ]
        })
        backend._server_games["Genesis"] = [
            {"rom_id": "10", "title": "Sonic", "genres": "Platformer", "rating": "4.8"}
        ]
        result = backend.libraryGames
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["genres"], "Platformer")
        self.assertEqual(result[0]["rating"], "4.8")

    def test_library_games_does_not_overwrite_existing_genres(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "10", "title": "Sonic", "platform": "Genesis",
                 "local_path": "/games/sonic.zip", "genres": "Action"}
            ]
        })
        backend._server_games["Genesis"] = [
            {"rom_id": "10", "title": "Sonic", "genres": "Platformer", "rating": "4.8"}
        ]
        result = backend.libraryGames
        self.assertEqual(result[0]["genres"], "Action")
        self.assertEqual(result[0]["rating"], "4.8")

    def test_library_games_merges_all_meta_fields_from_server(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "20", "title": "Mario", "platform": "SNES", "local_path": "/games/mario.zip"}
            ]
        })
        backend._server_games["SNES"] = [
            {
                "rom_id": "20",
                "first_release_date": "1990-11-21",
                "release_year": "1990",
                "companies": "Nintendo",
                "languages": "English",
                "revision": "1.0",
                "fanart_url": "http://example.com/fanart.jpg",
                "genres": "Platformer",
                "rating": "5.0",
            }
        ]
        result = backend.libraryGames
        game = result[0]
        self.assertEqual(game["first_release_date"], "1990-11-21")
        self.assertEqual(game["release_year"], "1990")
        self.assertEqual(game["companies"], "Nintendo")
        self.assertEqual(game["languages"], "English")
        self.assertEqual(game["revision"], "1.0")
        self.assertEqual(game["fanart_url"], "http://example.com/fanart.jpg")
        self.assertEqual(game["genres"], "Platformer")
        self.assertEqual(game["rating"], "5.0")

    def test_library_games_no_merge_when_no_server_data(self):
        backend = self._make_backend({
            "installed_games": [
                {"rom_id": "30", "title": "Zelda", "platform": "NES", "local_path": "/games/zelda.zip"}
            ]
        })
        result = backend.libraryGames
        self.assertNotIn("genres", result[0])
        self.assertNotIn("rating", result[0])


class TestAppBackendRomMetaFinishedUpdatesLibrary(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from grid_launcher.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_rom_meta_finished_updates_installed_game(self):
        game = {"rom_id": "42", "title": "Bomb Rush Cyberfunk", "platform": "PC"}
        backend = self._make_backend({"installed_games": [game]})

        backend._on_rom_meta_finished({"rom_id": "42", "metadata": {"companies": "Team Reptile", "first_release_date": "2023-08-18"}})

        self.assertEqual(game["companies"], "Team Reptile")
        self.assertEqual(game["first_release_date"], "2023-08-18")

    def test_rom_meta_finished_emits_library_games_changed(self):
        game = {"rom_id": "42", "title": "Bomb Rush Cyberfunk", "platform": "PC"}
        backend = self._make_backend({"installed_games": [game]})

        emitted = []
        backend.libraryGamesChanged.connect(lambda: emitted.append(True))
        backend._on_rom_meta_finished({"rom_id": "42", "metadata": {"companies": "Team Reptile"}})
        backend._flush_lib_changed()

        self.assertEqual(len(emitted), 1)

    def test_rom_meta_finished_does_not_overwrite_existing_value(self):
        game = {"rom_id": "42", "title": "Bomb Rush Cyberfunk", "platform": "PC", "genres": "Action"}
        backend = self._make_backend({"installed_games": [game]})

        backend._on_rom_meta_finished({"rom_id": "42", "metadata": {"genres": "RPG"}})

        self.assertEqual(game["genres"], "Action")

    def test_rom_meta_finished_no_emit_when_no_match(self):
        game = {"rom_id": "99", "title": "Some Other Game", "platform": "PC"}
        backend = self._make_backend({"installed_games": [game]})

        emitted = []
        backend.libraryGamesChanged.connect(lambda: emitted.append(True))
        backend._on_rom_meta_finished({"rom_id": "42", "metadata": {"companies": "Team Reptile"}})

        self.assertEqual(len(emitted), 0)

    def test_rom_meta_finished_saves_config_when_changed(self):
        game = {"rom_id": "42", "title": "Bomb Rush Cyberfunk", "platform": "PC"}
        backend = self._make_backend({"installed_games": [game]})

        emitted = []
        backend.saveConfigRequested.connect(lambda: emitted.append(True))
        backend._on_rom_meta_finished({"rom_id": "42", "metadata": {"companies": "Team Reptile"}})
        self.assertEqual(len(emitted), 1)

    def test_rom_meta_finished_does_not_save_config_when_nothing_changed(self):
        game = {
            "rom_id": "42",
            "title": "Bomb Rush Cyberfunk",
            "platform": "PC",
            "genres": "Action",
            "companies": "Team Reptile",
            "first_release_date": "2023-08-18",
            "description": "A game",
            "rating": "9",
            "filesize_bytes": "1234",
        }
        backend = self._make_backend({"installed_games": [game]})

        emitted = []
        backend.saveConfigRequested.connect(lambda: emitted.append(True))
        backend._on_rom_meta_finished({"rom_id": "42", "metadata": {
            "genres": "Action",
            "companies": "Team Reptile",
            "first_release_date": "2023-08-18",
        }})
        self.assertEqual(len(emitted), 0)


if __name__ == "__main__":
    unittest.main()
