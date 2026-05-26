from __future__ import annotations

import json
import os
import re
import subprocess
import time
import zipfile
from datetime import UTC, datetime
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

from PySide6.QtCore import QThread, QTimer
from PySide6.QtWidgets import QMessageBox

from rom_mate.background import AutoCloudSaveUploadWorker
from rom_mate.emulator import (
    azahar_save_path_overrides,
    azahar_state_path_overrides,
    cemu_save_path_overrides,
    cloud_save_block_reason_for_game,
    cloud_save_scope_for_game,
    dolphin_save_path_overrides,
    dolphin_state_path_overrides,
    eden_save_path_overrides,
    emulator_entry_matches_tokens,
    fbneo_save_path_overrides,
    fbneo_state_path_overrides,
    mame_save_path_overrides,
    mame_state_path_overrides,
    pcsx2_save_path_overrides,
    pcsx2_state_path_overrides,
    pcsx2_windows_documents_folder,
    pico8_save_path_overrides,
    redream_save_path_overrides,
    redream_state_path_overrides,
    retroarch_directory_settings,
    rpcs3_save_path_overrides,
    xemu_save_path_overrides,
    xenia_save_path_overrides,
    xenia_state_path_overrides,
)
from rom_mate.emulator.retroarch import flycast_vmu_file_candidates
from rom_mate.emulator.retroarch import retroarch_core_flags
from rom_mate.emulator.retroarch import retroarch_core_flags_for_platform
from rom_mate.library import (
    auto_cloud_upload_plan,
    cemu_save_directories_for_game,
    cleanup_temporary_paths,
    cloud_sync_candidates_for_game,
    cloud_sync_directory_candidates_for_game,
    directory_archive_upload_jobs,
    extract_zip_archive_bytes_to_directory,
    filter_upload_jobs_by_session_window,
    grouped_file_upload_jobs,
    is_local_newer_than_server,
    latest_server_record,
    latest_server_records_by_slot,
    no_matching_upload_message,
    partition_active_game_sessions,
    ppsspp_state_upload_jobs,
    relative_timestamp_text,
    restore_single_save_payload,
    restore_single_state_payload,
    retroarch_state_upload_jobs,
    save_record_timestamp,
    screenshot_download_candidate_paths,
    server_records_from_payload,
    session_cloud_sync_updates,
    session_filtered_directory_candidates,
    session_filtered_file_candidates,
    should_skip_known_latest,
    sort_server_records_by_recency,
    state_download_candidate_paths,
    summarize_auto_cloud_upload_result,
    upload_completion_message,
    zip_directory_for_upload,
    zip_selected_files_for_upload,
)
from rom_mate.library.cloud_transfer import (
    SUPPORTED_IMAGE_EXTENSIONS,
    normalize_candidate_url,
    resolve_native_save_dir,
    session_screenshot_path,
    zip_native_save_dirs_for_upload,
)


