from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable


EmulatorEntry = dict[str, str]


def select_emulator_executable_path(
    game: dict[str, str],
    archive_path: Path,
    *,
    launchable_emulator_file: Callable[[Path], bool],
) -> str:
    extracted_path_value = game.get("extracted_path", "")
    if isinstance(extracted_path_value, str) and extracted_path_value.strip():
        extracted_path = Path(extracted_path_value).expanduser()
        if extracted_path.exists() and extracted_path.is_file() and launchable_emulator_file(extracted_path):
            return str(extracted_path)

    extracted_dir_value = game.get("extracted_dir", "")
    if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
        extracted_dir = Path(extracted_dir_value).expanduser()
        if extracted_dir.exists() and extracted_dir.is_dir():
            candidates = [
                candidate
                for candidate in extracted_dir.rglob("*")
                if candidate.is_file() and launchable_emulator_file(candidate)
            ]
            if candidates:
                title_text = game.get("title", "")
                title = title_text.strip().casefold() if isinstance(title_text, str) else ""
                title_tokens = [token for token in re.split(r"[^a-z0-9]+", title) if len(token) > 2]
                preferred_executable_names: set[str] = set()
                if "nintendo switch" in title or "switch" in title:
                    preferred_executable_names.add("eden.exe")
                if "nintendo 3ds" in title or "3ds" in title:
                    preferred_executable_names.add("azahar.exe")

                def score(candidate: Path) -> tuple[int, int, int, int, str]:
                    candidate_file_name = candidate.name.casefold()
                    preferred_name = 0 if candidate_file_name in preferred_executable_names else 1
                    candidate_name = candidate.stem.casefold()
                    token_hits = sum(1 for token in title_tokens if token in candidate_name)
                    preferred_binary = 0 if candidate.suffix.casefold() == ".exe" else 1
                    return (
                        preferred_name,
                        -token_hits,
                        preferred_binary,
                        len(candidate.parts),
                        str(candidate).casefold(),
                    )

                candidates.sort(key=score)
                return str(candidates[0])

    if archive_path.exists() and archive_path.is_file() and launchable_emulator_file(archive_path):
        return str(archive_path)

    return ""


def auto_configured_emulator_name(
    base_name: str,
    game: dict[str, str],
    *,
    dolphin_variant_label_for_game: Callable[[dict[str, str]], str],
) -> str:
    normalized_name = base_name.strip()
    if normalized_name.casefold() != "dolphin":
        return normalized_name

    variant = dolphin_variant_label_for_game(game)
    if not variant:
        return normalized_name
    return f"{normalized_name} ({variant})"


def _multiline_profile_value(profile: dict[str, Any], key: str) -> str:
    return ";\n".join(
        item.strip()
        for item in profile.get(key, [])
        if isinstance(item, str) and item.strip()
    )


