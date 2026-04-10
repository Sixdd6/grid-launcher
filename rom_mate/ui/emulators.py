from __future__ import annotations

from copy import deepcopy
from typing import Any, Callable


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


def save_button_label(emulators: list[dict[str, str]], selected_row: int) -> str:
    return "Update" if 0 <= selected_row < len(emulators) else "Add New"


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


def _normalized_source_provider(provider_value: Any) -> str:
    provider = provider_value.strip().casefold() if isinstance(provider_value, str) else ""
    if not provider:
        return ""
    provider_aliases = {
        "github": "github",
        "github-release": "github",
        "github_release": "github",
        "githubrelease": "github",
    }
    return provider_aliases.get(provider, provider)


def source_download_emulator_entries(
    autoprofiles: list[dict[str, Any]] | None,
) -> list[dict[str, Any]]:
    profiles = autoprofiles if isinstance(autoprofiles, list) else []
    rows: list[dict[str, Any]] = []
    seen_keys: set[tuple[str, str, str, str]] = set()

    for profile in profiles:
        if not isinstance(profile, dict):
            continue

        name_value = profile.get("name", "")
        name = name_value.strip() if isinstance(name_value, str) else ""
        if not name:
            continue

        source_value = profile.get("source")
        if not isinstance(source_value, dict):
            continue

        provider = _normalized_source_provider(source_value.get("provider", ""))
        if not provider:
            continue

        owner_value = source_value.get("owner", "")
        owner = owner_value.strip() if isinstance(owner_value, str) else ""
        repo_value = source_value.get("repo", source_value.get("repository", ""))
        repo = repo_value.strip() if isinstance(repo_value, str) else ""
        if not owner or not repo:
            continue

        release_tag = ""
        for key in ("release_tag", "tag", "version"):
            value = source_value.get(key, "")
            if isinstance(value, str) and value.strip():
                release_tag = value.strip()
                break
        if not release_tag:
            release_tag = "latest"

        dedupe_key = (name.casefold(), provider, owner.casefold(), repo.casefold())
        if dedupe_key in seen_keys:
            continue
        seen_keys.add(dedupe_key)

        rows.append(
            {
                "name": name,
                "provider": provider,
                "owner": owner,
                "repo": repo,
                "release_tag": release_tag,
                "source_id": f"{owner}/{repo}",
                "source_metadata": deepcopy(source_value),
            }
        )

    rows.sort(key=lambda row: (row["name"].casefold(), row["source_id"].casefold()))
    return rows


def filter_source_download_emulator_entries(
    source_entries: list[dict[str, Any]],
    query: str = "",
    installed_emulator_names: list[str] | None = None,
) -> list[dict[str, Any]]:
    normalized_query_tokens = [
        token
        for token in (query.casefold().split() if isinstance(query, str) else [])
        if token
    ]
    installed_names = {
        name.strip().casefold()
        for name in (installed_emulator_names or [])
        if isinstance(name, str) and name.strip()
    }

    filtered_rows: list[dict[str, Any]] = []
    for row in source_entries:
        if not isinstance(row, dict):
            continue

        name_value = row.get("name", "")
        name = name_value.strip() if isinstance(name_value, str) else ""
        if not name:
            continue
        if name.casefold() in installed_names:
            continue

        if normalized_query_tokens:
            searchable_parts = [
                row.get("name", ""),
                row.get("provider", ""),
                row.get("owner", ""),
                row.get("repo", ""),
                row.get("release_tag", ""),
                row.get("source_id", ""),
            ]
            searchable_text = " ".join(
                value.strip().casefold()
                for value in searchable_parts
                if isinstance(value, str) and value.strip()
            )
            if not all(token in searchable_text for token in normalized_query_tokens):
                continue

        filtered_rows.append(dict(row))

    return filtered_rows


def available_source_download_emulator_entries(
    autoprofiles: list[dict[str, Any]] | None,
    query: str = "",
    installed_emulator_names: list[str] | None = None,
) -> list[dict[str, Any]]:
    return filter_source_download_emulator_entries(
        source_download_emulator_entries(autoprofiles),
        query=query,
        installed_emulator_names=installed_emulator_names,
    )
