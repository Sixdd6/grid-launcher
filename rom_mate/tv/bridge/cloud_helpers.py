from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from PySide6.QtCore import QObject, Signal, Slot

from rom_mate.core.api import api_get_bytes, api_get_json, api_post_multipart_json
from rom_mate.emulator import (
    azahar_save_path_overrides,
    cemu_save_path_overrides,
    dolphin_save_path_overrides,
    dolphin_state_path_overrides,
    eden_save_path_overrides,
    fbneo_save_path_overrides,
    fbneo_state_path_overrides,
    mame_save_path_overrides,
    mame_state_path_overrides,
    pcsx2_save_path_overrides,
    pcsx2_state_path_overrides,
    pico8_save_path_overrides,
    redream_save_path_overrides,
    redream_state_path_overrides,
    retroarch_directory_settings,
    rpcs3_save_path_overrides,
    split_configured_paths,
    xemu_save_path_overrides,
    xenia_save_path_overrides,
    xenia_state_path_overrides,
)
from rom_mate.emulator.azahar import azahar_state_path_overrides
from rom_mate.emulator.selection import EmulatorEntry
from rom_mate.library.cloud_restore import (
    latest_server_record,
    restore_single_save_payload,
    save_record_timestamp,
    server_records_from_payload,
)
from rom_mate.library.cloud_sync import cloud_sync_candidates_for_game
from rom_mate.library.cloud_transfer import SUPPORTED_IMAGE_EXTENSIONS

try:
    from rom_mate.emulator.selection import is_retroarch_emulator_name as _is_retroarch_emulator_name
except ImportError:
    def _is_retroarch_emulator_name(emulator_name: str) -> bool:
        return "retroarch" in emulator_name.strip().casefold()


# Module-level aliases for test patchability.
_api_get_json = api_get_json
_api_get_bytes = api_get_bytes
_api_post_multipart_json = api_post_multipart_json
_server_records_from_payload = server_records_from_payload
_latest_server_record = latest_server_record
_save_record_timestamp = save_record_timestamp
_restore_single_save_payload = restore_single_save_payload


def game_save_match_tokens(game: dict[str, str]) -> set[str]:
    tokens: set[str] = set()

    def add_token_variants(value: str) -> None:
        text = value.strip().casefold()
        if not text:
            return
        variants = {text, re.sub(r"[’']s\b", "", text).strip()}
        for variant in variants:
            if not variant:
                continue
            tokens.add(variant)
            compact = re.sub(r"[^a-z0-9]+", "", variant)
            if compact:
                tokens.add(compact)

    def add_nintendo_id_variants(value: str) -> None:
        raw_text = value.strip().upper()
        if not raw_text:
            return
        for matched in re.findall(r"\b[A-Z][A-Z0-9]{3,5}\b", raw_text):
            short_code = matched[:4].casefold()
            if short_code:
                tokens.add(short_code)
                ascii_hex = "".join(f"{ord(character):02x}" for character in matched[:4])
                if ascii_hex:
                    tokens.add(ascii_hex)
        for matched in re.findall(r"[0-9A-F]{16}", raw_text):
            normalized = matched.casefold()
            tokens.add(normalized)
            tokens.add(normalized[:8])
            tokens.add(normalized[8:])
        for high, low in re.findall(r"([0-9A-F]{8})[^0-9A-F]+([0-9A-F]{8})", raw_text):
            normalized_high = high.casefold()
            normalized_low = low.casefold()
            tokens.add(normalized_high)
            tokens.add(normalized_low)
            tokens.add(f"{normalized_high}{normalized_low}")

    title_value = game.get("title", "")
    if isinstance(title_value, str):
        add_token_variants(title_value)

    for field in ("title_id", "base_title_id"):
        value = game.get(field, "")
        if isinstance(value, str) and value.strip():
            add_token_variants(value)
            add_nintendo_id_variants(value)

    for field in ("rom_file_name", "extracted_path", "archive_path"):
        value = game.get(field, "")
        if not isinstance(value, str) or not value.strip():
            continue
        stem_value = Path(value).stem
        add_token_variants(stem_value)
        add_nintendo_id_variants(stem_value)

    ps3_game_id_value = game.get("ps3_game_id", "")
    ps3_game_id = ps3_game_id_value.strip().casefold() if isinstance(ps3_game_id_value, str) else ""
    if ps3_game_id:
        tokens.add(ps3_game_id)

    return {token for token in tokens if token}


