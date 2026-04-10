from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Callable


DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES = {
    ".ds_store",
    "desktop.ini",
    "ehthumbs.db",
    "thumbs.db",
}


def matching_platforms_for_emulator_keywords(assignable_platforms: list[str], keywords: list[str]) -> list[str]:
    if not keywords:
        return []

    def token_set(value: str) -> set[str]:
        tokens: set[str] = set()
        for chunk in re.findall(r"[A-Za-z0-9]+", value):
            folded_chunk = chunk.casefold()
            if folded_chunk:
                tokens.add(folded_chunk)

            split_chunks = re.sub(
                r"(?<=[A-Za-z])(?=[0-9])|(?<=[0-9])(?=[A-Za-z])|(?<=[a-z])(?=[A-Z])",
                " ",
                chunk,
            ).split()
            folded_parts = [part.casefold() for part in split_chunks if part]
            tokens.update(folded_parts)

            compact_alpha = "".join(part for part in folded_parts if part.isalpha())
            if compact_alpha:
                tokens.add(compact_alpha)

        return tokens

    matches: list[str] = []
    for platform in assignable_platforms:
        platform_tokens = token_set(platform)
        if not platform_tokens:
            continue

        for keyword in keywords:
            if not isinstance(keyword, str):
                continue
            keyword_tokens = token_set(keyword.strip())
            if not keyword_tokens:
                continue
            if not keyword_tokens.issubset(platform_tokens):
                continue

            extra_tokens = platform_tokens - keyword_tokens
            keyword_has_numeric_token = any(token.isdigit() for token in keyword_tokens)
            extra_has_numeric_token = any(token.isdigit() for token in extra_tokens)
            if extra_has_numeric_token and not keyword_has_numeric_token:
                continue

            extra_alpha_tokens = {t for t in extra_tokens if t.isalpha()}
            if extra_alpha_tokens:
                normalized_platform = platform.casefold()
                keyword_end = max(
                    (m.end() for kw_tok in keyword_tokens if kw_tok.isalpha()
                     for m in re.finditer(re.escape(kw_tok), normalized_platform)),
                    default=0,
                )
                if any(
                    any(m.start() >= keyword_end for m in re.finditer(re.escape(et), normalized_platform))
                    for et in extra_alpha_tokens
                ):
                    continue

            if platform not in matches:
                matches.append(platform)
            break
    return matches


def split_configured_paths(value: str) -> list[str]:
    return [
        item.strip()
        for item in re.split(r"[;\r\n]+", value)
        if isinstance(item, str) and item.strip()
    ]


def normalize_save_strategy_value(value: str) -> str:
    strategy = value.strip().casefold() if isinstance(value, str) else ""
    aliases = {
        "": "auto",
        "auto": "auto",
        "singlefile": "single_file",
        "single_file": "single_file",
        "single-file": "single_file",
        "single file": "single_file",
        "file": "single_file",
        "folder": "folder",
        "directory": "folder",
        "folder_per_game": "folder",
        "folder-per-game": "folder",
    }
    return aliases.get(strategy, "auto")


def normalize_ignore_extension_value(value: str) -> str:
    if not isinstance(value, str):
        return ""
    normalized = value.strip().casefold()
    if not normalized:
        return ""
    if "/" in normalized or "\\" in normalized:
        normalized = Path(normalized).suffix.casefold()
    if normalized.startswith("*."):
        normalized = normalized[1:]
    if not normalized.startswith("."):
        normalized = f".{normalized.lstrip('*')}"
    if not re.fullmatch(r"\.[a-z0-9]+", normalized):
        return ""
    if normalized in {".jpg", ".jpeg"}:
        return ""
    return normalized


