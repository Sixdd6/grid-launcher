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

    def test_normalizes_basic_entry(self) -> None:
        entry = self._normalize_entry(
            {
                "name": "PPSSPP",
                "path": "/usr/bin/ppsspp",
                "args": "%rom%",
            }
        )
        self.assertEqual(entry["name"], "PPSSPP")
        self.assertEqual(entry["path"], "/usr/bin/ppsspp")
        self.assertEqual(entry["args"], "%rom%")


if __name__ == "__main__":
    unittest.main()
