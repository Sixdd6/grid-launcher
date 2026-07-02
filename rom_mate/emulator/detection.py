from __future__ import annotations

import shutil
import sys
from pathlib import Path
from typing import Any

from rom_mate.core.path import xdg_data_home

# Flatpak app IDs for autoprofiles that need the true config root passed
# separately, because their own path-resolution logic has no Flatpak-aware
# candidate list and always derives the config directory from
# emulator_entry["path"] (which, for a Flatpak-detected entry, must be the
# "flatpak" binary itself in order to be launchable).
_PROFILES_NEEDING_CONFIG_ROOT = {
    "org.ppsspp.PPSSPP",
    "org.mamedev.MAME",
}


def installed_flatpak_app_ids() -> set[str]:
    """Return the set of installed Flatpak app IDs on this system.

    Reads directory listings under the per-user and system-wide Flatpak app
    install locations. Never shells out to the flatpak CLI. Missing or
    unreadable directories are silently ignored. Returns an empty set on
    non-Linux platforms.
    """
    if sys.platform not in ("linux", "linux2"):
        return set()

    app_dirs = [
        xdg_data_home() / "flatpak" / "app",
        Path("/var/lib/flatpak/app"),
    ]

    app_ids: set[str] = set()
    for app_dir in app_dirs:
        try:
            if not app_dir.is_dir():
                continue
            for entry in app_dir.iterdir():
                if entry.is_dir():
                    app_ids.add(entry.name)
        except OSError:
            continue

    return app_ids


def _flatpak_config_root_for_app_id(app_id: str) -> Path:
    """Return ~/.var/app/<app_id>/config for a given Flatpak app id."""
    return Path.home() / ".var" / "app" / app_id / "config"


def detect_installed_flatpak_emulators(
    autoprofiles: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Cross-reference installed Flatpak app IDs against autoprofiles carrying
    a non-empty 'flatpak_app_id' field, returning ready-to-use manual
    emulator entries for each match.
    """
    if sys.platform not in ("linux", "linux2"):
        return []

    flatpak_binary = shutil.which("flatpak")
    if not flatpak_binary:
        return []

    installed_app_ids = installed_flatpak_app_ids()
    if not installed_app_ids:
        return []

    detected: list[dict[str, Any]] = []
    for profile in autoprofiles:
        if not isinstance(profile, dict):
            continue

        app_id = profile.get("flatpak_app_id", "")
        app_id = app_id.strip() if isinstance(app_id, str) else ""
        if not app_id or app_id not in installed_app_ids:
            continue

        name = profile.get("name", "")
        name = name.strip() if isinstance(name, str) else ""
        args_template = profile.get("args", "")
        args_template = args_template.strip() if isinstance(args_template, str) else ""

        entry: dict[str, Any] = {
            "name": name,
            "path": flatpak_binary,
            "args": f"run {app_id} {args_template}".strip(),
            "flatpak_app_id": app_id,
        }

        if app_id in _PROFILES_NEEDING_CONFIG_ROOT:
            if app_id == "org.ppsspp.PPSSPP":
                config_root = _flatpak_config_root_for_app_id(app_id) / "ppsspp"
            else:
                config_root = _flatpak_config_root_for_app_id(app_id) / "mame"
            entry["_flatpak_config_root"] = str(config_root)

        detected.append(entry)

    return detected
