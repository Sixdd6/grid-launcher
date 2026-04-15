from __future__ import annotations

import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import sys
import unittest
from unittest.mock import MagicMock, patch

from PySide6.QtCore import QCoreApplication


class TestGameBackend(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def setUp(self):
        from rom_mate.emulator import retroarch as retroarch_module
        from rom_mate.emulator import rpcs3 as rpcs3_module

        if not hasattr(retroarch_module, "is_retroarch_emulator_name"):
            retroarch_module.is_retroarch_emulator_name = (  # type: ignore[attr-defined]
                lambda name: "retroarch" in str(name).strip().casefold()
            )
        if not hasattr(rpcs3_module, "is_rpcs3_emulator_name"):
            rpcs3_module.is_rpcs3_emulator_name = (  # type: ignore[attr-defined]
                lambda name: "rpcs3" in str(name).strip().casefold()
            )

        from rom_mate.tv.bridge.game_backend import GameBackend

        self.config = {
            "emulators": [{"name": "RetroArch", "path": "/usr/bin/retroarch", "args": "%rom%"}],
            "default_emulators": {"SNES": "RetroArch"},
            "launch_args": "",
            "installed_games": [],
        }
        self.game = {
            "name": "Super Game",
            "platform": "SNES",
            "local_path": "/games/super_game.sfc",
        }
        self.backend = GameBackend(self.config)

    def test_active_emulator_name_is_empty_on_init(self):
        self.assertEqual(self.backend.activeEmulatorName, "")

    def test_is_session_active_is_false_on_init(self):
        self.assertFalse(self.backend.isSessionActive)

    def test_launch_game_success_emits_session_started(self):
        received: list[str] = []
        self.backend.sessionStarted.connect(lambda name: received.append(name))

        mock_process = MagicMock()
        mock_process.poll.return_value = None

        with patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            return_value=("RetroArch", ["retroarch", "/games/super_game.sfc"], "/tmp"),
        ), patch("rom_mate.tv.bridge.game_backend._subprocess_popen", return_value=mock_process), patch(
            "rom_mate.tv.bridge.game_backend._ProcessWatchThread.start"
        ):
            self.backend.launchGame(self.game)

        self.assertEqual(received, ["RetroArch"])

    def test_launch_game_success_sets_active_emulator_name(self):
        mock_process = MagicMock()
        mock_process.poll.return_value = None

        with patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            return_value=("RetroArch", ["retroarch", "/games/super_game.sfc"], "/tmp"),
        ), patch("rom_mate.tv.bridge.game_backend._subprocess_popen", return_value=mock_process), patch(
            "rom_mate.tv.bridge.game_backend._ProcessWatchThread.start"
        ):
            self.backend.launchGame(self.game)

        self.assertEqual(self.backend.activeEmulatorName, "RetroArch")

    def test_launch_game_success_sets_session_active_true(self):
        mock_process = MagicMock()
        mock_process.poll.return_value = None

        with patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            return_value=("RetroArch", ["retroarch", "/games/super_game.sfc"], "/tmp"),
        ), patch("rom_mate.tv.bridge.game_backend._subprocess_popen", return_value=mock_process), patch(
            "rom_mate.tv.bridge.game_backend._ProcessWatchThread.start"
        ):
            self.backend.launchGame(self.game)

        self.assertTrue(self.backend.isSessionActive)

    def test_launch_game_error_emits_launch_error_message(self):
        received: list[str] = []
        self.backend.launchError.connect(lambda message: received.append(message))

        with patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            side_effect=ValueError("no emulator"),
        ):
            self.backend.launchGame(self.game)

        self.assertEqual(received, ["no emulator"])

    def test_launch_game_error_keeps_session_inactive(self):
        with patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            side_effect=ValueError("no emulator"),
        ):
            self.backend.launchGame(self.game)

        self.assertFalse(self.backend.isSessionActive)

    def test_stop_game_terminates_process_and_emits_session_ended(self):
        received: list[str] = []
        self.backend.sessionEnded.connect(lambda name: received.append(name))

        mock_process = MagicMock()
        mock_process.poll.return_value = None
        mock_process.wait.side_effect = RuntimeError("watch thread should not clear process in this test")

        with patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            return_value=("RetroArch", ["retroarch", "/games/super_game.sfc"], "/tmp"),
        ), patch("rom_mate.tv.bridge.game_backend._subprocess_popen", return_value=mock_process):
            self.backend.launchGame(self.game)
            self.backend.stopGame()

        mock_process.terminate.assert_called_once_with()
        self.assertEqual(received, [""])

    def test_stop_game_sets_session_inactive(self):
        mock_process = MagicMock()
        mock_process.poll.return_value = None
        mock_process.wait.side_effect = RuntimeError("watch thread should not clear process in this test")

        with patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            return_value=("RetroArch", ["retroarch", "/games/super_game.sfc"], "/tmp"),
        ), patch("rom_mate.tv.bridge.game_backend._subprocess_popen", return_value=mock_process):
            self.backend.launchGame(self.game)
            self.backend.stopGame()

        self.assertFalse(self.backend.isSessionActive)

    def test_sync_config_updates_backend_config(self):
        new_config = {
            "emulators": [{"name": "DuckStation", "path": "/usr/bin/duckstation", "args": "%rom%"}],
            "default_emulators": {"PSX": "DuckStation"},
            "launch_args": "",
            "installed_games": [{"name": "Crash"}],
        }

        self.backend.syncConfig(new_config)

        self.assertEqual(self.backend._config, new_config)


