from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable

from ..core.path import sanitize_path_component


EmulatorEntry = dict[str, str]


def emulator_install_directory(library_path: Path | str, emulator_name: str) -> Path:
    library_root = library_path.expanduser() if isinstance(library_path, Path) else Path(str(library_path)).expanduser()
    safe_emulator_name = sanitize_path_component(str(emulator_name), "emulator")
    return library_root / "Emulators" / safe_emulator_name


def select_emulator_executable_path(
    game: dict[str, str],
    archive_path: Path,
    *,
    launchable_emulator_file: Callable[[Path], bool],
) -> str:
    title_text = game.get("title", "")
    title = title_text.strip().casefold() if isinstance(title_text, str) else ""
    title_tokens = [token for token in re.split(r"[^a-z0-9]+", title) if len(token) > 2]
    preferred_executable_names: set[str] = set()
    if "nintendo switch" in title or "switch" in title:
        preferred_executable_names.add("eden.exe")
    if "nintendo 3ds" in title or "3ds" in title:
        preferred_executable_names.add("azahar.exe")

    extracted_path_fallback = ""

    extracted_dir_value = game.get("extracted_dir", "")
    extracted_dir = None
    if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
        candidate_dir = Path(extracted_dir_value).expanduser()
        if candidate_dir.exists() and candidate_dir.is_dir():
            extracted_dir = candidate_dir

    extracted_path_value = game.get("extracted_path", "")
    if isinstance(extracted_path_value, str) and extracted_path_value.strip():
        extracted_path = Path(extracted_path_value).expanduser()
        if extracted_path.exists() and extracted_path.is_file() and launchable_emulator_file(extracted_path):
            extracted_path_candidate = str(extracted_path)
            extracted_path_name = extracted_path.name.casefold()
            if (
                not preferred_executable_names
                or extracted_path_name in preferred_executable_names
                or extracted_dir is None
            ):
                return extracted_path_candidate
            extracted_path_fallback = extracted_path_candidate

    if extracted_dir is not None:
            candidates = [
                candidate
                for candidate in extracted_dir.rglob("*")
                if candidate.is_file() and launchable_emulator_file(candidate)
            ]
            if candidates:
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

    if extracted_path_fallback:
        return extracted_path_fallback

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


def _resolved_autoprofiles(
    autoprofiles: list[dict[str, Any]] | None = None,
    profiles: list[dict[str, Any]] | None = None,
) -> list[dict[str, Any]]:
    resolved_profiles = autoprofiles if isinstance(autoprofiles, list) else profiles
    return resolved_profiles if isinstance(resolved_profiles, list) else []


def emulator_source_registry_for_profile(
    profile: dict[str, Any] | None,
) -> dict[str, Any] | None:
    if not isinstance(profile, dict):
        return None
    source_value = profile.get("source")
    if not isinstance(source_value, dict):
        return None
    provider_value = source_value.get("provider", "")
    provider = provider_value.strip() if isinstance(provider_value, str) else ""
    if not provider:
        return None
    return dict(source_value)


def emulator_source_registry_for_name(
    emulator_name: str,
    autoprofiles: list[dict[str, Any]] | None = None,
    profiles: list[dict[str, Any]] | None = None,
) -> dict[str, Any] | None:
    selected_profile = _selected_manual_autoprofile(
        emulator_name,
        None,
        None,
        _resolved_autoprofiles(autoprofiles, profiles),
    )
    return emulator_source_registry_for_profile(selected_profile)


def emulator_source_registry_for_entry(
    emulator: EmulatorEntry,
    autoprofiles: list[dict[str, Any]],
    *,
    emulator_profile_for_entry: Callable[[dict[str, str], list[dict[str, Any]]], dict[str, Any] | None],
) -> dict[str, Any] | None:
    profile = emulator_profile_for_entry(emulator, autoprofiles)
    return emulator_source_registry_for_profile(profile)


