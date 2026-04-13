import sys
import json
import base64
import hashlib
import os
import re
import stat
import time
import shlex
import shutil
import subprocess
import zipfile
import mimetypes
import tempfile
import threading
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Callable
from urllib.error import HTTPError, URLError
from urllib.parse import parse_qsl, quote, urlencode, urlsplit, urlunsplit
from urllib.request import Request, urlopen

from PySide6.QtCore import QObject, QSize, QThread, QTimer, Qt, QUrl, Signal
from PySide6.QtGui import QCloseEvent, QDesktopServices, QIcon, QPixmap
from PySide6.QtNetwork import QNetworkAccessManager, QNetworkReply, QNetworkRequest
from PySide6.QtWidgets import (
    QApplication,
    QAbstractItemView,
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFrame,
    QFormLayout,
    QGridLayout,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QLineEdit,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QProgressBar,
    QScrollArea,
    QSizePolicy,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from rom_mate.core import (
    api_get_bytes,
    api_get_json,
    api_post_json,
    api_post_multipart_json,
    api_put_multipart_json,
    build_auth_headers,
    build_binary_auth_headers,
    format_http_error_details,
    merge_config_with_defaults as resolve_merge_config_with_defaults,
    normalize_default_emulators as resolve_normalize_default_emulators,
    normalize_default_retroarch_cores as resolve_normalize_default_retroarch_cores,
    normalize_emulators as resolve_normalize_emulators,
    normalize_installed_games as resolve_normalize_installed_games,
    load_api_token as resolve_load_api_token,
    load_ra_token as resolve_load_ra_token,
    path_key,
    path_within_path,
    sanitize_path_component,
    multipart_payload,
    save_api_token as resolve_save_api_token,
    save_ra_token as resolve_save_ra_token,
    set_api_token as resolve_set_api_token,
    windows_protect_data as resolve_windows_protect_data,
    windows_unprotect_data as resolve_windows_unprotect_data,
    write_config_file as resolve_write_config_file,
)
from rom_mate.library import (
    active_download_count_after_finish,
    apply_ps4_content_archive_without_ui as resolve_apply_ps4_content_archive_without_ui,
    apply_xenia_content_archive_without_ui as resolve_apply_xenia_content_archive_without_ui,
    apply_download_entry_install_progress,
    apply_download_entry_progress,
    apply_download_entry_status,
    archive_name_for_game as resolve_archive_name_for_game,
    auto_cloud_upload_plan,
    build_installed_game_record as resolve_build_installed_game_record,
    can_start_next_queued_install,
    cemu_save_directories_for_game,
    cleanup_install_archive as resolve_cleanup_install_archive,
    candidate_archive_paths_for_game as resolve_candidate_archive_paths_for_game,
    candidate_extracted_dirs_for_game as resolve_candidate_extracted_dirs_for_game,
    candidate_extracted_paths_for_game as resolve_candidate_extracted_paths_for_game,
    cleanup_temporary_paths,
    cloud_sync_candidates_for_game,
    cloud_sync_directory_candidates_for_game,
    cloud_sync_state,
    cloud_sync_state_for_game,
    cloud_sync_state_key,
    directory_archive_upload_jobs,
    directory_total_file_bytes as resolve_directory_total_file_bytes,
    extract_zip_archive_bytes_to_directory,
    extracted_dir_for_archive_path as resolve_extracted_dir_for_archive_path,
    extract_archive_for_game as resolve_extract_archive_for_game,
    extract_archive_into_directory as resolve_extract_archive_into_directory,
    file_upload_jobs,
    filter_directories_by_mtime_window,
    grouped_file_upload_jobs,
    filter_files_by_mtime_window,
    filter_upload_jobs_by_session_window,
    is_local_newer_than_server,
    latest_server_record,
    latest_server_records_by_slot,
    library_games_without_keys as resolve_library_games_without_keys,
    relative_timestamp_text,
    matching_installed_emulator_games as resolve_matching_installed_emulator_games,
    merge_archive_into_directory as resolve_merge_archive_into_directory,
    native_executable_candidates_for_game as resolve_native_executable_candidates_for_game,
    native_install_dir_for_game as resolve_native_install_dir_for_game,
    no_matching_upload_message,
    ps3_game_id_from_paths as resolve_ps3_game_id_from_paths,
    ps3_game_id_from_text as resolve_ps3_game_id_from_text,
    normalize_candidate_url,
    download_count_text,
    download_entry_detail_text,
    download_progress_display,
    download_speed_text,
    download_entry_status_from_error,
    filter_queue_by_download_entry_id,
    find_download_entry,
    format_size,
    game_has_server_update,
    has_newer_server_rom_version,
    rom_file_name_version,
    game_key,
    hydrate_install_game_metadata as resolve_hydrate_install_game_metadata,
    games_match_identity,
    installed_game_record,
    is_game_install_queued,
    is_game_installed,
    make_download_entry_data,
    normalize_cloud_sync_state,
    normalized_download_progress,
    normalized_transfer_progress,
    partition_active_game_sessions,
    pending_install_key,
    percent_text,
    queued_install_keys,
    remove_download_entry,
    remove_game_files as resolve_remove_game_files,
    uninstall_library_games as resolve_uninstall_library_games,
    ppsspp_state_upload_jobs,
    retroarch_state_upload_jobs,
    prepare_installed_game_without_ui as resolve_prepare_installed_game_without_ui,
    restore_single_save_payload,
    restore_single_state_payload,
    retry_download_game,
    resolved_native_executable_path_for_game as resolve_resolved_native_executable_path_for_game,
    rom_id_key,
    server_content_file_name_for_game as resolve_server_content_file_name_for_game,
    save_record_timestamp,
    select_extracted_launch_file as resolve_select_extracted_launch_file,
    server_records_from_payload,
    sort_server_records_by_recency,
    should_extract_archive_for_game as resolve_should_extract_archive_for_game,
    session_cloud_sync_updates,
    screenshot_download_candidate_paths,
    should_skip_known_latest,
    state_download_candidate_paths,
    tar_archive_total_install_bytes as resolve_tar_archive_total_install_bytes,
    tar_listing_line_size as resolve_tar_listing_line_size,
    session_filtered_directory_candidates,
    session_filtered_file_candidates,
    session_window_for_state_upload,
    should_reset_active_download_metrics,
    upload_completion_message,
    zip_directory_for_upload,
    zip_selected_files_for_upload,
    summarize_auto_cloud_upload_result,
    sync_install_metadata_to_details_game as resolve_sync_install_metadata_to_details_game,
    update_cloud_sync_state_for_game,
)
from rom_mate.library.cloud_transfer import SUPPORTED_IMAGE_EXTENSIONS, session_screenshot_path
from rom_mate.library.firmware_install import download_ps3_firmware_direct, install_platform_firmware
from rom_mate.cover import (
    apply_cover_to_label,
    cache_cover_image_for_game,
    cached_cover_cache_key,
    cached_cover_for_game,
    cached_cover_path_from_game,
    cached_cover_path_keys_for_games,
    cleanup_cached_cover_for_game,
    cover_cache_extension_from_payload,
    cover_url_from_rom_payload,
    installed_cover_cache_key,
    on_cover_reply,
    queue_cover_load,
    queue_game_cover_load,
    rescale_details_media_for_current_sizes as resolve_rescale_details_media_for_current_sizes,
    resolve_cover_url,
    resolved_cover_url_for_game as resolve_resolved_cover_url_for_game,
    screenshot_urls_from_game,
    screenshot_urls_from_rom_payload,
    update_details_layout_metrics as resolve_update_details_layout_metrics,
    update_details_screenshots as resolve_update_details_screenshots,
)
from rom_mate.emulator import (
    all_retroarch_cores as resolve_all_retroarch_cores,
    apply_launch_placeholders_to_args as resolve_apply_launch_placeholders_to_args,
    apply_manual_emulator_profile_defaults as resolve_apply_manual_emulator_profile_defaults,
    assign_profile_platform_defaults as resolve_assign_profile_platform_defaults,
    auto_configure_emulator_settings as resolve_auto_configure_emulator_settings,
    auto_configured_emulator_name as resolve_auto_configured_emulator_name,
    emulator_install_directory as resolve_emulator_install_directory,
    available_emulator_name_for_platform as resolve_available_emulator_name_for_platform,
    cloud_save_block_reason_for_game as resolve_cloud_save_block_reason_for_game,
    cloud_save_scope_for_game as resolve_cloud_save_scope_for_game,
    azahar_save_path_overrides as resolve_azahar_save_path_overrides,
    azahar_state_path_overrides as resolve_azahar_state_path_overrides,
    cemu_save_path_overrides as resolve_cemu_save_path_overrides,
    dolphin_save_path_overrides as resolve_dolphin_save_path_overrides,
    dolphin_state_path_overrides as resolve_dolphin_state_path_overrides,
    eden_save_path_overrides as resolve_eden_save_path_overrides,
    fbneo_save_path_overrides as resolve_fbneo_save_path_overrides,
    fbneo_state_path_overrides as resolve_fbneo_state_path_overrides,
    mame_save_path_overrides as resolve_mame_save_path_overrides,
    mame_state_path_overrides as resolve_mame_state_path_overrides,
    compatible_emulator_names_for_platform as resolve_compatible_emulator_names_for_platform,
    pcsx2_save_path_overrides as resolve_pcsx2_save_path_overrides,
    pcsx2_state_path_overrides as resolve_pcsx2_state_path_overrides,
    pcsx2_windows_documents_folder as resolve_pcsx2_windows_documents_folder,
    pico8_save_path_overrides as resolve_pico8_save_path_overrides,
    redream_save_path_overrides as resolve_redream_save_path_overrides,
    redream_state_path_overrides as resolve_redream_state_path_overrides,
    xemu_save_path_overrides as resolve_xemu_save_path_overrides,
    xenia_directory_settings as resolve_xenia_directory_settings,
    xenia_save_path_overrides as resolve_xenia_save_path_overrides,
    xenia_state_path_overrides as resolve_xenia_state_path_overrides,
    rpcs3_pup_path as resolve_rpcs3_pup_path,
    rpcs3_save_path_overrides as resolve_rpcs3_save_path_overrides,
    trigger_rpcs3_firmware_install as resolve_trigger_rpcs3_firmware_install,
    default_assignable_server_platforms as resolve_default_assignable_server_platforms,
    default_emulator_autoprofiles as resolve_default_emulator_autoprofiles,
    default_emulator_name_for_platform as resolve_default_emulator_name_for_platform,
    ensure_azahar_settings as resolve_ensure_azahar_settings,
    ensure_cemu_controller_config as resolve_ensure_cemu_controller_config,
    ensure_cemu_settings as resolve_ensure_cemu_settings,
    ensure_dolphin_settings as resolve_ensure_dolphin_settings,
    ensure_dolphin_skip_ipl as resolve_ensure_dolphin_skip_ipl,
    ensure_dolphin_gcpad_config as resolve_ensure_dolphin_gcpad_config,
    ensure_duckstation_memory_card_settings as resolve_ensure_duckstation_memory_card_settings,
    ensure_eden_settings as resolve_ensure_eden_settings,
    ensure_pcsx2_settings as resolve_ensure_pcsx2_settings,
    ensure_ppsspp_settings as resolve_ensure_ppsspp_settings,
    ensure_rpcs3_settings as resolve_ensure_rpcs3_settings,
    ps3_vfs_dev_hdd0_path as resolve_ps3_vfs_dev_hdd0_path,
    ensure_redream_settings as resolve_ensure_redream_settings,
    ensure_xemu_settings as resolve_ensure_xemu_settings,
    dolphin_target_platforms_for_variant as resolve_dolphin_target_platforms_for_variant,
    dolphin_variant_label_for_game as resolve_dolphin_variant_label_for_game,
    emulator_autoprofiles_path as resolve_emulator_autoprofiles_path,
    emulator_entry_by_name as resolve_emulator_entry_by_name,
    emulator_entry_matches_tokens as resolve_emulator_entry_matches_tokens,
    emulator_entry_has_usable_path as resolve_emulator_entry_has_usable_path,
    emulator_profile_for_entry as resolve_emulator_profile_for_entry,
    emulator_profile_for_game as resolve_emulator_profile_for_game,
    install_block_reason_for_game as resolve_install_block_reason_for_game,
    is_arcade_platform as resolve_is_arcade_platform,
    is_emulators_platform as resolve_is_emulators_platform,
    is_native_executable_platform as resolve_is_native_executable_platform,
    is_ps3_emulator_entry as resolve_is_ps3_emulator_entry,
    is_ps3_platform as resolve_is_ps3_platform,
    is_rpcs3_emulator_name as resolve_is_rpcs3_emulator_name,
    installed_retroarch_core_ids as resolve_installed_retroarch_core_ids,
    launch_placeholders_for_game as build_launch_placeholders_for_game,
    load_emulator_autoprofiles as resolve_emulator_autoprofiles,
    load_retroarch_compatibility_map as resolve_load_retroarch_compatibility_map,
    launchable_emulator_file as resolve_launchable_emulator_file,
    launchable_native_game_file as resolve_launchable_native_game_file,
    mapping_value_for_platform as resolve_mapping_value_for_platform,
    matching_platforms_for_emulator_keywords as resolve_matching_platforms_for_emulator_keywords,
    normalize_emulator_autoprofiles as resolve_normalize_emulator_autoprofiles,
    normalize_ignore_extension_value as resolve_normalize_ignore_extension_value,
    normalize_retroarch_platform_key as resolve_normalize_retroarch_platform_key,
    normalize_save_strategy_value as resolve_normalize_save_strategy_value,
    retroarch_core_id_from_file_name as resolve_retroarch_core_id_from_file_name,
    retroarch_core_config_files_metadata as resolve_retroarch_core_config_files_metadata,
    retroarch_core_saves_files_metadata as resolve_retroarch_core_saves_files_metadata,
    retroarch_core_firmware_metadata as resolve_retroarch_core_firmware_metadata,
    retroarch_core_argument_path as resolve_retroarch_core_argument_path,
    retroarch_core_id_from_name as resolve_retroarch_core_id_from_name,
    retroarch_core_list_path as resolve_retroarch_core_list_path,
    retroarch_core_value as resolve_retroarch_core_value,
    retroarch_cores_for_platform as resolve_retroarch_cores_for_platform,
    retroarch_directory_settings as resolve_retroarch_directory_settings,
    ensure_retroarch_save_location_settings as resolve_ensure_retroarch_save_location_settings,
    retroarch_markdown_label as resolve_retroarch_markdown_label,
    retroarch_platform_tokens as resolve_retroarch_platform_tokens,
    retroarch_system_keys_for_platform as resolve_retroarch_system_keys_for_platform,
    prepare_emulator_launch_command as resolve_prepare_emulator_launch_command,
    prepare_native_launch_command as resolve_prepare_native_launch_command,
    process_exited_early_message as resolve_process_exited_early_message,
    resolve_launch_arguments_for_game as resolve_launch_arguments_for_game,
    resolve_rom_path_for_game as resolve_rom_path_for_game,
    resolved_emulator_entry_for_game as resolve_resolved_emulator_entry_for_game,
    resolved_ignore_basenames_for_emulator as resolve_resolved_ignore_basenames_for_emulator,
    resolved_ignore_extensions_for_emulator as resolve_resolved_ignore_extensions_for_emulator,
    resolved_save_strategy_for_emulator as resolve_resolved_save_strategy_for_emulator,
    select_emulator_executable_path as resolve_select_emulator_executable_path,
    normalized_retroarch_core_args as resolve_normalized_retroarch_core_args,
    split_configured_paths as resolve_split_configured_paths,
    split_launch_template_args as resolve_split_launch_template_args,
    strip_wrapping_quotes as resolve_strip_wrapping_quotes,
    validate_launch_placeholders as resolve_validate_launch_placeholders,
)
from rom_mate.emulator.retroarch import retroarch_core_flags as resolve_retroarch_core_flags
from rom_mate.emulator.retroarch import retroarch_core_flags_for_platform as resolve_retroarch_core_flags_for_platform
from rom_mate.emulator.retroarch import flycast_vmu_file_candidates as resolve_flycast_vmu_file_candidates
from rom_mate.ui import (
    EmulatorConfigDialog,
    FirstRunSetupDialog,
    NativeGameSettingsDialog,
    apply_theme_inline_styles as resolve_apply_theme_inline_styles,
    build_downloads_page,
    emulator_form_state_for_row,
    make_emulator_entry_payload,
    is_hidden_library_platform as resolve_is_hidden_library_platform,
    make_download_entry_widget,
    make_game_card as resolve_make_game_card,
    mapping_list_entries,
    normalized_theme_choice as resolve_normalized_theme_choice,
    open_game_details as resolve_open_game_details,
    preferred_emulator_selection,
    refresh_downloads_page,
    remove_emulator_default_mappings,
    resolved_theme_variant as resolve_resolved_theme_variant,
    selected_retroarch_core,
    themed_svg_icon as resolve_themed_svg_icon,
    update_details_action_buttons as resolve_update_details_action_buttons,
    theme_color as resolve_theme_color,
    theme_colors as resolve_theme_colors,
    theme_stylesheet as resolve_theme_stylesheet,
    visible_library_games as resolve_visible_library_games,
    show_toast as resolve_show_toast,
    upsert_emulator_entry,
)
from rom_mate.ui.emulators import save_button_label
from rom_mate.ui.emulators import available_source_download_emulator_entries as resolve_available_source_download_emulator_entries
from rom_mate.server import (
    account_status_text,
    apply_server_status,
    cache_rom_id_for_details_game as resolve_cache_rom_id_for_details_game,
    classify_connection_failure,
    clear_cached_rom_id_for_details_game as resolve_clear_cached_rom_id_for_details_game,
    clear_server_connection_data as resolve_clear_server_connection_data,
    clear_server_search,
    connected_username,
    credentials_present,
    details_rom_id_cache as resolve_details_rom_id_cache,
    details_rom_id_cache_key as resolve_details_rom_id_cache_key,
    fetch_connection_payloads,
    fetch_server_rom_payload as resolve_fetch_server_rom_payload,
    on_server_platform_selected,
    on_server_search_changed,
    populate_server_platforms as resolve_populate_server_platforms,
    render_server_games,
    resolve_rom_id_for_game as resolve_resolve_rom_id_for_game,
    resolved_rom_file_name_for_game as resolve_resolved_rom_file_name_for_game,
    rom_file_name_from_payload as resolve_rom_file_name_from_payload,
    server_base_url,
    server_platform_ids,
)
from rom_mate.background import AutoCloudSaveUploadWorker, DetailsCloudRecordsWorker, InstallDownloadWorker, InstallFinalizeWorker
from rom_mate.ui.mixins.cloud_mixin import CloudSaveMixin
from rom_mate.ui.mixins.emulator_ui_mixin import EmulatorUIMixin
from rom_mate.ui.mixins.install_mixin import InstallMixin
from rom_mate.ui.mixins.details_view_mixin import DetailsViewMixin


class MainWindow(CloudSaveMixin, EmulatorUIMixin, InstallMixin, DetailsViewMixin, QMainWindow):
    _emulator_refresh_requested = Signal()
    _toast_requested = Signal(str, str)  # message, level
    _firmware_download_progress = Signal(int, int, float)  # downloaded_bytes, total_bytes, speed_bps
    _firmware_download_done = Signal(str)  # error string (empty on success)
    _platform_games_ready = Signal(str)  # platform_label — results staged in _platform_games_results

    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("Rom Mate Neo")
        self.setMinimumSize(1280, 640)
        self.resize(1280, 760)

        self.config = self._load_config()
        import logging as _logging
        _fw_logger = _logging.getLogger("rom_mate.library.firmware_install")
        if not _fw_logger.handlers:
            _fw_handler = _logging.StreamHandler()
            _fw_handler.setFormatter(_logging.Formatter("[DEBUG][Firmware] %(message)s"))
            _fw_logger.addHandler(_fw_handler)
        _fw_logger.setLevel(
            _logging.DEBUG if self._debug_prints_enabled() else _logging.WARNING
        )
        self.active_theme_choice = self._normalized_theme_choice(self.config.get("theme", "system"))
        self.active_theme_variant = self._resolved_theme_variant(self.active_theme_choice)
        self.active_theme_colors = self._theme_colors(self.active_theme_variant)
        self.server_url_input: QLineEdit | None = None
        self.api_token_input: QLineEdit | None = None
        self.ra_username_input: QLineEdit | None = None
        self.ra_api_key_input: QLineEdit | None = None
        self.ra_password_input: QLineEdit | None = None
        self.ra_login_status_label: QLabel | None = None
        self.ra_login_button: QPushButton | None = None
        self.ra_clear_button: QPushButton | None = None
        self.library_path_input: QLineEdit | None = None
        self.debug_prints_checkbox: QCheckBox | None = None
        self.auto_cloud_download_checkbox: QCheckBox | None = None
        self.auto_cloud_upload_checkbox: QCheckBox | None = None
        self.auto_cloud_skip_local_newer_checkbox: QCheckBox | None = None
        self.theme_input: QComboBox | None = None
        self.settings_status_label: QLabel | None = None
        self.account_status_label: QLabel | None = None
        self.emulator_list: QListWidget | None = None
        self.emulator_name_input: QLineEdit | None = None
        self.emulator_path_input: QLineEdit | None = None
        self.emulator_args_input: QLineEdit | None = None
        self.emulator_save_strategy_input: QComboBox | None = None
        self.emulator_ignore_files_input: QLineEdit | None = None
        self.emulator_ignore_extensions_input: QLineEdit | None = None
        self.emulator_save_paths_input: QLineEdit | None = None
        self.emulator_state_paths_input: QLineEdit | None = None
        self.save_emulator_button: QPushButton | None = None
        self.default_platform_combo: QComboBox | None = None
        self.default_emulator_combo: QComboBox | None = None
        self.default_core_combo: QComboBox | None = None
        self.default_mapping_list: QListWidget | None = None
        self.details_title_label: QLabel | None = None
        self.details_content_frame: QFrame | None = None
        self.details_center_stack: QStackedWidget | None = None
        self.details_cover_label: QLabel | None = None
        self.details_platform_label: QLabel | None = None
        self.details_version_label: QLabel | None = None
        self.details_rating_label: QLabel | None = None
        self.details_description_label: QLabel | None = None
        self.details_screenshot_labels: list[QLabel] = []
        self.details_screenshots_panel: QWidget | None = None
        self.details_screenshots_scroll: QScrollArea | None = None
        self.details_primary_button: QPushButton | None = None
        self.details_config_button: QPushButton | None = None
        self.details_details_button: QPushButton | None = None
        self.details_manage_saves_button: QPushButton | None = None
        self.details_manage_states_button: QPushButton | None = None
        self.details_ps4_content_button: QPushButton | None = None
        self.details_xbox360_content_button: QPushButton | None = None
        self.details_cloud_title_label: QLabel | None = None
        self.details_cloud_status_label: QLabel | None = None
        self.details_cloud_empty_label: QLabel | None = None
        self.details_cloud_upload_button: QPushButton | None = None
        self.details_cloud_list_layout: QVBoxLayout | None = None
        self.details_secondary_button: QPushButton | None = None
        self.details_update_button: QPushButton | None = None
        self.details_achievements_button: QPushButton | None = None
        self.server_platforms_list: QListWidget | None = None
        self.server_games_grid: QGridLayout | None = None
        self.server_games_content: QWidget | None = None
        self.server_search_input: QLineEdit | None = None
        self.server_search_clear_button: QPushButton | None = None
        self.library_scroll: QScrollArea | None = None
        self.library_empty_label: QLabel | None = None
        self.server_games_scroll: QScrollArea | None = None
        self.server_status_label: QLabel | None = None
        self.server_platform_ids: dict[str, int] = {}
        self.server_connected = False
        self.server_auto_reconnect = True
        self.cover_cache: dict[str, QPixmap | None] = {}
        self.cover_waiters: dict[str, list[QLabel]] = {}
        self.cover_loading: set[str] = set()
        self.cover_network = QNetworkAccessManager(self)
        self.current_main_page_index = 0
        self.current_details_game: dict[str, str] | None = None
        self._current_details_game: dict[str, str] | None = None
        self.current_details_source = "library"
        self.current_details_cloud_mode = "overview"
        self._pending_achievements_request_id: int | None = None
        self._ra_thread: QThread | None = None
        self._ra_worker: QObject | None = None
        self._ra_login_thread: QThread | None = None
        self._ra_login_worker: QObject | None = None
        self.details_achievements_panel: QWidget | None = None
        self._pending_pcgw_request_id: int | None = None
        self._pcgw_thread: QThread | None = None
        self._pcgw_worker: QObject | None = None
        self._pcgw_paths_cache: dict[str, list[str]] = {}
        self._retroarch_core_ids_cache: dict[str, set[str]] = {}
        self._platform_default_emulator_cache: dict[str, str] = {}
        self._platform_available_emulator_cache: dict[str, str] = {}
        self._server_platforms_loading: set[str] = set()
        self._platform_games_results: dict[str, tuple] = {}  # platform_label -> (games, rom_payloads, error)
        self._emulator_sync_settings_done: set[str] = set()
        self._cloud_emulator_entry_cache: dict[str, tuple] = {}  # (title::platform::save_type) -> (emulator_name, entry)
        self._sync_directory_paths_cache: dict[tuple[str, str, str], list] = {}  # (name, path, key) -> list[Path]
        saved_manual = self.config.get("native_manual_save_paths", {})
        if isinstance(saved_manual, dict):
            for title_key, paths in saved_manual.items():
                if isinstance(title_key, str) and isinstance(paths, list):
                    clean_paths = [p for p in paths if isinstance(p, str) and p]
                    if clean_paths:
                        self._pcgw_paths_cache[title_key + "__manual"] = clean_paths
        self.install_in_progress = False
        self.install_pending_game: dict[str, str] | None = None
        self.install_queue: list[dict[str, str]] = []
        self.install_thread: QThread | None = None
        self.install_worker: InstallDownloadWorker | None = None
        self.install_finalize_in_progress = False
        self.install_finalize_game: dict[str, str] | None = None
        self.install_finalize_entry_id: str | None = None
        self.install_finalize_thread: QThread | None = None
        self.install_finalize_worker: InstallFinalizeWorker | None = None
        self.download_status_widget: QWidget | None = None
        self.download_count_label: QLabel | None = None
        self.download_progress_bar: QProgressBar | None = None
        self.download_speed_label: QLabel | None = None
        self.downloads_scroll: QScrollArea | None = None
        self.downloads_list_layout: QVBoxLayout | None = None
        self.downloads_empty_label: QLabel | None = None
        self.downloads_refresh_timer = QTimer(self)
        self.downloads_refresh_timer.setSingleShot(True)
        self.downloads_refresh_timer.setInterval(120)
        self.downloads_refresh_timer.timeout.connect(self._refresh_downloads_page)
        self._emulator_refresh_requested.connect(self._refresh_emulator_views)
        self._platform_games_ready.connect(self._on_platform_games_ready)
        self._toast_requested.connect(self._show_toast)
        self._firmware_download_progress.connect(self._on_firmware_download_progress)
        self._firmware_download_done.connect(self._on_firmware_download_done)
        self.download_entry_detail_labels: dict[str, QLabel] = {}
        self.active_download_count = 0
        self.active_download_bytes = 0
        self.active_download_total = 0
        self.active_download_speed_bps = 0.0
        self.active_download_entry_id: str | None = None
        self._firmware_download_entry_id: str | None = None
        self.active_install_bytes = 0
        self.active_install_total = 0
        self.download_entries: list[dict[str, Any]] = []
        self.library_games = self._normalize_installed_games(self.config.get("installed_games", []))
        self.server_games_by_platform: dict[str, list[dict[str, str]]] = {}
        self.installed_game_update_keys: set[tuple[str, str]] = set()
        self.server_rom_payloads: dict[str, dict[str, Any]] = {}
        self.retroarch_compatibility_map: dict[str, list[str]] | None = None
        self.emulator_autoprofiles: list[dict[str, Any]] | None = None
        self.active_game_sessions: list[dict[str, Any]] = []
        self.auto_cloud_upload_threads: list[QThread] = []
        self.auto_cloud_upload_workers: list[AutoCloudSaveUploadWorker] = []
        self.details_cloud_threads: list[QThread] = []
        self.details_cloud_workers: list[DetailsCloudRecordsWorker] = []
        self.details_cloud_request_id = 0
        self.details_cloud_request_context: dict[str, Any] = {}
        self.session_poll_timer = QTimer(self)
        self.session_poll_timer.setSingleShot(False)
        self.session_poll_timer.setInterval(2500)
        self.session_poll_timer.timeout.connect(self._poll_active_game_sessions)
        self.session_poll_timer.start()

        root = QWidget()
        self.setCentralWidget(root)

        layout = QVBoxLayout(root)
        layout.setContentsMargins(16, 16, 16, 16)
        layout.setSpacing(12)

        nav_row = QHBoxLayout()
        nav_row.setSpacing(8)
        layout.addLayout(nav_row)

        self.stack = QStackedWidget()
        self.stack.addWidget(self._build_library_page())
        self.stack.addWidget(self._build_server_page())
        self.stack.addWidget(self._build_downloads_page())
        self.stack.addWidget(self._build_emulators_page())
        self.stack.addWidget(self._build_settings_page())
        self.stack.addWidget(self._build_game_details_page())
        layout.addWidget(self.stack)

        nav_buttons_by_label: dict[str, QPushButton] = {}
        for index, label in enumerate(("Library", "Server", "Downloads", "Emulators", "Settings")):
            button = QPushButton(label)
            button.setCheckable(True)
            button.clicked.connect(lambda checked, idx=index: self._switch_page(idx))
            button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            nav_buttons_by_label[label] = button

        self.nav_buttons = [
            nav_buttons_by_label["Library"],
            nav_buttons_by_label["Server"],
            nav_buttons_by_label["Downloads"],
            nav_buttons_by_label["Emulators"],
            nav_buttons_by_label["Settings"],
        ]

        nav_row.addWidget(nav_buttons_by_label["Library"])
        nav_row.addWidget(nav_buttons_by_label["Server"])
        nav_row.addWidget(nav_buttons_by_label["Downloads"])

        nav_row.addStretch()

        download_widget = QWidget()
        download_layout = QHBoxLayout(download_widget)
        download_layout.setContentsMargins(8, 4, 8, 4)
        download_layout.setSpacing(8)

        download_count = QLabel("0 active downloads")
        download_count.setStyleSheet(f"font-weight: 600; color: {self._theme_color('text', '#f8f8f2')};")
        self.download_count_label = download_count
        download_layout.addWidget(download_count)

        download_progress = QProgressBar()
        download_progress.setRange(0, 100)
        download_progress.setValue(0)
        download_progress.setTextVisible(False)
        download_progress.setFormat("0%")
        download_progress.setFixedWidth(220)
        self.download_progress_bar = download_progress
        download_layout.addWidget(download_progress)

        download_speed = QLabel("0 B/s")
        download_speed.setStyleSheet(f"font-weight: 600; color: {self._theme_color('accent', '#8be9fd')};")
        self.download_speed_label = download_speed
        download_layout.addWidget(download_speed)

        download_widget.setObjectName("downloadStatusWidget")
        download_widget.setVisible(False)
        self.download_status_widget = download_widget
        nav_row.addWidget(download_widget, 0, Qt.AlignmentFlag.AlignHCenter)
        nav_row.addStretch()

        self.account_status_label = QLabel()
        self.account_status_label.setStyleSheet(f"font-weight: 600; color: {self._theme_color('text', '#f8f8f2')};")
        nav_row.addWidget(self.account_status_label)

        nav_row.addWidget(nav_buttons_by_label["Emulators"])
        nav_row.addWidget(nav_buttons_by_label["Settings"])
        self._update_top_bar_identity()
        self._switch_page(0)
        self._apply_theme(self.active_theme_choice)

        self._refresh_library_grid()
        self._refresh_emulator_views()
        self._restore_window_geometry()
        self._connect_to_server(show_errors=False)

    def closeEvent(self, event: QCloseEvent) -> None:
        self._persist_window_geometry()
        super().closeEvent(event)

    def _switch_page(self, index: int) -> None:
        previous_index = self.stack.currentIndex()
        if previous_index == 5 and index != 5:
            self._cleanup_details_view_state()
        self.stack.setCurrentIndex(index)
        self.current_main_page_index = index
        for i, button in enumerate(self.nav_buttons):
            button.setChecked(i == index)
        if index == 1 and not self.server_connected and self.server_auto_reconnect:
            self._connect_to_server(show_errors=False)
        QTimer.singleShot(0, self._reflow_current_page_grid)

    def _reflow_current_page_grid(self) -> None:
        current_index = self.stack.currentIndex()
        if current_index == 0:
            self._refresh_library_grid()
            return
        if current_index == 1 and self.server_platforms_list is not None:
            selected_item = self.server_platforms_list.currentItem()
            if selected_item is not None:
                self._render_server_games(selected_item.text())

    def _build_library_page(self) -> QWidget:
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.setSpacing(10)

        header = QLabel("Installed Games")
        header.setStyleSheet("font-size: 20px; font-weight: 700;")
        layout.addWidget(header)

        content_frame = QFrame()
        content_frame.setObjectName("panel")
        content_layout = QVBoxLayout(content_frame)
        content_layout.setContentsMargins(12, 10, 12, 10)
        content_layout.setSpacing(10)

        empty_label = QLabel("No games installed...")
        empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        empty_label.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
        empty_label.setStyleSheet("color: #6272a4; font-size: 16px; font-weight: 600;")
        self.library_empty_label = empty_label
        content_layout.addWidget(empty_label)

        scroll = QScrollArea()
        scroll.setObjectName("libraryScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.viewport().setObjectName("libraryScrollViewport")
        self.library_scroll = scroll

        content = QWidget()
        content.setObjectName("libraryGridContent")
        grid = QGridLayout(content)
        grid.setSpacing(12)
        grid.setContentsMargins(8, 8, 8, 8)
        self.library_grid = grid

        scroll.setWidget(content)
        content_layout.addWidget(scroll)
        layout.addWidget(content_frame)
        return page

    def _build_server_page(self) -> QWidget:
        page = QWidget()
        layout = QHBoxLayout(page)
        layout.setSpacing(12)

        platforms_frame = QFrame()
        platforms_frame.setObjectName("panel")
        platforms_layout = QVBoxLayout(platforms_frame)
        platforms_layout.setContentsMargins(10, 10, 10, 10)

        platforms = QListWidget()
        platforms.setObjectName("serverPlatformsList")
        platforms.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Expanding)
        platforms.setMinimumWidth(120)
        platforms.setMaximumWidth(220)
        platforms.currentTextChanged.connect(self._on_server_platform_selected)
        self.server_platforms_list = platforms
        platforms_layout.addWidget(platforms)
        layout.addWidget(platforms_frame, 0)

        right = QFrame()
        right.setObjectName("panel")
        right_layout = QVBoxLayout(right)
        right_layout.setContentsMargins(12, 10, 12, 10)
        right_layout.setSpacing(10)

        header_row = QHBoxLayout()
        header_row.setContentsMargins(0, 0, 0, 0)
        header_row.setSpacing(8)

        header = QLabel("Server Games")
        header.setStyleSheet("font-size: 20px; font-weight: 700;")
        header_row.addWidget(header)
        header_row.addStretch()

        search_container = QFrame()
        search_container.setObjectName("serverSearchContainer")
        search_container.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        search_container.setFixedWidth(260)
        search_layout = QHBoxLayout(search_container)
        search_layout.setContentsMargins(0, 0, 0, 0)
        search_layout.setSpacing(0)

        server_search_input = QLineEdit()
        server_search_input.setObjectName("serverSearchInput")
        server_search_input.setPlaceholderText("Search games...")
        server_search_input.setClearButtonEnabled(False)
        server_search_input.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        server_search_input.textChanged.connect(self._on_server_search_changed)
        self.server_search_input = server_search_input
        search_layout.addWidget(server_search_input)

        server_search_clear_button = QPushButton("X")
        server_search_clear_button.setObjectName("serverSearchClearButton")
        server_search_clear_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        server_search_clear_button.setCursor(Qt.CursorShape.PointingHandCursor)
        server_search_clear_button.setFixedWidth(26)
        server_search_clear_button.setFixedHeight(28)
        server_search_clear_button.setVisible(False)
        server_search_clear_button.clicked.connect(self._clear_server_search)
        self.server_search_clear_button = server_search_clear_button
        search_layout.addWidget(server_search_clear_button)

        header_row.addWidget(search_container)

        right_layout.addLayout(header_row)

        self.server_status_label = QLabel("Not connected")
        self.server_status_label.setStyleSheet("color: #ff5555;")
        right_layout.addWidget(self.server_status_label)

        scroll = QScrollArea()
        scroll.setObjectName("serverGamesScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.viewport().setObjectName("serverGamesScrollViewport")
        self.server_games_scroll = scroll

        content = QWidget()
        content.setObjectName("serverGamesContent")
        grid = QGridLayout(content)
        grid.setSpacing(12)
        grid.setContentsMargins(8, 8, 8, 8)
        self.server_games_grid = grid
        self.server_games_content = content

        scroll.setWidget(content)
        right_layout.addWidget(scroll)

        layout.addWidget(right, 1)
        return page

    def _build_downloads_page(self) -> QWidget:
        page, empty_label, scroll, list_layout = build_downloads_page()
        self.downloads_empty_label = empty_label
        self.downloads_scroll = scroll
        self.downloads_list_layout = list_layout
        self._refresh_downloads_page()
        return page

    def _build_emulators_page(self) -> QWidget:
        page = QWidget()
        outer_layout = QVBoxLayout(page)
        outer_layout.setContentsMargins(0, 0, 0, 0)

        scroll = QScrollArea()
        scroll.setObjectName("emulatorsScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.viewport().setObjectName("emulatorsScrollViewport")
        outer_layout.addWidget(scroll)

        content = QWidget()
        content.setObjectName("emulatorsContent")
        layout = QHBoxLayout(content)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(12)

        def _lock_emulator_field_height(widget: QLineEdit | QComboBox, height: int = 34) -> None:
            widget.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
            widget.setMinimumHeight(height)
            widget.setMaximumHeight(height)

        left_panel = QFrame()
        left_panel.setObjectName("panel")
        left_layout = QVBoxLayout(left_panel)
        left_layout.setContentsMargins(12, 10, 12, 10)
        left_layout.addWidget(self._make_section_title("Installed Emulators"))

        emulator_list = QListWidget()
        emulator_list.setObjectName("installedEmulatorList")
        emulator_list.setSelectionMode(QAbstractItemView.SelectionMode.NoSelection)
        emulator_list.setAlternatingRowColors(True)
        self.emulator_list = emulator_list
        left_layout.addWidget(emulator_list)

        add_emulator_button = QPushButton("Add New Emulator")
        add_emulator_button.clicked.connect(lambda: MainWindow._open_add_emulator_dialog(self))
        left_layout.addWidget(add_emulator_button)

        download_source_button = QPushButton("Download Supported Emulator")
        download_source_button.clicked.connect(lambda: MainWindow._open_source_emulator_download_dialog(self))
        left_layout.addWidget(download_source_button)

        layout.addWidget(left_panel, 1)

        def _build_hidden_emulator_form_state() -> None:
            # Keep legacy form widgets available for _save_emulator() and tests.
            self.emulator_name_input = QLineEdit()
            _lock_emulator_field_height(self.emulator_name_input)
            self.emulator_path_input = QLineEdit()
            _lock_emulator_field_height(self.emulator_path_input)
            self.emulator_args_input = QLineEdit("%rom%")
            _lock_emulator_field_height(self.emulator_args_input)
            self.emulator_save_strategy_input = QComboBox()
            _lock_emulator_field_height(self.emulator_save_strategy_input)
            self.emulator_save_strategy_input.addItems(["auto", "single_file", "folder"])
            self.emulator_ignore_files_input = QLineEdit()
            _lock_emulator_field_height(self.emulator_ignore_files_input)
            self.emulator_ignore_extensions_input = QLineEdit()
            _lock_emulator_field_height(self.emulator_ignore_extensions_input)
            self.emulator_save_paths_input = QLineEdit()
            _lock_emulator_field_height(self.emulator_save_paths_input)
            self.emulator_state_paths_input = QLineEdit()
            _lock_emulator_field_height(self.emulator_state_paths_input)
            self.save_emulator_button = QPushButton(save_button_label(self._emulators(), -1))

        _build_hidden_emulator_form_state()

        defaults_panel = QFrame()
        defaults_panel.setObjectName("panel")
        defaults_layout = QVBoxLayout(defaults_panel)
        defaults_layout.setContentsMargins(12, 10, 12, 10)
        defaults_layout.addWidget(self._make_section_title("Default Emulator by Platform"))

        default_form = QFormLayout()
        self.default_platform_combo = QComboBox()
        _lock_emulator_field_height(self.default_platform_combo)
        self.default_emulator_combo = QComboBox()
        _lock_emulator_field_height(self.default_emulator_combo)
        self.default_core_combo = QComboBox()
        _lock_emulator_field_height(self.default_core_combo)
        self.default_emulator_combo.setMaximumWidth(220)
        self.default_core_combo.setMaximumWidth(220)
        self.default_platform_combo.currentTextChanged.connect(self._on_default_platform_changed)
        self.default_emulator_combo.currentTextChanged.connect(self._refresh_retroarch_core_options)
        default_form.addRow("Platform", self.default_platform_combo)
        default_form.addRow("Emulator", self.default_emulator_combo)
        default_form.addRow("Core", self.default_core_combo)
        defaults_layout.addLayout(default_form)

        set_default_button = QPushButton("Set Default")
        set_default_button.clicked.connect(self._set_default_emulator)
        defaults_layout.addWidget(set_default_button)

        self.default_mapping_list = QListWidget()
        self.default_mapping_list.setObjectName("defaultMappingList")
        self.default_mapping_list.setAlternatingRowColors(True)
        defaults_layout.addWidget(self.default_mapping_list, 1)

        layout.addWidget(defaults_panel, 2)

        scroll.setWidget(content)
        return page

    def _build_settings_page(self) -> QWidget:
        page = QWidget()
        outer_layout = QVBoxLayout(page)
        outer_layout.setContentsMargins(0, 0, 0, 0)

        scroll = QScrollArea()
        scroll.setObjectName("settingsScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.viewport().setObjectName("settingsScrollViewport")
        outer_layout.addWidget(scroll)

        content = QWidget()
        content.setObjectName("settingsContent")
        layout = QVBoxLayout(content)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(12)

        def _lock_settings_field_height(widget: QLineEdit | QComboBox, height: int = 34) -> None:
            widget.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
            widget.setMinimumHeight(height)
            widget.setMaximumHeight(height)

        header = QLabel("Settings")
        header.setStyleSheet("font-size: 20px; font-weight: 700;")
        layout.addWidget(header)

        server_panel = QFrame()
        server_panel.setObjectName("panel")
        server_layout = QVBoxLayout(server_panel)
        server_layout.setContentsMargins(12, 10, 12, 10)
        server_layout.addWidget(self._make_section_title("Server Connection"))

        server_form = QFormLayout()
        self.server_url_input = QLineEdit(self.config["server_url"])
        _lock_settings_field_height(self.server_url_input)
        self.api_token_input = QLineEdit(self.config["api_token"])
        _lock_settings_field_height(self.api_token_input)
        self.api_token_input.setEchoMode(QLineEdit.EchoMode.Password)
        server_form.addRow("Server URL", self.server_url_input)
        server_form.addRow("API Token", self.api_token_input)
        server_layout.addLayout(server_form)

        connect_button = QPushButton("Connect")
        connect_button.clicked.connect(self._connect_from_settings)
        connect_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)

        disconnect_button = QPushButton("Disconnect")
        disconnect_button.clicked.connect(self._disconnect_from_server)
        disconnect_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)

        connection_actions = QHBoxLayout()
        connection_actions.addWidget(connect_button)
        connection_actions.addWidget(disconnect_button)
        connection_actions.addStretch()
        server_layout.addLayout(connection_actions)
        layout.addWidget(server_panel)

        ra_panel = QFrame()
        ra_panel.setObjectName("panel")
        ra_panel_layout = QVBoxLayout(ra_panel)
        ra_panel_layout.setContentsMargins(12, 10, 12, 10)
        ra_panel_layout.addWidget(self._make_section_title("RetroAchievements"))

        ra_form = QFormLayout()
        self.ra_username_input = QLineEdit(self.config.get("retroachievements_username", ""))
        _lock_settings_field_height(self.ra_username_input)
        ra_form.addRow("Username", self.ra_username_input)

        self.ra_api_key_input = QLineEdit(self.config.get("retroachievements_api_key", ""))
        _lock_settings_field_height(self.ra_api_key_input)
        self.ra_api_key_input.setEchoMode(QLineEdit.EchoMode.Password)
        ra_form.addRow("Web API Key", self.ra_api_key_input)

        self.ra_password_input = QLineEdit()
        _lock_settings_field_height(self.ra_password_input)
        self.ra_password_input.setEchoMode(QLineEdit.EchoMode.Password)
        ra_form.addRow("Password", self.ra_password_input)

        ra_panel_layout.addLayout(ra_form)

        ra_button_row = QHBoxLayout()
        self.ra_login_button = QPushButton("Log In")
        self.ra_login_button.setFixedWidth(100)
        self.ra_login_button.clicked.connect(self._ra_login_clicked)
        ra_button_row.addWidget(self.ra_login_button)

        self.ra_clear_button = QPushButton("Clear Login")
        self.ra_clear_button.setFixedWidth(100)
        self.ra_clear_button.clicked.connect(self._ra_clear_credentials)
        ra_button_row.addWidget(self.ra_clear_button)

        ra_button_row.addStretch()
        ra_panel_layout.addLayout(ra_button_row)

        self.ra_login_status_label = QLabel()
        ra_token = self.config.get("retroachievements_token", "")
        ra_user = self.config.get("retroachievements_username", "")
        if ra_token and ra_user:
            self.ra_login_status_label.setText(f"Logged in as {ra_user}")
        else:
            self.ra_login_status_label.setText("Not logged in")
        ra_panel_layout.addWidget(self.ra_login_status_label)

        layout.addWidget(ra_panel)

        paths_panel = QFrame()
        paths_panel.setObjectName("panel")
        paths_layout = QVBoxLayout(paths_panel)
        paths_layout.setContentsMargins(12, 10, 12, 10)
        paths_layout.addWidget(self._make_section_title("Library Paths"))

        paths_form = QFormLayout()
        self.library_path_input = QLineEdit(self.config["library_path"])
        _lock_settings_field_height(self.library_path_input)

        library_path_row = QWidget()
        library_path_row_layout = QHBoxLayout(library_path_row)
        library_path_row_layout.setContentsMargins(0, 0, 0, 0)
        library_path_row_layout.setSpacing(8)
        library_path_row_layout.addWidget(self.library_path_input)

        browse_library_path_button = QPushButton("Browse...")
        browse_library_path_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        browse_library_path_button.clicked.connect(self._browse_library_path)
        library_path_row_layout.addWidget(browse_library_path_button)

        paths_form.addRow("Library Path", library_path_row)
        paths_layout.addLayout(paths_form)
        layout.addWidget(paths_panel)

        appearance_panel = QFrame()
        appearance_panel.setObjectName("panel")
        appearance_layout = QVBoxLayout(appearance_panel)
        appearance_layout.setContentsMargins(12, 10, 12, 10)
        appearance_layout.addWidget(self._make_section_title("Appearance"))

        appearance_form = QFormLayout()
        self.theme_input = QComboBox()
        _lock_settings_field_height(self.theme_input)
        self.theme_input.addItems(["system", "dark", "light"])
        self.theme_input.setCurrentText(self._normalized_theme_choice(self.config.get("theme", "system")))
        self.theme_input.currentTextChanged.connect(self._on_theme_selection_changed)
        appearance_form.addRow("Theme", self.theme_input)

        self.debug_prints_checkbox = QCheckBox("Enable debug prints")
        self.debug_prints_checkbox.setChecked(self._debug_prints_enabled())
        appearance_form.addRow("Debug", self.debug_prints_checkbox)

        appearance_layout.addLayout(appearance_form)
        layout.addWidget(appearance_panel)

        cloud_sync_panel = QFrame()
        cloud_sync_panel.setObjectName("panel")
        cloud_sync_layout = QVBoxLayout(cloud_sync_panel)
        cloud_sync_layout.setContentsMargins(12, 10, 12, 10)
        cloud_sync_layout.addWidget(self._make_section_title("Cloud Save Sync"))

        self.auto_cloud_download_checkbox = QCheckBox("Download latest cloud save before launch")
        self.auto_cloud_download_checkbox.setChecked(self._auto_cloud_save_download_enabled())
        cloud_sync_layout.addWidget(self.auto_cloud_download_checkbox)

        self.auto_cloud_upload_checkbox = QCheckBox("Upload saves automatically when game closes")
        self.auto_cloud_upload_checkbox.setChecked(self._auto_cloud_save_upload_enabled())
        cloud_sync_layout.addWidget(self.auto_cloud_upload_checkbox)

        self.auto_cloud_skip_local_newer_checkbox = QCheckBox("Skip download if local save appears newer")
        self.auto_cloud_skip_local_newer_checkbox.setChecked(self._auto_cloud_skip_download_if_local_newer())
        cloud_sync_layout.addWidget(self.auto_cloud_skip_local_newer_checkbox)

        cloud_hint = QLabel("Auto-sync applies to emulator-based games and uses the latest server save record only.")
        cloud_hint.setWordWrap(True)
        cloud_hint.setStyleSheet(f"color: {self._theme_color('muted', '#6272a4')};")
        cloud_sync_layout.addWidget(cloud_hint)
        layout.addWidget(cloud_sync_panel)

        controls_row = QHBoxLayout()
        save_button = QPushButton("Save Settings")
        save_button.clicked.connect(self._save_settings)
        controls_row.addWidget(save_button)

        open_folder_button = QPushButton("Open Config Folder")
        open_folder_button.clicked.connect(self._open_config_folder)
        controls_row.addWidget(open_folder_button)

        self.settings_status_label = QLabel("Loaded settings from ~/.rom-mate/config.json")
        controls_row.addWidget(self.settings_status_label)
        controls_row.addStretch()
        layout.addLayout(controls_row)

        layout.addStretch()
        scroll.setWidget(content)
        return page

    def _build_game_details_page(self) -> QWidget:
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.setSpacing(12)

        content = QFrame()
        content.setObjectName("panel")
        self.details_content_frame = content
        content_layout = QHBoxLayout(content)
        content_layout.setContentsMargins(12, 16, 12, 16)
        content_layout.setSpacing(20)

        cover_col = QVBoxLayout()
        cover_col.setSpacing(10)
        cover_col.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)

        button_bar = QHBoxLayout()
        button_bar.setContentsMargins(0, 0, 0, 0)
        button_bar.setSpacing(8)
        button_bar.setAlignment(Qt.AlignmentFlag.AlignLeft)

        back_button = QPushButton("Back")
        back_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        back_button.clicked.connect(self._return_from_details)
        button_bar.addWidget(back_button)

        details_button = QPushButton("Details")
        details_button.setCheckable(True)
        details_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        details_button.clicked.connect(self._perform_show_details_action)
        self.details_details_button = details_button
        button_bar.addWidget(details_button)

        self.details_achievements_button = QPushButton("Achievements")
        self.details_achievements_button.setCheckable(True)
        self.details_achievements_button.setObjectName("detailsAchievementsButton")
        self.details_achievements_button.clicked.connect(self._open_achievements_panel)
        self.details_achievements_button.setVisible(False)
        button_bar.addWidget(self.details_achievements_button)

        manage_saves_button = QPushButton("Manage Saves")
        manage_saves_button.setCheckable(True)
        manage_saves_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        manage_saves_button.clicked.connect(self._perform_manage_saves_action)
        self.details_manage_saves_button = manage_saves_button
        button_bar.addWidget(manage_saves_button)

        manage_states_button = QPushButton("Manage States")
        manage_states_button.setCheckable(True)
        manage_states_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        manage_states_button.clicked.connect(self._perform_manage_states_action)
        self.details_manage_states_button = manage_states_button
        button_bar.addWidget(manage_states_button)
        button_bar.addStretch()
        cover_col.addLayout(button_bar)

        cover = QLabel("Cover Art")
        cover.setAlignment(Qt.AlignmentFlag.AlignCenter)
        cover.setMinimumSize(260, 340)
        cover.setMaximumSize(860, 1120)
        cover.setStyleSheet(
            "background-color: transparent; border: none; border-radius: 8px; font-size: 20px;"
        )
        self.details_cover_label = cover
        cover_col.addWidget(cover, alignment=Qt.AlignmentFlag.AlignHCenter)
        cover_col.addStretch()
        content_layout.addLayout(cover_col, 2)

        details_stack = QStackedWidget()
        self.details_center_stack = details_stack

        overview_page = QWidget()
        details_col = QVBoxLayout(overview_page)
        details_col.setSpacing(10)
        details_col.setContentsMargins(0, 0, 0, 0)
        details_col.setAlignment(Qt.AlignmentFlag.AlignTop)

        action_row = QHBoxLayout()
        action_row.setSpacing(8)
        action_row.setAlignment(Qt.AlignmentFlag.AlignLeft)

        primary = QPushButton("Launch Game")
        primary.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        primary.clicked.connect(self._perform_game_action)
        self.details_primary_button = primary
        action_row.addWidget(primary)

        config_button = QPushButton("Config")
        config_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        config_button.clicked.connect(self._perform_game_config_action)
        self.details_config_button = config_button
        action_row.addWidget(config_button)

        ps4_content_button = QPushButton("Install Update/DLC")
        ps4_content_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        ps4_content_button.clicked.connect(self._perform_ps4_content_action)
        self.details_ps4_content_button = ps4_content_button
        action_row.addWidget(ps4_content_button)

        xbox360_content_button = QPushButton("Install Update/DLC")
        xbox360_content_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        xbox360_content_button.clicked.connect(self._perform_xbox360_content_action)
        self.details_xbox360_content_button = xbox360_content_button
        action_row.addWidget(xbox360_content_button)

        secondary = QPushButton("Uninstall")
        secondary.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        secondary.clicked.connect(self._perform_game_secondary_action)
        self.details_secondary_button = secondary
        action_row.addWidget(secondary)

        update_button = QPushButton("Update")
        update_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        update_handler = getattr(self, "_perform_game_update_action", None)
        if callable(update_handler):
            update_button.clicked.connect(update_handler)
        self.details_update_button = update_button
        action_row.addWidget(update_button)
        action_row.addStretch()
        details_col.addLayout(action_row)

        overview_scroll = QScrollArea()
        overview_scroll.setObjectName("detailsOverviewScroll")
        overview_scroll.setWidgetResizable(True)
        overview_scroll.setFrameShape(QFrame.Shape.NoFrame)
        overview_scroll.viewport().setObjectName("detailsOverviewScrollViewport")
        overview_scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        overview_scroll.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)

        overview_content = QWidget()
        overview_content.setObjectName("detailsOverviewContent")
        overview_content_layout = QVBoxLayout(overview_content)
        overview_content_layout.setContentsMargins(0, 0, 0, 0)
        overview_content_layout.setSpacing(10)
        overview_content_layout.setAlignment(Qt.AlignmentFlag.AlignTop)

        title = QLabel("Game Title")
        title.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter)
        title.setStyleSheet("font-size: 30px; font-weight: 700;")
        self.details_title_label = title
        overview_content_layout.addWidget(title)

        platform = QLabel("Platform: -")
        platform.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter)
        platform.setStyleSheet("font-size: 18px;")
        self.details_platform_label = platform
        overview_content_layout.addWidget(platform)

        version = QLabel("")
        version.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter)
        version.setStyleSheet(f"font-size: 16px; color: {self._theme_color('muted', '#6272a4')};")
        version.setVisible(False)
        self.details_version_label = version
        overview_content_layout.addWidget(version)

        rating = QLabel("Rating: -")
        rating.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter)
        rating.setStyleSheet("font-size: 18px;")
        self.details_rating_label = rating
        overview_content_layout.addWidget(rating)

        description = QLabel("Description")
        description.setWordWrap(True)
        description.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)
        description.setMinimumWidth(0)
        description.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
        description.setStyleSheet("font-size: 17px;")
        self.details_description_label = description
        overview_content_layout.addWidget(description)
        overview_content_layout.addStretch()

        overview_scroll.setWidget(overview_content)
        details_col.addWidget(overview_scroll, 1)
        details_stack.addWidget(overview_page)

        cloud_page = QFrame()
        cloud_page.setObjectName("panel")
        cloud_layout = QVBoxLayout(cloud_page)
        cloud_layout.setSpacing(10)
        cloud_layout.setContentsMargins(12, 12, 12, 12)

        cloud_header = QHBoxLayout()
        cloud_header.setContentsMargins(0, 0, 0, 0)
        cloud_header.setSpacing(8)

        cloud_title = QLabel("Manage Saves")
        cloud_title.setStyleSheet("font-size: 22px; font-weight: 700;")
        self.details_cloud_title_label = cloud_title
        cloud_header.addWidget(cloud_title)
        cloud_header.addStretch()

        cloud_upload_button = QPushButton("Upload Latest Save")
        cloud_upload_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        cloud_upload_button.clicked.connect(self._perform_current_cloud_upload_action)
        self.details_cloud_upload_button = cloud_upload_button
        cloud_header.addWidget(cloud_upload_button)

        cloud_layout.addLayout(cloud_header)

        cloud_status = QLabel("View and manage the cloud saves for this game.")
        cloud_status.setWordWrap(True)
        cloud_status.setStyleSheet(f"color: {self._theme_color('muted', '#6272a4')};")
        self.details_cloud_status_label = cloud_status
        cloud_layout.addWidget(cloud_status)

        cloud_scroll = QScrollArea()
        cloud_scroll.setObjectName("detailsCloudScroll")
        cloud_scroll.setWidgetResizable(True)
        cloud_scroll.setFrameShape(QFrame.Shape.NoFrame)
        cloud_scroll.viewport().setObjectName("detailsCloudScrollViewport")

        cloud_content = QFrame()
        cloud_content.setObjectName("detailsCloudListPanel")
        cloud_content.setFrameShape(QFrame.Shape.StyledPanel)
        cloud_content.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
        cloud_list_layout = QVBoxLayout(cloud_content)
        cloud_list_layout.setContentsMargins(10, 10, 10, 10)
        cloud_list_layout.setSpacing(10)
        self.details_cloud_list_layout = cloud_list_layout

        cloud_empty = QLabel("No cloud saves were found for this game yet.")
        cloud_empty.setWordWrap(True)
        cloud_empty.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignTop)
        cloud_empty.setStyleSheet(f"color: {self._theme_color('muted', '#6272a4')}; font-size: 15px;")
        self.details_cloud_empty_label = cloud_empty
        cloud_list_layout.addWidget(cloud_empty)
        cloud_list_layout.addStretch()

        cloud_scroll.setWidget(cloud_content)
        cloud_layout.addWidget(cloud_scroll, 1)
        details_stack.addWidget(cloud_page)
        content_layout.addWidget(details_stack, 4)

        screenshots_panel = QWidget()
        self.details_screenshots_panel = screenshots_panel
        screenshots_col = QVBoxLayout(screenshots_panel)
        screenshots_col.setContentsMargins(0, 0, 0, 0)
        screenshots_col.setSpacing(8)

        screenshots_title = QLabel("Screenshots")
        screenshots_title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        screenshots_title.setStyleSheet("font-size: 18px; font-weight: 600;")
        screenshots_col.addWidget(screenshots_title)

        screenshots_scroll = QScrollArea()
        screenshots_scroll.setWidgetResizable(True)
        screenshots_scroll.setMinimumWidth(230)
        screenshots_scroll.setMaximumWidth(600)
        screenshots_scroll.setStyleSheet("background-color: transparent; border: none;")
        self.details_screenshots_scroll = screenshots_scroll

        screenshots_content = QWidget()
        screenshots_content_layout = QVBoxLayout(screenshots_content)
        screenshots_content_layout.setContentsMargins(0, 0, 0, 0)
        screenshots_content_layout.setSpacing(10)

        self.details_screenshot_labels = []
        for index in range(5):
            screenshot_label = QLabel(f"Screenshot {index + 1}")
            screenshot_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
            screenshot_label.setMinimumSize(210, 118)
            screenshot_label.setMaximumSize(520, 320)
            screenshot_label.setStyleSheet(
                f"background-color: {self._theme_color('window', '#282a36')}; border: none; border-radius: 8px;"
            )
            self.details_screenshot_labels.append(screenshot_label)
            screenshots_content_layout.addWidget(
                screenshot_label,
                alignment=Qt.AlignmentFlag.AlignHCenter,
            )
        screenshots_content_layout.addStretch()

        screenshots_scroll.setWidget(screenshots_content)
        screenshots_col.addWidget(
            screenshots_scroll,
            alignment=Qt.AlignmentFlag.AlignHCenter,
        )
        content_layout.addWidget(screenshots_panel, 2)

        layout.addWidget(content)
        self._show_details_overview()
        self._update_details_layout_metrics()
        return page

    def _make_section_title(self, title: str) -> QLabel:
        label = QLabel(title)
        label.setStyleSheet("font-size: 15px; font-weight: 600;")
        return label

    def _normalized_theme_choice(self, value: Any) -> str:
        return resolve_normalized_theme_choice(value)

    def _resolved_theme_variant(self, theme_choice: str) -> str:
        return resolve_resolved_theme_variant(theme_choice, QApplication.instance())

    def _debug_prints_enabled(self) -> bool:
        value = self.config.get("debug_prints", True)
        if isinstance(value, bool):
            return value
        if isinstance(value, str):
            normalized = value.strip().casefold()
            if normalized in {"1", "true", "yes", "on"}:
                return True
            if normalized in {"0", "false", "no", "off"}:
                return False
        return bool(value)

    def _debug_timing_start(self, label: str, **details: Any) -> float:
        started_at = time.perf_counter()
        if self._debug_prints_enabled():
            detail_parts = [
                f"{key}={value}"
                for key, value in details.items()
                if value is not None and value != ""
            ]
            suffix = f" {' '.join(detail_parts)}" if detail_parts else ""
            print(f"[DEBUG][Timing] enter {label}{suffix}")
        return started_at

    def _debug_timing_end(self, label: str, started_at: float, **details: Any) -> None:
        if not self._debug_prints_enabled():
            return
        elapsed_ms = max(0.0, (time.perf_counter() - started_at) * 1000.0)
        detail_parts = [
            f"{key}={value}"
            for key, value in details.items()
            if value is not None and value != ""
        ]
        suffix = f" {' '.join(detail_parts)}" if detail_parts else ""
        print(f"[DEBUG][Timing] exit {label} elapsed_ms={elapsed_ms:.1f}{suffix}")

    def _config_bool(self, key: str, default: bool) -> bool:
        value = self.config.get(key, default)
        if isinstance(value, bool):
            return value
        if isinstance(value, str):
            normalized = value.strip().casefold()
            if normalized in {"1", "true", "yes", "on"}:
                return True
            if normalized in {"0", "false", "no", "off"}:
                return False
        return default

    def _config_int(self, key: str, default: int) -> int:
        value = self.config.get(key, default)
        if isinstance(value, bool):
            return default
        if isinstance(value, int):
            return value
        if isinstance(value, str):
            try:
                return int(value.strip())
            except ValueError:
                return default
        return default

    def _auto_cloud_save_download_enabled(self) -> bool:
        return self._config_bool("auto_cloud_save_download_on_launch", True)

    def _auto_cloud_save_upload_enabled(self) -> bool:
        return self._config_bool("auto_cloud_save_upload_on_exit", True)

    def _auto_cloud_skip_download_if_local_newer(self) -> bool:
        return self._config_bool("auto_cloud_save_skip_download_if_local_newer", True)

    def _auto_cloud_upload_delay_seconds(self) -> int:
        return max(0, min(self._config_int("auto_cloud_save_upload_delay_seconds", 3), 60))

    def _cloud_save_retention_limit(self) -> int:
        return 3

    def _theme_colors(self, theme_variant: str) -> dict[str, str]:
        return resolve_theme_colors(theme_variant)

    def _theme_color(self, key: str, fallback: str) -> str:
        return resolve_theme_color(self.active_theme_colors, key, fallback)

    def _theme_stylesheet(self) -> str:
        return resolve_theme_stylesheet(self.active_theme_colors)

    def _themed_svg_icon(self, asset_name: str, color: str, *, size: QSize | tuple[int, int] | None = None) -> QIcon:
        return resolve_themed_svg_icon(asset_name, color, size=size)

    def _refresh_installed_emulator_action_icons(self) -> None:
        if self.emulator_list is None:
            return

        action_icon_size = QSize(16, 16)
        launch_icon = self._themed_svg_icon(
            "play-1003-svgrepo-com.svg",
            self._theme_color("accent", "#8be9fd"),
            size=action_icon_size,
        )
        config_icon = self._themed_svg_icon(
            "gear-tools-wrench-svgrepo-com.svg",
            self._theme_color("text", "#f8f8f2"),
            size=action_icon_size,
        )
        uninstall_icon = self._themed_svg_icon(
            "trashcan-svgrepo-com.svg",
            self._theme_color("error", "#ff5555"),
            size=action_icon_size,
        )
        source_update_icon = self._themed_svg_icon(
            "save-floppy-svgrepo-com.svg",
            self._theme_color("accent", "#8be9fd"),
            size=action_icon_size,
        )

        for row_index in range(self.emulator_list.count()):
            item = self.emulator_list.item(row_index)
            if item is None:
                continue
            row_widget = self.emulator_list.itemWidget(item)
            if row_widget is None:
                continue

            launch_button = row_widget.findChild(QPushButton, "installedEmulatorLaunchButton")
            if launch_button is not None:
                launch_button.setIcon(launch_icon)
                launch_button.setIconSize(action_icon_size)

            config_button = row_widget.findChild(QPushButton, "installedEmulatorConfigButton")
            if config_button is not None:
                config_button.setIcon(config_icon)
                config_button.setIconSize(action_icon_size)

            uninstall_button = row_widget.findChild(QPushButton, "installedEmulatorUninstallButton")
            if uninstall_button is not None:
                uninstall_button.setIcon(uninstall_icon)
                uninstall_button.setIconSize(action_icon_size)

            source_update_button = row_widget.findChild(QPushButton, "installedEmulatorSourceUpdateButton")
            if source_update_button is not None:
                source_update_button.setIcon(source_update_icon)
                source_update_button.setIconSize(action_icon_size)

    def _apply_theme_inline_styles(self) -> None:
        resolve_apply_theme_inline_styles(
            self.active_theme_colors,
            download_count_label=self.download_count_label,
            download_speed_label=self.download_speed_label,
            account_status_label=self.account_status_label,
            library_empty_label=self.library_empty_label,
            downloads_empty_label=self.downloads_empty_label,
            details_cover_label=self.details_cover_label,
            details_cloud_status_label=self.details_cloud_status_label,
            details_cloud_empty_label=self.details_cloud_empty_label,
            screenshot_labels=self.details_screenshot_labels,
        )

    def _apply_theme(self, theme_choice: str) -> None:
        normalized = self._normalized_theme_choice(theme_choice)
        self.active_theme_choice = normalized
        self.active_theme_variant = self._resolved_theme_variant(normalized)
        self.active_theme_colors = self._theme_colors(self.active_theme_variant)
        self.setStyleSheet(self._theme_stylesheet())
        self._apply_theme_inline_styles()
        self._refresh_installed_emulator_action_icons()
        if self.current_details_cloud_mode in {"save", "state"}:
            self._refresh_details_cloud_panel()

    def _on_theme_selection_changed(self, selected_theme: str) -> None:
        normalized = self._normalized_theme_choice(selected_theme)
        self.config["theme"] = normalized
        self._apply_theme(normalized)
        self._refresh_library_grid()
        self._refresh_downloads_page()
        saved = self._save_config(self.config)
        if saved and self.settings_status_label is not None:
            self.settings_status_label.setText("Theme saved")

    def _config_defaults(self) -> dict[str, Any]:
        return {
            "server_url": "",
            "api_token": "",
            "username": "",
            "library_path": "",
            "first_run_completed": False,
            "launch_args": "",
            "debug_prints": True,
            "theme": "system",
            "window_geometry": "",
            "window_state": "normal",
            "emulators": [],
            "default_emulators": {},
            "default_retroarch_cores": {},
            "installed_games": [],
            "emulator_source_installs": {},
            "auto_cloud_save_download_on_launch": True,
            "auto_cloud_save_upload_on_exit": True,
            "auto_cloud_save_skip_download_if_local_newer": True,
            "auto_cloud_save_upload_delay_seconds": 3,
            "cloud_sync_state": {},
            "retroachievements_username": "",
            "retroachievements_api_key": "",
            "retroachievements_token": "",
        }

    def _persist_window_geometry(self) -> None:
        try:
            geometry_payload = base64.b64encode(bytes(self.saveGeometry())).decode("ascii")
        except (RuntimeError, ValueError):
            return

        self.config["window_geometry"] = geometry_payload
        self.config["window_state"] = "maximized" if self.isMaximized() else "normal"
        self._save_config(self.config)

    def _restore_window_geometry(self) -> None:
        geometry_value = self.config.get("window_geometry", "")
        if isinstance(geometry_value, str) and geometry_value.strip():
            try:
                geometry_bytes = base64.b64decode(geometry_value.encode("ascii"), validate=True)
            except (ValueError, UnicodeEncodeError):
                geometry_bytes = b""
            if geometry_bytes:
                self.restoreGeometry(geometry_bytes)

        window_state = self.config.get("window_state", "normal")
        if isinstance(window_state, str) and window_state.strip().casefold() == "maximized":
            self.setWindowState(self.windowState() | Qt.WindowState.WindowMaximized)

    def _config_dir(self) -> Path:
        return Path.home() / ".rom-mate"

    def _image_cache_dir(self) -> Path:
        return self._config_dir() / "imagecache"

    def _config_file(self) -> Path:
        return self._config_dir() / "config.json"

    def _token_file(self) -> Path:
        return self._config_dir() / "token.bin"

    def _ra_token_file(self) -> Path:
        return self._config_dir() / "ra_token.bin"

    def _windows_protect_data(self, raw: bytes) -> bytes:
        return resolve_windows_protect_data(raw)

    def _windows_unprotect_data(self, protected: bytes) -> bytes:
        return resolve_windows_unprotect_data(protected)

    def _load_api_token(self) -> str:
        return resolve_load_api_token(self._token_file())

    def _load_ra_token(self) -> str:
        return resolve_load_ra_token(self._ra_token_file())

    def _save_api_token(self, token: str) -> bool:
        return resolve_save_api_token(self._config_dir(), self._token_file(), token)

    def _save_ra_token(self, token: str) -> bool:
        return resolve_save_ra_token(self._config_dir(), self._ra_token_file(), token)

    def _set_api_token(self, token: str) -> bool:
        return resolve_set_api_token(self.config, token, save_token=self._save_api_token)

    def _load_config(self) -> dict[str, Any]:
        defaults = self._config_defaults()
        config_path = self._config_file()

        if not config_path.exists():
            return defaults

        try:
            content = json.loads(config_path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            return defaults

        merged = resolve_merge_config_with_defaults(
            defaults,
            content,
            normalize_emulators=self._normalize_emulators,
            normalize_default_emulators=self._normalize_default_emulators,
            normalize_default_retroarch_cores=self._normalize_default_retroarch_cores,
            normalize_installed_games=self._normalize_installed_games,
            normalize_cloud_sync_state=self._normalize_cloud_sync_state,
        )
        merged["emulator_source_installs"] = self._normalize_emulator_source_installs(
            content.get("emulator_source_installs", {})
        )

        stored_token = self._load_api_token()
        if stored_token:
            merged["api_token"] = stored_token
        else:
            legacy_token = merged.get("api_token", "")
            if isinstance(legacy_token, str) and legacy_token.strip() and self._save_api_token(legacy_token):
                merged["api_token"] = legacy_token.strip()
                migrated = merged.copy()
                migrated["api_token"] = ""
                self._save_config(migrated)

        ra_token = self._load_ra_token()
        if ra_token:
            merged["retroachievements_token"] = ra_token

        return merged

    def _collect_settings(self) -> dict[str, Any]:
        values = self._config_defaults()
        if self.server_url_input is not None:
            values["server_url"] = self.server_url_input.text().strip()
        if self.api_token_input is not None:
            values["api_token"] = self.api_token_input.text().strip()
        if self.ra_username_input is not None:
            values["retroachievements_username"] = self.ra_username_input.text().strip()
        if self.ra_api_key_input is not None:
            values["retroachievements_api_key"] = self.ra_api_key_input.text().strip()
        ra_token = self.config.get("retroachievements_token", "")
        if isinstance(ra_token, str) and ra_token.strip():
            values["retroachievements_token"] = ra_token.strip()
        existing_username = self.config.get("username", "")
        if isinstance(existing_username, str):
            values["username"] = existing_username.strip()
        if self.library_path_input is not None:
            values["library_path"] = self.library_path_input.text().strip()
        existing_launch_args = self.config.get("launch_args", "")
        if isinstance(existing_launch_args, str):
            values["launch_args"] = existing_launch_args.strip()
        if self.debug_prints_checkbox is not None:
            values["debug_prints"] = self.debug_prints_checkbox.isChecked()
        if self.auto_cloud_download_checkbox is not None:
            values["auto_cloud_save_download_on_launch"] = self.auto_cloud_download_checkbox.isChecked()
        if self.auto_cloud_upload_checkbox is not None:
            values["auto_cloud_save_upload_on_exit"] = self.auto_cloud_upload_checkbox.isChecked()
        if self.auto_cloud_skip_local_newer_checkbox is not None:
            values["auto_cloud_save_skip_download_if_local_newer"] = self.auto_cloud_skip_local_newer_checkbox.isChecked()
        if self.theme_input is not None:
            values["theme"] = self._normalized_theme_choice(self.theme_input.currentText())
        values["emulators"] = self.config.get("emulators", [])
        values["default_emulators"] = self.config.get("default_emulators", {})
        values["default_retroarch_cores"] = self.config.get("default_retroarch_cores", {})
        values["window_geometry"] = self.config.get("window_geometry", "")
        values["window_state"] = self.config.get("window_state", "normal")
        values["first_run_completed"] = bool(self.config.get("first_run_completed", False))
        values["installed_games"] = self.library_games
        values["emulator_source_installs"] = self._normalize_emulator_source_installs(
            self.config.get("emulator_source_installs", {})
        )
        values["auto_cloud_save_upload_delay_seconds"] = self._auto_cloud_upload_delay_seconds()
        values["cloud_sync_state"] = self._cloud_sync_state()
        return values

    def _first_run_setup_complete(self) -> bool:
        value = self.config.get("first_run_completed", False)
        if isinstance(value, bool):
            return value
        if isinstance(value, str):
            normalized = value.strip().casefold()
            if normalized in {"1", "true", "yes", "on"}:
                return True
            if normalized in {"0", "false", "no", "off"}:
                return False
        return bool(value)

    def _run_setup_dialog(self, message_text: str = "") -> bool:
        self._apply_theme("system")
        dialog = FirstRunSetupDialog(self, self.config, message_text)
        if dialog.exec() != QDialog.DialogCode.Accepted:
            return False

        self.config["server_url"] = dialog.server_url()
        self.config["library_path"] = dialog.library_path()
        if not self._set_api_token(dialog.api_token()):
            QMessageBox.warning(self, "Setup Error", "Could not securely save Client Token.")
            return False

        self.config["first_run_completed"] = True
        if not self._ensure_library_path_exists():
            return False
        if not self._save_config(self.config):
            return False

        if self.server_url_input is not None:
            self.server_url_input.setText(self.config.get("server_url", ""))
        if self.api_token_input is not None:
            self.api_token_input.setText(self.config.get("api_token", ""))
        if self.library_path_input is not None:
            self.library_path_input.setText(self.config.get("library_path", ""))

        self.server_auto_reconnect = True
        self._connect_to_server(show_errors=False)
        return True

    def _run_first_run_setup_if_needed(self) -> bool:
        if self._first_run_setup_complete():
            return True
        return self._run_setup_dialog()

    def _run_token_expired_setup(self) -> bool:
        return self._run_setup_dialog(
            "Your Client Token has expired. Enter a new Client Token to continue. You can change these settings later in Settings."
        )

    def _save_settings(self) -> None:
        self.config = self._collect_settings()
        token_value = self.config.get("api_token", "")
        if not isinstance(token_value, str) or not self._set_api_token(token_value):
            if self.settings_status_label is not None:
                self.settings_status_label.setText("Failed to securely save API token")
            QMessageBox.warning(self, "Save Error", "Could not securely save API token")
            return
        if not self._ensure_library_path_exists():
            return
        saved = self._save_config(self.config)
        if self.settings_status_label is not None and saved:
            self.settings_status_label.setText("Settings saved")
        self._clear_server_connection_data()
        self._set_server_status("Not connected", self._theme_color("error", "#ff5555"))
        self._update_top_bar_identity()

        if self._credentials_present() and self.server_auto_reconnect:
            self._connect_to_server(show_errors=False)

    def _ra_login_clicked(self) -> None:
        if self.ra_username_input is None or self.ra_password_input is None:
            return
        username = self.ra_username_input.text().strip()
        password = self.ra_password_input.text()
        if not username or not password:
            self._show_toast("Enter both username and password.")
            return
        if self.ra_login_button is not None:
            self.ra_login_button.setEnabled(False)

        from rom_mate.background.workers import RALoginWorker

        worker = RALoginWorker(username, password)
        thread = QThread(self)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.finished.connect(self._on_ra_login_finished)
        worker.finished.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.start()
        self._ra_login_thread = thread
        self._ra_login_worker = worker

    def _on_ra_login_finished(self, username: str, token: str, error: str) -> None:
        if self.ra_login_button is not None:
            self.ra_login_button.setEnabled(True)
        if error:
            self._show_toast(f"RA login failed: {error}")
            if self.ra_login_status_label is not None:
                self.ra_login_status_label.setText("Login failed")
            return
        self._save_ra_token(token)
        self.config["retroachievements_username"] = username
        self.config["retroachievements_token"] = token
        if self.ra_username_input is not None:
            self.ra_username_input.setText(username)
        if self.ra_password_input is not None:
            self.ra_password_input.clear()
        if self.ra_login_status_label is not None:
            self.ra_login_status_label.setText(f"Logged in as {username}")
        self._show_toast(f"Logged in as {username}")
        for emulator in self._emulators():
            emulator_name = str(emulator.get("name", "")).strip()
            emulator_path = str(emulator.get("path", "")).strip()
            if emulator_name and emulator_path:
                self._ensure_emulator_sync_settings(emulator_name, emulator_path)

    def _ra_clear_credentials(self) -> None:
        self._save_ra_token("")
        self.config["retroachievements_username"] = ""
        self.config["retroachievements_token"] = ""
        if self.ra_username_input is not None:
            self.ra_username_input.clear()
        if self.ra_password_input is not None:
            self.ra_password_input.clear()
        if self.ra_login_status_label is not None:
            self.ra_login_status_label.setText("Not logged in")
        self._show_toast("RetroAchievements credentials cleared.")

    def _ensure_library_path_exists(self) -> bool:
        value = self.config.get("library_path", "")
        if not isinstance(value, str):
            return False

        path = value.strip()
        if not path:
            return True

        try:
            Path(path).expanduser().mkdir(parents=True, exist_ok=True)
            return True
        except OSError as error:
            if self.settings_status_label is not None:
                self.settings_status_label.setText("Failed to create library folder")
            QMessageBox.warning(self, "Save Error", f"Could not create library folder: {error}")
            return False

    def _save_server_connection_settings(self) -> bool:
        if self.server_url_input is not None:
            self.config["server_url"] = self.server_url_input.text().strip()
        if self.api_token_input is not None:
            if not self._set_api_token(self.api_token_input.text()):
                if self.settings_status_label is not None:
                    self.settings_status_label.setText("Failed to securely save API token")
                QMessageBox.warning(self, "Save Error", "Could not securely save API token")
                return False

        saved = self._save_config(self.config)
        if saved and self.settings_status_label is not None:
            self.settings_status_label.setText("Server connection settings saved")
        return saved

    def _connect_from_settings(self, checked: bool = False) -> None:
        del checked
        self.server_auto_reconnect = True
        self._save_server_connection_settings()
        self._connect_to_server()

    def _disconnect_from_server(self, checked: bool = False) -> None:
        del checked
        self.server_auto_reconnect = False
        self._clear_server_connection_data()
        self._set_server_status("Disconnected", self._theme_color("error", "#ff5555"))
        self.config["username"] = ""
        self._update_top_bar_identity()

    def _clear_server_connection_data(self) -> None:
        resolve_clear_server_connection_data(self)
        self._refresh_installed_game_update_state()

    def _resize_server_platform_list(self) -> None:
        if self.server_platforms_list is None:
            return
        content_width = self.server_platforms_list.sizeHintForColumn(0)
        if content_width < 0:
            content_width = 0
        scroll_width = self.server_platforms_list.verticalScrollBar().sizeHint().width()
        frame_width = self.server_platforms_list.frameWidth() * 2
        target_width = content_width + scroll_width + frame_width + 20
        target_width = max(120, min(220, target_width))
        self.server_platforms_list.setFixedWidth(target_width)

    def _server_connected(self) -> bool:
        return self.server_connected

    def _update_top_bar_identity(self) -> None:
        if self.account_status_label is None:
            return
        self.account_status_label.setText(account_status_text(self.config, self._server_connected()))

    def _credentials_present(self) -> bool:
        return credentials_present(self.config)

    def _server_base_url(self) -> str:
        return server_base_url(self.config)

    def _resolve_cover_url(self, value: Any) -> str:
        return resolve_cover_url(value, self._server_base_url())

    def _cover_url_from_rom_payload(self, payload: dict[str, Any]) -> str:
        return cover_url_from_rom_payload(payload, self._resolve_cover_url)

    def _screenshot_urls_from_rom_payload(self, payload: dict[str, Any]) -> list[str]:
        return screenshot_urls_from_rom_payload(payload, self._resolve_cover_url)

    def _screenshot_urls_from_game(self, game: dict[str, str]) -> list[str]:
        return screenshot_urls_from_game(game.get("screenshot_urls", ""))

    def _cached_cover_path_from_game(self, game: dict[str, str]) -> Path | None:
        return cached_cover_path_from_game(game)

    def _cached_cover_cache_key(self, cached_cover_path: Path) -> str:
        return cached_cover_cache_key(cached_cover_path, self._path_key)

    def _cached_cover_for_game(self, game: dict[str, str]) -> QPixmap | None:
        return cached_cover_for_game(self, game)

    def _queue_game_cover_load(self, game: dict[str, str], label: QLabel) -> None:
        queue_game_cover_load(self, game, label)

    def _resolved_cover_url_for_game(self, game: dict[str, str]) -> str:
        return resolve_resolved_cover_url_for_game(
            game,
            self.server_rom_payloads,
            resolve_cover_url=self._resolve_cover_url,
            cover_url_from_rom_payload=self._cover_url_from_rom_payload,
        )

    def _installed_cover_cache_key(self, game: dict[str, str]) -> str:
        return installed_cover_cache_key(game)

    def _cover_cache_extension_from_payload(self, cover_url: str, payload: bytes, content_type: str = "") -> str:
        return cover_cache_extension_from_payload(cover_url, payload, content_type)

    def _cache_cover_image_for_game(self, game: dict[str, str]) -> str:
        return cache_cover_image_for_game(self, game)

    def _cached_cover_path_keys_for_games(self, games: list[dict[str, str]]) -> set[str]:
        return cached_cover_path_keys_for_games(self, games)

    def _cleanup_cached_cover_for_game(
        self,
        game: dict[str, str],
        protected_cache_paths: set[str] | None = None,
    ) -> bool:
        try:
            cleanup_cached_cover_for_game(self, game, protected_cache_paths)
        except OSError as error:
            cached_cover_path = self._cached_cover_path_from_game(game)
            cache_path_text = str(cached_cover_path) if cached_cover_path is not None else "(unknown path)"
            QMessageBox.warning(self, "Uninstall Error", f"Could not remove cached cover image: {cache_path_text}\n{error}")
            return False
        return True

    def _update_details_screenshots(self, game: dict[str, str]) -> None:
        resolve_update_details_screenshots(self, game)

    def _update_details_layout_metrics(self) -> None:
        resolve_update_details_layout_metrics(self)

    def _rescale_details_media_for_current_sizes(self) -> None:
        resolve_rescale_details_media_for_current_sizes(self)

    def _apply_cover_to_label(self, label: QLabel, pixmap: QPixmap | None) -> None:
        apply_cover_to_label(label, pixmap)

    def _queue_cover_load(self, cover_url: str, label: QLabel) -> None:
        queue_cover_load(self, cover_url, label)

    def _on_cover_reply(self, cover_url: str, reply: QNetworkReply) -> None:
        on_cover_reply(self, cover_url, reply)

    def _auth_headers(self) -> dict[str, str]:
        api_token = self.config.get("api_token", "")
        if not isinstance(api_token, str):
            api_token = ""
        return build_auth_headers(api_token)

    def _download_headers(self) -> dict[str, str]:
        api_token = self.config.get("api_token", "")
        if not isinstance(api_token, str):
            api_token = ""
        return build_binary_auth_headers(api_token)

    def _api_get(self, path: str, params: dict[str, Any] | None = None) -> Any:
        base_url = self._server_base_url()
        api_token = self.config.get("api_token", "")
        if not isinstance(api_token, str):
            api_token = ""
        return api_get_json(base_url, api_token, path, params)

    def _api_get_bytes(self, path: str, params: dict[str, Any] | None = None) -> bytes:
        base_url = self._server_base_url()
        api_token = self.config.get("api_token", "")
        if not isinstance(api_token, str):
            api_token = ""
        return api_get_bytes(base_url, api_token, path, params)

    def _multipart_payload(self, files: dict[str, Path]) -> tuple[str, bytes]:
        return multipart_payload(files)

    def _api_post_multipart(self, path: str, files: dict[str, Path], params: dict[str, Any] | None = None) -> Any:
        base_url = self._server_base_url()
        api_token = self.config.get("api_token", "")
        if not isinstance(api_token, str):
            api_token = ""
        return api_post_multipart_json(base_url, api_token, path, files, params)

    def _api_put_multipart(self, path: str, files: dict[str, Path], params: dict[str, Any] | None = None) -> Any:
        base_url = self._server_base_url()
        api_token = self.config.get("api_token", "")
        if not isinstance(api_token, str):
            api_token = ""
        return api_put_multipart_json(base_url, api_token, path, files, params)

    def _api_post_json(self, path: str, payload: dict[str, Any], params: dict[str, Any] | None = None) -> Any:
        base_url = self._server_base_url()
        api_token = self.config.get("api_token", "")
        if not isinstance(api_token, str):
            api_token = ""
        return api_post_json(base_url, api_token, path, payload, params)

    def _set_server_status(self, text: str, color: str | None = None) -> None:
        apply_server_status(self.server_status_label, text, color, self._theme_color("muted", "#6272a4"))

    def _connect_to_server(self, checked: bool = False, show_errors: bool = True) -> None:
        del checked
        if not self._credentials_present():
            self._clear_server_connection_data()
            self._set_server_status("Missing server URL or API token", self._theme_color("error", "#ff5555"))
            self._update_top_bar_identity()
            return

        self._set_server_status("Connecting...", self._theme_color("accent", "#8be9fd"))
        last_error: Exception | None = None
        try:
            me, platforms = fetch_connection_payloads(self._api_get)
            self.server_connected = True
            self._apply_connected_user(me)
            self._populate_server_platforms(platforms)
            self._set_server_status("Connected", self._theme_color("success", "#50fa7b"))
            self._update_top_bar_identity()
            return
        except (HTTPError, URLError, ValueError, json.JSONDecodeError) as error:
            last_error = error

        self._clear_server_connection_data()
        self._update_top_bar_identity()

        failure = classify_connection_failure(last_error)
        self._set_server_status(failure.status_text, self._theme_color("error", "#ff5555"))

        if failure.token_expired:
            if not self._run_token_expired_setup():
                self.close()
            return

        if show_errors and failure.dialog_text:
            QMessageBox.warning(self, "Server Connection", failure.dialog_text)

    def _apply_connected_user(self, me_payload: Any) -> None:
        username = connected_username(me_payload)
        if not username:
            return
        self.config["username"] = username

    def _populate_server_platforms(self, payload: Any) -> None:
        resolve_populate_server_platforms(self, payload, server_platform_ids)

    def _on_server_platform_selected(self, platform_label: str) -> None:
        on_server_platform_selected(self, platform_label)

    def _on_server_search_changed(self, search_text: str) -> None:
        on_server_search_changed(self, search_text)

    def _clear_server_search(self) -> None:
        clear_server_search(self)

    def _load_server_games(self, platform_label: str) -> None:
        if not self.server_connected:
            return

        platform_id = self.server_platform_ids.get(platform_label)
        if platform_id is None:
            return

        if platform_label in self._server_platforms_loading:
            return

        import threading
        from rom_mate.core.api import api_get_json
        from rom_mate.server.catalog import fetch_platform_rom_items, games_from_rom_items
        from rom_mate.cover.utils import cover_url_from_rom_payload, screenshot_urls_from_rom_payload, resolve_cover_url

        # Capture all config values on the main thread before spawning the background thread.
        captured_base_url = self._server_base_url()
        captured_api_token = self.config.get("api_token", "")
        if not isinstance(captured_api_token, str):
            captured_api_token = ""

        self._server_platforms_loading.add(platform_label)

        def _fetch() -> None:
            def _api_get(path: str, params: dict | None = None) -> Any:
                return api_get_json(captured_base_url, captured_api_token, path, params)

            def _cover_url(payload: Any) -> str:
                return cover_url_from_rom_payload(payload, lambda v: resolve_cover_url(v, captured_base_url))

            def _screenshot_urls(payload: Any) -> list:
                return screenshot_urls_from_rom_payload(payload, lambda v: resolve_cover_url(v, captured_base_url))

            try:
                all_items = fetch_platform_rom_items(_api_get, platform_id)
                fetched_games, fetched_payloads = games_from_rom_items(
                    all_items, platform_label, _cover_url, _screenshot_urls
                )
                # Store results then signal main thread — GIL makes the dict write safe.
                self._platform_games_results[platform_label] = (fetched_games, fetched_payloads, "")
            except Exception as exc:
                self._platform_games_results[platform_label] = ([], {}, str(exc))
            self._platform_games_ready.emit(platform_label)

        threading.Thread(target=_fetch, daemon=True).start()

    def _on_platform_games_ready(self, platform_label: str) -> None:
        result = self._platform_games_results.pop(platform_label, None)
        self._server_platforms_loading.discard(platform_label)

        if result is None:
            return

        games, rom_payloads, error = result
        if error:
            self.server_games_by_platform[platform_label] = []
            self._refresh_installed_game_update_state()
            self._set_server_status("Connected, but failed to load games", self._theme_color("warning", "#ffb86c"))
        else:
            self.server_rom_payloads.update(rom_payloads)
            self.server_games_by_platform[platform_label] = games
            self._refresh_installed_game_update_state()

        # Re-render if this is still the selected platform.
        if self.server_platforms_list is not None:
            selected_item = self.server_platforms_list.currentItem()
            if selected_item is not None and selected_item.text() == platform_label:
                self._render_server_games(platform_label)
    def _save_config(self, config: dict[str, Any]) -> bool:
        config_dir = self._config_dir()
        config_file = self._config_file()
        # Emulator paths or library path may have changed — clear session caches.
        self._emulator_sync_settings_done.clear()
        self._sync_directory_paths_cache.clear()
        self._cloud_emulator_entry_cache.clear()

        try:
            resolve_write_config_file(config_dir, config_file, config)
            return True
        except OSError as error:
            if self.settings_status_label is not None:
                self.settings_status_label.setText("Failed to save settings")
            QMessageBox.warning(self, "Save Error", f"Could not save config: {error}")
            return False

    def _open_config_folder(self) -> None:
        config_dir = self._config_dir()
        try:
            config_dir.mkdir(parents=True, exist_ok=True)
        except OSError as error:
            QMessageBox.warning(self, "Open Folder Error", f"Could not open config folder: {error}")
            return

        QDesktopServices.openUrl(QUrl.fromLocalFile(str(config_dir)))

    def _make_game_card(self, game: dict[str, str], source: str) -> QPushButton:
        return resolve_make_game_card(self, game, source)

    def _refresh_library_grid(self) -> None:
        if not hasattr(self, "library_grid") or self.library_scroll is None:
            return

        visible_games = self._visible_library_games()
        has_games = bool(visible_games)
        self.library_scroll.setVisible(has_games)
        if self.library_empty_label is not None:
            self.library_empty_label.setVisible(not has_games)

        for i in range(self.library_grid.rowCount()):
            self.library_grid.setRowStretch(i, 0)
        self._clear_layout(self.library_grid)
        if not has_games:
            return

        columns = self._grid_columns_for_width(self.library_scroll, self.library_grid)
        for i, game in enumerate(visible_games):
            card = self._make_game_card(game, "library")
            row = i // columns
            col = i % columns
            self.library_grid.addWidget(card, row, col)
        last_row = (len(visible_games) - 1) // columns
        self.library_grid.setRowStretch(last_row + 1, 1)

    def _visible_library_games(self) -> list[dict[str, str]]:
        return resolve_visible_library_games(self.library_games)

    def _is_hidden_library_platform(self, game: dict[str, str]) -> bool:
        return resolve_is_hidden_library_platform(game)

    def _render_server_games(self, platform: str) -> None:
        render_server_games(self, platform)

    def _grid_columns_for_width(self, scroll: QScrollArea, grid: QGridLayout) -> int:
        viewport_width = scroll.viewport().width()
        margins = grid.contentsMargins()
        usable_width = viewport_width - margins.left() - margins.right()
        spacing = grid.horizontalSpacing()
        if spacing < 0:
            spacing = 12
        card_width = 180
        column_span = card_width + spacing
        if usable_width <= 0 or column_span <= 0:
            return 1
        return max(1, (usable_width + spacing) // column_span)

    def _game_key(self, game: dict[str, str]) -> tuple[str, str]:
        return game_key(game)

    def _rom_id_key(self, game: dict[str, str]) -> str:
        return rom_id_key(game)

    def _games_match_identity(self, left: dict[str, str], right: dict[str, str]) -> bool:
        return games_match_identity(left, right)

    def _is_game_installed(self, game: dict[str, str]) -> bool:
        return is_game_installed(game, self.library_games)

    def _installed_game_record(self, game: dict[str, str]) -> dict[str, str] | None:
        return installed_game_record(game, self.library_games)

    def _persist_installed_games(self) -> bool:
        self.library_games = self._normalize_installed_games(self.library_games)
        self.config["installed_games"] = self.library_games
        return self._save_config(self.config)


    def _is_arcade_platform(self, game: dict[str, str]) -> bool:
        return resolve_is_arcade_platform(game)


    def _is_ps3_platform(self, game: dict[str, str]) -> bool:
        return resolve_is_ps3_platform(game)


    def _is_ps4_platform(self, game: dict[str, str]) -> bool:
        platform_value = game.get("platform", "")
        platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
        return platform in {"ps4", "playstation 4", "playstation4", "sony playstation 4"}


    def _is_xbox360_platform(self, game: dict[str, str]) -> bool:
        from rom_mate.emulator import is_xbox360_platform
        return is_xbox360_platform(game)


    def _is_windows_pc_platform(self, game: dict[str, str]) -> bool:
        platform_value = game.get("platform", "")
        if not isinstance(platform_value, str):
            return False
        platform = platform_value.strip().casefold()
        if not platform:
            return False
        return "windows" in platform or platform == "pc"


    def _format_version_tag_for_ui(self, version_tag: object) -> str:
        if isinstance(version_tag, int):
            return f"v{version_tag:05d}"
        if isinstance(version_tag, str):
            normalized = version_tag.strip()
            if normalized:
                return f"v{normalized}"
        return ""


    def _details_version_label_text_for_game(self, game: dict[str, str]) -> str:
        if not self._is_windows_pc_platform(game):
            return ""

        version_tag = rom_file_name_version(game.get("rom_file_name", ""))
        if version_tag is None:
            installed_game = self._installed_game_record(game)
            if installed_game is not None:
                version_tag = rom_file_name_version(installed_game.get("rom_file_name", ""))
        if version_tag is None:
            return ""
        formatted_version = self._format_version_tag_for_ui(version_tag)
        if not formatted_version:
            return ""
        return f"Version: {formatted_version}"


    def _details_update_button_text_for_game(self, game: dict[str, str]) -> str:
        default_label = "Update"
        if self._is_emulators_platform(game):
            return default_label

        installed_game = self._installed_game_record(game)
        if installed_game is None:
            return default_label

        server_game = self._server_game_for_identity(installed_game, installed_game.get("rom_id", ""))
        if not isinstance(server_game, dict):
            return default_label

        server_rom_file_name = server_game.get("rom_file_name", "")
        installed_rom_file_name = installed_game.get("rom_file_name", "")
        if not has_newer_server_rom_version(installed_rom_file_name, server_rom_file_name):
            return default_label

        target_version = rom_file_name_version(server_rom_file_name)
        if target_version is None:
            return default_label

        formatted_target_version = self._format_version_tag_for_ui(target_version)
        if not formatted_target_version:
            return default_label
        return f"Update to {formatted_target_version}"


    def _ps4_file_ids_by_category_from_text(self, value: str) -> dict[str, list[int]]:
        if not isinstance(value, str) or not value.strip():
            return {}
        try:
            parsed = json.loads(value)
        except json.JSONDecodeError:
            return {}
        if not isinstance(parsed, dict):
            return {}

        file_ids_by_category: dict[str, list[int]] = {}
        for raw_category, raw_ids in parsed.items():
            if not isinstance(raw_category, str):
                continue
            category = raw_category.strip().lower()
            if not category:
                continue
            if not isinstance(raw_ids, list):
                continue
            normalized_ids = [file_id for file_id in raw_ids if isinstance(file_id, int)]
            if normalized_ids:
                file_ids_by_category[category] = normalized_ids
        return file_ids_by_category


    def _xbox360_file_ids_by_category_from_text(self, value: str) -> dict[str, list[int]]:
        if not isinstance(value, str) or not value.strip():
            return {}
        try:
            parsed = json.loads(value)
        except (json.JSONDecodeError, ValueError):
            return {}
        if not isinstance(parsed, dict):
            return {}
        file_ids_by_category: dict[str, list[int]] = {}
        for raw_category, raw_ids in parsed.items():
            if not isinstance(raw_category, str):
                continue
            category = raw_category.strip().lower()
            if not category:
                continue
            if not isinstance(raw_ids, list):
                continue
            normalized_ids = [file_id for file_id in raw_ids if isinstance(file_id, int)]
            if normalized_ids:
                file_ids_by_category[category] = normalized_ids
        return file_ids_by_category


    def _ps4_file_ids_by_category_from_payload(self, payload: dict[str, Any]) -> dict[str, list[int]]:
        files = payload.get("files")
        if not isinstance(files, list):
            return {}

        file_ids_by_category: dict[str, list[int]] = {}
        for entry in files:
            if not isinstance(entry, dict):
                continue
            file_id = entry.get("id")
            if not isinstance(file_id, int):
                continue
            raw_category = entry.get("category")
            category = raw_category.strip().lower() if isinstance(raw_category, str) and raw_category.strip() else "game"
            file_ids_by_category.setdefault(category, []).append(file_id)
        return file_ids_by_category


    def _server_game_for_identity(self, game: dict[str, str], rom_id: str = "") -> dict[str, str] | None:
        normalized_rom_id = rom_id.strip()
        target_key = self._game_key(game)
        target_rom_key = normalized_rom_id.casefold()
        for games in self.server_games_by_platform.values():
            for server_game in games:
                if target_rom_key and self._rom_id_key(server_game) == target_rom_key:
                    return server_game
                if self._game_key(server_game) == target_key:
                    return server_game
        return None


    def _ps3_game_id_from_text(self, value: str) -> str:
        return resolve_ps3_game_id_from_text(value)


    def _ps3_game_id_from_paths(self, paths: list[Path]) -> str:
        return resolve_ps3_game_id_from_paths(paths)


    def _ps3_game_id_for_game(self, game: dict[str, str]) -> str:
        existing_value = game.get("ps3_game_id", "")
        existing = existing_value.strip().upper() if isinstance(existing_value, str) else ""
        if self._ps3_game_id_from_text(existing):
            return existing

        rom_file_name_value = game.get("rom_file_name", "")
        rom_file_name = rom_file_name_value.strip() if isinstance(rom_file_name_value, str) else ""
        game_id = self._ps3_game_id_from_text(rom_file_name)
        if game_id:
            return game_id

        title_value = game.get("title", "")
        title = title_value.strip() if isinstance(title_value, str) else ""
        game_id = self._ps3_game_id_from_text(title)
        if game_id:
            return game_id

        # Under VFS layout, extracted_dir points to dev_hdd0/game/<GAMEID>/
        extracted_dir_value = game.get("extracted_dir", "")
        extracted_dir_text = extracted_dir_value.strip() if isinstance(extracted_dir_value, str) else ""
        if extracted_dir_text:
            extracted_dir = Path(extracted_dir_text).expanduser()
            game_id = self._ps3_game_id_from_text(extracted_dir.name)
            if game_id:
                return game_id
        return ""


    def _ps3_dev_hdd0_for_game(self, game: dict[str, str]) -> Path | None:
        platform_value = game.get("platform", "")
        platform = platform_value.strip() if isinstance(platform_value, str) else ""
        emulator_name = self._default_emulator_name_for_platform(platform)
        if not emulator_name:
            return None
        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None:
            return None
        emulator_path_value = emulator_entry.get("path", "")
        emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
        library_path_value = self.config.get("library_path", "")
        library_path_text = library_path_value.strip() if isinstance(library_path_value, str) else ""
        ps3_library_path = str(Path(library_path_text).expanduser() / "PlayStation 3") if library_path_text else ""
        return resolve_ps3_vfs_dev_hdd0_path(emulator_path_text, ps3_library_path)


    def _is_emulators_platform(self, game: dict[str, str]) -> bool:
        return resolve_is_emulators_platform(game)


    def _is_native_executable_platform(self, game: dict[str, str]) -> bool:
        return resolve_is_native_executable_platform(game)


    def _launchable_native_game_file(self, path: Path) -> bool:
        return resolve_launchable_native_game_file(path)


    def _native_install_dir_for_game(self, game: dict[str, str]) -> Path | None:
        return resolve_native_install_dir_for_game(
            game,
            self._candidate_archive_paths_for_game(game),
        )


    def _native_executable_candidates_for_game(self, game: dict[str, str]) -> list[Path]:
        return resolve_native_executable_candidates_for_game(
            self._native_install_dir_for_game(game),
            self._launchable_native_game_file,
        )


    def _resolved_native_executable_path_for_game(self, game: dict[str, str]) -> Path | None:
        return resolve_resolved_native_executable_path_for_game(
            game,
            self._native_executable_candidates_for_game(game),
            self._launchable_native_game_file,
        )


    def _default_assignable_server_platforms(self) -> list[str]:
        return resolve_default_assignable_server_platforms(list(self.server_platform_ids.keys()))


    def _launchable_emulator_file(self, path: Path) -> bool:
        return resolve_launchable_emulator_file(path)


    def _select_emulator_executable_path(self, game: dict[str, str], archive_path: Path) -> str:
        return resolve_select_emulator_executable_path(
            game,
            archive_path,
            launchable_emulator_file=self._launchable_emulator_file,
        )


    def _matching_platforms_for_emulator_keywords(self, keywords: list[str]) -> list[str]:
        return resolve_matching_platforms_for_emulator_keywords(
            self._default_assignable_server_platforms(),
            keywords,
        )


    def _emulator_profile_for_entry(self, emulator: dict[str, str]) -> dict[str, Any] | None:
        return resolve_emulator_profile_for_entry(emulator, self._emulator_autoprofiles())


    def _emulator_supports_platform(self, emulator: dict[str, str], platform: str) -> bool:
        selected_platform = platform.strip()
        if not selected_platform:
            return True

        emulator_name_value = emulator.get("name", "")
        emulator_name = emulator_name_value.strip() if isinstance(emulator_name_value, str) else ""

        profile = self._emulator_profile_for_entry(emulator)
        profile_name_value = profile.get("name", "") if isinstance(profile, dict) else ""
        profile_name = profile_name_value.strip() if isinstance(profile_name_value, str) else ""
        is_retroarch = self._is_retroarch_emulator_name(emulator_name) or self._is_retroarch_emulator_name(profile_name)
        if is_retroarch:
            return bool(self._installed_retroarch_cores_for_platform(selected_platform, emulator_name))

        if profile is None:
            return True

        if bool(profile.get("all_platforms", False)):
            return True

        keywords = profile.get("platform_keywords", [])
        if not isinstance(keywords, list):
            return False

        supported_platforms = self._matching_platforms_for_emulator_keywords(keywords)
        selected_folded = selected_platform.casefold()
        return any(isinstance(candidate, str) and candidate.strip().casefold() == selected_folded for candidate in supported_platforms)


    def _compatible_emulator_names_for_platform(self, platform: str) -> list[str]:
        return resolve_compatible_emulator_names_for_platform(
            self._normalize_emulators(self._emulators()),
            platform,
            self._emulator_supports_platform,
        )


    def _emulator_profile_for_game(self, game: dict[str, str], executable_path: str) -> dict[str, Any]:
        return resolve_emulator_profile_for_game(game, executable_path, self._emulator_autoprofiles())


    def _dolphin_variant_label_for_game(self, game: dict[str, str]) -> str:
        return resolve_dolphin_variant_label_for_game(game)


    def _auto_configured_emulator_name(self, base_name: str, game: dict[str, str]) -> str:
        return resolve_auto_configured_emulator_name(
            base_name,
            game,
            dolphin_variant_label_for_game=self._dolphin_variant_label_for_game,
        )


    def _dolphin_target_platforms_for_variant(self, variant: str) -> list[str]:
        return resolve_dolphin_target_platforms_for_variant(
            variant,
            self._default_assignable_server_platforms(),
        )


    def _auto_configure_installed_emulator(self, game: dict[str, str], archive_path: Path) -> bool:
        if not self._is_emulators_platform(game):
            return False

        executable_path = self._select_emulator_executable_path(game, archive_path)
        if not executable_path:
            return False

        profile = self._emulator_profile_for_game(game, executable_path)

        def _installed_cores_for_platform_with_fallback(platform: str, emulator_name: str) -> list[str]:
            platform_cores = self._retroarch_cores_for_platform(platform)
            installed_ids = self._retroarch_installed_core_ids_for_emulator(emulator_name)
            if not installed_ids and self._is_retroarch_emulator_name(emulator_name, None):
                installed_ids = resolve_installed_retroarch_core_ids(executable_path)
            if not installed_ids:
                return []
            return [core for core in platform_cores if core in installed_ids]

        emulators, defaults, core_defaults = resolve_auto_configure_emulator_settings(
            game,
            executable_path,
            profile,
            self._normalize_emulators(self._emulators()),
            self._normalize_default_emulators(self.config.get("default_emulators", {})),
            self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {})),
            auto_configured_emulator_name=self._auto_configured_emulator_name,
            normalize_save_strategy_value=self._normalize_save_strategy_value,
            is_retroarch_emulator_name=self._is_retroarch_emulator_name,
            default_assignable_server_platforms=self._default_assignable_server_platforms,
            installed_retroarch_cores_for_platform=_installed_cores_for_platform_with_fallback,
            matching_platforms_for_emulator_keywords=self._matching_platforms_for_emulator_keywords,
            dolphin_variant_label_for_game=self._dolphin_variant_label_for_game,
            dolphin_target_platforms_for_variant=self._dolphin_target_platforms_for_variant,
        )
        emulators = [MainWindow._emulator_entry_with_source_identity(self, emulator) for emulator in emulators]
        self.config["emulators"] = self._normalize_emulators(emulators)
        self.config["default_emulators"] = defaults
        self.config["default_retroarch_cores"] = core_defaults

        profile_name = str(profile.get("name", "")) if isinstance(profile, dict) else ""
        self._ensure_emulator_sync_settings(profile_name, executable_path)

        self._refresh_emulator_views()
        self._save_config(self.config)
        return True


    def _resolved_emulator_entry_for_game(self, game: dict[str, str]) -> tuple[str, dict[str, str] | None]:
        return resolve_resolved_emulator_entry_for_game(
            game,
            default_emulator_name_for_platform_fn=self._default_emulator_name_for_platform,
            emulator_entry_by_name_fn=self._emulator_entry_by_name,
        )

    def _split_configured_paths(self, value: str) -> list[str]:
        return resolve_split_configured_paths(value)

    def _normalize_save_strategy_value(self, value: str) -> str:
        return resolve_normalize_save_strategy_value(value)

    def _resolved_save_strategy_for_emulator(self, emulator: dict[str, str], save_type: str) -> str:
        return resolve_resolved_save_strategy_for_emulator(
            emulator,
            save_type,
            emulator_profile_for_entry_fn=self._emulator_profile_for_entry,
        )

    def _resolved_ignore_basenames_for_emulator(self, emulator: dict[str, str]) -> set[str]:
        return resolve_resolved_ignore_basenames_for_emulator(
            emulator,
            emulator_profile_for_entry_fn=self._emulator_profile_for_entry,
        )

    def _normalize_ignore_extension_value(self, value: str) -> str:
        return resolve_normalize_ignore_extension_value(value)

    def _resolved_ignore_extensions_for_emulator(self, emulator: dict[str, str]) -> set[str]:
        return resolve_resolved_ignore_extensions_for_emulator(
            emulator,
            emulator_profile_for_entry_fn=self._emulator_profile_for_entry,
        )

    def _sync_directory_ignore_basenames_for_emulator(
        self,
        emulator_name: str,
        emulator: dict[str, str],
        save_type: str,
    ) -> set[str]:
        ignore_basenames = set(self._resolved_ignore_basenames_for_emulator(emulator))
        if save_type == "save" and self._is_pcsx2_emulator_name(emulator_name, emulator):
            ignore_basenames.add("_pcsx2_superblock")
        return ignore_basenames

    def _sync_directory_ignore_extensions_for_emulator(self, emulator: dict[str, str]) -> set[str]:
        return set(self._resolved_ignore_extensions_for_emulator(emulator))

    def _session_window_for_state_upload(self, game: dict[str, str]) -> tuple[float, float] | None:
        return session_window_for_state_upload(
            self.active_game_sessions,
            game,
            self._games_match_identity,
            self._cloud_sync_state_for_game(game),
            time.time(),
        )

    def _filter_files_by_mtime_window(self, files: list[Path], start_time: float, end_time: float) -> list[Path]:
        return filter_files_by_mtime_window(files, start_time, end_time)

    def _filter_directories_by_mtime_window(
        self,
        directories: list[Path],
        start_time: float,
        end_time: float,
        *,
        ignore_basenames: set[str] | None = None,
        ignore_extensions: set[str] | None = None,
    ) -> list[Path]:
        return filter_directories_by_mtime_window(
            directories,
            start_time,
            end_time,
            self._latest_file_mtime_under_path,
            ignore_basenames=ignore_basenames,
            ignore_extensions=ignore_extensions,
        )

    def _show_toast(self, message: str, level: str = "info") -> None:
        del level
        resolve_show_toast(self, message)

def main() -> None:
    app = QApplication(sys.argv)
    window = MainWindow()
    if not window._run_first_run_setup_if_needed():
        window.close()
        return
    window.show()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()

