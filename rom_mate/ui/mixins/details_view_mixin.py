from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError

from PySide6.QtCore import QSize, QThread, QTimer, Qt
from PySide6.QtWidgets import (
    QAbstractItemView,
    QDialog,
    QFrame,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QSizePolicy,
    QVBoxLayout,
    QWidget,
)
from PySide6.QtGui import QIcon, QPixmap
from rom_mate.ui.theme import themed_svg_pixmap

from rom_mate.background import DetailsCloudRecordsWorker
from rom_mate.emulator import (
    apply_launch_placeholders_to_args,
    available_emulator_name_for_platform,
    copy_ps3_custom_config_to_emulator,
    compatible_emulator_names_for_platform,
    default_emulator_name_for_platform,
    emulator_entry_by_name,
    emulator_entry_has_usable_path,
    install_block_reason_for_game,
    is_native_executable_platform,
    launch_placeholders_for_game,
    mapping_value_for_platform,
    normalized_retroarch_core_args,
    prepare_emulator_launch_command,
    prepare_native_launch_command,
    process_exited_early_message,
    resolve_launch_arguments_for_game,
    resolve_rom_path_for_game,
    retroarch_core_argument_path,
    retroarch_core_value,
    split_launch_template_args,
    strip_wrapping_quotes,
    validate_launch_placeholders,
)
from rom_mate.library import (
    can_start_next_queued_install,
    cloud_sync_state,
    cloud_sync_state_for_game,
    cloud_sync_state_key,
    format_size,
    hydrate_install_game_metadata,
    is_game_install_queued,
    normalize_cloud_sync_state,
    percent_text,
    relative_timestamp_text,
    sort_server_records_by_recency,
    sync_install_metadata_to_details_game,
    update_cloud_sync_state_for_game,
)
from rom_mate.server import (
    cache_rom_id_for_details_game,
    clear_cached_rom_id_for_details_game,
    details_rom_id_cache,
    details_rom_id_cache_key,
    resolve_rom_id_for_game,
)
from rom_mate.server.metadata import details_metadata_from_item
from rom_mate.ui import (
    NativeGameSettingsDialog,
    open_game_details,
    update_details_action_buttons,
)


