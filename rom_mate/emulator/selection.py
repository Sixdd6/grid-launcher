from __future__ import annotations

import re
from pathlib import Path
from typing import Callable


EmulatorEntry = dict[str, str]


def is_arcade_platform(game: dict[str, str]) -> bool:
    platform_value = game.get("platform", "")
    platform = platform_value.strip().lower() if isinstance(platform_value, str) else ""
    arcade_tokens = ("arcade", "mame", "fbneo", "final burn")
    return any(token in platform for token in arcade_tokens)


def is_ps3_platform(game: dict[str, str]) -> bool:
    platform_value = game.get("platform", "")
    platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
    return platform in {"playstation 3", "ps3"}


def is_ps4_platform(game: dict[str, str]) -> bool:
    platform_value = game.get("platform", "")
    platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
    normalized = re.sub(r"[^a-z0-9]+", " ", platform).strip()
    compact = normalized.replace(" ", "")
    tokens = set(normalized.split())

    if not normalized:
        return False
    if normalized in {"playstation 4", "ps4"}:
        return True
    if "ps4" in tokens:
        return True
    return "playstation4" in compact


def is_original_xbox_platform(game: dict[str, str]) -> bool:
    platform_value = game.get("platform", "")
    platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
    normalized = re.sub(r"[^a-z0-9]+", " ", platform).strip()
    compact = normalized.replace(" ", "")
    tokens = set(normalized.split())

    if not normalized or "xbox" not in tokens:
        return False
    if "xbox360" in compact or "360" in tokens or "one" in tokens or "series" in tokens:
        return False
    return True


def is_xbox360_platform(game: dict[str, str]) -> bool:
    platform_value = game.get("platform", "")
    platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
    normalized = re.sub(r"[^a-z0-9]+", " ", platform).strip()
    compact = normalized.replace(" ", "")
    tokens = set(normalized.split())

    if not normalized:
        return False
    if "xbox" not in tokens and "xbox360" not in compact:
        return False
    if "xbox360" in compact or "360" in tokens:
        return True
    return False


def cloud_save_scope_for_game(
    game: dict[str, str],
    *,
    emulator_name: str = "",
    is_xemu_emulator_name: Callable[[str], bool] | None = None,
    is_redream_emulator_name: Callable[[str], bool] | None = None,
    is_retroarch_emulator_name: Callable[[str], bool] | None = None,
    retroarch_core_flags: dict[str, bool] | None = None,
    save_type: str = "save",
) -> str:
    del game
    if save_type != "save":
        return "per-game"

    if (
        emulator_name.strip()
        and callable(is_xemu_emulator_name)
        and is_xemu_emulator_name(emulator_name)
    ):
        return "shared-single"

    if (
        emulator_name.strip()
        and callable(is_redream_emulator_name)
        and is_redream_emulator_name(emulator_name)
    ):
        return "shared-slotted"

    if (
        emulator_name.strip()
        and callable(is_retroarch_emulator_name)
        and is_retroarch_emulator_name(emulator_name)
        and isinstance(retroarch_core_flags, dict)
        and retroarch_core_flags.get("vmu_shared_saves", False)
    ):
        return "shared-slotted"

    return "per-game"


def cloud_save_block_reason_for_game(
    game: dict[str, str],
    *,
    is_native_executable_platform: Callable[[dict[str, str]], bool],
    emulator_name: str = "",
    is_xemu_emulator_name: Callable[[str], bool] | None = None,
    is_redream_emulator_name: Callable[[str], bool] | None = None,
    is_retroarch_emulator_name: Callable[[str], bool] | None = None,
    retroarch_core_flags: dict[str, bool] | None = None,
    save_type: str = "save",
) -> str:
    del is_xemu_emulator_name
    del is_redream_emulator_name

    if is_native_executable_platform(game):
        return "Cloud save management is only available for emulator-based games."

    if (
        save_type == "state"
        and callable(is_retroarch_emulator_name)
        and emulator_name
        and is_retroarch_emulator_name(emulator_name)
        and isinstance(retroarch_core_flags, dict)
    ):
        if not retroarch_core_flags.get("supports_save_states", True):
            return "This core does not support save states."
        if not retroarch_core_flags.get("cloud_sync_safe", True):
            return "Save state format for this core may not be stable across devices."

    if (
        save_type == "save"
        and callable(is_retroarch_emulator_name)
        and emulator_name
        and is_retroarch_emulator_name(emulator_name)
        and isinstance(retroarch_core_flags, dict)
    ):
        if not retroarch_core_flags.get("supports_saves", True):
            return "This core does not support battery saves."

    return ""


