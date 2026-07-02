import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from rom_mate.core import token_store


class TokenStoreTests(unittest.TestCase):
    def test_save_api_token_success_deletes_legacy_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            config_dir = Path(temp_dir)
            token_file = config_dir / "token.bin"
            token_file.write_bytes(b"legacy-bytes")

            with patch.object(token_store, "_keyring_set", return_value=True) as mock_set:
                result = token_store.save_api_token(config_dir, token_file, "secret-token")

            self.assertTrue(result)
            mock_set.assert_called_once_with("api_token", "secret-token")
            self.assertFalse(token_file.exists())

    def test_save_ra_token_success_deletes_legacy_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            config_dir = Path(temp_dir)
            token_file = config_dir / "ra_token.bin"
            token_file.write_bytes(b"legacy-bytes")

            with patch.object(token_store, "_keyring_set", return_value=True) as mock_set:
                result = token_store.save_ra_token(config_dir, token_file, "secret-token")

            self.assertTrue(result)
            mock_set.assert_called_once_with("retroachievements_token", "secret-token")
            self.assertFalse(token_file.exists())

    def test_save_ra_api_key_success_deletes_legacy_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            config_dir = Path(temp_dir)
            token_file = config_dir / "ra_api_key.bin"
            token_file.write_bytes(b"legacy-bytes")

            with patch.object(token_store, "_keyring_set", return_value=True) as mock_set:
                result = token_store.save_ra_api_key(config_dir, token_file, "secret-key")

            self.assertTrue(result)
            mock_set.assert_called_once_with("retroachievements_api_key", "secret-key")
            self.assertFalse(token_file.exists())

    def test_save_falls_back_to_dpapi_on_windows_when_keyring_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            config_dir = Path(temp_dir)
            token_file = config_dir / "token.bin"

            with patch.object(token_store, "_keyring_set", return_value=False), patch(
                "sys.platform", "win32"
            ), patch.object(
                token_store, "windows_protect_data", return_value=b"protected-bytes"
            ) as mock_protect:
                result = token_store.save_api_token(config_dir, token_file, "secret-token")

            self.assertTrue(result)
            mock_protect.assert_called_once_with(b"secret-token")
            self.assertTrue(token_file.exists())
            self.assertEqual(token_file.read_bytes(), b"protected-bytes")

    def test_save_refuses_on_non_windows_when_keyring_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            config_dir = Path(temp_dir)
            token_file = config_dir / "token.bin"

            with patch.object(token_store, "_keyring_set", return_value=False), patch(
                "sys.platform", "linux"
            ):
                result = token_store.save_api_token(config_dir, token_file, "secret-token")

            self.assertFalse(result)
            self.assertFalse(token_file.exists())

    def test_save_empty_string_clears_keyring_and_legacy_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            config_dir = Path(temp_dir)
            token_file = config_dir / "token.bin"
            token_file.write_bytes(b"legacy-bytes")

            with patch.object(token_store, "_keyring_delete") as mock_delete:
                result = token_store.save_api_token(config_dir, token_file, "")

            self.assertTrue(result)
            mock_delete.assert_called_once_with("api_token")
            self.assertFalse(token_file.exists())

    def test_load_returns_keyring_value_directly(self) -> None:
        token_file = Path("/nonexistent/token.bin")

        with patch.object(token_store, "_keyring_get", return_value="stored-token") as mock_get, patch.object(
            token_store, "_load_legacy_file"
        ) as mock_legacy:
            result = token_store.load_api_token(token_file)

        self.assertEqual(result, "stored-token")
        mock_get.assert_called_once_with("api_token")
        mock_legacy.assert_not_called()

    def test_load_migrates_legacy_file_on_success(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            config_dir = Path(temp_dir)
            token_file = config_dir / "token.bin"
            token_file.write_bytes(b"legacy-bytes")

            with patch.object(token_store, "_keyring_get", return_value=None), patch.object(
                token_store, "_load_legacy_file", return_value="legacy-token"
            ), patch.object(token_store, "_keyring_set", return_value=True) as mock_set:
                result = token_store.load_api_token(token_file)

            self.assertEqual(result, "legacy-token")
            mock_set.assert_called_once_with("api_token", "legacy-token")
            self.assertFalse(token_file.exists())

    def test_load_keeps_legacy_file_when_migration_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            config_dir = Path(temp_dir)
            token_file = config_dir / "token.bin"
            token_file.write_bytes(b"legacy-bytes")

            with patch.object(token_store, "_keyring_get", return_value=None), patch.object(
                token_store, "_load_legacy_file", return_value="legacy-token"
            ), patch.object(token_store, "_keyring_set", return_value=False) as mock_set:
                result = token_store.load_api_token(token_file)

            self.assertEqual(result, "legacy-token")
            mock_set.assert_called_once_with("api_token", "legacy-token")
            self.assertTrue(token_file.exists())

    def test_load_windows_legacy_file_uses_dpapi_unprotect(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            config_dir = Path(temp_dir)
            token_file = config_dir / "token.bin"
            token_file.write_bytes(b"protected-bytes")

            with patch.object(token_store, "_keyring_get", return_value=None), patch(
                "sys.platform", "win32"
            ), patch.object(
                token_store, "windows_unprotect_data", return_value=b"legacy-token"
            ) as mock_unprotect, patch.object(
                token_store, "_keyring_set", return_value=True
            ):
                result = token_store.load_api_token(token_file)

            self.assertEqual(result, "legacy-token")
            mock_unprotect.assert_called_once_with(b"protected-bytes")

    def test_load_returns_empty_string_when_nothing_found(self) -> None:
        token_file = Path("/nonexistent/token.bin")

        with patch.object(token_store, "_keyring_get", return_value=None):
            result = token_store.load_api_token(token_file)

        self.assertEqual(result, "")

    def test_load_ra_token_and_ra_api_key_use_distinct_accounts(self) -> None:
        token_file = Path("/nonexistent/token.bin")

        with patch.object(token_store, "_keyring_get", return_value="value") as mock_get:
            token_store.load_ra_token(token_file)
            token_store.load_ra_api_key(token_file)

        mock_get.assert_any_call("retroachievements_token")
        mock_get.assert_any_call("retroachievements_api_key")

    def test_set_api_token_calls_save_callback_and_updates_config_on_success(self) -> None:
        config: dict = {}
        save_token_mock = Mock(return_value=True)

        result = token_store.set_api_token(config, "new-token", save_token=save_token_mock)

        self.assertTrue(result)
        save_token_mock.assert_called_once_with("new-token")
        self.assertEqual(config["api_token"], "new-token")

    def test_set_api_token_does_not_update_config_on_failure(self) -> None:
        config: dict = {}
        save_token_mock = Mock(return_value=False)

        result = token_store.set_api_token(config, "new-token", save_token=save_token_mock)

        self.assertFalse(result)
        save_token_mock.assert_called_once_with("new-token")
        self.assertNotIn("api_token", config)


if __name__ == "__main__":
    unittest.main()
