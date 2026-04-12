from __future__ import annotations

import os
import re
import sys
from pathlib import Path
from typing import Callable


_HEX8_RE = re.compile(r"^[0-9a-fA-F]{8}$")
_HEX16_RE = re.compile(r"^[0-9a-fA-F]{16}$")
_STFS_MAGIC = {b"CON ", b"LIVE", b"PIRS"}
_STFS_CONTENT_TYPE_OFFSET = 0x344
_STFS_TITLE_ID_OFFSET = 0x360
_XUID_ANONYMOUS = "0000000000000000"


def _read_stfs_header(content_path: Path) -> tuple[str, str]:
    """Return (title_id_hex8, content_type_hex8) or ("", "") if not an STFS file."""
    try:
        with content_path.open("rb") as f:
            header = f.read(0x368)
    except OSError:
        return "", ""

    if len(header) < 0x368:
        return "", ""
    if header[:4] not in _STFS_MAGIC:
        return "", ""

    content_type = int.from_bytes(header[_STFS_CONTENT_TYPE_OFFSET:_STFS_CONTENT_TYPE_OFFSET + 4], "big")
    title_id = int.from_bytes(header[_STFS_TITLE_ID_OFFSET:_STFS_TITLE_ID_OFFSET + 4], "big")
    return f"{title_id:08X}", f"{content_type:08X}"


def apply_xenia_content_without_ui(
    content_path: Path,
    content_root: Path,
    *,
    expected_title_id: str = "",
) -> dict[str, object]:
    """Copy an STFS content package to the correct xenia content directory.

    Returns a dict with keys:
      - "title_id": detected TitleID (8-char hex string)
      - "content_type": detected ContentType (8-char hex string)
      - "destination": absolute path string where the file was placed
      - "error": non-empty string if something went wrong
    """
    if not isinstance(content_path, Path):
        content_path = Path(content_path)
    if not isinstance(content_root, Path):
        content_root = Path(content_root)

    if not content_path.exists() or not content_path.is_file():
        return {"title_id": "", "content_type": "", "destination": "", "error": f"Content file not found: {content_path}"}

    title_id, content_type = _read_stfs_header(content_path)
    if not title_id:
        return {"title_id": "", "content_type": "", "destination": "", "error": "File does not appear to be an STFS package (bad magic)"}

    if expected_title_id and expected_title_id.upper() != title_id.upper():
        return {
            "title_id": title_id,
            "content_type": content_type,
            "destination": "",
            "error": f"Title ID mismatch: expected {expected_title_id.upper()}, archive contains {title_id}",
        }

    dest_dir = content_root / _XUID_ANONYMOUS / title_id / content_type
    try:
        dest_dir.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        return {"title_id": title_id, "content_type": content_type, "destination": "", "error": str(exc)}

    dest_path = dest_dir / content_path.name
    try:
        import shutil
        shutil.copy2(str(content_path), str(dest_path))
    except OSError as exc:
        return {"title_id": title_id, "content_type": content_type, "destination": "", "error": str(exc)}

    return {
        "title_id": title_id,
        "content_type": content_type,
        "destination": str(dest_path),
        "error": "",
    }


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


def _clean_path_value(value: str) -> str:
    cleaned = value.strip()
    if cleaned and cleaned[0] not in {'"', "'"}:
        cleaned = re.split(r"\s+#", cleaned, maxsplit=1)[0].strip()
    return cleaned.strip().strip('"').strip("'")


def _bool_value(value: str, *, default: bool = False) -> bool:
    if not isinstance(value, str):
        return default
    normalized = value.strip().casefold()
    if not normalized:
        return default
    if normalized in {"1", "true", "yes", "on"}:
        return True
    if normalized in {"0", "false", "no", "off"}:
        return False
    return default


def _resolve_setting_path(base_root: Path, raw_value: str, default_name: str) -> str:
    value = _clean_path_value(raw_value) if isinstance(raw_value, str) else ""
    if not value:
        value = default_name

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute():
        candidate = base_root / candidate
    return str(candidate.resolve())


def _emulator_dir(emulator_path_text: str) -> Path:
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if not path_text:
        return Path()

    emulator_path = Path(path_text).expanduser()
    return emulator_path if emulator_path.is_dir() else emulator_path.parent


def _is_canary_variant(emulator_path_text: str) -> bool:
    normalized = emulator_path_text.strip().casefold() if isinstance(emulator_path_text, str) else ""
    return any(token in normalized for token in ("xenia_canary", "xenia-canary", "canary"))


