from __future__ import annotations

import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from PySide6.QtCore import QCoreApplication

from grid_launcher.tv.bridge.cloud_backend import CloudBackend, _CloudUploadWorker, _SlotFetchWorker

BASE_CONFIG = {
    "server_url": "http://romm.local",
    "api_token": "test-token",
    "username": "user",
    "installed_games": [],
    "emulators": [],
    "default_emulators": {},
}


class TestCloudBackend(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def test_initial_state_stores_config(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        self.assertEqual(backend._config, BASE_CONFIG)

    def test_load_slots_for_game_without_credentials_emits_error_and_skips_thread(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[str, str]] = []
        backend.slotsError.connect(
            lambda bundle: received.append(
                (
                    bundle.get("save_type", "") if isinstance(bundle, dict) else "",
                    bundle.get("error", "") if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=False), patch(
            "grid_launcher.tv.bridge.cloud_backend.QThread.start"
        ) as mock_thread_start:
            backend.loadSlotsForGame({"game": {"rom_id": "42"}, "save_type": "save"})

        self.assertEqual(received, [("save", "Not connected to server.")])
        mock_thread_start.assert_not_called()

    def test_load_slots_for_game_without_rom_id_emits_error(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[str, str]] = []
        backend.slotsError.connect(
            lambda bundle: received.append(
                (
                    bundle.get("save_type", "") if isinstance(bundle, dict) else "",
                    bundle.get("error", "") if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=True):
            backend.loadSlotsForGame({"game": {"rom_id": "", "id": ""}, "save_type": "save"})

        self.assertEqual(received, [("save", "Game has no server ID.")])

    def test_slot_fetch_worker_run_success_emits_finished_slots(self):
        worker = _SlotFetchWorker(config=dict(BASE_CONFIG), rom_id="42", save_type="save")
        finished: list[tuple[str, list[dict[str, str]]]] = []
        worker.finished.connect(
            lambda bundle: finished.append(
                (
                    bundle.get("save_type", "") if isinstance(bundle, dict) else "",
                    bundle.get("slots", []) if isinstance(bundle, dict) else [],
                )
            )
        )

        records = [
            {
                "id": 100,
                "file_name": "slot1.sav",
                "slot": "1",
                "emulator": "RetroArch",
                "updated_at": "2026-04-14T12:00:00Z",
            }
        ]

        with patch("grid_launcher.tv.bridge.cloud_backend._api_get_json", return_value=[{"id": 100}]), patch(
            "grid_launcher.tv.bridge.cloud_backend._server_records_from_payload", return_value=records
        ), patch("grid_launcher.tv.bridge.cloud_backend._latest_server_records_by_slot", return_value=records), patch(
            "grid_launcher.tv.bridge.cloud_backend._save_record_timestamp", return_value=1713096000.0
        ), patch(
            "grid_launcher.tv.bridge.cloud_backend._relative_timestamp_text", return_value="just now"
        ):
            worker.run()

        self.assertEqual(len(finished), 1)
        save_type, slots = finished[0]
        self.assertEqual(save_type, "save")
        self.assertTrue(slots)
        self.assertEqual(
            slots[0],
            {
                "id": "100",
                "file_name": "slot1.sav",
                "slot": "1",
                "emulator": "RetroArch",
                "timestamp_text": "just now",
                "updated_at": "2026-04-14T12:00:00Z",
            },
        )

    def test_slot_fetch_worker_run_success_emits_finished_empty_slots(self):
        worker = _SlotFetchWorker(config=dict(BASE_CONFIG), rom_id="42", save_type="state")
        finished: list[tuple[str, list[dict[str, str]]]] = []
        worker.finished.connect(
            lambda bundle: finished.append(
                (
                    bundle.get("save_type", "") if isinstance(bundle, dict) else "",
                    bundle.get("slots", []) if isinstance(bundle, dict) else [],
                )
            )
        )

        with patch("grid_launcher.tv.bridge.cloud_backend._api_get_json", return_value=[]), patch(
            "grid_launcher.tv.bridge.cloud_backend._server_records_from_payload", return_value=[]
        ), patch("grid_launcher.tv.bridge.cloud_backend._latest_server_records_by_slot", return_value=[]):
            worker.run()

        self.assertEqual(finished, [("state", [])])

    def test_slot_fetch_worker_run_api_error_emits_error_signal(self):
        worker = _SlotFetchWorker(config=dict(BASE_CONFIG), rom_id="42", save_type="save")
        errors: list[tuple[str, str]] = []
        worker.error.connect(
            lambda bundle: errors.append(
                (
                    bundle.get("save_type", "") if isinstance(bundle, dict) else "",
                    bundle.get("error", "") if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch("grid_launcher.tv.bridge.cloud_backend._api_get_json", side_effect=Exception("connection refused")):
            worker.run()

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0][0], "save")
        self.assertIn("connection refused", errors[0][1])

    def test_delete_slot_success_emits_complete_true(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.deleteComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "grid_launcher.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
        ), patch("grid_launcher.tv.bridge.cloud_backend._api_post_json", return_value=None):
            backend.deleteSlot({"save_id": "42", "save_type": "save"})

        self.assertEqual(received, [(True, "Save deleted.")])

    def test_delete_slot_api_error_emits_complete_false(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.deleteComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "grid_launcher.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
        ), patch("grid_launcher.tv.bridge.cloud_backend._api_post_json", side_effect=Exception("timeout")):
            backend.deleteSlot({"save_id": "42", "save_type": "save"})

        self.assertEqual(len(received), 1)
        self.assertFalse(received[0][0])
        self.assertIn("timeout", received[0][1])

    def test_restore_slot_without_install_dir_emits_complete_false(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.restoreComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        game = {"rom_id": "42", "id": "42", "name": "My Game", "install_dir": "", "local_path": ""}

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "grid_launcher.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
        ), patch("grid_launcher.tv.bridge.cloud_backend._api_get_bytes", return_value=b"fake bytes"):
            backend.restoreSlot({"game": game, "save_id": "1", "save_type": "save"})

        self.assertEqual(len(received), 1)
        self.assertFalse(received[0][0])
        self.assertIn("Cannot determine save location", received[0][1])

    def test_restore_slot_success_emits_complete_true(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.restoreComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            install_dir = Path(temp_dir) / "games" / "mygame"
            game = {
                "rom_id": "42",
                "id": "42",
                "name": "My Game",
                "install_dir": str(install_dir),
                "local_path": "",
            }

            with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
                "grid_launcher.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
            ), patch("grid_launcher.tv.bridge.cloud_backend._api_get_bytes", return_value=b"fake bytes"), patch(
                "grid_launcher.tv.bridge.cloud_backend._restore_single_save_payload",
                return_value=Path(temp_dir) / "save.sav",
            ):
                backend.restoreSlot({"game": game, "save_id": "1", "save_type": "save"})

        self.assertEqual(received, [(True, "Save restored successfully.")])

    def test_sync_config_updates_backend_config(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        backend.syncConfig({"api_token": "new"})
        self.assertEqual(backend._config["api_token"], "new")

    def test_upload_save_emits_upload_complete_false_when_no_credentials(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.uploadComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=False):
            backend.uploadSave({"game": {"id": "5", "name": "Game"}, "save_type": "save"})

        self.assertEqual(received, [(False, "Not signed in to cloud saves.")])

    def test_upload_save_starts_thread_when_credentials_present(self):
        backend = CloudBackend(dict(BASE_CONFIG))

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "grid_launcher.tv.bridge.cloud_backend.QThread.start"
        ) as mock_thread_start:
            backend.uploadSave({"game": {"id": "5", "name": "Game", "install_dir": "/tmp/fake"}, "save_type": "save"})

        mock_thread_start.assert_called_once_with()
        self.assertIsNotNone(backend._upload_thread)

    def test_upload_save_cancels_previous_upload_thread(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        previous_thread = MagicMock()
        previous_thread.isRunning.return_value = True
        backend._upload_thread = previous_thread

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "grid_launcher.tv.bridge.cloud_backend.QThread.start"
        ):
            backend.uploadSave({"game": {"id": "5", "name": "Game", "install_dir": "/tmp/fake"}, "save_type": "save"})

        previous_thread.quit.assert_called_once_with()
        previous_thread.wait.assert_called_once_with(2000)

    def test_on_upload_done_emits_upload_complete(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.uploadComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        backend._on_upload_done({"success": True, "message": "Save uploaded successfully."})

        self.assertEqual(received, [(True, "Save uploaded successfully.")])


class TestCloudUploadWorker(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def test_upload_worker_run_emits_success_when_emulator_found(self):
        worker = _CloudUploadWorker(
            config=dict(BASE_CONFIG),
            game_dict={"id": "1", "platform": "SNES", "name": "Game"},
            save_type="save",
        )
        received: list[tuple[bool, str]] = []
        worker.finished.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch(
            "grid_launcher.tv.bridge.cloud_backend.resolve_emulator_entry_for_game",
            return_value=("RetroArch", {"name": "RetroArch"}),
        ), patch(
            "grid_launcher.tv.bridge.cloud_backend.perform_tv_save_upload",
            return_value=(1, 1, []),
        ):
            worker.run()

        self.assertEqual(len(received), 1)
        self.assertTrue(received[0][0])
        self.assertTrue(received[0][1])
        self.assertIn("Uploaded", received[0][1])

    def test_upload_worker_run_emits_failure_when_no_emulator(self):
        worker = _CloudUploadWorker(
            config=dict(BASE_CONFIG),
            game_dict={"id": "1", "platform": "SNES", "name": "Game"},
            save_type="save",
        )
        received: list[tuple[bool, str]] = []
        worker.finished.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch(
            "grid_launcher.tv.bridge.cloud_backend.resolve_emulator_entry_for_game",
            return_value=("", None),
        ):
            worker.run()

        self.assertEqual(len(received), 1)
        self.assertFalse(received[0][0])
        self.assertIn("No emulator configured", received[0][1])

    def test_upload_worker_run_emits_failure_when_no_files_found(self):
        worker = _CloudUploadWorker(
            config=dict(BASE_CONFIG),
            game_dict={"id": "1", "platform": "SNES", "name": "Game"},
            save_type="save",
        )
        received: list[tuple[bool, str]] = []
        worker.finished.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch(
            "grid_launcher.tv.bridge.cloud_backend.resolve_emulator_entry_for_game",
            return_value=("RetroArch", {"name": "RetroArch"}),
        ), patch(
            "grid_launcher.tv.bridge.cloud_backend.perform_tv_save_upload",
            return_value=(0, 0, ["No save directories found for this emulator"]),
        ):
            worker.run()

        self.assertEqual(len(received), 1)
        self.assertFalse(received[0][0])
        self.assertIn("No save files", received[0][1])

    def test_upload_worker_run_partial_upload(self):
        worker = _CloudUploadWorker(
            config=dict(BASE_CONFIG),
            game_dict={"id": "1", "platform": "SNES", "name": "Game"},
            save_type="save",
        )
        received: list[tuple[bool, str]] = []
        worker.finished.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch(
            "grid_launcher.tv.bridge.cloud_backend.resolve_emulator_entry_for_game",
            return_value=("RetroArch", {"name": "RetroArch"}),
        ), patch(
            "grid_launcher.tv.bridge.cloud_backend.perform_tv_save_upload",
            return_value=(1, 2, ["file2.srm"]),
        ):
            worker.run()

        self.assertEqual(len(received), 1)
        self.assertTrue(received[0][0])
        self.assertIn("1/2", received[0][1])

    def test_upload_save_skips_when_no_credentials(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.uploadComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=False), patch(
            "grid_launcher.tv.bridge.cloud_backend.QThread.start"
        ) as mock_thread_start:
            backend.uploadSave({"game": {"id": "5", "name": "Game"}, "save_type": "save"})

        self.assertEqual(received, [(False, "Not signed in to cloud saves.")])
        mock_thread_start.assert_not_called()


class TestCloudBackendState(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def test_delete_slot_uses_state_endpoint(self):
        backend = CloudBackend(dict(BASE_CONFIG))

        with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "grid_launcher.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
        ), patch("grid_launcher.tv.bridge.cloud_backend._api_post_json", return_value=None) as mock_post:
            backend.deleteSlot({"save_id": "99", "save_type": "state"})

        mock_post.assert_called_once_with(
            "http://server",
            "test-token",
            "/api/states/delete",
            {"states": [99]},
        )

    def test_restore_slot_uses_local_path_parent_when_no_install_dir(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.restoreComplete.connect(
            lambda bundle: received.append(
                (
                    bool(bundle.get("success", False)) if isinstance(bundle, dict) else False,
                    str(bundle.get("message", "")) if isinstance(bundle, dict) else "",
                )
            )
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            local_path = Path(temp_dir) / "games" / "mygame" / "rom.sfc"
            game = {
                "rom_id": "42",
                "id": "42",
                "name": "My Game",
                "install_dir": "",
                "local_path": str(local_path),
            }

            with patch("grid_launcher.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
                "grid_launcher.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
            ), patch("grid_launcher.tv.bridge.cloud_backend._api_get_bytes", return_value=b"data"), patch(
                "grid_launcher.tv.bridge.cloud_backend._restore_single_save_payload",
                return_value=Path(temp_dir) / "restored.sav",
            ):
                backend.restoreSlot({"game": game, "save_id": "1", "save_type": "save"})

        self.assertEqual(received, [(True, "Save restored successfully.")])


if __name__ == "__main__":
    unittest.main()
