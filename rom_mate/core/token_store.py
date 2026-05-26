from __future__ import annotations

import base64
import ctypes
import sys
from pathlib import Path
from typing import Any, Callable


def _linux_load_token(service: str, username: str) -> str:
    try:
        import keyring

        value = keyring.get_password(service, username)
        return value or ""
    except Exception:
        return ""


def _linux_save_token(service: str, username: str, token: str) -> bool:
    try:
        import keyring

        if token:
            keyring.set_password(service, username, token)
        else:
            try:
                keyring.delete_password(service, username)
            except keyring.errors.PasswordDeleteError:
                pass
        return True
    except Exception:
        return False


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


def load_api_token(token_file: Path) -> str:
    if sys.platform == "linux":
        keyring_token = _linux_load_token("rom-mate-neo", "api-token")
        if keyring_token:
            return keyring_token

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
        decoded = raw.decode("utf-8")
        if sys.platform == "linux" and _linux_save_token("rom-mate-neo", "api-token", decoded):
            try:
                if token_file.exists():
                    token_file.unlink()
            except OSError:
                pass
        return decoded
    except (OSError, ValueError, UnicodeDecodeError):
        return ""


def load_ra_token(token_file: Path) -> str:
    if sys.platform == "linux":
        keyring_token = _linux_load_token("rom-mate-neo", "ra-token")
        if keyring_token:
            return keyring_token

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
        decoded = raw.decode("utf-8")
        if sys.platform == "linux" and _linux_save_token("rom-mate-neo", "ra-token", decoded):
            try:
                if token_file.exists():
                    token_file.unlink()
            except OSError:
                pass
        return decoded
    except (OSError, ValueError, UnicodeDecodeError):
        return ""


def save_api_token(config_dir: Path, token_file: Path, token: str) -> bool:
    normalized = token.strip()

    if sys.platform == "linux":
        saved = _linux_save_token("rom-mate-neo", "api-token", normalized)
        if saved:
            try:
                if token_file.exists():
                    token_file.unlink()
            except OSError:
                pass
            return True

    if not normalized:
        try:
            if token_file.exists():
                token_file.unlink()
            return True
        except OSError:
            return False

    try:
        config_dir.mkdir(parents=True, exist_ok=True)
        raw = normalized.encode("utf-8")
        if sys.platform.startswith("win"):
            payload = windows_protect_data(raw)
        else:
            payload = base64.b64encode(raw)
        token_file.write_bytes(payload)
        return True
    except OSError:
        return False


def save_ra_token(config_dir: Path, token_file: Path, token: str) -> bool:
    normalized = token.strip()

    if sys.platform == "linux":
        saved = _linux_save_token("rom-mate-neo", "ra-token", normalized)
        if saved:
            try:
                if token_file.exists():
                    token_file.unlink()
            except OSError:
                pass
            return True

    if not normalized:
        try:
            if token_file.exists():
                token_file.unlink()
            return True
        except OSError:
            return False

    try:
        config_dir.mkdir(parents=True, exist_ok=True)
        raw = normalized.encode("utf-8")
        if sys.platform.startswith("win"):
            payload = windows_protect_data(raw)
        else:
            payload = base64.b64encode(raw)
        token_file.write_bytes(payload)
        return True
    except OSError:
        return False


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
