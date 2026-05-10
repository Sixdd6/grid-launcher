from __future__ import annotations

import json
import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import sys
import threading
import unittest
from pathlib import Path
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
        self.backend = GameBackend(self.config, MagicMock())

    def test_active_emulator_name_is_empty_on_init(self):
        self.assertEqual(self.backend.activeEmulatorName, "")

    def test_active_game_title_empty_on_init(self):
        self.assertEqual(self.backend.activeGameTitle, "")

    def test_active_game_title_returns_title_field(self):
        self.backend._session_game = {"title": "Metroid", "name": ""}

        self.assertEqual(self.backend.activeGameTitle, "Metroid")

    def test_active_game_title_falls_back_to_name_when_title_empty(self):
        self.backend._session_game = {"title": "", "name": "Metroid Prime"}

        self.assertEqual(self.backend.activeGameTitle, "Metroid Prime")

    def test_active_game_title_empty_when_no_session_game(self):
        self.backend._session_game = None

        self.assertEqual(self.backend.activeGameTitle, "")

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
            },
            MagicMock(),
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

        backend = GameBackend({"emulators": [], "default_emulators": {}, "launch_args": ""}, MagicMock())
        backend._active_emulator_name = "RetroArch"
        backend._process = MagicMock()
        backend._process.wait.return_value = 0

        emitted: list[str] = []
        backend.sessionEnded.connect(lambda name: emitted.append(name))

        backend._on_process_exited("RetroArch")

        self.assertEqual(backend._active_emulator_name, "")
        self.assertIsNone(backend._process)
        self.assertEqual(emitted, ["RetroArch"])

    def test_process_watch_thread_monitors_native_game_with_empty_emulator_name(self):
        from rom_mate.tv.bridge.game_backend import _ProcessWatchThread, GameBackend

        backend = GameBackend({"emulators": [], "default_emulators": {}, "launch_args": ""}, MagicMock())
        backend._process = MagicMock()
        backend._process.wait.return_value = 0
        backend._active_emulator_name = ""

        thread = _ProcessWatchThread(backend)

        emitted: list[str] = []
        thread._exited.connect(lambda name: emitted.append(name))

        with patch("rom_mate.tv.bridge.game_backend._ProcessWatchThread.start"):
            thread.run()

        self.assertEqual(emitted, [""])

    def test_do_launch_connects_watch_thread_exited_signal(self):
        from PySide6.QtCore import Qt
        from rom_mate.tv.bridge.game_backend import GameBackend, _ProcessWatchThread

        backend = GameBackend({"emulators": [], "default_emulators": {}, "launch_args": ""}, MagicMock())

        mock_process = MagicMock()
        mock_process.poll.return_value = None

        created_threads: list[_ProcessWatchThread] = []

        original_init = _ProcessWatchThread.__init__

        def capture_thread(self_t, b, **kwargs):
            original_init(self_t, b, **kwargs)
            created_threads.append(self_t)

        with patch("rom_mate.tv.bridge.game_backend._subprocess_popen", return_value=mock_process), patch.object(
            _ProcessWatchThread, "__init__", capture_thread
        ), patch("rom_mate.tv.bridge.game_backend._ProcessWatchThread.start"):
            backend._do_launch("RetroArch", ["retroarch"], None)

        self.assertEqual(len(created_threads), 1)
        thread = created_threads[0]

        backend._process = mock_process
        handler = MagicMock()
        thread._exited.connect(handler, Qt.ConnectionType.QueuedConnection)

        thread._exited.emit("RetroArch")
        self.app.processEvents()

        self.assertTrue(handler.called)
        self.assertIsNone(backend._process)


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

        backend = GameBackend({**self.base_config, "auto_cloud_save_download_on_launch": True}, MagicMock())
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

        backend = GameBackend(dict(self.base_config), MagicMock())
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

        backend = GameBackend({**self.base_config, "auto_cloud_save_download_on_launch": True}, MagicMock())
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

        backend = GameBackend({**self.base_config, "auto_cloud_save_upload_on_exit": True}, MagicMock())
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

        backend = GameBackend({**self.base_config, "auto_cloud_save_upload_on_exit": True}, MagicMock())
        backend._session_game = None
        backend._process = MagicMock()

        with patch("rom_mate.tv.bridge.game_backend._credentials_present", return_value=True), patch(
            "rom_mate.tv.bridge.game_backend._TvAutoUploadWorker"
        ) as mock_upload_worker:
            backend._on_process_exited("RetroArch")

        mock_upload_worker.assert_not_called()

    def test_on_process_exited_skips_upload_when_auto_sync_disabled(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config), MagicMock())
        backend._session_game = dict(self.game)
        backend._process = MagicMock()

        with patch("rom_mate.tv.bridge.game_backend._TvAutoUploadWorker") as mock_upload_worker:
            backend._on_process_exited("RetroArch")

        mock_upload_worker.assert_not_called()

    def test_on_restore_done_calls_do_launch_on_success(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config), MagicMock())
        statuses: list[str] = []
        backend.cloudSyncStatus.connect(statuses.append)

        with patch.object(backend, "_do_launch") as mock_do_launch:
            backend._on_restore_done(True, "Save restored.", "RetroArch", ["retroarch"], None)

        mock_do_launch.assert_called_once_with("RetroArch", ["retroarch"], None)
        self.assertEqual(statuses, [""])

    def test_on_restore_done_calls_do_launch_even_on_failure(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config), MagicMock())

        with patch.object(backend, "_do_launch") as mock_do_launch:
            backend._on_restore_done(False, "Restore failed.", "RetroArch", ["retroarch"], None)

        mock_do_launch.assert_called_once_with("RetroArch", ["retroarch"], None)

    def test_on_auto_upload_done_emits_cloud_sync_status(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config), MagicMock())
        statuses: list[str] = []
        backend.cloudSyncStatus.connect(statuses.append)

        backend._on_auto_upload_done({"success": True, "message": "Auto-uploaded 1 save file(s)."})

        self.assertEqual(statuses, ["Auto-uploaded 1 save file(s)."])

    def test_on_auto_upload_done_skips_signal_when_empty_message(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        backend = GameBackend(dict(self.base_config), MagicMock())
        statuses: list[str] = []
        backend.cloudSyncStatus.connect(statuses.append)

        backend._on_auto_upload_done({"success": False, "message": ""})

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
            },
            MagicMock(),
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
            },
            MagicMock(),
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
            },
            MagicMock(),
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
            },
            MagicMock(),
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
            },
            MagicMock(),
        )
        backend._install_target_game = {"id": "1", "name": "Test"}

        received: list[tuple[bool, str, object]] = []
        backend.installComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                    bundle.get("game", {}) if isinstance(bundle, dict) else {},
                )
            )
        )

        backend._on_install_download_done("", "network timeout")

        self.assertEqual(received, [(False, "network timeout", {"id": "1", "name": "Test"})])

    def test_on_install_finalize_done_success_adds_to_installed_games(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        mock_main_window = MagicMock()
        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            },
            mock_main_window,
        )
        backend._install_target_game = {"id": "99", "name": "Test"}

        received: list[tuple[bool, str, object]] = []
        backend.installComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                    bundle.get("game", {}) if isinstance(bundle, dict) else {},
                )
            )
        )

        prepared_game = {"id": "99", "name": "Test", "extracted_path": "/games/Test/Test.iso"}
        backend._on_install_finalize_done(
            {
                "game_json": json.dumps(prepared_game),
                "archive_path": "/tmp/archive.zip",
                "warning": "",
                "error": "",
            }
        )

        self.assertEqual(len(received), 1)
        ok, msg, payload = received[0]
        self.assertTrue(ok)
        self.assertEqual(msg, "Game installed.")
        self.assertEqual(payload.get("id"), "99")
        self.assertEqual(payload.get("local_path"), "/games/Test/Test.iso")
        installed_games = backend._config.get("installed_games", [])
        self.assertTrue(any(entry.get("id") == "99" and entry.get("local_path") == "/games/Test/Test.iso" for entry in installed_games))

    def test_on_install_download_done_success_starts_finalize_thread(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        mock_main_window = MagicMock()
        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            },
            mock_main_window,
        )
        backend._install_target_game = {"id": "42", "name": "Test"}

        with patch("rom_mate.tv.bridge.game_backend.threading.Thread") as mock_thread_cls:
            mock_thread_instance = MagicMock()
            mock_thread_cls.return_value = mock_thread_instance
            backend._on_install_download_done("/tmp/archive.zip", "")

        mock_thread_cls.assert_called_once()
        mock_thread_instance.start.assert_called_once()
        self.assertIs(backend._finalize_thread, mock_thread_instance)

    def test_uninstall_game_emits_complete_and_removes_from_installed_games(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        mock_main_window = MagicMock()
        mock_main_window._uninstall_game.return_value = True
        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [{"id": "77", "name": "Test", "local_path": ""}],
            },
            mock_main_window,
        )

        received: list[tuple[bool, str, object]] = []
        backend.uninstallComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                    bundle.get("game", {}) if isinstance(bundle, dict) else {},
                )
            )
        )

        backend.uninstallGame({"id": "77", "name": "Test", "local_path": ""})

        mock_main_window._uninstall_game.assert_called_once()
        self.assertEqual(received, [(True, "Game uninstalled.", {"id": "77", "name": "Test", "local_path": ""})])

    def test_on_install_finalize_done_non_extracted_sets_archive_path(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        mock_main_window = MagicMock()
        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            },
            mock_main_window,
        )
        backend._install_target_game = {"id": "55", "name": "Test"}

        received: list[tuple[bool, str, object]] = []
        backend.installComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                    bundle.get("game", {}) if isinstance(bundle, dict) else {},
                )
            )
        )

        prepared_game = {"id": "55", "name": "Test", "extracted_path": ""}
        backend._on_install_finalize_done(
            {
                "game_json": json.dumps(prepared_game),
                "archive_path": "/tmp/archive.zip",
                "warning": "",
                "error": "",
            }
        )

        self.assertEqual(len(received), 1)
        ok, _msg, payload = received[0]
        self.assertTrue(ok)
        self.assertEqual(payload.get("archive_path"), "/tmp/archive.zip")
        self.assertEqual(payload.get("local_path"), "/tmp/archive.zip")
        installed_games = backend._config.get("installed_games", [])
        self.assertTrue(
            any(
                entry.get("id") == "55"
                and entry.get("archive_path") == "/tmp/archive.zip"
                and entry.get("local_path") == "/tmp/archive.zip"
                for entry in installed_games
            )
        )

    def test_on_install_finalize_done_persists_config(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        mock_main_window = MagicMock()
        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            },
            mock_main_window,
        )
        backend._install_target_game = {"id": "99", "name": "Test"}

        prepared_game = {"id": "99", "name": "Test", "extracted_path": "/games/Test/Test.iso"}
        backend._on_install_finalize_done(
            {
                "game_json": json.dumps(prepared_game),
                "archive_path": "/tmp/archive.zip",
                "warning": "",
                "error": "",
            }
        )

        mock_main_window._save_config.assert_called_once()

    def test_on_install_finalize_done_failure_does_not_persist_config(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        mock_main_window = MagicMock()
        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            },
            mock_main_window,
        )
        backend._install_target_game = {"id": "99", "name": "Test"}

        backend._on_install_finalize_done(
            {
                "game_json": "",
                "archive_path": "/tmp/archive.zip",
                "warning": "",
                "error": "Archive error",
            }
        )

        mock_main_window._save_config.assert_not_called()

    def test_uninstall_game_delegates_to_main_window(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        mock_main_window = MagicMock()
        mock_main_window._uninstall_game.return_value = True
        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [{"id": "55", "name": "Test", "local_path": "/games/Test/Test.iso"}],
            },
            mock_main_window,
        )

        backend.uninstallGame({"id": "55", "name": "Test", "local_path": "/games/Test/Test.iso"})

        mock_main_window._uninstall_game.assert_called_once()
        call_arg = mock_main_window._uninstall_game.call_args[0][0]
        self.assertEqual(call_arg.get("id"), "55")

    def test_uninstall_game_emits_failure_when_not_found(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        mock_main_window = MagicMock()
        mock_main_window._uninstall_game.return_value = False
        backend = GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            },
            mock_main_window,
        )

        received: list[tuple[bool, str, object]] = []
        backend.uninstallComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                    bundle.get("game", {}) if isinstance(bundle, dict) else {},
                )
            )
        )

        backend.uninstallGame({"id": "nonexistent", "name": "Test"})

        self.assertEqual(received, [(False, "Game not found in library.", {"id": "nonexistent", "name": "Test"})])


