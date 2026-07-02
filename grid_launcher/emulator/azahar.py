from __future__ import annotations

import configparser
import os
import re
import sys
from pathlib import Path
from typing import Callable

_SDMC_TITLE_GROUPS = ("00040000", "00040002", "0004000e", "0004008c", "00048004")
_NAND_TITLE_GROUPS = ("00040010", "00040030")
_ZERO_ID = "0" * 32


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


def _clean_path_value(value: str) -> str:
    return value.strip().strip('"').strip("'")


def _bool_value(value: str, *, default: bool = False) -> bool:
    if not isinstance(value, str):
        return default
    normalized = value.strip().casefold()
    if not normalized:
        return default
    if normalized in {"1", "true", "yes", "on"}:
        return True
    if normalized in {"0", "false", "no", "off"}:
        return False
    return default


def _resolve_setting_path(root: Path, raw_value: str, default_value: str) -> str:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_value

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute():
        candidate = root / candidate
    return str(candidate.resolve())


def _ensure_section_values(
    raw_content: str,
    section_name: str,
    desired_values: dict[str, str],
) -> tuple[str, bool]:
    if not desired_values:
        return raw_content, False

    lines = raw_content.splitlines()
    output_lines: list[str] = []
    changed = False
    target_key = section_name.casefold()
    in_target = False
    section_found = False
    seen_keys: set[str] = set()

    def flush_missing_keys() -> None:
        nonlocal changed
        for key, value in desired_values.items():
            if key in seen_keys:
                continue
            output_lines.append(f"{key} = {value}")
            seen_keys.add(key)
            changed = True

    for raw_line in lines:
        stripped = raw_line.strip()
        section_match = re.match(r"^\[(.+?)\]\s*$", stripped)
        if section_match:
            if in_target:
                flush_missing_keys()
            current_section = section_match.group(1).strip()
            in_target = current_section.casefold() == target_key
            if in_target:
                section_found = True
            output_lines.append(raw_line)
            continue

        if in_target:
            key_match = re.match(r"^\s*([A-Za-z0-9_%\\]+)\s*=", raw_line)
            if key_match:
                key = key_match.group(1)
                if key in desired_values:
                    if key in seen_keys:
                        changed = True
                        continue
                    replacement = f"{key} = {desired_values[key]}"
                    if raw_line.strip() != replacement:
                        changed = True
                    output_lines.append(replacement)
                    seen_keys.add(key)
                    continue

        output_lines.append(raw_line)

    if in_target:
        flush_missing_keys()

    if not section_found:
        if output_lines and output_lines[-1].strip():
            output_lines.append("")
        output_lines.append(f"[{section_name}]")
        for key, value in desired_values.items():
            output_lines.append(f"{key} = {value}")
        changed = True

    return "\n".join(output_lines).rstrip() + "\n", changed


def azahar_config_path_candidates(emulator_path_text: str) -> list[Path]:
    if not isinstance(emulator_path_text, str) or not emulator_path_text.strip():
        return []

    emulator_path = Path(emulator_path_text).expanduser()
    candidates = [
        emulator_path.parent / "user" / "config" / "qt-config.ini",
        emulator_path.parent / "qt-config.ini",
    ]
    appdata = os.environ.get("APPDATA", "")
    if isinstance(appdata, str) and appdata.strip():
        candidates.append(Path(appdata).expanduser() / "Azahar" / "qt-config.ini")
    candidates.append(Path.home() / ".config" / "Azahar" / "qt-config.ini")
    candidates.append(Path.home() / ".var" / "app" / "org.azahar_emu.Azahar" / "config" / "Azahar" / "qt-config.ini")
    return candidates


