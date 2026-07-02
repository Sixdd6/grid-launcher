from __future__ import annotations

import shlex
import sys
from pathlib import Path
from typing import Callable


_NATIVE_GAME_SUFFIXES = frozenset({".exe", ".bat", ".cmd", ".ps1", ".sh"})
_EMULATOR_SUFFIXES = frozenset({*_NATIVE_GAME_SUFFIXES, ".appimage"})


def _has_launchable_suffix(path: Path, allowed_suffixes: frozenset[str]) -> bool:
    return path.suffix.casefold() in allowed_suffixes


def launchable_native_game_file(path: Path) -> bool:
    return _has_launchable_suffix(path, _NATIVE_GAME_SUFFIXES)


def launchable_emulator_file(path: Path) -> bool:
    return _has_launchable_suffix(path, _EMULATOR_SUFFIXES)


def retroarch_core_argument_path(configured_core: str) -> str:
    core = configured_core.strip()
    if not core:
        return ""

    normalized = core.replace("\\", "/")
    if "/" in normalized:
        return normalized

    if sys.platform == "win32":
        extension = ".dll"
    elif sys.platform == "darwin":
        extension = ".dylib"
    else:
        extension = ".so"

    base = normalized
    for known in (".dll", ".dylib", ".so"):
        if base.casefold().endswith(known):
            base = base[: -len(known)]
            break

    if base.casefold().endswith("_libretro"):
        core_file = f"{base}{extension}"
    else:
        core_file = f"{base}_libretro{extension}"
    return f"cores/{core_file}"


def retroarch_core_value(
    emulator_name: str,
    platform: str,
    core_defaults: dict[str, str],
    is_retroarch_emulator_name: Callable[[str], bool],
    mapping_value_for_platform: Callable[[dict[str, str], str], str],
    retroarch_core_argument_path: Callable[[str], str],
) -> str:
    if not is_retroarch_emulator_name(emulator_name):
        return ""
    if not isinstance(platform, str) or not platform.strip():
        return ""

    configured_core = mapping_value_for_platform(core_defaults, platform)
    if not configured_core:
        return ""
    return retroarch_core_argument_path(configured_core)


def launch_placeholders_for_game(
    rom_path: str,
    emulator_name: str,
    core_value: str,
    is_rpcs3_emulator_name: Callable[[str], bool],
    ps3_game_id: str,
) -> dict[str, str]:
    rpcs3_game_token = ""
    resolved_ps3_game_id = ""
    if is_rpcs3_emulator_name(emulator_name):
        rpcs3_game_token = "%RPCS3_GAMEID%"
        resolved_ps3_game_id = ps3_game_id.strip() if isinstance(ps3_game_id, str) else ""

    return {
        "%rom%": rom_path,
        "%core%": core_value,
        "%RPCS3_GAMEID%": rpcs3_game_token,
        "%ps3_gameid%": resolved_ps3_game_id,
    }


def strip_wrapping_quotes(token: str) -> str:
    stripped = token.strip()
    if len(stripped) >= 2 and stripped[0] == stripped[-1] and stripped[0] in {'"', "'"}:
        return stripped[1:-1]
    return stripped


def apply_launch_placeholders_to_args(args: list[str], placeholders: dict[str, str]) -> list[str]:
    resolved_args: list[str] = []
    core_value = placeholders.get("%core%", "")
    core_missing = not core_value.strip()
    for arg in args:
        had_core_placeholder = "%core%" in arg
        resolved = arg
        for token, value in placeholders.items():
            resolved = resolved.replace(token, value)
        resolved = strip_wrapping_quotes(resolved)
        if had_core_placeholder and core_missing:
            if resolved_args and resolved_args[-1] in {"-L", "--libretro", "--core"}:
                resolved_args.pop()
            continue
        if resolved:
            resolved_args.append(resolved)
    return resolved_args


def split_launch_template_args(template: str) -> list[str]:
    if not template.strip():
        return []

    try:
        return shlex.split(template, posix=True)
    except ValueError:
        return shlex.split(template, posix=False)


def validate_launch_placeholders(combined_template: str, placeholders: dict[str, str]) -> None:
    if "%core%" in combined_template and not placeholders.get("%core%", "").strip():
        raise ValueError("No RetroArch core is configured for this platform. Set one in Emulators > Defaults.")
    if "%RPCS3_GAMEID%" in combined_template and not placeholders.get("%RPCS3_GAMEID%", "").strip():
        raise ValueError(
            "No PS3 game ID was found for this title. Reinstall or verify extracted content includes a valid PS3 title ID."
        )
    if "%ps3_gameid%" in combined_template and not placeholders.get("%ps3_gameid%", "").strip():
        raise ValueError(
            "No PS3 game ID was found for this title. Reinstall or verify extracted content includes a valid PS3 title ID."
        )