class DetailsViewMixin:
    """Mixin containing game details view methods for MainWindow."""
    def resizeEvent(self, event) -> None:
        QMainWindow.resizeEvent(self, event)
        self._reflow_current_page_grid()
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
        for field in (
            "ra_id",
            "rating",
            "description",
            "genres",
            "regions",
            "filesize_bytes",
            "revision",
            "languages",
            "tags",
            "fanart_url",
            "companies",
            "first_release_date",
            "ps4_has_update",
            "ps4_has_dlc",
            "ps4_file_ids_by_category",
            "xbox360_has_update",
            "xbox360_has_dlc",
            "xbox360_file_ids_by_category",
        ):
            current_value = game.get(field)
            if field == "rating" and isinstance(current_value, str) and current_value.strip().casefold() == "n/a":
                current_value = ""
            if (
                field == "description"
                and isinstance(current_value, str)
                and current_value.strip().casefold() == "no description available."
            ):
                current_value = ""
            if not current_value:
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


    def _details_missing_server_metadata(self, game: dict[str, str]) -> bool:
        for field in ("genres", "regions", "filesize_bytes", "rating", "companies"):
            current_value = game.get(field)
            if field == "rating" and isinstance(current_value, str) and current_value.strip().casefold() == "n/a":
                current_value = ""
            if isinstance(current_value, str):
                if current_value.strip():
                    continue
            elif current_value:
                continue
            return True
        return False


    def _start_rom_detail_lookup(self, rom_id: str, base_url: str, api_token: str) -> None:
        from rom_mate.background.workers import RomDetailWorker

        existing_thread = self._rom_detail_thread
        if existing_thread is not None and existing_thread.isRunning():
            existing_thread.quit()

        worker = RomDetailWorker(base_url, api_token, rom_id)
        thread = QThread(self)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.finished.connect(self._on_rom_detail_loaded)
        worker.finished.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(lambda t=thread, w=worker: self._cleanup_rom_detail_worker(t, w))
        thread.start()

        self._rom_detail_thread = thread
        self._rom_detail_worker = worker


    def _cleanup_rom_detail_worker(self, thread: QThread, worker: object) -> None:
        if self._rom_detail_thread is thread:
            self._rom_detail_thread = None
        if self._rom_detail_worker is worker:
            self._rom_detail_worker = None


    def _on_rom_detail_loaded(self, bundle: dict) -> None:
        rom_id = bundle.get("rom_id", "")
        payload = bundle.get("payload", {})
        error = bundle.get("error", "")
        if error:
            return
        current_game = self.current_details_game
        if current_game is None:
            return
        current_rom_id = current_game.get("rom_id", "")
        current_rom_id_text = current_rom_id.strip() if isinstance(current_rom_id, str) else str(current_rom_id).strip()
        if current_rom_id_text != str(rom_id).strip():
            return
        metadata = details_metadata_from_item(payload)
        for field in ("description", "genres", "regions", "rating", "filesize_bytes", "revision", "languages", "tags", "fanart_url", "companies", "first_release_date"):
            current_value = current_game.get(field)
            if field == "rating" and isinstance(current_value, str) and current_value.strip().casefold() == "n/a":
                current_value = ""
            if (
                field == "description"
                and isinstance(current_value, str)
                and current_value.strip().casefold() == "no description available."
            ):
                current_value = ""
            if isinstance(current_value, str):
                has_current_value = bool(current_value.strip())
            else:
                has_current_value = bool(current_value)
            if has_current_value:
                continue
            incoming_value = metadata.get(field)
            if isinstance(incoming_value, str) and incoming_value.strip():
                current_game[field] = incoming_value
        open_game_details(self, current_game, self.current_details_source)


    def _on_pcgw_paths_loaded(self, bundle: object) -> None:
        if not isinstance(bundle, dict):
            return
        request_id = bundle.get("request_id")
        raw_paths = bundle.get("paths", [])
        error = bundle.get("error", "")
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
        self._cloud_emulator_entry_cache.clear()
        self._enrich_game_for_details(game)
        self._current_details_game = game
        open_game_details(self, game, source)

        rom_id_value = game.get("rom_id", "")
        rom_id = rom_id_value.strip() if isinstance(rom_id_value, str) else str(rom_id_value).strip()
        base_url = self._server_base_url()
        api_token = str(self.config.get("api_token", "")).strip()
        if (
            self._details_missing_server_metadata(game)
            and rom_id
            and base_url
            and api_token
        ):
            self._start_rom_detail_lookup(rom_id, base_url, api_token)

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
        update_details_action_buttons(self)
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
        return details_rom_id_cache_key(game, game_key=self._game_key)


    def _details_rom_id_cache(self) -> dict[str, str]:
        return details_rom_id_cache(self.config.get("details_rom_id_cache", {}))


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
        if cache_rom_id_for_details_game(
            self.config,
            game,
            rom_id,
            details_rom_id_cache_key=self._details_rom_id_cache_key,
            details_rom_id_cache=self._details_rom_id_cache,
        ):
            self._save_config(self.config)


    def _clear_cached_rom_id_for_details_game(self, game: dict[str, str] | None) -> None:
        if clear_cached_rom_id_for_details_game(
            self.config,
            game,
            details_rom_id_cache_key=self._details_rom_id_cache_key,
            details_rom_id_cache=self._details_rom_id_cache,
        ):
            self._save_config(self.config)


    def _resolve_rom_id_for_game(self, game: dict[str, str]) -> str:
        return resolve_rom_id_for_game(
            game,
            self.server_games_by_platform,
            game_key=self._game_key,
            details_rom_id_cache_key=self._details_rom_id_cache_key,
            details_rom_id_cache=self._details_rom_id_cache,
        )


    def _hydrate_install_game_metadata(self, game: dict[str, str], rom_id: str) -> None:
        hydrate_install_game_metadata(
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


    def _percent_text(self, completed: int, total: int) -> str:
        return percent_text(completed, total)


    def _format_size(self, size_bytes: float) -> str:
        return format_size(size_bytes)

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

        record_emulator = str(record.get("emulator", "")).strip()
        if record_emulator == "native_multi_dir":
            return True, ""

        target_game = self._installed_game_record(self.current_details_game)
        resolved_game = target_game if target_game is not None else self.current_details_game
        if resolved_game is None:
            return False, "No game is selected."

        resolved_emulator_name, resolved_emulator_entry = self._resolved_emulator_entry_for_game(resolved_game)
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

    def _on_details_cloud_records_loaded(self, bundle: object) -> None:
        request_id = bundle.get("request_id", -1) if isinstance(bundle, dict) else -1
        save_type = bundle.get("save_type", "") if isinstance(bundle, dict) else ""
        records = bundle.get("records", []) if isinstance(bundle, dict) else []
        error = bundle.get("error", "") if isinstance(bundle, dict) else str(bundle)
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
        native_game = self.current_details_game if (
            save_type == "save"
            and self.current_details_game is not None
            and self._is_native_executable_platform(self.current_details_game)
        ) else None

        self._clear_layout_items(self.details_cloud_list_layout)

        native_paths: list[str] = []
        if isinstance(native_game, dict):
            native_paths = self._native_save_paths_for_game(native_game)
            self.details_cloud_list_layout.addWidget(self._native_cloud_saves_section_label())

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

        if isinstance(native_game, dict):
            self.details_cloud_list_layout.addStretch()
            separator = QFrame()
            separator.setFrameShape(QFrame.Shape.HLine)
            separator.setFrameShadow(QFrame.Shadow.Sunken)
            self.details_cloud_list_layout.addSpacing(8)
            self.details_cloud_list_layout.addWidget(separator)
            self.details_cloud_list_layout.addSpacing(8)
            self._render_native_save_path_section(native_game, native_paths)
        else:
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

    def _native_save_paths_for_game(self, game: dict) -> list[str]:
        cached = self._pcgw_paths_for_game(game) or []
        key = self._pcgw_cache_key(game)
        manual_key = key + "__manual"
        manual_paths: list[str] = self._pcgw_paths_cache.get(manual_key, [])
        return list(cached) + [path for path in manual_paths if path not in cached]

    def _native_cloud_saves_section_label(self) -> QLabel:
        label = QLabel("Cloud Saves")
        label.setStyleSheet("font-size: 16px; font-weight: 700;")
        return label

    def _render_native_save_path_section(self, game: dict, all_raw_paths: list[str]) -> None:
        if self.details_cloud_list_layout is None:
            return

        import os

        container = QFrame()
        container.setObjectName("detailsNativePathSection")
        container_layout = QVBoxLayout(container)
        container_layout.setContentsMargins(0, 0, 0, 0)
        container_layout.setSpacing(8)

        heading = QLabel("Save Locations")
        heading.setStyleSheet("font-size: 16px; font-weight: 700;")
        container_layout.addWidget(heading)

        path_list = QListWidget()
        path_list.setObjectName("nativeSaveDirList")
        path_list.setAlternatingRowColors(True)
        path_list.setSelectionMode(QAbstractItemView.SelectionMode.NoSelection)
        path_list.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        path_list.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        path_list.setSizeAdjustPolicy(QListWidget.SizeAdjustPolicy.AdjustToContents)

        for raw_path in all_raw_paths:
            expanded = os.path.expandvars(raw_path)
            item = QListWidgetItem()
            row = QWidget()
            row_layout = QHBoxLayout(row)
            row_layout.setContentsMargins(6, 4, 6, 4)
            row_layout.setSpacing(8)
            label = QLabel(raw_path)
            label.setStyleSheet("padding: 4px 0;")
            label.setToolTip(expanded)
            label.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
            row_layout.addWidget(label, 1)
            _close_pix = themed_svg_pixmap("svg/trashcan", "#888888", size=QSize(14, 14))
            remove_btn = QPushButton()
            remove_btn.setIcon(QIcon(_close_pix))
            remove_btn.setIconSize(QSize(14, 14))
            remove_btn.setFixedSize(QSize(24, 24))
            remove_btn.setToolTip("Remove this path")
            remove_btn.setAccessibleName("Remove")
            raw_path_capture = raw_path

            def _remove(checked=False, rp=raw_path_capture):
                self._pcgw_remove_path_for_game(game, rp)
                self._refresh_details_cloud_panel()

            remove_btn.clicked.connect(_remove)
            row_layout.addWidget(remove_btn)
            item.setSizeHint(row.sizeHint())
            path_list.addItem(item)
            path_list.setItemWidget(item, row)

        container_layout.addWidget(path_list)

        browse_btn = QPushButton("Browse...")
        browse_btn.setToolTip("Add a custom save folder for this game")

        def _browse(checked=False):
            from PySide6.QtWidgets import QFileDialog

            folder = QFileDialog.getExistingDirectory(self, "Select Save Folder")
            if folder:
                self._pcgw_add_manual_path_for_game(game, folder)
                self._refresh_details_cloud_panel()

        browse_btn.clicked.connect(_browse)
        container_layout.addWidget(browse_btn)

        self.details_cloud_list_layout.addWidget(container)

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

        all_raw_paths = self._native_save_paths_for_game(game)
        rom_id = self._cloud_sync_rom_id_for_game(game)

        # Show the path list (or empty state)
        if not all_raw_paths:
            self.details_cloud_status_label.setText("No save locations found on PCGamingWiki.")
            self.details_cloud_upload_button.setEnabled(False)
            self.details_cloud_upload_button.setToolTip("Add a save location to enable uploads.")
        else:
            self.details_cloud_status_label.setText(f"{len(all_raw_paths)} save location(s) configured.")
            self.details_cloud_upload_button.setEnabled(bool(rom_id))
            self.details_cloud_upload_button.setToolTip(
                "Upload save files from the listed locations." if rom_id else "Missing ROM id for this game."
            )

        self.details_cloud_list_layout.addWidget(self._native_cloud_saves_section_label())

        if not rom_id:
            self.details_cloud_empty_label.setText("Missing ROM id for this game.")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)
        else:
            self.details_cloud_empty_label.setText("Loading cloud saves from the server...")
            self.details_cloud_list_layout.addWidget(self.details_cloud_empty_label)

        self.details_cloud_list_layout.addStretch()
        separator = QFrame()
        separator.setFrameShape(QFrame.Shape.HLine)
        separator.setFrameShadow(QFrame.Shadow.Sunken)
        self.details_cloud_list_layout.addSpacing(8)
        self.details_cloud_list_layout.addWidget(separator)
        self.details_cloud_list_layout.addSpacing(8)
        self._render_native_save_path_section(game, all_raw_paths)

        if rom_id:
            self._start_details_cloud_records_worker(
                rom_id,
                "save",
                kind_label="saves",
                upload_reason="Uploads include all configured native save locations.",
                emulator_name="native_multi_dir",
            )

    def _pcgw_add_manual_path_for_game(self, game: dict, folder: str) -> None:
        from rom_mate.library.cloud_transfer import normalize_manual_save_path

        folder = normalize_manual_save_path(folder)
        key = self._pcgw_cache_key(game)
        manual_key = key + "__manual"
        existing = self._pcgw_paths_cache.get(manual_key, [])
        if folder not in existing:
            self._pcgw_paths_cache[manual_key] = existing + [folder]
        saved = self.config.setdefault("native_manual_save_paths", {})
        saved[key] = list(self._pcgw_paths_cache.get(key + "__manual", []))
        self._save_config(self.config)

    def _pcgw_remove_path_for_game(self, game: dict, raw_path: str) -> None:
        key = self._pcgw_cache_key(game)
        manual_key = key + "__manual"
        # Remove from PCGW list
        current = self._pcgw_paths_cache.get(key, [])
        self._pcgw_paths_cache[key] = [p for p in current if p != raw_path]
        # Remove from manual list
        manual = self._pcgw_paths_cache.get(manual_key, [])
        self._pcgw_paths_cache[manual_key] = [p for p in manual if p != raw_path]
        saved = self.config.setdefault("native_manual_save_paths", {})
        saved[key] = list(self._pcgw_paths_cache.get(key + "__manual", []))
        self._save_config(self.config)

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
        return mapping_value_for_platform(mapping, platform)

    def _default_emulator_name_for_platform(self, platform: str) -> str:
        if platform in self._platform_default_emulator_cache:
            return self._platform_default_emulator_cache[platform]
        emulators = self._normalize_emulators(self._emulators())
        compatible = compatible_emulator_names_for_platform(
            emulators,
            platform,
            self._emulator_supports_platform,
        )
        result = default_emulator_name_for_platform(
            platform,
            self._normalize_default_emulators(self.config.get("default_emulators", {})),
            emulators,
            self._emulator_supports_platform,
            compatible,
        )
        self._platform_default_emulator_cache[platform] = result
        return result

    def _emulator_entry_has_usable_path(self, emulator: dict[str, str]) -> bool:
        return emulator_entry_has_usable_path(emulator)

    def _available_emulator_name_for_platform(self, platform: str) -> str:
        if platform in self._platform_available_emulator_cache:
            return self._platform_available_emulator_cache[platform]
        emulators = self._normalize_emulators(self._emulators())
        default_name = self._default_emulator_name_for_platform(platform)
        result = available_emulator_name_for_platform(
            platform,
            emulators,
            self._emulator_supports_platform,
            default_name,
        )
        self._platform_available_emulator_cache[platform] = result
        return result

    def _install_block_reason_for_game(self, game: dict[str, str]) -> str:
        return install_block_reason_for_game(
            game,
            self._is_native_executable_platform,
            self._is_emulators_platform,
            self._available_emulator_name_for_platform,
        )

    def _emulator_entry_by_name(self, emulator_name: str) -> dict[str, str] | None:
        return emulator_entry_by_name(self._normalize_emulators(self._emulators()), emulator_name)

    def _launch_placeholders_for_game(self, game: dict[str, str], emulator_name: str) -> dict[str, str]:
        rom_path = self._resolved_rom_path_for_game(game)
        platform_value = game.get("platform", "")
        platform = platform_value.strip() if isinstance(platform_value, str) else ""
        core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
        core_value = retroarch_core_value(
            emulator_name,
            platform,
            core_defaults,
            self._is_retroarch_emulator_name,
            self._mapping_value_for_platform,
            self._retroarch_core_argument_path,
        )
        return launch_placeholders_for_game(
            rom_path,
            emulator_name,
            core_value,
            self._is_rpcs3_emulator_name,
            self._ps3_game_id_for_game(game),
        )

    def _retroarch_core_argument_path(self, configured_core: str) -> str:
        return retroarch_core_argument_path(configured_core)

    def _strip_wrapping_quotes(self, token: str) -> str:
        return strip_wrapping_quotes(token)

    def _apply_launch_placeholders_to_args(self, args: list[str], placeholders: dict[str, str]) -> list[str]:
        return apply_launch_placeholders_to_args(args, placeholders)

    def _split_launch_template_args(self, template: str) -> list[str]:
        return split_launch_template_args(template)

    def _resolved_launch_arguments_for_game(self, game: dict[str, str]) -> tuple[str, list[str]]:
        return resolve_launch_arguments_for_game(
            game,
            self.config.get("launch_args", ""),
            self._default_emulator_name_for_platform,
            self._emulator_entry_by_name,
            self._split_launch_template_args,
            self._launch_placeholders_for_game,
            validate_launch_placeholders,
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
        return normalized_retroarch_core_args(emulator_dir, args)

    def _launch_installed_game(self, game: dict[str, str]) -> bool:
        try:
            if self._is_native_executable_platform(game):
                command, working_directory, compat_env = prepare_native_launch_command(
                    game,
                    self._resolved_native_executable_path_for_game,
                    self._split_launch_template_args,
                )
                process = subprocess.Popen(
                    command,
                    cwd=working_directory,
                    env={**os.environ, **compat_env} if compat_env else None,
                    creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if sys.platform == "win32" else 0,
                )
                QTimer.singleShot(500, lambda p=process, c=command: self._warn_if_process_exited_early(p, c))
                return True

            emulator_name, command, working_directory = prepare_emulator_launch_command(
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
                if self._is_rpcs3_emulator_name(emulator_name):
                    _ps3_game_id = game.get("ps3_game_id", "").strip()
                    if _ps3_game_id:
                        _ps3_dev_hdd0 = self._ps3_dev_hdd0_for_game(game)
                        _rpcs3_root = self._rpcs3_data_root_for_game(game)
                        if _ps3_dev_hdd0 is not None and _rpcs3_root is not None:
                            copy_ps3_custom_config_to_emulator(_ps3_dev_hdd0.parent / "config", _rpcs3_root)
            process = subprocess.Popen(
                command,
                cwd=working_directory,
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if sys.platform == "win32" else 0,
            )
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
            process_exited_early_message(exit_code, command),
        )

    def _perform_game_action(self) -> None:
        if self.current_details_game is None:
            return
        if self._is_game_installed(self.current_details_game):
            installed_game = self._installed_game_record(self.current_details_game)
            launch_game = installed_game if installed_game is not None else self.current_details_game
            self._auto_sync_before_launch(launch_game)
            try:
                firmware_warnings = self._install_firmware_for_game_without_ui(launch_game, {})
            except Exception as e:
                pass

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

    def _perform_xbox360_content_action(self) -> None:
        if self.current_details_game is None:
            return

        block_reason = self._xbox360_content_install_block_reason(self.current_details_game)
        if block_reason:
            QMessageBox.warning(self, "Install Blocked", block_reason)
            return

        available_kinds = self._available_xbox360_content_kinds_for_game(self.current_details_game)
        if not available_kinds:
            QMessageBox.warning(
                self,
                "Install Error",
                "No Xbox 360 update or DLC content is available for this game from the current server metadata.",
            )
            return

        selected_kind = ""
        if len(available_kinds) == 1:
            selected_kind = available_kinds[0]
        else:
            chooser = QMessageBox(self)
            chooser.setWindowTitle("Install Xbox 360 Content")
            chooser.setText("Choose the content type to install:")
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

        file_ids_by_category = self._xbox360_file_ids_by_category_for_game(
            self.current_details_game,
            rom_id,
            allow_payload_lookup=True,
        )
        selected_file_ids = file_ids_by_category.get(selected_kind, [])
        if not selected_file_ids:
            QMessageBox.warning(
                self,
                "Install Error",
                f"No Xbox 360 {selected_kind} files were found for this title in server metadata.",
            )
            return

        install_game = dict(self.current_details_game)
        install_game["rom_id"] = rom_id
        install_game["_install_mode"] = "xbox360_content"
        install_game["_xenia_content_kind"] = selected_kind
        install_game["_ps4_file_ids_csv"] = ",".join(str(file_id) for file_id in selected_file_ids)
        install_game["_archive_name_override"] = self._xbox360_content_archive_name(install_game, selected_kind)
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
        install_game["xbox360_file_ids_by_category"] = json.dumps(
            file_ids_by_category,
            separators=(",", ":"),
            sort_keys=True,
        )

        self._start_async_install(install_game)

    def _queue_xbox360_content_for_game(self, installed_game: dict[str, str]) -> None:
        """Silently queue Xbox 360 title update and DLC downloads after base game install."""
        if not self._is_xbox360_platform(installed_game):
            return

        rom_id = self._resolve_rom_id_for_game(installed_game)
        if not rom_id:
            return

        file_ids_by_category = self._xbox360_file_ids_by_category_for_game(
            installed_game,
            rom_id,
            allow_payload_lookup=False,
        )
        if not file_ids_by_category:
            return

        resolved_file_name = self._resolved_rom_file_name_for_game(installed_game, rom_id)

        for kind in ("update", "dlc"):
            selected_file_ids = file_ids_by_category.get(kind, [])
            if not selected_file_ids:
                continue

            install_game = dict(installed_game)
            install_game["rom_id"] = rom_id
            install_game["_install_mode"] = "xbox360_content"
            install_game["_xenia_content_kind"] = kind
            install_game["_ps4_file_ids_csv"] = ",".join(str(file_id) for file_id in selected_file_ids)
            install_game["_archive_name_override"] = self._xbox360_content_archive_name(install_game, kind)
            if resolved_file_name:
                install_game["rom_file_name"] = resolved_file_name
            install_game["xbox360_file_ids_by_category"] = json.dumps(
                file_ids_by_category,
                separators=(",", ":"),
                sort_keys=True,
            )
            self._hydrate_install_game_metadata(install_game, rom_id)
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

        available_compat_tools = (
            self._available_compat_tools_for_dialog() if sys.platform != "win32" else []
        )
        existing_compat_tool_value = installed_game.get("native_compat_tool", "")
        existing_compat_tool = (
            existing_compat_tool_value.strip() if isinstance(existing_compat_tool_value, str) else ""
        )
        existing_wineprefix_value = installed_game.get("native_wineprefix", "")
        existing_wineprefix = (
            existing_wineprefix_value.strip() if isinstance(existing_wineprefix_value, str) else ""
        )

        dialog = NativeGameSettingsDialog(
            self,
            game_title=str(installed_game.get("title", "Game")),
            install_dir=install_dir,
            executable_candidates=executable_candidates,
            selected_executable_path=selected_executable_path,
            existing_launch_parameters=existing_launch_parameters,
            section_title_factory=self._make_section_title,
            available_compat_tools=available_compat_tools,
            existing_compat_tool=existing_compat_tool,
            existing_wineprefix=existing_wineprefix,
        )
        if dialog.exec() != int(QDialog.DialogCode.Accepted):
            return

        selected_executable = dialog.selected_executable_path()
        if not selected_executable:
            return

        native_launch_parameters = dialog.launch_parameters()
        installed_game["native_executable_path"] = selected_executable
        installed_game["native_launch_parameters"] = native_launch_parameters
        if sys.platform != "win32":
            installed_game["native_compat_tool"] = dialog.selected_compat_tool_path()
        if self.current_details_game is not None:
            self.current_details_game["native_executable_path"] = selected_executable
            self.current_details_game["native_launch_parameters"] = native_launch_parameters
            if sys.platform != "win32":
                self.current_details_game["native_compat_tool"] = dialog.selected_compat_tool_path()

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
            else is_native_executable_platform(self.current_details_game)
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
        sync_install_metadata_to_details_game(
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


