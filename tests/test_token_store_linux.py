from __future__ import annotations

import base64
import builtins
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import MagicMock, patch

from rom_mate.core.token_store import load_api_token, load_ra_token, save_api_token, save_ra_token


class TokenStoreLinuxTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.mkdtemp()
        self.config_dir = Path(self.temp_dir) / "config"
        self.token_file = self.config_dir / "token.dat"
        self.ra_token_file = self.config_dir / "ra_token.dat"

    def tearDown(self) -> None:
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def _write_base64_token(self, token_file: Path, token: str) -> None:
        token_file.parent.mkdir(parents=True, exist_ok=True)
        encoded = base64.b64encode(token.encode("utf-8")).decode("utf-8")
        token_file.write_text(encoded, encoding="utf-8")

    def _build_keyring_mock(self) -> MagicMock:
        keyring_mock = MagicMock()
        keyring_mock.errors = SimpleNamespace(PasswordDeleteError=Exception)
        return keyring_mock

    def test_load_api_token_uses_keyring_on_linux(self) -> None:
        keyring_mock = self._build_keyring_mock()
        keyring_mock.get_password.return_value = "tok123"

        nonexistent_path = Path(self.temp_dir) / "does-not-exist.dat"

        with patch("rom_mate.core.token_store.sys.platform", "linux"):
            with patch.dict(sys.modules, {"keyring": keyring_mock}):
                token = load_api_token(nonexistent_path)

        self.assertEqual(token, "tok123")
        self.assertFalse(nonexistent_path.exists())
        keyring_mock.get_password.assert_called_once_with("rom-mate-neo", "api-token")

    def test_load_api_token_falls_back_when_keyring_import_fails(self) -> None:
        expected = "file-token"
        self._write_base64_token(self.token_file, expected)

        original_import = builtins.__import__

        def import_side_effect(name, *args, **kwargs):
            if name == "keyring":
                raise ImportError("keyring unavailable")
            return original_import(name, *args, **kwargs)

        with patch("rom_mate.core.token_store.sys.platform", "linux"):
            with patch.dict(sys.modules, {}, clear=False):
                sys.modules.pop("keyring", None)
                with patch("builtins.__import__", side_effect=import_side_effect):
                    token = load_api_token(self.token_file)

        self.assertEqual(token, expected)

    def test_load_api_token_falls_back_when_keyring_returns_none(self) -> None:
        expected = "file-token"
        self._write_base64_token(self.token_file, expected)

        keyring_mock = self._build_keyring_mock()
        keyring_mock.get_password.return_value = None

        with patch("rom_mate.core.token_store.sys.platform", "linux"):
            with patch.dict(sys.modules, {"keyring": keyring_mock}):
                token = load_api_token(self.token_file)

        self.assertEqual(token, expected)

    def test_load_api_token_migrates_base64_file_to_keyring(self) -> None:
        expected = "migrate-me"
        self._write_base64_token(self.token_file, expected)

        keyring_mock = self._build_keyring_mock()
        keyring_mock.get_password.return_value = ""

        with patch("rom_mate.core.token_store.sys.platform", "linux"):
            with patch.dict(sys.modules, {"keyring": keyring_mock}):
                token = load_api_token(self.token_file)

        self.assertEqual(token, expected)
        keyring_mock.set_password.assert_called_once_with("rom-mate-neo", "api-token", expected)
        self.assertFalse(self.token_file.exists())

    def test_save_api_token_uses_keyring_on_linux(self) -> None:
        keyring_mock = self._build_keyring_mock()

        with patch("rom_mate.core.token_store.sys.platform", "linux"):
            with patch.dict(sys.modules, {"keyring": keyring_mock}):
                saved = save_api_token(self.config_dir, self.token_file, "  api-token-value  ")

        self.assertTrue(saved)
        keyring_mock.set_password.assert_called_once_with("rom-mate-neo", "api-token", "api-token-value")

    def test_save_api_token_deletes_legacy_file_on_keyring_success(self) -> None:
        self._write_base64_token(self.token_file, "legacy-token")

        keyring_mock = self._build_keyring_mock()

        with patch("rom_mate.core.token_store.sys.platform", "linux"):
            with patch.dict(sys.modules, {"keyring": keyring_mock}):
                saved = save_api_token(self.config_dir, self.token_file, "new-token")

        self.assertTrue(saved)
        self.assertFalse(self.token_file.exists())
        keyring_mock.set_password.assert_called_once_with("rom-mate-neo", "api-token", "new-token")

    def test_save_api_token_falls_back_to_file_when_keyring_raises(self) -> None:
        expected = "fallback-token"

        keyring_mock = self._build_keyring_mock()
        keyring_mock.set_password.side_effect = Exception("cannot save to keyring")

        with patch("rom_mate.core.token_store.sys.platform", "linux"):
            with patch.dict(sys.modules, {"keyring": keyring_mock}):
                saved = save_api_token(self.config_dir, self.token_file, expected)

        self.assertTrue(saved)
        self.assertTrue(self.token_file.exists())
        encoded = base64.b64encode(expected.encode("utf-8")).decode("utf-8")
        self.assertEqual(self.token_file.read_text(encoding="utf-8"), encoded)

    def test_save_ra_token_uses_keyring_on_linux(self) -> None:
        keyring_mock = self._build_keyring_mock()

        with patch("rom_mate.core.token_store.sys.platform", "linux"):
            with patch.dict(sys.modules, {"keyring": keyring_mock}):
                saved = save_ra_token(self.config_dir, self.ra_token_file, "  ra-token-value  ")

        self.assertTrue(saved)
        keyring_mock.set_password.assert_called_once_with("rom-mate-neo", "ra-token", "ra-token-value")

    def test_load_ra_token_uses_keyring_on_linux(self) -> None:
        keyring_mock = self._build_keyring_mock()
        keyring_mock.get_password.return_value = "ra123"

        nonexistent_path = Path(self.temp_dir) / "does-not-exist-ra.dat"

        with patch("rom_mate.core.token_store.sys.platform", "linux"):
            with patch.dict(sys.modules, {"keyring": keyring_mock}):
                token = load_ra_token(nonexistent_path)

        self.assertEqual(token, "ra123")
        self.assertFalse(nonexistent_path.exists())
        keyring_mock.get_password.assert_called_once_with("rom-mate-neo", "ra-token")


if __name__ == "__main__":
    unittest.main()
