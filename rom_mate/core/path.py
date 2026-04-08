from __future__ import annotations

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