def is_state_file_candidate(file_path: Path) -> bool:
    name = file_path.name.casefold()
    suffix = file_path.suffix.casefold()
    if suffix in SUPPORTED_IMAGE_EXTENSIONS:
        return False
    if suffix in {".state", ".savestate", ".st", ".ss", ".ppst", ".p2s"}:
        return True
    if ".state" in name:
        return True
    if re.search(r"[._]\d+\.sav$", name):
        return True
    if re.search(r"_resume\.sav$", name):
        return True
    return False


def emulator_name_matches_tokens(emulator_name: str, emulator_entry: dict[str, str], *tokens: str) -> bool:
    normalized_name = emulator_name.casefold() if isinstance(emulator_name, str) else ""
    entry_name_value = emulator_entry.get("name", "") if isinstance(emulator_entry, dict) else ""
    normalized_entry_name = entry_name_value.casefold() if isinstance(entry_name_value, str) else ""
    normalized_tokens = [token.casefold() for token in tokens if isinstance(token, str) and token.strip()]
    return any(token in normalized_name or token in normalized_entry_name for token in normalized_tokens)


def resolve_emulator_entry_for_game(game: dict[str, str], config: dict[str, Any]) -> tuple[str, EmulatorEntry | None]:
    platform = game.get("platform", "")
    if not isinstance(platform, str):
        return "", None

    default_emulators = config.get("default_emulators", {})
    default = default_emulators.get(platform, "") if isinstance(default_emulators, dict) else ""
    if not isinstance(default, str) or not default.strip():
        return "", None

    emulators = config.get("emulators", [])
    if not isinstance(emulators, list):
        return "", None

    for entry in emulators:
        if isinstance(entry, dict) and entry.get("name", "") == default:
            return default, entry
    return "", None


def resolve_emulator_save_directories(
    game: dict[str, str],
    emulator_name: str,
    emulator_entry: dict[str, str],
    save_type: str,
) -> list[str]:
    path_key = "save_paths" if save_type == "save" else "state_paths"
    configured_value = emulator_entry.get(path_key, "")
    if isinstance(configured_value, str) and configured_value.strip():
        return split_configured_paths(configured_value)

    emulator_path = emulator_entry.get("path", "")
    launch_template = emulator_entry.get("args", "")
    if not isinstance(emulator_path, str):
        emulator_path = ""
    if not isinstance(launch_template, str):
        launch_template = ""

    if _is_retroarch_emulator_name(emulator_name):
        settings = retroarch_directory_settings(emulator_path)
        if save_type == "save":
            for key in ("savefile_directory", "saves", "savefiles"):
                value = settings.get(key, "")
                if isinstance(value, str) and value.strip():
                    return [value.strip()]
        else:
            for key in ("savestate_directory", "states", "savestates"):
                value = settings.get(key, "")
                if isinstance(value, str) and value.strip():
                    return [value.strip()]
        return []

    if emulator_name_matches_tokens(emulator_name, emulator_entry, "dolphin"):
        if save_type == "save":
            return dolphin_save_path_overrides(emulator_path, launch_template, split_configured_paths)
        return dolphin_state_path_overrides(emulator_path, launch_template, split_configured_paths)

    if emulator_name_matches_tokens(emulator_name, emulator_entry, "pcsx2"):
        if save_type == "save":
            return pcsx2_save_path_overrides(emulator_path, launch_template, split_configured_paths)
        return pcsx2_state_path_overrides(emulator_path, launch_template, split_configured_paths)

    if save_type == "save" and emulator_name_matches_tokens(emulator_name, emulator_entry, "cemu"):
        return cemu_save_path_overrides(emulator_path, launch_template, split_configured_paths)

    if save_type == "save" and emulator_name_matches_tokens(emulator_name, emulator_entry, "rpcs3"):
        return rpcs3_save_path_overrides(emulator_path, launch_template, split_configured_paths)

    if emulator_name_matches_tokens(emulator_name, emulator_entry, "azahar"):
        if save_type == "save":
            return azahar_save_path_overrides(emulator_path, launch_template, split_configured_paths)
        return azahar_state_path_overrides(emulator_path, launch_template, split_configured_paths)

    if save_type == "save" and emulator_name_matches_tokens(emulator_name, emulator_entry, "eden"):
        return eden_save_path_overrides(emulator_path, launch_template, split_configured_paths)

    if emulator_name_matches_tokens(emulator_name, emulator_entry, "fbneo", "final burn"):
        if save_type == "save":
            return fbneo_save_path_overrides(emulator_path, launch_template, split_configured_paths)
        return fbneo_state_path_overrides(emulator_path, launch_template, split_configured_paths)

    if emulator_name_matches_tokens(emulator_name, emulator_entry, "mame"):
        if save_type == "save":
            return mame_save_path_overrides(emulator_path, launch_template, split_configured_paths)
        return mame_state_path_overrides(emulator_path, launch_template, split_configured_paths)

    if save_type == "save" and emulator_name_matches_tokens(emulator_name, emulator_entry, "pico8", "pico-8"):
        return pico8_save_path_overrides(emulator_path, launch_template, split_configured_paths)

    if emulator_name_matches_tokens(emulator_name, emulator_entry, "xenia"):
        if save_type == "save":
            return xenia_save_path_overrides(emulator_path, launch_template, split_configured_paths)
        return xenia_state_path_overrides(emulator_path, launch_template, split_configured_paths)

    if emulator_name_matches_tokens(emulator_name, emulator_entry, "redream"):
        if save_type == "save":
            return redream_save_path_overrides(emulator_path, launch_template, split_configured_paths)
        return redream_state_path_overrides(emulator_path, launch_template, split_configured_paths)

    if save_type == "save" and emulator_name_matches_tokens(emulator_name, emulator_entry, "xemu", "xemu.exe"):
        return xemu_save_path_overrides(emulator_path, launch_template, split_configured_paths)

    return []


