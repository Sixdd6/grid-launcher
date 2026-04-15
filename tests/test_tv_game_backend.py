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
        ), patch("rom_mate.tv.bridge.game_backend._subprocess_popen", return_value=mock_process), patch(
            "rom_mate.tv.bridge.game_backend._ProcessWatchThread.start"
        ):
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
        ), patch("rom_mate.tv.bridge.game_backend._subprocess_popen", return_value=mock_process), patch(
            "rom_mate.tv.bridge.game_backend._ProcessWatchThread.start"
        ):
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


class TestGameBackendAutoCloudSync(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def setUp(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        self.game = {
            "id": "1",
            "rom_id": "1",
            "name": "Super Game",
            "title": "Super Game",
            "platform": "SNES",
            "local_path": "/games/super_game.sfc",
        }
        self.base_config = {
            "emulators": [{"name": "RetroArch", "path": "/usr/bin/retroarch", "args": "%rom%"}],
            "default_emulators": {"SNES": "RetroArch"},
            "launch_args": "",
            "installed_games": [],
        }

    def test_launch_game_starts_restore_worker_when_auto_sync_enabled(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend({**self.base_config, "auto_cloud_sync": True})
        statuses: list[str] = []
        backend.cloudSyncStatus.connect(statuses.append)

        with patch("rom_mate.tv.bridge.game_backend._credentials_present", return_value=True), patch(
            "rom_mate.tv.bridge.game_backend.resolve_emulator_entry_for_game",
            return_value=("RetroArch", {"name": "RetroArch"}),
        ), patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            return_value=("RetroArch", ["retroarch"], None),
        ), patch("rom_mate.tv.bridge.game_backend._TvAutoRestoreWorker") as mock_restore_worker, patch(
            "rom_mate.tv.bridge.game_backend.QThread.start"
        ):
            backend.launchGame(self.game)

        self.assertIn("Restoring save…", statuses)
        mock_restore_worker.assert_called_once()

    def test_launch_game_calls_do_launch_directly_when_auto_sync_disabled(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config))
        started: list[str] = []
        backend.sessionStarted.connect(started.append)

        mock_process = MagicMock()
        mock_process.poll.return_value = None

        with patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            return_value=("RetroArch", ["retroarch"], None),
        ), patch("rom_mate.tv.bridge.game_backend._TvAutoRestoreWorker") as mock_restore_worker, patch(
            "rom_mate.tv.bridge.game_backend._subprocess_popen",
            return_value=mock_process,
        ), patch("rom_mate.tv.bridge.game_backend._ProcessWatchThread.start"):
            backend.launchGame(self.game)

        self.assertEqual(started, ["RetroArch"])
        mock_restore_worker.assert_not_called()

    def test_launch_game_skips_restore_when_no_emulator_entry(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend({**self.base_config, "auto_cloud_sync": True})
        started: list[str] = []
        backend.sessionStarted.connect(started.append)

        mock_process = MagicMock()
        mock_process.poll.return_value = None

        with patch("rom_mate.tv.bridge.game_backend._credentials_present", return_value=True), patch(
            "rom_mate.tv.bridge.game_backend.resolve_emulator_entry_for_game",
            return_value=("", None),
        ), patch(
            "rom_mate.tv.bridge.game_backend.prepare_emulator_launch_command",
            return_value=("RetroArch", ["retroarch"], None),
        ), patch("rom_mate.tv.bridge.game_backend._TvAutoRestoreWorker") as mock_restore_worker, patch(
            "rom_mate.tv.bridge.game_backend._subprocess_popen",
            return_value=mock_process,
        ), patch("rom_mate.tv.bridge.game_backend._ProcessWatchThread.start"):
            backend.launchGame(self.game)

        mock_restore_worker.assert_not_called()
        self.assertEqual(started, ["RetroArch"])

    def test_on_process_exited_triggers_auto_upload_when_enabled(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend({**self.base_config, "auto_cloud_sync": True})
        backend._session_game = dict(self.game)
        backend._process = MagicMock()

        with patch("rom_mate.tv.bridge.game_backend._credentials_present", return_value=True), patch(
            "rom_mate.tv.bridge.game_backend.resolve_emulator_entry_for_game",
            return_value=("RetroArch", {"name": "RetroArch"}),
        ), patch("rom_mate.tv.bridge.game_backend._TvAutoUploadWorker") as mock_upload_worker, patch(
            "rom_mate.tv.bridge.game_backend.QThread.start"
        ):
            backend._on_process_exited("RetroArch")

        mock_upload_worker.assert_called_once()

    def test_on_process_exited_skips_upload_when_no_session_game(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend({**self.base_config, "auto_cloud_sync": True})
        backend._session_game = None
        backend._process = MagicMock()

        with patch("rom_mate.tv.bridge.game_backend._credentials_present", return_value=True), patch(
            "rom_mate.tv.bridge.game_backend._TvAutoUploadWorker"
        ) as mock_upload_worker:
            backend._on_process_exited("RetroArch")

        mock_upload_worker.assert_not_called()

    def test_on_process_exited_skips_upload_when_auto_sync_disabled(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config))
        backend._session_game = dict(self.game)
        backend._process = MagicMock()

        with patch("rom_mate.tv.bridge.game_backend._TvAutoUploadWorker") as mock_upload_worker:
            backend._on_process_exited("RetroArch")

        mock_upload_worker.assert_not_called()

    def test_on_restore_done_calls_do_launch_on_success(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config))
        statuses: list[str] = []
        backend.cloudSyncStatus.connect(statuses.append)

        with patch.object(backend, "_do_launch") as mock_do_launch:
            backend._on_restore_done(True, "Save restored.", "RetroArch", ["retroarch"], None)

        mock_do_launch.assert_called_once_with("RetroArch", ["retroarch"], None)
        self.assertEqual(statuses, [""])

    def test_on_restore_done_calls_do_launch_even_on_failure(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config))

        with patch.object(backend, "_do_launch") as mock_do_launch:
            backend._on_restore_done(False, "Restore failed.", "RetroArch", ["retroarch"], None)

        mock_do_launch.assert_called_once_with("RetroArch", ["retroarch"], None)

    def test_on_auto_upload_done_emits_cloud_sync_status(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config))
        statuses: list[str] = []
        backend.cloudSyncStatus.connect(statuses.append)

        backend._on_auto_upload_done(True, "Auto-uploaded 1 save file(s).")

        self.assertEqual(statuses, ["Auto-uploaded 1 save file(s)."])

    def test_on_auto_upload_done_skips_signal_when_empty_message(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config))
        statuses: list[str] = []
        backend.cloudSyncStatus.connect(statuses.append)

        backend._on_auto_upload_done(False, "")

        self.assertEqual(statuses, [])


