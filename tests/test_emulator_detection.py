from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from rom_mate.emulator.detection import (
    detect_installed_flatpak_emulators,
    installed_flatpak_app_ids,
)


class InstalledFlatpakAppIdsTests(unittest.TestCase):
    def test_returns_empty_set_on_non_linux(self) -> None:
        with patch("sys.platform", "win32"):
            self.assertEqual(installed_flatpak_app_ids(), set())

    def test_reads_user_and_system_flatpak_app_dirs(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            data_home = Path(temp_dir) / "data-home"
            user_app_dir = data_home / "flatpak" / "app"
            user_app_dir.mkdir(parents=True)
            (user_app_dir / "org.azahar_emu.Azahar").mkdir()
            (user_app_dir / "org.DolphinEmu.dolphin-emu").mkdir()
            # A stray file (not a directory) must be ignored.
            (user_app_dir / "not-an-app.txt").write_text("", encoding="utf-8")

            with patch("sys.platform", "linux"), patch(
                "rom_mate.emulator.detection.xdg_data_home", return_value=data_home
            ):
                result = installed_flatpak_app_ids()

            self.assertEqual(
                result,
                {"org.azahar_emu.Azahar", "org.DolphinEmu.dolphin-emu"},
            )

    def test_missing_flatpak_dirs_do_not_raise(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            data_home = Path(temp_dir) / "nonexistent-data-home"

            with patch("sys.platform", "linux"), patch(
                "rom_mate.emulator.detection.xdg_data_home", return_value=data_home
            ):
                result = installed_flatpak_app_ids()

            self.assertEqual(result, set())


class DetectInstalledFlatpakEmulatorsTests(unittest.TestCase):
    def _autoprofiles(self) -> list[dict]:
        return [
            {
                "name": "Azahar (Nintendo 3DS)",
                "flatpak_app_id": "org.azahar_emu.Azahar",
                "args": '-f "%rom%"',
            },
            {
                "name": "PPSSPP (Playstation Portable)",
                "flatpak_app_id": "org.ppsspp.PPSSPP",
                "args": '--fullscreen --pause-menu-exit "%rom%"',
            },
            {
                "name": "MAME (Arcade)",
                "flatpak_app_id": "org.mamedev.MAME",
                "args": '"%rom%"',
            },
            {
                "name": "Pico-8",
                "flatpak_app_id": "",
                "args": '"%rom%"',
            },
            {
                # Name coincidentally matches an installed app id string, but
                # has no flatpak_app_id at all -- must never match.
                "name": "org.uninstalled.NotReal",
                "args": '"%rom%"',
            },
        ]

    def test_returns_empty_list_on_non_linux(self) -> None:
        with patch("sys.platform", "win32"):
            result = detect_installed_flatpak_emulators(self._autoprofiles())
        self.assertEqual(result, [])

    def test_returns_empty_list_when_flatpak_binary_missing(self) -> None:
        with patch("sys.platform", "linux"), patch(
            "rom_mate.emulator.detection.shutil.which", return_value=None
        ), patch(
            "rom_mate.emulator.detection.installed_flatpak_app_ids",
            return_value={"org.azahar_emu.Azahar"},
        ):
            result = detect_installed_flatpak_emulators(self._autoprofiles())
        self.assertEqual(result, [])

    def test_matches_and_builds_entries(self) -> None:
        installed = {
            "org.azahar_emu.Azahar",
            "org.ppsspp.PPSSPP",
            "org.mamedev.MAME",
            "org.uninstalled.NotReal",
        }
        with patch("sys.platform", "linux"), patch(
            "rom_mate.emulator.detection.shutil.which", return_value="/usr/bin/flatpak"
        ), patch(
            "rom_mate.emulator.detection.installed_flatpak_app_ids",
            return_value=installed,
        ):
            result = detect_installed_flatpak_emulators(self._autoprofiles())

        by_name = {entry["name"]: entry for entry in result}

        self.assertEqual(len(result), 3)
        self.assertIn("Azahar (Nintendo 3DS)", by_name)
        self.assertIn("PPSSPP (Playstation Portable)", by_name)
        self.assertIn("MAME (Arcade)", by_name)
        self.assertNotIn("Pico-8", by_name)
        self.assertNotIn("org.uninstalled.NotReal", by_name)

        azahar_entry = by_name["Azahar (Nintendo 3DS)"]
        self.assertEqual(azahar_entry["path"], "/usr/bin/flatpak")
        self.assertEqual(azahar_entry["flatpak_app_id"], "org.azahar_emu.Azahar")
        self.assertEqual(azahar_entry["args"], 'run org.azahar_emu.Azahar -f "%rom%"')
        self.assertNotIn("_flatpak_config_root", azahar_entry)

        ppsspp_entry = by_name["PPSSPP (Playstation Portable)"]
        self.assertEqual(
            ppsspp_entry["args"],
            'run org.ppsspp.PPSSPP --fullscreen --pause-menu-exit "%rom%"',
        )
        self.assertIn("_flatpak_config_root", ppsspp_entry)
        self.assertTrue(
            ppsspp_entry["_flatpak_config_root"].endswith(
                str(Path(".var") / "app" / "org.ppsspp.PPSSPP" / "config" / "ppsspp")
            )
        )

        mame_entry = by_name["MAME (Arcade)"]
        self.assertEqual(mame_entry["args"], 'run org.mamedev.MAME "%rom%"')
        self.assertIn("_flatpak_config_root", mame_entry)
        self.assertTrue(
            mame_entry["_flatpak_config_root"].endswith(
                str(Path(".var") / "app" / "org.mamedev.MAME" / "config" / "mame")
            )
        )

    def test_skips_profiles_with_no_flatpak_app_id(self) -> None:
        installed = {"Pico-8", "org.uninstalled.NotReal"}
        with patch("sys.platform", "linux"), patch(
            "rom_mate.emulator.detection.shutil.which", return_value="/usr/bin/flatpak"
        ), patch(
            "rom_mate.emulator.detection.installed_flatpak_app_ids",
            return_value=installed,
        ):
            result = detect_installed_flatpak_emulators(self._autoprofiles())
        self.assertEqual(result, [])


if __name__ == "__main__":
    unittest.main()
