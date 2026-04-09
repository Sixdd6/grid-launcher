from __future__ import annotations

import unittest

from rom_mate.background.workers import DetailsCloudRecordsWorker


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


if __name__ == "__main__":
    unittest.main()
