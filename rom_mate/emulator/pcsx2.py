from __future__ import annotations

import os
import re
import sys
from pathlib import Path
from typing import Callable

from rom_mate.core.path import xdg_config_home


def _windows_documents_folder() -> Path | None:
    """Return the user's Documents folder using the Windows Shell API.

    Uses SHGetKnownFolderPath(FOLDERID_Documents) which correctly resolves
    folder redirection (e.g. network shares, non-default locations).
    Returns None on non-Windows or if the API call fails.
    """
    if sys.platform != "win32":
        return None
    try:
        import ctypes
        import ctypes.wintypes

        class GUID(ctypes.Structure):
            _fields_ = [
                ("Data1", ctypes.c_ulong),
                ("Data2", ctypes.c_ushort),
                ("Data3", ctypes.c_ushort),
                ("Data4", ctypes.c_ubyte * 8),
            ]

        FOLDERID_Documents = GUID(
            0xFDD39AD0, 0x238F, 0x46AF,
            (ctypes.c_ubyte * 8)(0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7),
        )

        path_ptr = ctypes.c_wchar_p()
        hr = ctypes.windll.shell32.SHGetKnownFolderPath(
            ctypes.byref(FOLDERID_Documents),
            ctypes.wintypes.DWORD(0),
            ctypes.wintypes.HANDLE(0),
            ctypes.byref(path_ptr),
        )
        if hr != 0 or not path_ptr.value:
            return None
        result = Path(path_ptr.value)
        ctypes.windll.ole32.CoTaskMemFree(path_ptr)
        return result
    except (OSError, AttributeError, ValueError):
        return None


def pcsx2_windows_documents_folder() -> Path | None:
    return _windows_documents_folder()


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
            key_match = re.match(r"^\s*([A-Za-z0-9_]+)\s*=", raw_line)
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


def _section_has_key(raw_content: str, section_name: str, key_name: str) -> bool:
    target_section = section_name.casefold()
    target_key = key_name.casefold()
    in_target = False

    for raw_line in raw_content.splitlines():
        stripped = raw_line.strip()
        section_match = re.match(r"^\[(.+?)\]\s*$", stripped)
        if section_match:
            in_target = section_match.group(1).strip().casefold() == target_section
            continue
        if not in_target:
            continue

        key_match = re.match(r"^\s*([A-Za-z0-9_]+)\s*=", raw_line)
        if key_match and key_match.group(1).casefold() == target_key:
            return True

    return False


def pcsx2_config_path_candidates(emulator_path_text: str) -> list[Path]:
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    emulator_path = Path(path_text).expanduser() if path_text else Path()
    emulator_dir = emulator_path.parent

    candidates: list[Path] = []

    # Check portable mode first (portable.ini or portable.txt next to exe)
    portable_ini = emulator_dir / "portable.ini"
    portable_txt = emulator_dir / "portable.txt"
    if portable_ini.exists() or portable_txt.exists():
        candidates.append(emulator_dir / "inis" / "PCSX2.ini")

    # Default system locations
    documents = _windows_documents_folder()
    if documents is None:
        documents = Path.home() / "Documents"
    candidates.append(documents / "PCSX2" / "inis" / "PCSX2.ini")

    if sys.platform != "win32":
        xdg_config = xdg_config_home()
        if xdg_config is not None:
            candidates.append(xdg_config / "PCSX2" / "inis" / "PCSX2.ini")
        candidates.append(
            Path.home() / ".var" / "app" / "net.pcsx2.PCSX2" / "config" / "PCSX2" / "inis" / "PCSX2.ini"
        )

    candidates.append(Path.home() / ".config" / "PCSX2" / "inis" / "PCSX2.ini")
    candidates.append(Path.home() / "Library" / "Application Support" / "PCSX2" / "inis" / "PCSX2.ini")

    return candidates


