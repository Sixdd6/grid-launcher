from __future__ import annotations

import os
import unittest
from unittest.mock import MagicMock, patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication

from rom_mate.tv.widgets.pause_window import PauseWindow


class PauseWindowNavTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def _make_pause_backend(self) -> MagicMock:
        pause_backend = MagicMock()
        pause_backend.visible = False
        pause_backend.gameTitle = "Metroid Prime"
        pause_backend.emulatorName = "RetroArch"
        pause_backend.actions = ["Resume Game", "Quit to TV Mode"]
        pause_backend.visibleChanged = MagicMock()
        pause_backend.gameTitleChanged = MagicMock()
        pause_backend.emulatorNameChanged = MagicMock()
        pause_backend.resumeGame = MagicMock()
        pause_backend.quitGame = MagicMock()
        return pause_backend

    def test_initial_index_is_zero(self):
        pause_backend = self._make_pause_backend()

        window = PauseWindow(pause_backend, parent=None)

        self.assertEqual(window._current_index, 0)

    def test_handle_nav_down_increments_index(self):
        pause_backend = self._make_pause_backend()
        window = PauseWindow(pause_backend, parent=None)

        window.handle_nav("down")

        self.assertEqual(window._current_index, 1)

    def test_handle_nav_up_decrements_index(self):
        pause_backend = self._make_pause_backend()
        window = PauseWindow(pause_backend, parent=None)
        window._current_index = 1

        window.handle_nav("up")

        self.assertEqual(window._current_index, 0)

    def test_handle_nav_up_clamps_at_zero(self):
        pause_backend = self._make_pause_backend()
        window = PauseWindow(pause_backend, parent=None)

        window.handle_nav("up")

        self.assertEqual(window._current_index, 0)

    def test_handle_nav_down_clamps_at_last(self):
        pause_backend = self._make_pause_backend()
        window = PauseWindow(pause_backend, parent=None)
        window._current_index = 1

        window.handle_nav("down")

        self.assertEqual(window._current_index, 1)

    def test_handle_nav_confirm_at_index_0_calls_resume_game(self):
        pause_backend = self._make_pause_backend()
        window = PauseWindow(pause_backend, parent=None)
        window._current_index = 0

        with patch("rom_mate.tv.widgets.pause_window.QTimer.singleShot", side_effect=lambda _ms, callback: callback()):
            window.handle_nav("confirm")

        pause_backend.resumeGame.assert_called_once_with()
        pause_backend.quitGame.assert_not_called()

    def test_handle_nav_confirm_at_index_1_calls_quit_game(self):
        pause_backend = self._make_pause_backend()
        window = PauseWindow(pause_backend, parent=None)
        window._current_index = 1

        window.handle_nav("confirm")

        pause_backend.quitGame.assert_called_once_with()
        pause_backend.resumeGame.assert_not_called()

    def test_handle_nav_back_calls_resume_game(self):
        pause_backend = self._make_pause_backend()
        window = PauseWindow(pause_backend, parent=None)

        window.handle_nav("back")

        pause_backend.resumeGame.assert_called_once_with()
        pause_backend.quitGame.assert_not_called()

    def test_handle_nav_unknown_direction_is_noop(self):
        pause_backend = self._make_pause_backend()
        window = PauseWindow(pause_backend, parent=None)

        window.handle_nav("left")

        pause_backend.resumeGame.assert_not_called()
        pause_backend.quitGame.assert_not_called()
        self.assertEqual(window._current_index, 0)


if __name__ == "__main__":
    unittest.main()
