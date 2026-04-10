from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


class _WindowStub:
    def __init__(self, payload: object) -> None:
        self._payload = payload
        self.calls: list[tuple[str, dict[str, str]]] = []

    def _api_get(self, path: str, params: dict[str, str]):
        self.calls.append((path, params))
        return self._payload


class CloudStateFilterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_cloud_state_filter_tests", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load rom-mate.py for tests.")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        cls.module = module

    def test_server_state_records_filter_supported_image_extensions(self) -> None:
        payload = [
            {"id": 1, "file_name": "slot1.state"},
            {"id": 2, "file_name": "slot1.state.png"},
            {"id": 3, "file_name": "slot2.STATE.JPEG"},
            {"id": 4, "file_name": "slot3.state.bin"},
            {"id": 5, "file_name": "slot4.state.webp"},
        ]
        window = _WindowStub(payload)

        records = self.module.MainWindow._server_state_records_for_rom(window, "321")

        self.assertEqual(window.calls, [('/api/states', {'rom_id': '321'})])
        self.assertEqual([record.get("id") for record in records], [1, 4])

    def test_is_state_file_candidate_rejects_state_image_sidecars(self) -> None:
        window = object()

        self.assertFalse(self.module.MainWindow._is_state_file_candidate(window, Path("game.state.png")))
        self.assertFalse(self.module.MainWindow._is_state_file_candidate(window, Path("game.state1.png")))
        self.assertFalse(self.module.MainWindow._is_state_file_candidate(window, Path("game.state.jpg")))

    def test_is_state_file_candidate_accepts_state_and_state_slot_files(self) -> None:
        window = object()

        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("game.state")))
        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("game.state1")))


if __name__ == "__main__":
    unittest.main()