def emulator_profile_for_entry(
    emulator: dict[str, str],
    profiles: list[dict[str, Any]],
) -> dict[str, Any] | None:
    name_value = emulator.get("name", "")
    path_value = emulator.get("path", "")
    name = name_value.strip().casefold() if isinstance(name_value, str) else ""
    executable_name = Path(path_value).name.strip().casefold() if isinstance(path_value, str) else ""
    executable_stem = Path(executable_name).stem.casefold() if executable_name else ""

    for profile in profiles:
        profile_name = profile.get("name", "")
        profile_name_folded = profile_name.strip().casefold() if isinstance(profile_name, str) else ""
        if name and profile_name_folded == name:
            return profile

        match_tokens = profile.get("match_tokens", [])
        if not isinstance(match_tokens, list):
            continue
        normalized_tokens = {
            token.strip().casefold()
            for token in match_tokens
            if isinstance(token, str) and token.strip()
        }
        if not normalized_tokens:
            continue

        if executable_name and executable_name in normalized_tokens:
            return profile
        if executable_stem and any(Path(token).stem.casefold() == executable_stem for token in normalized_tokens):
            return profile

    return None


def emulator_entry_matches_tokens(
    emulator: dict[str, str] | None,
    tokens: set[str] | list[str] | tuple[str, ...],
    profiles: list[dict[str, Any]] | None = None,
) -> bool:
    if not isinstance(emulator, dict):
        return False

    normalized_tokens = {
        token.strip().casefold()
        for token in tokens
        if isinstance(token, str) and token.strip()
    }
    if not normalized_tokens:
        return False

    candidate_values: set[str] = set()

    def add_candidate(value: str) -> None:
        normalized = value.strip().casefold()
        if not normalized:
            return
        candidate_values.add(normalized)
        stem = Path(normalized).stem.casefold()
        if stem:
            candidate_values.add(stem)

    name_value = emulator.get("name", "")
    if isinstance(name_value, str):
        add_candidate(name_value)

    path_value = emulator.get("path", "")
    if isinstance(path_value, str) and path_value.strip():
        executable_path = Path(path_value.strip())
        add_candidate(executable_path.name)
        add_candidate(executable_path.stem)

    if isinstance(profiles, list):
        profile = emulator_profile_for_entry(emulator, profiles)
        if isinstance(profile, dict):
            profile_name = profile.get("name", "")
            if isinstance(profile_name, str):
                add_candidate(profile_name)
            match_tokens = profile.get("match_tokens", [])
            if isinstance(match_tokens, list):
                for token in match_tokens:
                    if isinstance(token, str):
                        add_candidate(token)

    return any(token == candidate or token in candidate for token in normalized_tokens for candidate in candidate_values)


def emulator_profile_for_game(
    game: dict[str, str],
    executable_path: str,
    profiles: list[dict[str, Any]],
) -> dict[str, Any]:
    title_value = game.get("title", "")
    title = title_value.strip() if isinstance(title_value, str) else ""
    executable_name = Path(executable_path).name.strip().casefold()
    token_matches: list[dict[str, Any]] = []

    for profile in profiles:
        match_tokens = profile.get("match_tokens", [])
        if executable_name and isinstance(match_tokens, list) and any(token == executable_name for token in match_tokens):
            token_matches.append(profile)

    selected_profile: dict[str, Any] | None = None
    if len(token_matches) == 1:
        selected_profile = token_matches[0]
    elif len(token_matches) > 1:
        for profile in token_matches:
            profile_name = profile.get("name", "")
            if isinstance(profile_name, str) and profile_name.strip() == title:
                selected_profile = profile
                break
        if selected_profile is None:
            selected_profile = token_matches[0]

    if isinstance(selected_profile, dict):
        resolved_name = selected_profile.get("name", "Emulator")
        if selected_profile.get("use_game_title_as_name", False):
            resolved_name = title or resolved_name
        return {
            "name": resolved_name,
            "args": selected_profile.get("args", "%rom%"),
            "all_platforms": bool(selected_profile.get("all_platforms", False)),
            "platform_keywords": selected_profile.get("platform_keywords", []),
            "save_strategy": selected_profile.get("save_strategy", "auto"),
            "ignore_files": selected_profile.get("ignore_files", []),
            "ignore_extensions": selected_profile.get("ignore_extensions", []),
            "save_directories": selected_profile.get("save_directories", []),
            "state_directories": selected_profile.get("state_directories", []),
        }

    return {
        "name": title or "Emulator",
        "args": "%rom%",
        "all_platforms": False,
        "platform_keywords": [],
        "save_strategy": "auto",
        "ignore_files": [],
        "ignore_extensions": [],
        "save_directories": [],
        "state_directories": [],
    }


