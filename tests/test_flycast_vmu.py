from __future__ import annotations

import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path

from rom_mate.emulator.retroarch import flycast_vmu_file_candidates
from rom_mate.emulator.retroarch import retroarch_core_flags
from rom_mate.emulator.retroarch import retroarch_core_flags_for_platform
from rom_mate.emulator.selection import cloud_save_scope_for_game


def _is_retroarch(name: str) -> bool:
    return "retroarch" in name.casefold()


def _is_not_retroarch(name: str) -> bool:
    del name
    return False


class FlycastVmuTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_flycast_vmu_tests", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load rom-mate.py for tests.")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        cls.module = module

    def _core_entries(self) -> list[dict]:
        root = Path(__file__).resolve().parent.parent
        entries_path = root / "retroarch-core-list.json"
        return json.loads(entries_path.read_text(encoding="utf-8"))

    def test_retroarch_core_flags_flycast_returns_vmu_shared_saves_true(self):
        result = retroarch_core_flags("flycast", self._core_entries())
        self.assertTrue(result["vmu_shared_saves"])

    def test_retroarch_core_flags_other_core_returns_vmu_shared_saves_false(self):
        result = retroarch_core_flags("snes9x", self._core_entries())
        self.assertFalse(result["vmu_shared_saves"])

    def test_flycast_vmu_file_candidates_finds_timestamped_files(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            vmu0 = root / "vmu0 [2026-04-11_13-16-11].bin"
            vmu1 = root / "vmu1.bin"
            vmu0.write_bytes(b"slot0")
            vmu1.write_bytes(b"slot1")

            result = flycast_vmu_file_candidates([root])

            self.assertEqual(set(result), {vmu0, vmu1})

    def test_flycast_vmu_file_candidates_deduplicates_to_newest_per_slot(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            newer = root / "vmu0 [2026-04-11].bin"
            older = root / "vmu0 [2026-04-10].bin"
            newer.write_bytes(b"new")
            older.write_bytes(b"old")

            old_mtime = 1_000_000
            new_mtime = old_mtime + 60
            os.utime(older, (old_mtime, old_mtime))
            os.utime(newer, (new_mtime, new_mtime))

            result = flycast_vmu_file_candidates([root])

            self.assertEqual(result, [newer])

    def test_flycast_vmu_file_candidates_ignores_non_vmu_files(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            (root / "game.srm").write_bytes(b"save")
            (root / "Sonic Adventure.srm").write_bytes(b"save")

            result = flycast_vmu_file_candidates([root])

            self.assertEqual(result, [])

    def test_flycast_vmu_file_candidates_returns_empty_for_missing_dir(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            missing = Path(temp_dir) / "does-not-exist"
            self.assertEqual(flycast_vmu_file_candidates([missing]), [])

    def test_cloud_save_scope_returns_shared_slotted_for_flycast_retroarch(self):
        result = cloud_save_scope_for_game(
            {"platform": "Dreamcast", "title": "Sonic Adventure"},
            emulator_name="RetroArch",
            is_retroarch_emulator_name=_is_retroarch,
            retroarch_core_flags={"vmu_shared_saves": True},
            save_type="save",
        )
        self.assertEqual(result, "shared-slotted")

    def test_cloud_save_scope_returns_per_game_for_state_type_even_with_vmu_flag(self):
        result = cloud_save_scope_for_game(
            {"platform": "Dreamcast", "title": "Sonic Adventure"},
            emulator_name="RetroArch",
            is_retroarch_emulator_name=_is_retroarch,
            retroarch_core_flags={"vmu_shared_saves": True},
            save_type="state",
        )
        self.assertEqual(result, "per-game")

    def test_cloud_save_scope_returns_per_game_for_non_retroarch_with_vmu_flag(self):
        result = cloud_save_scope_for_game(
            {"platform": "Dreamcast", "title": "Sonic Adventure"},
            emulator_name="Flycast",
            is_retroarch_emulator_name=_is_not_retroarch,
            retroarch_core_flags={"vmu_shared_saves": True},
            save_type="save",
        )
        self.assertEqual(result, "per-game")

    def test_retroarch_core_flags_for_platform_finds_flycast_by_dreamcast(self):
        result = retroarch_core_flags_for_platform("Sega Dreamcast", self._core_entries())
        self.assertIsInstance(result, dict)
        if result is not None:
            self.assertTrue(result["vmu_shared_saves"])

    def test_retroarch_core_flags_for_platform_finds_flycast_case_insensitive(self):
        result = retroarch_core_flags_for_platform("sega dreamcast", self._core_entries())
        self.assertIsInstance(result, dict)
        if result is not None:
            self.assertTrue(result["vmu_shared_saves"])

    def test_retroarch_core_flags_for_platform_returns_none_for_unknown_platform(self):
        self.assertIsNone(
            retroarch_core_flags_for_platform("Unknown System", self._core_entries())
        )

    def test_cloud_save_scope_shared_slotted_without_default_cores_config(self):
        class _WindowStub:
            config = {"default_retroarch_cores": {}}

            @staticmethod
            def _is_retroarch_emulator_name(name: str, emulator: dict[str, str] | None = None) -> bool:
                del emulator
                return "retroarch" in name.casefold()

            @staticmethod
            def _normalize_default_retroarch_cores(values: object) -> dict[str, str]:
                return values if isinstance(values, dict) else {}

            @staticmethod
            def _mapping_value_for_platform(mapping: dict[str, str], platform: str) -> str:
                del mapping
                del platform
                return ""

            def _retroarch_core_list_entries(self) -> list[dict]:
                return FlycastVmuTests._core_entries(self)

            @staticmethod
            def _default_emulator_name_for_platform(platform: str) -> str:
                del platform
                return ""

            @staticmethod
            def _is_xemu_emulator_name(name: str, emulator: dict[str, str] | None = None) -> bool:
                del name
                del emulator
                return False

            @staticmethod
            def _is_redream_emulator_name(name: str, emulator: dict[str, str] | None = None) -> bool:
                del name
                del emulator
                return False

        scope = self.module.MainWindow._cloud_save_scope_for_game(
            _WindowStub(),
            {"platform": "Sega Dreamcast", "title": "Sonic Adventure"},
            emulator_name="RetroArch",
            emulator={"name": "RetroArch"},
            save_type="save",
        )
        self.assertEqual(scope, "shared-slotted")


if __name__ == "__main__":
    unittest.main()