def is_emulators_platform(game: dict[str, str]) -> bool:
    platform_value = game.get("platform", "")
    if not isinstance(platform_value, str):
        return False
    return platform_value.strip().casefold() == "emulators"


def is_native_executable_platform(game: dict[str, str]) -> bool:
    platform_value = game.get("platform", "")
    if not isinstance(platform_value, str):
        return False
    platform = platform_value.strip().casefold()
    return platform in {"windows", "windows 9x"}


def is_rpcs3_emulator_name(emulator_name: str) -> bool:
    return "rpcs3" in emulator_name.strip().casefold()


def default_assignable_server_platforms(platforms: list[str]) -> list[str]:
    hidden_platforms = {"windows", "windows 9x", "emulators"}
    return [
        platform
        for platform in platforms
        if isinstance(platform, str) and platform.strip().casefold() not in hidden_platforms
    ]


def dolphin_variant_label_for_game(game: dict[str, str]) -> str:
    candidates: list[str] = []
    for field in ("title", "platform", "rom_file_name"):
        value = game.get(field, "")
        if isinstance(value, str) and value.strip():
            candidates.append(value.strip())

    if not candidates:
        return ""

    combined = " ".join(candidates).casefold()
    normalized = re.sub(r"[^a-z0-9]+", " ", combined).strip()
    compact = normalized.replace(" ", "")
    tokens = set(normalized.split())

    if "gamecube" in compact:
        return "GameCube"
    if "wiiu" in compact:
        return ""
    if "wii" in tokens:
        return "Wii"
    return ""


def dolphin_target_platforms_for_variant(variant: str, platforms: list[str]) -> list[str]:
    selected_variant = variant.strip().casefold()
    if selected_variant not in {"gamecube", "wii"}:
        return []

    matches: list[str] = []
    for platform in platforms:
        normalized = re.sub(r"[^a-z0-9]+", " ", platform.casefold()).strip()
        compact = normalized.replace(" ", "")
        tokens = set(normalized.split())

        if selected_variant == "gamecube":
            if "gamecube" in compact:
                matches.append(platform)
            continue

        if "wii" in tokens and "wiiu" not in compact:
            matches.append(platform)

    return matches


def mapping_value_for_platform(mapping: dict[str, str], platform: str) -> str:
    target = platform.strip()
    if not target:
        return ""

    direct = mapping.get(target, "")
    if isinstance(direct, str) and direct.strip():
        return direct.strip()

    folded = target.casefold()
    for key, value in mapping.items():
        if not isinstance(key, str) or not isinstance(value, str):
            continue
        if key.strip().casefold() != folded:
            continue
        if value.strip():
            return value.strip()
    return ""


def emulator_entry_by_name(emulators: list[EmulatorEntry], emulator_name: str) -> EmulatorEntry | None:
    target = emulator_name.strip().lower()
    if not target:
        return None
    for emulator in emulators:
        name = emulator.get("name", "")
        if isinstance(name, str) and name.strip().lower() == target:
            return emulator
    return None


def emulator_entry_has_usable_path(emulator: EmulatorEntry) -> bool:
    path_value = emulator.get("path", "")
    emulator_path_text = path_value.strip() if isinstance(path_value, str) else ""
    if not emulator_path_text:
        return False
    emulator_path = Path(emulator_path_text).expanduser()
    return emulator_path.exists() and emulator_path.is_file()


def compatible_emulator_names_for_platform(
    emulators: list[EmulatorEntry],
    platform: str,
    emulator_supports_platform: Callable[[EmulatorEntry, str], bool],
) -> list[str]:
    compatible: list[str] = []
    for emulator in emulators:
        name_value = emulator.get("name", "")
        name = name_value.strip() if isinstance(name_value, str) else ""
        if not name:
            continue
        if emulator_supports_platform(emulator, platform):
            compatible.append(name)
    return compatible


