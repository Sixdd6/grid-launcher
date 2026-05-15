from __future__ import annotations

import os
import unittest
from unittest.mock import MagicMock

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtGui import QPixmap
from PySide6.QtWidgets import QApplication

from rom_mate.tv.widgets.views.details_view import DetailsView


class _StubCoverLoader:
    def __init__(self) -> None:
        self.calls: list[tuple[str, object]] = []

    def load_async(self, url: str, callback) -> None:
        self.calls.append((url, callback))


def _signal_mock() -> MagicMock:
    signal = MagicMock()
    signal.connect = MagicMock()
    return signal


class TvDetailsViewScreenshotTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def _make_backends(self) -> tuple[MagicMock, MagicMock, MagicMock, MagicMock, MagicMock]:
        app_backend = MagicMock()
        app_backend.libraryGames = []
        app_backend.isConnected = False
        app_backend.romMetadataFetchStarted = _signal_mock()
        app_backend.romMetadataReady = _signal_mock()
        app_backend.favoriteToggleComplete = _signal_mock()
        app_backend.logHandleDiag = MagicMock()
        app_backend.fetchRomMetadata = MagicMock()
        app_backend.toggleFavorite = MagicMock()
        app_backend.setUiOverlayActive = MagicMock()

        cloud_backend = MagicMock()
        cloud_backend.slotsLoaded = _signal_mock()
        cloud_backend.slotsError = _signal_mock()
        cloud_backend.restoreComplete = _signal_mock()
        cloud_backend.deleteComplete = _signal_mock()
        cloud_backend.uploadComplete = _signal_mock()

        game_backend = MagicMock()
        game_backend.installProgress = _signal_mock()
        game_backend.installComplete = _signal_mock()
        game_backend.uninstallComplete = _signal_mock()
        game_backend.launchError = _signal_mock()
        game_backend.sessionStarted = _signal_mock()
        game_backend.sessionEnded = _signal_mock()
        game_backend.nativeExecPickerNeeded = _signal_mock()
        game_backend.isInstallActive = False

        pause_backend = MagicMock()
        controller_backend = MagicMock()
        return app_backend, cloud_backend, game_backend, pause_backend, controller_backend

    def test_refresh_screenshots_sets_card_height_from_pixmap_aspect_ratio(self) -> None:
        app_backend, cloud_backend, game_backend, pause_backend, controller_backend = self._make_backends()
        cover_loader = _StubCoverLoader()

        view = DetailsView(
            {
                "title": "Metroid Prime",
                "screenshot_urls": ["shot://1"],
            },
            app_backend,
            cloud_backend,
            game_backend,
            pause_backend,
            controller_backend,
            cover_loader,
            lambda: None,
        )
        self.addCleanup(view.deleteLater)

        view.resize(1600, 900)
        view.show()
        self.app.processEvents()

        view._refresh_screenshots()

        shot_callbacks = [callback for url, callback in cover_loader.calls if url == "shot://1"]
        self.assertTrue(shot_callbacks)

        card = view._shot_cards[0]
        card.setFixedWidth(500)
        self.app.processEvents()
        card_width = 500

        pixmap = QPixmap(1920, 1080)
        shot_callbacks[-1](pixmap)
        self.app.processEvents()

        expected_height = int(card_width * 1080 / 1920)
        actual_height = card.height()
        self.assertLessEqual(abs(actual_height - expected_height), 2)


if __name__ == "__main__":
    unittest.main()
