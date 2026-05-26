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


def ppsspp_psp_root_candidates(
    emulator_path_text: str,
    launch_template: str = "",
    split_launch_template_args: Callable[[str], list[str]] | None = None,
) -> list[Path]:
    candidates: list[Path] = []

    launch_home_root = _launch_home_root(launch_template, split_launch_template_args)
    if launch_home_root is not None:
        candidates.append((launch_home_root / "PSP").resolve())

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            candidates.append((emulator_dir / "memstick" / "PSP").resolve())

    if not path_text and launch_home_root is None and sys.platform != "win32":
        home_path = Path.home()
        candidates.append((home_path / ".config" / "ppsspp" / "PSP").resolve())
        candidates.append(
            (home_path / ".var" / "app" / "org.ppsspp.PPSSPP" / "config" / "ppsspp" / "PSP").resolve()
        )

    return _unique_paths(candidates)


def ppsspp_ini_path_candidates(
    emulator_path_text: str,
    launch_template: str = "",
    split_launch_template_args: Callable[[str], list[str]] | None = None,
) -> list[Path]:
    return _unique_paths(
        [
            root / "SYSTEM" / "PPSSPP.INI"
            for root in ppsspp_psp_root_candidates(emulator_path_text, launch_template, split_launch_template_args)
        ]
    )


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


def ensure_ppsspp_settings(
    emulator_path_text: str,
    *,
    retroachievements_username: str = "",
    retroachievements_token: str = "",
) -> dict:
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if not path_text:
        return {"changed": False}

    emulator_path = Path(path_text).expanduser()
    emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
    installed_txt = emulator_dir / "installed.txt"
    changed = False

    if installed_txt.exists():
        try:
            installed_txt.unlink()
            changed = True
        except OSError:
            pass

    if sys.platform == "win32":
        ini_path = emulator_dir / "memstick" / "PSP" / "SYSTEM" / "PPSSPP.INI"
    else:
        ini_candidates = ppsspp_ini_path_candidates(path_text)
        if not ini_candidates:
            return {"changed": changed}
        ini_path = next((candidate for candidate in ini_candidates if candidate.exists()), ini_candidates[0])

    content = ini_path.read_text(encoding="utf-8") if ini_path.exists() else ""

    sections: list[tuple[str, dict[str, str]]] = [
        ("General", {
            "CheckForNewVersion": "False",
            "SaveStateSlotCount": "3",
        }),
        ("Graphics", {
            "InternalResolution": "4",
            "MultiSampleLevel": "2",
            "Smart2DTexFiltering": "True",
            "TexScalingLevel": "4",
            "TexScalingType": "0",
            "TexDeposterize": "True",
            "TexHardwareScaling": "False",
            "TextureShader": "Off",
            "HardwareTessellation": "False",
        }),
        ("Sound", {
            "GameVolume": "25",
            "AchievementVolume": "40",
        }),
        ("Theme", {
            "ThemeName": "Slate Forest",
        }),
    ]

    ra_user = retroachievements_username.strip() if isinstance(retroachievements_username, str) else ""
    ra_tok = retroachievements_token.strip() if isinstance(retroachievements_token, str) else ""
    if ra_user and ra_tok:
        sections.append(("Achievements", {
            "AchievementsEnable": "True",
            "AchievementsUserName": ra_user,
            "AchievementsToken": ra_tok,
            "AchievementsChallengeMode": "False",
            "AchievementsLeaderboardTrackerPos": "3",
            "AchievementsLeaderboardStartedOrFailedPos": "3",
            "AchievementsLeaderboardSubmittedPos": "3",
            "AchievementsProgressPos": "3",
            "AchievementsChallengePos": "3",
            "AchievementsUnlockedPos": "4",
        }))

    any_ini_changed = False
    for section_name, values in sections:
        content, section_changed = _ensure_section_values(content, section_name, values)
        if section_changed:
            any_ini_changed = True

    if any_ini_changed:
        try:
            ini_path.parent.mkdir(parents=True, exist_ok=True)
            ini_path.write_text(content, encoding="utf-8")
            changed = True
        except OSError:
            pass

    if ra_user and ra_tok:
        dat_path = ini_path.parent / "ppsspp_retroachievements.dat"
        existing_dat = dat_path.read_text(encoding="utf-8").strip() if dat_path.exists() else ""
        if existing_dat != ra_tok:
            try:
                dat_path.parent.mkdir(parents=True, exist_ok=True)
                dat_path.write_text(ra_tok, encoding="utf-8")
                changed = True
            except OSError:
                pass

    return {"changed": changed}