class TestGameBackendNativeLaunch(unittest.TestCase):
    def _make_backend(self, config=None):
        from rom_mate.tv.bridge.game_backend import GameBackend

        return GameBackend(config or {}, MagicMock())

    @patch("rom_mate.tv.bridge.game_backend.is_native_executable_platform", return_value=True)
    @patch("rom_mate.tv.bridge.game_backend.native_install_dir_for_game", return_value=Path("/fake/dir"))
    @patch("rom_mate.tv.bridge.game_backend.native_executable_candidates_for_game", return_value=[Path("/fake/dir/sub/game.exe")])
    @patch("rom_mate.tv.bridge.game_backend.resolved_native_executable_path_for_game", return_value=None)
    def test_launch_game_native_no_exe_emits_picker_needed(self, *_):
        backend = self._make_backend()
        captured = []
        backend.nativeExecPickerNeeded.connect(lambda c: captured.append(c))
        backend.launchGame({"rom_id": "1", "platform": "Windows", "local_path": "/fake/dir"})
        self.assertEqual(len(captured), 1)
        self.assertEqual(len(captured[0]), 1)
        self.assertIn("label", captured[0][0])
        self.assertIn("path", captured[0][0])
        self.assertEqual(captured[0][0]["path"], str(Path("/fake/dir/sub/game.exe")))

    @patch("rom_mate.tv.bridge.game_backend._ProcessWatchThread.start")
    @patch("rom_mate.tv.bridge.game_backend._subprocess_popen")
    @patch("rom_mate.tv.bridge.game_backend.prepare_native_launch_command", return_value=(["game.exe"], "/fake/dir"))
    @patch("rom_mate.tv.bridge.game_backend.resolved_native_executable_path_for_game", return_value=Path("/fake/dir/game.exe"))
    @patch("rom_mate.tv.bridge.game_backend.native_executable_candidates_for_game", return_value=[Path("/fake/dir/game.exe")])
    @patch("rom_mate.tv.bridge.game_backend.native_install_dir_for_game", return_value=Path("/fake/dir"))
    @patch("rom_mate.tv.bridge.game_backend.is_native_executable_platform", return_value=True)
    def test_launch_game_native_with_exe_calls_do_launch(self, _isnative, _installdir, _candidates, _resolved, _prepare, mock_popen, _watchstart):
        mock_popen.return_value = MagicMock()
        backend = self._make_backend()
        backend.launchGame({"rom_id": "1", "platform": "Windows", "local_path": "/fake/dir"})
        mock_popen.assert_called_once()
        self.assertIsNotNone(backend._session_game)

    @patch("rom_mate.tv.bridge.game_backend.resolved_native_executable_path_for_game", return_value=None)
    @patch("rom_mate.tv.bridge.game_backend.native_executable_candidates_for_game", return_value=[])
    @patch("rom_mate.tv.bridge.game_backend.native_install_dir_for_game", return_value=Path("/fake/dir"))
    @patch("rom_mate.tv.bridge.game_backend.is_native_executable_platform", return_value=True)
    def test_launch_game_native_no_candidates_emits_error(self, *_):
        backend = self._make_backend()
        captured = []
        backend.launchError.connect(lambda m: captured.append(m))
        backend.launchGame({"rom_id": "1", "platform": "Windows", "local_path": "/fake/dir"})
        self.assertEqual(len(captured), 1)
        self.assertIn("No executable", captured[0])

    @patch("rom_mate.tv.bridge.game_backend._write_config_file")
    def test_save_native_executable_updates_config(self, mock_write):
        config = {
            "installed_games": [
                {"rom_id": "42", "title": "My Game", "local_path": "/games/mygame"}
            ]
        }
        backend = self._make_backend(config)
        backend.saveNativeExecutable({"rom_id": "42", "exe_path": "/games/mygame/launcher.exe"})
        self.assertEqual(config["installed_games"][0]["native_executable_path"], "/games/mygame/launcher.exe")
        mock_write.assert_called_once()

    @patch("rom_mate.tv.bridge.game_backend.native_executable_candidates_for_game", return_value=[Path("/fake/dir/sub/game.exe")])
    @patch("rom_mate.tv.bridge.game_backend.native_install_dir_for_game", return_value=Path("/fake/dir"))
    def test_get_native_executable_candidates_returns_list(self, *_):
        config = {
            "installed_games": [
                {"rom_id": "7", "extracted_dir": "/fake/dir"}
            ]
        }
        backend = self._make_backend(config)
        result = backend.getNativeExecutableCandidates("7")
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["label"], str(Path("sub/game.exe")))
        self.assertEqual(result[0]["path"], str(Path("/fake/dir/sub/game.exe")))


