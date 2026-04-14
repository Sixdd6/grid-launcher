from __future__ import annotations

import json
import os
import shutil
import stat
import tempfile
import time
from pathlib import Path
from typing import Any, Callable
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode

from PySide6.QtCore import QThread, QTimer
from PySide6.QtWidgets import QLabel, QMessageBox, QWidget

from rom_mate.background import InstallDownloadWorker, InstallFinalizeWorker
from rom_mate.core import format_http_error_details, path_key, path_within_path, sanitize_path_component
from rom_mate.emulator import (
    ensure_dolphin_gcpad_config,
    ensure_dolphin_skip_ipl,
    is_ps3_emulator_entry,
    is_rpcs3_emulator_name,
    ps3_vfs_dev_hdd0_path,
    retroarch_core_config_files_metadata,
    retroarch_core_firmware_metadata,
    retroarch_core_saves_files_metadata,
    retroarch_directory_settings,
    update_rpcs3_games_yml,
    xenia_directory_settings,
)
from rom_mate.library import (
    active_download_count_after_finish,
    apply_download_entry_install_progress,
    apply_download_entry_progress,
    apply_download_entry_status,
    apply_ps4_content_archive_without_ui,
    apply_xenia_content_archive_without_ui,
    archive_name_for_game,
    can_start_next_queued_install,
    candidate_archive_paths_for_game,
    candidate_extracted_dirs_for_game,
    candidate_extracted_paths_for_game,
    cleanup_install_archive,
    directory_total_file_bytes,
    download_count_text,
    download_entry_detail_text,
    download_entry_status_from_error,
    download_progress_display,
    download_speed_text,
    extract_archive_for_game,
    extract_archive_into_directory,
    extracted_dir_for_archive_path,
    filter_queue_by_download_entry_id,
    find_download_entry,
    format_size,
    game_has_server_update,
    has_newer_server_rom_version,
    installed_game_record,
    is_game_install_queued,
    is_game_installed,
    make_download_entry_data,
    matching_installed_emulator_games,
    merge_archive_into_directory,
    normalized_download_progress,
    normalized_transfer_progress,
    pending_install_key,
    percent_text,
    prepare_installed_game_without_ui,
    queued_install_keys,
    remove_download_entry,
    remove_game_files,
    resolved_native_executable_path_for_game,
    retry_download_game,
    rom_file_name_version,
    select_extracted_launch_file,
    server_content_file_name_for_game,
    should_extract_archive_for_game,
    should_reset_active_download_metrics,
    sync_install_metadata_to_details_game,
    tar_archive_total_install_bytes,
    tar_listing_line_size,
    uninstall_library_games,
    library_games_without_keys,
    build_installed_game_record,
)
from rom_mate.library.firmware_install import install_platform_firmware
from rom_mate.server import (
    fetch_server_rom_payload,
    resolved_rom_file_name_for_game,
    rom_file_name_from_payload,
)
from rom_mate.ui import make_download_entry_widget, refresh_downloads_page