def perform_tv_save_upload(
    config: dict[str, Any],
    game: dict[str, str],
    emulator_name: str,
    emulator_entry: dict[str, str],
    save_type: str,
) -> tuple[int, int, list[str]]:
    save_directories = resolve_emulator_save_directories(game, emulator_name, emulator_entry, save_type)
    if not save_directories:
        return 0, 0, ["No save directories found for this emulator"]

    candidates = cloud_sync_candidates_for_game(
        game,
        [Path(directory) for directory in save_directories],
        save_type,
        game_save_match_tokens,
        is_state_file_candidate,
    )
    total_count = len(candidates)
    if total_count == 0:
        return 0, 0, []

    rom_id = str(game.get("rom_id") or game.get("id") or "").strip()
    if not rom_id:
        return 0, total_count, ["Game has no ROM ID"]

    base_url = str(config.get("server_url", "") or "").rstrip("/")
    api_token = str(config.get("api_token", "") or "")
    endpoint = "/api/saves" if save_type == "save" else "/api/states"

    uploaded_count = 0
    failed_files: list[str] = []
    for candidate in candidates:
        try:
            _ = candidate.read_bytes()
            _api_post_multipart_json(
                base_url,
                api_token,
                endpoint,
                {"saveFile": candidate},
                {"rom_id": rom_id, "emulator": emulator_name},
            )
            uploaded_count += 1
        except Exception as error:
            failed_files.append(f"{candidate.name}: {error}")

    return uploaded_count, total_count, failed_files


class _TvAutoRestoreWorker(QObject):
    finished = Signal(bool, str)

    def __init__(self, config: dict[str, Any], game: dict[str, str], emulator_name: str, emulator_entry: dict[str, str]) -> None:
        super().__init__()
        self._config = config
        self._game = game
        self._emulator_name = emulator_name
        self._emulator_entry = emulator_entry

    @Slot()
    def run(self) -> None:
        try:
            rom_id = str(self._game.get("rom_id") or self._game.get("id") or "").strip()
            if not rom_id:
                self.finished.emit(False, "")
                return

            base_url = str(self._config.get("server_url", "") or "").rstrip("/")
            api_token = str(self._config.get("api_token", "") or "")
            raw_payload = _api_get_json(base_url, api_token, "/api/saves", {"rom_id": rom_id})
            records = _server_records_from_payload(raw_payload)
            record = _latest_server_record(records, self._emulator_name, _save_record_timestamp)
            if not record:
                self.finished.emit(False, "")
                return

            payload_bytes = _api_get_bytes(base_url, api_token, f"/api/saves/{record['id']}/content")
            directories = resolve_emulator_save_directories(self._game, self._emulator_name, self._emulator_entry, "save")

            if not directories:
                install_dir_value = self._game.get("install_dir", "")
                if isinstance(install_dir_value, str) and install_dir_value.strip():
                    directories = [install_dir_value.strip()]
                else:
                    local_path_value = self._game.get("local_path", "")
                    if isinstance(local_path_value, str) and local_path_value.strip():
                        directories = [str(Path(local_path_value.strip()).parent)]

            _restore_single_save_payload(
                [Path(directory) for directory in directories if directory],
                record,
                payload_bytes,
                [],
                str(self._game.get("title", "") or ""),
            )
            self.finished.emit(True, "Save restored from cloud.")
        except Exception as error:
            self.finished.emit(False, str(error))