class TestGameBackendPause(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def setUp(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        self.backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
            }
        )

    def test_pause_emulator_calls_suspend(self):
        from rom_mate.tv.bridge import game_backend as game_backend_module

        emitted: list[bool] = []
        self.backend.sessionPaused.connect(lambda: emitted.append(True))

        mock_process = MagicMock()
        mock_process.pid = 1234
        mock_process.poll.return_value = None
        self.backend._process = mock_process

        mock_psutil = MagicMock()
        mock_psutil.Process.return_value.suspend = MagicMock()

        with patch.object(game_backend_module, "_psutil", mock_psutil):
            self.backend.pauseEmulator()

        mock_psutil.Process.assert_called_once_with(1234)
        mock_psutil.Process.return_value.suspend.assert_called_once_with()
        self.assertEqual(emitted, [True])

    def test_pause_emulator_no_op_when_no_process(self):
        emitted: list[bool] = []
        self.backend.sessionPaused.connect(lambda: emitted.append(True))

        self.backend.pauseEmulator()

        self.assertEqual(emitted, [])

    def test_resume_emulator_calls_resume(self):
        from rom_mate.tv.bridge import game_backend as game_backend_module

        emitted: list[bool] = []
        self.backend.sessionResumed.connect(lambda: emitted.append(True))

        mock_process = MagicMock()
        mock_process.pid = 1234
        mock_process.poll.return_value = None
        self.backend._process = mock_process

        mock_psutil = MagicMock()
        mock_psutil.Process.return_value.resume = MagicMock()

        with patch.object(game_backend_module, "_psutil", mock_psutil):
            self.backend.resumeEmulator()

        mock_psutil.Process.assert_called_once_with(1234)
        mock_psutil.Process.return_value.resume.assert_called_once_with()
        self.assertEqual(emitted, [True])

    def test_request_pause_emits_when_active(self):
        emitted: list[bool] = []
        self.backend.pauseRequested.connect(lambda: emitted.append(True))

        self.backend._active_emulator_name = "RetroArch"
        mock_process = MagicMock()
        mock_process.poll.return_value = None
        self.backend._process = mock_process

        self.backend.requestPause()

        self.assertEqual(emitted, [True])

    def test_request_pause_does_not_emit_when_inactive(self):
        emitted: list[bool] = []
        self.backend.pauseRequested.connect(lambda: emitted.append(True))

        self.backend.requestPause()

        self.assertEqual(emitted, [])


class TestGameBackendSync(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def test_process_watch_thread_clears_state_on_exit(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend({"emulators": [], "default_emulators": {}, "launch_args": ""})
        backend._active_emulator_name = "RetroArch"
        backend._process = MagicMock()
        backend._process.wait.return_value = 0

        emitted: list[str] = []
        backend.sessionEnded.connect(lambda name: emitted.append(name))

        backend._on_process_exited("RetroArch")

        self.assertEqual(backend._active_emulator_name, "")
        self.assertIsNone(backend._process)
        self.assertEqual(emitted, ["RetroArch"])


if __name__ == "__main__":
    unittest.main()
