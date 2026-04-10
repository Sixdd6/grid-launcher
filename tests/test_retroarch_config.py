from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from rom_mate.emulator.retroarch import (
    ensure_retroarch_save_location_settings,
    retroarch_directory_settings,
)


class RetroArchConfigTests(unittest.TestCase):
    def test_ensure_retroarch_save_location_settings_disables_sorting_and_sets_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            retroarch_dir = Path(temp_dir) / "RetroArch"
            retroarch_dir.mkdir()
            emulator_path = retroarch_dir / "retroarch.exe"
            emulator_path.write_bytes(b"")
            config_path = retroarch_dir / "retroarch.cfg"
            config_path.write_text(
                "\n".join(
                    [
                        'savefile_directory = "default"',
                        'savestate_directory = "default"',
                        'sort_savefiles_enable = "true"',
                        'sort_savestates_enable = "true"',
                        'sort_savefiles_by_content_enable = "true"',
                        'sort_savestates_by_content_enable = "true"',
                        'savefiles_in_content_dir = "true"',
                        'savestates_in_content_dir = "true"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_retroarch_save_location_settings(str(emulator_path))
            settings = retroarch_directory_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertEqual(settings["savefile_directory"], "saves")
        self.assertEqual(settings["savestate_directory"], "states")
        self.assertFalse(settings["sort_savefiles_enable"])
        self.assertFalse(settings["sort_savestates_enable"])
        self.assertFalse(settings["sort_savefiles_by_content_enable"])
        self.assertFalse(settings["sort_savestates_by_content_enable"])
        self.assertFalse(settings["savefiles_in_content_dir"])
        self.assertFalse(settings["savestates_in_content_dir"])
        self.assertIn('savefile_directory = "saves"', text)
        self.assertIn('savestate_directory = "states"', text)

    def test_ensure_retroarch_save_location_settings_preserves_explicit_directories(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            retroarch_dir = Path(temp_dir) / "RetroArch"
            retroarch_dir.mkdir()
            emulator_path = retroarch_dir / "retroarch.exe"
            emulator_path.write_bytes(b"")
            config_path = retroarch_dir / "retroarch.cfg"
            config_path.write_text(
                "\n".join(
                    [
                        'savefile_directory = "D:/Custom Saves"',
                        'savestate_directory = "E:/Custom States"',
                        'sort_savefiles_enable = "true"',
                        'savestates_in_content_dir = "true"',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_retroarch_save_location_settings(str(emulator_path))
            settings = retroarch_directory_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertEqual(settings["savefile_directory"], "D:/Custom Saves")
        self.assertEqual(settings["savestate_directory"], "E:/Custom States")
        self.assertFalse(settings["sort_savefiles_enable"])
        self.assertFalse(settings["savestates_in_content_dir"])
        self.assertIn('savefile_directory = "D:/Custom Saves"', text)
        self.assertIn('savestate_directory = "E:/Custom States"', text)

    def test_ensure_retroarch_save_location_settings_enables_fullscreen_and_retroachievements(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            retroarch_dir = Path(temp_dir) / "RetroArch"
            retroarch_dir.mkdir()
            emulator_path = retroarch_dir / "retroarch.exe"
            emulator_path.write_bytes(b"")
            config_path = retroarch_dir / "retroarch.cfg"
            config_path.write_text('video_fullscreen = "false"\ncheevos_enable = "false"\n', encoding="utf-8")

            result = ensure_retroarch_save_location_settings(
                str(emulator_path),
                enable_fullscreen=True,
                retroachievements_username="retro_user",
                retroachievements_token="retro_token",
            )
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn('video_fullscreen = "true"', text)
        self.assertIn('cheevos_enable = "true"', text)
        self.assertIn('cheevos_username = "retro_user"', text)
        self.assertIn('cheevos_token = "retro_token"', text)

if __name__ == "__main__":
    unittest.main()
