from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

from rom_mate.library.cloud_sync import (
    _state_candidate_base_variants,
    _state_candidate_hash_group_key,
    _state_candidate_matches_game_tokens,
)


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

    def test_is_state_file_candidate_accepts_duckstation_slot_files(self) -> None:
        window = object()

        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("SCUS-94900_1.sav")))
        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("SCUS-94900_2.sav")))
        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("SCUS-94900_resume.sav")))
        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("SLUS-01234_0.sav")))

    def test_is_state_file_candidate_accepts_dot_slot_sav_files(self) -> None:
        window = object()

        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("GameName.0.sav")))
        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("GameName.1.sav")))
        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("D4A53E48.0.sav")))

    def test_state_candidate_base_variants_strips_duckstation_naming(self) -> None:
        slot_variants = _state_candidate_base_variants(Path("SCUS-94900_1.sav"))
        resume_variants = _state_candidate_base_variants(Path("SCUS-94900_resume.sav"))
        dot_slot_variants = _state_candidate_base_variants(Path("GameName.0.sav"))

        self.assertIn("scus-94900", slot_variants)
        self.assertIn("scus-94900", resume_variants)
        self.assertIn("gamename", dot_slot_variants)

    def test_state_candidate_hash_group_key_handles_duckstation_naming(self) -> None:
        # Existing hex hash format
        self.assertEqual(_state_candidate_hash_group_key(Path("D4A53E48.0.sav")), "d4a53e48")
        self.assertEqual(_state_candidate_hash_group_key(Path("D4A53E48.1.sav")), "d4a53e48")
        # DuckStation serial format
        self.assertEqual(_state_candidate_hash_group_key(Path("SCUS-94900_1.sav")), "scus-94900")
        self.assertEqual(_state_candidate_hash_group_key(Path("SCUS-94900_resume.sav")), "scus-94900")
        self.assertEqual(_state_candidate_hash_group_key(Path("SLUS-01234_0.sav")), "slus-01234")
        # Same serial produces same key
        self.assertEqual(
            _state_candidate_hash_group_key(Path("SCUS-94900_1.sav")),
            _state_candidate_hash_group_key(Path("SCUS-94900_resume.sav")),
        )
        # Non-matching files return empty
        self.assertEqual(_state_candidate_hash_group_key(Path("random.txt")), "")

    def test_is_state_file_candidate_accepts_pcsx2_p2s_files(self) -> None:
        window = object()

        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("SLUS-12345 (00000000).00.p2s")))
        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("SCPS-12345 (ABCDEF12).01.p2s")))
        self.assertTrue(self.module.MainWindow._is_state_file_candidate(window, Path("game.p2s")))

    def test_state_candidate_base_variants_strips_pcsx2_p2s_naming(self) -> None:
        full_variants = _state_candidate_base_variants(Path("SLUS-12345 (00000000).00.p2s"))
        self.assertIn("slus-12345", full_variants)

        no_crc_variants = _state_candidate_base_variants(Path("SLUS-12345.01.p2s"))
        self.assertIn("slus-12345", no_crc_variants)

        bare_variants = _state_candidate_base_variants(Path("game.p2s"))
        self.assertIn("game", bare_variants)

    def test_state_candidate_matches_game_tokens_pcsx2_p2s(self) -> None:
        tokens = {"slus12345"}
        self.assertTrue(_state_candidate_matches_game_tokens(Path("SLUS-12345 (00000000).00.p2s"), tokens))
        self.assertTrue(_state_candidate_matches_game_tokens(Path("SLUS-12345.01.p2s"), tokens))
        self.assertFalse(_state_candidate_matches_game_tokens(Path("SLUS-99999 (00000000).00.p2s"), tokens))


if __name__ == "__main__":
    unittest.main()
