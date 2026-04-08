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


def _clean_path_value(value: str) -> str:
    cleaned = value.strip()
    if cleaned and cleaned[0] not in {'"', "'"}:
        cleaned = re.split(r"\s+#", cleaned, maxsplit=1)[0].strip()
    return cleaned.strip().strip('"').strip("'")


def _resolve_setting_path(base_root: Path, raw_value: str, default_name: str) -> str:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_name

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute():
        candidate = base_root / candidate
    return str(candidate.resolve())


def _launch_config_path(
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

        if lowered_arg in {"-config_path", "--config_path", "-config-path", "--config-path"} and index < len(args):
            value, consumed_index = _consume_arg_value(args, index)
            index = consumed_index + 1
            if value:
                candidate = Path(os.path.expandvars(value)).expanduser()
                if candidate.suffix:
                    return candidate.resolve()
                return (candidate / "xemu.toml").resolve()
            continue

        for prefix in ("-config_path=", "--config_path=", "-config-path=", "--config-path="):
            if lowered_arg.startswith(prefix):
                _, _, raw_value = normalized_arg.partition("=")
                value = raw_value.strip().strip('"').strip("'")
                if value:
                    candidate = Path(os.path.expandvars(value)).expanduser()
                    if candidate.suffix:
                        return candidate.resolve()
                    return (candidate / "xemu.toml").resolve()

    return None


def _default_base_root() -> Path:
    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA", "").strip()
        if appdata:
            return (Path(appdata).expanduser() / "xemu" / "xemu").resolve()

    home_path = Path.home()
    if sys.platform == "darwin":
        return (home_path / "Library" / "Application Support" / "xemu" / "xemu").resolve()

    xdg_data_home = os.environ.get("XDG_DATA_HOME", "").strip()
    if xdg_data_home:
        return (Path(xdg_data_home).expanduser() / "xemu" / "xemu").resolve()

    return (home_path / ".local" / "share" / "xemu" / "xemu").resolve()


def xemu_base_path_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    candidates: list[Path] = []

    config_override = _launch_config_path(launch_template, split_launch_template_args)
    if config_override is not None:
        candidates.append(config_override.parent.resolve())

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            portable_config = emulator_dir / "xemu.toml"
            if portable_config.exists() or (emulator_dir / "xbox_hdd.qcow2").exists() or (emulator_dir / "eeprom.bin").exists():
                candidates.append(emulator_dir.resolve())

    candidates.append(_default_base_root())
    return _unique_paths(candidates)


def xemu_config_path_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    config_override = _launch_config_path(launch_template, split_launch_template_args)
    if config_override is not None:
        return _unique_paths(
            [
                config_override.resolve(),
                *[
                    (root / "xemu.toml").resolve()
                    for root in xemu_base_path_candidates(emulator_path_text, launch_template, split_launch_template_args)
                    if root.resolve() != config_override.parent.resolve()
                ],
            ]
        )

    return _unique_paths(
        [root / "xemu.toml" for root in xemu_base_path_candidates(emulator_path_text, launch_template, split_launch_template_args)]
    )


def _parse_inline_table(raw_value: str) -> dict[str, str]:
    stripped = raw_value.strip()
    if not stripped.startswith("{") or not stripped.endswith("}"):
        return {}

    values: dict[str, str] = {}
    body = stripped[1:-1]
    for match in re.finditer(r"([A-Za-z0-9_\.]+)\s*=\s*(\"[^\"]*\"|'[^']*'|[^,]+)", body):
        key = match.group(1).strip().casefold()
        values[key] = match.group(2).strip()
    return values


def _parse_toml_sections(raw_content: str) -> dict[str, dict[str, str]]:
    sections: dict[str, dict[str, str]] = {}
    current_section = ""

    for raw_line in raw_content.splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("#"):
            continue

        section_match = re.match(r"^\[(.+?)\]\s*$", stripped)
        if section_match:
            current_section = section_match.group(1).strip().casefold()
            sections.setdefault(current_section, {})
            continue

        if "=" not in raw_line:
            continue

        key, value = raw_line.split("=", 1)
        normalized_key = key.strip().casefold()
        normalized_value = value.strip()

        target_section = current_section
        target_key = normalized_key
        if "." in normalized_key:
            prefix, _, suffix = normalized_key.rpartition(".")
            target_section = f"{current_section}.{prefix}".strip(".") if current_section else prefix
            target_key = suffix

        sections.setdefault(target_section, {})[target_key] = normalized_value

        if normalized_key == "files":
            inline_values = _parse_inline_table(normalized_value)
            if inline_values:
                file_section = sections.setdefault(f"{current_section}.files".strip("."), {})
                for inline_key, inline_value in inline_values.items():
                    file_section[inline_key.rsplit(".", 1)[-1]] = inline_value

    return sections


def xemu_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    base_paths = xemu_base_path_candidates(emulator_path_text, launch_template, split_launch_template_args)
    config_paths = xemu_config_path_candidates(emulator_path_text, launch_template, split_launch_template_args)

    defaults = {
        "config_path": "",
        "base_path": "",
        "hdd_path": "",
        "eeprom_path": "",
    }

    for base_root, config_path in zip(base_paths, config_paths, strict=False):
        settings = defaults.copy()
        settings["base_path"] = str(base_root.resolve())
        settings["config_path"] = str(config_path.resolve())
        settings["hdd_path"] = str((base_root / "xbox_hdd.qcow2").resolve())
        settings["eeprom_path"] = str((base_root / "eeprom.bin").resolve())

        if config_path.exists() and config_path.is_file():
            try:
                raw_content = config_path.read_text(encoding="utf-8")
            except OSError:
                raw_content = ""

            if raw_content:
                sections = _parse_toml_sections(raw_content)
                file_settings = sections.get("sys.files", {})
                settings["hdd_path"] = _resolve_setting_path(base_root, file_settings.get("hdd_path", ""), "xbox_hdd.qcow2")
                settings["eeprom_path"] = _resolve_setting_path(base_root, file_settings.get("eeprom_path", ""), "eeprom.bin")
            return settings

        if base_root.exists() and base_root.is_dir():
            return settings

    if base_paths:
        base_root = base_paths[0]
        defaults["base_path"] = str(base_root.resolve())
        defaults["config_path"] = str((base_root / "xemu.toml").resolve())
        defaults["hdd_path"] = str((base_root / "xbox_hdd.qcow2").resolve())
        defaults["eeprom_path"] = str((base_root / "eeprom.bin").resolve())

    return defaults


def xemu_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = xemu_directory_settings(emulator_path_text, launch_template, split_launch_template_args)

    unique: list[str] = []
    seen: set[str] = set()
    for key_name in ("hdd_path", "eeprom_path"):
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
