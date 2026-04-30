from __future__ import annotations

import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import sys
import unittest
from unittest.mock import MagicMock

from PySide6.QtCore import QCoreApplication

_app = QCoreApplication.instance() or QCoreApplication(sys.argv)


class PauseBackendTests(unittest.TestCase):
    def setUp(self):
        from rom_mate.tv.bridge.pause_backend import PauseBackend

        self.game_backend = MagicMock()
        self.game_backend.isSessionActive = False
        self.game_backend.activeGameTitle = ""
        self.game_backend.activeEmulatorName = ""
        self.backend = PauseBackend(self.game_backend)

    def test_visible_is_false_on_init(self):
        self.assertFalse(self.backend.visible)

    def test_game_title_empty_on_init(self):
        self.assertEqual(self.backend.gameTitle, "")

    def test_emulator_name_empty_on_init(self):
        self.assertEqual(self.backend.emulatorName, "")

    def test_actions_contains_resume_and_quit(self):
        self.assertIn("Resume Game", self.backend.actions)
        self.assertIn("Quit to TV Mode", self.backend.actions)

    def test_open_for_active_session_is_noop_when_inactive(self):
        self.game_backend.isSessionActive = False

        self.backend.openForActiveSession()

        self.assertFalse(self.backend.visible)
        self.game_backend.pauseEmulator.assert_not_called()

    def test_open_for_active_session_calls_pause_emulator(self):
        self.game_backend.isSessionActive = True

        self.backend.openForActiveSession()

        self.game_backend.pauseEmulator.assert_called_once_with()

    def test_open_for_active_session_sets_visible_true(self):
        self.game_backend.isSessionActive = True

        self.backend.openForActiveSession()

        self.assertTrue(self.backend.visible)

    def test_open_for_active_session_snapshots_game_title(self):
        self.game_backend.isSessionActive = True
        self.game_backend.activeGameTitle = "Metroid"

        self.backend.openForActiveSession()

        self.assertEqual(self.backend.gameTitle, "Metroid")

    def test_open_for_active_session_snapshots_emulator_name(self):
        self.game_backend.isSessionActive = True
        self.game_backend.activeEmulatorName = "RetroArch"

        self.backend.openForActiveSession()

        self.assertEqual(self.backend.emulatorName, "RetroArch")

    def test_open_for_active_session_emulator_name_fallback(self):
        self.game_backend.isSessionActive = True
        self.game_backend.activeEmulatorName = ""

        self.backend.openForActiveSession()

        self.assertEqual(self.backend.emulatorName, "Native Game")

    def test_resume_game_calls_resume_emulator(self):
        self.game_backend.isSessionActive = True
        self.backend.openForActiveSession()

        self.backend.resumeGame()

        self.game_backend.resumeEmulator.assert_called_once_with()

    def test_resume_game_sets_visible_false(self):
        self.game_backend.isSessionActive = True
        self.backend.openForActiveSession()

        self.backend.resumeGame()

        self.assertFalse(self.backend.visible)

    def test_resume_game_is_noop_when_not_visible(self):
        self.backend.resumeGame()

        self.game_backend.resumeEmulator.assert_not_called()

    def test_quit_game_calls_stop_game(self):
        self.game_backend.isSessionActive = True
        self.backend.openForActiveSession()

        self.backend.quitGame()

        self.game_backend.stopGame.assert_called_once_with()

    def test_quit_game_sets_visible_false(self):
        self.game_backend.isSessionActive = True
        self.backend.openForActiveSession()

        self.backend.quitGame()

        self.assertFalse(self.backend.visible)

    def test_dismiss_calls_resume_emulator(self):
        self.game_backend.isSessionActive = True
        self.backend.openForActiveSession()

        self.backend.dismiss()

        self.game_backend.resumeEmulator.assert_called_once_with()

    def test_dismiss_sets_visible_false(self):
        self.game_backend.isSessionActive = True
        self.backend.openForActiveSession()

        self.backend.dismiss()

        self.assertFalse(self.backend.visible)

    def test_dismiss_is_noop_when_not_visible(self):
        self.backend.dismiss()

        self.game_backend.resumeEmulator.assert_not_called()


if __name__ == "__main__":
    unittest.main()