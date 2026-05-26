from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from rom_mate.emulator.launch import retroarch_core_argument_path
from rom_mate.emulator.retroarch import (
    ensure_retroarch_save_location_settings,
    retroarch_core_flags,
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

    def test_ensure_retroarch_save_location_settings_writes_cheevos_leaderboard_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            retroarch_dir = Path(temp_dir) / "RetroArch"
            retroarch_dir.mkdir()
            emulator_path = retroarch_dir / "retroarch.exe"
            emulator_path.write_bytes(b"")
            config_path = retroarch_dir / "retroarch.cfg"
            config_path.write_text("", encoding="utf-8")

            result = ensure_retroarch_save_location_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn('cheevos_hardcore_mode_enable = "false"', text)
        self.assertIn('cheevos_visibility_lboard_start = "false"', text)
        self.assertIn('cheevos_visibility_lboard_submit = "false"', text)
        self.assertIn('cheevos_visibility_lboard_trackers = "false"', text)


class TestRetroarchCoreFlags(unittest.TestCase):
    """Tests for retroarch_core_flags()."""

    _SNES_ENTRY = {
        "core_file": "snes9x_libretro.dll",
        "platforms": ["Nintendo SNES"],
    }
    _MAME_ENTRY = {
        "core_file": "mame_current_libretro.dll",
        "platforms": ["Arcade"],
        "supports_save_states": False,
        "supports_saves": False,
        "cloud_sync_safe": False,
    }
    _FBNEO_ENTRY = {
        "core_file": "fbneo_libretro.dll",
        "platforms": ["Arcade"],
        "cloud_sync_safe": False,
    }

    def test_all_true_defaults_when_no_flags_set(self):
        result = retroarch_core_flags("snes9x", [self._SNES_ENTRY])
        self.assertTrue(result["supports_save_states"])
        self.assertTrue(result["supports_saves"])
        self.assertTrue(result["cloud_sync_safe"])

    def test_explicit_false_values_returned(self):
        result = retroarch_core_flags("mame_current", [self._MAME_ENTRY])
        self.assertFalse(result["supports_save_states"])
        self.assertFalse(result["supports_saves"])
        self.assertFalse(result["cloud_sync_safe"])

    def test_partial_flags_merged_with_defaults(self):
        # fbneo only has cloud_sync_safe: False; other flags default to True
        result = retroarch_core_flags("fbneo", [self._FBNEO_ENTRY])
        self.assertTrue(result["supports_save_states"])
        self.assertTrue(result["supports_saves"])
        self.assertFalse(result["cloud_sync_safe"])

    def test_all_true_when_core_not_found(self):
        result = retroarch_core_flags("unknown_core", [self._SNES_ENTRY])
        self.assertTrue(result["supports_save_states"])
        self.assertTrue(result["supports_saves"])
        self.assertTrue(result["cloud_sync_safe"])

    def test_all_true_for_empty_entries(self):
        result = retroarch_core_flags("snes9x", [])
        self.assertTrue(result["supports_save_states"])
        self.assertTrue(result["supports_saves"])
        self.assertTrue(result["cloud_sync_safe"])

    def test_skips_non_dict_entries(self):
        entries = ["not_a_dict", None, self._MAME_ENTRY]
        result = retroarch_core_flags("mame_current", entries)
        self.assertFalse(result["cloud_sync_safe"])

    def test_returns_dict_with_exactly_four_keys(self):
        result = retroarch_core_flags("snes9x", [self._SNES_ENTRY])
        self.assertEqual(
            set(result.keys()),
            {"supports_save_states", "supports_saves", "cloud_sync_safe", "vmu_shared_saves"},
        )


class RetroarchCoreArgumentPathTests(unittest.TestCase):
    def test_core_argument_path_linux_so(self) -> None:
        with patch("rom_mate.emulator.launch.sys.platform", "linux"):
            self.assertEqual("cores/snes9x_libretro.so", retroarch_core_argument_path("snes9x"))

    def test_core_argument_path_macos_dylib(self) -> None:
        with patch("rom_mate.emulator.launch.sys.platform", "darwin"):
            self.assertEqual("cores/snes9x_libretro.dylib", retroarch_core_argument_path("snes9x"))

if __name__ == "__main__":
    unittest.main()