def _resolve_launch_path(base_root: Path, raw_value: str) -> str:
    value = _clean_path_value(raw_value)
    if not value:
        return ""

    candidate = Path(os.path.expandvars(value)).expanduser()
    if not candidate.is_absolute() and str(base_root):
        candidate = base_root / candidate
    return str(candidate.resolve())


def _launch_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]] | None,
) -> dict[str, object]:
    args = _split_launch_args(launch_template, split_launch_template_args)
    emulator_dir = _emulator_dir(emulator_path_text)
    overrides: dict[str, object] = {
        "config_path": "",
        "storage_root": "",
        "content_root": "",
        "cache_root": "",
        "portable": None,
    }

    path_options = {
        "config_path": {"-config", "--config"},
        "storage_root": {"-storage_root", "--storage_root", "-storage-root", "--storage-root"},
        "content_root": {"-content_root", "--content_root", "-content-root", "--content-root"},
        "cache_root": {"-cache_root", "--cache_root", "-cache-root", "--cache-root"},
    }
    portable_options = {"-portable", "--portable"}

    index = 0
    while index < len(args):
        raw_arg = args[index]
        index += 1
        if not isinstance(raw_arg, str) or not raw_arg.strip():
            continue

        normalized_arg = raw_arg.strip()
        lowered_arg = normalized_arg.casefold()

        matched_option = False
        for override_key, option_names in path_options.items():
            if lowered_arg in option_names and index < len(args):
                value, consumed_index = _consume_arg_value(args, index)
                index = consumed_index + 1
                if value:
                    overrides[override_key] = _resolve_launch_path(emulator_dir, value)
                matched_option = True
                break

            for option_name in option_names:
                prefix = f"{option_name}="
                if lowered_arg.startswith(prefix):
                    _, _, raw_value = normalized_arg.partition("=")
                    if raw_value.strip():
                        overrides[override_key] = _resolve_launch_path(emulator_dir, raw_value)
                    matched_option = True
                    break
            if matched_option:
                break

        if matched_option:
            continue

        if lowered_arg in portable_options:
            portable_value = True
            if index < len(args):
                next_value = args[index].strip()
                next_lowered = next_value.casefold()
                if next_lowered in {"0", "1", "true", "false", "yes", "no", "on", "off"}:
                    portable_value = _bool_value(next_value, default=True)
                    index += 1
            overrides["portable"] = portable_value
            continue

        for option_name in portable_options:
            prefix = f"{option_name}="
            if lowered_arg.startswith(prefix):
                _, _, raw_value = normalized_arg.partition("=")
                overrides["portable"] = _bool_value(raw_value, default=True)
                break

    return overrides


def _default_user_storage_root() -> Path:
    home_path = Path.home()
    if sys.platform == "win32":
        return (home_path / "Documents" / "Xenia").resolve()
    if sys.platform == "darwin":
        return (home_path / "Library" / "Application Support" / "Xenia").resolve()

    xdg_data_home = os.environ.get("XDG_DATA_HOME", "").strip()
    if xdg_data_home:
        return (Path(xdg_data_home).expanduser() / "Xenia").resolve()
    return (home_path / ".local" / "share" / "Xenia").resolve()


def _config_name_candidates(is_canary: bool) -> list[str]:
    names = ["xenia.config.toml", "xenia-config.toml"]
    if is_canary:
        names = [
            "xenia-canary.config.toml",
            "xenia-canary-config.toml",
            "xenia_canary.config.toml",
            "xenia_canary-config.toml",
            *names,
        ]

    ordered: list[str] = []
    seen: set[str] = set()
    for name in names:
        key = name.casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        ordered.append(name)
    return ordered


def _parse_toml_sections(raw_content: str) -> dict[str, dict[str, str]]:
    sections: dict[str, dict[str, str]] = {}
    current_section = ""

    for raw_line in raw_content.splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("#"):
            continue

        section_match = re.match(r"^\[(.+?)\]\s*$", stripped)
        if section_match:
            current_section = section_match.group(1).strip().casefold()
            sections.setdefault(current_section, {})
            continue

        if "=" not in raw_line:
            continue

        key, value = raw_line.split("=", 1)
        sections.setdefault(current_section, {})[key.strip().casefold()] = value.strip()

    return sections


