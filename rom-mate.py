import sys
import json
import base64
import ctypes
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
    build_auth_headers,
    build_binary_auth_headers,
    format_http_error_details,
    merge_config_with_defaults as resolve_merge_config_with_defaults,
    normalize_default_emulators as resolve_normalize_default_emulators,
    normalize_default_retroarch_cores as resolve_normalize_default_retroarch_cores,
    normalize_emulators as resolve_normalize_emulators,
    normalize_installed_games as resolve_normalize_installed_games,
    load_api_token as resolve_load_api_token,
    path_key,
    path_within_path,
    sanitize_path_component,
    multipart_payload,
    save_api_token as resolve_save_api_token,
    set_api_token as resolve_set_api_token,
    windows_protect_data as resolve_windows_protect_data,
    windows_unprotect_data as resolve_windows_unprotect_data,
    write_config_file as resolve_write_config_file,
)
from rom_mate.library import (
    active_download_count_after_finish,
    apply_ps4_content_archive_without_ui as resolve_apply_ps4_content_archive_without_ui,
    apply_download_entry_install_progress,
    apply_download_entry_progress,
    apply_download_entry_status,
    archive_name_for_game as resolve_archive_name_for_game,
    auto_cloud_upload_plan,
    build_installed_game_record as resolve_build_installed_game_record,
    can_start_next_queued_install,
    cemu_save_directories_for_game,
    candidate_archive_paths_for_game as resolve_candidate_archive_paths_for_game,
    candidate_extracted_dirs_for_game as resolve_candidate_extracted_dirs_for_game,
    candidate_extracted_paths_for_game as resolve_candidate_extracted_paths_for_game,
    cleanup_temporary_paths,
    configure_ps3_install_links as resolve_configure_ps3_install_links,
    cloud_sync_candidates_for_game,
    cloud_sync_directory_candidates_for_game,
    cloud_sync_state,
    cloud_sync_state_for_game,
    cloud_sync_state_key,
    detected_ps3_game_id as resolve_detected_ps3_game_id,
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
    native_executable_candidates_for_game as resolve_native_executable_candidates_for_game,
    native_install_dir_for_game as resolve_native_install_dir_for_game,
    no_matching_upload_message,
    ps3_game_id_from_paths as resolve_ps3_game_id_from_paths,
    ps3_game_id_from_text as resolve_ps3_game_id_from_text,
    ps3_game_ids_for_game as resolve_ps3_game_ids_for_game,
    ps3_games_yml_install_path as resolve_ps3_games_yml_install_path,
    ps3_games_yml_path_for_game as resolve_ps3_games_yml_path_for_game,
    ps3_games_yml_paths_for_game as resolve_ps3_games_yml_paths_for_game,
    ps3_link_paths_from_game as resolve_ps3_link_paths_from_game,
    ps3_link_plan_for_extracted_dir as resolve_ps3_link_plan_for_extracted_dir,
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
    prepare_installed_game_without_ui as resolve_prepare_installed_game_without_ui,
    restore_single_save_payload,
    restore_single_state_payload,
    retry_download_game,
    remove_rpcs3_games_yml_entries as resolve_remove_rpcs3_games_yml_entries,
    resolved_native_executable_path_for_game as resolve_resolved_native_executable_path_for_game,
    rom_id_key,
    server_content_file_name_for_game as resolve_server_content_file_name_for_game,
    save_record_timestamp,
    select_extracted_launch_file as resolve_select_extracted_launch_file,
    server_records_from_payload,
    sort_server_records_by_recency,
    should_extract_archive_for_game as resolve_should_extract_archive_for_game,
    update_rpcs3_games_yml_for_install as resolve_update_rpcs3_games_yml_for_install,
    upsert_rpcs3_games_yml_entry as resolve_upsert_rpcs3_games_yml_entry,
    session_cloud_sync_updates,
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
    pico8_save_path_overrides as resolve_pico8_save_path_overrides,
    redream_save_path_overrides as resolve_redream_save_path_overrides,
    redream_state_path_overrides as resolve_redream_state_path_overrides,
    xemu_save_path_overrides as resolve_xemu_save_path_overrides,
    xenia_save_path_overrides as resolve_xenia_save_path_overrides,
    xenia_state_path_overrides as resolve_xenia_state_path_overrides,
    rpcs3_save_path_overrides as resolve_rpcs3_save_path_overrides,
    default_assignable_server_platforms as resolve_default_assignable_server_platforms,
    default_emulator_autoprofiles as resolve_default_emulator_autoprofiles,
    default_emulator_name_for_platform as resolve_default_emulator_name_for_platform,
    ensure_duckstation_memory_card_settings as resolve_ensure_duckstation_memory_card_settings,
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
    fetch_platform_rom_items,
    fetch_server_rom_payload as resolve_fetch_server_rom_payload,
    games_from_rom_items,
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


class MainWindow(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("Rom Mate Neo")
        self.setMinimumSize(1280, 640)
        self.resize(1280, 760)

        self.config = self._load_config()
        self.active_theme_choice = self._normalized_theme_choice(self.config.get("theme", "system"))
        self.active_theme_variant = self._resolved_theme_variant(self.active_theme_choice)
        self.active_theme_colors = self._theme_colors(self.active_theme_variant)
        self.server_url_input: QLineEdit | None = None
        self.api_token_input: QLineEdit | None = None
        self.ra_username_input: QLineEdit | None = None
        self.ra_api_key_input: QLineEdit | None = None
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
        self.details_achievements_panel: QWidget | None = None
        self._pending_pcgw_request_id: int | None = None
        self._pcgw_thread: QThread | None = None
        self._pcgw_worker: QObject | None = None
        self._pcgw_paths_cache: dict[str, list[str]] = {}
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
        self.download_entry_detail_labels: dict[str, QLabel] = {}
        self.active_download_count = 0
        self.active_download_bytes = 0
        self.active_download_total = 0
        self.active_download_speed_bps = 0.0
        self.active_download_entry_id: str | None = None
        self.active_install_bytes = 0
        self.active_install_total = 0
        self.download_entries: list[dict[str, Any]] = []
        self.library_games = self._normalize_installed_games(self.config.get("installed_games", []))
        self.server_games_by_platform: dict[str, list[dict[str, str]]] = {}
        self.installed_game_update_keys: set[tuple[str, str]] = set()
        self.server_rom_payloads: dict[str, dict[str, Any]] = {}
        self.retroarch_compatibility_map: dict[str, list[str]] | None = None
        self.emulator_autoprofiles: list[dict[str, Any]] | None = None
        self.ps3_file_symlink_elevation_consent: bool | None = None
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
        self.ra_api_key_input = QLineEdit(self.config.get("retroachievements_api_key", ""))
        _lock_settings_field_height(self.ra_api_key_input)
        self.ra_api_key_input.setEchoMode(QLineEdit.EchoMode.Password)
        ra_form.addRow("Username", self.ra_username_input)
        ra_form.addRow("API Key", self.ra_api_key_input)
        ra_panel_layout.addLayout(ra_form)
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

    def _windows_protect_data(self, raw: bytes) -> bytes:
        return resolve_windows_protect_data(raw)

    def _windows_unprotect_data(self, protected: bytes) -> bytes:
        return resolve_windows_unprotect_data(protected)

    def _load_api_token(self) -> str:
        return resolve_load_api_token(self._token_file())

    def _save_api_token(self, token: str) -> bool:
        return resolve_save_api_token(self._config_dir(), self._token_file(), token)

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

        try:
            all_items = fetch_platform_rom_items(self._api_get, platform_id)
        except (HTTPError, URLError, ValueError, json.JSONDecodeError):
            self.server_games_by_platform[platform_label] = []
            self._refresh_installed_game_update_state()
            self._set_server_status("Connected, but failed to load games", self._theme_color("warning", "#ffb86c"))
            return

        games, rom_payloads = games_from_rom_items(
            all_items,
            platform_label,
            self._cover_url_from_rom_payload,
            self._screenshot_urls_from_rom_payload,
        )
        self.server_rom_payloads.update(rom_payloads)
        self.server_games_by_platform[platform_label] = games
        self._refresh_installed_game_update_state()

    def _save_config(self, config: dict[str, Any]) -> bool:
        config_dir = self._config_dir()
        config_file = self._config_file()

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

        self._clear_layout(self.library_grid)
        if not has_games:
            return

        columns = self._grid_columns_for_width(self.library_scroll, self.library_grid)
        for i, game in enumerate(visible_games):
            card = self._make_game_card(game, "library")
            row = i // columns
            col = i % columns
            self.library_grid.addWidget(card, row, col)

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
        return True

    def _register_installed_game(self, game: dict[str, str], archive_path: Path) -> None:
        game_key_value = self._game_key(game)
        self.library_games = [entry for entry in self.library_games if self._game_key(entry) != game_key_value]
        self.library_games.append(
            resolve_build_installed_game_record(
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

    def _is_arcade_platform(self, game: dict[str, str]) -> bool:
        return resolve_is_arcade_platform(game)

    def _is_ps3_platform(self, game: dict[str, str]) -> bool:
        return resolve_is_ps3_platform(game)

    def _is_ps4_platform(self, game: dict[str, str]) -> bool:
        platform_value = game.get("platform", "")
        platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
        return platform in {"ps4", "playstation 4", "playstation4", "sony playstation 4"}

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
        self._update_details_action_buttons()

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

    def _ps4_content_archive_name(self, game: dict[str, str], content_kind: str) -> str:
        safe_title = self._sanitize_path_component(game.get("title", "ps4-content"), "ps4-content")
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
        return resolve_apply_ps4_content_archive_without_ui(
            installed_game,
            archive_path,
            content_kind=content_kind,
            extracted_dir_for_archive_path=self._extracted_dir_for_archive_path,
            extract_archive_into_directory=resolve_extract_archive_into_directory,
            install_progress_callback=install_progress_callback,
        )

    def _sync_ps4_content_metadata_to_installed_game(self, source_game: dict[str, str], updated_game: dict[str, str]) -> None:
        installed_game = self._installed_game_record(source_game)
        if installed_game is None:
            return
        installed_game["ps4_game_id"] = updated_game.get("ps4_game_id", "")
        installed_game["ps4_content"] = updated_game.get("ps4_content", "")
        self._persist_installed_games()

    def _ps3_emulator_root_for_game(self, game: dict[str, str]) -> Path | None:
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
        if not emulator_path_text:
            return None

        emulator_path = Path(emulator_path_text).expanduser()
        if emulator_path.exists() and emulator_path.is_dir():
            return emulator_path
        emulator_root = emulator_path.parent
        if emulator_root.exists() and emulator_root.is_dir():
            return emulator_root
        return None

    def _ps3_link_plan_for_extracted_dir(self, extracted_dir: Path, emulator_root: Path) -> list[tuple[Path, Path, bool]]:
        return resolve_ps3_link_plan_for_extracted_dir(extracted_dir, emulator_root, self._path_key)

    def _ps3_game_id_from_text(self, value: str) -> str:
        return resolve_ps3_game_id_from_text(value)

    def _ps3_game_id_from_paths(self, paths: list[Path]) -> str:
        return resolve_ps3_game_id_from_paths(paths)

    def _detected_ps3_game_id(self, extracted_dir: Path, link_targets: list[Path]) -> str:
        return resolve_detected_ps3_game_id(extracted_dir, link_targets)

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

        link_paths = self._ps3_link_paths_from_game(game)
        game_id = self._ps3_game_id_from_paths(link_paths)
        if game_id:
            return game_id

        extracted_dir_value = game.get("extracted_dir", "")
        extracted_dir_text = extracted_dir_value.strip() if isinstance(extracted_dir_value, str) else ""
        if extracted_dir_text:
            extracted_dir = Path(extracted_dir_text).expanduser()
            if extracted_dir.exists() and extracted_dir.is_dir():
                game_id = self._detected_ps3_game_id(extracted_dir, link_paths)
                if game_id:
                    return game_id
        return ""

    def _is_rpcs3_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "rpcs3", emulator=emulator) or resolve_is_rpcs3_emulator_name(emulator_name)

    def _is_ps3_emulator_entry(self, emulator: dict[str, str]) -> bool:
        return resolve_is_ps3_emulator_entry(emulator, self._emulator_profile_for_entry)

    def _has_installed_ps3_games(self) -> bool:
        return any(self._is_ps3_platform(game) for game in self.library_games)

    def _create_link_path(self, source_path: Path, link_path: Path, is_directory: bool) -> None:
        link_path.parent.mkdir(parents=True, exist_ok=True)
        if link_path.exists():
            return

        if is_directory:
            result = subprocess.run(
                ["cmd", "/c", "mklink", "/J", str(link_path), str(source_path)],
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode == 0:
                return

            try:
                os.symlink(str(source_path), str(link_path), target_is_directory=True)
                return
            except (AttributeError, NotImplementedError, OSError):
                pass

            error_text = result.stderr.strip() or result.stdout.strip() or "Unknown link creation error"
            raise OSError(error_text)

        try:
            os.symlink(str(source_path), str(link_path), target_is_directory=False)
            return
        except (AttributeError, NotImplementedError, OSError) as error:
            if self._create_elevated_file_symlink(source_path, link_path):
                return
            raise OSError(str(error)) from error

    def _create_elevated_file_symlink(self, source_path: Path, link_path: Path) -> bool:
        if os.name != "nt":
            return False

        if self.ps3_file_symlink_elevation_consent is None:
            answer = QMessageBox.question(
                self,
                "Administrator Permission Required",
                "Creating file symlinks for this PS3 install requires administrator permission on Windows.\n\n"
                "Do you want to allow a one-time elevation prompt for this operation?",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.Yes,
            )
            self.ps3_file_symlink_elevation_consent = answer == QMessageBox.StandardButton.Yes

        if not self.ps3_file_symlink_elevation_consent:
            return False

        source_text = str(source_path)
        link_text = str(link_path)
        command_params = f'/c mklink "{link_text}" "{source_text}"'
        result = ctypes.windll.shell32.ShellExecuteW(None, "runas", "cmd.exe", command_params, None, 0)
        if result <= 32:
            return False

        deadline = time.monotonic() + 10.0
        while time.monotonic() < deadline:
            if link_path.exists():
                return True
            time.sleep(0.1)
        return link_path.exists()

    def _remove_link_path(self, link_path: Path) -> None:
        if not link_path.exists() and not link_path.is_symlink():
            return
        if link_path.is_symlink():
            link_path.unlink()
            return
        if link_path.is_file():
            link_path.unlink()
            return
        if link_path.is_dir():
            try:
                os.rmdir(link_path)
            except OSError:
                self._remove_directory_tree(link_path)
            return
        raise OSError(f"Unsupported link path type: {link_path}")

    def _ps3_link_paths_from_game(self, game: dict[str, str]) -> list[Path]:
        return resolve_ps3_link_paths_from_game(game)

    def _ps3_games_yml_path_for_game(self, game: dict[str, str]) -> Path | None:
        return resolve_ps3_games_yml_path_for_game(game, self._ps3_emulator_root_for_game)

    def _ps3_games_yml_paths_for_game(self, game: dict[str, str]) -> list[Path]:
        return resolve_ps3_games_yml_paths_for_game(
            game,
            self._ps3_games_yml_path_for_game(game),
            self._ps3_link_paths_from_game(game),
            self._path_key,
        )

    def _ps3_games_yml_install_path(
        self,
        game_id: str,
        link_paths: list[Path],
        extracted_dir: Path,
        emulator_root: Path,
    ) -> str:
        return resolve_ps3_games_yml_install_path(game_id, link_paths, extracted_dir, emulator_root)

    def _upsert_rpcs3_games_yml_entry(self, games_yml_path: Path, game_id: str, install_path: str) -> None:
        resolve_upsert_rpcs3_games_yml_entry(games_yml_path, game_id, install_path)

    def _ps3_game_ids_for_game(self, game: dict[str, str]) -> set[str]:
        return resolve_ps3_game_ids_for_game(
            game,
            self._ps3_game_id_from_text,
            self._ps3_game_id_for_game,
            self._ps3_link_paths_from_game(game),
        )

    def _remove_rpcs3_games_yml_entries(self, games_yml_path: Path, game_ids: set[str]) -> None:
        resolve_remove_rpcs3_games_yml_entries(games_yml_path, game_ids, self._ps3_game_id_from_text)

    def _remove_rpcs3_games_yml_for_game(self, game: dict[str, str]) -> None:
        game_ids = self._ps3_game_ids_for_game(game)
        if not game_ids:
            return

        for games_yml_path in self._ps3_games_yml_paths_for_game(game):
            self._remove_rpcs3_games_yml_entries(games_yml_path, game_ids)

    def _update_rpcs3_games_yml_for_install(self, game: dict[str, str], extracted_dir: Path, link_paths: list[Path]) -> str:
        return resolve_update_rpcs3_games_yml_for_install(
            game,
            extracted_dir,
            link_paths,
            detected_ps3_game_id=self._detected_ps3_game_id,
            ps3_game_id_from_text=self._ps3_game_id_from_text,
            ps3_emulator_root_for_game=self._ps3_emulator_root_for_game,
            ps3_games_yml_install_path=self._ps3_games_yml_install_path,
            ps3_games_yml_path_for_game=self._ps3_games_yml_path_for_game,
            upsert_rpcs3_games_yml_entry=self._upsert_rpcs3_games_yml_entry,
        )

    def _configure_ps3_install_links(self, game: dict[str, str], extracted_dir: Path) -> list[Path]:
        self.ps3_file_symlink_elevation_consent = None
        try:
            return resolve_configure_ps3_install_links(
                extracted_dir,
                self._ps3_emulator_root_for_game(game),
                self._ps3_link_plan_for_extracted_dir,
                self._create_link_path,
                self._remove_link_path,
            )
        finally:
            self.ps3_file_symlink_elevation_consent = None

    def _should_extract_archive_for_game(self, game: dict[str, str], archive_path: Path) -> bool:
        return resolve_should_extract_archive_for_game(
            game,
            archive_path,
            is_native_executable_platform=self._is_native_executable_platform,
            is_arcade_platform=self._is_arcade_platform,
            is_ps3_platform=self._is_ps3_platform,
        )

    def _extracted_dir_for_archive_path(self, archive_path: Path) -> Path:
        return resolve_extracted_dir_for_archive_path(archive_path)

    def _select_extracted_launch_file(self, game: dict[str, str], extracted_dir: Path, archive_path: Path) -> Path | None:
        return resolve_select_extracted_launch_file(
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
        return resolve_extract_archive_for_game(
            game,
            archive_path,
            extracted_dir_for_archive_path=self._extracted_dir_for_archive_path,
            select_extracted_launch_file=self._select_extracted_launch_file,
            install_progress_callback=install_progress_callback,
        )

    def _directory_total_file_bytes(self, directory: Path) -> int:
        return resolve_directory_total_file_bytes(directory)

    def _tar_archive_total_install_bytes(self, archive_path: Path) -> int:
        return resolve_tar_archive_total_install_bytes(archive_path)

    def _tar_listing_line_size(self, line: str) -> int:
        return resolve_tar_listing_line_size(line)

    def _prepare_installed_game(self, game: dict[str, str], archive_path: Path) -> dict[str, str] | None:
        prepared, warning_text = self._prepare_installed_game_without_ui(game, archive_path, configure_ps3_links=True)
        if prepared is None:
            title = game.get("title", "Game")
            error_text = warning_text or f"Failed to extract archive for {title}"
            QMessageBox.warning(self, "Install Error", f"Failed to install {title}: {error_text}")
            return None
        if warning_text:
            QMessageBox.warning(self, "Install Warning", warning_text)
        return prepared

    def _prepare_installed_game_without_ui(
        self,
        game: dict[str, str],
        archive_path: Path,
        *,
        configure_ps3_links: bool,
        install_progress_callback: Callable[[int, int], None] | None = None,
    ) -> tuple[dict[str, str] | None, str]:
        return resolve_prepare_installed_game_without_ui(
            game,
            archive_path,
            configure_ps3_links=configure_ps3_links,
            should_extract_archive_for_game=self._should_extract_archive_for_game,
            extract_archive_for_game=self._extract_archive_for_game,
            is_ps3_platform=self._is_ps3_platform,
            configure_ps3_install_links=self._configure_ps3_install_links,
            update_rpcs3_games_yml_for_install=self._update_rpcs3_games_yml_for_install,
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

    def _is_emulators_platform(self, game: dict[str, str]) -> bool:
        return resolve_is_emulators_platform(game)

    def _is_native_executable_platform(self, game: dict[str, str]) -> bool:
        return resolve_is_native_executable_platform(game)

    def _cloud_save_block_reason_for_game(
        self,
        game: dict[str, str],
        emulator_name: str = "",
        emulator: dict[str, str] | None = None,
        *,
        save_type: str = "save",
    ) -> str:
        return resolve_cloud_save_block_reason_for_game(
            game,
            is_native_executable_platform=self._is_native_executable_platform,
            emulator_name=emulator_name,
            is_xemu_emulator_name=lambda value: self._is_xemu_emulator_name(value, emulator),
            is_redream_emulator_name=lambda value: self._is_redream_emulator_name(value, emulator),
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
        return resolve_cloud_save_scope_for_game(
            game,
            emulator_name=emulator_name,
            is_xemu_emulator_name=lambda value: self._is_xemu_emulator_name(value, emulator),
            is_redream_emulator_name=lambda value: self._is_redream_emulator_name(value, emulator),
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
        resolved_game = self._installed_game_record(game) or game
        emulator_name, emulator_entry = self._resolved_emulator_entry_for_game(resolved_game)
        if emulator_entry is not None:
            if callable(timing_end):
                timing_end("_resolved_cloud_emulator_entry_for_game", started_at, emulator=emulator_name, source="default")
            return emulator_name, emulator_entry
        if not self._is_emulators_platform(resolved_game):
            if callable(timing_end):
                timing_end("_resolved_cloud_emulator_entry_for_game", started_at, source="none")
            return emulator_name, emulator_entry

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
            return candidate_name, candidate

        if callable(timing_end):
            timing_end("_resolved_cloud_emulator_entry_for_game", started_at, source="unresolved")
        return emulator_name, emulator_entry

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
            installed_retroarch_cores_for_platform=self._installed_retroarch_cores_for_platform,
            matching_platforms_for_emulator_keywords=self._matching_platforms_for_emulator_keywords,
            dolphin_variant_label_for_game=self._dolphin_variant_label_for_game,
            dolphin_target_platforms_for_variant=self._dolphin_target_platforms_for_variant,
        )
        self.config["emulators"] = self._normalize_emulators(emulators)
        self.config["default_emulators"] = defaults
        self.config["default_retroarch_cores"] = core_defaults

        profile_name = str(profile.get("name", "")) if isinstance(profile, dict) else ""
        self._ensure_emulator_sync_settings(profile_name, executable_path)

        self._refresh_emulator_views()
        self._save_config(self.config)
        return True

    def _archive_name_for_game(self, game: dict[str, str]) -> str:
        return resolve_archive_name_for_game(game, self._sanitize_path_component)

    def _server_content_file_name_for_game(self, game: dict[str, str]) -> str:
        return resolve_server_content_file_name_for_game(game)

    def _rom_file_name_from_payload(self, payload: dict[str, Any]) -> str:
        return resolve_rom_file_name_from_payload(payload)

    def _fetch_server_rom_payload(self, rom_id: str, force_refresh: bool = False) -> dict[str, Any] | None:
        return resolve_fetch_server_rom_payload(
            rom_id,
            self.server_rom_payloads,
            api_get=self._api_get,
            force_refresh=force_refresh,
        )

    def _resolved_rom_file_name_for_game(self, game: dict[str, str], rom_id: str) -> str:
        return resolve_resolved_rom_file_name_for_game(
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
        return resolve_candidate_archive_paths_for_game(
            game,
            self._platform_library_dir,
            self._archive_name_for_game,
            self._library_path_dir,
        )

    def _candidate_extracted_paths_for_game(self, game: dict[str, str]) -> list[Path]:
        return resolve_candidate_extracted_paths_for_game(
            game,
            self._select_extracted_launch_file,
        )

    def _candidate_extracted_dirs_for_game(self, game: dict[str, str]) -> list[Path]:
        return resolve_candidate_extracted_dirs_for_game(
            game,
            self._candidate_archive_paths_for_game(game),
            self._extracted_dir_for_archive_path,
        )

    def _remove_game_files(self, game: dict[str, str]) -> bool:
        try:
            resolve_remove_game_files(
                game,
                is_ps3_platform=self._is_ps3_platform,
                ps3_link_paths_from_game=self._ps3_link_paths_from_game,
                remove_link_path=self._remove_link_path,
                remove_rpcs3_games_yml_for_game=self._remove_rpcs3_games_yml_for_game,
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
        return resolve_matching_installed_emulator_games(
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

        self.library_games, removed = resolve_uninstall_library_games(
            self.library_games,
            {self._game_key(game) for game in matches},
            game_key=self._game_key,
            library_games_without_keys=resolve_library_games_without_keys,
            cached_cover_path_keys_for_games=self._cached_cover_path_keys_for_games,
            remove_game_files=self._remove_game_files,
            cleanup_cached_cover_for_game=self._cleanup_cached_cover_for_game,
        )
        if removed:
            self._refresh_library_grid()
            self._persist_installed_games()
        return removed

    def _uninstall_game(self, game: dict[str, str]) -> bool:
        self.library_games, removed = resolve_uninstall_library_games(
            self.library_games,
            {self._game_key(game)},
            game_key=self._game_key,
            library_games_without_keys=resolve_library_games_without_keys,
            cached_cover_path_keys_for_games=self._cached_cover_path_keys_for_games,
            remove_game_files=self._remove_game_files,
            cleanup_cached_cover_for_game=self._cleanup_cached_cover_for_game,
        )
        if removed:
            self._refresh_installed_game_update_state()
            self._refresh_library_grid()
            self._persist_installed_games()
        return removed

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        self._reflow_current_page_grid()
        self._update_details_layout_metrics()
        QTimer.singleShot(0, self._update_details_layout_metrics)

    def _enrich_game_for_details(self, game: dict) -> None:
        """Merge missing server-side fields into a game dict before opening details.

        This ensures library games (which may lack ra_id or update metadata) get
        enriched from the live server game list, so the details page behaviour is
        identical regardless of which page the user navigated from.
        """
        rom_id = game.get("rom_id", "")
        if not rom_id:
            return
        server_game = next(
            (
                g
                for games in self.server_games_by_platform.values()
                for g in games
                if g.get("rom_id") == rom_id
            ),
            None,
        )
        if server_game is None:
            return
        for field in ("ra_id", "ps4_has_update", "ps4_has_dlc", "ps4_file_ids_by_category"):
            if not game.get(field):
                value = server_game.get(field)
                if value:
                    game[field] = value

    def _pcgw_cache_key(self, game: dict) -> str:
        return game.get("title", "").strip()

    def _pcgw_paths_for_game(self, game: dict) -> list[str] | None:
        """Return cached path list, or None if not yet fetched."""
        key = self._pcgw_cache_key(game)
        if key not in self._pcgw_paths_cache:
            return None
        return self._pcgw_paths_cache[key]

    def _start_pcgw_lookup_for_game(self, game: dict) -> None:
        from rom_mate.background.workers import PCGamingWikiWorker

        request_id = int(time.time() * 1000) % 1_000_000
        self._pending_pcgw_request_id = request_id

        title = game.get("title", "").strip()
        worker = PCGamingWikiWorker(request_id, title)
        thread = QThread(self)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.finished.connect(self._on_pcgw_paths_loaded)
        worker.finished.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.start()

        self._pcgw_thread = thread
        self._pcgw_worker = worker

    def _on_pcgw_paths_loaded(self, request_id: int, raw_paths: list, error: str) -> None:
        if request_id != self._pending_pcgw_request_id:
            return
        game = self._current_details_game
        if game is None:
            return
        key = self._pcgw_cache_key(game)
        self._pcgw_paths_cache[key] = raw_paths
        # Refresh the native save panel if still showing it
        if self.current_details_cloud_mode == "save":
            self._refresh_details_cloud_panel()

    def _expanded_pcgw_paths_for_game(self, game: dict) -> list:
        """Return PCGamingWiki paths expanded via os.path.expandvars as Path objects."""
        import os
        from pathlib import Path

        raw_paths = self._pcgw_paths_for_game(game) or []
        result = []
        for raw in raw_paths:
            expanded = os.path.expandvars(raw)
            result.append(Path(expanded))
        return result

    def _open_game_details(self, game: dict[str, str], source: str) -> None:
        self._enrich_game_for_details(game)
        self._current_details_game = game
        resolve_open_game_details(self, game, source)
        details_achievements_button = self.details_achievements_button
        if details_achievements_button is None:
            return
        from rom_mate.server.retroachievements import resolve_ra_game_id

        ra_game_id = resolve_ra_game_id(
            game,
            self.config.get("retroachievements_username", ""),
            self.config.get("retroachievements_api_key", ""),
        )
        details_achievements_button.setVisible(ra_game_id is not None)

    def _update_details_action_buttons(self) -> None:
        resolve_update_details_action_buttons(self)
        if self.current_details_cloud_mode != "overview":
            self._refresh_details_cloud_panel()

    def _is_game_install_queued(self, game: dict[str, str]) -> bool:
        return is_game_install_queued(game, self.install_queue)

    def _start_next_queued_install(self) -> None:
        if not can_start_next_queued_install(
            self.install_in_progress,
            self.install_finalize_in_progress,
            self.install_queue,
        ):
            self._update_details_action_buttons()
            self._update_download_status_ui()
            return
        next_game = self.install_queue.pop(0)
        self._start_async_install(next_game)

    def _details_rom_id_cache_key(self, game: dict[str, str] | None) -> str:
        return resolve_details_rom_id_cache_key(game, game_key=self._game_key)

    def _details_rom_id_cache(self) -> dict[str, str]:
        return resolve_details_rom_id_cache(self.config.get("details_rom_id_cache", {}))

    def _normalize_cloud_sync_state(self, value: Any) -> dict[str, dict[str, Any]]:
        return normalize_cloud_sync_state(value)

    def _cloud_sync_state(self) -> dict[str, dict[str, Any]]:
        normalized = cloud_sync_state(self.config.get("cloud_sync_state", {}))
        self.config["cloud_sync_state"] = normalized
        return normalized

    def _cloud_sync_state_key(self, game: dict[str, str]) -> str:
        return cloud_sync_state_key(game, self._rom_id_key, self._game_key)

    def _cloud_sync_state_for_game(self, game: dict[str, str]) -> dict[str, Any]:
        return cloud_sync_state_for_game(self._cloud_sync_state(), game, self._rom_id_key, self._game_key)

    def _update_cloud_sync_state_for_game(self, game: dict[str, str], updates: dict[str, Any]) -> None:
        state_map = update_cloud_sync_state_for_game(
            self._cloud_sync_state(),
            game,
            updates,
            self._rom_id_key,
            self._game_key,
        )
        self.config["cloud_sync_state"] = state_map
        self._save_config(self.config)

    def _cache_rom_id_for_details_game(self, game: dict[str, str], rom_id: str) -> None:
        if resolve_cache_rom_id_for_details_game(
            self.config,
            game,
            rom_id,
            details_rom_id_cache_key=self._details_rom_id_cache_key,
            details_rom_id_cache=self._details_rom_id_cache,
        ):
            self._save_config(self.config)

    def _clear_cached_rom_id_for_details_game(self, game: dict[str, str] | None) -> None:
        if resolve_clear_cached_rom_id_for_details_game(
            self.config,
            game,
            details_rom_id_cache_key=self._details_rom_id_cache_key,
            details_rom_id_cache=self._details_rom_id_cache,
        ):
            self._save_config(self.config)

    def _resolve_rom_id_for_game(self, game: dict[str, str]) -> str:
        return resolve_resolve_rom_id_for_game(
            game,
            self.server_games_by_platform,
            game_key=self._game_key,
            details_rom_id_cache_key=self._details_rom_id_cache_key,
            details_rom_id_cache=self._details_rom_id_cache,
        )

    def _hydrate_install_game_metadata(self, game: dict[str, str], rom_id: str) -> None:
        resolve_hydrate_install_game_metadata(
            game,
            rom_id,
            server_games_by_platform=self.server_games_by_platform,
            server_rom_payloads=self.server_rom_payloads,
            game_key=self._game_key,
            rom_id_key=self._rom_id_key,
            fetch_server_rom_payload=self._fetch_server_rom_payload,
            resolved_cover_url_for_game=self._resolved_cover_url_for_game,
            cover_url_from_rom_payload=self._cover_url_from_rom_payload,
            screenshot_urls_from_rom_payload=self._screenshot_urls_from_rom_payload,
        )

    def _cleanup_details_view_state(self) -> None:
        self._clear_cached_rom_id_for_details_game(self.current_details_game)
        self._show_details_overview()
        self.current_details_game = None
        self._current_details_game = None

    def _start_async_install(self, game: dict[str, str]) -> bool:
        install_mode_value = game.get("_install_mode", "base")
        install_mode = install_mode_value.strip().lower() if isinstance(install_mode_value, str) else "base"
        if install_mode in {"source_emulator", "source_emulator_update"}:
            return self._start_async_source_emulator_install(game)
        is_ps4_content_install = install_mode == "ps4_content"

        if is_ps4_content_install:
            ps4_block_reason = self._ps4_content_install_block_reason(game)
            if ps4_block_reason:
                QMessageBox.warning(self, "Install Blocked", ps4_block_reason)
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
        resolve_sync_install_metadata_to_details_game(
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

        archive_name_value = install_game.get("_archive_name_override", "")
        archive_name = archive_name_value.strip() if isinstance(archive_name_value, str) and archive_name_value.strip() else self._archive_name_for_game(install_game)
        archive_path = install_path / archive_name
        rom_id_path = quote(rom_id, safe="")
        file_name_path = quote(self._server_content_file_name_for_game(install_game), safe="")
        download_url = f"{base_url}/api/roms/{rom_id_path}/content/{file_name_path}"
        file_ids_csv_value = install_game.get("_ps4_file_ids_csv", "")
        file_ids_csv = file_ids_csv_value.strip() if isinstance(file_ids_csv_value, str) else ""
        if file_ids_csv:
            download_url = f"{download_url}?{urlencode({'file_ids': file_ids_csv})}"

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

        if install_mode not in {"ps4_content", "update", "source_emulator", "source_emulator_update"} and self._is_game_installed(game):
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
        is_source_install = install_mode in {"source_emulator", "source_emulator_update"}
        is_source_update = install_mode == "source_emulator_update"
        is_native_update = install_mode == "native_update"
        content_kind_value = game.get("_ps4_content_kind", "") if isinstance(game, dict) else ""
        content_kind = content_kind_value.strip().lower() if isinstance(content_kind_value, str) else "content"
        install_label = f"PS4 {content_kind}" if is_ps4_content_install else title

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
            if warning_text.strip():
                QMessageBox.warning(self, "Install Warning", warning_text.strip())
            self._update_download_status_ui()
            self._update_details_action_buttons()
            self._start_next_queued_install()
            return

        if is_native_update:
            self._register_installed_game(installed_game, Path(archive_path))
            if entry_id:
                self._set_download_entry_status(entry_id, "completed")
            if warning_text.strip():
                QMessageBox.warning(self, "Install Warning", warning_text.strip())
            self._show_toast(f"Updated '{title}' successfully.", level="success")
            self._update_download_status_ui()
            self._update_details_action_buttons()
            self._start_next_queued_install()
            return

        extracted_dir_text = installed_game.get("extracted_dir", "")
        if self._is_ps3_platform(installed_game) and isinstance(extracted_dir_text, str) and extracted_dir_text.strip():
            try:
                extracted_dir = Path(extracted_dir_text)
                ps3_links = self._configure_ps3_install_links(installed_game, extracted_dir)
                installed_game["ps3_links"] = json.dumps([str(path) for path in ps3_links])
                installed_game["ps3_game_id"] = self._update_rpcs3_games_yml_for_install(installed_game, extracted_dir, ps3_links)
            except OSError as ps3_error:
                if entry_id:
                    self._set_download_entry_status(entry_id, "failed", str(ps3_error))
                self._update_download_status_ui()
                self._update_details_action_buttons()
                QMessageBox.warning(
                    self,
                    "Install Error",
                    f"Failed to prepare PS3 symlink layout for {title}: {ps3_error}",
                )
                self._start_next_queued_install()
                return

        archive_file = Path(archive_path)
        self._register_installed_game(installed_game, archive_file)
        auto_configured = self._auto_configure_installed_emulator(installed_game, archive_file)
        if is_source_install:
            self._record_source_emulator_install(installed_game)
        if entry_id:
            self._set_download_entry_status(entry_id, "completed")
        if warning_text.strip():
            QMessageBox.warning(self, "Install Warning", warning_text.strip())
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

    def _percent_text(self, completed: int, total: int) -> str:
        return percent_text(completed, total)

    def _format_size(self, size_bytes: float) -> str:
        return format_size(size_bytes)

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

    def _return_from_details(self) -> None:
        self._switch_page(self.current_main_page_index)

    def _show_details_overview(self) -> None:
        self.current_details_cloud_mode = "overview"
        if self.details_center_stack is not None:
            self.details_center_stack.setCurrentIndex(0)
        if self.details_details_button is not None:
            self.details_details_button.setChecked(True)
        if self.details_manage_saves_button is not None:
            self.details_manage_saves_button.setChecked(False)
        if self.details_manage_states_button is not None:
            self.details_manage_states_button.setChecked(False)
        details_achievements_button = getattr(self, "details_achievements_button", None)
        if details_achievements_button is not None:
            details_achievements_button.setChecked(False)

    def _perform_show_details_action(self) -> None:
        self._show_details_overview()
        self._update_details_layout_metrics()
        QTimer.singleShot(0, self._update_details_layout_metrics)

    def _perform_manage_saves_action(self) -> None:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start(
            "_perform_manage_saves_action",
            title=(self.current_details_game or {}).get("title", ""),
            platform=(self.current_details_game or {}).get("platform", ""),
        ) if callable(timing_start) else 0.0
        self._toggle_details_cloud_mode("save")
        if callable(timing_end):
            timing_end("_perform_manage_saves_action", started_at, mode=self.current_details_cloud_mode)

    def _perform_manage_states_action(self) -> None:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start(
            "_perform_manage_states_action",
            title=(self.current_details_game or {}).get("title", ""),
            platform=(self.current_details_game or {}).get("platform", ""),
        ) if callable(timing_start) else 0.0
        self._toggle_details_cloud_mode("state")
        if callable(timing_end):
            timing_end("_perform_manage_states_action", started_at, mode=self.current_details_cloud_mode)

    def _details_cloud_button_visible(self, save_type: str) -> bool:
        button = self.details_manage_saves_button if save_type == "save" else self.details_manage_states_button
        if button is None:
            return False
        visible_fn = getattr(button, "isVisible", None)
        if callable(visible_fn):
            return bool(visible_fn())
        visible_value = getattr(button, "visible", None)
        if isinstance(visible_value, bool):
            return visible_value
        return True

    def _details_cloud_active_button_text(self, save_type: str) -> str:
        button = self.details_manage_saves_button if save_type == "save" else self.details_manage_states_button
        if button is not None:
            text_value = getattr(button, "text", None)
            if callable(text_value):
                resolved_text = str(text_value()).strip()
                if resolved_text:
                    return resolved_text
            elif isinstance(text_value, str) and text_value.strip():
                return text_value.strip()
        if save_type == "save":
            return self._details_cloud_button_text(self.current_details_game or {}, save_type)
        return "Manage States"

    def _show_details_cloud_loading_state(self, save_type: str) -> None:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start("_show_details_cloud_loading_state", save_type=save_type) if callable(timing_start) else 0.0
        if save_type not in {"save", "state"}:
            if callable(timing_end):
                timing_end("_show_details_cloud_loading_state", started_at, result="ignored-invalid")
            return

        kind_label = "saves" if save_type == "save" else "states"
        title_text = self._details_cloud_active_button_text(save_type)

        if self.details_center_stack is not None:
            self.details_center_stack.setCurrentIndex(1)
        if self.details_cloud_title_label is not None:
            self.details_cloud_title_label.setText(title_text)
        if self.details_cloud_upload_button is not None:
            if save_type == "save" and title_text == "Emulator Saves":
                self.details_cloud_upload_button.setText("Upload Emulator Saves")
            else:
                self.details_cloud_upload_button.setText(
                    "Upload Latest Save" if save_type == "save" else "Upload Latest State"
                )
            self.details_cloud_upload_button.setEnabled(False)
            self.details_cloud_upload_button.setToolTip("")
        if self.details_cloud_status_label is not None:
            self.details_cloud_status_label.setText(f"Loading cloud {kind_label}...")
        if self.details_cloud_empty_label is not None and self.details_cloud_list_layout is not None:
            self._clear_layout_items(self.details_cloud_list_layout)
            self.details_cloud_empty_label.setText(f"Loading cloud {kind_label} from the server...")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
            self.details_cloud_list_layout.addStretch()
        if callable(timing_end):
            timing_end("_show_details_cloud_loading_state", started_at, title=title_text)

    def _toggle_details_cloud_mode(self, save_type: str) -> None:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start(
            "_toggle_details_cloud_mode",
            save_type=save_type,
            current_mode=self.current_details_cloud_mode,
        ) if callable(timing_start) else 0.0
        if save_type not in {"save", "state"}:
            if callable(timing_end):
                timing_end("_toggle_details_cloud_mode", started_at, result="ignored-invalid")
            return
        if self.current_details_cloud_mode == save_type:
            self._show_details_overview()
            self._update_details_layout_metrics()
            if callable(timing_end):
                timing_end("_toggle_details_cloud_mode", started_at, result="returned-overview")
            return
        if self.current_details_game is None or not self._details_cloud_button_visible(save_type):
            if callable(timing_end):
                timing_end("_toggle_details_cloud_mode", started_at, result="unsupported")
            return

        self.current_details_cloud_mode = save_type
        if self.details_details_button is not None:
            self.details_details_button.setChecked(False)
        if self.details_manage_saves_button is not None:
            self.details_manage_saves_button.setChecked(save_type == "save")
        if self.details_manage_states_button is not None:
            self.details_manage_states_button.setChecked(save_type == "state")
        details_achievements_button = getattr(self, "details_achievements_button", None)
        if details_achievements_button is not None:
            details_achievements_button.setChecked(False)

        self._show_details_cloud_loading_state(save_type)
        self._update_details_layout_metrics()
        QTimer.singleShot(25, self._refresh_details_cloud_panel)
        QTimer.singleShot(0, self._update_details_layout_metrics)
        if callable(timing_end):
            timing_end("_toggle_details_cloud_mode", started_at, result="scheduled-refresh")

    def _perform_current_cloud_upload_action(self) -> None:
        if self.current_details_cloud_mode == "save":
            self._perform_upload_saves_action()
        elif self.current_details_cloud_mode == "state":
            self._perform_upload_states_action()

    def _clear_layout_items(self, layout: Any) -> None:
        while layout.count():
            item = layout.takeAt(0)
            child_layout = item.layout()
            widget = item.widget()
            if child_layout is not None:
                self._clear_layout_items(child_layout)
            if widget is not None:
                if widget is self.details_cloud_empty_label:
                    widget.setParent(None)
                else:
                    widget.deleteLater()

    def _details_cloud_record_title(self, record: dict[str, Any], save_type: str) -> str:
        file_name = str(record.get("file_name", "")).strip()
        if file_name:
            return file_name
        record_id = str(record.get("id", "")).strip() or "?"
        return f"Cloud {'Save' if save_type == 'save' else 'State'} #{record_id}"

    def _details_cloud_uploaded_text(self, record: dict[str, Any]) -> tuple[str, str]:
        timestamp = self._save_record_timestamp(record)
        if timestamp <= 0:
            return "Unknown upload time", "Unknown"
        uploaded_at = datetime.fromtimestamp(timestamp).astimezone().strftime("%Y-%m-%d %H:%M")
        return uploaded_at, relative_timestamp_text(timestamp)

    def _details_cloud_restore_enabled(self, record: dict[str, Any], save_type: str) -> tuple[bool, str]:
        if self.current_details_game is None:
            return False, "No game is selected."

        target_game = self._installed_game_record(self.current_details_game)
        resolved_game = target_game if target_game is not None else self.current_details_game
        if resolved_game is None:
            return False, "No game is selected."

        resolved_emulator_name, resolved_emulator_entry = self._resolved_emulator_entry_for_game(resolved_game)
        record_emulator = str(record.get("emulator", "")).strip()
        compatibility_emulator_name = record_emulator or resolved_emulator_name
        cache_key = (
            save_type,
            self._game_key(resolved_game),
            compatibility_emulator_name.casefold(),
        )
        request_context = self.details_cloud_request_context if isinstance(self.details_cloud_request_context, dict) else {}
        restore_cache = request_context.setdefault("restore_enabled_cache", {}) if isinstance(request_context, dict) else {}
        cached_result = restore_cache.get(cache_key)
        if isinstance(cached_result, tuple) and len(cached_result) == 2:
            return bool(cached_result[0]), str(cached_result[1])

        compatibility_emulator_entry = resolved_emulator_entry
        record_emulator_entry = self._emulator_entry_by_name(record_emulator) if record_emulator else None
        if record_emulator_entry is not None:
            compatibility_emulator_entry = record_emulator_entry
        compatibility_reason = self._cloud_save_block_reason_for_game(
            resolved_game,
            compatibility_emulator_name,
            compatibility_emulator_entry,
        )
        if compatibility_reason:
            result = (False, compatibility_reason)
            restore_cache[cache_key] = result
            return result

        shared_notice = self._details_cloud_scope_notice(
            resolved_game,
            compatibility_emulator_name,
            compatibility_emulator_entry,
            save_type=save_type,
        )

        if save_type == "state" and record_emulator and self._is_rpcs3_emulator_name(record_emulator, record_emulator_entry):
            result = (False, "RPCS3 savestate restore is not supported yet.")
            restore_cache[cache_key] = result
            return result

        emulator_name = record_emulator
        emulator_entry = record_emulator_entry
        if record_emulator and emulator_entry is None:
            result = (False, f"Configure emulator '{record_emulator}' in Emulators to restore this entry.")
            restore_cache[cache_key] = result
            return result

        if emulator_entry is None:
            emulator_name, emulator_entry = self._resolved_emulator_entry_for_game(resolved_game)
            if emulator_entry is None:
                result = (False, "No default emulator is configured for this platform.")
                restore_cache[cache_key] = result
                return result

        directory_key = "save_paths" if save_type == "save" else "state_paths"
        directories = self._resolved_sync_directory_paths(emulator_entry, directory_key)
        if not directories:
            kind_label = "save" if save_type == "save" else "state"
            result = (False, f"No configured {kind_label} directories were found for emulator '{emulator_name}'.")
            restore_cache[cache_key] = result
            return result

        result = (True, shared_notice)
        restore_cache[cache_key] = result
        return result

    def _make_details_cloud_record_widget(self, record: dict[str, Any], save_type: str) -> QWidget:
        entry = QFrame()
        entry.setObjectName("detailsCloudRecord")
        entry.setFrameShape(QFrame.Shape.StyledPanel)
        entry.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
        entry.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        entry_layout = QHBoxLayout(entry)
        entry_layout.setContentsMargins(12, 10, 12, 10)
        entry_layout.setSpacing(12)

        info_layout = QVBoxLayout()
        info_layout.setSpacing(4)

        title_label = QLabel(self._details_cloud_record_title(record, save_type))
        title_label.setStyleSheet("font-size: 16px; font-weight: 600;")
        info_layout.addWidget(title_label)

        emulator_name = str(record.get("emulator", "")).strip() or "Unknown emulator"
        slot_value = str(record.get("slot", "")).strip()
        size_value = record.get("file_size_bytes", 0)
        try:
            size_text = format_size(int(size_value))
        except (TypeError, ValueError):
            size_text = "Unknown size"

        summary_parts = [emulator_name, size_text]
        if save_type == "save" and slot_value:
            summary_parts.append(f"Slot {slot_value}")
        summary_label = QLabel(" • ".join(summary_parts))
        summary_label.setStyleSheet(f"color: {self._theme_color('muted', '#6272a4')};")
        info_layout.addWidget(summary_label)

        uploaded_at, relative_text = self._details_cloud_uploaded_text(record)
        time_label = QLabel(f"Uploaded {uploaded_at} ({relative_text})")
        time_label.setStyleSheet(f"color: {self._theme_color('muted', '#6272a4')};")
        info_layout.addWidget(time_label)

        entry_layout.addLayout(info_layout, 1)

        actions_layout = QHBoxLayout()
        actions_layout.setSpacing(8)

        restore_button = QPushButton("Restore")
        restore_button.setObjectName("detailsCloudActionButton")
        restore_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        restore_enabled, restore_tooltip = self._details_cloud_restore_enabled(record, save_type)
        restore_button.setEnabled(restore_enabled)
        restore_button.setToolTip(restore_tooltip)
        restore_button.clicked.connect(
            lambda checked=False, payload=dict(record), kind=save_type: self._confirm_restore_details_cloud_record(
                payload,
                kind,
            )
        )
        actions_layout.addWidget(restore_button)

        delete_button = QPushButton("Delete")
        delete_button.setObjectName("detailsCloudActionButton")
        delete_button.setProperty("role", "danger")
        delete_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        delete_button.clicked.connect(
            lambda checked=False, payload=dict(record), kind=save_type: self._confirm_delete_details_cloud_record(
                payload,
                kind,
            )
        )
        actions_layout.addWidget(delete_button)

        entry_layout.addLayout(actions_layout)

        for widget in (entry, restore_button, delete_button):
            style = widget.style()
            if style is not None:
                style.unpolish(widget)
                style.polish(widget)
            widget.update()

        return entry

    def _start_details_cloud_records_worker(
        self,
        rom_id: str,
        save_type: str,
        *,
        kind_label: str,
        upload_reason: str,
        emulator_name: str,
    ) -> None:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start("_start_details_cloud_records_worker", save_type=save_type, rom_id=rom_id) if callable(timing_start) else 0.0
        self.details_cloud_request_id += 1
        request_id = self.details_cloud_request_id
        self.details_cloud_request_context = {
            "request_id": request_id,
            "save_type": save_type,
            "kind_label": kind_label,
            "upload_reason": upload_reason,
            "emulator_name": emulator_name,
        }

        thread = QThread(self)
        worker = DetailsCloudRecordsWorker(self, request_id, rom_id, save_type)
        worker.moveToThread(thread)

        thread.started.connect(worker.run)
        worker.finished.connect(self._on_details_cloud_records_loaded)
        worker.finished.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(lambda t=thread, w=worker: self._cleanup_details_cloud_worker(t, w))

        self.details_cloud_threads.append(thread)
        self.details_cloud_workers.append(worker)
        QTimer.singleShot(0, thread.start)
        if callable(timing_end):
            timing_end("_start_details_cloud_records_worker", started_at, request_id=request_id)

    def _cleanup_details_cloud_worker(self, thread: QThread, worker: DetailsCloudRecordsWorker) -> None:
        self.details_cloud_threads = [item for item in self.details_cloud_threads if item is not thread]
        self.details_cloud_workers = [item for item in self.details_cloud_workers if item is not worker]

    def _on_details_cloud_records_loaded(
        self,
        request_id: int,
        save_type: str,
        records: object,
        error: str,
    ) -> None:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start("_on_details_cloud_records_loaded", request_id=request_id, save_type=save_type) if callable(timing_start) else 0.0
        if (
            self.details_cloud_status_label is None
            or self.details_cloud_empty_label is None
            or self.details_cloud_list_layout is None
        ):
            if callable(timing_end):
                timing_end("_on_details_cloud_records_loaded", started_at, result="missing-ui")
            return
        if request_id != self.details_cloud_request_id:
            if callable(timing_end):
                timing_end("_on_details_cloud_records_loaded", started_at, result="stale-request")
            return
        if self.current_details_cloud_mode != save_type:
            if callable(timing_end):
                timing_end("_on_details_cloud_records_loaded", started_at, result="mode-changed")
            return

        request_context = self.details_cloud_request_context
        if int(request_context.get("request_id", -1)) != request_id:
            if callable(timing_end):
                timing_end("_on_details_cloud_records_loaded", started_at, result="context-mismatch")
            return

        kind_label = str(request_context.get("kind_label", "saves" if save_type == "save" else "states"))
        upload_reason = str(request_context.get("upload_reason", "")).strip()
        emulator_name = str(request_context.get("emulator_name", "")).strip()

        self._clear_layout_items(self.details_cloud_list_layout)

        if error:
            self.details_cloud_status_label.setText(f"Could not load cloud {kind_label}: {error}")
            self.details_cloud_empty_label.setText(f"Cloud {kind_label} could not be loaded right now.")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
            self.details_cloud_list_layout.addStretch()
            if callable(timing_end):
                timing_end("_on_details_cloud_records_loaded", started_at, result="error", message=error)
            return

        ordered_records = sort_server_records_by_recency(
            [item for item in records if isinstance(item, dict)] if isinstance(records, list) else [],
            self._save_record_timestamp,
        )
        if ordered_records:
            status_parts = [f"Showing {len(ordered_records)} cloud {kind_label}."]
            if upload_reason:
                status_parts.append(upload_reason)
            elif emulator_name:
                status_parts.append(f"Local uploads use {emulator_name}.")
            self.details_cloud_status_label.setText(" ".join(status_parts))
            for record in ordered_records:
                self.details_cloud_list_layout.addWidget(self._make_details_cloud_record_widget(record, save_type))
        else:
            self.details_cloud_status_label.setText(
                upload_reason if upload_reason else f"No cloud {kind_label} were found for this game yet."
            )
            self.details_cloud_empty_label.setText(f"No cloud {kind_label} were found on the server for this game.")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)

        self.details_cloud_list_layout.addStretch()
        if callable(timing_end):
            timing_end("_on_details_cloud_records_loaded", started_at, result="rendered", count=len(ordered_records))

    def _refresh_details_cloud_panel(self) -> None:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start("_refresh_details_cloud_panel", mode=self.current_details_cloud_mode) if callable(timing_start) else 0.0
        if (
            self.details_center_stack is None
            or self.details_cloud_title_label is None
            or self.details_cloud_status_label is None
            or self.details_cloud_empty_label is None
            or self.details_cloud_upload_button is None
            or self.details_cloud_list_layout is None
        ):
            if callable(timing_end):
                timing_end("_refresh_details_cloud_panel", started_at, result="missing-ui")
            return

        save_type = self.current_details_cloud_mode
        if save_type not in {"save", "state"}:
            if callable(timing_end):
                timing_end("_refresh_details_cloud_panel", started_at, result="overview")
            return

        kind_label = "saves" if save_type == "save" else "states"
        singular_label = "save" if save_type == "save" else "state"
        self.details_center_stack.setCurrentIndex(1)

        current_game = self.current_details_game
        title_text = self._details_cloud_active_button_text(save_type) if current_game is not None else (
            "Manage Saves" if save_type == "save" else "Manage States"
        )
        self.details_cloud_title_label.setText(title_text)
        if save_type == "save" and title_text == "Emulator Saves":
            self.details_cloud_upload_button.setText("Upload Emulator Saves")
        else:
            self.details_cloud_upload_button.setText("Upload Latest Save" if save_type == "save" else "Upload Latest State")

        self._clear_layout_items(self.details_cloud_list_layout)

        if current_game is None:
            self.details_cloud_status_label.setText("No game is selected.")
            self.details_cloud_upload_button.setEnabled(False)
            self.details_cloud_upload_button.setToolTip("")
            self.details_cloud_empty_label.setText("Choose a game to view its cloud saves or states.")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
            self.details_cloud_list_layout.addStretch()
            return

        installed_game = self._installed_game_record(current_game)
        if self._is_native_executable_platform(current_game):
            self._refresh_native_save_panel(current_game, save_type)
            return
        target_game = installed_game if installed_game is not None else current_game
        emulator_name, emulator_entry = self._resolved_cloud_emulator_entry_for_game(
            target_game,
            save_type=save_type,
        )
        shared_emulator_view = bool(
            save_type == "save"
            and self._is_emulators_platform(target_game)
            and emulator_entry is not None
            and self._cloud_save_scope_for_game(
                target_game,
                emulator_name,
                emulator_entry,
                save_type=save_type,
            ) != "per-game"
        )
        if installed_game is None and not shared_emulator_view:
            self.details_cloud_status_label.setText(f"Install this game to manage cloud {kind_label}.")
            self.details_cloud_upload_button.setEnabled(False)
            self.details_cloud_upload_button.setToolTip("")
            self.details_cloud_empty_label.setText(f"Cloud {kind_label} can be managed after the game is installed.")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
            self.details_cloud_list_layout.addStretch()
            return

        compatibility_reason = self._cloud_save_block_reason_for_game(
            target_game,
            emulator_name,
            emulator_entry,
            save_type=save_type,
        )
        if compatibility_reason:
            self.details_cloud_status_label.setText(compatibility_reason)
            self.details_cloud_upload_button.setEnabled(False)
            self.details_cloud_upload_button.setToolTip(compatibility_reason)
            self.details_cloud_empty_label.setText(compatibility_reason)
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
            self.details_cloud_list_layout.addStretch()
            return
        upload_reason = self._details_cloud_scope_notice(
            target_game,
            emulator_name,
            emulator_entry,
            save_type=save_type,
        )
        upload_enabled = True
        if emulator_entry is None:
            upload_enabled = False
            upload_reason = "No default emulator is configured for this platform."
        else:
            directory_key = "save_paths" if save_type == "save" else "state_paths"
            if save_type == "state" and self._is_rpcs3_emulator_name(emulator_name, emulator_entry):
                upload_enabled = False
                upload_reason = "RPCS3 savestate uploads are not supported yet."
            elif not self._resolved_sync_directory_paths(emulator_entry, directory_key):
                upload_enabled = False
                upload_reason = (
                    f"No configured {singular_label} directories were found for emulator '{emulator_name}'."
                )

        self.details_cloud_upload_button.setEnabled(upload_enabled)
        self.details_cloud_upload_button.setToolTip(upload_reason)

        rom_id = self._cloud_sync_rom_id_for_game(
            target_game,
            save_type=save_type,
            emulator_name=emulator_name,
            emulator=emulator_entry,
        )
        if not rom_id:
            self.details_cloud_status_label.setText("Missing ROM id for this game.")
            self.details_cloud_empty_label.setText(f"Cloud {kind_label} could not be loaded for this game.")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
            self.details_cloud_list_layout.addStretch()
            return

        self.details_cloud_status_label.setText(f"Loading cloud {kind_label}...")
        self.details_cloud_empty_label.setText(f"Loading cloud {kind_label} from the server...")
        self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
        self.details_cloud_list_layout.addStretch()

        self._start_details_cloud_records_worker(
            rom_id,
            save_type,
            kind_label=kind_label,
            upload_reason=upload_reason,
            emulator_name=emulator_name,
        )
        if callable(timing_end):
            timing_end("_refresh_details_cloud_panel", started_at, result="worker-started", rom_id=rom_id)

    def _refresh_native_save_panel(self, game: dict, save_type: str) -> None:
        """Populate the cloud panel for a native Windows game using PCGamingWiki paths."""
        if save_type != "save":
            self.details_cloud_status_label.setText("Save states are not supported for native games.")
            self.details_cloud_upload_button.setEnabled(False)
            self.details_cloud_upload_button.setToolTip("")
            self.details_cloud_empty_label.setText("Only save file backups are supported for native games.")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
            self.details_cloud_list_layout.addStretch()
            return

        cached = self._pcgw_paths_for_game(game)

        if cached is None:
            # Not yet fetched - show loading state and kick off background lookup
            self.details_cloud_status_label.setText("Looking up save locations on PCGamingWiki…")
            self.details_cloud_upload_button.setEnabled(False)
            self.details_cloud_upload_button.setToolTip("Waiting for save location lookup…")
            self.details_cloud_empty_label.setText("Fetching save locations from PCGamingWiki…")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
            self.details_cloud_list_layout.addStretch()
            self._start_pcgw_lookup_for_game(game)
            return

        # Build a combined list: PCGW paths + any manually added paths
        key = self._pcgw_cache_key(game)
        manual_key = key + "__manual"
        manual_paths: list[str] = self._pcgw_paths_cache.get(manual_key, [])
        all_raw_paths = list(cached) + [p for p in manual_paths if p not in cached]

        # Show the path list (or empty state)
        if not all_raw_paths:
            self.details_cloud_status_label.setText("No save locations found on PCGamingWiki.")
            self.details_cloud_upload_button.setEnabled(False)
            self.details_cloud_upload_button.setToolTip("Add a save location to enable uploads.")
        else:
            self.details_cloud_status_label.setText(f"{len(all_raw_paths)} save location(s) configured.")
            rom_id = self._cloud_sync_rom_id_for_game(game)
            self.details_cloud_upload_button.setEnabled(bool(rom_id))
            self.details_cloud_upload_button.setToolTip(
                "Upload save files from the listed locations." if rom_id else "Missing ROM id for this game."
            )

        # Render path rows
        import os
        from pathlib import Path

        for raw_path in all_raw_paths:
            expanded = os.path.expandvars(raw_path)
            p = Path(expanded)
            row = QWidget()
            row_layout = QHBoxLayout(row)
            row_layout.setContentsMargins(0, 4, 0, 4)
            label = QLabel(raw_path)
            label.setWordWrap(True)
            label.setToolTip(expanded)
            row_layout.addWidget(label, 1)
            remove_btn = QPushButton("✕")
            remove_btn.setFixedWidth(28)
            remove_btn.setToolTip("Remove this path")
            raw_path_capture = raw_path

            def _remove(checked=False, rp=raw_path_capture):
                self._pcgw_remove_path_for_game(game, rp)
                self._refresh_details_cloud_panel()

            remove_btn.clicked.connect(_remove)
            row_layout.addWidget(remove_btn)
            self.details_cloud_list_layout.addWidget(row)

        # Browse button (always shown so users can add paths)
        browse_btn = QPushButton("Browse…")
        browse_btn.setToolTip("Add a custom save folder for this game")

        def _browse(checked=False):
            from PySide6.QtWidgets import QFileDialog

            folder = QFileDialog.getExistingDirectory(self, "Select Save Folder")
            if folder:
                self._pcgw_add_manual_path_for_game(game, folder)
                self._refresh_details_cloud_panel()

        browse_btn.clicked.connect(_browse)
        self.details_cloud_list_layout.addWidget(browse_btn)

        if not all_raw_paths:
            self.details_cloud_empty_label.setText(
                "No save locations were found automatically. Use Browse to add one."
            )
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)

        self.details_cloud_list_layout.addStretch()

    def _pcgw_add_manual_path_for_game(self, game: dict, folder: str) -> None:
        key = self._pcgw_cache_key(game) + "__manual"
        existing = self._pcgw_paths_cache.get(key, [])
        if folder not in existing:
            self._pcgw_paths_cache[key] = existing + [folder]

    def _pcgw_remove_path_for_game(self, game: dict, raw_path: str) -> None:
        key = self._pcgw_cache_key(game)
        manual_key = key + "__manual"
        # Remove from PCGW list
        current = self._pcgw_paths_cache.get(key, [])
        self._pcgw_paths_cache[key] = [p for p in current if p != raw_path]
        # Remove from manual list
        manual = self._pcgw_paths_cache.get(manual_key, [])
        self._pcgw_paths_cache[manual_key] = [p for p in manual if p != raw_path]

    def _confirm_restore_details_cloud_record(self, record: dict[str, Any], save_type: str) -> None:
        if self.current_details_game is None:
            return

        installed_game = self._installed_game_record(self.current_details_game)
        target_game = installed_game if installed_game is not None else self.current_details_game
        kind_label = "save" if save_type == "save" else "state"
        game_title = str(target_game.get("title", "this game"))
        shared_notice = self._details_cloud_scope_notice(target_game, save_type=save_type)
        dialog_text = f"Restore the selected cloud {kind_label} for '{game_title}' and overwrite the local {kind_label} data?"
        if shared_notice:
            dialog_text = f"{dialog_text}\n\nWarning: {shared_notice}"
        response = QMessageBox.question(
            self,
            f"Restore Cloud {kind_label.title()}",
            dialog_text,
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.No,
        )
        if response != QMessageBox.StandardButton.Yes:
            return

        restored = False
        if save_type == "save":
            restored = self._restore_cloud_save_for_game(target_game, save_record=dict(record))
        else:
            restored = self._restore_cloud_state_for_game(target_game, state_record=dict(record))

        if restored:
            self._refresh_details_cloud_panel()

    def _confirm_delete_details_cloud_record(self, record: dict[str, Any], save_type: str) -> None:
        title = self._details_cloud_record_title(record, save_type)
        kind_label = "save" if save_type == "save" else "state"
        context_game = self._installed_game_record(self.current_details_game) if self.current_details_game is not None else None
        if context_game is None and self.current_details_game is not None:
            context_game = self.current_details_game
        shared_notice = self._details_cloud_scope_notice(context_game or {}, save_type=save_type)
        dialog_text = f"Delete '{title}' from the server? This cannot be undone."
        if shared_notice:
            dialog_text = f"{dialog_text}\n\nWarning: {shared_notice}"
        response = QMessageBox.question(
            self,
            f"Delete Cloud {kind_label.title()}",
            dialog_text,
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.No,
        )
        if response != QMessageBox.StandardButton.Yes:
            return

        if self._delete_cloud_record(record, save_type):
            self._refresh_details_cloud_panel()

    def _delete_cloud_record(self, record: dict[str, Any], save_type: str) -> bool:
        record_id = str(record.get("id", "")).strip()
        if not record_id:
            QMessageBox.warning(self, "Cloud Sync", "The selected cloud entry is missing an id.")
            return False

        try:
            numeric_id = int(record_id)
        except (TypeError, ValueError):
            QMessageBox.warning(self, "Cloud Sync", f"The selected cloud entry id is invalid: {record_id}")
            return False

        endpoint_path = "/api/saves/delete" if save_type == "save" else "/api/states/delete"
        payload_key = "saves" if save_type == "save" else "states"
        kind_label = "save" if save_type == "save" else "state"
        try:
            self._api_post_json(endpoint_path, {payload_key: [numeric_id]})
        except HTTPError as error:
            if error.code not in {404, 410}:
                QMessageBox.warning(self, "Cloud Sync", f"Failed to delete cloud {kind_label}: {error}")
                return False
        except (URLError, OSError, ValueError, json.JSONDecodeError) as error:
            QMessageBox.warning(self, "Cloud Sync", f"Failed to delete cloud {kind_label}: {error}")
            return False

        QMessageBox.information(self, "Cloud Sync", f"Cloud {kind_label} deleted successfully.")
        return True

    def _mapping_value_for_platform(self, mapping: dict[str, str], platform: str) -> str:
        return resolve_mapping_value_for_platform(mapping, platform)

    def _default_emulator_name_for_platform(self, platform: str) -> str:
        emulators = self._normalize_emulators(self._emulators())
        compatible = resolve_compatible_emulator_names_for_platform(
            emulators,
            platform,
            self._emulator_supports_platform,
        )
        return resolve_default_emulator_name_for_platform(
            platform,
            self._normalize_default_emulators(self.config.get("default_emulators", {})),
            emulators,
            self._emulator_supports_platform,
            compatible,
        )

    def _emulator_entry_has_usable_path(self, emulator: dict[str, str]) -> bool:
        return resolve_emulator_entry_has_usable_path(emulator)

    def _available_emulator_name_for_platform(self, platform: str) -> str:
        emulators = self._normalize_emulators(self._emulators())
        default_name = self._default_emulator_name_for_platform(platform)
        return resolve_available_emulator_name_for_platform(
            platform,
            emulators,
            self._emulator_supports_platform,
            default_name,
        )

    def _install_block_reason_for_game(self, game: dict[str, str]) -> str:
        return resolve_install_block_reason_for_game(
            game,
            self._is_native_executable_platform,
            self._is_emulators_platform,
            self._available_emulator_name_for_platform,
        )

    def _emulator_entry_by_name(self, emulator_name: str) -> dict[str, str] | None:
        return resolve_emulator_entry_by_name(self._normalize_emulators(self._emulators()), emulator_name)

    def _launch_placeholders_for_game(self, game: dict[str, str], emulator_name: str) -> dict[str, str]:
        rom_path = self._resolved_rom_path_for_game(game)
        platform_value = game.get("platform", "")
        platform = platform_value.strip() if isinstance(platform_value, str) else ""
        core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
        core_value = resolve_retroarch_core_value(
            emulator_name,
            platform,
            core_defaults,
            self._is_retroarch_emulator_name,
            self._mapping_value_for_platform,
            self._retroarch_core_argument_path,
        )
        return build_launch_placeholders_for_game(
            rom_path,
            emulator_name,
            core_value,
            self._is_rpcs3_emulator_name,
            self._ps3_game_id_for_game(game),
        )

    def _retroarch_core_argument_path(self, configured_core: str) -> str:
        return resolve_retroarch_core_argument_path(configured_core)

    def _strip_wrapping_quotes(self, token: str) -> str:
        return resolve_strip_wrapping_quotes(token)

    def _apply_launch_placeholders_to_args(self, args: list[str], placeholders: dict[str, str]) -> list[str]:
        return resolve_apply_launch_placeholders_to_args(args, placeholders)

    def _split_launch_template_args(self, template: str) -> list[str]:
        return resolve_split_launch_template_args(template)

    def _resolved_launch_arguments_for_game(self, game: dict[str, str]) -> tuple[str, list[str]]:
        return resolve_launch_arguments_for_game(
            game,
            self.config.get("launch_args", ""),
            self._default_emulator_name_for_platform,
            self._emulator_entry_by_name,
            self._split_launch_template_args,
            self._launch_placeholders_for_game,
            resolve_validate_launch_placeholders,
            self._apply_launch_placeholders_to_args,
        )

    def _resolved_rom_path_for_game(self, game: dict[str, str]) -> str:
        return resolve_rom_path_for_game(
            game,
            self._is_arcade_platform,
            self._candidate_extracted_paths_for_game,
            self._candidate_archive_paths_for_game,
        )

    def _normalized_retroarch_core_args(self, emulator_dir: Path, args: list[str]) -> list[str]:
        return resolve_normalized_retroarch_core_args(emulator_dir, args)

    def _launch_installed_game(self, game: dict[str, str]) -> bool:
        try:
            if self._is_native_executable_platform(game):
                command, working_directory = resolve_prepare_native_launch_command(
                    game,
                    self._resolved_native_executable_path_for_game,
                    self._split_launch_template_args,
                )
                process = subprocess.Popen(command, cwd=working_directory)
                QTimer.singleShot(500, lambda p=process, c=command: self._warn_if_process_exited_early(p, c))
                return True

            emulator_name, command, working_directory = resolve_prepare_emulator_launch_command(
                game,
                self._default_emulator_name_for_platform,
                self._emulator_entry_by_name,
                self._resolved_rom_path_for_game,
                self._resolved_launch_arguments_for_game,
                self._is_retroarch_emulator_name,
                self._normalized_retroarch_core_args,
            )
            emulator_entry = self._emulator_entry_by_name(emulator_name)
            if emulator_entry is not None:
                emulator_path_value = emulator_entry.get("path", "")
                if isinstance(emulator_path_value, str):
                    self._ensure_emulator_sync_settings(emulator_name, emulator_path_value)
            process = subprocess.Popen(command, cwd=working_directory)
            QTimer.singleShot(500, lambda p=process, c=command: self._warn_if_process_exited_early(p, c))
            self._register_game_session_for_auto_upload(game, process, emulator_name)
            return True
        except ValueError as error:
            QMessageBox.warning(self, "Launch Error", str(error))
            return False
        except OSError as error:
            QMessageBox.warning(self, "Launch Error", f"Failed to launch game:\n{error}")
            return False

    def _warn_if_process_exited_early(self, process: subprocess.Popen, command: list[str]) -> None:
        exit_code = process.poll()
        if exit_code is None:
            return
        QMessageBox.warning(
            self,
            "Launch Error",
            resolve_process_exited_early_message(exit_code, command),
        )

    def _perform_game_action(self) -> None:
        if self.current_details_game is None:
            return
        if self._is_game_installed(self.current_details_game):
            installed_game = self._installed_game_record(self.current_details_game)
            launch_game = installed_game if installed_game is not None else self.current_details_game
            self._auto_sync_before_launch(launch_game)
            self._launch_installed_game(launch_game)
            return

        self._start_async_install(self.current_details_game)

    def _perform_ps4_content_action(self) -> None:
        if self.current_details_game is None:
            return

        block_reason = self._ps4_content_install_block_reason(self.current_details_game)
        if block_reason:
            QMessageBox.warning(self, "Install Blocked", block_reason)
            return

        available_kinds = self._available_ps4_content_kinds_for_game(self.current_details_game)
        if not available_kinds:
            QMessageBox.warning(
                self,
                "Install Error",
                "No PS4 update or DLC content is available for this game from the current server metadata.",
            )
            return

        selected_kind = ""
        if len(available_kinds) == 1:
            selected_kind = available_kinds[0]
        else:
            chooser = QMessageBox(self)
            chooser.setWindowTitle("Install PS4 Content")
            chooser.setText("Choose the PS4 content type to install:")
            update_button = chooser.addButton("Install Update", QMessageBox.ButtonRole.AcceptRole)
            dlc_button = chooser.addButton("Install DLC", QMessageBox.ButtonRole.ActionRole)
            chooser.addButton(QMessageBox.StandardButton.Cancel)
            chooser.exec()
            clicked_button = chooser.clickedButton()
            if clicked_button is update_button:
                selected_kind = "update"
            elif clicked_button is dlc_button:
                selected_kind = "dlc"
            else:
                return

        rom_id = self._resolve_rom_id_for_game(self.current_details_game)
        if not rom_id:
            QMessageBox.warning(self, "Install Error", "Selected game is missing a ROM id and cannot be downloaded.")
            return

        file_ids_by_category = self._ps4_file_ids_by_category_for_game(
            self.current_details_game,
            rom_id,
            allow_payload_lookup=True,
        )
        selected_file_ids = file_ids_by_category.get(selected_kind, [])
        if not selected_file_ids:
            QMessageBox.warning(
                self,
                "Install Error",
                f"No PS4 {selected_kind} files were found for this title in server metadata.",
            )
            return

        install_game = dict(self.current_details_game)
        install_game["rom_id"] = rom_id
        install_game["_install_mode"] = "ps4_content"
        install_game["_ps4_content_kind"] = selected_kind
        install_game["_ps4_file_ids_csv"] = ",".join(str(file_id) for file_id in selected_file_ids)
        install_game["_archive_name_override"] = self._ps4_content_archive_name(install_game, selected_kind)
        self._hydrate_install_game_metadata(install_game, rom_id)

        resolved_file_name = self._resolved_rom_file_name_for_game(install_game, rom_id)
        if not resolved_file_name:
            QMessageBox.warning(
                self,
                "Install Error",
                "Server did not return a usable ROM filename/path for this title. Refresh server metadata and try again.",
            )
            return
        install_game["rom_file_name"] = resolved_file_name
        install_game["ps4_file_ids_by_category"] = json.dumps(
            file_ids_by_category,
            separators=(",", ":"),
            sort_keys=True,
        )

        self._start_async_install(install_game)

    def _perform_game_config_action(self) -> None:
        if self.current_details_game is None:
            return
        installed_game = self._installed_game_record(self.current_details_game)
        if installed_game is None:
            return
        if not self._is_native_executable_platform(self.current_details_game):
            return

        install_dir = self._native_install_dir_for_game(installed_game)
        executable_candidates = self._native_executable_candidates_for_game(installed_game)
        if install_dir is None or not executable_candidates:
            QMessageBox.warning(
                self,
                "Game Settings",
                "No launchable executables were found in this game's install directory.",
            )
            return

        selected_path = installed_game.get("native_executable_path", "")
        selected_executable_path = selected_path.strip() if isinstance(selected_path, str) else ""
        existing_launch_parameters_value = installed_game.get("native_launch_parameters", "")
        existing_launch_parameters = (
            existing_launch_parameters_value.strip() if isinstance(existing_launch_parameters_value, str) else ""
        )

        dialog = NativeGameSettingsDialog(
            self,
            game_title=str(installed_game.get("title", "Game")),
            install_dir=install_dir,
            executable_candidates=executable_candidates,
            selected_executable_path=selected_executable_path,
            existing_launch_parameters=existing_launch_parameters,
            section_title_factory=self._make_section_title,
        )
        if dialog.exec() != int(QDialog.DialogCode.Accepted):
            return

        selected_executable = dialog.selected_executable_path()
        if not selected_executable:
            return

        native_launch_parameters = dialog.launch_parameters()
        installed_game["native_executable_path"] = selected_executable
        installed_game["native_launch_parameters"] = native_launch_parameters
        if self.current_details_game is not None:
            self.current_details_game["native_executable_path"] = selected_executable
            self.current_details_game["native_launch_parameters"] = native_launch_parameters

        if self._debug_prints_enabled():
            try:
                native_args = self._split_launch_template_args(native_launch_parameters)
                debug_command = [selected_executable, *native_args]
                debug_command_text = subprocess.list2cmdline(debug_command)
                print(f"[DEBUG] Saved native launch command: {debug_command_text}")
            except ValueError as error:
                debug_command = [selected_executable, native_launch_parameters]
                debug_command_text = subprocess.list2cmdline(debug_command)
                print(f"[DEBUG] Saved native launch command (parse error): {debug_command_text}")
                print(f"[DEBUG] Native launch parameter parse error: {error}")

        self._persist_installed_games()

    def _perform_game_secondary_action(self) -> None:
        if self.current_details_game is None:
            return
        if not self._is_game_installed(self.current_details_game):
            return

        if self._uninstall_game(self.current_details_game):
            self._update_details_action_buttons()
            return

    def _perform_game_update_action(self) -> None:
        if self.current_details_game is None:
            return
        if not self._is_game_installed(self.current_details_game):
            return
        if not self._details_update_available_for_game(self.current_details_game):
            return

        rom_id = self._resolve_rom_id_for_game(self.current_details_game)
        if rom_id:
            self.current_details_game["rom_id"] = rom_id
            self._cache_rom_id_for_details_game(self.current_details_game, rom_id)

        server_game = self._server_game_for_identity(self.current_details_game, rom_id)
        if server_game is None:
            QMessageBox.warning(
                self,
                "Update Error",
                "A newer server version is no longer available for this game.",
            )
            self._refresh_installed_game_update_state()
            return

        update_game = dict(server_game)
        resolved_rom_id = self._resolve_rom_id_for_game(update_game)
        if not resolved_rom_id:
            QMessageBox.warning(self, "Update Error", "Selected game is missing a ROM id and cannot be updated.")
            return

        update_game["rom_id"] = resolved_rom_id
        native_platform_check = getattr(self, "_is_native_executable_platform", None)
        is_native_platform = (
            native_platform_check(self.current_details_game)
            if callable(native_platform_check)
            else resolve_is_native_executable_platform(self.current_details_game)
        )
        if is_native_platform:
            installed_rec = self._installed_game_record(self.current_details_game)
            if installed_rec is None:
                QMessageBox.warning(self, "Update Error", "Installed game record not found.")
                return
            extracted_dir_text = installed_rec.get("extracted_dir", "").strip()
            if not extracted_dir_text or not Path(extracted_dir_text).is_dir():
                QMessageBox.warning(
                    self,
                    "Update Error",
                    "Game install directory could not be found. Reinstall the game and try again.",
                )
                return
            confirm = QMessageBox.question(
                self,
                "Confirm Update",
                f"This will download the new version of '{update_game.get('title', 'this game')}' "
                "and merge updated files into your install directory.\n\n"
                "Your save files and configuration will be preserved.\n\n"
                "Continue?",
            )
            if confirm != QMessageBox.StandardButton.Yes:
                return
            update_game["_install_mode"] = "native_update"
            update_game["extracted_dir"] = extracted_dir_text
            update_game["native_executable_path"] = installed_rec.get("native_executable_path", "")
            update_game["native_launch_parameters"] = installed_rec.get("native_launch_parameters", "")
        else:
            update_game["_install_mode"] = "update"
        self._hydrate_install_game_metadata(update_game, resolved_rom_id)
        resolved_file_name = self._resolved_rom_file_name_for_game(update_game, resolved_rom_id)
        if not resolved_file_name:
            QMessageBox.warning(
                self,
                "Update Error",
                "Server did not return a usable ROM filename/path for this title. Refresh server metadata and try again.",
            )
            return

        update_game["rom_file_name"] = resolved_file_name
        resolve_sync_install_metadata_to_details_game(
            self.current_details_game,
            update_game,
            game_key=self._game_key,
        )
        self._start_async_install(update_game)

    def _open_achievements_panel(self) -> None:
        game = self._current_details_game
        if game is None:
            return

        from rom_mate.server.retroachievements import resolve_ra_game_id

        ra_username = (
            self.ra_username_input.text().strip()
            if self.ra_username_input is not None
            else self.config.get("retroachievements_username", "")
        )
        ra_api_key = (
            self.ra_api_key_input.text().strip()
            if self.ra_api_key_input is not None
            else self.config.get("retroachievements_api_key", "")
        )

        ra_game_id = resolve_ra_game_id(
            game,
            ra_username,
            ra_api_key,
        )
        if ra_game_id is None:
            self._show_toast("No RetroAchievements ID found for this game.")
            return
        self._load_achievements_for_ra_id(ra_game_id)

    def _load_achievements_for_ra_id(self, ra_game_id: int) -> None:
        from rom_mate.background.workers import RetroAchievementsWorker

        request_id = int(time.time() * 1000) % 1000000
        self._pending_achievements_request_id = request_id

        ra_username = (
            self.ra_username_input.text().strip()
            if self.ra_username_input is not None
            else self.config.get("retroachievements_username", "")
        )
        ra_api_key = (
            self.ra_api_key_input.text().strip()
            if self.ra_api_key_input is not None
            else self.config.get("retroachievements_api_key", "")
        )

        worker = RetroAchievementsWorker(
            request_id,
            ra_game_id,
            ra_username,
            ra_api_key,
        )
        thread = QThread(self)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.finished.connect(self._on_achievements_loaded)
        worker.finished.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.start()

        self._ra_thread = thread
        self._ra_worker = worker

    def _on_achievements_loaded(self, request_id: int, achievements: list, error: str) -> None:
        if request_id != self._pending_achievements_request_id:
            return
        if error:
            self._show_toast(f"Could not load achievements: {error}")
            return

        from rom_mate.ui.game_views import build_achievements_panel

        panel = build_achievements_panel(achievements, load_image_fn=self._queue_cover_load)
        self._show_achievements_panel(panel)

    def _show_achievements_panel(self, panel: QWidget) -> None:
        if self.details_center_stack is None:
            return

        current_panel = self.details_achievements_panel
        if current_panel is not None:
            self.details_center_stack.removeWidget(current_panel)
            current_panel.deleteLater()

        self.details_achievements_panel = panel
        self.details_center_stack.addWidget(panel)
        self.details_center_stack.setCurrentWidget(panel)
        if self.details_details_button is not None:
            self.details_details_button.setChecked(False)
        if self.details_manage_saves_button is not None:
            self.details_manage_saves_button.setChecked(False)
        if self.details_manage_states_button is not None:
            self.details_manage_states_button.setChecked(False)
        details_achievements_button = getattr(self, "details_achievements_button", None)
        if details_achievements_button is not None:
            details_achievements_button.setChecked(True)
        self._update_details_layout_metrics()
        QTimer.singleShot(0, self._update_details_layout_metrics)

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
            directory_settings = resolve_retroarch_directory_settings(emulator.get("path", ""))
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
                azahar_override_paths = resolve_azahar_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                azahar_override_paths = resolve_azahar_state_path_overrides(
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
                dolphin_override_paths = resolve_dolphin_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                dolphin_override_paths = resolve_dolphin_state_path_overrides(
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
                pcsx2_override_paths = resolve_pcsx2_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                pcsx2_override_paths = resolve_pcsx2_state_path_overrides(
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
            rpcs3_override_paths = resolve_rpcs3_save_path_overrides(
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
            cemu_override_paths = resolve_cemu_save_path_overrides(
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
            pico8_override_paths = resolve_pico8_save_path_overrides(
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
                fbneo_override_paths = resolve_fbneo_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                fbneo_override_paths = resolve_fbneo_state_path_overrides(
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
                mame_override_paths = resolve_mame_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                mame_override_paths = resolve_mame_state_path_overrides(
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
            eden_override_paths = resolve_eden_save_path_overrides(
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
                xenia_override_paths = resolve_xenia_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                xenia_override_paths = resolve_xenia_state_path_overrides(
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
                redream_override_paths = resolve_redream_save_path_overrides(
                    emulator.get("path", ""),
                    emulator.get("args", ""),
                    self._split_launch_template_args,
                )
            elif key == "state_paths":
                redream_override_paths = resolve_redream_state_path_overrides(
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
            xemu_override_paths = resolve_xemu_save_path_overrides(
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
            return []

        emulator_path_value = emulator.get("path", "")
        emulator_path = Path(emulator_path_value).expanduser() if isinstance(emulator_path_value, str) else Path()
        emulator_dir = emulator_path.parent if emulator_path_value else Path()

        library_value = self.config.get("library_path", "")
        library_path = Path(library_value).expanduser() if isinstance(library_value, str) and library_value.strip() else Path()
        config_dir = self._config_dir()

        resolved: list[Path] = []
        for raw_path in all_paths:
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
        if suffix in {".state", ".savestate", ".st", ".ss", ".ppst"}:
            return True
        if ".state" in name:
            return True
        if re.search(r"\.\d+\.sav$", name):
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

        if resolve_emulator_entry_matches_tokens(entry, tokens, self._emulator_autoprofiles()):
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
        return self._emulator_matches_tokens(emulator_name, "xemu", emulator=emulator)

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
        return server_records_from_payload(payload)

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
        fallback_dirs = [Path(os.path.expandvars(r)) for r in all_raw]

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
                                    target_root = Path(os.path.expandvars(raw_path))
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
                    restore_dir = Path(os.path.expandvars(raw_path))
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

        try:
            ignore_basenames = self._resolved_ignore_basenames_for_emulator(emulator_entry)
            ignore_extensions = self._resolved_ignore_extensions_for_emulator(emulator_entry)
            restored_file = self._restore_single_state_file(
                game,
                directories,
                state_record,
                payload,
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

        success_count = 0
        failed_files: list[str] = []

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
            print(
                f"[DEBUG][CloudSync] Upload {status} save_type={save_type} title={game.get('title', '')} "
                f"rom_id={rom_id} emulator={emulator_name} uploaded={success_count}/{len(upload_jobs)} "
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
        return success_count, len(upload_jobs), failed_files

    def _upload_native_saves_for_game(
        self,
        game: dict,
        *,
        show_dialogs: bool = True,
    ) -> tuple[int, int, list[str]]:
        import io
        import json as _json
        import os
        import tempfile
        import zipfile as _zipfile
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

        # Build dir_map: only include directories that currently exist
        dir_map: list[tuple[str, Path]] = []
        for raw in all_raw:
            expanded = Path(os.path.expandvars(raw))
            if expanded.exists():
                dir_map.append((raw, expanded))

        if not dir_map:
            show_warning("None of the configured save locations exist on this device yet.")
            return 0, 0, []

        # Build a single combined zip archive
        safe_title = self._sanitize_path_component(game.get("title", "game"), "save")
        fd, tmp_path = tempfile.mkstemp(prefix=f"rom-mate-{safe_title}-", suffix=".zip")
        os.close(fd)
        archive_path = Path(tmp_path)

        total_files = 0
        manifest: dict[str, str] = {}
        try:
            with _zipfile.ZipFile(archive_path, mode="w", compression=_zipfile.ZIP_DEFLATED) as archive:
                for idx, (raw_path, directory) in enumerate(dir_map):
                    manifest[str(idx)] = raw_path
                    prefix = f"{idx}/"
                    for candidate in sorted(directory.rglob("*"), key=lambda p: str(p).casefold()):
                        if not candidate.is_file():
                            continue
                        try:
                            relative = candidate.relative_to(directory)
                        except ValueError:
                            relative = Path(candidate.name)
                        archive_member = prefix + relative.as_posix()
                        archive.write(candidate, archive_member)
                        total_files += 1
                # Write manifest last so it's easy to find
                manifest_bytes = _json.dumps(manifest, indent=2).encode("utf-8")
                archive.writestr("_rom_mate_dirs.json", manifest_bytes)
        except OSError as exc:
            archive_path.unlink(missing_ok=True)
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

    def _on_auto_cloud_upload_finished(self, game: object, result: object) -> None:
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

    def _normalize_emulators(self, value: Any) -> list[dict[str, str]]:
        return resolve_normalize_emulators(value, self._normalize_save_strategy_value)

    def _normalize_default_emulators(self, value: Any) -> dict[str, str]:
        return resolve_normalize_default_emulators(value)

    def _normalize_default_retroarch_cores(self, value: Any) -> dict[str, str]:
        return resolve_normalize_default_retroarch_cores(value)

    def _normalize_emulator_source_installs(self, value: Any) -> dict[str, dict[str, str]]:
        if not isinstance(value, dict):
            return {}

        normalized: dict[str, dict[str, str]] = {}
        for raw_key, raw_payload in value.items():
            key = raw_key.strip().casefold() if isinstance(raw_key, str) else ""
            if not key or not isinstance(raw_payload, dict):
                continue

            payload: dict[str, str] = {}
            for field in ("name", "provider", "owner", "repo", "release_tag", "installed_at"):
                raw_field_value = raw_payload.get(field, "")
                payload[field] = raw_field_value.strip() if isinstance(raw_field_value, str) else ""
            normalized[key] = payload
        return normalized

    def _emulator_source_installs(self) -> dict[str, dict[str, str]]:
        installs = self._normalize_emulator_source_installs(self.config.get("emulator_source_installs", {}))
        self.config["emulator_source_installs"] = installs
        return installs

    def _record_source_emulator_install(self, game: dict[str, str]) -> None:
        source_id_value = game.get("_source_id", "")
        source_id = source_id_value.strip().casefold() if isinstance(source_id_value, str) else ""
        if not source_id:
            return

        source_metadata = game.get("_source_metadata")
        if not isinstance(source_metadata, dict):
            return

        provider_value = source_metadata.get("provider", "")
        owner_value = source_metadata.get("owner", "")
        repo_value = source_metadata.get("repo", source_metadata.get("repository", ""))
        installs = self._emulator_source_installs()
        installs[source_id] = {
            "name": str(game.get("title", "")).strip(),
            "provider": provider_value.strip() if isinstance(provider_value, str) else "",
            "owner": owner_value.strip() if isinstance(owner_value, str) else "",
            "repo": repo_value.strip() if isinstance(repo_value, str) else "",
            "release_tag": str(game.get("_source_release_tag", "latest")).strip() or "latest",
            "installed_at": datetime.now(UTC).isoformat(),
        }
        self.config["emulator_source_installs"] = installs
        self._save_config(self.config)

    def _is_duckstation_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "duckstation", emulator=emulator)

    def _ensure_emulator_sync_settings(self, emulator_name: str, emulator_path_text: str) -> None:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start("_ensure_emulator_sync_settings", emulator=emulator_name) if callable(timing_start) else 0.0
        path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
        if not path_text:
            if callable(timing_end):
                timing_end("_ensure_emulator_sync_settings", started_at, result="no-path")
            return
        emulator_entry = {"name": emulator_name, "path": path_text}
        if self._is_retroarch_emulator_name(emulator_name, emulator_entry):
            resolve_ensure_retroarch_save_location_settings(path_text)
        if self._is_duckstation_emulator_name(emulator_name, emulator_entry):
            resolve_ensure_duckstation_memory_card_settings(path_text)
        if callable(timing_end):
            timing_end("_ensure_emulator_sync_settings", started_at, result="done")

    def _is_retroarch_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "retroarch", emulator=emulator)

    def _emulator_autoprofiles_path(self) -> Path:
        return resolve_emulator_autoprofiles_path(Path(__file__).resolve().parent)

    def _default_emulator_autoprofiles(self) -> list[dict[str, Any]]:
        return resolve_default_emulator_autoprofiles()

    def _normalize_emulator_autoprofiles(self, value: Any) -> list[dict[str, Any]]:
        return resolve_normalize_emulator_autoprofiles(
            value,
            self._normalize_save_strategy_value,
            self._normalize_ignore_extension_value,
        )

    def _emulator_autoprofiles(self) -> list[dict[str, Any]]:
        profiles = resolve_emulator_autoprofiles(
            self.emulator_autoprofiles,
            Path(__file__).resolve().parent,
            self._normalize_save_strategy_value,
            self._normalize_ignore_extension_value,
        )
        self.emulator_autoprofiles = profiles
        return profiles

    def _retroarch_core_list_path(self) -> Path:
        return resolve_retroarch_core_list_path(Path(__file__).resolve().parent)

    def _retroarch_markdown_label(self, value: str) -> str:
        return resolve_retroarch_markdown_label(value)

    def _retroarch_core_id_from_name(self, core_name: str) -> str:
        return resolve_retroarch_core_id_from_name(core_name)

    def _retroarch_core_id_from_file_name(self, core_file_name: str) -> str:
        return resolve_retroarch_core_id_from_file_name(core_file_name)

    def _retroarch_compatibility_map_from_markdown(self) -> dict[str, list[str]]:
        if isinstance(self.retroarch_compatibility_map, dict):
            return self.retroarch_compatibility_map

        compatibility = resolve_load_retroarch_compatibility_map(self._retroarch_core_list_path())
        self.retroarch_compatibility_map = compatibility
        return compatibility

    def _normalize_retroarch_platform_key(self, value: str) -> str:
        return resolve_normalize_retroarch_platform_key(value)

    def _retroarch_platform_tokens(self, value: str) -> set[str]:
        return resolve_retroarch_platform_tokens(value)

    def _all_retroarch_cores(self, compatibility: dict[str, list[str]]) -> list[str]:
        return resolve_all_retroarch_cores(compatibility)

    def _retroarch_installed_core_ids_for_emulator(self, emulator_name: str) -> set[str]:
        if not self._is_retroarch_emulator_name(emulator_name):
            return set()

        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None:
            return set()

        emulator_path_value = emulator_entry.get("path", "")
        emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
        return resolve_installed_retroarch_core_ids(emulator_path_text)

    def _installed_retroarch_cores_for_platform(self, platform: str, emulator_name: str) -> list[str]:
        platform_cores = self._retroarch_cores_for_platform(platform)
        installed_core_ids = self._retroarch_installed_core_ids_for_emulator(emulator_name)
        if not installed_core_ids:
            return []
        return [core for core in platform_cores if core in installed_core_ids]

    def _retroarch_system_keys_for_platform(self, platform: str) -> list[str]:
        return resolve_retroarch_system_keys_for_platform(
            platform,
            self._retroarch_compatibility_map_from_markdown(),
        )

    def _retroarch_cores_for_platform(self, platform: str) -> list[str]:
        return resolve_retroarch_cores_for_platform(
            platform,
            self._retroarch_compatibility_map_from_markdown(),
        )

    def _refresh_retroarch_core_options(self) -> None:
        if self.default_core_combo is None or self.default_platform_combo is None or self.default_emulator_combo is None:
            return

        platform = self.default_platform_combo.currentText().strip()
        emulator_name = self.default_emulator_combo.currentText().strip()
        is_retroarch = bool(platform) and self._is_retroarch_emulator_name(emulator_name)

        core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
        self.config["default_retroarch_cores"] = core_defaults
        saved_core = core_defaults.get(platform, "") if platform else ""
        installed_cores = self._installed_retroarch_cores_for_platform(platform, emulator_name)

        self.default_core_combo.blockSignals(True)
        self.default_core_combo.clear()
        if is_retroarch:
            for core in installed_cores:
                self.default_core_combo.addItem(core, core)

        selected_core = selected_retroarch_core(saved_core, installed_cores, is_retroarch)
        if selected_core:
            saved_index = self.default_core_combo.findData(selected_core)
            self.default_core_combo.setCurrentIndex(saved_index if saved_index >= 0 else -1)
        elif emulator_name and not is_retroarch:
            self.default_core_combo.addItem("N/A", "")
            self.default_core_combo.setCurrentIndex(0)
        else:
            self.default_core_combo.setCurrentIndex(-1)

        self.default_core_combo.setEnabled(is_retroarch)
        self.default_core_combo.blockSignals(False)

    def _on_default_platform_changed(self, platform: str) -> None:
        if self.default_emulator_combo is None:
            return

        selected_before_refresh = self.default_emulator_combo.currentText().strip()
        compatible_emulators = self._compatible_emulator_names_for_platform(platform)
        self.default_emulator_combo.blockSignals(True)
        self.default_emulator_combo.clear()
        self.default_emulator_combo.addItems(compatible_emulators)
        self.default_emulator_combo.blockSignals(False)

        defaults = self._normalize_default_emulators(self.config.get("default_emulators", {}))
        self.config["default_emulators"] = defaults
        preferred_emulator = preferred_emulator_selection(
            compatible_emulators,
            self._mapping_value_for_platform(defaults, platform),
            selected_before_refresh,
        )
        if preferred_emulator:
            self.default_emulator_combo.setCurrentText(preferred_emulator)

        self._refresh_retroarch_core_options()

    def _normalize_installed_games(self, value: Any) -> list[dict[str, str]]:
        return resolve_normalize_installed_games(value, self._game_key)

    def _emulators(self) -> list[dict[str, str]]:
        emulators = self.config.get("emulators", [])
        if not isinstance(emulators, list):
            return []
        return emulators

    def _refresh_emulator_views(self) -> None:
        if self.emulator_list is None:
            return
        emulators = self._normalize_emulators(self._emulators())
        self.config["emulators"] = emulators
        action_icon_size = QSize(16, 16)
        theme_color = getattr(self, "_theme_color", None)
        if callable(theme_color):
            accent_color = theme_color("accent", "#8be9fd")
            text_color = theme_color("text", "#f8f8f2")
            error_color = theme_color("error", "#ff5555")
        else:
            accent_color = "#8be9fd"
            text_color = "#f8f8f2"
            error_color = "#ff5555"

        themed_svg_icon = getattr(self, "_themed_svg_icon", None)
        if not callable(themed_svg_icon):
            themed_svg_icon = resolve_themed_svg_icon

        launch_icon = themed_svg_icon(
            "play-1003-svgrepo-com.svg",
            accent_color,
            size=action_icon_size,
        )
        config_icon = themed_svg_icon(
            "gear-tools-wrench-svgrepo-com.svg",
            text_color,
            size=action_icon_size,
        )
        uninstall_icon = themed_svg_icon(
            "trashcan-svgrepo-com.svg",
            error_color,
            size=action_icon_size,
        )
        source_update_icon = themed_svg_icon(
            "save-floppy-svgrepo-com.svg",
            accent_color,
            size=action_icon_size,
        )

        selected_name = ""
        selected_index = self.emulator_list.currentRow()
        if 0 <= selected_index < len(emulators):
            existing_name = emulators[selected_index].get("name", "")
            if isinstance(existing_name, str):
                selected_name = existing_name.strip()

        self.emulator_list.clear()
        for row, entry in enumerate(emulators):
            item = QListWidgetItem()
            row_widget = QWidget()
            row_layout = QHBoxLayout(row_widget)
            row_layout.setContentsMargins(6, 2, 6, 2)
            row_layout.setSpacing(8)

            name_label = QLabel(entry["name"])
            name_label.setStyleSheet("padding: 4px 0;")
            row_layout.addWidget(name_label, 1)

            launch_button = QPushButton()
            launch_button.setObjectName("installedEmulatorLaunchButton")
            launch_button.setToolTip("Launch")
            launch_button.setIcon(launch_icon)
            launch_button.setIconSize(action_icon_size)
            launch_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            launch_button.clicked.connect(lambda checked=False, current_row=row: self._launch_emulator_at_index(current_row))
            row_layout.addWidget(launch_button)

            config_button = QPushButton()
            config_button.setObjectName("installedEmulatorConfigButton")
            config_button.setToolTip("Config")
            config_button.setIcon(config_icon)
            config_button.setIconSize(action_icon_size)
            config_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            config_button.setMinimumHeight(config_button.sizeHint().height() + 2)
            config_button.clicked.connect(
                lambda checked=False, current_row=row: MainWindow._open_emulator_config_dialog_for_row(self, current_row)
            )
            row_layout.addWidget(config_button)

            uninstall_button = QPushButton()
            uninstall_button.setObjectName("installedEmulatorUninstallButton")
            uninstall_button.setToolTip("Uninstall")
            uninstall_button.setIcon(uninstall_icon)
            uninstall_button.setIconSize(action_icon_size)
            uninstall_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            uninstall_button.clicked.connect(lambda checked=False, current_row=row: self._remove_emulator_at_index(current_row))
            row_layout.addWidget(uninstall_button)

            source_entry = self._source_download_entry_for_emulator_name(entry.get("name", ""))
            if source_entry is not None:
                source_update_button = QPushButton()
                source_update_button.setObjectName("installedEmulatorSourceUpdateButton")
                source_update_button.setToolTip("Update from Source")
                source_update_button.setIcon(source_update_icon)
                source_update_button.setIconSize(action_icon_size)
                source_update_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
                source_update_button.clicked.connect(
                    lambda checked=False, current_row=row: self._start_source_emulator_update_at_index(current_row)
                )
                row_layout.addWidget(source_update_button)

            item.setSizeHint(row_widget.sizeHint())
            self.emulator_list.addItem(item)
            self.emulator_list.setItemWidget(item, row_widget)

        if selected_name:
            for row, emulator in enumerate(emulators):
                emulator_name = emulator.get("name", "")
                if isinstance(emulator_name, str) and emulator_name.strip().casefold() == selected_name.casefold():
                    self.emulator_list.setCurrentRow(row)
                    break

        server_platforms = self._default_assignable_server_platforms()
        if self.default_platform_combo is not None:
            selected_platform = self.default_platform_combo.currentText()
            self.default_platform_combo.clear()
            self.default_platform_combo.addItems(server_platforms)
            if selected_platform:
                self.default_platform_combo.setCurrentText(selected_platform)

        if self.default_mapping_list is not None:
            self.default_mapping_list.clear()
            defaults = self._normalize_default_emulators(self.config.get("default_emulators", {}))
            core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
            self.config["default_emulators"] = defaults
            self.config["default_retroarch_cores"] = core_defaults
            for row_text in mapping_list_entries(
                server_platforms,
                defaults,
                core_defaults,
                self._is_retroarch_emulator_name,
            ):
                self.default_mapping_list.addItem(row_text)

        selected_platform = self.default_platform_combo.currentText() if self.default_platform_combo is not None else ""
        self._on_default_platform_changed(selected_platform)

    def _load_emulator_from_selection(self, row: int) -> None:
        if (
            self.emulator_name_input is None
            or self.emulator_path_input is None
            or self.emulator_args_input is None
            or self.emulator_save_strategy_input is None
            or self.emulator_ignore_files_input is None
            or self.emulator_ignore_extensions_input is None
            or self.emulator_save_paths_input is None
            or self.emulator_state_paths_input is None
        ):
            return

        if self.save_emulator_button is not None:
            self.save_emulator_button.setText(save_button_label(self._emulators(), row))

        form_state = emulator_form_state_for_row(
            self._emulators(),
            row,
            self._normalize_save_strategy_value,
        )
        self.emulator_name_input.setText(form_state["name"])
        self.emulator_path_input.setText(form_state["path"])
        self.emulator_args_input.setText(form_state["args"])
        self.emulator_save_strategy_input.setCurrentText(form_state["save_strategy"])
        self.emulator_ignore_files_input.setText(form_state["ignore_files"])
        self.emulator_ignore_extensions_input.setText(form_state["ignore_extensions"])
        self.emulator_save_paths_input.setText(form_state["save_paths"])
        self.emulator_state_paths_input.setText(form_state["state_paths"])

    def _open_add_emulator_dialog(self) -> None:
        self._open_emulator_config_dialog_for_row(-1)

    def _available_source_download_emulator_entries(
        self,
        query: str = "",
        installed_emulator_names: list[str] | None = None,
    ) -> list[dict[str, str]]:
        return resolve_available_source_download_emulator_entries(
            self._emulator_autoprofiles(),
            query=query,
            installed_emulator_names=installed_emulator_names,
        )

    def _source_download_entry_for_emulator_name(self, emulator_name: str) -> dict[str, Any] | None:
        name = emulator_name.strip() if isinstance(emulator_name, str) else ""
        if not name:
            return None

        source_rows = self._available_source_download_emulator_entries(query=name)
        for row in source_rows:
            row_name_value = row.get("name", "")
            row_name = row_name_value.strip() if isinstance(row_name_value, str) else ""
            if row_name and row_name.casefold() == name.casefold():
                return dict(row)

        installs = self._emulator_source_installs()
        for source_id, install_entry in installs.items():
            install_name_value = install_entry.get("name", "")
            install_name = install_name_value.strip() if isinstance(install_name_value, str) else ""
            if not install_name or install_name.casefold() != name.casefold():
                continue

            provider_value = install_entry.get("provider", "")
            owner_value = install_entry.get("owner", "")
            repo_value = install_entry.get("repo", "")
            release_tag_value = install_entry.get("release_tag", "")
            provider = provider_value.strip() if isinstance(provider_value, str) else ""
            owner = owner_value.strip() if isinstance(owner_value, str) else ""
            repo = repo_value.strip() if isinstance(repo_value, str) else ""
            release_tag = release_tag_value.strip() if isinstance(release_tag_value, str) and release_tag_value.strip() else "latest"

            if (not owner or not repo) and isinstance(source_id, str) and "/" in source_id:
                source_owner, _, source_repo = source_id.partition("/")
                if not owner:
                    owner = source_owner.strip()
                if not repo:
                    repo = source_repo.strip()
            if not owner or not repo:
                continue

            source_metadata = {
                "provider": provider or "github",
                "owner": owner,
                "repo": repo,
                "release_tag": release_tag,
            }
            return {
                "name": install_name,
                "provider": source_metadata["provider"],
                "owner": owner,
                "repo": repo,
                "release_tag": release_tag,
                "source_id": f"{owner}/{repo}",
                "source_metadata": source_metadata,
            }

        return None

    def _build_source_emulator_install_game(self, selected: dict[str, Any], install_mode: str) -> dict[str, Any]:
        selected_source_metadata = selected.get("source_metadata")
        if isinstance(selected_source_metadata, dict):
            source_metadata = dict(selected_source_metadata)
        else:
            source_metadata = {
                "provider": selected.get("provider", ""),
                "owner": selected.get("owner", ""),
                "repo": selected.get("repo", ""),
                "release_tag": selected.get("release_tag", "latest"),
            }

        source_release_tag = ""
        for key in ("release_tag", "tag", "version"):
            value = source_metadata.get(key, "")
            if isinstance(value, str) and value.strip():
                source_release_tag = value.strip()
                break
        if not source_release_tag:
            source_release_tag = str(selected.get("release_tag", "latest")).strip() or "latest"

        return {
            "title": selected.get("name", "Emulator").strip() or "Emulator",
            "platform": "Emulators",
            "rating": "N/A",
            "description": "Installed from configured source metadata.",
            "rom_file_name": f"{selected.get('repo', 'source-download')}.zip",
            "_install_mode": install_mode,
            "_source_id": selected.get("source_id", "").strip(),
            "_source_release_tag": source_release_tag,
            "_source_metadata": source_metadata,
            "_archive_name_override": (
                f"{self._sanitize_path_component(selected.get('name', 'emulator'), 'emulator')}"
                f"-{self._sanitize_path_component(source_release_tag, 'latest')}.zip"
            ),
        }

    def _open_source_emulator_download_dialog(self) -> None:
        installed_names = [
            entry.get("name", "")
            for entry in self._normalize_emulators(self._emulators())
            if isinstance(entry, dict)
        ]
        source_rows = self._available_source_download_emulator_entries(
            installed_emulator_names=installed_names,
        )
        if not source_rows:
            QMessageBox.information(
                self,
                "Source Downloads",
                "No supported source emulator downloads are currently available.",
            )
            return

        labels = [
            f"{row['name']} - {row['source_id']} ({row['release_tag']})"
            for row in source_rows
        ]
        selected_label, ok = QInputDialog.getItem(
            self,
            "Download Supported Emulator",
            "Select emulator source:",
            labels,
            0,
            False,
        )
        if not ok:
            return

        selected_index = labels.index(selected_label) if selected_label in labels else -1
        if selected_index < 0:
            return

        selected = source_rows[selected_index]
        install_game = self._build_source_emulator_install_game(selected, "source_emulator")
        self._start_async_install(install_game)

    def _start_source_emulator_update_at_index(self, index: int) -> None:
        emulators = self._normalize_emulators(self._emulators())
        if index < 0 or index >= len(emulators):
            return

        emulator = emulators[index]
        source_entry = self._source_download_entry_for_emulator_name(emulator.get("name", ""))
        if source_entry is None:
            emulator_name = str(emulator.get("name", "Emulator")).strip() or "Emulator"
            QMessageBox.information(
                self,
                "Source Update",
                f"No source metadata is available for '{emulator_name}'.",
            )
            return

        install_game = self._build_source_emulator_install_game(source_entry, "source_emulator_update")
        self._start_async_install(install_game)

    def _extract_emulator_archive(self, emulator_name: str, archive_path: Path) -> tuple[str, str]:
        library_path = self._library_path_dir()
        if library_path is None:
            QMessageBox.warning(self, "Validation", "Set a Library Path in Settings before adding an emulator archive.")
            return "", ""

        extract_target_dir = resolve_emulator_install_directory(library_path, emulator_name)
        try:
            resolve_extract_archive_into_directory(archive_path, extract_target_dir)
        except OSError as error:
            QMessageBox.warning(self, "Archive Error", f"Failed to extract emulator archive: {error}")
            return "", ""

        resolved_path = self._select_emulator_executable_path(
            {
                "title": emulator_name,
                "extracted_dir": str(extract_target_dir),
            },
            archive_path,
        )
        if not resolved_path:
            QMessageBox.warning(
                self,
                "Archive Extracted",
                "Archive extraction finished, but no launchable executable was detected. Open Config to set the executable path manually.",
            )
        return str(extract_target_dir), resolved_path

    def _open_emulator_config_dialog_for_row(self, row: int) -> None:
        emulators = self._normalize_emulators(self._emulators())
        self.config["emulators"] = emulators
        selected_row = row if 0 <= row < len(emulators) else -1
        existing_entry = emulators[selected_row] if selected_row >= 0 else None

        dialog = EmulatorConfigDialog(
            self,
            emulator=existing_entry,
            is_new_entry=selected_row < 0,
            save_strategy_values=["auto", "single_file", "folder"],
        )
        if dialog.exec() != QDialog.DialogCode.Accepted:
            return

        entry = dialog.entry_payload()
        entry_name = entry.get("name", "").strip()
        executable_path = entry.get("path", "").strip()
        archive_path_text = entry.get("archive_path", "").strip()
        if archive_path_text:
            archive_path = Path(archive_path_text).expanduser()
            if not archive_path.exists() or not archive_path.is_file():
                QMessageBox.warning(self, "Validation", f"Archive file was not found:\n{archive_path}")
                return
            extracted_dir, detected_path = self._extract_emulator_archive(entry_name, archive_path)
            if extracted_dir:
                entry["archive_path"] = str(archive_path)
            if not executable_path and detected_path:
                entry["path"] = detected_path

        if self.emulator_name_input is None:
            return
        if self.emulator_path_input is None:
            return
        if self.emulator_args_input is None:
            return
        if self.emulator_save_strategy_input is None:
            return
        if self.emulator_ignore_files_input is None:
            return
        if self.emulator_ignore_extensions_input is None:
            return
        if self.emulator_save_paths_input is None:
            return
        if self.emulator_state_paths_input is None:
            return

        if self.emulator_list is not None:
            self.emulator_list.setCurrentRow(selected_row)

        self.emulator_name_input.setText(entry.get("name", ""))
        self.emulator_path_input.setText(entry.get("path", ""))
        self.emulator_args_input.setText(entry.get("args", "%rom%"))
        self.emulator_save_strategy_input.setCurrentText(entry.get("save_strategy", "auto"))
        self.emulator_ignore_files_input.setText(entry.get("ignore_files", ""))
        self.emulator_ignore_extensions_input.setText(entry.get("ignore_extensions", ""))
        self.emulator_save_paths_input.setText(entry.get("save_paths", ""))
        self.emulator_state_paths_input.setText(entry.get("state_paths", ""))
        self._save_emulator()

    def _save_emulator(self) -> None:
        if (
            self.emulator_name_input is None
            or self.emulator_path_input is None
            or self.emulator_args_input is None
            or self.emulator_save_strategy_input is None
            or self.emulator_ignore_files_input is None
            or self.emulator_ignore_extensions_input is None
            or self.emulator_save_paths_input is None
            or self.emulator_state_paths_input is None
        ):
            return
        name = self.emulator_name_input.text().strip()
        if not name:
            QMessageBox.warning(self, "Validation", "Emulator name is required")
            return

        path = self.emulator_path_input.text().strip()
        args = self.emulator_args_input.text().strip() or "%rom%"
        save_strategy = self._normalize_save_strategy_value(self.emulator_save_strategy_input.currentText())
        ignore_files = self.emulator_ignore_files_input.text().strip()
        ignore_extensions = self.emulator_ignore_extensions_input.text().strip()
        save_paths = self.emulator_save_paths_input.text().strip()
        state_paths = self.emulator_state_paths_input.text().strip()

        emulators = self._emulators()
        target_index = -1
        if self.emulator_list is not None:
            target_index = self.emulator_list.currentRow()

        is_new_manual_entry = target_index < 0 or target_index >= len(emulators)

        self._ensure_emulator_sync_settings(name, path)

        entry = make_emulator_entry_payload(
            name,
            path,
            args,
            save_strategy,
            ignore_files,
            ignore_extensions,
            save_paths,
            state_paths,
        )

        if is_new_manual_entry:
            entry = resolve_apply_manual_emulator_profile_defaults(
                entry,
                self._emulator_autoprofiles(),
                emulator_profile_for_entry=resolve_emulator_profile_for_entry,
                normalize_save_strategy_value=self._normalize_save_strategy_value,
            )

        self.config["emulators"] = self._normalize_emulators(
            upsert_emulator_entry(emulators, entry, target_index)
        )

        if is_new_manual_entry:
            profile = self._emulator_profile_for_entry(entry)
            if isinstance(profile, dict):
                defaults, core_defaults = resolve_assign_profile_platform_defaults(
                    None,
                    entry.get("name", "").strip(),
                    profile,
                    self._normalize_default_emulators(self.config.get("default_emulators", {})),
                    self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {})),
                    is_retroarch_emulator_name=self._is_retroarch_emulator_name,
                    default_assignable_server_platforms=self._default_assignable_server_platforms,
                    installed_retroarch_cores_for_platform=self._installed_retroarch_cores_for_platform,
                    matching_platforms_for_emulator_keywords=self._matching_platforms_for_emulator_keywords,
                    dolphin_variant_label_for_game=self._dolphin_variant_label_for_game,
                    dolphin_target_platforms_for_variant=self._dolphin_target_platforms_for_variant,
                )
                self.config["default_emulators"] = defaults
                self.config["default_retroarch_cores"] = core_defaults

        self._refresh_emulator_views()
        self._clear_emulator_selection()
        self._save_config(self.config)
        if is_new_manual_entry:
            self._show_toast(f"Added emulator '{name}'.", level="success")

    def _show_toast(self, message: str, level: str = "info") -> None:
        del level
        resolve_show_toast(self, message)

    def _remove_emulator_at_index(self, index: int) -> None:
        emulators = self._emulators()
        if index < 0 or index >= len(emulators):
            return

        emulator_to_remove = emulators[index]
        if self._is_ps3_emulator_entry(emulator_to_remove) and self._has_installed_ps3_games():
            QMessageBox.warning(
                self,
                "Uninstall Blocked",
                "Cannot uninstall RPCS3 while PlayStation 3 games are still installed. Uninstall those games first to avoid orphaned launch links.",
            )
            return

        if not self._uninstall_emulator_files(emulator_to_remove):
            return

        removed_name = emulator_to_remove["name"]
        emulators.pop(index)
        self.config["emulators"] = self._normalize_emulators(emulators)

        defaults, core_defaults = remove_emulator_default_mappings(
            self._normalize_default_emulators(self.config.get("default_emulators", {})),
            self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {})),
            removed_name,
        )
        self.config["default_emulators"] = defaults
        self.config["default_retroarch_cores"] = core_defaults

        self._refresh_emulator_views()
        self._save_config(self.config)

    def _remove_emulator(self) -> None:
        if self.emulator_list is None:
            return
        index = self.emulator_list.currentRow()
        self._remove_emulator_at_index(index)

    def _clear_emulator_selection(self) -> None:
        if self.emulator_list is not None:
            self.emulator_list.setCurrentRow(-1)
        self._load_emulator_from_selection(-1)

    def _launch_selected_emulator(self) -> None:
        if self.emulator_list is None:
            return

        index = self.emulator_list.currentRow()
        self._launch_emulator_at_index(index)

    def _launch_emulator_at_index(self, index: int) -> None:
        emulators = self._emulators()
        if index < 0 or index >= len(emulators):
            return

        emulator = emulators[index]
        emulator_name = emulator.get("name", "Emulator")
        emulator_path_value = emulator.get("path", "")
        emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
        if not emulator_path_text:
            QMessageBox.warning(self, "Launch Error", f"Emulator '{emulator_name}' has no executable path configured.")
            return

        emulator_path = Path(emulator_path_text).expanduser()
        if not emulator_path.exists() or not emulator_path.is_file():
            QMessageBox.warning(self, "Launch Error", f"Emulator executable not found:\n{emulator_path}")
            return

        command = [str(emulator_path)]
        try:
            self._ensure_emulator_sync_settings(emulator_name, emulator_path_text)
            process = subprocess.Popen(command, cwd=str(emulator_path.parent))
            QTimer.singleShot(500, lambda p=process, c=command: self._warn_if_process_exited_early(p, c))
        except OSError as error:
            QMessageBox.warning(self, "Launch Error", f"Failed to launch emulator:\n{error}")

    def _set_default_emulator(self) -> None:
        if self.default_platform_combo is None or self.default_emulator_combo is None:
            return
        platform = self.default_platform_combo.currentText().strip()
        emulator_name = self.default_emulator_combo.currentText().strip()
        if not platform or not emulator_name:
            return

        defaults = self._normalize_default_emulators(self.config.get("default_emulators", {}))
        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None or not self._emulator_supports_platform(emulator_entry, platform):
            QMessageBox.warning(self, "Validation", f"Emulator '{emulator_name}' does not match platform '{platform}'.")
            return
        defaults[platform] = emulator_name
        self.config["default_emulators"] = defaults

        core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
        if self._is_retroarch_emulator_name(emulator_name):
            selected_core = ""
            if self.default_core_combo is not None:
                data = self.default_core_combo.currentData()
                if isinstance(data, str):
                    selected_core = data.strip()
            if selected_core:
                core_defaults[platform] = selected_core
            else:
                QMessageBox.warning(self, "Validation", "Select a RetroArch core before setting this default.")
                return
        else:
            core_defaults.pop(platform, None)
        self.config["default_retroarch_cores"] = core_defaults

        self._refresh_emulator_views()
        self._save_config(self.config)

    def _browse_emulator_path(self) -> None:
        if self.emulator_path_input is None:
            return

        selected_file, _ = QFileDialog.getOpenFileName(
            self,
            "Select Emulator Executable",
            self.emulator_path_input.text().strip(),
            "Executables (*.exe);;All Files (*)",
        )
        if selected_file:
            self.emulator_path_input.setText(selected_file)

    def _browse_library_path(self) -> None:
        if self.library_path_input is None:
            return

        current_path = self.library_path_input.text().strip()
        selected_directory = QFileDialog.getExistingDirectory(
            self,
            "Select Library Folder",
            current_path,
        )
        if selected_directory:
            self.library_path_input.setText(selected_directory)

    def _clear_layout(self, layout: QGridLayout) -> None:
        while layout.count() > 0:
            item = layout.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.deleteLater()


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
