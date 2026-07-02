import os
import unittest
from pathlib import Path
from unittest.mock import patch

from grid_launcher.core.config import normalize_emulators
from grid_launcher.core.path import grid_launcher_share_dir, xdg_config_home, xdg_data_home


class CorePathXdgTests(unittest.TestCase):
    def test_xdg_config_home_uses_env_var(self) -> None:
        with patch.dict(os.environ, {"XDG_CONFIG_HOME": "/tmp/somecfg"}, clear=False):
            self.assertEqual(xdg_config_home(), Path("/tmp/somecfg"))

    def test_xdg_config_home_falls_back_when_unset(self) -> None:
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("XDG_CONFIG_HOME", None)
            self.assertEqual(xdg_config_home(), Path.home() / ".config")

    def test_xdg_config_home_falls_back_when_empty(self) -> None:
        with patch.dict(os.environ, {"XDG_CONFIG_HOME": ""}, clear=False):
            self.assertEqual(xdg_config_home(), Path.home() / ".config")

    def test_xdg_data_home_uses_env_var(self) -> None:
        with patch.dict(os.environ, {"XDG_DATA_HOME": "/tmp/somedata"}, clear=False):
            self.assertEqual(xdg_data_home(), Path("/tmp/somedata"))

    def test_xdg_data_home_falls_back_when_unset(self) -> None:
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("XDG_DATA_HOME", None)
            self.assertEqual(xdg_data_home(), Path.home() / ".local" / "share")

    def test_xdg_data_home_falls_back_when_empty(self) -> None:
        with patch.dict(os.environ, {"XDG_DATA_HOME": ""}, clear=False):
            self.assertEqual(xdg_data_home(), Path.home() / ".local" / "share")


class CorePathGridLauncherShareDirTests(unittest.TestCase):
    def test_grid_launcher_share_dir_uses_env_var(self) -> None:
        with patch.dict(os.environ, {"GRID_LAUNCHER_SHARE_DIR": "/app/share/grid-launcher"}, clear=False):
            self.assertEqual(
                grid_launcher_share_dir(Path("/fallback")), Path("/app/share/grid-launcher")
            )

    def test_grid_launcher_share_dir_falls_back_when_unset(self) -> None:
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("GRID_LAUNCHER_SHARE_DIR", None)
            self.assertEqual(grid_launcher_share_dir(Path("/fallback")), Path("/fallback"))

    def test_grid_launcher_share_dir_falls_back_when_empty(self) -> None:
        with patch.dict(os.environ, {"GRID_LAUNCHER_SHARE_DIR": ""}, clear=False):
            self.assertEqual(grid_launcher_share_dir(Path("/fallback")), Path("/fallback"))


class NormalizeEmulatorsTests(unittest.TestCase):
    @staticmethod
    def _normalize_entry(entry: dict) -> dict:
        result = normalize_emulators([entry], lambda value: value)
        return result[0]

    def test_preserves_flatpak_app_id(self) -> None:
        entry = self._normalize_entry(
            {
                "name": "PPSSPP",
                "path": "/usr/bin/ppsspp",
                "args": "%rom%",
                "flatpak_app_id": "org.ppsspp.PPSSPP",
            }
        )
        self.assertEqual(entry["flatpak_app_id"], "org.ppsspp.PPSSPP")

    def test_preserves_flatpak_cores_dir(self) -> None:
        cores_dir = "~/.var/app/org.libretro.RetroArch/config/retroarch/cores"
        entry = self._normalize_entry(
            {
                "name": "RetroArch",
                "path": "/usr/bin/retroarch",
                "args": "%rom%",
                "flatpak_cores_dir": cores_dir,
            }
        )
        self.assertEqual(entry["flatpak_cores_dir"], cores_dir)

    def test_omits_flatpak_app_id_when_empty(self) -> None:
        entry = self._normalize_entry(
            {
                "name": "PPSSPP",
                "path": "/usr/bin/ppsspp",
                "args": "%rom%",
                "flatpak_app_id": "",
            }
        )
        self.assertNotIn("flatpak_app_id", entry)

    def test_omits_flatpak_cores_dir_when_empty(self) -> None:
        entry = self._normalize_entry(
            {
                "name": "RetroArch",
                "path": "/usr/bin/retroarch",
                "args": "%rom%",
                "flatpak_cores_dir": "",
            }
        )
        self.assertNotIn("flatpak_cores_dir", entry)

    def test_omits_flatpak_fields_when_absent(self) -> None:
        entry = self._normalize_entry(
            {
                "name": "PPSSPP",
                "path": "/usr/bin/ppsspp",
                "args": "%rom%",
            }
        )
        self.assertNotIn("flatpak_app_id", entry)
        self.assertNotIn("flatpak_cores_dir", entry)


if __name__ == "__main__":
    unittest.main()
