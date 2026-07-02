from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import Callable


def vita3k_pref_path(emulator_path: str) -> Path | None:
    """Return the Vita3K pref-path (VFS root) for the given emulator executable path.

    Discovery priority:
    1. Portable mode: <emulator_dir>/portable/ directory exists → use it.
    2. Config file: <emulator_dir>/config.yml contains a 'pref-path:' scalar.
    3. Platform default:
       - Linux:   ~/.local/share/Vita3K/Vita3K
       - Windows: ~/AppData/Roaming/Vita3K/Vita3K
       - macOS:   ~/Library/Application Support/Vita3K/Vita3K
    """
    if not isinstance(emulator_path, str) or not emulator_path.strip():
        return None

    emulator_dir = Path(emulator_path.strip()).expanduser().parent

    # 1. Portable mode
    portable_dir = emulator_dir / "portable"
    if portable_dir.is_dir():
        return portable_dir

    # 2. config.yml
    config_path = emulator_dir / "config.yml"
    try:
        if config_path.is_file():
            for line in config_path.read_text(encoding="utf-8", errors="replace").splitlines():
                stripped = line.strip()
                if not stripped.startswith("pref-path:"):
                    continue
                raw_value = stripped[len("pref-path:"):].strip()
                # Strip surrounding single or double quotes
                if len(raw_value) >= 2 and raw_value[0] in ('"', "'") and raw_value[-1] == raw_value[0]:
                    raw_value = raw_value[1:-1]
                if raw_value:
                    return Path(raw_value).expanduser()
    except OSError:
        pass

    # 3. Platform default
    if sys.platform.startswith("linux"):
        return Path.home() / ".local" / "share" / "Vita3K" / "Vita3K"
    if sys.platform.startswith("win32"):
        return Path.home() / "AppData" / "Roaming" / "Vita3K" / "Vita3K"
    if sys.platform.startswith("darwin"):
        return Path.home() / "Library" / "Application Support" / "Vita3K" / "Vita3K"

    return None


_USER_ID_RE = re.compile(r"^\d{2}$")


def vita3k_save_path_overrides(
    emulator_path: str,
    launch_template: str,  # accepted for uniform signature; Vita3K does not encode user IDs in args
    split_launch_template_args: Callable,  # accepted for uniform signature; unused
) -> list[str]:
    """Return save directories for Vita3K.

    Enumerates <pref_path>/ux0/user/{user_id}/savedata for all two-digit user
    directories that exist. User '00' is always included as a fallback even if
    the directory does not yet exist.
    """
    pref_path = vita3k_pref_path(emulator_path)
    if pref_path is None:
        return []

    user_root = pref_path / "ux0" / "user"
    found_ids: list[str] = []

    try:
        if user_root.is_dir():
            for child in sorted(user_root.iterdir(), key=lambda p: p.name):
                if child.is_dir() and _USER_ID_RE.fullmatch(child.name):
                    found_ids.append(child.name)
    except OSError:
        pass

    # Ensure user '00' is always present, at the front
    if "00" not in found_ids:
        found_ids.insert(0, "00")

    seen: set[str] = set()
    result: list[str] = []
    for user_id in found_ids:
        path_str = str(pref_path / "ux0" / "user" / user_id / "savedata")
        if path_str not in seen:
            seen.add(path_str)
            result.append(path_str)

    return result