class TestGameBackendInstall(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def test_is_install_active_false_initially(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            }
        )

        self.assertFalse(backend.isInstallActive)

    def test_install_game_emits_error_when_no_server_url(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "library_path": "/games",
                "installed_games": [],
            }
        )

        received: list[str] = []
        backend.launchError.connect(lambda message: received.append(message))

        backend.installGame({"id": "42", "name": "Test Game"})

        self.assertEqual(received, ["No server URL configured."])
        self.assertFalse(backend.isInstallActive)

    def test_install_game_emits_error_when_no_library_path(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "",
                "installed_games": [],
            }
        )

        received: list[str] = []
        backend.launchError.connect(lambda message: received.append(message))

        backend.installGame({"id": "42", "name": "Test Game"})

        self.assertEqual(received, ["No library path configured."])
        self.assertFalse(backend.isInstallActive)

    def test_install_game_emits_error_when_already_installing(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            }
        )
        backend._install_thread = MagicMock()
        backend._install_thread.isRunning.return_value = True

        received: list[str] = []
        backend.launchError.connect(lambda message: received.append(message))

        with patch("rom_mate.tv.bridge.game_backend.QThread.start") as mock_thread_start:
            backend.installGame({"id": "42", "name": "Test Game"})

        self.assertEqual(received, ["An install is already in progress."])
        mock_thread_start.assert_not_called()

    def test_on_install_download_done_error_emits_install_complete_false(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            }
        )
        backend._install_target_game = {"id": "1", "name": "Test"}

        received: list[tuple[bool, str, object]] = []
        backend.installComplete.connect(lambda ok, msg, payload: received.append((ok, msg, payload)))

        backend._on_install_download_done("", "network timeout")

        self.assertEqual(received, [(False, "network timeout", {"id": "1", "name": "Test"})])

    def test_on_install_finalize_done_success_adds_to_installed_games(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            }
        )
        backend._install_target_game = {"id": "99", "name": "Test"}

        received: list[tuple[bool, str, object]] = []
        backend.installComplete.connect(lambda ok, msg, payload: received.append((ok, msg, payload)))

        backend._on_install_finalize_done(True, "", "/games/Test/Test.iso")

        self.assertEqual(
            received,
            [(True, "Game installed.", {"id": "99", "name": "Test", "local_path": "/games/Test/Test.iso"})],
        )
        installed_games = backend._config.get("installed_games", [])
        self.assertTrue(any(entry.get("id") == "99" and entry.get("local_path") == "/games/Test/Test.iso" for entry in installed_games))

    def test_uninstall_game_emits_complete_and_removes_from_installed_games(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [{"id": "77", "name": "Test", "local_path": ""}],
            }
        )

        received: list[tuple[bool, str, object]] = []
        backend.uninstallComplete.connect(lambda ok, msg, payload: received.append((ok, msg, payload)))

        backend.uninstallGame({"id": "77", "name": "Test", "local_path": ""})

        self.assertEqual(received, [(True, "Game uninstalled.", {"id": "77", "name": "Test", "local_path": ""})])
        installed_games = backend._config.get("installed_games", [])
        self.assertFalse(any(entry.get("id") == "77" for entry in installed_games))


if __name__ == "__main__":
    unittest.main()