def xenia_directory_settings(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> dict[str, str]:
    emulator_dir = _emulator_dir(emulator_path_text)
    is_canary = _is_canary_variant(emulator_path_text)
    launch_overrides = _launch_path_overrides(emulator_path_text, launch_template, split_launch_template_args)

    portable_file_exists = bool(str(emulator_dir) and (emulator_dir / "portable.txt").exists())
    default_portable = is_canary and sys.platform == "win32"
    portable_override = launch_overrides.get("portable")
    portable_mode = portable_file_exists or (
        portable_override if isinstance(portable_override, bool) else default_portable
    )

    storage_root_text = launch_overrides.get("storage_root", "")
    if isinstance(storage_root_text, str) and storage_root_text.strip():
        storage_root = Path(storage_root_text).expanduser().resolve()
    elif portable_mode and str(emulator_dir):
        storage_root = emulator_dir.resolve()
    else:
        storage_root = _default_user_storage_root()

    defaults = {
        "variant": "canary" if is_canary else "master",
        "config_path": "",
        "storage_root": str(storage_root.resolve()),
        "content_root": str((storage_root / "content").resolve()),
        "cache_root": str((storage_root / ("cache_host" if is_canary else "cache")).resolve()),
        "portable": "true" if portable_mode else "false",
    }

    content_root_override = launch_overrides.get("content_root", "")
    if isinstance(content_root_override, str) and content_root_override.strip():
        defaults["content_root"] = _resolve_setting_path(storage_root, content_root_override, "content")

    cache_root_override = launch_overrides.get("cache_root", "")
    if isinstance(cache_root_override, str) and cache_root_override.strip():
        defaults["cache_root"] = _resolve_setting_path(
            storage_root,
            cache_root_override,
            "cache_host" if is_canary else "cache",
        )

    config_candidates: list[Path] = []
    config_override = launch_overrides.get("config_path", "")
    if isinstance(config_override, str) and config_override.strip():
        config_candidates.append(Path(config_override).expanduser().resolve())

    for root in _unique_paths([candidate for candidate in (storage_root, emulator_dir) if str(candidate)]):
        config_candidates.extend((root / name).resolve() for name in _config_name_candidates(is_canary))

    for candidate in _unique_paths(config_candidates):
        if not candidate.exists() or not candidate.is_file():
            continue

        defaults["config_path"] = str(candidate.resolve())
        try:
            raw_content = candidate.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            return defaults

        sections = _parse_toml_sections(raw_content)
        storage = sections.get("storage", {})

        if not (isinstance(content_root_override, str) and content_root_override.strip()):
            defaults["content_root"] = _resolve_setting_path(
                storage_root,
                storage.get("content_root", ""),
                "content",
            )

        if not (isinstance(cache_root_override, str) and cache_root_override.strip()):
            defaults["cache_root"] = _resolve_setting_path(
                storage_root,
                storage.get("cache_root", ""),
                "cache_host" if is_canary else "cache",
            )
        return defaults

    return defaults


def _save_roots_for_title_dir(title_dir: Path) -> list[Path]:
    candidates: list[Path] = []

    for relative_path in (
        Path("00000001"),
        Path("Headers") / "00000001",
        Path("profile"),
    ):
        candidate = title_dir / relative_path
        if candidate.exists() and candidate.is_dir():
            candidates.append(candidate.resolve())

    return _unique_paths(candidates)


def _existing_xenia_save_roots(content_root: Path) -> list[Path]:
    discovered: list[Path] = []
    if not content_root.exists() or not content_root.is_dir():
        return discovered

    for first_level in sorted(content_root.iterdir(), key=lambda item: item.name.casefold()):
        if not first_level.is_dir():
            continue

        if _HEX16_RE.fullmatch(first_level.name):
            for title_dir in sorted(first_level.iterdir(), key=lambda item: item.name.casefold()):
                if title_dir.is_dir() and _HEX8_RE.fullmatch(title_dir.name):
                    discovered.extend(_save_roots_for_title_dir(title_dir))
            continue

        if _HEX8_RE.fullmatch(first_level.name):
            discovered.extend(_save_roots_for_title_dir(first_level))

    return _unique_paths(discovered)


def xenia_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    settings = xenia_directory_settings(emulator_path_text, launch_template, split_launch_template_args)
    content_root_text = settings.get("content_root", "")
    if not isinstance(content_root_text, str) or not content_root_text.strip():
        return []

    unique: list[str] = []
    seen: set[str] = set()
    for candidate in _existing_xenia_save_roots(Path(content_root_text).expanduser().resolve()):
        normalized = str(candidate.resolve())
        key = normalized.casefold()
        if key in seen:
            continue
        seen.add(key)
        unique.append(normalized)
    return unique


def xenia_state_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    del emulator_path_text, launch_template, split_launch_template_args
    return []