def ensure_azahar_settings(emulator_path_text: str) -> dict:
    if isinstance(emulator_path_text, str) and emulator_path_text.strip():
        _emulator_path = Path(emulator_path_text.strip()).expanduser()
        _emulator_dir = _emulator_path if _emulator_path.is_dir() else _emulator_path.parent
        _user_dir = _emulator_dir / "user"
        if not _user_dir.exists():
            try:
                _user_dir.mkdir(parents=True, exist_ok=True)
            except OSError:
                pass

    config_candidates = azahar_config_path_candidates(emulator_path_text)
    if not config_candidates:
        return {"config_path": None, "changed": False}

    config_path = next((candidate for candidate in config_candidates if candidate.exists()), config_candidates[0])

    try:
        content = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    except OSError:
        return {"config_path": None, "changed": False}

    changed = False

    content, section_changed = _ensure_section_values(
        content,
        "Renderer",
        {
            "resolution_factor\\default": "false",
            "resolution_factor": "4",
            "use_vsync\\default": "false",
            "use_vsync": "true",
        },
    )
    changed = changed or section_changed

    content, section_changed = _ensure_section_values(
        content,
        "Audio",
        {
            "volume\\default": "false",
            "volume": "0.4",
        },
    )
    changed = changed or section_changed

    content, section_changed = _ensure_section_values(
        content,
        "UI",
        {
            "enable_discord_presence\\default": "false",
            "enable_discord_presence": "false",
            "confirmClose\\default": "false",
            "confirmClose": "false",
            "fullscreen\\default": "false",
            "fullscreen": "true",
            "pauseWhenInBackground\\default": "false",
            "pauseWhenInBackground": "true",
            "hideInactiveMouse\\default": "false",
            "hideInactiveMouse": "true",
            "Shortcuts\\Main%20Window\\Fullscreen\\KeySeq\\default": "false",
            "Shortcuts\\Main%20Window\\Fullscreen\\KeySeq": "F1",
            "Shortcuts\\Main%20Window\\Stop%20Emulation\\KeySeq\\default": "false",
            "Shortcuts\\Main%20Window\\Stop%20Emulation\\KeySeq": "Escape",
        },
    )
    changed = changed or section_changed

    if changed:
        try:
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(content, encoding="utf-8")
        except OSError:
            return {"config_path": None, "changed": False}

    return {"config_path": str(config_path), "changed": changed}


def azahar_user_root_candidates(
    emulator_path_text: str,
    launch_template: str = "",
    split_launch_template_args: Callable[[str], list[str]] | None = None,
) -> list[Path]:
    del launch_template, split_launch_template_args

    candidates: list[Path] = []
    emulator_dir = Path()

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            portable_root = (emulator_dir / "user").resolve()
            if portable_root.exists() and portable_root.is_dir():
                candidates.append(portable_root)

    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA", "")
        if isinstance(appdata, str) and appdata.strip():
            candidates.append((Path(appdata).expanduser() / "Azahar").resolve())
    else:
        xdg_data_home = os.environ.get("XDG_DATA_HOME", "")
        if isinstance(xdg_data_home, str) and xdg_data_home.strip():
            candidates.append((Path(xdg_data_home).expanduser() / "Azahar").resolve())

        home_path = Path.home()
        candidates.extend(
            [
                home_path / ".local" / "share" / "Azahar",
                home_path / "Library" / "Application Support" / "Azahar",
            ]
        )
        candidates.append(home_path / ".var" / "app" / "org.azahar_emu.Azahar" / "data" / "Azahar")

    if emulator_dir:
        candidates.append((emulator_dir / "user").resolve())

    return _unique_paths([candidate.expanduser().resolve() for candidate in candidates if str(candidate)])


def azahar_settings_path_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    return _unique_paths(
        [root / "config" / "qt-config.ini" for root in azahar_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)]
    )


