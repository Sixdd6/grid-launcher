from __future__ import annotations

import os
import re
import sys
from pathlib import Path
from typing import Callable


def _unique_paths(paths: list[Path]) -> list[Path]:
    unique: list[Path] = []
    seen: set[str] = set()
    for candidate in paths:
        key = str(candidate).casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        unique.append(candidate)
    return unique


def _split_launch_args(
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> list[str]:
    template = launch_template.strip() if isinstance(launch_template, str) else ""
    if not template or not callable(split_launch_template_args):
        return []

    try:
        return split_launch_template_args(template)
    except ValueError:
        return []


def _consume_arg_value(args: list[str], start_index: int) -> tuple[str, int]:
    if start_index >= len(args):
        return "", start_index

    token = args[start_index].strip()
    if not token:
        return "", start_index

    quote = token[0] if token[0] in {'"', "'"} else ""
    if quote and (len(token) == 1 or not token.endswith(quote)):
        parts = [token]
        index = start_index + 1
        while index < len(args):
            parts.append(args[index])
            if args[index].strip().endswith(quote):
                break
            index += 1
        token = " ".join(parts)
        return token.strip().strip('"').strip("'"), index

    return token.strip().strip('"').strip("'"), start_index


def _launch_home_root(
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> Path | None:
    args = _split_launch_args(launch_template, split_launch_template_args)
    index = 0

    while index < len(args):
        raw_arg = args[index]
        index += 1
        if not isinstance(raw_arg, str) or not raw_arg.strip():
            continue

        normalized_arg = raw_arg.strip()
        lowered_arg = normalized_arg.casefold()

        if lowered_arg in {"-home", "--home"} and index < len(args):
            value, consumed_index = _consume_arg_value(args, index)
            index = consumed_index + 1
            if value:
                return Path(os.path.expandvars(value)).expanduser().resolve()
            continue

        for prefix in ("-home=", "--home="):
            if lowered_arg.startswith(prefix):
                _, _, raw_value = normalized_arg.partition("=")
                value = raw_value.strip().strip('"').strip("'")
                if value:
                    return Path(os.path.expandvars(value)).expanduser().resolve()

    return None


def _clean_path_value(value: str) -> str:
    return value.strip().strip('"').strip("'")


def _resolve_setting_path(root: Path, raw_value: str, default_value: str) -> str:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_value

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute():
        candidate = root / candidate
    return str(candidate.resolve())


def _parse_config_values(raw_content: str) -> dict[str, str]:
    values: dict[str, str] = {}

    for raw_line in raw_content.splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith(("#", ";", "--")):
            continue

        match = re.match(r"^([A-Za-z0-9_]+)\s+(.+?)\s*$", stripped)
        if not match:
            continue

        key = match.group(1).strip().casefold()
        value = match.group(2).strip()
        values[key] = value

    return values


def pico8_user_root_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    candidates: list[Path] = []

    launch_home_root = _launch_home_root(launch_template, split_launch_template_args)
    if launch_home_root is not None:
        candidates.append(launch_home_root)

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    emulator_dir = Path()
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            for local_candidate in (emulator_dir, emulator_dir / "pico-8", emulator_dir / "userdata"):
                if (local_candidate / "config.txt").exists() or (local_candidate / "cdata").exists() or (local_candidate / "cstore").exists():
                    candidates.append(local_candidate.resolve())

    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA", "")
        if isinstance(appdata, str) and appdata.strip():
            candidates.append((Path(appdata).expanduser() / "pico-8").resolve())
    else:
        home_path = Path.home()
        if sys.platform == "darwin":
            candidates.append((home_path / "Library" / "Application Support" / "pico-8").resolve())
        else:
            candidates.append((home_path / ".lexaloffle" / "pico-8").resolve())

    return _unique_paths([candidate.expanduser().resolve() for candidate in candidates if str(candidate)])


def pico8_settings_path_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    return _unique_paths(
        [root / "config.txt" for root in pico8_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)]
    )


def pico8_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    user_roots = pico8_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)
    settings_candidates = pico8_settings_path_candidates(emulator_path_text, launch_template, split_launch_template_args)

    defaults = {
        "config_path": "",
        "user_root": "",
        "carts_root": "",
        "cdata_root": "",
        "cstore_root": "",
        "backup_root": "",
        "desktop_path": "",
    }

    for root, candidate in zip(user_roots, settings_candidates, strict=False):
        defaults["user_root"] = str(root.resolve())
        defaults["config_path"] = str(candidate.resolve())
        defaults["carts_root"] = str((root / "carts").resolve())
        defaults["desktop_path"] = str((root / "desktop").resolve())
        defaults["cdata_root"] = str((root / "cdata").resolve())
        defaults["cstore_root"] = str((root / "cstore").resolve())
        defaults["backup_root"] = str((root / "backup").resolve())

        if candidate.exists() and candidate.is_file():
            try:
                raw_content = candidate.read_text(encoding="utf-8")
            except OSError:
                raw_content = ""

            if raw_content:
                config_values = _parse_config_values(raw_content)
                defaults["carts_root"] = _resolve_setting_path(root, config_values.get("root_path", ""), "carts")
                defaults["desktop_path"] = _resolve_setting_path(root, config_values.get("desktop", ""), "desktop")
            return defaults

        if root.exists() and root.is_dir():
            return defaults

    return defaults


def pico8_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = pico8_directory_settings(emulator_path_text, launch_template, split_launch_template_args)

    unique: list[str] = []
    seen: set[str] = set()
    for key_name in ("cdata_root", "cstore_root"):
        value = settings.get(key_name, "")
        if not isinstance(value, str) or not value.strip():
            continue
        normalized = value.strip()
        key = normalized.casefold()
        if key in seen:
            continue
        seen.add(key)
        unique.append(normalized)
    return unique
