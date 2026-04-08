from __future__ import annotations

from typing import Callable


def emulator_form_state_for_row(
    emulators: list[dict[str, str]],
    row: int,
    normalize_save_strategy: Callable[[str], str],
) -> dict[str, str]:
    default_state = {
        "name": "",
        "path": "",
        "args": "%rom%",
        "save_strategy": "auto",
        "ignore_files": "",
        "ignore_extensions": "",
        "save_paths": "",
        "state_paths": "",
    }
    if row < 0 or row >= len(emulators):
        return default_state

    emulator = emulators[row]
    return {
        "name": str(emulator.get("name", "")).strip(),
        "path": str(emulator.get("path", "")).strip(),
        "args": str(emulator.get("args", "%rom%")).strip() or "%rom%",
        "save_strategy": normalize_save_strategy(str(emulator.get("save_strategy", "auto"))),
        "ignore_files": str(emulator.get("ignore_files", "")).strip(),
        "ignore_extensions": str(emulator.get("ignore_extensions", "")).strip(),
        "save_paths": str(emulator.get("save_paths", "")).strip(),
        "state_paths": str(emulator.get("state_paths", "")).strip(),
    }


def make_emulator_entry_payload(
    name: str,
    path: str,
    args: str,
    save_strategy: str,
    ignore_files: str,
    ignore_extensions: str,
    save_paths: str,
    state_paths: str,
) -> dict[str, str]:
    return {
        "name": name.strip(),
        "path": path.strip(),
        "args": args.strip() or "%rom%",
        "save_strategy": save_strategy.strip() or "auto",
        "ignore_files": ignore_files.strip(),
        "ignore_extensions": ignore_extensions.strip(),
        "save_paths": save_paths.strip(),
        "state_paths": state_paths.strip(),
    }


def upsert_emulator_entry(
    emulators: list[dict[str, str]],
    entry: dict[str, str],
    target_index: int,
) -> list[dict[str, str]]:
    updated = list(emulators)
    if 0 <= target_index < len(updated):
        updated[target_index] = entry
    else:
        updated.append(entry)
    return updated


def mapping_list_entries(
    server_platforms: list[str],
    defaults: dict[str, str],
    core_defaults: dict[str, str],
    is_retroarch_emulator_name: Callable[[str], bool],
) -> list[str]:
    rows: list[str] = []
    for platform in sorted(server_platforms, key=str.casefold):
        emulator_name = defaults.get(platform, "(none)")
        if emulator_name != "(none)" and is_retroarch_emulator_name(emulator_name):
            core_name = core_defaults.get(platform, "")
            suffix = f" ({core_name})" if core_name else ""
            rows.append(f"{platform}: {emulator_name}{suffix}")
        else:
            rows.append(f"{platform}: {emulator_name}")
    return rows


def preferred_emulator_selection(
    compatible_emulators: list[str],
    preferred_emulator: str,
    selected_before_refresh: str,
) -> str:
    if preferred_emulator and preferred_emulator in compatible_emulators:
        return preferred_emulator
    if selected_before_refresh and selected_before_refresh in compatible_emulators:
        return selected_before_refresh
    return compatible_emulators[0] if compatible_emulators else ""


def selected_retroarch_core(
    saved_core: str,
    installed_cores: list[str],
    is_retroarch: bool,
) -> str:
    if not is_retroarch or not installed_cores:
        return ""
    if saved_core and saved_core in installed_cores:
        return saved_core
    return installed_cores[0]


def remove_emulator_default_mappings(
    defaults: dict[str, str],
    core_defaults: dict[str, str],
    removed_name: str,
) -> tuple[dict[str, str], dict[str, str]]:
    updated_defaults = dict(defaults)
    for platform in list(updated_defaults.keys()):
        if updated_defaults[platform] == removed_name:
            updated_defaults.pop(platform)

    updated_core_defaults = dict(core_defaults)
    for platform in list(updated_core_defaults.keys()):
        if platform not in updated_defaults:
            updated_core_defaults.pop(platform)

    return updated_defaults, updated_core_defaults