class InstallMixin:
    """Mixin containing download/install lifecycle methods for MainWindow."""


    def _install_game(self, game: dict[str, str]) -> bool:
        if self._is_game_installed(game):
            return False
        archive_path = self._download_game_archive(game)
        if archive_path is None:
            return False
        installed_game = self._prepare_installed_game(game, archive_path)
        if installed_game is None:
            return False
        self._auto_configure_installed_emulator(installed_game, archive_path)
        self._register_installed_game(installed_game, archive_path)
        self._write_rpcs3_games_yml_for_game(installed_game)
        return True


    def _register_installed_game(self, game: dict[str, str], archive_path: Path) -> None:
        game_key_value = self._game_key(game)
        self.library_games = [entry for entry in self.library_games if self._game_key(entry) != game_key_value]
        self.library_games.append(
            build_installed_game_record(
                game,
                archive_path,
                resolved_cover_url=self._resolved_cover_url_for_game(game),
                cached_cover_path=self._cache_cover_image_for_game(game),
            )
        )
        self.library_games = self._normalize_installed_games(self.library_games)
        self._refresh_installed_game_update_state()
        self._refresh_library_grid()
        self._persist_installed_games()


    def _refresh_installed_game_update_state(self) -> None:
        update_keys: set[tuple[str, str]] = set()
        for installed_game in self.library_games:
            has_update = False
            if not self._is_emulators_platform(installed_game):
                server_game = self._server_game_for_identity(installed_game, installed_game.get("rom_id", ""))
                if isinstance(server_game, dict):
                    has_update = game_has_server_update(
                        installed_game,
                        server_game,
                        is_emulators_platform=self._is_emulators_platform,
                    )
            installed_game["update_available"] = "true" if has_update else "false"
            if has_update:
                update_keys.add(self._game_key(installed_game))

        self.installed_game_update_keys = update_keys
        if self.current_details_game is not None:
            self.current_details_game["update_available"] = (
                "true" if self._details_update_available_for_game(self.current_details_game) else "false"
            )
            # Only refresh the update button, not the full (expensive) action button set.
            details_update_button = getattr(self, "details_update_button", None)
            if details_update_button is not None:
                has_update = self.current_details_game.get("update_available") == "true"
                installed = self._is_game_installed(self.current_details_game)
                is_emulator_entry = self._is_emulators_platform(self.current_details_game)
                show_update = installed and not is_emulator_entry and has_update
                update_button_text_fn = getattr(self, "_details_update_button_text_for_game", None)
                update_button_text = update_button_text_fn(self.current_details_game) if callable(update_button_text_fn) else "Update"
                details_update_button.setText(update_button_text)
                details_update_button.setVisible(show_update)
                details_update_button.setEnabled(show_update and not self.install_in_progress)


    def _details_update_available_for_game(self, game: dict[str, str]) -> bool:
        if self._is_emulators_platform(game):
            return False

        installed_game = self._installed_game_record(game)
        if installed_game is None:
            return False

        game_key_value = self._game_key(installed_game)
        if game_key_value in self.installed_game_update_keys:
            return True

        server_game = self._server_game_for_identity(installed_game, installed_game.get("rom_id", ""))
        if not isinstance(server_game, dict):
            return False

        has_update = game_has_server_update(
            installed_game,
            server_game,
            is_emulators_platform=self._is_emulators_platform,
        )
        installed_game["update_available"] = "true" if has_update else "false"
        if has_update:
            self.installed_game_update_keys.add(game_key_value)
        else:
            self.installed_game_update_keys.discard(game_key_value)
        return has_update


    def _ps4_file_ids_by_category_for_game(
        self,
        game: dict[str, str],
        rom_id: str = "",
        *,
        allow_payload_lookup: bool,
    ) -> dict[str, list[int]]:
        for candidate in (
            game,
            self._installed_game_record(game),
            self._server_game_for_identity(game, rom_id),
        ):
            if not isinstance(candidate, dict):
                continue
            parsed = self._ps4_file_ids_by_category_from_text(candidate.get("ps4_file_ids_by_category", ""))
            if parsed:
                return parsed

        if allow_payload_lookup:
            normalized_rom_id = rom_id.strip()
            if normalized_rom_id:
                payload = self._fetch_server_rom_payload(normalized_rom_id, force_refresh=True)
                if isinstance(payload, dict):
                    parsed = self._ps4_file_ids_by_category_from_payload(payload)
                    if parsed:
                        return parsed
        return {}


    def _xbox360_file_ids_by_category_for_game(
        self,
        game: dict[str, str],
        rom_id: str = "",
        *,
        allow_payload_lookup: bool,
    ) -> dict[str, list[int]]:
        for candidate in (
            game,
            self._installed_game_record(game),
            self._server_game_for_identity(game, rom_id),
        ):
            if not isinstance(candidate, dict):
                continue
            parsed = self._xbox360_file_ids_by_category_from_text(candidate.get("xbox360_file_ids_by_category", ""))
            if parsed:
                return parsed

        if allow_payload_lookup:
            normalized_rom_id = rom_id.strip()
            if normalized_rom_id:
                payload = self._fetch_server_rom_payload(normalized_rom_id, force_refresh=True)
                if isinstance(payload, dict):
                    parsed = self._ps4_file_ids_by_category_from_payload(payload)
                    if parsed:
                        return parsed
        return {}


    def _available_ps4_content_kinds_for_game(self, game: dict[str, str]) -> list[str]:
        if not self._is_ps4_platform(game):
            return []
        file_ids_by_category = self._ps4_file_ids_by_category_for_game(game, allow_payload_lookup=False)
        kinds = [kind for kind in ("update", "dlc") if file_ids_by_category.get(kind)]
        return kinds


    def _details_ps4_content_button_text(self, game: dict[str, str]) -> str:
        kinds = self._available_ps4_content_kinds_for_game(game)
        if kinds == ["update"]:
            return "Install Update"
        if kinds == ["dlc"]:
            return "Install DLC"
        if len(kinds) == 2:
            return "Install Update/DLC"
        return ""


    def _available_xbox360_content_kinds_for_game(self, game: dict[str, str]) -> list[str]:
        if not self._is_xbox360_platform(game):
            return []
        file_ids_by_category = self._xbox360_file_ids_by_category_for_game(game, allow_payload_lookup=False)
        return [kind for kind in ("update", "dlc") if file_ids_by_category.get(kind)]


    def _details_xbox360_content_button_text(self, game: dict[str, str]) -> str:
        kinds = self._available_xbox360_content_kinds_for_game(game)
        if kinds == ["update"]:
            return "Install Update"
        if kinds == ["dlc"]:
            return "Install DLC"
        if len(kinds) == 2:
            return "Install Update/DLC"
        return ""


    def _ps4_content_install_block_reason(self, game: dict[str, str]) -> str:
        if not self._is_ps4_platform(game):
            return ""
        if not self._is_game_installed(game):
            return "Install the base PS4 game before applying update or DLC content."
        if not self._resolve_rom_id_for_game(game):
            return "This game is missing a ROM id, so update/DLC content cannot be downloaded."
        if not self._available_ps4_content_kinds_for_game(game):
            return "No update or DLC content is available for this PS4 game on the server."
        return ""


    def _xbox360_content_install_block_reason(self, game: dict[str, str]) -> str:
        if not self._is_game_installed(game):
            return "Game must be installed before content can be applied."
        rom_id = self._resolve_rom_id_for_game(game)
        if not rom_id:
            return "Game is missing a ROM ID."
        return ""


    def _ps4_content_archive_name(self, game: dict[str, str], content_kind: str) -> str:
        safe_title = self._sanitize_path_component(game.get("title", "ps4-content"), "ps4-content")
        kind = content_kind.strip().lower() or "content"
        return f"{safe_title}-{kind}.zip"


    def _xbox360_content_archive_name(self, game: dict[str, str], content_kind: str) -> str:
        safe_title = self._sanitize_path_component(game.get("title", "xbox360-content"), "xbox360-content")
        kind = content_kind.strip().lower() or "content"
        return f"{safe_title}-{kind}.zip"


    def _apply_ps4_content_archive_without_ui(
        self,
        installed_game: dict[str, str],
        archive_path: Path,
        *,
        content_kind: str,
        install_progress_callback: Callable[[int, int], None] | None = None,
    ) -> tuple[dict[str, str] | None, str]:
        return apply_ps4_content_archive_without_ui(
            installed_game,
            archive_path,
            content_kind=content_kind,
            extracted_dir_for_archive_path=self._extracted_dir_for_archive_path,
            extract_archive_into_directory=extract_archive_into_directory,
            install_progress_callback=install_progress_callback,
        )


    def _apply_xenia_content_archive_without_ui(
        self,
        installed_game: dict[str, str],
        archive_path: Path,
        *,
        install_progress_callback: Callable[[int, int], None] | None = None,
    ) -> tuple[dict[str, str] | None, str]:
        platform = installed_game.get("platform", "Xbox 360")
        emulator_name = self._default_emulator_name_for_platform(platform)
        emulator = self._emulator_entry_by_name(emulator_name) if emulator_name else None
        emulator_path = emulator.get("path", "") if isinstance(emulator, dict) else ""
        emulator_args = emulator.get("args", "") if isinstance(emulator, dict) else ""

        settings = xenia_directory_settings(
            emulator_path,
            emulator_args,
            self._split_launch_template_args,
        )
        content_root_text = settings.get("content_root", "")
        if not content_root_text:
            return None, "Could not determine Xenia content directory. Is Xenia configured?"

        results, warning_text = apply_xenia_content_archive_without_ui(
            archive_path,
            Path(content_root_text),
            extracted_dir_for_archive_path=self._extracted_dir_for_archive_path,
            extract_archive_into_directory=extract_archive_into_directory,
            install_progress_callback=install_progress_callback,
        )

        if not results and warning_text:
            return None, warning_text

        game = dict(installed_game)
        return game, warning_text


    def _sync_ps4_content_metadata_to_installed_game(self, source_game: dict[str, str], updated_game: dict[str, str]) -> None:
        installed_game = self._installed_game_record(source_game)
        if installed_game is None:
            return
        installed_game["ps4_game_id"] = updated_game.get("ps4_game_id", "")
        installed_game["ps4_content"] = updated_game.get("ps4_content", "")
        self._persist_installed_games()


    def _is_rpcs3_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "rpcs3", emulator=emulator) or is_rpcs3_emulator_name(emulator_name)


    def _is_ps3_emulator_entry(self, emulator: dict[str, str]) -> bool:
        return is_ps3_emulator_entry(emulator, self._emulator_profile_for_entry)


    def _should_extract_archive_for_game(self, game: dict[str, str], archive_path: Path) -> bool:
        return should_extract_archive_for_game(
            game,
            archive_path,
            is_native_executable_platform=self._is_native_executable_platform,
            is_arcade_platform=self._is_arcade_platform,
            is_ps3_platform=self._is_ps3_platform,
        )


    def _extracted_dir_for_archive_path(self, archive_path: Path) -> Path:
        return extracted_dir_for_archive_path(archive_path)


    def _select_extracted_launch_file(self, game: dict[str, str], extracted_dir: Path, archive_path: Path) -> Path | None:
        return select_extracted_launch_file(
            game,
            extracted_dir,
            archive_path,
            is_ps3_platform=self._is_ps3_platform,
        )


    def _extract_archive_for_game(
        self,
        game: dict[str, str],
        archive_path: Path,
        install_progress_callback: Callable[[int, int], None] | None = None,
    ) -> tuple[Path, Path]:
        return extract_archive_for_game(
            game,
            archive_path,
            extracted_dir_for_archive_path=self._extracted_dir_for_archive_path,
            select_extracted_launch_file=self._select_extracted_launch_file,
            install_progress_callback=install_progress_callback,
        )


    def _directory_total_file_bytes(self, directory: Path) -> int:
        return directory_total_file_bytes(directory)


    def _tar_archive_total_install_bytes(self, archive_path: Path) -> int:
        return tar_archive_total_install_bytes(archive_path)


    def _tar_listing_line_size(self, line: str) -> int:
        return tar_listing_line_size(line)


    def _show_install_warning_if_actionable(self, warning_text: str) -> None:
        text = warning_text.strip() if isinstance(warning_text, str) else ""
        if not text:
            return
        if "could not delete archive" in text.casefold():
            return
        QMessageBox.warning(self, "Install Warning", text)


    def _prepare_installed_game(self, game: dict[str, str], archive_path: Path) -> dict[str, str] | None:
        prepared, warning_text = self._prepare_installed_game_without_ui(game, archive_path)
        if prepared is None:
            title = game.get("title", "Game")
            error_text = warning_text or f"Failed to extract archive for {title}"
            QMessageBox.warning(self, "Install Error", f"Failed to install {title}: {error_text}")
            return None
        self._show_install_warning_if_actionable(warning_text)
        return prepared


    def _prepare_installed_game_without_ui(
        self,
        game: dict[str, str],
        archive_path: Path,
        *,
        cleanup_archive_on_success: bool = True,
        install_progress_callback: Callable[[int, int], None] | None = None,
    ) -> tuple[dict[str, str] | None, str]:
        return prepare_installed_game_without_ui(
            game,
            archive_path,
            should_extract_archive_for_game=self._should_extract_archive_for_game,
            extract_archive_for_game=self._extract_archive_for_game,
            is_ps3_platform=self._is_ps3_platform,
            ps3_dev_hdd0_root=self._ps3_dev_hdd0_for_game,
            ps3_games_root=self._ps3_games_dir_for_game,
            cleanup_archive_on_success=cleanup_archive_on_success,
            install_progress_callback=install_progress_callback,
        )


    def _write_rpcs3_games_yml_for_game(self, installed_game: dict[str, str]) -> None:
        if not self._is_ps3_platform(installed_game):
            return
        game_id = installed_game.get("ps3_game_id", "").strip()
        if not game_id:
            return
        data_root = self._rpcs3_data_root_for_game(installed_game)
        if data_root is None:
            return
        dev_hdd0 = self._ps3_dev_hdd0_for_game(installed_game)
        if dev_hdd0 is None:
            return
        games_dir = self._ps3_games_dir_for_game(installed_game)
        update_rpcs3_games_yml(data_root, game_id, dev_hdd0, games_dir)


    def _install_firmware_for_game_without_ui(self, game: dict, prepared_game: dict) -> str:
        del prepared_game
        platform = game.get("platform", "")
        game_name = game.get("title", "")
        if not platform:
            return ""

        platform_id = getattr(self, "server_platform_ids", {}).get(platform)
        if platform_id is None or not isinstance(platform_id, int):
            return ""

        emulator_name = self._default_emulator_name_for_platform(platform)
        if not emulator_name:
            return ""

        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None:
            return ""

        firmware_dirs = self._resolved_firmware_directories(emulator_entry)
        config_file_dirs: list = []
        saves_file_dirs: list = []
        extract_zip_with_paths = False

        if self._is_retroarch_emulator_name(emulator_name, emulator=emulator_entry):
            core_defaults = self._normalize_default_retroarch_cores(
                self.config.get("default_retroarch_cores", {})
            )
            configured_core = self._mapping_value_for_platform(core_defaults, platform)
            if not configured_core:
                return ""
            core_entries = self._retroarch_core_list_entries()
            metadata = retroarch_core_firmware_metadata(configured_core, core_entries)
            if metadata is None:
                firmware_dirs = []
            else:
                subdirectory = metadata.get("subdirectory")
                if isinstance(subdirectory, str) and subdirectory.strip():
                    firmware_dirs = [
                        ((d[0] / subdirectory, d[1]) if isinstance(d, tuple) else d / subdirectory)
                        for d in firmware_dirs
                    ]
                file_names = metadata.get("files", [])
                if isinstance(file_names, list) and file_names:
                    firmware_dirs = [
                        (d if isinstance(d, tuple) else (d, list(file_names)))
                        for d in firmware_dirs
                    ]
                extract_zip_with_paths = bool(metadata.get("extract_with_paths", False))

            config_metadata = retroarch_core_config_files_metadata(
                configured_core, core_entries
            )
            if isinstance(config_metadata, dict):
                base_dir = config_metadata.get("base_dir")
                if isinstance(base_dir, str) and base_dir.strip():
                    emulator_path_value = emulator_entry.get("path", "")
                    emulator_path = (
                        Path(emulator_path_value).expanduser()
                        if isinstance(emulator_path_value, str)
                        else Path()
                    )
                    emulator_dir = emulator_path.parent if emulator_path_value else Path()
                    file_names = config_metadata.get("files", [])
                    if isinstance(file_names, list) and file_names:
                        config_file_dirs = [(emulator_dir / base_dir, list(file_names))]
                    else:
                        config_file_dirs = [emulator_dir / base_dir]

            saves_metadata = retroarch_core_saves_files_metadata(
                configured_core, core_entries
            )
            if isinstance(saves_metadata, dict):
                saves_file_name = saves_metadata.get("file")
                if isinstance(saves_file_name, str) and saves_file_name.strip():
                    directory_settings = retroarch_directory_settings(
                        emulator_entry.get("path", "")
                    )
                    savefile_dir_str = directory_settings.get("savefile_directory", "")
                    emulator_path_value = emulator_entry.get("path", "")
                    emulator_path = (
                        Path(emulator_path_value).expanduser()
                        if isinstance(emulator_path_value, str) and emulator_path_value
                        else None
                    )
                    emulator_dir = emulator_path.parent if emulator_path else None
                    if emulator_dir:
                        if isinstance(savefile_dir_str, str) and savefile_dir_str.strip():
                            stripped = savefile_dir_str.strip()
                            if stripped.lower() == "default":
                                saves_dir = emulator_dir / "saves"
                            elif stripped.startswith(":\\") or stripped.startswith(":/"):
                                # RetroArch root-relative notation - strip the :\ or :/ prefix.
                                relative_part = stripped[2:]
                                saves_dir = (emulator_dir / relative_part).resolve()
                            else:
                                saves_dir = Path(stripped).expanduser()
                                if not saves_dir.is_absolute():
                                    saves_dir = (emulator_dir / saves_dir).resolve()
                        else:
                            saves_dir = emulator_dir / "saves"
                        saves_file_dirs = [(saves_dir, [saves_file_name])]
                        if self._debug_prints_enabled():
                            print(f"[DEBUG][Firmware] saves_dir resolved to: {saves_dir}")
        elif self._is_cemu_emulator_name(emulator_name, emulator=emulator_entry):
            firmware_dirs = [
                d if isinstance(d, tuple) else (d, ["keys.txt"])
                for d in firmware_dirs
            ]

        if not firmware_dirs and not config_file_dirs and not saves_file_dirs:
            return ""

        warnings: list[str] = []

        if firmware_dirs:
            try:
                warnings.extend(
                    install_platform_firmware(
                        self._api_get,
                        self._api_get_bytes,
                        platform_id,
                        firmware_dirs,
                        skip_existing=True,
                        extract_zip_with_paths=extract_zip_with_paths,
                    )
                )
            except Exception as e:
                warnings.append(f"Firmware install error: {e}")

        if config_file_dirs:
            try:
                warnings.extend(
                    install_platform_firmware(
                        self._api_get,
                        self._api_get_bytes,
                        platform_id,
                        config_file_dirs,
                        skip_existing=True,
                    )
                )
            except Exception as e:
                warnings.append(f"Firmware install error: {e}")

        if saves_file_dirs:
            try:
                warnings.extend(
                    install_platform_firmware(
                        self._api_get,
                        self._api_get_bytes,
                        platform_id,
                        saves_file_dirs,
                        skip_existing=True,
                        extract_zip_with_paths=True,
                    )
                )
            except Exception as e:
                warnings.append(f"Firmware install error: {e}")

        if self._is_dolphin_emulator_name(emulator_name, emulator=emulator_entry):
            try:
                ensure_dolphin_skip_ipl(emulator_entry.get("path", ""))
            except Exception:
                pass
            try:
                ensure_dolphin_gcpad_config(emulator_entry.get("path", ""))
            except Exception:
                pass

        return "\n".join(warnings)


    def _cleanup_install_archives_without_ui(
        self,
        game: dict[str, str],
        archive_path: Path,
        *,
        include_main: bool = True,
        include_supplementals: bool = True,
        install_progress_callback: Callable[[int, int], None] | None = None,
    ) -> str:
        del install_progress_callback
        warnings: list[str] = []
        title = str(game.get("title", "Game")).strip() or "Game"

        if include_main:
            cleanup_error = cleanup_install_archive(archive_path)
            if cleanup_error:
                warnings.append(f"Extracted {title}, but could not delete archive:\n{cleanup_error}")

        if include_supplementals:
            source_metadata = game.get("_source_metadata") if isinstance(game, dict) else None
            supplemental_value = source_metadata.get("supplemental_downloads", []) if isinstance(source_metadata, dict) else []
            if isinstance(supplemental_value, list):
                for index, raw_spec in enumerate(supplemental_value, start=1):
                    if not isinstance(raw_spec, dict):
                        continue
                    asset_name_value = raw_spec.get("asset_name", "")
                    asset_name = asset_name_value.strip() if isinstance(asset_name_value, str) else ""
                    suffix = Path(asset_name).suffix or archive_path.suffix or ".zip"
                    supplemental_path = archive_path.with_name(f"{archive_path.stem}-supplemental-{index}{suffix}")
                    cleanup_error = cleanup_install_archive(supplemental_path)
                    if cleanup_error:
                        warnings.append(
                            "Applied supplemental emulator files, but could not delete archive:\n"
                            f"{cleanup_error}"
                        )

        return "\n\n".join(part for part in warnings if part.strip())


    def _apply_source_supplemental_archives_without_ui(
        self,
        game: dict[str, str],
        archive_path: Path,
        installed_game: dict[str, str],
        *,
        install_progress_callback: Callable[[int, int], None] | None = None,
    ) -> None:
        source_metadata = game.get("_source_metadata") if isinstance(game, dict) else None
        if not isinstance(source_metadata, dict):
            return

        supplemental_value = source_metadata.get("supplemental_downloads", [])
        if not isinstance(supplemental_value, list) or not supplemental_value:
            return

        extracted_dir_text = installed_game.get("extracted_dir", "") if isinstance(installed_game, dict) else ""
        extracted_path_text = installed_game.get("extracted_path", "") if isinstance(installed_game, dict) else ""
        if isinstance(extracted_dir_text, str) and extracted_dir_text.strip():
            target_dir = Path(extracted_dir_text.strip())
        elif isinstance(extracted_path_text, str) and extracted_path_text.strip():
            target_dir = Path(extracted_path_text.strip()).expanduser().parent
        else:
            return

        if not target_dir.exists() or not target_dir.is_dir():
            return

        for index, raw_spec in enumerate(supplemental_value, start=1):
            if not isinstance(raw_spec, dict):
                continue
            asset_name_value = raw_spec.get("asset_name", "")
            asset_name = asset_name_value.strip() if isinstance(asset_name_value, str) else ""
            suffix = Path(asset_name).suffix or archive_path.suffix or ".zip"
            supplemental_path = archive_path.with_name(f"{archive_path.stem}-supplemental-{index}{suffix}")
            if not supplemental_path.exists() or not supplemental_path.is_file():
                continue
            temp_dir = target_dir.parent / f".{target_dir.name}-supplemental-{index}-merge"
            merge_archive_into_directory(
                supplemental_path,
                target_dir,
                temp_dir,
                install_progress_callback=install_progress_callback,
            )


    def _apply_native_game_update_without_ui(
        self,
        update_game: dict[str, str],
        archive_path: Path,
        *,
        install_progress_callback: Callable[[int, int], None] | None = None,
    ) -> tuple[dict[str, str] | None, str]:
        from rom_mate.library.archive_preparation import prepare_native_game_update_without_ui

        installed_game = self._installed_game_record(update_game) or update_game

        return prepare_native_game_update_without_ui(
            installed_game,
            update_game,
            archive_path,
            temp_dir_for_game=self._native_update_temp_dir_for_game,
            select_extracted_launch_file=self._select_extracted_launch_file,
            install_progress_callback=install_progress_callback,
        )


    def _native_update_temp_dir_for_game(self, game: dict[str, str]) -> Path:
        """Return a temp dir on the same filesystem as the game's install directory.

        Using the same parent folder as extracted_dir ensures the extraction and
        merge happen on the same filesystem, avoiding cross-drive file copies.
        """
        extracted_dir_text = game.get("extracted_dir", "").strip()
        if extracted_dir_text:
            extracted_dir = Path(extracted_dir_text)
            safe_name = self._sanitize_path_component(game.get("title", "game"), "game")
            return extracted_dir.parent / f"{safe_name}-temp"
        # Fallback: system temp (should not normally be reached)
        import tempfile
        safe_name = self._sanitize_path_component(game.get("title", "game"), "game")
        return Path(tempfile.gettempdir()) / f"rom-mate-{safe_name}-temp"


    def _archive_name_for_game(self, game: dict[str, str]) -> str:
        return archive_name_for_game(game, self._sanitize_path_component)


    def _server_content_file_name_for_game(self, game: dict[str, str]) -> str:
        return server_content_file_name_for_game(game)


    def _rom_file_name_from_payload(self, payload: dict[str, Any]) -> str:
        return rom_file_name_from_payload(payload)


    def _fetch_server_rom_payload(self, rom_id: str, force_refresh: bool = False) -> dict[str, Any] | None:
        return fetch_server_rom_payload(
            rom_id,
            self.server_rom_payloads,
            api_get=self._api_get,
            force_refresh=force_refresh,
        )


    def _resolved_rom_file_name_for_game(self, game: dict[str, str], rom_id: str) -> str:
        return resolved_rom_file_name_for_game(
            game,
            rom_id,
            self.server_rom_payloads,
            fetch_server_rom_payload=self._fetch_server_rom_payload,
            rom_file_name_from_payload=self._rom_file_name_from_payload,
        )


    def _sanitize_path_component(self, value: str, fallback: str) -> str:
        return sanitize_path_component(value, fallback)


    def _platform_library_dir(self, game: dict[str, str]) -> Path | None:
        library_path = self._library_path_dir()
        if library_path is None:
            return None
        platform_value = game.get("platform", "Platform")
        platform = platform_value if isinstance(platform_value, str) else "Platform"
        safe_platform = self._sanitize_path_component(platform, "platform")
        return library_path / safe_platform


    def _library_path_dir(self) -> Path | None:
        library_path_value = self.config.get("library_path", "")
        if not isinstance(library_path_value, str) or not library_path_value.strip():
            return None
        return Path(library_path_value).expanduser()


    def _download_game_archive(self, game: dict[str, str]) -> Path | None:
        rom_id = game.get("rom_id", "").strip()
        if not rom_id:
            QMessageBox.warning(self, "Install Error", "This game cannot be installed because it has no ROM id.")
            return None

        resolved_file_name = self._resolved_rom_file_name_for_game(game, rom_id)
        if not resolved_file_name:
            QMessageBox.warning(
                self,
                "Install Error",
                "Server did not return a usable ROM filename/path for this title. Refresh server metadata and try again.",
            )
            return None
        game["rom_file_name"] = resolved_file_name

        install_path = self._platform_library_dir(game)
        if install_path is None:
            QMessageBox.warning(self, "Install Error", "Set a Library Path in Settings before installing games.")
            return None

        try:
            install_path.mkdir(parents=True, exist_ok=True)
        except OSError as error:
            QMessageBox.warning(self, "Install Error", f"Could not prepare library folder: {error}")
            return None

        title = game.get("title", "Game")
        archive_name = self._archive_name_for_game(game)
        archive_path = install_path / archive_name
        rom_id_path = quote(rom_id, safe="")
        file_name_path = quote(self._server_content_file_name_for_game(game), safe="")

        download_path = f"/api/roms/{rom_id_path}/content/{file_name_path}"
        try:
            payload = self._api_get_bytes(download_path)
            archive_path.write_bytes(payload)
            return archive_path
        except HTTPError as error:
            detail = format_http_error_details(error)
            if self._debug_prints_enabled():
                print(f"[DEBUG][InstallDownload] url={self._server_base_url()}{download_path}")
                print(f"[DEBUG][InstallDownload] error={detail}")
            QMessageBox.warning(self, "Install Error", f"Failed to download {title} from server.")
            return None
        except (URLError, OSError, ValueError) as error:
            if self._debug_prints_enabled():
                print(f"[DEBUG][InstallDownload] url={self._server_base_url()}{download_path}")
                print(f"[DEBUG][InstallDownload] error={error}")
            QMessageBox.warning(self, "Install Error", f"Failed to download {title} from server.")
            return None


    def _candidate_archive_paths_for_game(self, game: dict[str, str]) -> list[Path]:
        return candidate_archive_paths_for_game(
            game,
            self._platform_library_dir,
            self._archive_name_for_game,
            self._library_path_dir,
        )


    def _candidate_extracted_paths_for_game(self, game: dict[str, str]) -> list[Path]:
        return candidate_extracted_paths_for_game(
            game,
            self._select_extracted_launch_file,
        )


    def _candidate_extracted_dirs_for_game(self, game: dict[str, str]) -> list[Path]:
        return candidate_extracted_dirs_for_game(
            game,
            self._candidate_archive_paths_for_game(game),
            self._extracted_dir_for_archive_path,
        )


    def _remove_game_files(self, game: dict[str, str]) -> bool:
        try:
            remove_game_files(
                game,
                is_ps3_platform=self._is_ps3_platform,
                is_native_executable_platform=self._is_native_executable_platform,
                candidate_extracted_dirs_for_game=self._candidate_extracted_dirs_for_game,
                remove_directory_tree=self._remove_directory_tree,
                candidate_archive_paths_for_game=self._candidate_archive_paths_for_game,
            )
        except OSError as error:
            QMessageBox.warning(self, "Uninstall Error", str(error))
            return False
        return True


    def _remove_directory_tree(self, directory: Path) -> None:
        def onerror(func: Any, path: str, exc_info: tuple[Any, Any, Any]) -> None:
            del exc_info
            try:
                os.chmod(path, stat.S_IWRITE)
                func(path)
            except OSError as error:
                raise error

        shutil.rmtree(directory, onerror=onerror)


    def _path_key(self, path: Path) -> str:
        return path_key(path)


    def _path_within_path(self, path: Path, root: Path) -> bool:
        return path_within_path(path, root)


    def _matching_installed_emulator_games(self, emulator_path: Path) -> list[dict[str, str]]:
        return matching_installed_emulator_games(
            self.library_games,
            emulator_path,
            is_emulators_platform=self._is_emulators_platform,
            candidate_archive_paths_for_game=self._candidate_archive_paths_for_game,
            candidate_extracted_paths_for_game=self._candidate_extracted_paths_for_game,
            candidate_extracted_dirs_for_game=self._candidate_extracted_dirs_for_game,
            path_key=self._path_key,
            path_within_path=self._path_within_path,
        )


    def _uninstall_emulator_files(self, emulator: dict[str, str]) -> bool:
        emulator_path_value = emulator.get("path", "")
        emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
        if not emulator_path_text:
            return True

        emulator_path = Path(emulator_path_text)
        matches = self._matching_installed_emulator_games(emulator_path)
        if not matches:
            return True

        self.library_games, removed = uninstall_library_games(
            self.library_games,
            {self._game_key(game) for game in matches},
            game_key=self._game_key,
            library_games_without_keys=library_games_without_keys,
            cached_cover_path_keys_for_games=self._cached_cover_path_keys_for_games,
            remove_game_files=self._remove_game_files,
            cleanup_cached_cover_for_game=self._cleanup_cached_cover_for_game,
        )
        if removed:
            self._refresh_library_grid()
            self._persist_installed_games()
        return removed


    def _uninstall_game(self, game: dict[str, str]) -> bool:
        self.library_games, removed = uninstall_library_games(
            self.library_games,
            {self._game_key(game)},
            game_key=self._game_key,
            library_games_without_keys=library_games_without_keys,
            cached_cover_path_keys_for_games=self._cached_cover_path_keys_for_games,
            remove_game_files=self._remove_game_files,
            cleanup_cached_cover_for_game=self._cleanup_cached_cover_for_game,
        )
        if removed:
            self._refresh_installed_game_update_state()
            self._refresh_library_grid()
            self._persist_installed_games()
        return removed


    def _start_async_install(self, game: dict[str, str]) -> bool:
        install_mode_value = game.get("_install_mode", "base")
        install_mode = install_mode_value.strip().lower() if isinstance(install_mode_value, str) else "base"
        if install_mode in {"source_emulator", "source_emulator_update"}:
            return self._start_async_source_emulator_install(game)
        is_ps4_content_install = install_mode == "ps4_content"
        is_xbox360_content_install = install_mode == "xbox360_content"

        if is_ps4_content_install:
            ps4_block_reason = self._ps4_content_install_block_reason(game)
            if ps4_block_reason:
                QMessageBox.warning(self, "Install Blocked", ps4_block_reason)
                return False
        elif is_xbox360_content_install:
            xbox360_block_reason = self._xbox360_content_install_block_reason(game)
            if xbox360_block_reason:
                QMessageBox.warning(self, "Install Blocked", xbox360_block_reason)
                return False
        else:
            install_block_reason = self._install_block_reason_for_game(game)
            if install_block_reason:
                QMessageBox.warning(self, "Install Blocked", install_block_reason)
                return False

        rom_id = self._resolve_rom_id_for_game(game)
        if not rom_id:
            QMessageBox.warning(self, "Install Error", "Selected game is missing a ROM id and cannot be downloaded.")
            return False

        install_game = dict(game)
        install_game["rom_id"] = rom_id
        self._hydrate_install_game_metadata(install_game, rom_id)
        resolved_file_name = self._resolved_rom_file_name_for_game(install_game, rom_id)
        if not resolved_file_name:
            QMessageBox.warning(
                self,
                "Install Error",
                "Server did not return a usable ROM filename/path for this title. Refresh server metadata and try again.",
            )
            return False
        install_game["rom_file_name"] = resolved_file_name
        sync_install_metadata_to_details_game(
            self.current_details_game,
            install_game,
            game_key=self._game_key,
        )

        install_path = self._platform_library_dir(install_game)
        if install_path is None:
            QMessageBox.warning(self, "Install Error", "Set a Library Path in Settings before installing games.")
            return False

        try:
            install_path.mkdir(parents=True, exist_ok=True)
        except OSError as error:
            QMessageBox.warning(self, "Install Error", f"Could not prepare library folder: {error}")
            return False

        base_url = self._server_base_url()
        if not base_url:
            QMessageBox.warning(self, "Install Error", "Set a Server URL in Settings before installing games.")
            return False

        nested_file_name_value = install_game.get("rom_nested_file_name", "")
        nested_file_name = nested_file_name_value.strip() if isinstance(nested_file_name_value, str) else ""
        archive_name_value = install_game.get("_archive_name_override", "")
        archive_name_override = archive_name_value.strip() if isinstance(archive_name_value, str) else ""
        archive_name = nested_file_name or archive_name_override or self._archive_name_for_game(install_game)
        archive_path = install_path / archive_name
        rom_id_path = quote(rom_id, safe="")
        file_name_path = quote(self._server_content_file_name_for_game(install_game), safe="")
        download_url = f"{base_url}/api/roms/{rom_id_path}/content/{file_name_path}"
        file_ids_csv_value = install_game.get("_ps4_file_ids_csv", "")
        file_ids_csv = file_ids_csv_value.strip() if isinstance(file_ids_csv_value, str) else ""
        if file_ids_csv:
            download_url = f"{download_url}?{urlencode({'file_ids': file_ids_csv})}"
        elif not is_ps4_content_install and not is_xbox360_content_install:
            base_file_id_value = install_game.get("rom_base_file_id", "")
            base_file_id = base_file_id_value.strip() if isinstance(base_file_id_value, str) else ""
            if base_file_id:
                download_url = f"{download_url}?{urlencode({'file_ids': base_file_id})}"

        install_key = self._game_key(install_game)
        pending_key = pending_install_key(
            self.install_in_progress,
            self.install_finalize_in_progress,
            self.install_pending_game,
            self.install_finalize_game,
        )
        queued_keys = queued_install_keys(self.install_queue)

        if self.install_in_progress or self.install_finalize_in_progress:
            if install_key == pending_key or install_key in queued_keys:
                return False
            queued_game = dict(install_game)
            entry_id = self._create_download_entry(queued_game, "queued")
            queued_game["_download_entry_id"] = entry_id
            self.install_queue.append(queued_game)
            self._update_details_action_buttons()
            self._update_download_status_ui()
            return True

        pending_entry_id = game.get("_download_entry_id", "") if isinstance(game.get("_download_entry_id", ""), str) else ""
        entry_id = pending_entry_id.strip()
        if entry_id:
            self._set_download_entry_status(entry_id, "downloading")
        else:
            entry_id = self._create_download_entry(install_game, "downloading")

        self.install_in_progress = True
        install_game["_download_entry_id"] = entry_id
        self.install_pending_game = install_game
        self.active_download_entry_id = entry_id
        self.active_download_count += 1
        self.active_download_bytes = 0
        self.active_download_total = 0
        self.active_download_speed_bps = 0.0
        self._update_download_status_ui()
        self._update_details_action_buttons()

        thread = QThread(self)
        worker = InstallDownloadWorker(
            download_url,
            self._download_headers(),
            archive_path,
            debug_enabled=self._debug_prints_enabled(),
        )
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.progress.connect(self._on_async_install_progress)
        worker.finished.connect(self._on_async_install_finished)
        worker.finished.connect(thread.quit)
        thread.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(self._on_install_thread_finished)

        self.install_thread = thread
        self.install_worker = worker
        thread.start()
        return True


    def _start_async_source_emulator_install(self, game: dict[str, str]) -> bool:
        install_mode_value = game.get("_install_mode", "source_emulator")
        install_mode = install_mode_value.strip().lower() if isinstance(install_mode_value, str) else "source_emulator"
        is_source_update = install_mode == "source_emulator_update"

        if not is_source_update:
            install_block_reason = self._install_block_reason_for_game(game)
            if install_block_reason:
                QMessageBox.warning(self, "Install Blocked", install_block_reason)
                return False

        source_metadata = game.get("_source_metadata")
        if not isinstance(source_metadata, dict):
            QMessageBox.warning(
                self,
                "Install Error",
                "Selected emulator source is missing metadata and cannot be downloaded.",
            )
            return False

        install_path = self._platform_library_dir(game)
        if install_path is None:
            QMessageBox.warning(self, "Install Error", "Set a Library Path in Settings before downloading emulators.")
            return False

        try:
            install_path.mkdir(parents=True, exist_ok=True)
        except OSError as error:
            QMessageBox.warning(self, "Install Error", f"Could not prepare library folder: {error}")
            return False

        archive_name_value = game.get("_archive_name_override", "")
        archive_name = (
            archive_name_value.strip()
            if isinstance(archive_name_value, str) and archive_name_value.strip()
            else self._archive_name_for_game(game)
        )
        archive_path = install_path / archive_name

        install_key = self._game_key(game)
        pending_key = pending_install_key(
            self.install_in_progress,
            self.install_finalize_in_progress,
            self.install_pending_game,
            self.install_finalize_game,
        )
        queued_keys = queued_install_keys(self.install_queue)

        if self.install_in_progress or self.install_finalize_in_progress:
            if install_key == pending_key or install_key in queued_keys:
                return False
            queued_game = dict(game)
            entry_id = self._create_download_entry(queued_game, "queued")
            queued_game["_download_entry_id"] = entry_id
            self.install_queue.append(queued_game)
            self._update_details_action_buttons()
            self._update_download_status_ui()
            return True

        pending_entry_id = game.get("_download_entry_id", "") if isinstance(game.get("_download_entry_id", ""), str) else ""
        entry_id = pending_entry_id.strip()
        if entry_id:
            self._set_download_entry_status(entry_id, "downloading")
        else:
            entry_id = self._create_download_entry(game, "downloading")

        self.install_in_progress = True
        install_game = dict(game)
        install_game["_download_entry_id"] = entry_id
        self.install_pending_game = install_game
        self.active_download_entry_id = entry_id
        self.active_download_count += 1
        self.active_download_bytes = 0
        self.active_download_total = 0
        self.active_download_speed_bps = 0.0
        self._update_download_status_ui()
        self._update_details_action_buttons()

        thread = QThread(self)
        worker = InstallDownloadWorker(
            "",
            {},
            archive_path,
            source_metadata=source_metadata,
            debug_enabled=self._debug_prints_enabled(),
        )
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.progress.connect(self._on_async_install_progress)
        worker.finished.connect(self._on_async_install_finished)
        worker.finished.connect(thread.quit)
        thread.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(self._on_install_thread_finished)

        self.install_thread = thread
        self.install_worker = worker
        thread.start()
        return True


    def _on_async_install_finished(self, archive_path: str, error: str) -> None:
        game = self.install_pending_game
        entry_id = self.active_download_entry_id
        self.install_in_progress = False
        self.install_pending_game = None
        self.active_download_entry_id = None
        self.active_download_count = active_download_count_after_finish(self.active_download_count)
        if should_reset_active_download_metrics(self.active_download_count):
            self.active_download_bytes = 0
            self.active_download_total = 0
            self.active_download_speed_bps = 0.0

        if game is None:
            if entry_id:
                status = download_entry_status_from_error(error)
                self._set_download_entry_status(entry_id, status, error)
            self._update_download_status_ui()
            self._update_details_action_buttons()
            self._start_next_queued_install()
            return

        title = game.get("title", "Game")
        install_mode_value = game.get("_install_mode", "base")
        install_mode = install_mode_value.strip().lower() if isinstance(install_mode_value, str) else "base"
        is_source_install = install_mode in {"source_emulator", "source_emulator_update"}
        if error:
            status = download_entry_status_from_error(error)
            if entry_id:
                self._set_download_entry_status(entry_id, status, error)
            self._update_download_status_ui()
            self._update_details_action_buttons()
            if status != "cancelled":
                failed_source = "source" if is_source_install else "server"
                QMessageBox.warning(self, "Install Error", f"Failed to download {title} from {failed_source}.")
            self._start_next_queued_install()
            return

        if install_mode not in {"ps4_content", "xbox360_content", "update", "source_emulator", "source_emulator_update"} and self._is_game_installed(game):
            if entry_id:
                self._set_download_entry_status(entry_id, "completed")
            self._update_download_status_ui()
            self._update_details_action_buttons()
            self._start_next_queued_install()
            return

        archive_file = Path(archive_path)
        if entry_id:
            self._set_download_entry_status(entry_id, "installing")
        self._start_async_install_finalize(game, archive_file, entry_id)


    def _start_async_install_finalize(self, game: dict[str, str], archive_file: Path, entry_id: str | None) -> None:
        self.install_finalize_in_progress = True
        self.install_finalize_game = dict(game)
        self.install_finalize_entry_id = entry_id
        self.active_install_bytes = 0
        self.active_install_total = 0
        if entry_id:
            self._set_download_entry_install_progress(entry_id, 0, 0)
        self._update_download_status_ui()
        self._update_details_action_buttons()

        install_mode_value = game.get("_install_mode", "base")
        install_mode = install_mode_value.strip().lower() if isinstance(install_mode_value, str) else "base"
        content_kind_value = game.get("_ps4_content_kind", "")
        content_kind = content_kind_value.strip().lower() if isinstance(content_kind_value, str) else ""
        if install_mode == "ps4_content":
            finalize_content_kind = content_kind
        elif install_mode == "xbox360_content":
            finalize_content_kind = "xenia_content"
        elif install_mode == "native_update":
            finalize_content_kind = "native_update"
        else:
            finalize_content_kind = ""

        thread = QThread(self)
        worker = InstallFinalizeWorker(self, game, archive_file, content_kind=finalize_content_kind)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.progress.connect(self._on_async_install_finalize_progress)
        worker.finished.connect(self._on_async_install_finalize_finished)
        worker.finished.connect(thread.quit)
        thread.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(self._on_install_finalize_thread_finished)

        self.install_finalize_thread = thread
        self.install_finalize_worker = worker
        thread.start()


    def _on_async_install_finalize_finished(
        self,
        prepared_game: object,
        archive_path: str,
        warning_text: str,
        error: str,
    ) -> None:
        game = self.install_finalize_game
        entry_id = self.install_finalize_entry_id
        self.install_finalize_in_progress = False
        self.install_finalize_game = None
        self.install_finalize_entry_id = None
        self.active_install_bytes = 0
        self.active_install_total = 0

        title = game.get("title", "Game") if isinstance(game, dict) else "Game"
        install_mode_value = game.get("_install_mode", "base") if isinstance(game, dict) else "base"
        install_mode = install_mode_value.strip().lower() if isinstance(install_mode_value, str) else "base"
        is_ps4_content_install = install_mode == "ps4_content"
        is_xbox360_content_install = install_mode == "xbox360_content"
        is_source_install = install_mode in {"source_emulator", "source_emulator_update"}
        is_source_update = install_mode == "source_emulator_update"
        is_native_update = install_mode == "native_update"
        content_kind_value = game.get("_ps4_content_kind", "") if isinstance(game, dict) else ""
        content_kind = content_kind_value.strip().lower() if isinstance(content_kind_value, str) else "content"
        if is_ps4_content_install:
            install_label = f"PS4 {content_kind}"
        elif is_xbox360_content_install:
            xenia_kind_value = game.get("_xenia_content_kind", "content") if isinstance(game, dict) else "content"
            install_label = f"Xbox 360 {xenia_kind_value.strip().lower() or 'content'}"
        else:
            install_label = title

        if error or not isinstance(prepared_game, dict):
            if entry_id:
                failure_text = error.strip() if isinstance(error, str) and error.strip() else "Failed to extract downloaded archive"
                self._set_download_entry_status(entry_id, "failed", failure_text)
            archive_file = Path(archive_path)
            if archive_file.exists() and archive_file.is_file():
                try:
                    archive_file.unlink()
                except OSError:
                    pass
            self._update_download_status_ui()
            self._update_details_action_buttons()
            if error:
                QMessageBox.warning(self, "Install Error", f"Failed to install {install_label}: {error}")
            self._start_next_queued_install()
            return

        installed_game = dict(prepared_game)
        if is_ps4_content_install:
            self._sync_ps4_content_metadata_to_installed_game(game, installed_game)
            if self.current_details_game is not None:
                self.current_details_game["ps4_game_id"] = installed_game.get("ps4_game_id", "")
                self.current_details_game["ps4_content"] = installed_game.get("ps4_content", "")
            if entry_id:
                self._set_download_entry_status(entry_id, "completed")
            self._show_install_warning_if_actionable(warning_text)
            self._update_download_status_ui()
            self._update_details_action_buttons()
            self._start_next_queued_install()
            return

        if is_xbox360_content_install:
            if entry_id:
                self._set_download_entry_status(entry_id, "completed")
            self._show_install_warning_if_actionable(warning_text)
            self._update_download_status_ui()
            self._update_details_action_buttons()
            self._start_next_queued_install()
            return

        if is_native_update:
            self._register_installed_game(installed_game, Path(archive_path))
            if entry_id:
                self._set_download_entry_status(entry_id, "completed")
            self._show_install_warning_if_actionable(warning_text)
            self._show_toast(f"Updated '{title}' successfully.", level="success")
            self._update_download_status_ui()
            self._update_details_action_buttons()
            self._start_next_queued_install()
            return

        archive_file = Path(archive_path)
        self._register_installed_game(installed_game, archive_file)
        self._write_rpcs3_games_yml_for_game(installed_game)
        self._queue_xbox360_content_for_game(installed_game)
        auto_configured = self._auto_configure_installed_emulator(installed_game, archive_file)
        if is_source_install:
            self._record_source_emulator_install(installed_game)
        if entry_id:
            self._set_download_entry_status(entry_id, "completed")
        self._show_install_warning_if_actionable(warning_text)
        if is_source_install:
            if auto_configured:
                emulator_title = installed_game.get("title", "Emulator")
                if is_source_update:
                    self._show_toast(f"Updated emulator '{emulator_title}' from source.", level="success")
                else:
                    self._show_toast(f"Installed emulator '{emulator_title}' from source.", level="success")
            else:
                QMessageBox.warning(
                    self,
                    "Install Warning",
                    "Emulator install completed, but automatic emulator configuration did not detect a launch path."
                    " Use Add New Emulator or Config to set the executable path manually.",
                )
        self._update_download_status_ui()
        self._update_details_action_buttons()
        self._start_next_queued_install()


    def _on_install_thread_finished(self) -> None:
        self.install_thread = None
        self.install_worker = None


    def _on_install_finalize_thread_finished(self) -> None:
        self.install_finalize_thread = None
        self.install_finalize_worker = None


    def _on_async_install_progress(self, downloaded_bytes: int, total_bytes: int, speed_bps: float) -> None:
        (
            self.active_download_bytes,
            self.active_download_total,
            self.active_download_speed_bps,
        ) = normalized_download_progress(downloaded_bytes, total_bytes, speed_bps)
        if self.active_download_entry_id:
            self._set_download_entry_progress(
                self.active_download_entry_id,
                self.active_download_bytes,
                self.active_download_total,
                self.active_download_speed_bps,
            )
        self._update_download_status_ui()


    def _on_async_install_finalize_progress(self, installed_bytes: int, total_bytes: int) -> None:
        self.active_install_bytes, self.active_install_total = normalized_transfer_progress(installed_bytes, total_bytes)
        if self.install_finalize_entry_id:
            self._set_download_entry_install_progress(
                self.install_finalize_entry_id,
                self.active_install_bytes,
                self.active_install_total,
            )
        self._update_download_status_ui()


    def _on_firmware_download_progress(self, downloaded_bytes: int, total_bytes: int, speed_bps: float) -> None:
        self.active_download_bytes = downloaded_bytes
        self.active_download_total = total_bytes
        self.active_download_speed_bps = speed_bps
        if self._firmware_download_entry_id:
            self._set_download_entry_progress(
                self._firmware_download_entry_id,
                downloaded_bytes,
                total_bytes,
                speed_bps,
            )
        self._update_download_status_ui()


    def _on_firmware_download_done(self, error: str) -> None:
        self.active_download_count = active_download_count_after_finish(self.active_download_count)
        if should_reset_active_download_metrics(self.active_download_count):
            self.active_download_bytes = 0
            self.active_download_total = 0
            self.active_download_speed_bps = 0.0
        entry_id = self._firmware_download_entry_id
        self._firmware_download_entry_id = None
        if entry_id:
            status = "failed" if error else "completed"
            self._set_download_entry_status(entry_id, status, error)
        self._update_download_status_ui()


    def _update_download_status_ui(self) -> None:
        if self.download_status_widget is None:
            return
        queued_count = len(self.install_queue)
        has_active_downloads = self.active_download_count > 0
        has_install_work = has_active_downloads or queued_count > 0 or self.install_finalize_in_progress
        self.download_status_widget.setVisible(has_install_work)
        if self.download_count_label is not None:
            self.download_count_label.setText(
                download_count_text(
                    self.active_download_count,
                    queued_count,
                    self.install_finalize_in_progress,
                )
            )
        if self.download_progress_bar is not None:
            min_value, max_value, progress_value, progress_text = download_progress_display(
                self.active_download_count,
                self.active_download_bytes,
                self.active_download_total,
                self.install_finalize_in_progress,
                self.active_install_bytes,
                self.active_install_total,
                queued_count,
            )
            self.download_progress_bar.setRange(min_value, max_value)
            if max_value > min_value:
                self.download_progress_bar.setValue(progress_value)
            self.download_progress_bar.setFormat(progress_text)
        if self.download_speed_label is not None:
            self.download_speed_label.setText(
                download_speed_text(
                    self.active_download_speed_bps,
                    self.install_finalize_in_progress,
                    self.active_download_count,
                    self.active_install_bytes,
                    self.active_install_total,
                )
            )


    def _create_download_entry(self, game: dict[str, Any], status: str, error: str = "") -> str:
        entry_id = f"{time.time_ns()}-{len(self.download_entries)}"
        self.download_entries.append(make_download_entry_data(entry_id, game, status, error))
        self._schedule_downloads_page_refresh()
        return entry_id


    def _find_download_entry(self, entry_id: str) -> dict[str, Any] | None:
        return find_download_entry(self.download_entries, entry_id)


    def _set_download_entry_status(self, entry_id: str, status: str, error: str = "") -> None:
        entry = self._find_download_entry(entry_id)
        if entry is None:
            return
        apply_download_entry_status(entry, status, error)
        self._schedule_downloads_page_refresh()


    def _set_download_entry_progress(self, entry_id: str, downloaded_bytes: int, total_bytes: int, speed_bps: float) -> None:
        entry = self._find_download_entry(entry_id)
        if entry is None:
            return
        apply_download_entry_progress(entry, downloaded_bytes, total_bytes, speed_bps)
        detail_label = self.download_entry_detail_labels.get(entry_id)
        if detail_label is not None:
            detail_label.setText(self._download_entry_detail_text(entry))


    def _set_download_entry_install_progress(self, entry_id: str, installed_bytes: int, total_bytes: int) -> None:
        entry = self._find_download_entry(entry_id)
        if entry is None:
            return
        apply_download_entry_install_progress(entry, installed_bytes, total_bytes)
        detail_label = self.download_entry_detail_labels.get(entry_id)
        if detail_label is not None:
            detail_label.setText(self._download_entry_detail_text(entry))


    def _dismiss_download_entry(self, entry_id: str) -> None:
        self.download_entries = remove_download_entry(self.download_entries, entry_id)
        self._schedule_downloads_page_refresh()


    def _schedule_downloads_page_refresh(self) -> None:
        if self.downloads_refresh_timer.isActive():
            return
        self.downloads_refresh_timer.start()


    def _retry_download_entry(self, entry_id: str) -> None:
        entry = self._find_download_entry(entry_id)
        if entry is None:
            return
        game_copy = retry_download_game(entry)
        if game_copy is None:
            return
        self._dismiss_download_entry(entry_id)
        self._start_async_install(game_copy)


    def _cancel_download_entry(self, entry_id: str) -> None:
        if self.active_download_entry_id == entry_id and self.install_worker is not None:
            self.install_worker.request_cancel()
            self._set_download_entry_status(entry_id, "cancelling")
            return

        queued_before = len(self.install_queue)
        self.install_queue = filter_queue_by_download_entry_id(self.install_queue, entry_id)
        if len(self.install_queue) != queued_before:
            self._set_download_entry_status(entry_id, "cancelled", "Cancelled while queued")
            self._update_download_status_ui()
            self._update_details_action_buttons()


    def _download_entry_detail_text(self, entry: dict[str, Any]) -> str:
        return download_entry_detail_text(entry)


    def _make_download_entry_widget(self, entry: dict[str, Any]) -> tuple[QWidget, QLabel]:
        return make_download_entry_widget(
            entry,
            self._download_entry_detail_text(entry),
            self._theme_color('muted', '#6272a4'),
            self._cancel_download_entry,
            self._retry_download_entry,
            self._dismiss_download_entry,
        )


    def _refresh_downloads_page(self) -> None:
        if self.downloads_list_layout is None or self.downloads_empty_label is None or self.downloads_scroll is None:
            return

        self.download_entry_detail_labels = refresh_downloads_page(
            self.downloads_list_layout,
            self.downloads_empty_label,
            self.downloads_scroll,
            self.download_entries,
            self._make_download_entry_widget,
        )


