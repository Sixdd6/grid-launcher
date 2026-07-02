from __future__ import annotations

import getpass
from pathlib import Path


def translate_windows_path_to_wine_prefix(raw_path: str, prefix: Path) -> Path | None:
    """Translate a Windows env-var save path to its Linux path inside a Wine prefix.

    Accepts a path such as ``%APPDATA%\\MyGame\\saves`` and returns the
    equivalent location inside *prefix* (e.g.
    ``<prefix>/drive_c/users/<user>/AppData/Roaming/MyGame/saves``).

    Returns ``None`` when the leading environment variable is not recognized.
    Matching of the environment variable is case-insensitive.
    """
    username = getpass.getuser()
    drive_c = prefix / "drive_c"
    user_home = drive_c / "users" / username

    mappings: list[tuple[str, Path]] = [
        ("%USERPROFILE%\\AppData\\LocalLow", user_home / "AppData" / "LocalLow"),
        ("%USERPROFILE%\\Documents", user_home / "Documents"),
        ("%APPDATA%", user_home / "AppData" / "Roaming"),
        ("%LOCALAPPDATA%", user_home / "AppData" / "Local"),
        ("%USERPROFILE%", user_home),
        ("%PROGRAMDATA%", drive_c / "ProgramData"),
        ("%PUBLIC%", drive_c / "users" / "Public"),
        ("%WINDIR%", drive_c / "windows"),
    ]

    raw_cf = raw_path.casefold()
    for token, base in mappings:
        token_cf = token.casefold()
        if raw_cf.startswith(token_cf):
            suffix = raw_path[len(token):]
            suffix = suffix.replace("\\", "/").lstrip("/")
            if suffix:
                return base / suffix
            return base

    return None
