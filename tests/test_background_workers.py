from __future__ import annotations

import tempfile
import unittest
from io import BytesIO
from pathlib import Path
from unittest.mock import patch
from urllib.error import HTTPError

from rom_mate.background.workers import DetailsCloudRecordsWorker, InstallDownloadWorker


class _StubWindow:
    def __init__(self) -> None:
        self.called: list[tuple[str, str]] = []

    def _server_save_records_for_rom(self, rom_id: str):
        self.called.append(("save", rom_id))
        return [{"id": "1", "file_name": "save-1.zip"}]

    def _server_state_records_for_rom(self, rom_id: str):
        self.called.append(("state", rom_id))
        return [{"id": "2", "file_name": "state-1.zip"}]


class _FailingWindow(_StubWindow):
    def _server_save_records_for_rom(self, rom_id: str):
        raise ValueError("boom")


class DetailsCloudRecordsWorkerTests(unittest.TestCase):
    def test_worker_fetches_save_records(self) -> None:
        window = _StubWindow()
        worker = DetailsCloudRecordsWorker(window, 7, "99", "save")
        results: list[tuple[int, str, object, str]] = []
        worker.finished.connect(lambda request_id, save_type, records, error: results.append((request_id, save_type, records, error)))

        worker.run()

        self.assertEqual(window.called, [("save", "99")])
        self.assertEqual(results, [(7, "save", [{"id": "1", "file_name": "save-1.zip"}], "")])

    def test_worker_emits_error_for_failed_requests(self) -> None:
        window = _FailingWindow()
        worker = DetailsCloudRecordsWorker(window, 9, "88", "save")
        results: list[tuple[int, str, object, str]] = []
        worker.finished.connect(lambda request_id, save_type, records, error: results.append((request_id, save_type, records, error)))

        worker.run()

        self.assertEqual(results, [(9, "save", [], "boom")])


class InstallDownloadWorkerTests(unittest.TestCase):
    def test_http_error_includes_status_reason_url_and_body(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "game.zip"
            worker = InstallDownloadWorker(
                "https://server.example/api/roms/1/content/game.zip",
                {"Accept": "*/*"},
                archive_path,
            )
            results: list[tuple[str, str]] = []
            worker.finished.connect(lambda path, error: results.append((path, error)))

            http_error = HTTPError(
                "https://server.example/api/roms/1/content/game.zip",
                403,
                "Forbidden",
                None,
                BytesIO(b'{"detail":"Token invalid for this ROM"}'),
            )
            with patch("rom_mate.background.workers.urlopen", side_effect=http_error):
                worker.run()

            self.assertEqual(len(results), 1)
            self.assertEqual(results[0][0], "")
            self.assertIn("HTTP 403 Forbidden", results[0][1])
            self.assertIn("url=https://server.example/api/roms/1/content/game.zip", results[0][1])
            self.assertIn("Token invalid for this ROM", results[0][1])

    def test_debug_logging_prints_url_and_error_details(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "game.zip"
            worker = InstallDownloadWorker(
                "https://server.example/api/roms/1/content/game.zip",
                {"Accept": "*/*"},
                archive_path,
                debug_enabled=True,
            )
            results: list[tuple[str, str]] = []
            worker.finished.connect(lambda path, error: results.append((path, error)))

            http_error = HTTPError(
                "https://server.example/api/roms/1/content/game.zip",
                401,
                "Unauthorized",
                None,
                BytesIO(b"access denied"),
            )
            with patch("rom_mate.background.workers.urlopen", side_effect=http_error):
                with patch("builtins.print") as mock_print:
                    worker.run()

            self.assertEqual(len(results), 1)
            printed_text = "\n".join(" ".join(str(item) for item in call.args) for call in mock_print.call_args_list)
            self.assertIn("[DEBUG][InstallDownload] url=https://server.example/api/roms/1/content/game.zip", printed_text)
            self.assertIn("[DEBUG][InstallDownload] error=HTTP 401 Unauthorized", printed_text)


if __name__ == "__main__":
    unittest.main()
