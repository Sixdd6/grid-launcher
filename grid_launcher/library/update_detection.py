from __future__ import annotations

from datetime import datetime, timezone
import re
from typing import Any, Callable


_VERSION_TAG_PATTERN = re.compile(r"\(v(\d{5})\)", re.IGNORECASE)
_SEMVER_TAG_PATTERN = re.compile(r"\(v(\d+(?:\.\d+)+)\)", re.IGNORECASE)


def game_server_updated_at(game: dict[str, Any]) -> str:
    for key in ("server_updated_at", "rom_updated_at", "updated_at"):
        value = game.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return ""


def rom_file_name_version(rom_file_name: str) -> int | str | None:
    if not isinstance(rom_file_name, str):
        return None

    numeric_match = _VERSION_TAG_PATTERN.search(rom_file_name)
    if numeric_match is not None:
        return int(numeric_match.group(1))

    semver_match = _SEMVER_TAG_PATTERN.search(rom_file_name)
    if semver_match is None:
        return None
    return semver_match.group(1)


def _rom_file_name_version_for_compare(rom_file_name: str) -> tuple[str, tuple[int, ...]] | None:
    version_tag = rom_file_name_version(rom_file_name)
    if isinstance(version_tag, int):
        return ("numeric", (version_tag,))
    if isinstance(version_tag, str):
        parts = tuple(int(part) for part in version_tag.split("."))
        return ("semver", parts)
    return None


def _semver_is_newer(installed_parts: tuple[int, ...], server_parts: tuple[int, ...]) -> bool:
    max_length = max(len(installed_parts), len(server_parts))
    for index in range(max_length):
        installed_value = installed_parts[index] if index < len(installed_parts) else 0
        server_value = server_parts[index] if index < len(server_parts) else 0
        if server_value > installed_value:
            return True
        if server_value < installed_value:
            return False
    return False


def has_newer_server_rom_version(installed_rom_file_name: str, server_rom_file_name: str) -> bool:
    installed_version = _rom_file_name_version_for_compare(installed_rom_file_name)
    server_version = _rom_file_name_version_for_compare(server_rom_file_name)
    if installed_version is None or server_version is None:
        return False

    installed_kind, installed_parts = installed_version
    server_kind, server_parts = server_version
    if installed_kind != server_kind:
        return False

    if installed_kind == "numeric":
        return server_parts[0] > installed_parts[0]

    return _semver_is_newer(installed_parts, server_parts)


def _is_windows_pc_platform(game: dict[str, Any]) -> bool:
    platform = game.get("platform", "")
    if not isinstance(platform, str):
        return False
    normalized = platform.strip().casefold()
    if not normalized:
        return False
    return "windows" in normalized or normalized == "pc"


def _parse_timestamp(value: str) -> float | None:
    text = value.strip()
    if not text:
        return None
    candidate = text.replace("Z", "+00:00")
    try:
        parsed = datetime.fromisoformat(candidate)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.timestamp()


def game_has_server_update(
    installed_game: dict[str, Any],
    server_game: dict[str, Any],
    *,
    is_emulators_platform: Callable[[dict[str, Any]], bool] | None = None,
) -> bool:
    platform_check = is_emulators_platform or (lambda game: str(game.get("platform", "")).strip().casefold() == "emulators")
    if platform_check(installed_game) or platform_check(server_game):
        return False

    if _is_windows_pc_platform(installed_game) or _is_windows_pc_platform(server_game):
        installed_rom_file_name = installed_game.get("rom_file_name", "")
        server_rom_file_name = server_game.get("rom_file_name", "")
        if has_newer_server_rom_version(installed_rom_file_name, server_rom_file_name):
            return True

    # Legacy installs do not have an install-time server timestamp.
    installed_updated_at = _parse_timestamp(game_server_updated_at(installed_game))
    if installed_updated_at is None:
        return False

    server_updated_at = _parse_timestamp(game_server_updated_at(server_game))
    if server_updated_at is None:
        return False

    return server_updated_at > installed_updated_at
