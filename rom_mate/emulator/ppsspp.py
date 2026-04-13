from __future__ import annotations
import re
from pathlib import Path


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

    ini_path = emulator_dir / "memstick" / "PSP" / "SYSTEM" / "PPSSPP.INI"
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
