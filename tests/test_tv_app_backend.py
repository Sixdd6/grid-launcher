from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from PySide6.QtCore import QCoreApplication

_app = QCoreApplication.instance() or QCoreApplication(sys.argv)


class TestAppBackendConfigDefaults(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from rom_mate.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_tv_guide_exclusion_list_default_when_key_missing(self):
        backend = self._make_backend({})
        result = backend.tvGuideExclusionList
        self.assertEqual(result, ["RPCS3", "Cemu", "Dolphin", "Xemu", "Xenia"])

    def test_tv_guide_exclusion_list_from_config(self):
        backend = self._make_backend({"tv_guide_button_exclusion_list": ["RPCS3", "Cemu"]})
        self.assertEqual(backend.tvGuideExclusionList, ["RPCS3", "Cemu"])

    def test_tv_guide_exclusion_list_invalid_type_returns_default(self):
        backend = self._make_backend({"tv_guide_button_exclusion_list": "not-a-list"})
        self.assertEqual(backend.tvGuideExclusionList, ["RPCS3", "Cemu", "Dolphin", "Xemu", "Xenia"])

    def test_home_view_default(self):
        backend = self._make_backend({})
        self.assertEqual(backend.homeView, "home")

    def test_home_view_from_config(self):
        backend = self._make_backend({"tv_mode_home_view": "library"})
        self.assertEqual(backend.homeView, "library")

    def test_home_view_invalid_type_returns_default(self):
        backend = self._make_backend({"tv_mode_home_view": 42})
        self.assertEqual(backend.homeView, "home")

    def test_library_games_empty_when_no_installed_games(self):
        backend = self._make_backend({})
        self.assertEqual(backend.libraryGames, [])

    def test_library_games_returns_installed_games(self):
        games = [{"title": "Doom", "platform": "PC"}]
        backend = self._make_backend({"installed_games": games})
        self.assertEqual(backend.libraryGames, games)

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
        from rom_mate.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    @patch("rom_mate.tv.bridge.app_backend._write_config_file")
    def test_set_guide_exclusion_list_updates_config(self, mock_write):
        backend = self._make_backend({})
        backend.setGuideExclusionList(["RPCS3", "Dolphin"])
        self.assertEqual(backend._config["tv_guide_button_exclusion_list"], ["RPCS3", "Dolphin"])

    @patch("rom_mate.tv.bridge.app_backend._write_config_file")
    def test_set_guide_exclusion_list_persists(self, mock_write):
        backend = self._make_backend({})
        backend.setGuideExclusionList(["RPCS3"])
        self.assertTrue(mock_write.called)

    @patch("rom_mate.tv.bridge.app_backend._write_config_file")
    def test_set_home_view_valid(self, mock_write):
        backend = self._make_backend({})
        backend.setHomeView("server")
        self.assertEqual(backend._config["tv_mode_home_view"], "server")

    @patch("rom_mate.tv.bridge.app_backend._write_config_file")
    def test_set_home_view_invalid_ignored(self, mock_write):
        backend = self._make_backend({"tv_mode_home_view": "home"})
        backend.setHomeView("invalid_value")
        self.assertEqual(backend._config.get("tv_mode_home_view"), "home")
        mock_write.assert_not_called()


class TestAppBackendSyncConfig(unittest.TestCase):
    def test_sync_config_updates_library_games(self):
        from rom_mate.tv.bridge.app_backend import AppBackend
        backend = AppBackend({}, Path("/tmp/covers"))
        new_config = {"installed_games": [{"title": "Zelda", "platform": "N64"}]}
        backend.syncConfig(new_config)
        self.assertEqual(backend.libraryGames, new_config["installed_games"])


class TestAppBackendConnectToServer(unittest.TestCase):
    def _make_backend(self, config: dict) -> "AppBackend":
        from rom_mate.tv.bridge.app_backend import AppBackend
        return AppBackend(config, Path("/tmp/covers"))

    def test_connect_to_server_emits_status_when_no_credentials(self):
        backend = self._make_backend({"server_url": "", "api_token": ""})
        emitted = []
        backend.connectionStatusChanged.connect(lambda s: emitted.append(s))
        backend.connectToServer()
        self.assertEqual(len(emitted), 1)
        self.assertIn("No credentials", emitted[0])


class TestAppBackendSettings(unittest.TestCase):
    def setUp(self):
        from rom_mate.tv.bridge.app_backend import AppBackend

        self._write_patcher = patch("rom_mate.tv.bridge.app_backend._write_config_file")
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

        self.assertTrue(self.backend._config["auto_cloud_sync"])
        self.assertEqual(emitted[-1], True)

    def test_server_url_property(self):
        self.backend.syncConfig({"server_url": "http://myserver"})
        self.assertEqual(self.backend.serverUrl, "http://myserver")

    def test_is_auto_sync_defaults_false(self):
        self.assertFalse(self.backend.isAutoSync)

    def test_set_auto_sync_false(self):
        emitted = []
        self.backend.autoSyncChanged.connect(lambda value: emitted.append(value))

        self.backend.setAutoSync(False)

        self.assertFalse(self.backend._config["auto_cloud_sync"])
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
        from rom_mate.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        self.assertFalse(backend.uiOverlayActive)

    def test_set_ui_overlay_active_true(self):
        from rom_mate.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        backend.setUiOverlayActive(True)
        self.assertTrue(backend.uiOverlayActive)

    def test_set_ui_overlay_active_false(self):
        from rom_mate.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        backend.setUiOverlayActive(True)
        backend.setUiOverlayActive(False)
        self.assertFalse(backend.uiOverlayActive)

    def test_set_ui_overlay_active_emits_signal_on_change(self):
        from rom_mate.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        fired = []
        backend.overlayStateChanged.connect(lambda: fired.append(1))
        backend.setUiOverlayActive(True)
        self.assertEqual(len(fired), 1)

    def test_set_ui_overlay_active_no_emit_if_unchanged(self):
        from rom_mate.tv.bridge.app_backend import AppBackend

        backend = AppBackend(config={}, image_cache_dir=Path("/tmp"))
        fired = []
        backend.overlayStateChanged.connect(lambda: fired.append(1))
        backend.setUiOverlayActive(False)
        self.assertEqual(len(fired), 0)


class TestAppBackendCuratedRows(unittest.TestCase):
    def _make_backend(self, config: dict = None) -> "AppBackend":
        from rom_mate.tv.bridge.app_backend import AppBackend
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
        self.assertEqual(backend.favoritesGames, [{"title": "Tetris"}])

    def test_on_new_additions_finished_updates_list(self):
        backend = self._make_backend()
        backend._on_new_additions_finished([{"title": "Doom"}])
        self.assertEqual(backend.newAdditionsGames, [{"title": "Doom"}])

    def test_on_highly_rated_finished_updates_list(self):
        backend = self._make_backend()
        backend._on_highly_rated_finished([{"title": "Half-Life"}])
        self.assertEqual(backend.highlyRatedGames, [{"title": "Half-Life"}])

    def test_catalog_finish_triggers_curated_fetch(self):
        from unittest.mock import patch, MagicMock
        from rom_mate.tv.bridge.app_backend import AppBackend

        backend = AppBackend({}, Path("/tmp/covers"))
        with patch("rom_mate.server.catalog.server_platform_ids", return_value={}), \
             patch("rom_mate.server.state.account_status_text", return_value="Connected"), \
             patch.object(backend, "_start_curated_rows_fetch") as mock_curated:
            backend._on_catalog_finished({}, {})
            mock_curated.assert_called_once()


if __name__ == "__main__":
    unittest.main()