class TestGameBackendInstallActiveFinalize(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def _make_backend(self):
        from rom_mate.tv.bridge.game_backend import GameBackend

        return GameBackend(
            {
                "emulators": [],
                "default_emulators": {},
                "launch_args": "",
                "server_url": "http://romm.local",
                "library_path": "/games",
                "installed_games": [],
            },
            MagicMock(),
        )

    def test_install_active_during_finalize_phase(self):
        backend = self._make_backend()

        mock_thread = MagicMock()
        mock_thread.is_alive.return_value = True
        backend._finalize_thread = mock_thread
        backend._install_thread = None

        self.assertTrue(backend.isInstallActive)

    def test_install_game_rejected_when_finalize_thread_running(self):
        backend = self._make_backend()

        mock_thread = MagicMock()
        mock_thread.is_alive.return_value = True
        backend._finalize_thread = mock_thread
        backend._install_thread = None

        received: list[str] = []
        backend.launchError.connect(lambda message: received.append(message))

        with patch("rom_mate.tv.bridge.game_backend.QThread.start") as mock_thread_start:
            backend.installGame({"id": "42", "name": "Test Game"})

        self.assertEqual(received, ["An install is already in progress."])
        mock_thread_start.assert_not_called()

    def test_install_game_rejected_when_install_target_game_set(self):
        backend = self._make_backend()

        # Simulate state where threads have cleared but _install_target_game is still set
        backend._install_thread = None
        backend._finalize_thread = None
        backend._install_target_game = {"rom_id": "123", "name": "In Progress Game"}

        received: list[tuple[bool, str, object]] = []
        backend.installComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                    bundle.get("game", {}) if isinstance(bundle, dict) else {},
                )
            )
        )

        with patch("rom_mate.tv.bridge.game_backend.QThread.start") as mock_thread_start:
            backend.installGame({"id": "99", "name": "New Game"})

        self.assertEqual(len(received), 1)
        ok, msg, _ = received[0]
        self.assertFalse(ok)
        self.assertEqual(msg, "An install is already in progress.")
        mock_thread_start.assert_not_called()
        # Ensure original target game was not overwritten
        self.assertEqual(backend._install_target_game.get("rom_id"), "123")

    def test_is_install_active_false_when_finalize_thread_not_running(self):
        backend = self._make_backend()

        mock_thread = MagicMock()
        mock_thread.is_alive.return_value = False
        backend._finalize_thread = mock_thread
        backend._install_thread = None

        self.assertFalse(backend.isInstallActive)

    def test_install_state_active_during_finalize_phase_via_signal(self):
        # Verifies that _installStateChanged is emitted AFTER _finalize_thread is
        # assigned, so isInstallActive reads True at the moment of the signal.
        backend = self._make_backend()

        observed: list[bool] = []
        backend._installStateChanged.connect(lambda: observed.append(backend.isInstallActive))

        mock_finalize = MagicMock()
        mock_finalize.is_alive.return_value = True

        backend._install_thread = None
        backend._install_worker = None
        backend._finalize_thread = mock_finalize
        backend._installStateChanged.emit()

        self.assertEqual(len(observed), 1)
        self.assertTrue(observed[0])