def default_emulator_name_for_platform(
    platform: str,
    defaults: dict[str, str],
    emulators: list[EmulatorEntry],
    emulator_supports_platform: Callable[[EmulatorEntry, str], bool],
    compatible_names: list[str] | None = None,
) -> str:
    configured = mapping_value_for_platform(defaults, platform)
    if configured:
        configured_entry = emulator_entry_by_name(emulators, configured)
        if configured_entry is not None and emulator_supports_platform(configured_entry, platform):
            return configured

    resolved_compatible = (
        compatible_names
        if compatible_names is not None
        else compatible_emulator_names_for_platform(emulators, platform, emulator_supports_platform)
    )
    if resolved_compatible:
        return resolved_compatible[0]
    return ""


def available_emulator_name_for_platform(
    platform: str,
    emulators: list[EmulatorEntry],
    emulator_supports_platform: Callable[[EmulatorEntry, str], bool],
    default_name: str = "",
) -> str:
    selected_platform = platform.strip()
    if not selected_platform:
        return ""

    candidate_names: list[str] = []
    if default_name:
        candidate_names.append(default_name)
    for emulator_name in compatible_emulator_names_for_platform(emulators, selected_platform, emulator_supports_platform):
        if emulator_name not in candidate_names:
            candidate_names.append(emulator_name)

    for emulator_name in candidate_names:
        entry = emulator_entry_by_name(emulators, emulator_name)
        if entry is None:
            continue
        if emulator_entry_has_usable_path(entry):
            return emulator_name
    return ""


def resolved_emulator_entry_for_game(
    game: dict[str, str],
    *,
    default_emulator_name_for_platform_fn: Callable[[str], str],
    emulator_entry_by_name_fn: Callable[[str], EmulatorEntry | None],
) -> tuple[str, EmulatorEntry | None]:
    platform_value = game.get("platform", "")
    platform = platform_value.strip() if isinstance(platform_value, str) else ""
    if not platform:
        return "", None

    emulator_name = default_emulator_name_for_platform_fn(platform)
    if not emulator_name:
        return "", None
    return emulator_name, emulator_entry_by_name_fn(emulator_name)


def is_ps3_emulator_entry(
    emulator: EmulatorEntry,
    emulator_profile_for_entry: Callable[[EmulatorEntry], dict[str, object] | None],
) -> bool:
    name_value = emulator.get("name", "")
    name = name_value.strip() if isinstance(name_value, str) else ""
    if name and is_rpcs3_emulator_name(name):
        return True

    path_value = emulator.get("path", "")
    path_text = path_value.strip() if isinstance(path_value, str) else ""
    executable_stem = Path(path_text).stem.casefold() if path_text else ""
    if "rpcs3" in executable_stem:
        return True

    profile = emulator_profile_for_entry(emulator)
    if profile is None:
        return False

    keywords = profile.get("platform_keywords", [])
    if not isinstance(keywords, list):
        return False

    for keyword in keywords:
        if not isinstance(keyword, str):
            continue
        tokens = {token for token in keyword.casefold().replace("-", " ").split() if token}
        if "ps3" in tokens:
            return True
        if {"playstation", "3"}.issubset(tokens):
            return True
    return False


def install_block_reason_for_game(
    game: dict[str, str],
    is_native_executable_platform: Callable[[dict[str, str]], bool],
    is_emulators_platform: Callable[[dict[str, str]], bool],
    available_emulator_name: Callable[[str], str],
) -> str:
    if is_native_executable_platform(game) or is_emulators_platform(game):
        return ""

    platform_value = game.get("platform", "")
    platform = platform_value.strip() if isinstance(platform_value, str) else ""
    if not platform:
        return "Selected game has no platform value and cannot be installed."

    if available_emulator_name(platform):
        return ""

    return (
        f"No available emulator is configured for platform '{platform}'. "
        "Add/configure one in Emulators before installing this game."
    )
