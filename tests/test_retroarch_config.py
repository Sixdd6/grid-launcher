from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from grid_launcher.emulator.retroarch import (
    ensure_retroarch_save_location_settings,
    installed_retroarch_core_ids,
    load_retroarch_slug_core_map,
    retroarch_core_flags,
    retroarch_core_id_from_file_name,
    retroarch_cores_for_slug,
    retroarch_directory_settings,
    retroarch_slug_core_map_path,
    retroarch_system_keys_for_platform,
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


class TestRetroarchCoreIdFromFileName(unittest.TestCase):
    """Tests for retroarch_core_id_from_file_name()."""

    def test_strips_so_extension(self):
        self.assertEqual(retroarch_core_id_from_file_name("snes9x_libretro.so"), "snes9x")

    def test_strips_dylib_extension(self):
        self.assertEqual(retroarch_core_id_from_file_name("snes9x_libretro.dylib"), "snes9x")

    def test_strips_dll_regression(self):
        self.assertEqual(retroarch_core_id_from_file_name("snes9x_libretro.dll"), "snes9x")

    def test_strips_uppercase_extension(self):
        # The implementation casefolds the file name, so extensions are matched
        # case-insensitively.
        self.assertEqual(retroarch_core_id_from_file_name("snes9x_libretro.SO"), "snes9x")


class TestInstalledRetroarchCoreIds(unittest.TestCase):
    """Tests for installed_retroarch_core_ids()."""

    def test_returns_empty_when_path_empty(self):
        self.assertEqual(installed_retroarch_core_ids("", cores_dir=None), set())

    def test_returns_empty_when_cores_dir_missing(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "retroarch"
            emulator_path.touch()
            result = installed_retroarch_core_ids(str(emulator_path))
        self.assertEqual(result, set())

    def test_finds_dll_on_windows(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "retroarch.exe"
            emulator_path.touch()
            cores_dir = Path(temp_dir) / "cores"
            cores_dir.mkdir()
            (cores_dir / "snes9x_libretro.dll").touch()
            result = installed_retroarch_core_ids(str(emulator_path), _platform="win32")
        self.assertEqual(result, {"snes9x"})

    def test_finds_so_on_linux(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "retroarch"
            emulator_path.touch()
            cores_dir = Path(temp_dir) / "cores"
            cores_dir.mkdir()
            (cores_dir / "snes9x_libretro.so").touch()
            result = installed_retroarch_core_ids(str(emulator_path), _platform="linux")
        self.assertEqual(result, {"snes9x"})

    def test_finds_dylib_on_macos(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "retroarch"
            emulator_path.touch()
            cores_dir = Path(temp_dir) / "cores"
            cores_dir.mkdir()
            (cores_dir / "snes9x_libretro.dylib").touch()
            result = installed_retroarch_core_ids(str(emulator_path), _platform="darwin")
        self.assertEqual(result, {"snes9x"})

    def test_ignores_wrong_extension_on_platform(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "retroarch"
            emulator_path.touch()
            cores_dir = Path(temp_dir) / "cores"
            cores_dir.mkdir()
            (cores_dir / "snes9x_libretro.dll").touch()
            result = installed_retroarch_core_ids(str(emulator_path), _platform="linux")
        self.assertEqual(result, set())

    def test_cores_dir_override(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "retroarch"
            emulator_path.touch()
            override_dir = Path(temp_dir) / "flatpak_cores"
            override_dir.mkdir()
            (override_dir / "snes9x_libretro.so").touch()
            result = installed_retroarch_core_ids(
                str(emulator_path), cores_dir=override_dir, _platform="linux"
            )
        self.assertEqual(result, {"snes9x"})

    def test_cores_dir_override_ignores_invalid_emulator_path(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            override_dir = Path(temp_dir) / "flatpak_cores"
            override_dir.mkdir()
            (override_dir / "snes9x_libretro.so").touch()
            result = installed_retroarch_core_ids(
                "", cores_dir=override_dir, _platform="linux"
            )
        self.assertEqual(result, {"snes9x"})

    def test_finds_cores_in_appimage_home_directory(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            appimage_path = Path(temp_dir) / "RetroArch-Linux-x86_64.AppImage"
            appimage_path.touch()
            cores_dir = (
                Path(temp_dir)
                / "RetroArch-Linux-x86_64.AppImage.home"
                / ".config"
                / "retroarch"
                / "cores"
            )
            cores_dir.mkdir(parents=True)
            (cores_dir / "snes9x_libretro.so").touch()
            result = installed_retroarch_core_ids(str(appimage_path), _platform="linux")
        self.assertEqual(result, {"snes9x"})

    def test_appimage_home_takes_priority_over_sibling_cores_dir(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            appimage_path = Path(temp_dir) / "RetroArch-Linux-x86_64.AppImage"
            appimage_path.touch()
            cores_dir = (
                Path(temp_dir)
                / "RetroArch-Linux-x86_64.AppImage.home"
                / ".config"
                / "retroarch"
                / "cores"
            )
            cores_dir.mkdir(parents=True)
            (cores_dir / "snes9x_libretro.so").touch()
            sibling_cores = Path(temp_dir) / "cores"
            sibling_cores.mkdir()
            (sibling_cores / "other_libretro.so").touch()
            result = installed_retroarch_core_ids(str(appimage_path), _platform="linux")
        self.assertEqual(result, {"snes9x"})

    def test_falls_back_to_sibling_cores_when_no_appimage_home(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            appimage_path = Path(temp_dir) / "RetroArch-Linux-x86_64.AppImage"
            appimage_path.touch()
            sibling_cores = Path(temp_dir) / "cores"
            sibling_cores.mkdir()
            (sibling_cores / "snes9x_libretro.so").touch()
            result = installed_retroarch_core_ids(str(appimage_path), _platform="linux")
        self.assertEqual(result, {"snes9x"})


class TestRetroarchSystemKeysForPlatform(unittest.TestCase):
    """Tests for retroarch_system_keys_for_platform() token-overlap matching."""

    def test_exact_match_still_works(self):
        compatibility = {"game boy advance": ["mgba"]}
        result = retroarch_system_keys_for_platform("Game Boy Advance", compatibility)
        self.assertEqual(result, ["game boy advance"])

    def test_manufacturer_prefix_stripped(self):
        compatibility = {"game boy advance": ["mgba"]}
        result = retroarch_system_keys_for_platform(
            "Nintendo - Game Boy Advance", compatibility
        )
        self.assertEqual(result, ["game boy advance"])

    def test_no_match_below_threshold(self):
        compatibility = {"game boy advance": ["mgba"]}
        result = retroarch_system_keys_for_platform("Game", compatibility)
        self.assertEqual(result, [])

    def test_returns_empty_for_unknown_platform(self):
        compatibility = {"game boy advance": ["mgba"]}
        result = retroarch_system_keys_for_platform("PlayStation Portable", compatibility)
        self.assertEqual(result, [])

    def test_best_match_selected(self):
        compatibility = {
            "game boy": ["gambatte"],
            "game boy advance": ["mgba"],
        }
        result = retroarch_system_keys_for_platform(
            "Nintendo - Game Boy Advance", compatibility
        )
        self.assertEqual(result, ["game boy advance"])


class TestRetroarchSlugCoreMap(unittest.TestCase):
    def test_cores_for_slug_known_platform(self) -> None:
        slug_core_map = {"gba": ["mgba", "vba_next"]}
        result = retroarch_cores_for_slug("gba", slug_core_map)
        self.assertEqual(result, ["mgba", "vba_next"])

    def test_cores_for_slug_unknown_slug_returns_empty(self) -> None:
        self.assertEqual(retroarch_cores_for_slug("unknown", {"gba": ["mgba"]}), [])

    def test_cores_for_slug_empty_slug_returns_empty(self) -> None:
        self.assertEqual(retroarch_cores_for_slug("", {"gba": ["mgba"]}), [])

    def test_cores_for_slug_empty_map_returns_empty(self) -> None:
        self.assertEqual(retroarch_cores_for_slug("gba", {}), [])

    def test_load_slug_core_map_valid_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "romm-platform-cores.json"
            path.write_text(
                json.dumps({"snes": ["snes9x", "snes9x2010"]}),
                encoding="utf-8",
            )
            result = load_retroarch_slug_core_map(path)
        self.assertEqual(result, {"snes": ["snes9x", "snes9x2010"]})

    def test_load_slug_core_map_missing_file_returns_empty(self) -> None:
        self.assertEqual(
            load_retroarch_slug_core_map(Path("/nonexistent/romm-platform-cores.json")),
            {},
        )

    def test_load_slug_core_map_invalid_json_returns_empty(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "romm-platform-cores.json"
            path.write_text("not valid json{{{", encoding="utf-8")
            result = load_retroarch_slug_core_map(path)
        self.assertEqual(result, {})

    def test_load_slug_core_map_skips_invalid_entries(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "romm-platform-cores.json"
            path.write_text(
                json.dumps({"gba": ["mgba"], "bad": 123, "": ["x"]}),
                encoding="utf-8",
            )
            result = load_retroarch_slug_core_map(path)
        self.assertEqual(result, {"gba": ["mgba"]})


if __name__ == "__main__":
    unittest.main()