class TestGameBackendInstallPlatformSubfolder(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def _make_backend(self, extra_config=None):
        from rom_mate.tv.bridge.game_backend import GameBackend

        config = {
            "emulators": [],
            "default_emulators": {},
            "launch_args": "",
            "server_url": "http://romm.local",
            "library_path": "/games",
            "installed_games": [],
        }
        if extra_config:
            config.update(extra_config)
        return GameBackend(config, MagicMock())

    def test_install_game_uses_platform_subfolder(self):
        backend = self._make_backend()
        captured_archive_paths: list[Path] = []

        def fake_worker(download_url, headers, archive_path):
            captured_archive_paths.append(archive_path)
            worker = MagicMock()
            worker.progress = MagicMock()
            worker.finished = MagicMock()
            worker.progress.connect = MagicMock()
            worker.finished.connect = MagicMock()
            return worker

        game = {"id": "42", "name": "Breath of the Wild", "platform": "Nintendo Switch"}

        with patch("rom_mate.tv.bridge.game_backend._InstallDownloadWorker", side_effect=fake_worker), \
             patch("rom_mate.tv.bridge.game_backend.QThread") as mock_qthread_cls, \
             patch("rom_mate.tv.bridge.game_backend.Path.mkdir"):
            mock_thread = MagicMock()
            mock_thread.isRunning.return_value = False
            mock_qthread_cls.return_value = mock_thread
            backend.installGame(game)

        self.assertEqual(len(captured_archive_paths), 1)
        archive_path = captured_archive_paths[0]
        self.assertIn("Nintendo Switch", str(archive_path))
        self.assertEqual(Path(archive_path).parent, Path("/games/Nintendo Switch"))

    def test_install_game_uses_library_root_when_no_platform(self):
        backend = self._make_backend()
        captured_archive_paths: list[Path] = []

        def fake_worker(download_url, headers, archive_path):
            captured_archive_paths.append(archive_path)
            worker = MagicMock()
            worker.progress = MagicMock()
            worker.finished = MagicMock()
            worker.progress.connect = MagicMock()
            worker.finished.connect = MagicMock()
            return worker

        game = {"id": "99", "name": "Mystery Game"}

        with patch("rom_mate.tv.bridge.game_backend._InstallDownloadWorker", side_effect=fake_worker), \
             patch("rom_mate.tv.bridge.game_backend.QThread") as mock_qthread_cls, \
             patch("rom_mate.tv.bridge.game_backend.Path.mkdir"):
            mock_thread = MagicMock()
            mock_thread.isRunning.return_value = False
            mock_qthread_cls.return_value = mock_thread
            backend.installGame(game)

        self.assertEqual(len(captured_archive_paths), 1)
        archive_path = captured_archive_paths[0]
        parts = Path(archive_path).parts
        # Should be directly in /games/, not in a platform subfolder
        self.assertEqual(parts[-2], "games")
        self.assertEqual(Path(archive_path).parent, Path("/games"))

    def test_install_game_archive_named_after_rom_file(self):
        backend = self._make_backend()
        captured_archive_paths: list[Path] = []

        def fake_worker(download_url, headers, archive_path):
            captured_archive_paths.append(archive_path)
            worker = MagicMock()
            worker.progress = MagicMock()
            worker.finished = MagicMock()
            worker.progress.connect = MagicMock()
            worker.finished.connect = MagicMock()
            return worker

        game = {
            "id": "1470",
            "name": "Sonic the Hedgehog",
            "platform": "Sega Genesis",
            "rom_file_name": "Sonic the Hedgehog (USA).zip",
        }

        with patch("rom_mate.tv.bridge.game_backend._InstallDownloadWorker", side_effect=fake_worker), \
             patch("rom_mate.tv.bridge.game_backend.QThread") as mock_qthread_cls, \
             patch("rom_mate.tv.bridge.game_backend.Path.mkdir"):
            mock_thread = MagicMock()
            mock_thread.isRunning.return_value = False
            mock_qthread_cls.return_value = mock_thread
            backend.installGame(game)

        self.assertEqual(len(captured_archive_paths), 1)
        archive_path = captured_archive_paths[0]
        self.assertTrue(str(archive_path).endswith("Sonic the Hedgehog (USA).zip"))

    def test_install_game_archive_fallback_name_when_no_rom_file(self):
        backend = self._make_backend()
        captured_archive_paths: list[Path] = []

        def fake_worker(download_url, headers, archive_path):
            captured_archive_paths.append(archive_path)
            worker = MagicMock()
            worker.progress = MagicMock()
            worker.finished = MagicMock()
            worker.progress.connect = MagicMock()
            worker.finished.connect = MagicMock()
            return worker

        game = {"id": "1470", "name": "Mystery Game", "platform": "Sega Genesis"}

        with patch("rom_mate.tv.bridge.game_backend._InstallDownloadWorker", side_effect=fake_worker), \
             patch("rom_mate.tv.bridge.game_backend.QThread") as mock_qthread_cls, \
             patch("rom_mate.tv.bridge.game_backend.Path.mkdir"):
            mock_thread = MagicMock()
            mock_thread.isRunning.return_value = False
            mock_qthread_cls.return_value = mock_thread
            backend.installGame(game)

        self.assertEqual(len(captured_archive_paths), 1)
        archive_path = captured_archive_paths[0]
        self.assertEqual(Path(archive_path).name, "_tv_download_1470.tmp")


if __name__ == "__main__":
    unittest.main()
