import os
import unittest
from pathlib import Path
from unittest.mock import patch

from rom_mate.core.path import rom_mate_share_dir, xdg_config_home, xdg_data_home


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


class CorePathRomMateShareDirTests(unittest.TestCase):
    def test_rom_mate_share_dir_uses_env_var(self) -> None:
        with patch.dict(os.environ, {"ROM_MATE_SHARE_DIR": "/app/share/rom-mate-neo"}, clear=False):
            self.assertEqual(
                rom_mate_share_dir(Path("/fallback")), Path("/app/share/rom-mate-neo")
            )

    def test_rom_mate_share_dir_falls_back_when_unset(self) -> None:
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("ROM_MATE_SHARE_DIR", None)
            self.assertEqual(rom_mate_share_dir(Path("/fallback")), Path("/fallback"))

    def test_rom_mate_share_dir_falls_back_when_empty(self) -> None:
        with patch.dict(os.environ, {"ROM_MATE_SHARE_DIR": ""}, clear=False):
            self.assertEqual(rom_mate_share_dir(Path("/fallback")), Path("/fallback"))


if __name__ == "__main__":
    unittest.main()