def resolve_launch_arguments_for_game(
    game: dict[str, str],
    launch_args_value: str,
    default_emulator_name_for_platform: Callable[[str], str],
    emulator_entry_by_name: Callable[[str], dict[str, str] | None],
    split_launch_template_args: Callable[[str], list[str]],
    launch_placeholders_for_game_fn: Callable[[dict[str, str], str], dict[str, str]],
    validate_launch_placeholders_fn: Callable[[str, dict[str, str]], None],
    apply_launch_placeholders_to_args_fn: Callable[[list[str], dict[str, str]], list[str]],
) -> tuple[str, list[str]]:
    platform_value = game.get("platform", "")
    platform = platform_value.strip() if isinstance(platform_value, str) else ""
    emulator_name = default_emulator_name_for_platform(platform)
    emulator_entry = emulator_entry_by_name(emulator_name)

    emulator_args = "%rom%"
    if emulator_entry is not None:
        args_value = emulator_entry.get("args", "%rom%")
        if isinstance(args_value, str) and args_value.strip():
            emulator_args = args_value.strip()

    global_args = launch_args_value.strip() if isinstance(launch_args_value, str) else ""
    combined_template = " ".join(part for part in (emulator_args, global_args) if part).strip()

    parsed_template_args = split_launch_template_args(combined_template)
    placeholders = launch_placeholders_for_game_fn(game, emulator_name)
    validate_launch_placeholders_fn(combined_template, placeholders)
    resolved_args = apply_launch_placeholders_to_args_fn(parsed_template_args, placeholders)
    return emulator_name, resolved_args


def resolve_rom_path_for_game(
    game: dict[str, str],
    is_arcade_platform: Callable[[dict[str, str]], bool],
    candidate_extracted_paths_for_game: Callable[[dict[str, str]], list[Path]],
    candidate_archive_paths_for_game: Callable[[dict[str, str]], list[Path]],
) -> str:
    if not is_arcade_platform(game):
        for candidate in candidate_extracted_paths_for_game(game):
            if candidate.exists() and candidate.is_file():
                return str(candidate)

    for candidate in candidate_archive_paths_for_game(game):
        if candidate.exists() and candidate.is_file():
            return str(candidate)

    archive_path_value = game.get("archive_path", "")
    if isinstance(archive_path_value, str):
        return archive_path_value.strip()
    return ""


def normalized_retroarch_core_args(emulator_dir: Path, args: list[str]) -> list[str]:
    normalized_args = list(args)
    core_option_tokens = {"-L", "--libretro", "--core"}
    for index, token in enumerate(normalized_args[:-1]):
        if token not in core_option_tokens:
            continue

        core_token = normalized_args[index + 1].strip()
        if not core_token:
            continue

        core_path = Path(core_token).expanduser()
        if core_path.is_absolute():
            continue

        candidate = (emulator_dir / core_path).resolve(strict=False)
        if candidate.exists() and candidate.is_file():
            normalized_args[index + 1] = str(candidate)
    return normalized_args


def prepare_native_launch_command(
    game: dict[str, str],
    resolved_native_executable_path_for_game: Callable[[dict[str, str]], Path | None],
    split_launch_template_args_fn: Callable[[str], list[str]],
) -> tuple[list[str], str]:
    native_executable = resolved_native_executable_path_for_game(game)
    if native_executable is None:
        raise ValueError(
            "No launchable native executable is configured for this game. Use Game Settings to select one."
        )

    custom_parameters_value = game.get("native_launch_parameters", "")
    custom_parameters = custom_parameters_value.strip() if isinstance(custom_parameters_value, str) else ""
    try:
        native_args = split_launch_template_args_fn(custom_parameters)
    except ValueError as error:
        raise ValueError(f"Invalid custom launch parameters: {error}") from error

    return [str(native_executable), *native_args], str(native_executable.parent)



def prepare_emulator_launch_command(
    game: dict[str, str],
    default_emulator_name_for_platform: Callable[[str], str],
    emulator_entry_by_name: Callable[[str], dict[str, str] | None],
    resolved_rom_path_for_game: Callable[[dict[str, str]], str],
    resolved_launch_arguments_for_game: Callable[[dict[str, str]], tuple[str, list[str]]],
    is_retroarch_emulator_name: Callable[[str], bool],
    normalized_retroarch_core_args_fn: Callable[[Path, list[str]], list[str]],
) -> tuple[str, list[str], str]:
    platform_value = game.get("platform", "")
    platform = platform_value.strip() if isinstance(platform_value, str) else ""
    emulator_name = default_emulator_name_for_platform(platform)
    if not emulator_name:
        raise ValueError("No emulator is configured. Add one in Emulators settings.")

    emulator_entry = emulator_entry_by_name(emulator_name)
    if emulator_entry is None:
        raise ValueError(f"Default emulator '{emulator_name}' was not found.")

    emulator_path_value = emulator_entry.get("path", "")
    emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
    if not emulator_path_text:
        raise ValueError(f"Emulator '{emulator_name}' has no executable path configured.")

    emulator_path = Path(emulator_path_text).expanduser()
    if not emulator_path.exists() or not emulator_path.is_file():
        raise ValueError(f"Emulator executable not found:\n{emulator_path}")

    rom_path = resolved_rom_path_for_game(game)
    if not rom_path:
        raise ValueError("No ROM file is available for this game.")

    rom_file = Path(rom_path).expanduser()
    if not rom_file.exists() or not rom_file.is_file():
        raise ValueError(f"ROM file not found:\n{rom_file}")

    try:
        _, parsed_args = resolved_launch_arguments_for_game(game)
    except ValueError as error:
        raise ValueError(f"Invalid launch arguments: {error}") from error

    if is_retroarch_emulator_name(emulator_name):
        parsed_args = normalized_retroarch_core_args_fn(emulator_path.parent, parsed_args)

    return emulator_name, [str(emulator_path), *parsed_args], str(emulator_path.parent)



def process_exited_early_message(exit_code: int, command: list[str]) -> str:
    command_text = " ".join(command)
    return f"Process exited immediately (code {exit_code}).\nCommand:\n{command_text}"
