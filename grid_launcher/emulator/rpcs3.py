from __future__ import annotations

import configparser
import os
import re
import shutil
from pathlib import Path
from typing import Callable

from grid_launcher.core.path import xdg_config_home

_USER_ID_PATTERN = re.compile(r"^\d{8}$")


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


def _is_valid_user_id(value: str) -> bool:
    return bool(_USER_ID_PATTERN.fullmatch(value)) and value != "00000000"


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


def _launch_user_id(
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> str:
    args = _split_launch_args(launch_template, split_launch_template_args)
    for index, raw_arg in enumerate(args):
        if not isinstance(raw_arg, str) or not raw_arg.strip():
            continue

        normalized_arg = raw_arg.strip()
        lowered_arg = normalized_arg.casefold()

        if lowered_arg == "--user-id" and index + 1 < len(args):
            next_arg = args[index + 1]
            if isinstance(next_arg, str):
                candidate = next_arg.strip()
                if _is_valid_user_id(candidate):
                    return candidate
            continue

        if lowered_arg.startswith("--user-id="):
            _, candidate = normalized_arg.split("=", 1)
            candidate = candidate.strip()
            if _is_valid_user_id(candidate):
                return candidate

    return ""


def _clean_path_value(value: str) -> str:
    cleaned = value.strip()
    if cleaned and cleaned[0] not in {'"', "'"}:
        cleaned = re.split(r"\s+#", cleaned, maxsplit=1)[0].strip()
    return cleaned.strip().strip('"').strip("'")


def _yaml_scalar_value(raw_content: str, key: str) -> str:
    pattern = re.compile(rf'^\s*["\']?{re.escape(key)}["\']?\s*:\s*(.+?)\s*$', re.IGNORECASE | re.MULTILINE)
    match = pattern.search(raw_content)
    if not match:
        return ""

    raw_value = match.group(1).strip()
    if raw_value in {"", "{}", "[]", "|", ">"}:
        return ""
    return _clean_path_value(raw_value)


def _resolve_rpcs3_path(base_root: Path, raw_value: str, default_value: str = "") -> Path:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_value

    if not value:
        return base_root.resolve()

    emulator_dir_value = str(base_root.resolve()).replace("\\", "/")
    if not emulator_dir_value.endswith("/"):
        emulator_dir_value += "/"

    expanded = os.path.expandvars(value.replace("$(EmulatorDir)", emulator_dir_value))
    candidate = Path(expanded).expanduser()
    if not candidate.is_absolute():
        candidate = base_root / candidate
    return candidate.resolve()


def _ensure_yaml_section_values(
    raw_content: str,
    section_name: str,
    desired_values: dict[str, str],
) -> tuple[str, bool]:
    """Add missing keys to a YAML section. Never overwrites existing keys."""
    if not desired_values:
        return raw_content, False

    lines = raw_content.splitlines()
    output_lines: list[str] = []
    changed = False
    target_section = section_name.strip()
    in_target = False
    section_found = False
    seen_keys: set[str] = set()

    def flush_missing_keys() -> None:
        nonlocal changed
        for key, value in desired_values.items():
            if key in seen_keys:
                continue
            output_lines.append(f"  {key}: {value}")
            seen_keys.add(key)
            changed = True

    for raw_line in lines:
        section_match = re.match(r"^([A-Za-z][^:\n]*):[ \t]*$", raw_line)
        if section_match:
            if in_target:
                flush_missing_keys()
            current_section = section_match.group(1).strip()
            in_target = current_section == target_section
            if in_target:
                section_found = True
            output_lines.append(raw_line)
            continue

        if in_target:
            key_match = re.match(r"^  ([^:]+):", raw_line)
            if key_match:
                seen_keys.add(key_match.group(1).strip())

        output_lines.append(raw_line)

    if in_target:
        flush_missing_keys()

    if not section_found:
        if output_lines and output_lines[-1].strip():
            output_lines.append("")
        output_lines.append(f"{section_name}:")
        for key, value in desired_values.items():
            output_lines.append(f"  {key}: {value}")
        changed = True

    return "\n".join(output_lines).rstrip() + "\n", changed


def _ensure_rpcs3_gui_section_values(
    raw_content: str,
    section_name: str,
    desired_values: dict[str, str],
    annotate: bool = True,
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
    seen_annotations: set[str] = set()

    def _fmt(key: str, value: str) -> str:
        return f"{key} = {value}" if annotate else f"{key}={value}"

    def flush_missing_keys() -> None:
        nonlocal changed
        for key, value in desired_values.items():
            if key in seen_keys:
                continue
            if annotate and key not in seen_annotations:
                output_lines.append(f"{key}\\default=false")
                seen_annotations.add(key)
            output_lines.append(_fmt(key, value))
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
            annotation_match = re.match(r"^\s*([A-Za-z0-9_]+)\\default\s*=", raw_line)
            if annotation_match:
                key = annotation_match.group(1)
                if key in desired_values:
                    if not annotate:
                        changed = True
                        continue
                    if key in seen_annotations:
                        changed = True
                        continue
                    replacement = f"{key}\\default=false"
                    if raw_line.strip() != replacement:
                        changed = True
                    output_lines.append(replacement)
                    seen_annotations.add(key)
                    continue
                output_lines.append(raw_line)
                continue

            key_match = re.match(r"^\s*([A-Za-z0-9_]+)\s*=", raw_line)
            if key_match:
                key = key_match.group(1)
                if key in desired_values:
                    if key in seen_keys:
                        changed = True
                        continue
                    if annotate and key not in seen_annotations:
                        output_lines.append(f"{key}\\default=false")
                        seen_annotations.add(key)
                        changed = True
                    replacement = _fmt(key, desired_values[key])
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
            if annotate:
                output_lines.append(f"{key}\\default=false")
            output_lines.append(_fmt(key, value))
        changed = True

    return "\n".join(output_lines).rstrip() + "\n", changed


def rpcs3_pup_path(emulator_path_text: str) -> Path | None:
    """Return the path to PS3UPDAT.PUP in the emulator directory, or None if not found."""
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if not path_text:
        return None
    emulator_path = Path(path_text).expanduser()
    emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
    pup_path = emulator_dir / "PS3UPDAT.PUP"
    return pup_path.resolve() if pup_path.exists() and pup_path.is_file() else None


def rpcs3_data_root(emulator_path_text: str) -> Path | None:
    """Return RPCS3 data root, preferring portable/ when present."""
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if not path_text:
        return None

    emulator_path = Path(path_text).expanduser()
    if emulator_path.exists() and emulator_path.is_dir():
        emulator_dir = emulator_path
    elif emulator_path.exists() and emulator_path.is_file():
        emulator_dir = emulator_path.parent
    else:
        emulator_dir = emulator_path.parent
        if not emulator_dir.exists() or not emulator_dir.is_dir():
            return None

    portable_dir = emulator_dir / "portable"
    if portable_dir.exists() and portable_dir.is_dir():
        return portable_dir.resolve()
    return emulator_dir.resolve()


def update_rpcs3_games_yml(
    data_root: Path,
    game_id: str,
    dev_hdd0_root: Path,
    games_root: Path | None = None,
) -> bool:
    normalized_game_id = game_id.strip() if isinstance(game_id, str) else ""
    if not normalized_game_id:
        return False
    if not isinstance(data_root, Path) or not isinstance(dev_hdd0_root, Path):
        return False

    try:
        if isinstance(games_root, Path):
            game_dir = games_root / normalized_game_id
        else:
            game_dir = dev_hdd0_root / "game" / normalized_game_id

        game_dir_posix = game_dir.resolve().as_posix()
        if not game_dir_posix.endswith("/"):
            game_dir_posix += "/"

        games_yml_path = data_root / "config" / "games.yml"
        config_dir = data_root / "config"
        config_dir.mkdir(parents=True, exist_ok=True)

        existing_lines: list[str] = []
        if games_yml_path.exists() and games_yml_path.is_file():
            existing_lines = games_yml_path.read_text(encoding="utf-8").splitlines()

        updated_line = f'{normalized_game_id}: "{game_dir_posix}"'
        found = False
        next_lines: list[str] = []
        for line in existing_lines:
            stripped = line.strip()
            if ":" not in stripped:
                next_lines.append(line)
                continue
            raw_key = stripped.split(":", 1)[0].strip().strip('"').strip("'")
            if raw_key == normalized_game_id:
                next_lines.append(updated_line)
                found = True
                continue
            next_lines.append(line)

        if not found:
            next_lines.append(updated_line)

        output_text = "\n".join(next_lines)
        if next_lines:
            output_text += "\n"
        games_yml_path.write_text(output_text, encoding="utf-8")
    except OSError:
        return False

    return True


def trigger_rpcs3_firmware_install(exe_path: str, pup_path: str) -> bool:
    """Launch RPCS3 with --installfw to install PS3 firmware. Shows a GUI dialog.

    Returns True if the process was launched successfully, False otherwise.
    """
    import subprocess

    exe = Path(exe_path).expanduser().resolve()
    pup = Path(pup_path).expanduser().resolve()
    if not exe.exists() or not exe.is_file():
        return False
    if not pup.exists() or not pup.is_file():
        return False
    try:
        subprocess.Popen([str(exe), "--installfw", str(pup)], cwd=str(exe.parent))
        return True
    except OSError:
        return False


def ensure_rpcs3_vfs_settings(emulator_path_text: str, ps3_library_path: str) -> dict[str, object]:
    """Write/update portable/config/vfs.yml to redirect /dev_hdd0/ into the PS3 library folder.

    Only writes keys that are not already present, preserving any user-set VFS paths.
    Returns {"vfs_path": path_or_None, "dev_hdd0": path_str, "games": path_str, "changed": bool}.
    """
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    library_text = ps3_library_path.strip() if isinstance(ps3_library_path, str) else ""
    if not path_text or not library_text:
        return {"vfs_path": None, "dev_hdd0": "", "games": "", "changed": False}

    emulator_path = Path(path_text).expanduser()
    if not emulator_path.exists() or not emulator_path.is_file():
        return {"vfs_path": None, "dev_hdd0": "", "games": "", "changed": False}

    emulator_dir = emulator_path.parent
    portable_dir = emulator_dir / "portable"
    config_dir = portable_dir / "config"
    vfs_path = config_dir / "vfs.yml"

    library_path = Path(library_text).expanduser().resolve()
    dev_hdd0_path = library_path / ".vfs" / "dev_hdd0"
    games_path = library_path / ".vfs" / "games"
    dev_hdd0_str = dev_hdd0_path.as_posix()
    if not dev_hdd0_str.endswith("/"):
        dev_hdd0_str += "/"
    games_str = games_path.as_posix()
    if not games_str.endswith("/"):
        games_str += "/"

    desired: dict[str, str] = {
        "$(EmulatorDir)": "",
        "/dev_hdd0/": dev_hdd0_str,
        "/games/": games_str,
    }

    try:
        config_dir.mkdir(parents=True, exist_ok=True)
        existing_content = vfs_path.read_text(encoding="utf-8") if vfs_path.exists() else ""

        output_lines = existing_content.splitlines() if existing_content else []
        changed = False

        # Collect keys already present so we never overwrite them
        existing_keys: set[str] = set()
        for line in output_lines:
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            colon_pos = stripped.find(":")
            if colon_pos == -1:
                continue
            key = stripped[:colon_pos].strip().strip('"').strip("'")
            if key:
                existing_keys.add(key)

        for key, value in desired.items():
            if key in existing_keys:
                continue
            if value:
                output_lines.append(f'"{key}": "{value}"')
            else:
                output_lines.append(f'"{key}": ""')
            changed = True

        if changed:
            output_text = "\n".join(output_lines)
            if output_text and not output_text.endswith("\n"):
                output_text += "\n"
            vfs_path.write_text(output_text, encoding="utf-8")

    except OSError:
        return {"vfs_path": None, "dev_hdd0": "", "games": "", "changed": False}

    return {
        "vfs_path": str(vfs_path),
        "dev_hdd0": str(dev_hdd0_path),
        "games": str(games_path),
        "changed": changed,
    }


def ps3_vfs_dev_hdd0_path(emulator_path_text: str, ps3_library_path: str) -> Path | None:
    """Return the resolved dev_hdd0 root for PS3 game installs.

    Reads from vfs.yml if present; falls back to <ps3_library>/.vfs/dev_hdd0/.
    Returns None if neither emulator path nor library path is usable.
    """
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    library_text = ps3_library_path.strip() if isinstance(ps3_library_path, str) else ""

    # Try reading from existing vfs.yml first
    for data_root in rpcs3_data_root_candidates(path_text):
        for vfs_candidate in _vfs_path_candidates_for_root(data_root):
            if not vfs_candidate.exists() or not vfs_candidate.is_file():
                continue
            try:
                raw_content = vfs_candidate.read_text(encoding="utf-8")
            except OSError:
                continue
            raw_emulator_root = _yaml_scalar_value(raw_content, "$(EmulatorDir)")
            emulator_root = _resolve_rpcs3_path(data_root, raw_emulator_root)
            raw_dev_hdd0 = _yaml_scalar_value(raw_content, "/dev_hdd0/")
            if raw_dev_hdd0:
                return _resolve_rpcs3_path(emulator_root, raw_dev_hdd0)
            break

    # Fall back to library-derived default
    if not library_text:
        return None
    library_path = Path(library_text).expanduser().resolve()
    return library_path / ".vfs" / "dev_hdd0"


def ps3_vfs_games_path(emulator_path_text: str, ps3_library_path: str) -> Path | None:
    """Return the resolved games root for PS3 disc installs.

    Reads from vfs.yml if present; falls back to <ps3_library>/.vfs/games/.
    Returns None if neither emulator path nor library path is usable.
    """
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    library_text = ps3_library_path.strip() if isinstance(ps3_library_path, str) else ""

    for data_root in rpcs3_data_root_candidates(path_text):
        for vfs_candidate in _vfs_path_candidates_for_root(data_root):
            if not vfs_candidate.exists() or not vfs_candidate.is_file():
                continue
            try:
                raw_content = vfs_candidate.read_text(encoding="utf-8")
            except OSError:
                continue
            raw_emulator_root = _yaml_scalar_value(raw_content, "$(EmulatorDir)")
            emulator_root = _resolve_rpcs3_path(data_root, raw_emulator_root)
            raw_games = _yaml_scalar_value(raw_content, "/games/")
            if raw_games:
                return _resolve_rpcs3_path(emulator_root, raw_games)
            break

    if not library_text:
        return None
    library_path = Path(library_text).expanduser().resolve()
    return library_path / ".vfs" / "games"


def ensure_rpcs3_settings(emulator_path_text: str, ps3_library_path: str = "") -> dict[str, object]:
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if not path_text:
        return {"config_path": None, "gui_config_path": None, "changed": False}

    emulator_path = Path(path_text).expanduser()
    if not emulator_path.exists() or not emulator_path.is_file():
        return {"config_path": None, "gui_config_path": None, "changed": False}

    emulator_dir = emulator_path.parent
    portable_dir = emulator_dir / "portable"
    config_dir = portable_dir / "config"
    gui_dir = portable_dir / "GuiConfigs"
    config_path = config_dir / "config.yml"
    gui_path = gui_dir / "GuiSettings.ini"
    current_settings_path = gui_dir / "CurrentSettings.ini"

    try:
        portable_dir.mkdir(parents=True, exist_ok=True)
        config_dir.mkdir(parents=True, exist_ok=True)
        gui_dir.mkdir(parents=True, exist_ok=True)

        changed = False

        yml_content = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
        yml_content, c1 = _ensure_yaml_section_values(yml_content, "Miscellaneous", {"Start games in fullscreen mode": "true"})
        yml_content, c2 = _ensure_yaml_section_values(yml_content, "Audio", {"Master Volume": "40"})
        if c1 or c2:
            config_path.write_text(yml_content, encoding="utf-8")
            changed = True

        gui_content = gui_path.read_text(encoding="utf-8") if gui_path.exists() else ""
        gui_content, g1 = _ensure_rpcs3_gui_section_values(gui_content, "main_window", {
            "infoBoxEnabledWelcome": "false",
            "confirmationBoxExitGame": "false",
            "confirmationBoxBootGame": "false",
            "infoBoxEnabledInstallPUP": "false",
        })
        if g1:
            gui_path.write_text(gui_content, encoding="utf-8")
            changed = True

        current_content = current_settings_path.read_text(encoding="utf-8") if current_settings_path.exists() else ""
        current_content, cs1 = _ensure_rpcs3_gui_section_values(current_content, "Meta", {
            "checkUpdateStart": "false",
            "useRichPresence": "false",
        }, annotate=False)
        current_content, cs2 = _ensure_rpcs3_gui_section_values(current_content, "main_window", {
            "infoBoxEnabledWelcome": "false",
            "confirmationBoxExitGame": "false",
            "confirmationBoxBootGame": "false",
            "infoBoxEnabledInstallPUP": "false",
        }, annotate=False)
        if cs1 or cs2:
            current_settings_path.write_text(current_content, encoding="utf-8")
            changed = True

        if ps3_library_path.strip():
            vfs_result = ensure_rpcs3_vfs_settings(emulator_path_text, ps3_library_path)
            if vfs_result.get("changed"):
                changed = True

    except OSError:
        return {"config_path": None, "gui_config_path": None, "changed": False}

    return {
        "config_path": str(config_path),
        "gui_config_path": str(gui_path),
        "changed": changed,
    }


def rpcs3_data_root_candidates(emulator_path_text: str) -> list[Path]:
    candidates: list[Path] = []

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            portable_dir = emulator_dir / "portable"
            if portable_dir.exists() and portable_dir.is_dir():
                candidates.append(portable_dir.resolve())
            candidates.append(emulator_dir.resolve())

    config_env = os.environ.get("RPCS3_CONFIG_DIR", "")
    if isinstance(config_env, str) and config_env.strip():
        candidates.insert(1 if candidates else 0, Path(config_env).expanduser().resolve())

    home_path = Path.home()
    candidates.extend(
        [
            xdg_config_home() / "rpcs3",
            home_path / ".var" / "app" / "net.rpcs3.RPCS3" / "config" / "rpcs3",
            home_path / "Library" / "Application Support" / "rpcs3",
        ]
    )

    return _unique_paths(candidates)


def _vfs_path_candidates_for_root(data_root: Path) -> list[Path]:
    return _unique_paths([data_root / "config" / "vfs.yml", data_root / "vfs.yml"])


def _persistent_settings_path_candidates_for_root(data_root: Path) -> list[Path]:
    return _unique_paths([data_root / "GuiConfigs" / "persistent_settings.dat"])


def _persistent_active_user(data_root: Path) -> tuple[str, str]:
    for candidate in _persistent_settings_path_candidates_for_root(data_root):
        if not candidate.exists() or not candidate.is_file():
            continue

        parser = configparser.ConfigParser()
        try:
            parser.read(candidate, encoding="utf-8")
        except (configparser.Error, OSError, UnicodeDecodeError):
            continue

        active_user = parser.get("Users", "active_user", fallback="").strip()
        if _is_valid_user_id(active_user):
            return active_user, str(candidate)
        return "", str(candidate)

    return "", ""


def rpcs3_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    launch_user = _launch_user_id(launch_template, split_launch_template_args)
    defaults = {
        "config_path": "",
        "persistent_settings_path": "",
        "data_root": "",
        "dev_hdd0": "",
        "current_user": launch_user or "00000001",
    }

    for data_root in rpcs3_data_root_candidates(emulator_path_text):
        settings = defaults.copy()
        settings["data_root"] = str(data_root)

        persistent_user, persistent_settings_path = _persistent_active_user(data_root)
        if persistent_settings_path:
            settings["persistent_settings_path"] = persistent_settings_path
        if not launch_user and persistent_user:
            settings["current_user"] = persistent_user

        emulator_root = data_root.resolve()
        dev_hdd0_root = emulator_root / "dev_hdd0"

        for candidate in _vfs_path_candidates_for_root(data_root):
            if not candidate.exists() or not candidate.is_file():
                continue

            try:
                raw_content = candidate.read_text(encoding="utf-8")
            except OSError:
                continue

            settings["config_path"] = str(candidate)
            raw_emulator_root = _yaml_scalar_value(raw_content, "$(EmulatorDir)")
            emulator_root = _resolve_rpcs3_path(data_root, raw_emulator_root)
            raw_dev_hdd0 = _yaml_scalar_value(raw_content, "/dev_hdd0/")
            dev_hdd0_root = _resolve_rpcs3_path(emulator_root, raw_dev_hdd0, "$(EmulatorDir)dev_hdd0/")
            break

        settings["dev_hdd0"] = str(dev_hdd0_root)
        return settings

    return defaults


def rpcs3_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = rpcs3_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    dev_hdd0_path = settings.get("dev_hdd0", "")
    if not isinstance(dev_hdd0_path, str) or not dev_hdd0_path.strip():
        return []

    home_root = Path(dev_hdd0_path.strip()).expanduser() / "home"
    current_user = settings.get("current_user", "")

    raw_paths: list[Path] = []
    if isinstance(current_user, str) and _is_valid_user_id(current_user):
        raw_paths.append(home_root / current_user / "savedata")

    if home_root.exists() and home_root.is_dir():
        for child in sorted(home_root.iterdir(), key=lambda item: item.name):
            if not child.is_dir() or not _is_valid_user_id(child.name):
                continue
            raw_paths.append(child / "savedata")

    raw_paths.append(home_root / "00000001" / "savedata")

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


def copy_ps3_custom_config_to_emulator(vfs_config_dir: Path, rpcs3_data_root: Path) -> None:
    """Copy all PS3 custom config files from the platform VFS into the RPCS3 emulator config dir.

    vfs_config_dir should be <platform_vfs_root>/config.
    Files in vfs_config_dir/custom_configs/ are merged into rpcs3_data_root/config/custom_configs/.
    Silently skips if the source directory doesn't exist. Wraps OSError silently so a config
    copy failure never prevents game launch.
    """
    src_dir = vfs_config_dir / "custom_configs"
    if not src_dir.is_dir():
        return
    dest_dir = rpcs3_data_root / "config" / "custom_configs"
    try:
        dest_dir.mkdir(parents=True, exist_ok=True)
        for src_file in src_dir.iterdir():
            if src_file.is_file():
                shutil.copy2(src_file, dest_dir / src_file.name)
    except OSError:
        pass