def resolved_save_strategy_for_emulator(
    emulator: dict[str, str],
    save_type: str,
    *,
    emulator_profile_for_entry_fn: Callable[[dict[str, str]], dict[str, Any] | None],
) -> str:
    configured_value = emulator.get("save_strategy", "")
    configured_strategy = normalize_save_strategy_value(configured_value) if isinstance(configured_value, str) else "auto"
    if configured_strategy != "auto":
        return configured_strategy

    profile = emulator_profile_for_entry_fn(emulator)
    profile_value = profile.get("save_strategy", "") if isinstance(profile, dict) else ""
    profile_strategy = normalize_save_strategy_value(profile_value) if isinstance(profile_value, str) else "auto"
    if profile_strategy != "auto":
        return profile_strategy

    if save_type == "state":
        return "single_file"
    return "auto"


def resolved_ignore_basenames_for_emulator(
    emulator: dict[str, str],
    *,
    emulator_profile_for_entry_fn: Callable[[dict[str, str]], dict[str, Any] | None],
) -> set[str]:
    configured_value = emulator.get("ignore_files", "")
    configured_values = split_configured_paths(configured_value) if isinstance(configured_value, str) else []

    profile = emulator_profile_for_entry_fn(emulator)
    profile_values: list[str] = []
    if isinstance(profile, dict):
        raw_profile_values = profile.get("ignore_files", [])
        if isinstance(raw_profile_values, list):
            profile_values = [item.strip() for item in raw_profile_values if isinstance(item, str) and item.strip()]

    all_values = configured_values if configured_values else profile_values
    basenames: set[str] = set(DEFAULT_CLOUD_SYNC_IGNORE_BASENAMES)
    for value in all_values:
        base_name = Path(value).name.strip().casefold()
        if base_name and Path(base_name).suffix.casefold() not in {".jpg", ".jpeg"}:
            basenames.add(base_name)
    return basenames


def resolved_ignore_extensions_for_emulator(
    emulator: dict[str, str],
    *,
    emulator_profile_for_entry_fn: Callable[[dict[str, str]], dict[str, Any] | None],
) -> set[str]:
    configured_value = emulator.get("ignore_extensions", "")
    configured_values = split_configured_paths(configured_value) if isinstance(configured_value, str) else []

    profile = emulator_profile_for_entry_fn(emulator)
    profile_values: list[str] = []
    if isinstance(profile, dict):
        raw_profile_values = profile.get("ignore_extensions", [])
        if isinstance(raw_profile_values, list):
            profile_values = [item.strip() for item in raw_profile_values if isinstance(item, str) and item.strip()]

    all_values = configured_values if configured_values else profile_values
    normalized: set[str] = set()
    for value in all_values:
        normalized_value = normalize_ignore_extension_value(value)
        if normalized_value:
            normalized.add(normalized_value)
    return normalized


def emulator_autoprofiles_path(base_path: Path) -> Path:
    return base_path / "emulator-autoprofiles.json"


