from __future__ import annotations

import base64
import ctypes
import sys
from pathlib import Path
from typing import Any, Callable

import keyring

_SERVICE_NAME = "GRIDLauncher"
_ACCOUNT_API_TOKEN = "api_token"
_ACCOUNT_RA_TOKEN = "retroachievements_token"
_ACCOUNT_RA_API_KEY = "retroachievements_api_key"


def windows_protect_data(raw: bytes) -> bytes:
    class DataBlob(ctypes.Structure):
        _fields_ = [("cbData", ctypes.c_uint32), ("pbData", ctypes.POINTER(ctypes.c_byte))]

    if not raw:
        return b""

    in_buffer = ctypes.create_string_buffer(raw, len(raw))
    in_blob = DataBlob(len(raw), ctypes.cast(in_buffer, ctypes.POINTER(ctypes.c_byte)))
    out_blob = DataBlob()

    if not ctypes.windll.crypt32.CryptProtectData(
        ctypes.byref(in_blob),
        None,
        None,
        None,
        None,
        0,
        ctypes.byref(out_blob),
    ):
        raise OSError("Could not securely protect token")

    try:
        return ctypes.string_at(out_blob.pbData, out_blob.cbData)
    finally:
        ctypes.windll.kernel32.LocalFree(out_blob.pbData)


def windows_unprotect_data(protected: bytes) -> bytes:
    class DataBlob(ctypes.Structure):
        _fields_ = [("cbData", ctypes.c_uint32), ("pbData", ctypes.POINTER(ctypes.c_byte))]

    if not protected:
        return b""

    in_buffer = ctypes.create_string_buffer(protected, len(protected))
    in_blob = DataBlob(len(protected), ctypes.cast(in_buffer, ctypes.POINTER(ctypes.c_byte)))
    out_blob = DataBlob()

    if not ctypes.windll.crypt32.CryptUnprotectData(
        ctypes.byref(in_blob),
        None,
        None,
        None,
        None,
        0,
        ctypes.byref(out_blob),
    ):
        raise OSError("Could not securely unprotect token")

    try:
        return ctypes.string_at(out_blob.pbData, out_blob.cbData)
    finally:
        ctypes.windll.kernel32.LocalFree(out_blob.pbData)


def _keyring_get(account: str) -> str | None:
    """Return the stored value, or None if unavailable/not found/keyring errored."""
    try:
        return keyring.get_password(_SERVICE_NAME, account)
    except Exception:
        return None


def _keyring_set(account: str, value: str) -> bool:
    """Return True if successfully stored via keyring."""
    try:
        keyring.set_password(_SERVICE_NAME, account, value)
        return True
    except Exception:
        return False


def _keyring_delete(account: str) -> None:
    """Best-effort delete; swallow errors (nothing stored / backend unavailable is fine)."""
    try:
        keyring.delete_password(_SERVICE_NAME, account)
    except Exception:
        pass


def _load_legacy_file(token_file: Path) -> str:
    """Decode a legacy DPAPI(win)/base64(else) file. Return "" if missing/unreadable/corrupt."""
    if not token_file.exists():
        return ""

    try:
        payload = token_file.read_bytes()
    except OSError:
        return ""

    if not payload:
        return ""

    try:
        if sys.platform.startswith("win"):
            raw = windows_unprotect_data(payload)
        else:
            raw = base64.b64decode(payload, validate=True)
        return raw.decode("utf-8")
    except (OSError, ValueError, UnicodeDecodeError):
        return ""


def _delete_legacy_file(token_file: Path) -> None:
    try:
        if token_file.exists():
            token_file.unlink()
    except OSError:
        pass


def _save_secret(account: str, config_dir: Path, token_file: Path, value: str) -> bool:
    """Store `value` under `account` in keyring, refusing insecure storage on failure.

    Empty value clears any stored secret (keyring + legacy file) and returns True.
    On keyring failure, Windows falls back to DPAPI file storage (last-resort, still
    genuinely encrypted); all other platforms refuse and return False.
    """
    normalized = value.strip()

    if not normalized:
        _keyring_delete(account)
        _delete_legacy_file(token_file)
        return True

    if _keyring_set(account, normalized):
        _delete_legacy_file(token_file)
        return True

    if sys.platform.startswith("win"):
        try:
            config_dir.mkdir(parents=True, exist_ok=True)
            payload = windows_protect_data(normalized.encode("utf-8"))
            token_file.write_bytes(payload)
            return True
        except OSError:
            return False

    return False


def _load_secret(account: str, token_file: Path) -> str:
    """Load the value for `account`, migrating a legacy file into keyring if found."""
    stored = _keyring_get(account)
    if stored:
        return stored

    legacy_value = _load_legacy_file(token_file)
    if not legacy_value:
        return ""

    if _keyring_set(account, legacy_value):
        _delete_legacy_file(token_file)

    return legacy_value


def load_api_token(token_file: Path) -> str:
    return _load_secret(_ACCOUNT_API_TOKEN, token_file)


def load_ra_token(token_file: Path) -> str:
    return _load_secret(_ACCOUNT_RA_TOKEN, token_file)


def load_ra_api_key(token_file: Path) -> str:
    return _load_secret(_ACCOUNT_RA_API_KEY, token_file)


def save_api_token(config_dir: Path, token_file: Path, token: str) -> bool:
    return _save_secret(_ACCOUNT_API_TOKEN, config_dir, token_file, token)


def save_ra_token(config_dir: Path, token_file: Path, token: str) -> bool:
    return _save_secret(_ACCOUNT_RA_TOKEN, config_dir, token_file, token)


def save_ra_api_key(config_dir: Path, token_file: Path, api_key: str) -> bool:
    return _save_secret(_ACCOUNT_RA_API_KEY, config_dir, token_file, api_key)


def set_api_token(
    config: dict[str, Any],
    token: str,
    *,
    save_token: Callable[[str], bool],
) -> bool:
    normalized = token.strip()
    saved = save_token(normalized)
    if saved:
        config["api_token"] = normalized
    return saved