class CloudSaveMixin:
    """Mixin containing cloud save/restore/sync methods for MainWindow."""

    def _cloud_save_block_reason_for_game(
        self,
        game: dict[str, str],
        emulator_name: str = "",
        emulator: dict[str, str] | None = None,
        *,
        save_type: str = "save",
    ) -> str:
        resolved_emulator_name = emulator_name.strip()
        if not resolved_emulator_name and isinstance(emulator, dict):
            emulator_value = emulator.get("name", "")
            if isinstance(emulator_value, str):
                resolved_emulator_name = emulator_value.strip()

        platform_value = game.get("platform", "")
        platform = platform_value.strip() if isinstance(platform_value, str) else ""
        if not resolved_emulator_name and platform:
            resolved_emulator_name = self._default_emulator_name_for_platform(platform)

        flags: dict[str, bool] | None = None
        if resolved_emulator_name and self._is_retroarch_emulator_name(resolved_emulator_name, emulator):
            core_defaults = self._normalize_default_retroarch_cores(
                self.config.get("default_retroarch_cores", {})
            )
            core_id = self._mapping_value_for_platform(core_defaults, platform)
            if core_id:
                flags = retroarch_core_flags(core_id, self._retroarch_core_list_entries())

        return cloud_save_block_reason_for_game(
            game,
            is_native_executable_platform=self._is_native_executable_platform,
            emulator_name=resolved_emulator_name,
            is_xemu_emulator_name=lambda value: self._is_xemu_emulator_name(value, emulator),
            is_redream_emulator_name=lambda value: self._is_redream_emulator_name(value, emulator),
            is_retroarch_emulator_name=self._is_retroarch_emulator_name,
            retroarch_core_flags=flags,
            save_type=save_type,
        )

    def _cloud_save_scope_for_game(
        self,
        game: dict[str, str],
        emulator_name: str = "",
        emulator: dict[str, str] | None = None,
        *,
        save_type: str = "save",
    ) -> str:
        resolved_emulator_name = emulator_name.strip()
        if not resolved_emulator_name and isinstance(emulator, dict):
            emulator_value = emulator.get("name", "")
            if isinstance(emulator_value, str):
                resolved_emulator_name = emulator_value.strip()

        platform_value = game.get("platform", "")
        platform = platform_value.strip() if isinstance(platform_value, str) else ""
        if not resolved_emulator_name and platform:
            resolved_emulator_name = self._default_emulator_name_for_platform(platform)

        flags: dict[str, bool] | None = None
        if resolved_emulator_name and self._is_retroarch_emulator_name(resolved_emulator_name, emulator):
            core_defaults = self._normalize_default_retroarch_cores(
                self.config.get("default_retroarch_cores", {})
            )
            core_id = self._mapping_value_for_platform(core_defaults, platform)
            if core_id:
                flags = retroarch_core_flags(core_id, self._retroarch_core_list_entries())
            elif platform:
                flags = retroarch_core_flags_for_platform(platform, self._retroarch_core_list_entries())

        return cloud_save_scope_for_game(
            game,
            emulator_name=resolved_emulator_name,
            is_xemu_emulator_name=lambda value: self._is_xemu_emulator_name(value, emulator),
            is_redream_emulator_name=lambda value: self._is_redream_emulator_name(value, emulator),
            is_retroarch_emulator_name=self._is_retroarch_emulator_name,
            retroarch_core_flags=flags,
            save_type=save_type,
        )

    def _resolved_cloud_emulator_entry_for_game(
        self,
        game: dict[str, str],
        *,
        save_type: str = "save",
    ) -> tuple[str, dict[str, str] | None]:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start(
            "_resolved_cloud_emulator_entry_for_game",
            save_type=save_type,
            title=game.get("title", "") if isinstance(game, dict) else "",
            platform=game.get("platform", "") if isinstance(game, dict) else "",
        ) if callable(timing_start) else 0.0
        _title = game.get("title", "") if isinstance(game, dict) else ""
        _platform = game.get("platform", "") if isinstance(game, dict) else ""
        _cache_key = f"{_title}::{_platform}::{save_type}"
        if _cache_key in self._cloud_emulator_entry_cache:
            _cached = self._cloud_emulator_entry_cache[_cache_key]
            if callable(timing_end):
                timing_end("_resolved_cloud_emulator_entry_for_game", started_at, emulator=_cached[0], source="cached")
            return _cached
        resolved_game = self._installed_game_record(game) or game
        emulator_name, emulator_entry = self._resolved_emulator_entry_for_game(resolved_game)
        if emulator_entry is not None:
            if callable(timing_end):
                timing_end("_resolved_cloud_emulator_entry_for_game", started_at, emulator=emulator_name, source="default")
            _result: tuple[str, dict[str, str] | None] = (emulator_name, emulator_entry)
            self._cloud_emulator_entry_cache[_cache_key] = _result
            return _result
        if not self._is_emulators_platform(resolved_game):
            if callable(timing_end):
                timing_end("_resolved_cloud_emulator_entry_for_game", started_at, source="none")
            _result = (emulator_name, emulator_entry)
            self._cloud_emulator_entry_cache[_cache_key] = _result
            return _result

        for candidate in self._normalize_emulators(self._emulators()):
            candidate_name = str(candidate.get("name", "")).strip()
            if not candidate_name:
                continue
            if not self._emulator_game_matches_shared_sync(resolved_game, candidate_name, candidate):
                continue
            if save_type == "save" and self._cloud_save_scope_for_game(
                resolved_game,
                candidate_name,
                candidate,
                save_type=save_type,
            ) == "per-game":
                continue
            if callable(timing_end):
                timing_end("_resolved_cloud_emulator_entry_for_game", started_at, emulator=candidate_name, source="shared")
            _shared_result: tuple[str, dict[str, str] | None] = (candidate_name, candidate)
            self._cloud_emulator_entry_cache[_cache_key] = _shared_result
            return _shared_result

        if callable(timing_end):
            timing_end("_resolved_cloud_emulator_entry_for_game", started_at, source="unresolved")
        _unresolved_result: tuple[str, dict[str, str] | None] = (emulator_name, emulator_entry)
        self._cloud_emulator_entry_cache[_cache_key] = _unresolved_result
        return _unresolved_result

    def _details_cloud_button_text(self, game: dict[str, str], save_type: str) -> str:
        if save_type != "save":
            return "Manage States"

        resolved_game = self._installed_game_record(game) or game
        emulator_name, emulator_entry = self._resolved_cloud_emulator_entry_for_game(
            resolved_game,
            save_type=save_type,
        )
        if emulator_entry is not None and self._cloud_save_scope_for_game(
            resolved_game,
            emulator_name,
            emulator_entry,
            save_type=save_type,
        ) != "per-game":
            return "Emulator Saves"
        return "Manage Saves"

    def _details_cloud_scope_notice(
        self,
        game: dict[str, str],
        emulator_name: str = "",
        emulator: dict[str, str] | None = None,
        *,
        save_type: str = "save",
    ) -> str:
        if save_type != "save":
            return ""

        resolved_game = self._installed_game_record(game) or game
        resolved_emulator_name = emulator_name
        resolved_emulator = emulator
        if not resolved_emulator_name or resolved_emulator is None:
            resolved_emulator_name, resolved_emulator = self._resolved_cloud_emulator_entry_for_game(
                resolved_game,
                save_type=save_type,
            )
        if resolved_emulator is None:
            return ""

        save_scope = self._cloud_save_scope_for_game(
            resolved_game,
            resolved_emulator_name,
            resolved_emulator,
            save_type=save_type,
        )
        emulator_label = resolved_emulator_name.strip() or "this emulator"
        if save_scope == "shared-single":
            return (
                f"These cloud saves are shared {emulator_label} media. Restoring or deleting one affects every game "
                "using this emulator."
            )
        if save_scope == "shared-slotted":
            return (
                f"These cloud saves are shared {emulator_label} memory-card backups. Deleting one removes the "
                "backup for every game using that emulator slot."
            )
        return ""

    def _details_cloud_mode_supported(self, game: dict[str, str], save_type: str) -> bool:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start(
            "_details_cloud_mode_supported",
            save_type=save_type,
            title=game.get("title", "") if isinstance(game, dict) else "",
            platform=game.get("platform", "") if isinstance(game, dict) else "",
        ) if callable(timing_start) else 0.0
        if save_type not in {"save", "state"}:
            if callable(timing_end):
                timing_end("_details_cloud_mode_supported", started_at, result=False, reason="invalid-type")
            return False
        if self._is_native_executable_platform(game):
            if save_type == "state":
                if callable(timing_end):
                    timing_end("_details_cloud_mode_supported", started_at, result=False, reason="native-no-states")
                return False
            # Allow save mode for installed native games
            installed_game = self._installed_game_record(game)
            result = installed_game is not None
            if callable(timing_end):
                timing_end("_details_cloud_mode_supported", started_at, result=result, reason="native-save")
            return result

        installed_game = self._installed_game_record(game)
        resolved_game = installed_game or game
        if installed_game is None and not self._is_emulators_platform(resolved_game):
            if callable(timing_end):
                timing_end("_details_cloud_mode_supported", started_at, result=False, reason="not-installed")
            return False

        emulator_name, emulator_entry = self._resolved_cloud_emulator_entry_for_game(
            resolved_game,
            save_type=save_type,
        )
        if emulator_entry is None:
            if callable(timing_end):
                timing_end("_details_cloud_mode_supported", started_at, result=False, reason="no-emulator")
            return False

        save_scope = self._cloud_save_scope_for_game(
            resolved_game,
            emulator_name,
            emulator_entry,
            save_type=save_type,
        )
        if save_type == "save" and self._is_emulators_platform(resolved_game) and save_scope == "per-game":
            if callable(timing_end):
                timing_end("_details_cloud_mode_supported", started_at, result=False, reason="per-game-emulator-entry")
            return False
        if save_type == "state" and (
            self._is_emulators_platform(resolved_game)
            or self._is_rpcs3_emulator_name(emulator_name, emulator_entry)
        ):
            if callable(timing_end):
                timing_end("_details_cloud_mode_supported", started_at, result=False, reason="state-blocked")
            return False
        if self._cloud_save_block_reason_for_game(
            resolved_game,
            emulator_name,
            emulator_entry,
            save_type=save_type,
        ):
            if callable(timing_end):
                timing_end("_details_cloud_mode_supported", started_at, result=False, reason="compatibility-blocked")
            return False

        directory_key = "save_paths" if save_type == "save" else "state_paths"
        result = bool(self._resolved_sync_directory_paths(emulator_entry, directory_key))
        if callable(timing_end):
            timing_end(
                "_details_cloud_mode_supported",
                started_at,
                result=result,
                emulator=emulator_name,
                scope=save_scope,
            )
        return result

    def _emulator_game_matches_shared_sync(
        self,
        game: dict[str, str],
        emulator_name: str,
        emulator: dict[str, str] | None = None,
    ) -> bool:
        if not self._is_emulators_platform(game):
            return False

        candidate_text = " ".join(
            str(game.get(field, "")).strip()
            for field in ("title", "platform", "description", "rom_file_name")
        ).casefold()
        if not candidate_text:
            return False

        if self._is_xemu_emulator_name(emulator_name, emulator):
            return "xemu" in candidate_text
        if self._is_redream_emulator_name(emulator_name, emulator):
            return "redream" in candidate_text
        return False

    def _shared_cloud_sync_owner_game(
        self,
        emulator_name: str,
        emulator: dict[str, str] | None = None,
        *,
        save_type: str = "save",
    ) -> dict[str, str] | None:
        if self._cloud_save_scope_for_game({}, emulator_name, emulator, save_type=save_type) == "per-game":
            return None

        candidates: list[dict[str, str]] = []

        for game in self.library_games:
            if self._emulator_game_matches_shared_sync(game, emulator_name, emulator):
                candidates.append(game)

        for games in self.server_games_by_platform.values():
            for game in games:
                if self._emulator_game_matches_shared_sync(game, emulator_name, emulator):
                    candidates.append(game)

        if not candidates:
            path_value = emulator.get("path", "") if isinstance(emulator, dict) else ""
            path_text = path_value.strip() if isinstance(path_value, str) else ""
            if path_text:
                try:
                    candidates.extend(self._matching_installed_emulator_games(Path(path_text).expanduser()))
                except OSError:
                    pass

        seen_keys: set[tuple[str, str]] = set()
        for candidate in candidates:
            candidate_key = self._game_key(candidate)
            if candidate_key in seen_keys:
                continue
            seen_keys.add(candidate_key)

            owner_rom_id = self._resolve_rom_id_for_game(candidate)
            if owner_rom_id:
                return candidate
        return None

    def _cloud_sync_rom_id_for_game(
        self,
        game: dict[str, str],
        *,
        save_type: str = "save",
        emulator_name: str = "",
        emulator: dict[str, str] | None = None,
    ) -> str:
        resolved_game = self._installed_game_record(game) or game
        resolved_emulator_name = emulator_name
        resolved_emulator = emulator
        if not resolved_emulator_name or resolved_emulator is None:
            resolved_emulator_name, resolved_emulator = self._resolved_cloud_emulator_entry_for_game(
                resolved_game,
                save_type=save_type,
            )

        if save_type == "save":
            owner_game = self._shared_cloud_sync_owner_game(
                resolved_emulator_name,
                resolved_emulator,
                save_type=save_type,
            )
            if owner_game is not None:
                owner_rom_id = self._resolve_rom_id_for_game(owner_game)
                if owner_rom_id:
                    return owner_rom_id

        return self._resolve_rom_id_for_game(resolved_game)

    def _session_filtered_file_candidates(self, game: dict[str, str], files: list[Path]) -> list[Path]:
        return session_filtered_file_candidates(files, self._session_window_for_state_upload(game))

    def _session_filtered_directory_candidates(
        self,
        game: dict[str, str],
        directories: list[Path],
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> list[Path]:
        return session_filtered_directory_candidates(
            directories,
            self._session_window_for_state_upload(game),
            self._latest_file_mtime_under_path,
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )

    def _cloud_sync_directory_candidates_for_game(
        self,
        game: dict[str, str],
        directories: list[Path],
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> list[Path]:
        return cloud_sync_directory_candidates_for_game(
            game,
            directories,
            self._game_save_match_tokens,
            self._latest_file_mtime_under_path,
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )

    def _cloud_sync_targets_for_game(
        self,
        game: dict[str, str],
        emulator_name: str,
        emulator: dict[str, str],
        directories: list[Path],
        save_type: str,
    ) -> tuple[list[Path], list[Path]]:
        files: list[Path] = []
        folder_targets: list[Path] = []
        save_strategy = self._resolved_save_strategy_for_emulator(emulator, save_type)
        ignore_basenames = self._sync_directory_ignore_basenames_for_emulator(emulator_name, emulator, save_type)
        ignore_extensions = self._sync_directory_ignore_extensions_for_emulator(emulator)

        explicit_file_roots = [path for path in directories if path.exists() and path.is_file()]

        if save_type == "state":
            files = self._cloud_sync_candidates_for_game(
                game,
                directories,
                "state",
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )
            files = self._session_filtered_file_candidates(game, files)
            return files, folder_targets

        if self._is_cemu_emulator_name(emulator_name, emulator):
            folder_targets = self._cemu_save_directories_for_game(
                game,
                directories,
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )
        elif self._is_dolphin_emulator_name(emulator_name, emulator):
            files, folder_targets = self._dolphin_save_targets_for_game(
                game,
                directories,
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )
        elif save_strategy == "folder":
            folder_targets = self._cloud_sync_directory_candidates_for_game(
                game,
                directories,
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )
        elif (
            self._is_retroarch_emulator_name(emulator_name, emulator)
            and self._cloud_save_scope_for_game(game, emulator_name, emulator, save_type="save") == "shared-slotted"
        ):
            vmu_files = flycast_vmu_file_candidates(directories)
            if vmu_files:
                files = self._cloud_sync_candidates_for_game(
                    game,
                    vmu_files,
                    "save",
                    ignore_basenames=ignore_basenames,
                    ignore_extensions=ignore_extensions,
                )
            else:
                files = self._cloud_sync_candidates_for_game(
                    game,
                    directories,
                    "save",
                    ignore_basenames=ignore_basenames,
                    ignore_extensions=ignore_extensions,
                )
        elif save_strategy == "single_file":
            files = self._cloud_sync_candidates_for_game(
                game,
                directories,
                "save",
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )
        elif self._is_ppsspp_emulator_name(emulator_name, emulator):
            folder_targets = self._ppsspp_save_directories_for_game(game, directories)
        elif self._is_rpcs3_emulator_name(emulator_name, emulator):
            folder_targets = self._rpcs3_save_directories_for_game(game, directories)
        elif self._is_pcsx2_emulator_name(emulator_name, emulator):
            folder_targets = self._pcsx2_save_directories_for_game(game, directories)
        else:
            files = self._cloud_sync_candidates_for_game(
                game,
                directories,
                "save",
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )

        if not files and not folder_targets and explicit_file_roots:
            files = self._cloud_sync_candidates_for_game(
                game,
                explicit_file_roots,
                "save",
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )

        if files:
            files = self._session_filtered_file_candidates(game, files)
        if folder_targets:
            folder_targets = self._session_filtered_directory_candidates(
                game,
                folder_targets,
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )
        return files, folder_targets

    def _resolved_sync_directory_paths(self, emulator: dict[str, str], key: str) -> list[Path]:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        emulator_name_value = emulator.get("name", "")
        emulator_name = emulator_name_value if isinstance(emulator_name_value, str) else ""
        started_at = timing_start("_resolved_sync_directory_paths", emulator=emulator_name, key=key) if callable(timing_start) else 0.0
        emulator_path_raw = emulator.get("path", "")
        emulator_path_raw = emulator_path_raw if isinstance(emulator_path_raw, str) else ""
        _sync_cache_key = (emulator_name, emulator_path_raw, key)
        _sync_cache = getattr(self, "_sync_directory_paths_cache", None)
        if _sync_cache is not None and _sync_cache_key in _sync_cache:
            _cached_result = _sync_cache[_sync_cache_key]
            if callable(timing_end):
                timing_end("_resolved_sync_directory_paths", started_at, result=len(_cached_result))
            return list(_cached_result)
        configured_value = emulator.get(key, "")
        configured_paths = self._split_configured_paths(configured_value) if isinstance(configured_value, str) else []


        profile = self._emulator_profile_for_entry(emulator)
        profile_key = "save_directories" if key == "save_paths" else "state_directories"
        profile_paths: list[str] = []
        if isinstance(profile, dict):
            raw_profile_paths = profile.get(profile_key, [])
            if isinstance(raw_profile_paths, list):
                profile_paths = [item.strip() for item in raw_profile_paths if isinstance(item, str) and item.strip()]

        all_paths = configured_paths if configured_paths else profile_paths
        self._ensure_emulator_sync_settings(emulator_name, emulator.get("path", ""))
        if not configured_paths and self._is_retroarch_emulator_name(emulator_name, emulator):
            directory_settings = retroarch_directory_settings(emulator.get("path", ""))
            override_key = "savefile_directory" if key == "save_paths" else "savestate_directory"
            override_path = directory_settings.get(override_key, "")
            if isinstance(override_path, str) and override_path.strip():
                all_paths = [override_path.strip(), *all_paths]

            fallback_paths = ["saves", "savefiles"] if key == "save_paths" else ["states", "savestates"]
            merged_paths: list[str] = []
            for raw_path in [*all_paths, *fallback_paths]:
                if not isinstance(raw_path, str) or not raw_path.strip():
                    continue
                if raw_path not in merged_paths:
                    merged_paths.append(raw_path)
            all_paths = merged_paths

        if not configured_paths and self._is_azahar_emulator_name(emulator_name, emulator):
            azahar_override_paths: list[str] = []
            if key == "save_paths":
                azahar_override_paths = azahar_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                azahar_override_paths = azahar_state_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )

            if azahar_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*azahar_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and self._is_dolphin_emulator_name(emulator_name, emulator):
            dolphin_override_paths: list[str] = []
            if key == "save_paths":
                dolphin_override_paths = dolphin_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                dolphin_override_paths = dolphin_state_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )

            if dolphin_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*dolphin_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and self._is_pcsx2_emulator_name(emulator_name, emulator):
            pcsx2_override_paths: list[str] = []
            if key == "save_paths":
                pcsx2_override_paths = pcsx2_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                pcsx2_override_paths = pcsx2_state_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )

            if pcsx2_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*pcsx2_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and key == "save_paths" and self._is_rpcs3_emulator_name(emulator_name, emulator):
            rpcs3_override_paths = rpcs3_save_path_overrides(
                emulator.get("path", ""),
                emulator.get("args", ""),
                self._split_launch_template_args,
            )
            if rpcs3_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*rpcs3_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and key == "save_paths" and self._is_cemu_emulator_name(emulator_name, emulator):
            cemu_override_paths = cemu_save_path_overrides(
                emulator.get("path", ""),
                emulator.get("args", ""),
                self._split_launch_template_args,
            )
            if cemu_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*cemu_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and key == "save_paths" and self._is_pico8_emulator_name(emulator_name, emulator):
            pico8_override_paths = pico8_save_path_overrides(
                emulator.get("path", ""),
                emulator.get("args", ""),
                self._split_launch_template_args,
            )
            if pico8_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*pico8_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and self._is_fbneo_emulator_name(emulator_name, emulator):
            fbneo_override_paths: list[str] = []
            if key == "save_paths":
                fbneo_override_paths = fbneo_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                fbneo_override_paths = fbneo_state_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )

            if fbneo_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*fbneo_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and self._is_mame_emulator_name(emulator_name, emulator):
            mame_override_paths: list[str] = []
            if key == "save_paths":
                mame_override_paths = mame_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                mame_override_paths = mame_state_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )

            if mame_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*mame_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and key == "save_paths" and self._is_eden_emulator_name(emulator_name, emulator):
            eden_override_paths = eden_save_path_overrides(
                emulator.get("path", ""),
                emulator.get("args", ""),
                self._split_launch_template_args,
            )
            if eden_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*eden_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and self._is_xenia_emulator_name(emulator_name, emulator):
            xenia_override_paths: list[str] = []
            if key == "save_paths":
                xenia_override_paths = xenia_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                xenia_override_paths = xenia_state_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )

            if xenia_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*xenia_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and self._is_redream_emulator_name(emulator_name, emulator):
            redream_override_paths: list[str] = []
            if key == "save_paths":
                redream_override_paths = redream_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                redream_override_paths = redream_state_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )

            if redream_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*redream_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not configured_paths and key == "save_paths" and self._is_xemu_emulator_name(emulator_name, emulator):
            xemu_override_paths = xemu_save_path_overrides(
                emulator.get("path", ""),
                emulator.get("args", ""),
                self._split_launch_template_args,
            )
            if xemu_override_paths:
                merged_paths: list[str] = []
                for raw_path in [*xemu_override_paths, *all_paths]:
                    if not isinstance(raw_path, str) or not raw_path.strip():
                        continue
                    if raw_path not in merged_paths:
                        merged_paths.append(raw_path)
                all_paths = merged_paths

        if not all_paths:
            if callable(timing_end):
                timing_end("_resolved_sync_directory_paths", started_at, result=0)
            if _sync_cache is not None:
                _sync_cache[_sync_cache_key] = []
            return []

        emulator_path_value = emulator.get("path", "")
        emulator_path = Path(emulator_path_value).expanduser() if isinstance(emulator_path_value, str) else Path()
        emulator_dir = emulator_path.parent if emulator_path_value else Path()

        library_value = self.config.get("library_path", "")
        library_path = Path(library_value).expanduser() if isinstance(library_value, str) and library_value.strip() else Path()
        config_dir = self._config_dir()
        win_docs = pcsx2_windows_documents_folder()
        documents_str = str(win_docs) if win_docs is not None else os.path.join(os.path.expandvars("%USERPROFILE%"), "Documents")

        resolved: list[Path] = []
        for raw_path in all_paths:
            expanded = os.path.expandvars(raw_path)
            replacements = {
                "%EMULATOR_DIR%": str(emulator_dir),
                "%LIBRARY_DIR%": str(library_path),
                "%CONFIG_DIR%": str(config_dir),
                "%DOCUMENTS%": documents_str,
            }
            for token, token_value in replacements.items():
                expanded = expanded.replace(token, token_value)

            stripped = expanded.strip()
            if self._is_retroarch_emulator_name(emulator_name, emulator) and stripped.lower() == "default":
                default_dir = "saves" if key == "save_paths" else "states"
                candidate = (emulator_dir / default_dir).resolve()
            elif self._is_retroarch_emulator_name(emulator_name, emulator) and (
                stripped.startswith(":\\") or stripped.startswith(":/")
            ):
                # RetroArch root-relative notation - strip the :\ or :/ prefix.
                candidate = (emulator_dir / stripped[2:]).resolve()
            else:
                candidate = Path(expanded).expanduser()
                if not candidate.is_absolute() and emulator_dir:
                    candidate = (emulator_dir / candidate).resolve()
                elif candidate.is_absolute():
                    candidate = candidate.resolve()

            if candidate.exists() and (candidate.is_dir() or candidate.is_file()):
                resolved.append(candidate)

        unique: list[Path] = []
        seen: set[str] = set()
        for path in resolved:
            key_value = str(path).casefold()
            if key_value in seen:
                continue
            seen.add(key_value)
            unique.append(path)
        if callable(timing_end):
            timing_end("_resolved_sync_directory_paths", started_at, result=len(unique))
        if _sync_cache is not None:
            _sync_cache[_sync_cache_key] = list(unique)
        return unique

    def _resolved_screenshot_directories(self, emulator: dict[str, str]) -> list[Path]:
        profile = self._emulator_profile_for_entry(emulator)
        profile_paths: list[str] = []
        if isinstance(profile, dict):
            raw_profile_paths = profile.get("screenshot_directories", [])
            if isinstance(raw_profile_paths, list):
                profile_paths = [item.strip() for item in raw_profile_paths if isinstance(item, str) and item.strip()]

        if not profile_paths:
            return []

        emulator_path_value = emulator.get("path", "")
        emulator_path = Path(emulator_path_value).expanduser() if isinstance(emulator_path_value, str) else Path()
        emulator_dir = emulator_path.parent if emulator_path_value else Path()

        library_value = self.config.get("library_path", "")
        library_path = Path(library_value).expanduser() if isinstance(library_value, str) and library_value.strip() else Path()
        config_dir = self._config_dir()

        resolved: list[Path] = []
        for raw_path in profile_paths:
            expanded = os.path.expandvars(raw_path)
            replacements = {
                "%EMULATOR_DIR%": str(emulator_dir),
                "%LIBRARY_DIR%": str(library_path),
                "%CONFIG_DIR%": str(config_dir),
            }
            for token, token_value in replacements.items():
                expanded = expanded.replace(token, token_value)

            candidate = Path(expanded).expanduser()
            if not candidate.is_absolute() and emulator_dir:
                candidate = (emulator_dir / candidate).resolve()
            elif candidate.is_absolute():
                candidate = candidate.resolve()

            if candidate.exists() and candidate.is_dir():
                resolved.append(candidate)

        unique: list[Path] = []
        seen: set[str] = set()
        for path in resolved:
            key_value = str(path).casefold()
            if key_value in seen:
                continue
            seen.add(key_value)
            unique.append(path)
        return unique

    def _resolved_firmware_directories(self, emulator: dict[str, str]) -> list:
        profile = self._emulator_profile_for_entry(emulator)
        profile_paths: list = []
        if isinstance(profile, dict):
            raw_profile_paths = profile.get("firmware_directories", [])
            if isinstance(raw_profile_paths, list):
                profile_paths = raw_profile_paths

        if not profile_paths:
            return []

        emulator_path_value = emulator.get("path", "")
        emulator_path = Path(emulator_path_value).expanduser() if isinstance(emulator_path_value, str) else Path()
        emulator_dir = emulator_path.parent if emulator_path_value else Path()

        library_value = self.config.get("library_path", "")
        library_path = Path(library_value).expanduser() if isinstance(library_value, str) and library_value.strip() else Path()
        config_dir = self._config_dir()

        resolved: list = []
        for raw_path in profile_paths:
            route_keywords: list[str] | None = None
            path_value = ""
            if isinstance(raw_path, str) and raw_path.strip():
                path_value = raw_path.strip()
            elif isinstance(raw_path, dict):
                candidate_path = raw_path.get("path", "")
                candidate_match = raw_path.get("match", [])
                if not isinstance(candidate_path, str) or not candidate_path.strip():
                    continue
                if not isinstance(candidate_match, list):
                    continue
                normalized_keywords = [
                    item.strip().lower()
                    for item in candidate_match
                    if isinstance(item, str) and item.strip()
                ]
                if not normalized_keywords:
                    continue
                path_value = candidate_path.strip()
                route_keywords = normalized_keywords
            else:
                continue

            expanded = os.path.expandvars(path_value)
            replacements = {
                "%EMULATOR_DIR%": str(emulator_dir),
                "%LIBRARY_DIR%": str(library_path),
                "%CONFIG_DIR%": str(config_dir),
            }
            for token, token_value in replacements.items():
                expanded = expanded.replace(token, token_value)

            candidate = Path(expanded).expanduser()
            if not candidate.is_absolute() and emulator_dir:
                candidate = (emulator_dir / candidate).resolve()
            elif candidate.is_absolute():
                candidate = candidate.resolve()

            if route_keywords is None:
                resolved.append(candidate)
            else:
                resolved.append((candidate, route_keywords))

        unique: list = []
        seen: set[str] = set()
        for entry in resolved:
            path = entry[0] if isinstance(entry, tuple) else entry
            key_value = str(path).casefold()
            if key_value in seen:
                continue
            seen.add(key_value)
            unique.append(entry)
        return unique

    def _cemu_save_directories_for_game(
        self,
        game: dict[str, str],
        directories: list[Path],
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> list[Path]:
        return cemu_save_directories_for_game(
            game,
            directories,
            self._cemu_title_id_tokens,
            self._latest_file_mtime_under_path,
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )

    def _dolphin_save_targets_for_game(
        self,
        game: dict[str, str],
        directories: list[Path],
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> tuple[list[Path], list[Path]]:
        files = self._cloud_sync_candidates_for_game(
            game,
            directories,
            "save",
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )
        folder_targets = self._cloud_sync_directory_candidates_for_game(
            game,
            directories,
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )
        return files, folder_targets

    def _pcsx2_save_directories_for_game(self, game: dict[str, str], directories: list[Path]) -> list[Path]:
        id_tokens = self._ps2_game_id_tokens(game)
        candidates: list[Path] = []

        for directory in directories:
            if not directory.exists() or not directory.is_dir():
                continue
            for child in directory.iterdir():
                if not child.is_dir():
                    continue
                if not any(candidate.is_file() for candidate in child.rglob("*")):
                    continue

                normalized_name = re.sub(r"[^A-Z0-9]+", "", child.name.upper())
                normalized_relative = re.sub(r"[^A-Z0-9]+", "", str(child.relative_to(directory)).upper())
                if id_tokens and not any(token in normalized_name or token in normalized_relative for token in id_tokens):
                    continue
                candidates.append(child)

        candidates.sort(key=lambda item: self._latest_file_mtime_under_path(item), reverse=True)
        unique: list[Path] = []
        seen: set[str] = set()
        for path in candidates:
            key_value = str(path).casefold()
            if key_value in seen:
                continue
            seen.add(key_value)
            unique.append(path)
        return unique

    def _rpcs3_save_directories_for_game(self, game: dict[str, str], directories: list[Path]) -> list[Path]:
        game_ids = self._ps3_game_ids_for_game(game)
        candidates: list[tuple[int, float, Path]] = []

        for directory_index, directory in enumerate(directories):
            if not directory.exists() or not directory.is_dir():
                continue
            for child in directory.iterdir():
                if not child.is_dir():
                    continue
                normalized_name = re.sub(r"[^A-Z0-9]+", "", child.name.upper())
                if game_ids and not any(game_id in normalized_name for game_id in game_ids):
                    continue
                candidates.append((directory_index, self._latest_file_mtime_under_path(child), child))

        candidates.sort(key=lambda item: (item[0], -item[1]))

        unique: list[Path] = []
        seen: set[str] = set()
        for _, _, path in candidates:
            key_value = str(path).casefold()
            if key_value in seen:
                continue
            seen.add(key_value)
            unique.append(path)
        return unique

    def _game_save_match_tokens(self, game: dict[str, str]) -> set[str]:
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

        title_value = game.get("title", "")
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

    def _cemu_title_id_tokens(self, game: dict[str, str]) -> set[str]:
        tokens: set[str] = set()

        def add_title_id_variants(value: str) -> None:
            text = value.strip().upper()
            if not text:
                return
            for matched in re.findall(r"[0-9A-F]{16}", text):
                normalized = matched.upper()
                tokens.add(normalized)
                tokens.add(normalized[:8])
                tokens.add(normalized[8:])
            for high, low in re.findall(r"([0-9A-F]{8})[^0-9A-F]+([0-9A-F]{8})", text):
                combined = f"{high}{low}"
                tokens.add(combined)
                tokens.add(high)
                tokens.add(low)

        for field in (
            "title_id",
            "base_title_id",
            "rom_id",
            "rom_file_name",
            "extracted_path",
            "archive_path",
            "extracted_dir",
            "native_executable_path",
        ):
            value = game.get(field, "")
            if isinstance(value, str) and value.strip():
                add_title_id_variants(value)

        xml_candidates: list[Path] = []
        for extracted_path in self._candidate_extracted_paths_for_game(game):
            parent = extracted_path.parent
            xml_candidates.extend([
                parent / "app.xml",
                parent / "meta.xml",
                parent / "code" / "app.xml",
                parent / "meta" / "meta.xml",
                parent.parent / "code" / "app.xml",
                parent.parent / "meta" / "meta.xml",
            ])

        extracted_dir_value = game.get("extracted_dir", "")
        extracted_dir_text = extracted_dir_value.strip() if isinstance(extracted_dir_value, str) else ""
        if extracted_dir_text:
            extracted_dir = Path(extracted_dir_text).expanduser()
            xml_candidates.extend([
                extracted_dir / "code" / "app.xml",
                extracted_dir / "meta" / "meta.xml",
            ])

        seen_paths: set[str] = set()
        for xml_path in xml_candidates:
            key_value = str(xml_path).casefold()
            if key_value in seen_paths or not xml_path.exists() or not xml_path.is_file():
                continue
            seen_paths.add(key_value)
            try:
                add_title_id_variants(xml_path.read_text(encoding="utf-8", errors="ignore"))
            except OSError:
                continue

        return {token for token in tokens if token}

    def _is_state_file_candidate(self, file_path: Path) -> bool:
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

    def _emulator_matches_tokens(
        self,
        emulator_name: str,
        *tokens: str,
        emulator: dict[str, str] | None = None,
    ) -> bool:
        entry = emulator
        if entry is None and isinstance(emulator_name, str) and emulator_name.strip():
            entry = self._emulator_entry_by_name(emulator_name)

        if emulator_entry_matches_tokens(entry, tokens, self._emulator_autoprofiles()):
            return True

        normalized_name = emulator_name.strip().casefold() if isinstance(emulator_name, str) else ""
        return any(token.strip().casefold() in normalized_name for token in tokens if isinstance(token, str) and token.strip())

    def _is_ppsspp_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "ppsspp", emulator=emulator)

    def _is_azahar_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "azahar", emulator=emulator)

    def _is_dolphin_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "dolphin", emulator=emulator)

    def _is_cemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "cemu", emulator=emulator)

    def _is_pico8_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "pico8", "pico-8", emulator=emulator)

    def _is_eden_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "eden", emulator=emulator)

    def _is_fbneo_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "fbneo", "final burn", emulator=emulator)

    def _is_mame_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "mame", emulator=emulator)

    def _is_xemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "xemu", "xemu.exe", emulator=emulator)

    def _is_xenia_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "xenia", emulator=emulator)

    def _is_redream_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "redream", emulator=emulator)

    def _is_pcsx2_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "pcsx2", emulator=emulator)

    def _ps2_game_id_tokens(self, game: dict[str, str]) -> set[str]:
        tokens: set[str] = set()
        for field in ("title", "rom_file_name", "extracted_path", "archive_path"):
            value = game.get(field, "")
            if not isinstance(value, str) or not value.strip():
                continue
            upper_value = value.strip().upper()
            for matched in re.findall(r"[A-Z]{4}[-_ ]?\d{3}\.\d{2}|[A-Z]{4}[-_ ]?\d{5}", upper_value):
                normalized = re.sub(r"[^A-Z0-9]+", "", matched)
                if normalized:
                    tokens.add(normalized)
        return tokens

    def _psp_game_id_tokens(self, game: dict[str, str]) -> set[str]:
        tokens: set[str] = set()
        for field in ("title", "rom_file_name", "extracted_path", "archive_path"):
            value = game.get(field, "")
            if not isinstance(value, str) or not value.strip():
                continue
            upper_value = value.strip().upper()
            for matched in re.findall(r"[A-Z]{4}[-_ ]?\d{5}", upper_value):
                normalized = re.sub(r"[^A-Z0-9]+", "", matched)
                if normalized:
                    tokens.add(normalized)
        return tokens

    def _ppsspp_save_directories_for_game(self, game: dict[str, str], directories: list[Path]) -> list[Path]:
        id_tokens = self._psp_game_id_tokens(game)
        candidates: list[Path] = []
        for directory in directories:
            if not directory.exists() or not directory.is_dir():
                continue
            for child in directory.iterdir():
                if not child.is_dir():
                    continue
                normalized_name = re.sub(r"[^A-Z0-9]+", "", child.name.upper())
                if id_tokens and not any(token in normalized_name for token in id_tokens):
                    continue
                candidates.append(child)

        candidates.sort(key=lambda item: item.stat().st_mtime if item.exists() else 0, reverse=True)
        unique: list[Path] = []
        seen: set[str] = set()
        for path in candidates:
            key_value = str(path).casefold()
            if key_value in seen:
                continue
            seen.add(key_value)
            unique.append(path)
        return unique

    def _zip_directory_for_upload(
        self,
        directory: Path,
        game: dict[str, str],
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> Path:
        title_value = game.get("title", "Game")
        title = title_value if isinstance(title_value, str) else "Game"
        safe_title = self._sanitize_path_component(title, "game")
        return zip_directory_for_upload(
            directory,
            safe_title,
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )

    def _ppsspp_state_upload_jobs(
        self,
        game: dict[str, str],
        directories: list[Path],
        file_field: str,
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> list[tuple[str, dict[str, Path]]]:
        return ppsspp_state_upload_jobs(
            self._psp_game_id_tokens(game),
            directories,
            file_field,
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )

    def _save_record_timestamp(self, record: dict[str, Any]) -> float:
        return save_record_timestamp(record)

    def _latest_file_mtime_under_path(
        self,
        root: Path,
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> float:
        if not root.exists():
            return 0.0
        blocked_basenames = {
            name.casefold()
            for name in (ignore_basenames or set())
            if isinstance(name, str) and name.strip()
        }
        blocked_extensions = {
            extension.casefold()
            for extension in (ignore_extensions or set())
            if isinstance(extension, str) and extension.strip()
        }
        if root.is_file():
            if blocked_basenames and root.name.casefold() in blocked_basenames:
                return 0.0
            if blocked_extensions and root.suffix.casefold() in blocked_extensions:
                return 0.0
            try:
                return float(root.stat().st_mtime)
            except OSError:
                return 0.0
        if not root.is_dir():
            return 0.0
        latest = 0.0
        for candidate in root.rglob("*"):
            if not candidate.is_file():
                continue
            if blocked_basenames and candidate.name.casefold() in blocked_basenames:
                continue
            if blocked_extensions and candidate.suffix.casefold() in blocked_extensions:
                continue
            try:
                latest = max(latest, candidate.stat().st_mtime)
            except OSError:
                continue
        return latest

    def _latest_local_state_mtime_for_game(
        self,
        game: dict[str, str],
        emulator_name: str,
        directories: list[Path],
    ) -> float:
        if not directories:
            return 0.0

        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if self._is_rpcs3_emulator_name(emulator_name, emulator_entry):
            return 0.0
        if emulator_entry is None:
            emulator_entry = {"name": emulator_name, "path": "", "args": "%rom%", "save_strategy": "auto"}

        candidates, _ = self._cloud_sync_targets_for_game(game, emulator_name, emulator_entry, directories, "state")

        latest = 0.0
        for candidate in candidates:
            try:
                latest = max(latest, candidate.stat().st_mtime)
            except OSError:
                continue
        return latest

    def _latest_local_save_mtime_for_game(
        self,
        game: dict[str, str],
        emulator_name: str,
        directories: list[Path],
    ) -> float:
        if not directories:
            return 0.0

        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None:
            emulator_entry = {"name": emulator_name, "path": "", "args": "%rom%", "save_strategy": "auto"}

        files, folder_targets = self._cloud_sync_targets_for_game(game, emulator_name, emulator_entry, directories, "save")
        ignore_basenames = self._sync_directory_ignore_basenames_for_emulator(emulator_name, emulator_entry, "save")
        ignore_extensions = self._sync_directory_ignore_extensions_for_emulator(emulator_entry)
        latest = 0.0
        for candidate in files:
            try:
                latest = max(latest, candidate.stat().st_mtime)
            except OSError:
                continue
        for save_directory in folder_targets:
            latest = max(
                latest,
                self._latest_file_mtime_under_path(
                    save_directory,
                    ignore_basenames=ignore_basenames,
                    ignore_extensions=ignore_extensions,
                ),
            )
        return latest

    def _server_save_records_for_rom(self, rom_id: str) -> list[dict[str, Any]]:
        payload = self._api_get("/api/saves", {"rom_id": rom_id})
        return server_records_from_payload(payload)

    def _latest_server_save_record(self, rom_id: str, emulator_name: str) -> dict[str, Any] | None:
        records = self._server_save_records_for_rom(rom_id)
        return latest_server_record(records, emulator_name, self._save_record_timestamp)

    def _latest_server_save_records_for_game(
        self,
        game: dict[str, str],
        rom_id: str,
        emulator_name: str,
        emulator_entry: dict[str, str] | None,
    ) -> list[dict[str, Any]]:
        records = self._server_save_records_for_rom(rom_id)
        save_scope = self._cloud_save_scope_for_game(
            game,
            emulator_name,
            emulator_entry,
            save_type="save",
        )
        if save_scope != "per-game":
            return latest_server_records_by_slot(records, emulator_name, self._save_record_timestamp)
        latest = latest_server_record(records, emulator_name, self._save_record_timestamp)
        return [latest] if latest is not None else []

    def _cloud_save_slot_for_upload_job(
        self,
        game: dict[str, str],
        emulator_name: str,
        emulator_entry: dict[str, str] | None,
        save_type: str,
        display_name: str,
        files_payload: dict[str, Path],
    ) -> str:
        if save_type != "save":
            return ""

        save_scope = self._cloud_save_scope_for_game(
            game,
            emulator_name,
            emulator_entry,
            save_type=save_type,
        )
        if save_scope == "shared-single":
            return "shared-media"
        if save_scope != "shared-slotted":
            return ""

        candidate_names = [display_name.strip().casefold()]
        for path in files_payload.values():
            if isinstance(path, Path):
                candidate_names.append(path.stem.strip().casefold())
                candidate_names.append(path.name.strip().casefold())

        for candidate in candidate_names:
            match = re.search(r"vmu([0-3])", candidate)
            if match is not None:
                return f"vmu{match.group(1)}"
        return ""

    def _server_state_records_for_rom(self, rom_id: str) -> list[dict[str, Any]]:
        payload = self._api_get("/api/states", {"rom_id": rom_id})
        records = server_records_from_payload(payload)
        blocked_extensions = tuple(
            extension.casefold()
            for extension in SUPPORTED_IMAGE_EXTENSIONS
            if isinstance(extension, str) and extension.strip()
        )
        if not blocked_extensions:
            return records

        filtered_records: list[dict[str, Any]] = []
        for record in records:
            if not isinstance(record, dict):
                continue
            file_name_value = record.get("file_name", "")
            file_name = file_name_value.strip().casefold() if isinstance(file_name_value, str) else ""
            if file_name and file_name.endswith(blocked_extensions):
                continue
            filtered_records.append(record)
        return filtered_records

    def _latest_server_state_record(self, rom_id: str, emulator_name: str) -> dict[str, Any] | None:
        records = self._server_state_records_for_rom(rom_id)
        return latest_server_record(records, emulator_name, self._save_record_timestamp)

    def _prune_server_save_records(self, rom_id: str, emulator_name: str, keep_latest: int) -> tuple[int, list[str]]:
        keep = max(1, keep_latest)
        records = self._server_save_records_for_rom(rom_id)
        emulator_key = emulator_name.strip().casefold()
        matching_records = [
            item
            for item in records
            if isinstance(item, dict)
            and (
                not emulator_key
                or (
                    isinstance(item.get("emulator"), str)
                    and item.get("emulator", "").strip().casefold() == emulator_key
                )
            )
        ]
        debug_enabled = self._debug_prints_enabled()

        def _id_rank(record: dict[str, Any]) -> int:
            value = record.get("id", 0)
            try:
                return int(value)
            except (TypeError, ValueError):
                return 0

        matching_records.sort(key=lambda item: (self._save_record_timestamp(item), _id_rank(item)), reverse=True)

        grouped_records: dict[str, list[dict[str, Any]]] = {}
        group_order: list[str] = []
        for item in matching_records:
            slot_value = item.get("slot", "")
            slot_key = slot_value.strip().casefold() if isinstance(slot_value, str) else ""
            if not slot_key:
                file_name_value = item.get("file_name", "")
                if isinstance(file_name_value, str) and file_name_value.strip():
                    slot_key = Path(file_name_value).stem.strip().casefold()
            if not slot_key:
                slot_key = "__default__"
            if slot_key not in grouped_records:
                grouped_records[slot_key] = []
                group_order.append(slot_key)
            grouped_records[slot_key].append(item)

        stale_records: list[dict[str, Any]] = []
        for group_key in group_order:
            stale_records.extend(grouped_records.get(group_key, [])[keep:])

        if debug_enabled:
            stale_ids_preview = [str(item.get("id", "")).strip() for item in stale_records[:10] if isinstance(item, dict)]
            print(
                f"[DEBUG][CloudSync] Retention prune plan rom_id={rom_id} emulator={emulator_name} "
                f"total_records={len(matching_records)} keep={keep} stale_count={len(stale_records)} "
                f"stale_ids={stale_ids_preview}"
            )

        deleted_count = 0
        failed_ids: list[str] = []
        for record in stale_records:
            save_id = str(record.get("id", "")).strip()
            if not save_id:
                continue
            try:
                numeric_save_id = int(save_id)
            except (TypeError, ValueError):
                failed_ids.append(save_id)
                if debug_enabled:
                    print(f"[DEBUG][CloudSync] Retention delete skipped_invalid_id id={save_id}")
                continue

            endpoint_path = "/api/saves/delete"
            payload = {"saves": [numeric_save_id]}
            try:
                if debug_enabled:
                    print(f"[DEBUG][CloudSync] Retention delete request path={endpoint_path} payload={payload}")
                self._api_post_json(endpoint_path, payload)
                deleted_count += 1
            except HTTPError as error:
                if error.code in {404, 410}:
                    deleted_count += 1
                    if debug_enabled:
                        print(
                            f"[DEBUG][CloudSync] Retention delete skipped_missing path={endpoint_path} "
                            f"status={error.code}"
                        )
                    continue
                failed_ids.append(save_id)
                if debug_enabled:
                    print(
                        f"[DEBUG][CloudSync] Retention delete failed path={endpoint_path} status={error.code}"
                    )
            except (HTTPError, URLError, OSError, ValueError, json.JSONDecodeError):
                failed_ids.append(save_id)
                if debug_enabled:
                    print(f"[DEBUG][CloudSync] Retention delete failed path={endpoint_path}")

        return deleted_count, failed_ids

    def _download_server_save_content(self, save_id: str) -> bytes:
        save_id_path = quote(save_id, safe="")
        return self._api_get_bytes(f"/api/saves/{save_id_path}/content")

    def _download_server_state_content(self, state_id: str) -> bytes:
        state_id_path = quote(state_id, safe="")
        state_record = self._api_get(f"/api/states/{state_id_path}")
        if not isinstance(state_record, dict):
            raise ValueError("State record payload is invalid.")

        for candidate in state_download_candidate_paths(state_record):
            try:
                if candidate.startswith(("http://", "https://")):
                    request = Request(normalize_candidate_url(candidate), headers=self._authorized_headers(), method="GET")
                    with urlopen(request, timeout=60) as response:
                        return response.read()

                relative_path = candidate if candidate.startswith("/") else f"/{candidate}"
                relative_path = normalize_candidate_url(relative_path)
                return self._api_get_bytes(relative_path)
            except (HTTPError, URLError, OSError, ValueError):
                continue

        raise ValueError("State content path could not be resolved from server record.")

    def _download_screenshot_from_state_record(self, state_record: dict) -> tuple[bytes, str] | None:
        screenshot_record = state_record.get("screenshot")
        if not isinstance(screenshot_record, dict):
            return None

        if screenshot_record.get("missing_from_fs") is True:
            return None

        candidates = screenshot_download_candidate_paths(screenshot_record)
        if not candidates:
            return None

        screenshot_extension = screenshot_record.get("file_extension", "").strip() or ".png"
        for candidate in candidates:
            try:
                if candidate.startswith(("http://", "https://")):
                    request = Request(normalize_candidate_url(candidate), headers=self._authorized_headers(), method="GET")
                    with urlopen(request, timeout=60) as response:
                        return response.read(), screenshot_extension

                relative_path = candidate if candidate.startswith("/") else f"/{candidate}"
                relative_path = normalize_candidate_url(relative_path)
                return self._api_get_bytes(relative_path), screenshot_extension
            except (HTTPError, URLError, OSError, ValueError):
                continue

        return None

    def _extract_zip_archive_bytes_to_directory(
        self,
        payload: bytes,
        target_root: Path,
        *,
        skip_basenames: set[str] | None = None,
        skip_extensions: set[str] | None = None,
    ) -> int:
        return extract_zip_archive_bytes_to_directory(
            payload,
            target_root,
            skip_basenames=skip_basenames,
            skip_extensions=skip_extensions,
        )

    def _restore_single_save_file(
        self,
        game: dict[str, str],
        directories: list[Path],
        save_record: dict[str, Any],
        payload: bytes,
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> Path | None:
        candidate_paths = self._cloud_sync_candidates_for_game(
            game,
            directories,
            "save",
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )
        fallback_name = f"{self._sanitize_path_component(game.get('title', 'game'), 'save')}.srm"
        return restore_single_save_payload(
            directories,
            save_record,
            payload,
            candidate_paths,
            fallback_name,
            skip_basenames=ignore_basenames,
            skip_extensions=ignore_extensions,
        )

    def _restore_single_state_file(
        self,
        game: dict[str, str],
        directories: list[Path],
        state_record: dict[str, Any],
        payload: bytes,
        screenshot_bytes: bytes | None = None,
        screenshot_extension: str = ".png",
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> Path | None:
        candidate_paths = self._cloud_sync_candidates_for_game(
            game,
            directories,
            "state",
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )
        fallback_name = f"{self._sanitize_path_component(game.get('title', 'game'), 'state')}.state"
        return restore_single_state_payload(
            directories,
            state_record,
            payload,
            candidate_paths,
            fallback_name,
            screenshot_bytes=screenshot_bytes,
            screenshot_extension=screenshot_extension,
            skip_basenames=ignore_basenames,
            skip_extensions=ignore_extensions,
        )

    def _restore_cloud_save_for_game(
        self,
        game: dict[str, str],
        save_record: dict[str, Any] | None = None,
        *,
        show_dialogs: bool = True,
        skip_if_local_newer: bool = False,
        skip_if_known_latest: bool = False,
    ) -> bool:
        debug_enabled = self._debug_prints_enabled()

        def show_warning(message: str) -> None:
            if show_dialogs:
                QMessageBox.warning(self, "Cloud Sync", message)

        def show_info(message: str) -> None:
            if show_dialogs:
                QMessageBox.information(self, "Cloud Sync", message)

        if self._is_native_executable_platform(game):
            return self._restore_native_cloud_save_for_game(game, save_record=save_record, show_dialogs=show_dialogs)

        emulator_name, emulator_entry = self._resolved_cloud_emulator_entry_for_game(game, save_type="save")
        compatibility_reason = self._cloud_save_block_reason_for_game(
            game,
            emulator_name,
            emulator_entry,
            save_type="save",
        )
        if compatibility_reason:
            show_info(compatibility_reason)
            return False

        rom_id = self._cloud_sync_rom_id_for_game(
            game,
            save_type="save",
            emulator_name=emulator_name,
            emulator=emulator_entry,
        )
        if not rom_id:
            show_warning("Missing ROM id for this game.")
            return False

        emulator_name, emulator_entry = self._resolved_cloud_emulator_entry_for_game(game, save_type="save")
        requested_emulator_name = ""
        if save_record is not None:
            emulator_value = save_record.get("emulator", "")
            if isinstance(emulator_value, str):
                requested_emulator_name = emulator_value.strip()

        if requested_emulator_name:
            requested_entry = self._emulator_entry_by_name(requested_emulator_name)
            if requested_entry is None and requested_emulator_name.casefold() != emulator_name.strip().casefold():
                show_warning(f"Emulator '{requested_emulator_name}' is not configured on this device.")
                return False
            if requested_entry is not None:
                emulator_name = requested_emulator_name
                emulator_entry = requested_entry

        if emulator_entry is None:
            show_warning("No default emulator is configured for this game's platform.")
            return False

        directories = self._resolved_sync_directory_paths(emulator_entry, "save_paths")
        if not directories:
            show_warning(f"No save directories were found for emulator '{emulator_name}'. Configure them in Emulators.")
            return False

        save_records_to_restore: list[dict[str, Any]]
        if save_record is None:
            try:
                save_records_to_restore = self._latest_server_save_records_for_game(
                    game,
                    rom_id,
                    emulator_name,
                    emulator_entry,
                )
            except (HTTPError, URLError, ValueError, json.JSONDecodeError) as error:
                show_warning(f"Failed to query server saves: {error}")
                return False
        else:
            save_records_to_restore = [dict(save_record)]

        if not save_records_to_restore:
            show_info("No cloud save was found on the server for this game.")
            return False

        primary_save_record = save_records_to_restore[0]
        save_id = str(primary_save_record.get("id", "")).strip()
        if not save_id:
            show_warning("Server save record is missing an id.")
            return False

        save_scope = self._cloud_save_scope_for_game(
            game,
            emulator_name,
            emulator_entry,
            save_type="save",
        )

        if skip_if_known_latest and save_scope == "per-game":
            sync_state = self._cloud_sync_state_for_game(game)
            last_downloaded_save_id = str(sync_state.get("last_downloaded_save_id", "")).strip()
            if last_downloaded_save_id and last_downloaded_save_id == save_id:
                local_latest_mtime = self._latest_local_save_mtime_for_game(game, emulator_name, directories)
                if should_skip_known_latest(last_downloaded_save_id, save_id, local_latest_mtime):
                    if debug_enabled:
                        print(
                            f"[DEBUG][CloudSync] Restore skipped already_latest title={game.get('title', '')} "
                            f"rom_id={rom_id} emulator={emulator_name} save_id={save_id}"
                        )
                    return False
                if debug_enabled:
                    print(
                        f"[DEBUG][CloudSync] Restore continuing local_missing title={game.get('title', '')} "
                        f"rom_id={rom_id} emulator={emulator_name} save_id={save_id}"
                    )

        if skip_if_local_newer:
            if self._is_pcsx2_emulator_name(emulator_name, emulator_entry) and not self._ps2_game_id_tokens(game):
                if debug_enabled:
                    print(
                        f"[DEBUG][CloudSync] Restore local_newer_check_skipped title={game.get('title', '')} "
                        f"rom_id={rom_id} emulator={emulator_name} reason=missing_ps2_id_tokens"
                    )
            else:
                local_latest_mtime = self._latest_local_save_mtime_for_game(game, emulator_name, directories)
                server_latest_timestamp = max(
                    (self._save_record_timestamp(item) for item in save_records_to_restore),
                    default=0.0,
                )
                if is_local_newer_than_server(local_latest_mtime, server_latest_timestamp):
                    if debug_enabled:
                        print(
                            f"[DEBUG][CloudSync] Restore skipped local_newer title={game.get('title', '')} "
                            f"rom_id={rom_id} emulator={emulator_name} local_mtime={local_latest_mtime:.0f} "
                            f"server_ts={server_latest_timestamp:.0f} save_id={save_id}"
                        )
                    return False

        is_folder_save = (
            self._is_ppsspp_emulator_name(emulator_name, emulator_entry)
            or self._is_rpcs3_emulator_name(emulator_name, emulator_entry)
            or self._is_pcsx2_emulator_name(emulator_name, emulator_entry)
            or self._is_cemu_emulator_name(emulator_name, emulator_entry)
        )
        skip_basenames = self._sync_directory_ignore_basenames_for_emulator(emulator_name, emulator_entry, "save")
        skip_extensions = self._sync_directory_ignore_extensions_for_emulator(emulator_entry)
        restored_target = ""
        latest_restored_id = save_id
        latest_server_timestamp = max(
            (self._save_record_timestamp(item) for item in save_records_to_restore),
            default=0.0,
        )
        try:
            for active_save_record in save_records_to_restore:
                active_save_id = str(active_save_record.get("id", "")).strip()
                if not active_save_id:
                    raise ValueError("Server save record is missing an id.")

                payload = self._download_server_save_content(active_save_id)
                if not payload:
                    raise ValueError("Downloaded cloud save content was empty.")

                if is_folder_save:
                    restored_count = self._extract_zip_archive_bytes_to_directory(
                        payload,
                        directories[0],
                        skip_basenames=skip_basenames,
                        skip_extensions=skip_extensions,
                    )
                    if restored_count <= 0:
                        raise ValueError("Save archive downloaded, but no files were restored.")
                    restored_target = str(directories[0])
                else:
                    restored_file = self._restore_single_save_file(
                        game,
                        directories,
                        active_save_record,
                        payload,
                        ignore_basenames=skip_basenames,
                        ignore_extensions=skip_extensions,
                    )
                    if restored_file is None:
                        raise ValueError("Save content downloaded, but no file was restored.")
                    restored_target = str(restored_file)

                active_timestamp = self._save_record_timestamp(active_save_record)
                if active_timestamp >= latest_server_timestamp:
                    latest_server_timestamp = active_timestamp
                    latest_restored_id = active_save_id
        except (HTTPError, URLError, OSError, ValueError, zipfile.BadZipFile) as error:
            show_warning(f"Failed to restore cloud save: {error}")
            return False

        self._update_cloud_sync_state_for_game(
            game,
            {
                "last_downloaded_save_id": latest_restored_id,
                "last_server_timestamp": latest_server_timestamp,
            },
        )

        if debug_enabled:
            print(
                f"[DEBUG][CloudSync] Restore success save_type=save title={game.get('title', '')} "
                f"rom_id={rom_id} emulator={emulator_name} restored={len(save_records_to_restore)} "
                f"save_id={latest_restored_id} target={restored_target}"
            )

        show_info("Cloud save restored successfully.")
        return True

    def _restore_native_cloud_save_for_game(
        self,
        game: dict,
        *,
        save_record=None,
        show_dialogs: bool = True,
    ) -> bool:
        import io
        import json as _json
        import os
        import tempfile
        import zipfile as _zipfile
        from pathlib import Path
        from urllib.error import HTTPError, URLError

        def show_warning(msg: str) -> None:
            if show_dialogs:
                QMessageBox.warning(self, "Cloud Sync", msg)

        # Build fallback directory list in case manifest is missing (legacy records)
        key = self._pcgw_cache_key(game)
        manual_key = key + "__manual"
        cached = self._pcgw_paths_for_game(game) or []
        manual = self._pcgw_paths_cache.get(manual_key, [])
        all_raw = list(cached) + [p for p in manual if p not in cached]
        _win_docs = pcsx2_windows_documents_folder()
        fallback_dirs = [resolve_native_save_dir(r, _win_docs) for r in all_raw]

        rom_id = self._cloud_sync_rom_id_for_game(game)
        if not rom_id:
            show_warning("Missing ROM id for this game.")
            return False

        try:
            if save_record is None:
                try:
                    records = self._latest_server_save_records_for_game(game, rom_id, "", {})
                except (HTTPError, URLError, ValueError) as error:
                    show_warning(f"Failed to query server saves: {error}")
                    return False
            else:
                records = [dict(save_record)]

            if not records:
                if show_dialogs:
                    QMessageBox.information(self, "Cloud Sync", "No cloud save found on the server for this game.")
                return False

            for record in records:
                save_id = str(record.get("id", "")).strip()
                if not save_id:
                    continue
                payload = self._download_server_save_content(save_id)
                if not payload:
                    raise ValueError("Downloaded cloud save content was empty.")

                emulator_field = str(record.get("emulator", "") or "")

                if emulator_field == "native_multi_dir":
                    # New format: combined archive with manifest
                    fd, tmp_path = tempfile.mkstemp(prefix="rom-mate-restore-", suffix=".zip")
                    os.close(fd)
                    tmp_zip = Path(tmp_path)
                    try:
                        tmp_zip.write_bytes(payload)
                        if not _zipfile.is_zipfile(tmp_zip):
                            raise ValueError("Downloaded save is not a valid zip archive.")

                        with _zipfile.ZipFile(tmp_zip, "r") as archive:
                            # Read manifest
                            try:
                                manifest_bytes = archive.read("_rom_mate_dirs.json")
                                manifest: dict[str, str] = _json.loads(manifest_bytes)
                            except (KeyError, _json.JSONDecodeError):
                                manifest = {}

                            for member in archive.infolist():
                                member_name = member.filename
                                if member_name == "_rom_mate_dirs.json" or member_name.endswith("/"):
                                    continue

                                # Parse index prefix: "0/path/to/file.sav"
                                parts = member_name.split("/", 1)
                                if len(parts) != 2:
                                    continue
                                dir_idx, relative_str = parts[0], parts[1]
                                if not relative_str:
                                    continue

                                # Resolve target directory
                                if dir_idx in manifest:
                                    raw_path = manifest[dir_idx]
                                    target_root = resolve_native_save_dir(raw_path, _win_docs)
                                elif fallback_dirs:
                                    target_root = fallback_dirs[0]
                                else:
                                    continue

                                # Security: prevent path traversal
                                resolved_root = target_root.resolve()
                                destination = (resolved_root / relative_str).resolve()
                                try:
                                    destination.relative_to(resolved_root)
                                except ValueError:
                                    continue

                                destination.parent.mkdir(parents=True, exist_ok=True)
                                with archive.open(member, "r") as src, destination.open("wb") as dst:
                                    import shutil
                                    shutil.copyfileobj(src, dst)
                    finally:
                        tmp_zip.unlink(missing_ok=True)

                elif emulator_field.startswith("native_dir:"):
                    # Previous per-directory format (legacy)
                    raw_path = emulator_field[len("native_dir:"):]
                    restore_dir = resolve_native_save_dir(raw_path, _win_docs)
                    restore_dir.mkdir(parents=True, exist_ok=True)
                    self._extract_zip_archive_bytes_to_directory(payload, restore_dir)

                else:
                    # Unknown/old format — restore to first fallback dir
                    if not fallback_dirs:
                        raise ValueError("No restore directories configured.")
                    restore_dir = fallback_dirs[0]
                    restore_dir.mkdir(parents=True, exist_ok=True)
                    self._extract_zip_archive_bytes_to_directory(payload, restore_dir)

        except Exception as error:
            show_warning(f"Failed to restore cloud save: {error}")
            return False

        if show_dialogs:
            QMessageBox.information(self, "Cloud Sync", "Cloud save restored successfully.")
        return True

    def _restore_cloud_state_for_game(
        self,
        game: dict[str, str],
        state_record: dict[str, Any] | None = None,
        *,
        show_dialogs: bool = True,
        skip_if_known_latest: bool = False,
    ) -> bool:
        debug_enabled = self._debug_prints_enabled()

        def show_warning(message: str) -> None:
            if show_dialogs:
                QMessageBox.warning(self, "Cloud Sync", message)

        def show_info(message: str) -> None:
            if show_dialogs:
                QMessageBox.information(self, "Cloud Sync", message)

        emulator_name, emulator_entry = self._resolved_cloud_emulator_entry_for_game(game, save_type="state")
        compatibility_reason = self._cloud_save_block_reason_for_game(
            game,
            emulator_name,
            emulator_entry,
            save_type="state",
        )
        if compatibility_reason:
            show_info(compatibility_reason)
            return False

        rom_id = self._resolve_rom_id_for_game(game)
        if not rom_id:
            show_warning("Missing ROM id for this game.")
            return False

        emulator_name, emulator_entry = self._resolved_cloud_emulator_entry_for_game(game, save_type="state")
        requested_emulator_name = ""
        if state_record is not None:
            emulator_value = state_record.get("emulator", "")
            if isinstance(emulator_value, str):
                requested_emulator_name = emulator_value.strip()

        if requested_emulator_name:
            requested_entry = self._emulator_entry_by_name(requested_emulator_name)
            if requested_entry is None and requested_emulator_name.casefold() != emulator_name.strip().casefold():
                show_warning(f"Emulator '{requested_emulator_name}' is not configured on this device.")
                return False
            if requested_entry is not None:
                emulator_name = requested_emulator_name
                emulator_entry = requested_entry

        if emulator_entry is None:
            show_warning("No default emulator is configured for this game's platform.")
            return False

        if self._is_rpcs3_emulator_name(emulator_name, emulator_entry):
            show_info("RPCS3 savestate restore is not supported yet.")
            return False

        directories = self._resolved_sync_directory_paths(emulator_entry, "state_paths")
        if not directories:
            show_warning(f"No state directories were found for emulator '{emulator_name}'. Configure them in Emulators.")
            return False

        if state_record is None:
            try:
                state_record = self._latest_server_state_record(rom_id, emulator_name)
            except (HTTPError, URLError, ValueError, json.JSONDecodeError) as error:
                show_warning(f"Failed to query server states: {error}")
                return False

        if state_record is None:
            show_info("No cloud save state was found on the server for this game.")
            return False

        state_id = str(state_record.get("id", "")).strip()
        if not state_id:
            show_warning("Server state record is missing an id.")
            return False

        if skip_if_known_latest:
            sync_state = self._cloud_sync_state_for_game(game)
            last_downloaded_state_id = str(sync_state.get("last_downloaded_state_id", "")).strip()
            if last_downloaded_state_id and last_downloaded_state_id == state_id:
                local_latest_mtime = self._latest_local_state_mtime_for_game(game, emulator_name, directories)
                if should_skip_known_latest(last_downloaded_state_id, state_id, local_latest_mtime):
                    if debug_enabled:
                        print(
                            f"[DEBUG][CloudSync] Restore skipped already_latest save_type=state title={game.get('title', '')} "
                            f"rom_id={rom_id} emulator={emulator_name} state_id={state_id}"
                        )
                    return False
                if debug_enabled:
                    print(
                        f"[DEBUG][CloudSync] Restore continuing local_missing save_type=state title={game.get('title', '')} "
                        f"rom_id={rom_id} emulator={emulator_name} state_id={state_id}"
                    )

        try:
            payload = self._download_server_state_content(state_id)
        except (HTTPError, URLError, OSError, ValueError) as error:
            show_warning(f"Failed to download cloud state content: {error}")
            return False

        if not payload:
            show_warning("Downloaded cloud state content was empty.")
            return False

        screenshot_result = None
        try:
            screenshot_result = self._download_screenshot_from_state_record(state_record)
        except (HTTPError, URLError, OSError, ValueError) as error:
            if debug_enabled:
                print(
                    f"[DEBUG][CloudSync] Restore screenshot download failed save_type=state title={game.get('title', '')} "
                    f"rom_id={rom_id} emulator={emulator_name} state_id={state_id} error={error}"
                )
        screenshot_bytes, screenshot_ext = screenshot_result if screenshot_result else (None, ".png")

        try:
            ignore_basenames = self._resolved_ignore_basenames_for_emulator(emulator_entry)
            ignore_extensions = self._resolved_ignore_extensions_for_emulator(emulator_entry)
            restored_file = self._restore_single_state_file(
                game,
                directories,
                state_record,
                payload,
                screenshot_bytes,
                screenshot_ext,
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )
            if restored_file is None:
                show_warning("State content downloaded, but no file was restored.")
                return False
        except OSError as error:
            show_warning(f"Failed to restore cloud state: {error}")
            return False

        if debug_enabled:
            print(
                f"[DEBUG][CloudSync] Restore success save_type=state title={game.get('title', '')} "
                f"rom_id={rom_id} emulator={emulator_name} state_id={state_id} target={restored_file}"
            )

        self._update_cloud_sync_state_for_game(
            game,
            {
                "last_downloaded_state_id": state_id,
            },
        )

        show_info("Cloud state restored successfully.")
        return True

    def _cloud_sync_candidates_for_game(
        self,
        game: dict[str, str],
        directories: list[Path],
        save_type: str,
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> list[Path]:
        return cloud_sync_candidates_for_game(
            game,
            directories,
            save_type,
            self._game_save_match_tokens,
            self._is_state_file_candidate,
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )

    def _upload_cloud_files_for_game(
        self,
        game: dict[str, str],
        save_type: str,
        *,
        show_dialogs: bool = True,
    ) -> tuple[int, int, list[str]]:
        debug_enabled = self._debug_prints_enabled()

        def show_warning(message: str) -> None:
            if show_dialogs:
                QMessageBox.warning(self, "Cloud Sync", message)

        def show_info(message: str) -> None:
            if show_dialogs:
                QMessageBox.information(self, "Cloud Sync", message)

        emulator_name, emulator_entry = self._resolved_cloud_emulator_entry_for_game(game, save_type=save_type)
        if self._is_native_executable_platform(game):
            return self._upload_native_saves_for_game(game, show_dialogs=show_dialogs)
        compatibility_reason = self._cloud_save_block_reason_for_game(
            game,
            emulator_name,
            emulator_entry,
            save_type=save_type,
        )
        if compatibility_reason:
            show_info(compatibility_reason)
            return 0, 0, []

        rom_id = self._cloud_sync_rom_id_for_game(
            game,
            save_type=save_type,
            emulator_name=emulator_name,
            emulator=emulator_entry,
        )
        if not rom_id:
            show_warning("Missing ROM id for this game.")
            return 0, 0, []

        if emulator_entry is None:
            show_warning("No default emulator is configured for this game's platform.")
            return 0, 0, []

        directory_key = "save_paths" if save_type == "save" else "state_paths"
        directories = self._resolved_sync_directory_paths(emulator_entry, directory_key)
        if not directories:
            kind_label = "save" if save_type == "save" else "state"
            show_warning(f"No {kind_label} directories were found for emulator '{emulator_name}'. Configure them in Emulators.")
            return 0, 0, []

        endpoint = "/api/saves" if save_type == "save" else "/api/states"
        file_field = "saveFile" if save_type == "save" else "stateFile"
        is_ppsspp = self._is_ppsspp_emulator_name(emulator_name, emulator_entry)
        is_rpcs3 = self._is_rpcs3_emulator_name(emulator_name, emulator_entry)

        if is_rpcs3 and save_type == "state":
            show_info("RPCS3 savestate uploads are not supported yet.")
            return 0, 0, []

        upload_jobs: list[tuple[str, dict[str, Path]]] = []
        temporary_archives: list[Path] = []
        success_count = 0
        failed_files: list[str] = []

        if save_type == "save":
            save_files, save_directories = self._cloud_sync_targets_for_game(
                game,
                emulator_name,
                emulator_entry,
                directories,
                "save",
            )
            ignore_basenames = self._sync_directory_ignore_basenames_for_emulator(emulator_name, emulator_entry, "save")
            ignore_extensions = self._sync_directory_ignore_extensions_for_emulator(emulator_entry)
            archived_jobs, directory_archives = directory_archive_upload_jobs(
                save_directories,
                file_field,
                lambda save_directory: self._zip_directory_for_upload(
                    save_directory,
                    game,
                    ignore_basenames=ignore_basenames,
                    ignore_extensions=ignore_extensions,
                ),
            )
            temporary_archives.extend(directory_archives)
            save_scope = self._cloud_save_scope_for_game(
                game,
                emulator_name,
                emulator_entry,
                save_type="save",
            )
            if save_scope == "shared-single" and save_files:
                shared_archive = zip_selected_files_for_upload(
                    save_files,
                    self._sanitize_path_component(emulator_name or game.get("title", "game"), "save"),
                    ignore_basenames=ignore_basenames,
                    ignore_extensions=ignore_extensions,
                )
                temporary_archives.append(shared_archive)
                grouped_jobs = [(f"{emulator_name or 'Shared Save'} Storage", {file_field: shared_archive})]
                file_archives: list[Path] = []
            else:
                grouped_jobs, file_archives = grouped_file_upload_jobs(
                    save_files,
                    file_field,
                    lambda files: zip_selected_files_for_upload(
                        files,
                        self._sanitize_path_component(game.get("title", "game"), "save"),
                        ignore_basenames=ignore_basenames,
                        ignore_extensions=ignore_extensions,
                    ),
                )
                temporary_archives.extend(file_archives)
            upload_jobs.extend(archived_jobs)
            upload_jobs.extend(grouped_jobs)
            if not upload_jobs:
                show_info(no_matching_upload_message(save_type))
                return 0, 0, []
        elif is_ppsspp and save_type == "state":
            ignore_basenames = self._resolved_ignore_basenames_for_emulator(emulator_entry)
            ignore_extensions = self._resolved_ignore_extensions_for_emulator(emulator_entry)
            upload_jobs = self._ppsspp_state_upload_jobs(
                game,
                directories,
                file_field,
                ignore_basenames=ignore_basenames,
                ignore_extensions=ignore_extensions,
            )
            upload_jobs = filter_upload_jobs_by_session_window(
                upload_jobs,
                self._session_window_for_state_upload(game),
            )
            if not upload_jobs:
                show_info(no_matching_upload_message(save_type, is_ppsspp_state=True))
                return 0, 0, []
        else:
            files, _ = self._cloud_sync_targets_for_game(
                game,
                emulator_name,
                emulator_entry,
                directories,
                save_type,
            )
            if not files:
                show_info(no_matching_upload_message(save_type))
                return 0, 0, []
            ignore_basenames = self._sync_directory_ignore_basenames_for_emulator(emulator_name, emulator_entry, save_type)
            ignore_extensions = self._sync_directory_ignore_extensions_for_emulator(emulator_entry)
            if save_type == "state" and self._is_retroarch_emulator_name(emulator_name, emulator_entry):
                upload_jobs, ra_archives = retroarch_state_upload_jobs(
                    files,
                    file_field,
                    ignore_basenames=ignore_basenames,
                    ignore_extensions=ignore_extensions,
                )
                temporary_archives.extend(ra_archives)
            else:
                upload_jobs, grouped_archives = grouped_file_upload_jobs(
                    files,
                    file_field,
                    lambda selected_files: zip_selected_files_for_upload(
                        selected_files,
                        self._sanitize_path_component(game.get("title", "game"), save_type),
                        ignore_basenames=ignore_basenames,
                        ignore_extensions=ignore_extensions,
                    ),
                )
                temporary_archives.extend(grouped_archives)

        screenshot_dirs = self._resolved_screenshot_directories(emulator_entry)
        if screenshot_dirs:
            session_win = self._session_window_for_state_upload(game)
            sidecar_screenshot = session_screenshot_path(screenshot_dirs, session_win)
            if sidecar_screenshot is not None:
                for _, files_payload in upload_jobs:
                    if "screenshotFile" not in files_payload:
                        files_payload["screenshotFile"] = sidecar_screenshot

        for display_name, files_payload in upload_jobs:
            params: dict[str, Any] = {
                "rom_id": rom_id,
                "emulator": emulator_name,
            }
            if save_type == "save":
                params["overwrite"] = "true"
                slot_value = self._cloud_save_slot_for_upload_job(
                    game,
                    emulator_name,
                    emulator_entry,
                    save_type,
                    display_name,
                    files_payload,
                )
                if slot_value:
                    params["slot"] = slot_value
            try:
                self._api_post_multipart(endpoint, files_payload, params=params)
                success_count += 1
            except (HTTPError, URLError, OSError, ValueError, json.JSONDecodeError) as error:
                failed_files.append(display_name)

        cleanup_temporary_paths(temporary_archives)

        retention_limit = self._cloud_save_retention_limit()
        retention_deleted = 0
        retention_failed_ids: list[str] = []
        if save_type == "save" and success_count > 0:
            try:
                retention_deleted, retention_failed_ids = self._prune_server_save_records(
                    rom_id,
                    emulator_name,
                    retention_limit,
                )
            except (HTTPError, URLError, OSError, ValueError, json.JSONDecodeError) as error:
                retention_failed_ids = [str(error)]

        if debug_enabled:
            status = "success" if not failed_files else ("partial" if success_count > 0 else "failure")
            total_upload_jobs = len(upload_jobs)
            print(
                f"[DEBUG][CloudSync] Upload {status} save_type={save_type} title={game.get('title', '')} "
                f"rom_id={rom_id} emulator={emulator_name} uploaded={success_count}/{total_upload_jobs} "
                f"failed={failed_files[:5]} retention_deleted={retention_deleted} "
                f"retention_limit={retention_limit} retention_failed={retention_failed_ids[:5]}"
            )

        completion_message, is_warning = upload_completion_message(
            save_type,
            success_count,
            failed_files,
            retention_failed_ids,
            retention_limit,
        )
        if is_warning:
            show_warning(completion_message)
        else:
            show_info(completion_message)
        total_upload_jobs = len(upload_jobs)
        return success_count, total_upload_jobs, failed_files

    def _upload_native_saves_for_game(
        self,
        game: dict,
        *,
        show_dialogs: bool = True,
    ) -> tuple[int, int, list[str]]:
        import os as _os
        from pathlib import Path

        def show_warning(msg: str) -> None:
            if show_dialogs:
                QMessageBox.warning(self, "Cloud Sync", msg)

        def show_info(msg: str) -> None:
            if show_dialogs:
                QMessageBox.information(self, "Cloud Sync", msg)

        # Build combined path list (PCGW + manual)
        key = self._pcgw_cache_key(game)
        manual_key = key + "__manual"
        cached = self._pcgw_paths_for_game(game) or []
        manual = self._pcgw_paths_cache.get(manual_key, [])
        all_raw = list(cached) + [p for p in manual if p not in cached]

        if not all_raw:
            show_warning(
                "No save locations are configured for this game. "
                "Use 'Manage Saves' → 'Browse' to add one."
            )
            return 0, 0, []

        rom_id = self._cloud_sync_rom_id_for_game(game)
        if not rom_id:
            show_warning("Missing ROM id for this game.")
            return 0, 0, []

        # Build dir_map: only include directories that currently exist.
        # Use the Shell-resolved Documents path to handle network drive redirection.
        _win_docs = pcsx2_windows_documents_folder()
        dir_map: list[tuple[str, Path]] = []
        for raw in all_raw:
            expanded = resolve_native_save_dir(raw, _win_docs)
            if expanded.exists():
                dir_map.append((raw, expanded))

        if not dir_map:
            path_list = "\n".join(f"  • {resolve_native_save_dir(r, _win_docs)}" for r in all_raw)
            show_warning(
                f"None of the configured save locations exist on this device yet.\n\n"
                f"Checked:\n{path_list}"
            )
            return 0, 0, []

        # Build a single combined zip archive
        safe_title = self._sanitize_path_component(game.get("title", "game"), "save")
        try:
            archive_path, total_files, _manifest = zip_native_save_dirs_for_upload(dir_map, safe_title)
        except OSError as exc:
            show_warning(f"Failed to create save archive: {exc}")
            return 0, 0, []

        if total_files == 0:
            archive_path.unlink(missing_ok=True)
            show_info(no_matching_upload_message("save"))
            return 0, 0, []

        endpoint = "/api/saves"
        file_field = "saveFile"
        files_payload: dict[str, Path] = {file_field: archive_path}
        params: dict[str, Any] = {
            "rom_id": rom_id,
            "emulator": "native_multi_dir",
            "overwrite": "true",
        }
        try:
            self._api_post_multipart(endpoint, files_payload, params=params)
            success_count = 1
            failed_files: list[str] = []
        except (HTTPError, URLError, OSError, ValueError, json.JSONDecodeError) as exc:
            success_count = 0
            failed_files = [safe_title]
        finally:
            archive_path.unlink(missing_ok=True)

        # Retention pruning — single upload job counts as 1
        retention_limit = self._cloud_save_retention_limit()
        retention_failed_ids: list[str] = []
        if success_count > 0:
            try:
                _, retention_failed_ids = self._prune_server_save_records(
                    rom_id,
                    "native_multi_dir",
                    retention_limit,
                )
            except (HTTPError, URLError, OSError, ValueError, json.JSONDecodeError) as error:
                retention_failed_ids = [str(error)]

        completion_message, is_warning = upload_completion_message(
            "save",
            success_count,
            failed_files,
            retention_failed_ids,
            retention_limit,
        )
        if is_warning:
            show_warning(completion_message)
        else:
            show_info(completion_message)

        return success_count, 1, failed_files

    def _perform_upload_saves_action(self) -> None:
        if self.current_details_game is None:
            return
        installed_game = self._installed_game_record(self.current_details_game)
        if installed_game is None:
            return
        self._upload_cloud_files_for_game(installed_game, "save")
        if self.current_details_cloud_mode != "overview":
            self._refresh_details_cloud_panel()

    def _perform_restore_saves_action(self) -> None:
        if self.current_details_game is None:
            return
        installed_game = self._installed_game_record(self.current_details_game)
        if installed_game is None:
            return
        restored = self._restore_cloud_save_for_game(installed_game)
        if restored and self.current_details_cloud_mode != "overview":
            self._refresh_details_cloud_panel()

    def _perform_upload_states_action(self) -> None:
        if self.current_details_game is None:
            return
        installed_game = self._installed_game_record(self.current_details_game)
        if installed_game is None:
            return
        self._upload_cloud_files_for_game(installed_game, "state")
        if self.current_details_cloud_mode != "overview":
            self._refresh_details_cloud_panel()

    def _perform_restore_states_action(self) -> None:
        if self.current_details_game is None:
            return
        installed_game = self._installed_game_record(self.current_details_game)
        if installed_game is None:
            return
        restored = self._restore_cloud_state_for_game(installed_game)
        if restored and self.current_details_cloud_mode != "overview":
            self._refresh_details_cloud_panel()

    def _auto_sync_before_launch(self, game: dict[str, str]) -> None:
        if not self._auto_cloud_save_download_enabled():
            return
        if not self._credentials_present() or not self._server_connected():
            return
        if not self._cloud_save_block_reason_for_game(game, save_type="save"):
            self._restore_cloud_save_for_game(
                game,
                show_dialogs=False,
                skip_if_local_newer=self._auto_cloud_skip_download_if_local_newer(),
                skip_if_known_latest=True,
            )
        if not self._cloud_save_block_reason_for_game(game, save_type="state"):
            self._restore_cloud_state_for_game(
                game,
                show_dialogs=False,
                skip_if_known_latest=True,
            )

    def _register_game_session_for_auto_upload(
        self,
        game: dict[str, str],
        process: subprocess.Popen,
        emulator_name: str,
    ) -> None:
        if self._cloud_save_block_reason_for_game(game, emulator_name, save_type="save") and self._cloud_save_block_reason_for_game(
            game,
            emulator_name,
            save_type="state",
        ):
            return
        started_at = time.time()
        session = {
            "game": dict(game),
            "process": process,
            "emulator_name": emulator_name.strip(),
            "started_at": started_at,
        }
        self.active_game_sessions.append(session)
        self._update_cloud_sync_state_for_game(
            game,
            {
                "last_session_started_at": started_at,
                "last_session_ended_at": 0.0,
            },
        )

    def _poll_active_game_sessions(self) -> None:
        if not self.active_game_sessions:
            return

        remaining, finished = partition_active_game_sessions(self.active_game_sessions)
        for session in finished:
            self._handle_finished_game_session(session)

        self.active_game_sessions = remaining

    def _handle_finished_game_session(self, session: dict[str, Any]) -> None:
        ended_at = time.time()
        game, updates = session_cloud_sync_updates(session, ended_at)
        if game is not None:
            if updates:
                self._update_cloud_sync_state_for_game(game, updates)
            session["ended_at"] = ended_at

        game_title = game.get("title", "") if isinstance(game, dict) else ""
        emulator_name_value = session.get("emulator_name", "")
        emulator_name = emulator_name_value.strip() if isinstance(emulator_name_value, str) else ""

        if not self._auto_cloud_save_upload_enabled():
            return
        if not self._credentials_present() or not self._server_connected():
            return

        delay_ms = self._auto_cloud_upload_delay_seconds() * 1000
        session_copy = dict(session)
        if delay_ms <= 0:
            self._auto_upload_after_session(session_copy)
            return
        QTimer.singleShot(delay_ms, lambda item=session_copy: self._auto_upload_after_session(item))

    def _auto_upload_after_session(self, session: dict[str, Any]) -> None:
        game = session.get("game")
        if not isinstance(game, dict):
            return
        emulator_name_value = session.get("emulator_name", "")
        emulator_name = emulator_name_value.strip() if isinstance(emulator_name_value, str) else ""
        if not emulator_name:
            emulator_name, _ = self._resolved_emulator_entry_for_game(game)
        game_title = game.get("title", "")
        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None:
            return

        sync_state = self._cloud_sync_state_for_game(game)

        local_latest_save_mtime = 0.0
        include_save_upload = not self._cloud_save_block_reason_for_game(
            game,
            emulator_name,
            emulator_entry,
            save_type="save",
        )
        save_directories = self._resolved_sync_directory_paths(emulator_entry, "save_paths") if include_save_upload else []
        if save_directories:
            local_latest_save_mtime = self._latest_local_save_mtime_for_game(game, emulator_name, save_directories)

        local_latest_state_mtime = 0.0
        state_directories = self._resolved_sync_directory_paths(emulator_entry, "state_paths")
        include_state_upload = (
            bool(state_directories)
            and not self._is_rpcs3_emulator_name(emulator_name, emulator_entry)
            and not self._cloud_save_block_reason_for_game(
                game,
                emulator_name,
                emulator_entry,
                save_type="state",
            )
        )
        if include_state_upload:
            local_latest_state_mtime = self._latest_local_state_mtime_for_game(game, emulator_name, state_directories)

        upload_types, latest_mtimes = auto_cloud_upload_plan(
            sync_state,
            local_latest_save_mtime,
            local_latest_state_mtime,
            include_state_upload,
        )
        if not upload_types:
            return

        self._start_auto_cloud_upload_worker(game, upload_types, latest_mtimes)

    def _start_auto_cloud_upload_worker(
        self,
        game: dict[str, str],
        upload_types: list[str],
        local_latest_mtimes: dict[str, float],
    ) -> None:
        thread = QThread(self)
        worker = AutoCloudSaveUploadWorker(self, game, upload_types, local_latest_mtimes)
        worker.moveToThread(thread)

        thread.started.connect(worker.run)
        worker.finished.connect(self._on_auto_cloud_upload_finished)
        worker.finished.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(lambda t=thread, w=worker: self._cleanup_auto_cloud_upload_worker(t, w))

        self.auto_cloud_upload_threads.append(thread)
        self.auto_cloud_upload_workers.append(worker)
        thread.start()

    def _cleanup_auto_cloud_upload_worker(self, thread: QThread, worker: AutoCloudSaveUploadWorker) -> None:
        self.auto_cloud_upload_threads = [item for item in self.auto_cloud_upload_threads if item is not thread]
        self.auto_cloud_upload_workers = [item for item in self.auto_cloud_upload_workers if item is not worker]

    def _on_auto_cloud_upload_finished(self, bundle: object) -> None:
        game = bundle.get("game") if isinstance(bundle, dict) else None
        result = bundle.get("result") if isinstance(bundle, dict) else None
        if not isinstance(game, dict) or not isinstance(result, dict):
            return

        updates, debug_segments, any_uploaded, any_failed = summarize_auto_cloud_upload_result(
            result,
            datetime.now(UTC).isoformat(timespec="seconds").replace("+00:00", "Z"),
        )

        if updates:
            self._update_cloud_sync_state_for_game(game, updates)

        if self._debug_prints_enabled() and debug_segments:
            if any_uploaded and not any_failed:
                status = "success"
            elif any_uploaded:
                status = "partial"
            else:
                status = "failure"
            print(
                f"[DEBUG][CloudSync] Auto upload {status} title={game.get('title', '')} "
                f"{' '.join(debug_segments)}"
            )