def ensure_pcsx2_settings(
    emulator_path_text: str,
    *,
    enable_fullscreen: bool = False,
    retroachievements_username: str = "",
    retroachievements_token: str = "",
    bios_directory: str = "",
) -> dict:
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if not path_text:
        return {"config_path": None, "changed": False}

    emulator_path = Path(path_text).expanduser()
    if not emulator_path.exists() or not emulator_path.is_file():
        return {"config_path": None, "changed": False}

    emulator_dir = emulator_path.parent
    if sys.platform == "win32":
        portable_ini_path = emulator_dir / "portable.ini"
        if not portable_ini_path.exists():
            try:
                portable_ini_path.write_text("", encoding="utf-8")
            except OSError:
                pass
        config_path = emulator_dir / "inis" / "PCSX2.ini"
    else:
        config_candidates = pcsx2_config_path_candidates(path_text)
        if not config_candidates:
            return {"config_path": None, "changed": False}
        config_path = next((candidate for candidate in config_candidates if candidate.exists()), config_candidates[0])

    try:
        content = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
        changed = False

        content, section_changed = _ensure_section_values(
            content, "UI", {"SetupWizardIncomplete": "false", "SettingsVersion": "1"},
        )
        changed = changed or section_changed

        content, section_changed = _ensure_section_values(
            content, "AutoUpdater", {"CheckAtStartup": "false"},
        )
        changed = changed or section_changed

        content, section_changed = _ensure_section_values(
            content,
            "UI",
            {
                "InhibitScreensaver": "true",
                **({} if _section_has_key(content, "UI", "ConfirmShutdown") else {"ConfirmShutdown": "false"}),
                **({} if _section_has_key(content, "UI", "PauseOnFocusLoss") else {"PauseOnFocusLoss": "true"}),
                **({} if _section_has_key(content, "UI", "HideMouseCursor") else {"HideMouseCursor": "true"}),
            },
        )
        changed = changed or section_changed

        content, section_changed = _ensure_section_values(
            content,
            "EmuCore",
            {
                "EnableDiscordPresence": "false",
            },
        )
        changed = changed or section_changed

        emu_defaults = {}
        for key, value in (
            ("EnableWideScreenPatches", "true"),
            ("EnableNoInterlacingPatches", "true"),
        ):
            if not _section_has_key(content, "EmuCore", key):
                emu_defaults[key] = value
        if emu_defaults:
            content, section_changed = _ensure_section_values(
                content, "EmuCore", emu_defaults,
            )
            changed = changed or section_changed

        ra_user = retroachievements_username.strip() if isinstance(retroachievements_username, str) else ""
        ra_tok = retroachievements_token.strip() if isinstance(retroachievements_token, str) else ""
        if ra_user and ra_tok:
            content, section_changed = _ensure_section_values(
                content,
                "Achievements",
                {"Enabled": "true", "Username": ra_user, "Token": ra_tok},
            )
            changed = changed or section_changed

        content, section_changed = _ensure_section_values(
            content,
            "EmuCore/GS",
            {
                "pcrtc_antiblur": "true",
                "pcrtc_offsets": "false",
            },
        )
        changed = changed or section_changed

        gs_defaults = {}
        for key, value in (
            ("VsyncEnable", "true"),
            ("Renderer", "14"),
            ("filter", "2"),
            ("accurate_blending_unit", "3"),
            ("MaxAnisotropy", "4"),
            ("dithering_ps2", "2"),
            ("CASMode", "2"),
            ("CASSharpness", "50"),
            ("hw_mipmap", "true"),
            ("texture_preloading", "2"),
        ):
            if not _section_has_key(content, "EmuCore/GS", key):
                gs_defaults[key] = value
        if gs_defaults:
            content, section_changed = _ensure_section_values(
                content, "EmuCore/GS", gs_defaults,
            )
            changed = changed or section_changed

        speedhack_defaults = {}
        for key, value in (
            ("fastCDVD", "false"),
            ("vuThread", "true"),
            ("vu1Instant", "true"),
        ):
            if not _section_has_key(content, "EmuCore/Speedhacks", key):
                speedhack_defaults[key] = value
        if speedhack_defaults:
            content, section_changed = _ensure_section_values(
                content, "EmuCore/Speedhacks", speedhack_defaults,
            )
            changed = changed or section_changed

        if not _section_has_key(content, "Pad1", "Type"):
            content, section_changed = _ensure_section_values(
                content,
                "Pad1",
                {
                    "Type": "DualShock2",
                    "InvertL": "0",
                    "InvertR": "0",
                    "Deadzone": "0",
                    "AxisScale": "1.33",
                    "LargeMotorScale": "1",
                    "SmallMotorScale": "1",
                    "ButtonDeadzone": "0",
                    "PressureModifier": "0.5",
                    "Up": "SDL-0/DPadUp",
                    "Right": "SDL-0/DPadRight",
                    "Down": "SDL-0/DPadDown",
                    "Left": "SDL-0/DPadLeft",
                    "Triangle": "SDL-0/FaceNorth",
                    "Circle": "SDL-0/FaceEast",
                    "Cross": "SDL-0/FaceSouth",
                    "Square": "SDL-0/FaceWest",
                    "Select": "SDL-0/Back",
                    "Start": "SDL-0/Start",
                    "L1": "SDL-0/LeftShoulder",
                    "L2": "SDL-0/+LeftTrigger",
                    "R1": "SDL-0/RightShoulder",
                    "R2": "SDL-0/+RightTrigger",
                    "L3": "SDL-0/LeftStick",
                    "R3": "SDL-0/RightStick",
                    "LUp": "SDL-0/-LeftY",
                    "LRight": "SDL-0/+LeftX",
                    "LDown": "SDL-0/+LeftY",
                    "LLeft": "SDL-0/-LeftX",
                    "RUp": "SDL-0/-RightY",
                    "RRight": "SDL-0/+RightX",
                    "RDown": "SDL-0/+RightY",
                    "RLeft": "SDL-0/-RightX",
                    "LargeMotor": "SDL-0/LargeMotor",
                    "SmallMotor": "SDL-0/SmallMotor",
                },
            )
            changed = changed or section_changed

        if not _section_has_key(content, "Hotkeys", "OpenPauseMenu"):
            content, section_changed = _ensure_section_values(
                content, "Hotkeys", {"OpenPauseMenu": "SDL-0/Guide"},
            )
            changed = changed or section_changed

        if not _section_has_key(content, "SPU2/Output", "StandardVolume"):
            content, section_changed = _ensure_section_values(
                content, "SPU2/Output", {"StandardVolume": "40"},
            )
            changed = changed or section_changed

        if not _section_has_key(content, "EmuCore/GS", "upscale_multiplier"):
            content, section_changed = _ensure_section_values(
                content, "EmuCore/GS", {"upscale_multiplier": "3"},
            )
            changed = changed or section_changed

        if enable_fullscreen:
            content, section_changed = _ensure_section_values(
                content, "UI", {"StartFullscreen": "true"},
            )
            changed = changed or section_changed

        bios_dir = bios_directory.strip() if isinstance(bios_directory, str) else ""
        if bios_dir and not _section_has_key(content, "Folders", "Bios"):
            content, section_changed = _ensure_section_values(
                content, "Folders", {"Bios": bios_dir},
            )
            changed = changed or section_changed

        if changed:
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(content, encoding="utf-8")
    except OSError:
        return {"config_path": None, "changed": False}

    return {"config_path": config_path, "changed": changed}


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


