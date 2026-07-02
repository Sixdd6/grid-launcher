from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


class Vita3KPrefPathTests(unittest.TestCase):
    def _fn(self, *args, **kwargs):
        from grid_launcher.emulator.vita3k import vita3k_pref_path
        return vita3k_pref_path(*args, **kwargs)

    def test_empty_path_returns_none(self) -> None:
        self.assertIsNone(self._fn(""))

    def test_non_string_path_returns_none(self) -> None:
        self.assertIsNone(self._fn(None))  # type: ignore[arg-type]

    def test_portable_dir_takes_priority_over_config_yml(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            portable_dir = emu_dir / "portable"
            portable_dir.mkdir()
            config = emu_dir / "config.yml"
            config.write_text("pref-path: /other/path\n", encoding="utf-8")
            result = self._fn(str(emu_dir / "Vita3K"))
        self.assertEqual(result, portable_dir)

    def test_config_yml_pref_path_used_when_no_portable_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            config = emu_dir / "config.yml"
            config.write_text("pref-path: /custom/vita3k/data\n", encoding="utf-8")
            result = self._fn(str(emu_dir / "Vita3K"))
        self.assertEqual(result, Path("/custom/vita3k/data"))

    def test_config_yml_double_quoted_value(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            config = emu_dir / "config.yml"
            config.write_text('pref-path: "/path/with spaces"\n', encoding="utf-8")
            result = self._fn(str(emu_dir / "Vita3K"))
        self.assertEqual(result, Path("/path/with spaces"))

    def test_config_yml_single_quoted_value(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            config = emu_dir / "config.yml"
            config.write_text("pref-path: '/single/quoted'\n", encoding="utf-8")
            result = self._fn(str(emu_dir / "Vita3K"))
        self.assertEqual(result, Path("/single/quoted"))

    def test_config_yml_tilde_expanded(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            config = emu_dir / "config.yml"
            config.write_text("pref-path: ~/Vita3K\n", encoding="utf-8")
            result = self._fn(str(emu_dir / "Vita3K"))
        self.assertEqual(result, Path.home() / "Vita3K")

    def test_config_yml_missing_key_falls_through_to_platform_default(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            config = emu_dir / "config.yml"
            config.write_text("some-other-key: value\n", encoding="utf-8")
            with patch("sys.platform", "linux"):
                result = self._fn(str(emu_dir / "Vita3K"))
        self.assertEqual(result, Path.home() / ".local" / "share" / "Vita3K" / "Vita3K")

    def test_platform_default_linux(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            with patch("sys.platform", "linux"):
                result = self._fn(str(emu_dir / "Vita3K"))
        self.assertEqual(result, Path.home() / ".local" / "share" / "Vita3K" / "Vita3K")

    def test_platform_default_windows(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            with patch("sys.platform", "win32"):
                result = self._fn(str(emu_dir / "Vita3K.exe"))
        self.assertEqual(result, Path.home() / "AppData" / "Roaming" / "Vita3K" / "Vita3K")

    def test_platform_default_darwin(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            with patch("sys.platform", "darwin"):
                result = self._fn(str(emu_dir / "Vita3K"))
        self.assertEqual(result, Path.home() / "Library" / "Application Support" / "Vita3K" / "Vita3K")

    def test_unknown_platform_returns_none(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            emu_dir = Path(tmp)
            with patch("sys.platform", "freebsd"):
                result = self._fn(str(emu_dir / "Vita3K"))
        self.assertIsNone(result)


class Vita3KSavePathOverridesTests(unittest.TestCase):
    def _fn(self, *args, **kwargs):
        from grid_launcher.emulator.vita3k import vita3k_save_path_overrides
        return vita3k_save_path_overrides(*args, **kwargs)

    def test_returns_empty_when_pref_path_is_none(self) -> None:
        with patch("grid_launcher.emulator.vita3k.vita3k_pref_path", return_value=None):
            result = self._fn("/some/path/Vita3K", "", lambda s: [])
        self.assertEqual(result, [])

    def test_default_user_00_always_included_when_no_user_dirs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pref_path = Path(tmp)
            with patch("grid_launcher.emulator.vita3k.vita3k_pref_path", return_value=pref_path):
                result = self._fn("/emu/Vita3K", "", lambda s: [])
        expected = str(pref_path / "ux0" / "user" / "00" / "savedata")
        self.assertIn(expected, result)

    def test_single_existing_user_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pref_path = Path(tmp)
            (pref_path / "ux0" / "user" / "00").mkdir(parents=True)
            with patch("grid_launcher.emulator.vita3k.vita3k_pref_path", return_value=pref_path):
                result = self._fn("/emu/Vita3K", "", lambda s: [])
        self.assertEqual(result, [str(pref_path / "ux0" / "user" / "00" / "savedata")])

    def test_multiple_user_dirs_all_returned(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pref_path = Path(tmp)
            for uid in ("00", "01", "02"):
                (pref_path / "ux0" / "user" / uid).mkdir(parents=True)
            with patch("grid_launcher.emulator.vita3k.vita3k_pref_path", return_value=pref_path):
                result = self._fn("/emu/Vita3K", "", lambda s: [])
        self.assertEqual(len(result), 3)
        for uid in ("00", "01", "02"):
            self.assertIn(str(pref_path / "ux0" / "user" / uid / "savedata"), result)

    def test_user_00_prepended_when_only_other_users_exist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pref_path = Path(tmp)
            (pref_path / "ux0" / "user" / "01").mkdir(parents=True)
            with patch("grid_launcher.emulator.vita3k.vita3k_pref_path", return_value=pref_path):
                result = self._fn("/emu/Vita3K", "", lambda s: [])
        self.assertEqual(result[0], str(pref_path / "ux0" / "user" / "00" / "savedata"))
        self.assertIn(str(pref_path / "ux0" / "user" / "01" / "savedata"), result)

    def test_non_two_digit_dirs_are_excluded(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pref_path = Path(tmp)
            user_root = pref_path / "ux0" / "user"
            user_root.mkdir(parents=True)
            (user_root / "00").mkdir()
            (user_root / "temp").mkdir()
            (user_root / "abc").mkdir()
            (user_root / "001").mkdir()
            with patch("grid_launcher.emulator.vita3k.vita3k_pref_path", return_value=pref_path):
                result = self._fn("/emu/Vita3K", "", lambda s: [])
        self.assertEqual(len(result), 1)
        self.assertIn("00", result[0])
        self.assertNotIn("temp", str(result))
        self.assertNotIn("abc", str(result))
        self.assertNotIn("001", str(result))

    def test_launch_template_arg_is_accepted_but_unused(self) -> None:
        with patch("grid_launcher.emulator.vita3k.vita3k_pref_path", return_value=None):
            result = self._fn("/emu/Vita3K", "--exit-on-game-close \"%rom%\"", None)
        self.assertEqual(result, [])


if __name__ == "__main__":
    unittest.main()