def _selected_manual_autoprofile(
    selected_name: str,
    selected_profile: dict[str, Any] | None,
    profile: dict[str, Any] | None,
    all_profiles: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if isinstance(selected_profile, dict):
        return selected_profile
    if isinstance(profile, dict):
        return profile
    if not selected_name:
        return None

    normalized_selected_name = selected_name.casefold()
    for candidate in all_profiles:
        if not isinstance(candidate, dict):
            continue
        candidate_name = candidate.get("name", "")
        if isinstance(candidate_name, str) and candidate_name.strip().casefold() == normalized_selected_name:
            return candidate
    return None


def manual_emulator_autofill_suggestions(
    prefix: str = "",
    typed_prefix: str = "",
    query: str = "",
    text: str = "",
    profiles: list[dict[str, Any]] | None = None,
    autoprofiles: list[dict[str, Any]] | None = None,
) -> list[str]:
    resolved_prefix = ""
    for value in (prefix, typed_prefix, query, text):
        if isinstance(value, str) and value.strip():
            resolved_prefix = value.strip()
            break
    if not resolved_prefix:
        return []

    normalized_prefix = resolved_prefix.casefold()
    suggestions: list[str] = []
    seen_names: set[str] = set()
    for profile in _resolved_autoprofiles(autoprofiles, profiles):
        if not isinstance(profile, dict):
            continue
        profile_name = profile.get("name", "")
        if not isinstance(profile_name, str):
            continue
        normalized_name = profile_name.strip()
        if not normalized_name:
            continue
        normalized_key = normalized_name.casefold()
        if normalized_key in seen_names:
            continue
        if normalized_name.casefold().startswith(normalized_prefix):
            seen_names.add(normalized_key)
            suggestions.append(normalized_name)
    return suggestions


def manual_entry_from_autofill_suggestion(
    selected_suggestion: str = "",
    suggestion_name: str = "",
    profile_name: str = "",
    selected_profile: dict[str, Any] | None = None,
    profile: dict[str, Any] | None = None,
    entry: EmulatorEntry | None = None,
    manual_entry: EmulatorEntry | None = None,
    emulator_entry: EmulatorEntry | None = None,
    profiles: list[dict[str, Any]] | None = None,
    autoprofiles: list[dict[str, Any]] | None = None,
    *,
    normalize_save_strategy_value: Callable[[str], str],
) -> EmulatorEntry:
    resolved_entry = dict(manual_entry or entry or emulator_entry or {})
    resolved_profiles = _resolved_autoprofiles(autoprofiles, profiles)

    resolved_selected_name = ""
    for value in (selected_suggestion, suggestion_name, profile_name):
        if isinstance(value, str) and value.strip():
            resolved_selected_name = value.strip()
            break

    selected = _selected_manual_autoprofile(
        resolved_selected_name,
        selected_profile,
        profile,
        resolved_profiles,
    )
    if not isinstance(selected, dict):
        return resolved_entry

    selected_name = selected.get("name", "")
    if isinstance(selected_name, str) and selected_name.strip():
        resolved_entry["name"] = selected_name.strip()
    elif resolved_selected_name:
        resolved_entry["name"] = resolved_selected_name

    return apply_manual_emulator_profile_defaults(
        resolved_entry,
        resolved_profiles,
        emulator_profile_for_entry=lambda _entry, _profiles: selected,
        normalize_save_strategy_value=normalize_save_strategy_value,
    )


def apply_manual_emulator_profile_defaults(
    emulator: EmulatorEntry,
    autoprofiles: list[dict[str, Any]],
    *,
    emulator_profile_for_entry: Callable[[dict[str, str], list[dict[str, Any]]], dict[str, Any] | None],
    normalize_save_strategy_value: Callable[[str], str],
) -> EmulatorEntry:
    resolved_emulator = dict(emulator)
    profile = emulator_profile_for_entry(resolved_emulator, autoprofiles)
    if not isinstance(profile, dict):
        return resolved_emulator

    current_name = resolved_emulator.get("name", "")
    if not isinstance(current_name, str) or not current_name.strip():
        profile_name = profile.get("name", "")
        if isinstance(profile_name, str) and profile_name.strip():
            resolved_emulator["name"] = profile_name.strip()

    current_args = resolved_emulator.get("args", "%rom%")
    if not isinstance(current_args, str) or not current_args.strip() or current_args.strip() == "%rom%":
        profile_args = profile.get("args", "%rom%")
        if isinstance(profile_args, str) and profile_args.strip():
            resolved_emulator["args"] = profile_args.strip()

    current_save_strategy = resolved_emulator.get("save_strategy", "")
    current_save_strategy_value = normalize_save_strategy_value(current_save_strategy) if isinstance(current_save_strategy, str) else "auto"
    if current_save_strategy_value == "auto":
        profile_save_strategy = normalize_save_strategy_value(str(profile.get("save_strategy", "auto")))
        resolved_emulator["save_strategy"] = profile_save_strategy

    field_key_map = {
        "ignore_files": "ignore_files",
        "ignore_extensions": "ignore_extensions",
        "save_paths": "save_directories",
        "state_paths": "state_directories",
    }
    for emulator_key, profile_key in field_key_map.items():
        current_value = resolved_emulator.get(emulator_key, "")
        if isinstance(current_value, str) and current_value.strip():
            continue
        resolved_emulator[emulator_key] = _multiline_profile_value(profile, profile_key)

    return resolved_emulator


def defaults_for_manual_emulator_entry(
    manual_entry: EmulatorEntry | None = None,
    autoprofiles: list[dict[str, Any]] | None = None,
    entry: EmulatorEntry | None = None,
    emulator_entry: EmulatorEntry | None = None,
    profiles: list[dict[str, Any]] | None = None,
    *,
    emulator_profile_for_entry: Callable[[dict[str, str], list[dict[str, Any]]], dict[str, Any] | None] | None = None,
    emulator_entry_matches_tokens: Callable[[dict[str, str], set[str]], bool] | None = None,
    normalize_save_strategy_value: Callable[[str], str],
) -> EmulatorEntry:
    resolved_entry = manual_entry or entry or emulator_entry or {}
    resolved_profiles = autoprofiles if isinstance(autoprofiles, list) else profiles
    if not isinstance(resolved_profiles, list):
        resolved_profiles = []

    if callable(emulator_profile_for_entry):
        return apply_manual_emulator_profile_defaults(
            resolved_entry,
            resolved_profiles,
            emulator_profile_for_entry=emulator_profile_for_entry,
            normalize_save_strategy_value=normalize_save_strategy_value,
        )

    def _profile_for_entry(candidate: dict[str, str], all_profiles: list[dict[str, Any]]) -> dict[str, Any] | None:
        if not callable(emulator_entry_matches_tokens):
            return None
        for profile in all_profiles:
            if not isinstance(profile, dict):
                continue
            tokens = profile.get("match_tokens", [])
            if not isinstance(tokens, list):
                continue
            normalized_tokens = {
                token.strip().casefold()
                for token in tokens
                if isinstance(token, str) and token.strip()
            }
            if normalized_tokens and emulator_entry_matches_tokens(candidate, normalized_tokens):
                return profile
        return None

    return apply_manual_emulator_profile_defaults(
        resolved_entry,
        resolved_profiles,
        emulator_profile_for_entry=_profile_for_entry,
        normalize_save_strategy_value=normalize_save_strategy_value,
    )


def defaulted_manual_emulator_entry(
    manual_entry: EmulatorEntry | None = None,
    autoprofiles: list[dict[str, Any]] | None = None,
    entry: EmulatorEntry | None = None,
    emulator_entry: EmulatorEntry | None = None,
    profiles: list[dict[str, Any]] | None = None,
    *,
    emulator_profile_for_entry: Callable[[dict[str, str], list[dict[str, Any]]], dict[str, Any] | None] | None = None,
    emulator_entry_matches_tokens: Callable[[dict[str, str], set[str]], bool] | None = None,
    normalize_save_strategy_value: Callable[[str], str],
) -> EmulatorEntry:
    return defaults_for_manual_emulator_entry(
        manual_entry,
        autoprofiles,
        entry=entry,
        emulator_entry=emulator_entry,
        profiles=profiles,
        emulator_profile_for_entry=emulator_profile_for_entry,
        emulator_entry_matches_tokens=emulator_entry_matches_tokens,
        normalize_save_strategy_value=normalize_save_strategy_value,
    )


def assign_profile_platform_defaults(
    game: dict[str, str] | None,
    emulator_name: str,
    profile: dict[str, Any],
    defaults: dict[str, str],
    core_defaults: dict[str, str],
    *,
    is_retroarch_emulator_name: Callable[[str], bool],
    default_assignable_server_platforms: Callable[[], list[str]],
    installed_retroarch_cores_for_platform: Callable[[str, str], list[str]],
    matching_platforms_for_emulator_keywords: Callable[[list[str]], list[str]],
    dolphin_variant_label_for_game: Callable[[dict[str, str]], str],
    dolphin_target_platforms_for_variant: Callable[[str], list[str]],
) -> tuple[dict[str, str], dict[str, str]]:
    resolved_defaults = dict(defaults)
    resolved_core_defaults = dict(core_defaults)

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
        if game and isinstance(profile_name, str) and profile_name.strip().casefold() == "dolphin":
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

    return resolved_defaults, resolved_core_defaults


def assign_default_platforms_for_manual_emulator(
    emulator_name: str,
    profile: dict[str, Any],
    defaults: dict[str, str] | None = None,
    platform_defaults: dict[str, str] | None = None,
    core_defaults: dict[str, str] | None = None,
    assignable_platforms: list[str] | None = None,
    *,
    is_retroarch_emulator_name: Callable[[str], bool],
    default_assignable_server_platforms: Callable[[], list[str]],
    installed_retroarch_cores_for_platform: Callable[[str, str], list[str]],
    matching_platforms_for_emulator_keywords: Callable[..., list[str]],
) -> tuple[dict[str, str], dict[str, str]]:
    resolved_defaults = defaults if isinstance(defaults, dict) else platform_defaults
    if not isinstance(resolved_defaults, dict):
        resolved_defaults = {}
    resolved_core_defaults = dict(core_defaults) if isinstance(core_defaults, dict) else {}

    if isinstance(assignable_platforms, list):
        default_platform_resolver = lambda: list(assignable_platforms)
        matching_platform_resolver = lambda keywords: matching_platforms_for_emulator_keywords(assignable_platforms, keywords)
    else:
        default_platform_resolver = default_assignable_server_platforms
        matching_platform_resolver = matching_platforms_for_emulator_keywords

    return assign_profile_platform_defaults(
        None,
        emulator_name,
        profile,
        resolved_defaults,
        resolved_core_defaults,
        is_retroarch_emulator_name=is_retroarch_emulator_name,
        default_assignable_server_platforms=default_platform_resolver,
        installed_retroarch_cores_for_platform=installed_retroarch_cores_for_platform,
        matching_platforms_for_emulator_keywords=matching_platform_resolver,
        dolphin_variant_label_for_game=lambda _game: "",
        dolphin_target_platforms_for_variant=lambda _variant: [],
    )


def assign_default_platforms_for_emulator(
    emulator_name: str,
    profile: dict[str, Any],
    defaults: dict[str, str] | None = None,
    platform_defaults: dict[str, str] | None = None,
    core_defaults: dict[str, str] | None = None,
    assignable_platforms: list[str] | None = None,
    *,
    is_retroarch_emulator_name: Callable[[str], bool],
    default_assignable_server_platforms: Callable[[], list[str]],
    installed_retroarch_cores_for_platform: Callable[[str, str], list[str]],
    matching_platforms_for_emulator_keywords: Callable[..., list[str]],
) -> tuple[dict[str, str], dict[str, str]]:
    return assign_default_platforms_for_manual_emulator(
        emulator_name,
        profile,
        defaults,
        platform_defaults=platform_defaults,
        core_defaults=core_defaults,
        assignable_platforms=assignable_platforms,
        is_retroarch_emulator_name=is_retroarch_emulator_name,
        default_assignable_server_platforms=default_assignable_server_platforms,
        installed_retroarch_cores_for_platform=installed_retroarch_cores_for_platform,
        matching_platforms_for_emulator_keywords=matching_platforms_for_emulator_keywords,
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

    resolved_defaults, resolved_core_defaults = assign_profile_platform_defaults(
        game,
        emulator_name,
        profile,
        resolved_defaults,
        resolved_core_defaults,
        is_retroarch_emulator_name=is_retroarch_emulator_name,
        default_assignable_server_platforms=default_assignable_server_platforms,
        installed_retroarch_cores_for_platform=installed_retroarch_cores_for_platform,
        matching_platforms_for_emulator_keywords=matching_platforms_for_emulator_keywords,
        dolphin_variant_label_for_game=dolphin_variant_label_for_game,
        dolphin_target_platforms_for_variant=dolphin_target_platforms_for_variant,
    )

    return resolved_emulators, resolved_defaults, resolved_core_defaults