def _portable_data_root(
    emulator_dir: Path,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> Path | None:
    args = _split_launch_args(launch_template, split_launch_template_args)
    portable_requested = any(
        isinstance(arg, str) and arg.strip().casefold() == "-portable"
        for arg in args
    )

    portable_ini_path = emulator_dir / "portable.ini"
    portable_txt_path = emulator_dir / "portable.txt"
    if not portable_requested and not portable_ini_path.exists() and not portable_txt_path.exists():
        return None

    portable_suffix = ""
    if portable_txt_path.exists() and portable_txt_path.is_file():
        try:
            portable_suffix = portable_txt_path.read_text(encoding="utf-8").strip()
        except OSError:
            portable_suffix = ""

    portable_root = (emulator_dir / portable_suffix).expanduser() if portable_suffix else emulator_dir
    return portable_root.resolve()


def pcsx2_data_root_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    portable_roots: list[Path] = []
    fallback_roots: list[Path] = []

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            # App-managed installs now always create portable.ini in ensure_pcsx2_settings().
            portable_root = _portable_data_root(emulator_dir, launch_template, split_launch_template_args)
            if portable_root is not None:
                portable_roots.append(portable_root)
            fallback_roots.append(emulator_dir.resolve())

    user_roots: list[Path] = []
    win_docs = _windows_documents_folder()
    if win_docs is not None:
        user_roots.append(win_docs / "PCSX2")
    for raw_base in (
        os.environ.get("OneDrive", ""),
        os.environ.get("USERPROFILE", ""),
        os.environ.get("HOME", ""),
    ):
        if isinstance(raw_base, str) and raw_base.strip():
            user_roots.append(Path(raw_base).expanduser() / "Documents" / "PCSX2")

    home_path = Path.home()
    user_roots.extend(
        [
            home_path / "Documents" / "PCSX2",
            home_path / ".config" / "PCSX2",
            home_path / "Library" / "Application Support" / "PCSX2",
        ]
    )

    xdg_config_home = os.environ.get("XDG_CONFIG_HOME", "").strip()
    if xdg_config_home:
        user_roots.append(Path(xdg_config_home).expanduser() / "PCSX2")

    return _unique_paths([*portable_roots, *user_roots, *fallback_roots])


def pcsx2_settings_path_candidates(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[Path]:
    return _unique_paths(
        [root / "inis" / "PCSX2.ini" for root in pcsx2_data_root_candidates(emulator_path_text, launch_template, split_launch_template_args)]
    )


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


def _parse_ini_sections(raw_content: str) -> dict[str, dict[str, str]]:
    sections: dict[str, dict[str, str]] = {}
    current_section = ""

    for raw_line in raw_content.splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith(("#", ";")):
            continue

        section_match = re.match(r"^\[(.+?)\]\s*$", stripped)
        if section_match:
            current_section = section_match.group(1).strip().casefold()
            sections.setdefault(current_section, {})
            continue

        if not current_section or "=" not in raw_line:
            continue

        key, value = raw_line.split("=", 1)
        sections.setdefault(current_section, {})[key.strip().casefold()] = value.strip()

    return sections


def pcsx2_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    defaults = {
        "config_path": "",
        "data_root": "",
        "memory_cards": "",
        "savestates": "",
        "slot1_filename": "Mcd001.ps2",
        "slot2_filename": "Mcd002.ps2",
    }

    data_roots = pcsx2_data_root_candidates(emulator_path_text, launch_template, split_launch_template_args)
    settings_candidates = pcsx2_settings_path_candidates(emulator_path_text, launch_template, split_launch_template_args)

    for root, candidate in zip(data_roots, settings_candidates, strict=False):
        if not candidate.exists() or not candidate.is_file():
            continue

        try:
            raw_content = candidate.read_text(encoding="utf-8")
        except OSError:
            continue

        sections = _parse_ini_sections(raw_content)
        folder_settings = sections.get("folders", {})
        memory_card_settings = sections.get("memorycards", {})

        defaults["config_path"] = str(candidate)
        defaults["data_root"] = str(root)
        defaults["memory_cards"] = _resolve_setting_path(root, folder_settings.get("memorycards", ""), "memcards")
        defaults["savestates"] = _resolve_setting_path(root, folder_settings.get("savestates", ""), "sstates")

        slot1_filename = _clean_path_value(memory_card_settings.get("slot1_filename", ""))
        slot2_filename = _clean_path_value(memory_card_settings.get("slot2_filename", ""))
        if slot1_filename:
            defaults["slot1_filename"] = slot1_filename
        if slot2_filename:
            defaults["slot2_filename"] = slot2_filename
        return defaults

    if data_roots:
        default_root = data_roots[0]
        defaults["data_root"] = str(default_root)
        defaults["memory_cards"] = str((default_root / "memcards").resolve())
        defaults["savestates"] = str((default_root / "sstates").resolve())

    return defaults


def pcsx2_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = pcsx2_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    memory_cards_root = settings.get("memory_cards", "")
    if not isinstance(memory_cards_root, str) or not memory_cards_root.strip():
        return []

    root_path = Path(memory_cards_root.strip()).expanduser()
    raw_paths = [
        root_path / settings.get("slot1_filename", "Mcd001.ps2"),
        root_path / settings.get("slot2_filename", "Mcd002.ps2"),
        root_path,
    ]

    unique: list[str] = []
    seen: set[str] = set()
    for candidate in raw_paths:
        key = str(candidate).casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        unique.append(str(candidate))
    return unique


def pcsx2_state_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = pcsx2_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    savestates_root = settings.get("savestates", "")
    if not isinstance(savestates_root, str) or not savestates_root.strip():
        return []
    return [savestates_root.strip()]
