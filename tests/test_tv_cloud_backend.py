from __future__ import annotations

import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from PySide6.QtCore import QCoreApplication

from rom_mate.tv.bridge.cloud_backend import CloudBackend, _SlotFetchWorker

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
        backend.slotsError.connect(lambda save_type, msg: received.append((save_type, msg)))

        with patch("rom_mate.tv.bridge.cloud_backend.credentials_present", return_value=False), patch(
            "rom_mate.tv.bridge.cloud_backend.QThread.start"
        ) as mock_thread_start:
            backend.loadSlotsForGame({"rom_id": "42"}, "save")

        self.assertEqual(received, [("save", "Not connected to server.")])
        mock_thread_start.assert_not_called()

    def test_load_slots_for_game_without_rom_id_emits_error(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[str, str]] = []
        backend.slotsError.connect(lambda save_type, msg: received.append((save_type, msg)))

        with patch("rom_mate.tv.bridge.cloud_backend.credentials_present", return_value=True):
            backend.loadSlotsForGame({"rom_id": "", "id": ""}, "save")

        self.assertEqual(received, [("save", "Game has no server ID.")])

    def test_slot_fetch_worker_run_success_emits_finished_slots(self):
        worker = _SlotFetchWorker(config=dict(BASE_CONFIG), rom_id="42", save_type="save")
        finished: list[tuple[str, list[dict[str, str]]]] = []
        worker.finished.connect(lambda save_type, slots: finished.append((save_type, slots)))

        records = [
            {
                "id": 100,
                "file_name": "slot1.sav",
                "slot": "1",
                "emulator": "RetroArch",
                "updated_at": "2026-04-14T12:00:00Z",
            }
        ]

        with patch("rom_mate.tv.bridge.cloud_backend._api_get_json", return_value=[{"id": 100}]), patch(
            "rom_mate.tv.bridge.cloud_backend._server_records_from_payload", return_value=records
        ), patch("rom_mate.tv.bridge.cloud_backend._latest_server_records_by_slot", return_value=records), patch(
            "rom_mate.tv.bridge.cloud_backend._save_record_timestamp", return_value=1713096000.0
        ), patch(
            "rom_mate.tv.bridge.cloud_backend._relative_timestamp_text", return_value="just now"
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
        worker.finished.connect(lambda save_type, slots: finished.append((save_type, slots)))

        with patch("rom_mate.tv.bridge.cloud_backend._api_get_json", return_value=[]), patch(
            "rom_mate.tv.bridge.cloud_backend._server_records_from_payload", return_value=[]
        ), patch("rom_mate.tv.bridge.cloud_backend._latest_server_records_by_slot", return_value=[]):
            worker.run()

        self.assertEqual(finished, [("state", [])])

    def test_slot_fetch_worker_run_api_error_emits_error_signal(self):
        worker = _SlotFetchWorker(config=dict(BASE_CONFIG), rom_id="42", save_type="save")
        errors: list[tuple[str, str]] = []
        worker.error.connect(lambda save_type, msg: errors.append((save_type, msg)))

        with patch("rom_mate.tv.bridge.cloud_backend._api_get_json", side_effect=Exception("connection refused")):
            worker.run()

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0][0], "save")
        self.assertIn("connection refused", errors[0][1])

    def test_delete_slot_success_emits_complete_true(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.deleteComplete.connect(lambda ok, msg: received.append((ok, msg)))

        with patch("rom_mate.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "rom_mate.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
        ), patch("rom_mate.tv.bridge.cloud_backend._api_post_json", return_value=None):
            backend.deleteSlot("42", "save")

        self.assertEqual(received, [(True, "Save deleted.")])

    def test_delete_slot_api_error_emits_complete_false(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.deleteComplete.connect(lambda ok, msg: received.append((ok, msg)))

        with patch("rom_mate.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "rom_mate.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
        ), patch("rom_mate.tv.bridge.cloud_backend._api_post_json", side_effect=Exception("timeout")):
            backend.deleteSlot("42", "save")

        self.assertEqual(len(received), 1)
        self.assertFalse(received[0][0])
        self.assertIn("timeout", received[0][1])

    def test_restore_slot_without_install_dir_emits_complete_false(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.restoreComplete.connect(lambda ok, msg: received.append((ok, msg)))

        game = {"rom_id": "42", "id": "42", "name": "My Game", "install_dir": "", "local_path": ""}

        with patch("rom_mate.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "rom_mate.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
        ), patch("rom_mate.tv.bridge.cloud_backend._api_get_bytes", return_value=b"fake bytes"):
            backend.restoreSlot(game, "1", "save")

        self.assertEqual(len(received), 1)
        self.assertFalse(received[0][0])
        self.assertIn("Cannot determine save location", received[0][1])

    def test_restore_slot_success_emits_complete_true(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.restoreComplete.connect(lambda ok, msg: received.append((ok, msg)))

        with tempfile.TemporaryDirectory() as temp_dir:
            install_dir = Path(temp_dir) / "games" / "mygame"
            game = {
                "rom_id": "42",
                "id": "42",
                "name": "My Game",
                "install_dir": str(install_dir),
                "local_path": "",
            }

            with patch("rom_mate.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
                "rom_mate.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
            ), patch("rom_mate.tv.bridge.cloud_backend._api_get_bytes", return_value=b"fake bytes"), patch(
                "rom_mate.tv.bridge.cloud_backend._restore_single_save_payload",
                return_value=Path(temp_dir) / "save.sav",
            ):
                backend.restoreSlot(game, "1", "save")

        self.assertEqual(received, [(True, "Save restored successfully.")])

    def test_sync_config_updates_backend_config(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        backend.syncConfig({"api_token": "new"})
        self.assertEqual(backend._config["api_token"], "new")


class TestCloudBackendState(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication(sys.argv)

    def test_delete_slot_uses_state_endpoint(self):
        backend = CloudBackend(dict(BASE_CONFIG))

        with patch("rom_mate.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
            "rom_mate.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
        ), patch("rom_mate.tv.bridge.cloud_backend._api_post_json", return_value=None) as mock_post:
            backend.deleteSlot("99", "state")

        mock_post.assert_called_once_with(
            "http://server",
            "test-token",
            "/api/states/delete",
            {"states": [99]},
        )

    def test_restore_slot_uses_local_path_parent_when_no_install_dir(self):
        backend = CloudBackend(dict(BASE_CONFIG))
        received: list[tuple[bool, str]] = []
        backend.restoreComplete.connect(lambda ok, msg: received.append((ok, msg)))

        with tempfile.TemporaryDirectory() as temp_dir:
            local_path = Path(temp_dir) / "games" / "mygame" / "rom.sfc"
            game = {
                "rom_id": "42",
                "id": "42",
                "name": "My Game",
                "install_dir": "",
                "local_path": str(local_path),
            }

            with patch("rom_mate.tv.bridge.cloud_backend.credentials_present", return_value=True), patch(
                "rom_mate.tv.bridge.cloud_backend.server_base_url", return_value="http://server"
            ), patch("rom_mate.tv.bridge.cloud_backend._api_get_bytes", return_value=b"data"), patch(
                "rom_mate.tv.bridge.cloud_backend._restore_single_save_payload",
                return_value=Path(temp_dir) / "restored.sav",
            ):
                backend.restoreSlot(game, "1", "save")

        self.assertEqual(received, [(True, "Save restored successfully.")])


if __name__ == "__main__":
    unittest.main()
