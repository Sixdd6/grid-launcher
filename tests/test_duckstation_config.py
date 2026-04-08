from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from rom_mate.emulator.duckstation import (
    duckstation_memory_card_settings,
    ensure_duckstation_memory_card_settings,
)


class DuckStationConfigTests(unittest.TestCase):
    def test_ensure_duckstation_memory_card_settings_forces_per_game_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "\n".join(
                    [
                        "[MemoryCards]",
                        "Card1Type = Shared",
                        "Card2Type = Shared",
                        "UsePlaylistTitle = false",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            settings = duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertEqual(settings["card1_type"], "PerGameTitle")
        self.assertEqual(settings["card2_type"], "None")
        self.assertTrue(settings["use_playlist_title"])
        self.assertEqual(settings["directory"], "memcards")
        self.assertIn("[MemoryCards]", text)
        self.assertIn("Card1Type = PerGameTitle", text)
        self.assertIn("Card2Type = None", text)
        self.assertIn("UsePlaylistTitle = true", text)
        self.assertIn("Directory = memcards", text)

    def test_ensure_duckstation_memory_card_settings_preserves_explicit_memcard_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "\n".join(
                    [
                        "[MemoryCards]",
                        "Directory = D:/CustomMemcards",
                        "Card1Type = Shared",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            settings = duckstation_memory_card_settings(str(emulator_path))

        self.assertTrue(result["changed"])
        self.assertEqual(settings["directory"], "D:/CustomMemcards")
        self.assertEqual(settings["card1_type"], "PerGameTitle")
        self.assertEqual(settings["card2_type"], "None")


if __name__ == "__main__":
    unittest.main()