def auto_configure_emulator_settings(
    game: dict[str, str],
    executable_path: str,
    profile: dict[str, Any],
    emulators: list[EmulatorEntry],
    defaults: dict[str, str],
    core_defaults: dict[str, str],
    *,
    auto_configured_emulator_name: Callable[[str, dict[str, str]], str],
    normalize_save_strategy_value: Callable[[str], str],
    is_retroarch_emulator_name: Callable[[str], bool],
    default_assignable_server_platforms: Callable[[], list[str]],
    installed_retroarch_cores_for_platform: Callable[[str, str], list[str]],
    matching_platforms_for_emulator_keywords: Callable[[list[str]], list[str]],
    dolphin_variant_label_for_game: Callable[[dict[str, str]], str],
    dolphin_target_platforms_for_variant: Callable[[str], list[str]],
) -> tuple[list[EmulatorEntry], dict[str, str], dict[str, str]]:
    emulator_name = auto_configured_emulator_name(str(profile.get("name", "Emulator")), game)
    resolved_emulators = [dict(emulator) for emulator in emulators]
    resolved_defaults = dict(defaults)
    resolved_core_defaults = dict(core_defaults)

    args_value = profile.get("args", "%rom%")
    args_template = args_value.strip() if isinstance(args_value, str) else "%rom%"
    args_template = args_template or "%rom%"
    profile_save_strategy = normalize_save_strategy_value(str(profile.get("save_strategy", "auto")))
    profile_ignore_files = _multiline_profile_value(profile, "ignore_files")
    profile_ignore_extensions = _multiline_profile_value(profile, "ignore_extensions")
    profile_save_paths = _multiline_profile_value(profile, "save_directories")
    profile_state_paths = _multiline_profile_value(profile, "state_directories")

    target_index = -1
    for index, emulator in enumerate(resolved_emulators):
        existing_name = emulator.get("name", "")
        if isinstance(existing_name, str) and existing_name.strip().casefold() == emulator_name.casefold():
            target_index = index
            break

    if target_index >= 0:
        existing = resolved_emulators[target_index]
        existing_args = existing.get("args", "%rom%")
        existing_save_strategy = existing.get("save_strategy", "")
        existing_ignore_files = existing.get("ignore_files", "")
        existing_ignore_extensions = existing.get("ignore_extensions", "")
        existing_save_paths = existing.get("save_paths", "")
        existing_state_paths = existing.get("state_paths", "")
        should_update_args = (
            is_retroarch_emulator_name(emulator_name)
            or not isinstance(existing_args, str)
            or not existing_args.strip()
            or existing_args.strip() == "%rom%"
        )
        resolved_emulators[target_index] = {
            "name": emulator_name,
            "path": executable_path,
            "args": args_template if should_update_args else existing_args.strip(),
            "save_strategy": (
                normalize_save_strategy_value(existing_save_strategy)
                if isinstance(existing_save_strategy, str) and existing_save_strategy.strip()
                else profile_save_strategy
            ),
            "ignore_files": (
                existing_ignore_files.strip()
                if isinstance(existing_ignore_files, str) and existing_ignore_files.strip()
                else profile_ignore_files
            ),
            "ignore_extensions": (
                existing_ignore_extensions.strip()
                if isinstance(existing_ignore_extensions, str) and existing_ignore_extensions.strip()
                else profile_ignore_extensions
            ),
            "save_paths": (
                existing_save_paths.strip()
                if isinstance(existing_save_paths, str) and existing_save_paths.strip()
                else profile_save_paths
            ),
            "state_paths": (
                existing_state_paths.strip()
                if isinstance(existing_state_paths, str) and existing_state_paths.strip()
                else profile_state_paths
            ),
        }
    else:
        resolved_emulators.append(
            {
                "name": emulator_name,
                "path": executable_path,
                "args": args_template,
                "save_strategy": profile_save_strategy,
                "ignore_files": profile_ignore_files,
                "ignore_extensions": profile_ignore_extensions,
                "save_paths": profile_save_paths,
                "state_paths": profile_state_paths,
            }
        )

    profile_all_platforms = bool(profile.get("all_platforms", False))
    if profile_all_platforms:
        target_platforms = default_assignable_server_platforms()
        if is_retroarch_emulator_name(emulator_name):
            target_platforms = [
                platform
                for platform in target_platforms
                if installed_retroarch_cores_for_platform(platform, emulator_name)
            ]
    else:
        platform_keywords = profile.get("platform_keywords", [])
        target_platforms = matching_platforms_for_emulator_keywords(platform_keywords if isinstance(platform_keywords, list) else [])
        profile_name = profile.get("name", "")
        if isinstance(profile_name, str) and profile_name.strip().casefold() == "dolphin":
            variant = dolphin_variant_label_for_game(game)
            variant_platforms = dolphin_target_platforms_for_variant(variant)
            if variant_platforms:
                target_platforms = variant_platforms

    for platform in target_platforms:
        current_value = resolved_defaults.get(platform, "")
        current_default = current_value.strip() if isinstance(current_value, str) else ""
        if not current_default:
            resolved_defaults[platform] = emulator_name
            continue
        if not is_retroarch_emulator_name(emulator_name) and is_retroarch_emulator_name(current_default):
            resolved_defaults[platform] = emulator_name

    if is_retroarch_emulator_name(emulator_name):
        for platform in target_platforms:
            if resolved_defaults.get(platform, "").strip().casefold() != emulator_name.casefold():
                continue
            existing_core = resolved_core_defaults.get(platform, "")
            if isinstance(existing_core, str) and existing_core.strip():
                continue
            cores = installed_retroarch_cores_for_platform(platform, emulator_name)
            if cores:
                resolved_core_defaults[platform] = cores[0]

    return resolved_emulators, resolved_defaults, resolved_core_defaults
