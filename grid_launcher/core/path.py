from __future__ import annotations

import os
from pathlib import Path


def sanitize_path_component(value: str, fallback: str) -> str:
    illegal_characters = set('<>:"/\\|?*')
    sanitized = "".join("_" if ch in illegal_characters or ord(ch) < 32 else ch for ch in value)
    while sanitized.endswith((" ", ".")):
        sanitized = f"{sanitized[:-1]}_"
    return sanitized if sanitized.strip(" _.") else fallback


def path_key(path: Path) -> str:
    expanded = path.expanduser()
    try:
        return str(expanded.resolve(strict=False)).casefold()
    except OSError:
        return str(expanded).casefold()


def path_within_path(path: Path, root: Path) -> bool:
    resolved_path_key = path_key(path)
    resolved_root_key = path_key(root).rstrip("\\/")
    if not resolved_root_key:
        return False
    return resolved_path_key == resolved_root_key or resolved_path_key.startswith(
        f"{resolved_root_key}\\"
    ) or resolved_path_key.startswith(f"{resolved_root_key}/")


def xdg_config_home() -> Path:
    value = os.environ.get("XDG_CONFIG_HOME")
    if value:
        return Path(value).expanduser()
    return Path.home() / ".config"


def xdg_data_home() -> Path:
    value = os.environ.get("XDG_DATA_HOME")
    if value:
        return Path(value).expanduser()
    return Path.home() / ".local" / "share"


def grid_launcher_share_dir(fallback: Path) -> Path:
    """Return the directory containing GRID Launcher's bundled data files
    (retroarch-core-list.json, emulator-autoprofiles.json, assets/).
    Under Flatpak, this is /app/share/grid-launcher (set via the
    GRID_LAUNCHER_SHARE_DIR environment variable by the Flatpak launch wrapper).
    Falls back to the given path (the existing dev/PyInstaller resolution)
    when the env var is unset or empty."""
    share_dir = os.environ.get("GRID_LAUNCHER_SHARE_DIR", "").strip()
    if share_dir:
        return Path(share_dir).expanduser()
    return fallback


def compat_tool_install_directory() -> Path:
    return xdg_data_home() / "grid-launcher" / "compat-tools"