def azahar_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    user_roots = azahar_user_root_candidates(emulator_path_text, launch_template, split_launch_template_args)
    settings_candidates = azahar_settings_path_candidates(emulator_path_text, launch_template, split_launch_template_args)

    defaults = {
        "config_path": "",
        "user_root": "",
        "nand_root": "",
        "sdmc_root": "",
        "states_root": "",
        "use_custom_storage": "false",
        "use_virtual_sd": "true",
    }

    first_existing_root: Path | None = None

    for root, candidate in zip(user_roots, settings_candidates, strict=False):
        if first_existing_root is None and root.exists() and root.is_dir():
            first_existing_root = root.resolve()

        if not candidate.exists() or not candidate.is_file():
            continue

        parser = configparser.ConfigParser(interpolation=None, strict=False)
        try:
            parser.read(candidate, encoding="utf-8")
        except (configparser.Error, OSError, UnicodeDecodeError):
            continue

        storage = parser["Data Storage"] if parser.has_section("Data Storage") else {}
        use_custom_storage = _bool_value(storage.get("use_custom_storage", "false"))
        use_virtual_sd = _bool_value(storage.get("use_virtual_sd", "true"), default=True)

        defaults["config_path"] = str(candidate.resolve())
        defaults["user_root"] = str(root.resolve())
        defaults["states_root"] = str((root / "states").resolve())
        defaults["use_custom_storage"] = "true" if use_custom_storage else "false"
        defaults["use_virtual_sd"] = "true" if use_virtual_sd else "false"

        if use_custom_storage:
            defaults["nand_root"] = _resolve_setting_path(root, storage.get("nand_directory", ""), "nand")
            defaults["sdmc_root"] = _resolve_setting_path(root, storage.get("sdmc_directory", ""), "sdmc")
        else:
            defaults["nand_root"] = str((root / "nand").resolve())
            defaults["sdmc_root"] = str((root / "sdmc").resolve())
        return defaults

    default_root = first_existing_root or (user_roots[0] if user_roots else Path())
    if str(default_root):
        defaults["user_root"] = str(default_root.resolve())
        defaults["nand_root"] = str((default_root / "nand").resolve())
        defaults["sdmc_root"] = str((default_root / "sdmc").resolve())
        defaults["states_root"] = str((default_root / "states").resolve())

    return defaults


def _existing_sdmc_title_roots(sdmc_root: Path) -> list[Path]:
    container_root = sdmc_root / "Nintendo 3DS"
    discovered: list[Path] = []

    if container_root.exists() and container_root.is_dir():
        for system_dir in sorted(container_root.iterdir(), key=lambda item: item.name):
            if not system_dir.is_dir():
                continue
            for storage_dir in sorted(system_dir.iterdir(), key=lambda item: item.name):
                if not storage_dir.is_dir():
                    continue
                title_root = storage_dir / "title"
                for group in _SDMC_TITLE_GROUPS:
                    candidate = (title_root / group).resolve()
                    if candidate.exists() and candidate.is_dir():
                        discovered.append(candidate)

    if discovered:
        return _unique_paths(discovered)

    default_title_root = container_root / _ZERO_ID / _ZERO_ID / "title"
    return _unique_paths([(default_title_root / group).resolve() for group in _SDMC_TITLE_GROUPS])


def _existing_nand_title_roots(nand_root: Path) -> list[Path]:
    discovered: list[Path] = []
    title_containers: list[Path] = []

    direct_title_root = nand_root / "title"
    if direct_title_root.exists() and direct_title_root.is_dir():
        title_containers.append(direct_title_root)

    if nand_root.exists() and nand_root.is_dir():
        for child in sorted(nand_root.iterdir(), key=lambda item: item.name):
            if not child.is_dir():
                continue
            title_root = child / "title"
            if title_root.exists() and title_root.is_dir():
                title_containers.append(title_root)

    for title_root in title_containers:
        for group in _NAND_TITLE_GROUPS:
            candidate = (title_root / group).resolve()
            if candidate.exists() and candidate.is_dir():
                discovered.append(candidate)

    if discovered:
        return _unique_paths(discovered)

    fallbacks = [(nand_root / _ZERO_ID / "title" / group).resolve() for group in _NAND_TITLE_GROUPS]
    fallbacks.extend((nand_root / "title" / group).resolve() for group in _NAND_TITLE_GROUPS)
    return _unique_paths(fallbacks)


def azahar_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = azahar_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    raw_paths: list[Path] = []

    sdmc_root_text = settings.get("sdmc_root", "")
    if settings.get("use_virtual_sd", "true") != "false" and isinstance(sdmc_root_text, str) and sdmc_root_text.strip():
        raw_paths.extend(_existing_sdmc_title_roots(Path(sdmc_root_text).expanduser().resolve()))

    nand_root_text = settings.get("nand_root", "")
    if isinstance(nand_root_text, str) and nand_root_text.strip():
        raw_paths.extend(_existing_nand_title_roots(Path(nand_root_text).expanduser().resolve()))

    unique: list[str] = []
    seen: set[str] = set()
    for candidate in raw_paths:
        resolved = candidate.resolve()
        key = str(resolved).casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        unique.append(str(resolved))
    return unique


def azahar_state_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = azahar_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    states_root = settings.get("states_root", "")
    if not isinstance(states_root, str) or not states_root.strip():
        return []
    return [states_root.strip()]