def default_emulator_autoprofiles(base_path: Path | None = None) -> list[dict[str, Any]]:
    resolved_base_path = base_path if isinstance(base_path, Path) else Path(__file__).resolve().parents[2]
    autoprofiles_path = emulator_autoprofiles_path(resolved_base_path)
    if not autoprofiles_path.exists():
        return []

    try:
        parsed = json.loads(autoprofiles_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return []

    return parsed if isinstance(parsed, list) else []


def normalize_emulator_autoprofiles(
    value: Any,
    normalize_save_strategy_value: Callable[[str], str],
    normalize_ignore_extension_value: Callable[[str], str],
) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []

    normalized: list[dict[str, Any]] = []
    for item in value:
        if not isinstance(item, dict):
            continue

        match_tokens = item.get("match_tokens", [])
        if not isinstance(match_tokens, list):
            continue
        normalized_tokens = [
            token.strip().casefold()
            for token in match_tokens
            if isinstance(token, str) and token.strip()
        ]
        if not normalized_tokens:
            continue
        primary_token = normalized_tokens[0]

        name = item.get("name", "")
        if not isinstance(name, str) or not name.strip():
            continue

        args = item.get("args", "%rom%")
        if not isinstance(args, str):
            args = "%rom%"

        all_platforms = bool(item.get("all_platforms", False))

        platform_keywords = item.get("platform_keywords", [])
        normalized_keywords: list[str] = []
        if isinstance(platform_keywords, list):
            normalized_keywords = [
                keyword.strip()
                for keyword in platform_keywords
                if isinstance(keyword, str) and keyword.strip()
            ]

        use_game_title_as_name = bool(item.get("use_game_title_as_name", False))

        save_strategy = item.get("save_strategy", "auto")
        if not isinstance(save_strategy, str):
            save_strategy = "auto"
        normalized_save_strategy = normalize_save_strategy_value(save_strategy)

        ignore_files = item.get("ignore_files", [])
        normalized_ignore_files: list[str] = []
        if isinstance(ignore_files, list):
            normalized_ignore_files = [
                file_name.strip()
                for file_name in ignore_files
                if isinstance(file_name, str) and file_name.strip()
            ]

        ignore_extensions = item.get("ignore_extensions", [])
        normalized_ignore_extensions: list[str] = []
        if isinstance(ignore_extensions, list):
            normalized_ignore_extensions = [
                normalized_extension
                for extension in ignore_extensions
                if isinstance(extension, str)
                for normalized_extension in [normalize_ignore_extension_value(extension)]
                if normalized_extension
            ]

        save_directories = item.get("save_directories", [])
        normalized_save_directories: list[str] = []
        if isinstance(save_directories, list):
            normalized_save_directories = [
                directory.strip()
                for directory in save_directories
                if isinstance(directory, str) and directory.strip()
            ]

        state_directories = item.get("state_directories", [])
        normalized_state_directories: list[str] = []
        if isinstance(state_directories, list):
            normalized_state_directories = [
                directory.strip()
                for directory in state_directories
                if isinstance(directory, str) and directory.strip()
            ]

        source = item.get("source")
        normalized_source = source.copy() if isinstance(source, dict) else None

        normalized_profile = {
            "match_tokens": [primary_token],
            "name": name.strip(),
            "args": args.strip() or "%rom%",
            "all_platforms": all_platforms,
            "platform_keywords": normalized_keywords,
            "use_game_title_as_name": use_game_title_as_name,
            "save_strategy": normalized_save_strategy,
            "ignore_files": normalized_ignore_files,
            "ignore_extensions": normalized_ignore_extensions,
            "save_directories": normalized_save_directories,
            "state_directories": normalized_state_directories,
        }
        if normalized_source is not None:
            normalized_profile["source"] = normalized_source
        normalized.append(normalized_profile)

    return normalized


def load_emulator_autoprofiles(
    cached_profiles: Any,
    base_path: Path,
    normalize_save_strategy_value: Callable[[str], str],
    normalize_ignore_extension_value: Callable[[str], str],
) -> list[dict[str, Any]]:
    if isinstance(cached_profiles, list):
        return cached_profiles

    parsed = default_emulator_autoprofiles(base_path)
    normalized = normalize_emulator_autoprofiles(
        parsed,
        normalize_save_strategy_value,
        normalize_ignore_extension_value,
    )
    return normalized if normalized else []
