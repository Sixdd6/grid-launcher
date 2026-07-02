import unittest
from pathlib import Path
from unittest.mock import patch

from rom_mate.emulator.launch import prepare_native_launch_command, detect_umu_run


def _resolve_executable(game):
    return Path("/games/mygame/game.exe")


def _split_args(value):
    return []


def _game(**overrides):
    game = {
        "native_launch_parameters": "",
        "native_compat_tool": "",
        "native_wineprefix": "",
    }
    game.update(overrides)
    return game


class CompatToolLaunchTests(unittest.TestCase):
    def test_prepare_native_launch_no_compat_tool(self):
        command, cwd, env_overrides = prepare_native_launch_command(
            _game(native_compat_tool=""), _resolve_executable, _split_args
        )
        self.assertEqual(env_overrides, {})
        self.assertNotIn("wine", command[0])
        self.assertNotIn("umu-run", command[0])

    def test_prepare_native_launch_wine(self):
        with patch("rom_mate.emulator.launch.shutil.which", return_value="/usr/bin/wine"), patch(
            "rom_mate.emulator.launch.os.makedirs"
        ):
            command, cwd, env_overrides = prepare_native_launch_command(
                _game(native_compat_tool="wine", native_wineprefix="/tmp/prefix"),
                _resolve_executable,
                _split_args,
            )
        self.assertEqual(command[0], "/usr/bin/wine")
        self.assertEqual(env_overrides, {"WINEPREFIX": "/tmp/prefix"})

    def test_prepare_native_launch_wine_no_prefix(self):
        with patch("rom_mate.emulator.launch.shutil.which", return_value="/usr/bin/wine"):
            command, cwd, env_overrides = prepare_native_launch_command(
                _game(native_compat_tool="wine", native_wineprefix=""),
                _resolve_executable,
                _split_args,
            )
        self.assertEqual(command[0], "/usr/bin/wine")
        self.assertEqual(env_overrides, {})

    def test_prepare_native_launch_proton_umu_found(self):
        with patch("rom_mate.emulator.launch.shutil.which", return_value="/usr/bin/umu-run"), patch(
            "rom_mate.emulator.launch.os.makedirs"
        ):
            command, cwd, env_overrides = prepare_native_launch_command(
                _game(native_compat_tool="/path/to/GE-Proton", native_wineprefix="/tmp/pfx"),
                _resolve_executable,
                _split_args,
            )
        self.assertEqual(command[0], "/usr/bin/umu-run")
        self.assertEqual(
            env_overrides,
            {"PROTONPATH": "/path/to/GE-Proton", "WINEPREFIX": "/tmp/pfx"},
        )

    def test_prepare_native_launch_proton_umu_missing(self):
        with patch("rom_mate.emulator.launch.shutil.which", return_value=None):
            with self.assertRaises(ValueError):
                prepare_native_launch_command(
                    _game(native_compat_tool="/path/to/GE-Proton"),
                    _resolve_executable,
                    _split_args,
                )

    def test_detect_umu_run_found(self):
        with patch("rom_mate.emulator.launch.shutil.which", return_value="/usr/bin/umu-run"):
            self.assertEqual(detect_umu_run(), "/usr/bin/umu-run")

    def test_detect_umu_run_not_found(self):
        with patch("rom_mate.emulator.launch.shutil.which", return_value=None):
            self.assertIsNone(detect_umu_run())


if __name__ == "__main__":
    unittest.main()
