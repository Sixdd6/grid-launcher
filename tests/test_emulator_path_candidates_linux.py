from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from rom_mate.emulator.azahar import azahar_config_path_candidates
from rom_mate.emulator.cemu import cemu_settings_path_candidates
from rom_mate.emulator.dolphin import dolphin_user_root_candidates
from rom_mate.emulator.duckstation import duckstation_config_path_candidates
from rom_mate.emulator.eden import eden_config_path_candidates
from rom_mate.emulator.pcsx2 import pcsx2_config_path_candidates
from rom_mate.emulator.profiles import is_available_on_current_platform
from rom_mate.emulator.xemu import xemu_config_path_candidates


class EmulatorPathCandidatesLinuxTests(unittest.TestCase):
    def test_azahar_config_candidates_include_xdg_and_exclude_appdata_token(self) -> None:
        temp_root = Path(tempfile.mkdtemp())
        xdg_config = temp_root / "config"
        xdg_data = temp_root / "data"

        with patch("sys.platform", "linux"):
            with patch.dict(
                os.environ,
                {
                    "XDG_CONFIG_HOME": str(xdg_config),
                    "XDG_DATA_HOME": str(xdg_data),
                },
                clear=False,
            ):
                candidates = azahar_config_path_candidates("/tmp/azahar")

        self.assertIn(
            Path.home() / ".var" / "app" / "org.azahar_emu.Azahar" / "data" / "azahar-emu" / "qt-config.ini",
            candidates,
        )
        self.assertIn(xdg_config / "azahar-emu" / "qt-config.ini", candidates)
        for candidate in candidates:
            self.assertNotIn("%APPDATA%", str(candidate))

    def test_eden_config_candidates_include_xdg_and_exclude_appdata_token(self) -> None:
        temp_root = Path(tempfile.mkdtemp())
        xdg_config = temp_root / "config"
        xdg_data = temp_root / "data"

        with patch("sys.platform", "linux"):
            with patch.dict(
                os.environ,
                {
                    "XDG_CONFIG_HOME": str(xdg_config),
                    "XDG_DATA_HOME": str(xdg_data),
                },
                clear=False,
            ):
                candidates = eden_config_path_candidates("/tmp/eden")

        self.assertIn(xdg_config / "eden" / "qt-config.ini", candidates)
        for candidate in candidates:
            self.assertNotIn("%APPDATA%", str(candidate))

    def test_dolphin_user_root_candidates_include_xdg_and_flatpak_paths(self) -> None:
        temp_root = Path(tempfile.mkdtemp())
        xdg_config = temp_root / "config"
        xdg_data = temp_root / "data"

        with patch("sys.platform", "linux"):
            with patch.dict(
                os.environ,
                {
                    "XDG_CONFIG_HOME": str(xdg_config),
                    "XDG_DATA_HOME": str(xdg_data),
                },
                clear=False,
            ):
                candidates = dolphin_user_root_candidates("", "", lambda _: [])

        self.assertIn((xdg_data / "dolphin-emu").resolve(), candidates)
        self.assertIn(
            (Path.home() / ".var" / "app" / "org.DolphinEmu.dolphin-emu" / "data" / "dolphin-emu").resolve(),
            candidates,
        )

    def test_pcsx2_config_candidates_include_flatpak_path(self) -> None:
        temp_root = Path(tempfile.mkdtemp())
        xdg_config = temp_root / "config"
        xdg_data = temp_root / "data"

        with patch("sys.platform", "linux"):
            with patch.dict(
                os.environ,
                {
                    "XDG_CONFIG_HOME": str(xdg_config),
                    "XDG_DATA_HOME": str(xdg_data),
                },
                clear=False,
            ):
                candidates = pcsx2_config_path_candidates("")

        self.assertIn(
            Path.home() / ".var" / "app" / "net.pcsx2.PCSX2" / "config" / "PCSX2" / "inis" / "PCSX2.ini",
            candidates,
        )

    def test_duckstation_config_candidates_include_flatpak_path(self) -> None:
        temp_root = Path(tempfile.mkdtemp())
        xdg_config = temp_root / "config"
        xdg_data = temp_root / "data"

        with patch("sys.platform", "linux"):
            with patch.dict(
                os.environ,
                {
                    "XDG_CONFIG_HOME": str(xdg_config),
                    "XDG_DATA_HOME": str(xdg_data),
                },
                clear=False,
            ):
                candidates = duckstation_config_path_candidates("/tmp/duckstation")

        self.assertIn(
            Path.home() / ".var" / "app" / "org.duckstation.DuckStation" / "config" / "duckstation" / "settings.ini",
            candidates,
        )

    def test_xemu_config_candidates_include_flatpak_path(self) -> None:
        temp_root = Path(tempfile.mkdtemp())
        xdg_config = temp_root / "config"
        xdg_data = temp_root / "data"

        with patch("sys.platform", "linux"):
            with patch.dict(
                os.environ,
                {
                    "XDG_CONFIG_HOME": str(xdg_config),
                    "XDG_DATA_HOME": str(xdg_data),
                },
                clear=False,
            ):
                candidates = xemu_config_path_candidates("", "", lambda _: [])

        self.assertIn(
            Path.home() / ".var" / "app" / "app.xemu.xemu" / "data" / "xemu" / "xemu" / "xemu.toml",
            candidates,
        )

    def test_cemu_config_candidates_include_xdg_and_flatpak_and_exclude_appdata_strings(self) -> None:
        temp_root = Path(tempfile.mkdtemp())
        xdg_config = temp_root / "config"
        xdg_data = temp_root / "data"

        with patch("sys.platform", "linux"):
            with patch.dict(
                os.environ,
                {
                    "XDG_CONFIG_HOME": str(xdg_config),
                    "XDG_DATA_HOME": str(xdg_data),
                },
                clear=False,
            ):
                candidates = cemu_settings_path_candidates("")

        self.assertIn(xdg_config / "Cemu" / "settings.xml", candidates)
        self.assertIn(
            Path.home() / ".var" / "app" / "info.cemu.Cemu" / "config" / "Cemu" / "settings.xml",
            candidates,
        )
        for candidate in candidates:
            self.assertNotIn("APPDATA", str(candidate).upper())

    def test_is_available_on_current_platform_handles_xenia_by_platform(self) -> None:
        with patch("sys.platform", "linux"):
            self.assertFalse(is_available_on_current_platform("xenia"))
            self.assertTrue(is_available_on_current_platform("dolphin"))

        with patch("sys.platform", "win32"):
            self.assertTrue(is_available_on_current_platform("xenia"))


if __name__ == "__main__":
    unittest.main()
