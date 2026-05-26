from __future__ import annotations

import json
import subprocess
import sys
import threading
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from PySide6.QtCore import QSize, QThread, QTimer
from PySide6.QtWidgets import (
    QDialog,
    QFileDialog,
    QGridLayout,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QListWidgetItem,
    QMessageBox,
    QPushButton,
    QSizePolicy,
    QVBoxLayout,
    QWidget,
)

from rom_mate.core import (
    normalize_default_emulators,
    normalize_default_retroarch_cores,
    normalize_emulators,
    normalize_installed_games,
)
from rom_mate.background.workers import SourceVersionCheckWorker
from rom_mate.emulator import (
    all_retroarch_cores,
    apply_manual_emulator_profile_defaults,
    assign_profile_platform_defaults,
    default_emulator_autoprofiles,
    emulator_autoprofiles_path,
    emulator_install_directory,
    emulator_profile_for_entry,
    ensure_azahar_settings,
    ensure_cemu_controller_config,
    ensure_cemu_settings,
    ensure_dolphin_settings,
    ensure_duckstation_memory_card_settings,
    ensure_eden_settings,
    ensure_pcsx2_settings,
    ensure_ppsspp_settings,
    ensure_redream_settings,
    ensure_retroarch_save_location_settings,
    ensure_rpcs3_settings,
    ensure_xemu_settings,
    installed_retroarch_core_ids,
    load_emulator_autoprofiles as resolve_emulator_autoprofiles,
    load_retroarch_compatibility_map,
    normalize_emulator_autoprofiles,
    normalize_retroarch_platform_key,
    retroarch_core_id_from_file_name,
    retroarch_core_id_from_name,
    retroarch_core_list_path,
    retroarch_cores_for_platform,
    retroarch_markdown_label,
    retroarch_platform_tokens,
    retroarch_system_keys_for_platform,
    eden_has_firmware,
    eden_keys_path,
    rpcs3_pup_path,
    trigger_rpcs3_firmware_install,
)
from rom_mate.library import extract_archive_into_directory
from rom_mate.library.firmware_install import download_ps3_firmware_direct, install_platform_firmware
from rom_mate.ui import (
    EmulatorConfigDialog,
    emulator_form_state_for_row,
    make_emulator_entry_payload,
    mapping_list_entries,
    preferred_emulator_selection,
    remove_emulator_default_mappings,
    selected_retroarch_core,
    themed_svg_icon,
    upsert_emulator_entry,
)
from rom_mate.ui.emulators import (
    available_source_download_emulator_entries,
    save_button_label,
)

_TV_GUIDE_DEFAULT_EXCLUSIONS: frozenset[str] = frozenset({
    "rpcs3", "cemu", "dolphin", "xemu", "xenia", "retroarch"
})


class EmulatorUIMixin:
    """Mixin containing emulator management UI methods for MainWindow."""
    def _normalize_emulators(self, value: Any) -> list[dict[str, str]]:
        return normalize_emulators(value, self._normalize_save_strategy_value)

    def _normalize_default_emulators(self, value: Any) -> dict[str, str]:
        return normalize_default_emulators(value)

    def _normalize_default_retroarch_cores(self, value: Any) -> dict[str, str]:
        return normalize_default_retroarch_cores(value)

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

    def _ensure_emulator_sync_settings(self, emulator_name: str, emulator_path_text: str) -> None:
        timing_start = getattr(self, "_debug_timing_start", None)
        timing_end = getattr(self, "_debug_timing_end", None)
        started_at = timing_start("_ensure_emulator_sync_settings", emulator=emulator_name) if callable(timing_start) else 0.0
        romm_username = str(self.config.get("username", "")).strip() if hasattr(self, "config") and self.config else ""
        path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
        if not path_text:
            if callable(timing_end):
                timing_end("_ensure_emulator_sync_settings", started_at, result="no-path")
            return
        sync_cache_key = f"{emulator_name}::{path_text}"
        sync_done = getattr(self, "_emulator_sync_settings_done", None)
        if sync_done is not None and sync_cache_key in sync_done:
            if callable(timing_end):
                timing_end("_ensure_emulator_sync_settings", started_at, result="done")
            return
        emulator_entry = {"name": emulator_name, "path": path_text}
        ra_username = str(self.config.get("retroachievements_username", "")).strip()
        ra_token = str(self.config.get("retroachievements_token", "")).strip()
        if self._is_retroarch_emulator_name(emulator_name, emulator_entry):
            ensure_retroarch_save_location_settings(
                path_text,
                enable_fullscreen=True,
                retroachievements_username=ra_username,
                retroachievements_token=ra_token,
                username=romm_username,
            )
        if self._is_duckstation_emulator_name(emulator_name, emulator_entry):
            ensure_duckstation_memory_card_settings(
                path_text,
                enable_fullscreen=True,
            )
        if self._is_xemu_emulator_name(emulator_name, emulator_entry):
            ensure_xemu_settings(path_text)
        if self._is_pcsx2_emulator_name(emulator_name, emulator_entry):
            bios_dir = ""
            firmware_dirs = self._resolved_firmware_directories(emulator_entry)
            for fw_entry in firmware_dirs:
                fw_path = fw_entry[0] if isinstance(fw_entry, tuple) else fw_entry
                bios_dir = str(fw_path)
                break
            ensure_pcsx2_settings(
                path_text,
                enable_fullscreen=True,
                retroachievements_username=ra_username,
                retroachievements_token=ra_token,
                bios_directory=bios_dir,
            )
        if self._is_dolphin_emulator_name(emulator_name, emulator_entry):
            ensure_dolphin_settings(path_text)
        if self._is_azahar_emulator_name(emulator_name, emulator_entry):
            ensure_azahar_settings(path_text)
        if self._is_eden_emulator_name(emulator_name, emulator_entry):
            ensure_eden_settings(path_text)
        if self._is_rpcs3_emulator_name(emulator_name, emulator_entry):
            library_path_value = self.config.get("library_path", "")
            library_path_text = library_path_value.strip() if isinstance(library_path_value, str) else ""
            ps3_library_path = str(Path(library_path_text).expanduser() / "PlayStation 3") if library_path_text else ""
            ensure_rpcs3_settings(path_text, ps3_library_path=ps3_library_path)
            self._trigger_rpcs3_firmware_download_background(emulator_entry, path_text)
        if self._is_ppsspp_emulator_name(emulator_name, emulator_entry):
            ensure_ppsspp_settings(
                path_text,
                retroachievements_username=ra_username,
                retroachievements_token=ra_token,
            )
        if self._is_cemu_emulator_name(emulator_name, emulator_entry):
            ensure_cemu_settings(path_text)
            ensure_cemu_controller_config(path_text)
        if self._is_redream_emulator_name(emulator_name, emulator_entry):
            ensure_redream_settings(path_text)
        if sync_done is not None:
            sync_done.add(sync_cache_key)
        if callable(timing_end):
            timing_end("_ensure_emulator_sync_settings", started_at, result="done")

    def _emulator_autoprofiles_path(self) -> Path:
        return emulator_autoprofiles_path(Path(__file__).resolve().parents[3])

    def _default_emulator_autoprofiles(self) -> list[dict[str, Any]]:
        return default_emulator_autoprofiles()

    def _normalize_emulator_autoprofiles(self, value: Any) -> list[dict[str, Any]]:
        return normalize_emulator_autoprofiles(
            value,
            self._normalize_save_strategy_value,
            self._normalize_ignore_extension_value,
        )

    def _emulator_autoprofiles(self) -> list[dict[str, Any]]:
        profiles = resolve_emulator_autoprofiles(
            self.emulator_autoprofiles,
            Path(__file__).resolve().parents[3],
            self._normalize_save_strategy_value,
            self._normalize_ignore_extension_value,
        )
        self.emulator_autoprofiles = profiles
        return profiles

    def _retroarch_core_list_path(self) -> Path:
        return retroarch_core_list_path(Path(__file__).resolve().parents[3])

    def _retroarch_markdown_label(self, value: str) -> str:
        return retroarch_markdown_label(value)

    def _retroarch_core_id_from_name(self, core_name: str) -> str:
        return retroarch_core_id_from_name(core_name)

    def _retroarch_core_id_from_file_name(self, core_file_name: str) -> str:
        return retroarch_core_id_from_file_name(core_file_name)

    def _retroarch_core_list_entries(self) -> list:
        cache = getattr(self, "_retroarch_core_list_entries_cache", None)
        if isinstance(cache, list):
            return cache
        path = self._retroarch_core_list_path()
        if not path.exists():
            self._retroarch_core_list_entries_cache = []
            return []
        try:
            raw = path.read_text(encoding="utf-8")
            entries = json.loads(raw)
        except (OSError, json.JSONDecodeError):
            entries = []
        if not isinstance(entries, list):
            entries = []
        self._retroarch_core_list_entries_cache = entries
        return entries

    def _retroarch_compatibility_map_from_markdown(self) -> dict[str, list[str]]:
        if isinstance(self.retroarch_compatibility_map, dict):
            return self.retroarch_compatibility_map

        compatibility = load_retroarch_compatibility_map(self._retroarch_core_list_path())
        self.retroarch_compatibility_map = compatibility
        return compatibility

    def _normalize_retroarch_platform_key(self, value: str) -> str:
        return normalize_retroarch_platform_key(value)

    def _retroarch_platform_tokens(self, value: str) -> set[str]:
        return retroarch_platform_tokens(value)

    def _all_retroarch_cores(self, compatibility: dict[str, list[str]]) -> list[str]:
        return all_retroarch_cores(compatibility)

    def _retroarch_installed_core_ids_for_emulator(self, emulator_name: str) -> set[str]:
        if not self._is_retroarch_emulator_name(emulator_name):
            return set()

        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None:
            return set()

        emulator_path_value = emulator_entry.get("path", "")
        emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
        if emulator_path_text in self._retroarch_core_ids_cache:
            return self._retroarch_core_ids_cache[emulator_path_text]
        result = installed_retroarch_core_ids(emulator_path_text)
        self._retroarch_core_ids_cache[emulator_path_text] = result
        return result

    def _installed_retroarch_cores_for_platform(self, platform: str, emulator_name: str) -> list[str]:
        platform_cores = self._retroarch_cores_for_platform(platform)
        installed_core_ids = self._retroarch_installed_core_ids_for_emulator(emulator_name)
        if not installed_core_ids:
            return []
        return [core for core in platform_cores if core in installed_core_ids]

    def _retroarch_system_keys_for_platform(self, platform: str) -> list[str]:
        return retroarch_system_keys_for_platform(
            platform,
            self._retroarch_compatibility_map_from_markdown(),
        )

    def _retroarch_cores_for_platform(self, platform: str) -> list[str]:
        return retroarch_cores_for_platform(
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
        return normalize_installed_games(value, self._game_key)

    def _emulators(self) -> list[dict[str, str]]:
        emulators = self.config.get("emulators", [])
        if not isinstance(emulators, list):
            return []
        return emulators

    def _refresh_emulator_views(self) -> None:
        self._retroarch_core_ids_cache.clear()
        self._platform_default_emulator_cache.clear()
        self._platform_available_emulator_cache.clear()
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

        themed_svg_icon_fn = getattr(self, "_themed_svg_icon", None)
        if not callable(themed_svg_icon_fn):
            themed_svg_icon_fn = themed_svg_icon

        launch_icon = themed_svg_icon_fn(
            "svg/play.svg",
            accent_color,
            size=action_icon_size,
        )
        config_icon = themed_svg_icon_fn(
            "svg/config.svg",
            text_color,
            size=action_icon_size,
        )
        uninstall_icon = themed_svg_icon_fn(
            "svg/trashcan.svg",
            error_color,
            size=action_icon_size,
        )
        source_update_icon = themed_svg_icon_fn(
            "svg/cloud.svg",
            accent_color,
            size=action_icon_size,
        )

        selected_name = ""
        selected_index = self.emulator_list.currentRow()
        if 0 <= selected_index < len(emulators):
            existing_name = emulators[selected_index].get("name", "")
            if isinstance(existing_name, str):
                selected_name = existing_name.strip()

        scroll_pos = self.emulator_list.verticalScrollBar().value()
        self.emulator_list.clear()
        for row, entry in enumerate(emulators):
            item = QListWidgetItem()
            row_widget = QWidget()
            outer_layout = QVBoxLayout(row_widget)
            outer_layout.setContentsMargins(6, 4, 6, 4)
            outer_layout.setSpacing(2)

            top_row = QHBoxLayout()
            top_row.setContentsMargins(0, 0, 0, 0)
            top_row.setSpacing(8)

            name_label = QLabel(entry["name"])
            name_label.setStyleSheet("padding: 4px 0;")
            top_row.addWidget(name_label, 1)
            outer_layout.addLayout(top_row)

            if self._is_azahar_emulator_name(entry.get("name", ""), entry):
                azahar_note = QLabel(
                    "Controller setup: Settings \u2192 Controls \u2192 Auto Map  \u00b7  Press Esc to close emulator"
                )
                azahar_note.setObjectName("azaharControllerNote")
                azahar_note.setWordWrap(True)
                azahar_note.setStyleSheet("color: palette(mid); font-size: 10px; padding: 1px 0;")
                outer_layout.addWidget(azahar_note)

            if self._is_eden_emulator_name(entry.get("name", ""), entry):
                eden_note = QLabel(
                    "Controller setup: Controls \u2192 Configure \u2192 Map Controller"
                )
                eden_note.setObjectName("edenControllerNote")
                eden_note.setWordWrap(True)
                eden_note.setStyleSheet("color: palette(mid); font-size: 10px; padding: 1px 0;")
                outer_layout.addWidget(eden_note)

                path_value = entry.get("path", "")
                path_text = path_value.strip() if isinstance(path_value, str) else ""
                if not eden_keys_path(path_text):
                    keys_note = QLabel(
                        "Switch keys (prod.keys) must be placed in user/keys/ before playing games."
                    )
                    keys_note.setObjectName("edenKeysNote")
                    keys_note.setWordWrap(True)
                    keys_note.setStyleSheet("color: palette(mid); font-size: 10px; padding: 1px 0;")
                    outer_layout.addWidget(keys_note)

                if not eden_has_firmware(path_text):
                    firmware_note = QLabel(
                        "Switch firmware must be installed via Emulation \u2192 Install Firmware before playing games."
                    )
                    firmware_note.setObjectName("edenFirmwareNote")
                    firmware_note.setWordWrap(True)
                    firmware_note.setStyleSheet("color: palette(mid); font-size: 10px; padding: 1px 0;")
                    outer_layout.addWidget(firmware_note)

            if self._is_xemu_emulator_name(entry.get("name", ""), entry):
                xemu_note = QLabel(
                    "Controller setup: required to connect a controller first \u2014 layout is auto-detected"
                )
                xemu_note.setObjectName("xemuControllerNote")
                xemu_note.setWordWrap(True)
                xemu_note.setStyleSheet("color: palette(mid); font-size: 10px; padding: 1px 0;")
                outer_layout.addWidget(xemu_note)

            if self._is_rpcs3_emulator_name(entry.get("name", "")):
                rpcs3_controller_note = QLabel("Must setup controller after install")
                rpcs3_controller_note.setObjectName("rpcs3ControllerNote")
                rpcs3_controller_note.setWordWrap(True)
                rpcs3_controller_note.setStyleSheet("color: palette(mid); font-size: 10px; padding: 1px 0;")
                outer_layout.addWidget(rpcs3_controller_note)

                path_value = entry.get("path", "")
                path_text = path_value.strip() if isinstance(path_value, str) else ""
                pup_path = rpcs3_pup_path(path_text)
                if pup_path is not None:
                    fw_note = QLabel("PS3 firmware downloaded \u2014 click Install to activate it.")
                    fw_note.setWordWrap(True)
                    fw_note.setStyleSheet("color: palette(mid); font-size: 10px; padding: 1px 0;")
                    outer_layout.addWidget(fw_note)

                    fw_row = QHBoxLayout()
                    fw_row.setContentsMargins(0, 0, 0, 0)
                    fw_row.setSpacing(4)
                    fw_btn = QPushButton("Install PS3 Firmware")
                    fw_btn.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
                    fw_btn.clicked.connect(
                        lambda checked=False, ep=path_text, pp=str(pup_path): (
                            self._show_toast(
                                {
                                    "message": "PS3 firmware installation started — follow the RPCS3 dialog to complete.",
                                    "level": "success",
                                }
                            )
                            if trigger_rpcs3_firmware_install(ep, pp)
                            else self._show_toast(
                                "Could not launch RPCS3 to install firmware. Check the emulator path.",
                            )
                        )
                    )
                    fw_row.addWidget(fw_btn)
                    fw_row.addStretch(1)
                    outer_layout.addLayout(fw_row)

            bottom_row = QHBoxLayout()
            bottom_row.setContentsMargins(0, 0, 0, 0)
            bottom_row.setSpacing(4)
            bottom_row.addStretch(1)

            launch_button = QPushButton()
            launch_button.setObjectName("installedEmulatorLaunchButton")
            launch_button.setToolTip("Launch")
            launch_button.setIcon(launch_icon)
            launch_button.setIconSize(action_icon_size)
            launch_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            launch_button.clicked.connect(lambda checked=False, current_row=row: self._launch_emulator_at_index(current_row))
            bottom_row.addWidget(launch_button)

            config_button = QPushButton()
            config_button.setObjectName("installedEmulatorConfigButton")
            config_button.setToolTip("Config")
            config_button.setIcon(config_icon)
            config_button.setIconSize(action_icon_size)
            config_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            config_button.setMinimumHeight(config_button.sizeHint().height() + 2)
            config_button.clicked.connect(
                lambda checked=False, current_row=row: self._open_emulator_config_dialog_for_row(current_row)
            )
            bottom_row.addWidget(config_button)

            uninstall_button = QPushButton()
            uninstall_button.setObjectName("installedEmulatorUninstallButton")
            uninstall_button.setToolTip("Uninstall")
            uninstall_button.setIcon(uninstall_icon)
            uninstall_button.setIconSize(action_icon_size)
            uninstall_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            uninstall_button.clicked.connect(lambda checked=False, current_row=row: self._remove_emulator_at_index(current_row))
            bottom_row.addWidget(uninstall_button)

            source_entry = self._source_download_entry_for_emulator_name(
                entry.get("name", ""),
                entry,
            )
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
                bottom_row.addWidget(source_update_button)

            outer_layout.addLayout(bottom_row)

            item.setSizeHint(row_widget.sizeHint())
            self.emulator_list.addItem(item)
            self.emulator_list.setItemWidget(item, row_widget)

        if selected_name:
            for row, emulator in enumerate(emulators):
                emulator_name = emulator.get("name", "")
                if isinstance(emulator_name, str) and emulator_name.strip().casefold() == selected_name.casefold():
                    self.emulator_list.setCurrentRow(row)
                    break
        self.emulator_list.verticalScrollBar().setValue(scroll_pos)

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

        QTimer.singleShot(0, self._warm_emulator_platform_caches)

    def _warm_emulator_platform_caches(self) -> None:
        import threading

        platforms: set[str] = set(self.server_platform_ids.keys())
        for game in self.library_games:
            platform_value = game.get("platform", "")
            if isinstance(platform_value, str):
                platform = platform_value.strip()
                if platform:
                    platforms.add(platform)
        threading.Thread(
            target=self._do_warm_emulator_platform_caches,
            args=(platforms,),
            daemon=True,
        ).start()

    def _do_warm_emulator_platform_caches(self, platforms: set[str]) -> None:
        for platform in platforms:
            self._available_emulator_name_for_platform(platform)

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
        installed_source_ids: list[str] | None = None
        if installed_emulator_names is not None:
            installed_source_ids = []
            for emulator in self._normalize_emulators(self._emulators()):
                if not isinstance(emulator, dict):
                    continue
                source_id_value = emulator.get("source_id", "")
                source_id = source_id_value.strip() if isinstance(source_id_value, str) else ""
                if not source_id:
                    source_owner_value = emulator.get("source_owner", "")
                    source_repo_value = emulator.get("source_repo", "")
                    source_owner = source_owner_value.strip() if isinstance(source_owner_value, str) else ""
                    source_repo = source_repo_value.strip() if isinstance(source_repo_value, str) else ""
                    if source_owner and source_repo:
                        source_id = f"{source_owner}/{source_repo}"
                if not source_id:
                    profile = self._emulator_profile_for_entry(emulator)
                    profile_source = profile.get("source") if isinstance(profile, dict) else None
                    if isinstance(profile_source, dict):
                        owner_value = profile_source.get("owner", "")
                        repo_value = profile_source.get("repo", profile_source.get("repository", ""))
                        owner = owner_value.strip() if isinstance(owner_value, str) else ""
                        repo = repo_value.strip() if isinstance(repo_value, str) else ""
                        if owner and repo:
                            source_id = f"{owner}/{repo}"
                if source_id:
                    installed_source_ids.append(source_id)

        return available_source_download_emulator_entries(
            self._emulator_autoprofiles(),
            query=query,
            installed_emulator_names=installed_emulator_names,
            installed_source_ids=installed_source_ids,
        )

    def _source_download_entry_from_metadata(
        self,
        source_metadata: dict[str, Any],
        *,
        display_name: str = "",
    ) -> dict[str, Any] | None:
        if not isinstance(source_metadata, dict):
            return None

        provider_value = source_metadata.get("provider", "")
        owner_value = source_metadata.get("owner", "")
        repo_value = source_metadata.get("repo", source_metadata.get("repository", ""))
        provider = provider_value.strip() if isinstance(provider_value, str) else ""
        owner = owner_value.strip() if isinstance(owner_value, str) else ""
        repo = repo_value.strip() if isinstance(repo_value, str) else ""
        if not owner or not repo:
            return None

        release_tag = ""
        for key in ("release_tag", "tag", "version"):
            value = source_metadata.get(key, "")
            if isinstance(value, str) and value.strip():
                release_tag = value.strip()
                break
        if not release_tag:
            release_tag = "latest"

        source_id = f"{owner}/{repo}"
        source_rows = self._available_source_download_emulator_entries(query=repo)
        for row in source_rows:
            row_source_id_value = row.get("source_id", "")
            row_source_id = row_source_id_value.strip() if isinstance(row_source_id_value, str) else ""
            if not row_source_id or row_source_id.casefold() != source_id.casefold():
                continue
            resolved = dict(row)
            if display_name:
                resolved["name"] = display_name
            merged_source = row.get("source_metadata")
            merged_source_metadata = dict(merged_source) if isinstance(merged_source, dict) else {}
            merged_source_metadata.update(dict(source_metadata))
            resolved["source_metadata"] = merged_source_metadata
            resolved["release_tag"] = release_tag
            return resolved

        return {
            "name": display_name or repo or "Emulator",
            "provider": provider or "github",
            "owner": owner,
            "repo": repo,
            "release_tag": release_tag,
            "source_id": source_id,
            "source_metadata": dict(source_metadata),
        }

    def _source_download_entry_for_emulator_name(
        self,
        emulator_name: str,
        emulator: dict[str, str] | None = None,
    ) -> dict[str, Any] | None:
        name = emulator_name.strip() if isinstance(emulator_name, str) else ""

        if isinstance(emulator, dict):
            source_id_value = emulator.get("source_id", "")
            source_id = source_id_value.strip() if isinstance(source_id_value, str) else ""
            source_metadata: dict[str, Any] = {}
            for source_field, metadata_key in (
                ("source_provider", "provider"),
                ("source_owner", "owner"),
                ("source_repo", "repo"),
                ("source_release_tag", "release_tag"),
            ):
                raw_value = emulator.get(source_field, "")
                if isinstance(raw_value, str) and raw_value.strip():
                    source_metadata[metadata_key] = raw_value.strip()
            if source_id and "/" in source_id:
                source_owner, _, source_repo = source_id.partition("/")
                source_metadata.setdefault("owner", source_owner.strip())
                source_metadata.setdefault("repo", source_repo.strip())
            if source_metadata:
                resolved = self._source_download_entry_from_metadata(source_metadata, display_name=name)
                if resolved is not None:
                    return resolved

            profile = self._emulator_profile_for_entry(emulator)
            profile_source = profile.get("source") if isinstance(profile, dict) else None
            if isinstance(profile_source, dict):
                resolved = self._source_download_entry_from_metadata(profile_source, display_name=name)
                if resolved is not None:
                    return resolved

            if source_id:
                installs = self._emulator_source_installs()
                install_entry = installs.get(source_id.casefold())
                if isinstance(install_entry, dict):
                    resolved = self._source_download_entry_from_metadata(install_entry, display_name=name)
                    if resolved is not None:
                        return resolved

        if not name:
            return None

        source_rows = self._available_source_download_emulator_entries(query=name)
        for row in source_rows:
            row_name_value = row.get("name", "")
            row_name = row_name_value.strip() if isinstance(row_name_value, str) else ""
            if row_name and row_name.casefold() == name.casefold():
                return dict(row)

        installs = self._emulator_source_installs()
        for _source_id, install_entry in installs.items():
            install_name_value = install_entry.get("name", "")
            install_name = install_name_value.strip() if isinstance(install_name_value, str) else ""
            if not install_name or install_name.casefold() != name.casefold():
                continue
            resolved = self._source_download_entry_from_metadata(install_entry, display_name=name)
            if resolved is not None:
                return resolved

        return None

    def _emulator_entry_with_source_identity(self, emulator: dict[str, str]) -> dict[str, str]:
        if not isinstance(emulator, dict):
            return {}

        resolved_entry = dict(emulator)
        source_lookup = getattr(self, "_source_download_entry_for_emulator_name", None)
        if not callable(source_lookup):
            return resolved_entry

        source_entry = source_lookup(
            str(resolved_entry.get("name", "")),
            resolved_entry,
        )
        if source_entry is None:
            return resolved_entry

        source_metadata = source_entry.get("source_metadata")
        if not isinstance(source_metadata, dict):
            return resolved_entry

        provider_value = source_metadata.get("provider", source_entry.get("provider", ""))
        owner_value = source_metadata.get("owner", source_entry.get("owner", ""))
        repo_value = source_metadata.get("repo", source_entry.get("repo", ""))
        release_tag_value = source_entry.get("release_tag", source_metadata.get("release_tag", "latest"))
        provider = provider_value.strip() if isinstance(provider_value, str) else ""
        owner = owner_value.strip() if isinstance(owner_value, str) else ""
        repo = repo_value.strip() if isinstance(repo_value, str) else ""
        release_tag = release_tag_value.strip() if isinstance(release_tag_value, str) else ""
        if owner and repo:
            resolved_entry["source_id"] = f"{owner}/{repo}"
            resolved_entry["source_provider"] = provider or "github"
            resolved_entry["source_owner"] = owner
            resolved_entry["source_repo"] = repo
            resolved_entry["source_release_tag"] = release_tag or "latest"
        return resolved_entry

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
        emulator_rows = self._normalize_emulators(self._emulators())
        if index < 0 or index >= len(emulator_rows):
            return

        emulator = emulator_rows[index]
        emulator_name = str(emulator.get("name", "")).strip()
        print(f"[DEBUG] Update from source clicked for: {emulator_name}")
        source_entry = self._source_download_entry_for_emulator_name(emulator_name)
        if source_entry is None:
            print("[DEBUG] No source entry found")
        else:
            print(f"[DEBUG] source_entry: {source_entry}")
        if source_entry is None:
            QMessageBox.information(
                self,
                "No Source",
                "No source download configured for this emulator.",
            )
            return

        source_id_value = source_entry.get("source_id", "")
        source_id = source_id_value.strip() if isinstance(source_id_value, str) else ""
        installed_info = self._emulator_source_installs().get(source_id, {})
        installed_tag_value = installed_info.get("release_tag", "latest") if isinstance(installed_info, dict) else "latest"
        installed_tag = installed_tag_value.strip() if isinstance(installed_tag_value, str) else "latest"
        if not installed_tag:
            installed_tag = "latest"
        print(f"[DEBUG] installed_tag: {installed_tag}")

        source_check_thread = getattr(self, "_source_check_thread", None)
        source_check_running = False
        if source_check_thread is not None:
            try:
                source_check_running = bool(source_check_thread.isRunning())
            except Exception:
                source_check_running = False
        print(f"[DEBUG] Concurrent check guard: thread={source_check_thread}, running={source_check_running}")
        if source_check_thread is not None and source_check_running:
            return

        self._source_check_emulator_name = emulator_name
        self._source_check_index = index
        source_metadata = source_entry.get("source_metadata", {})
        worker = SourceVersionCheckWorker(source_metadata, installed_tag)
        thread = QThread(self)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.finished.connect(self._on_source_version_check_finished_slot)
        worker.finished.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(lambda: setattr(self, "_source_check_thread", None))
        thread.finished.connect(lambda: setattr(self, "_source_check_worker", None))
        print("[DEBUG] Starting SourceVersionCheckWorker thread")
        thread.start()
        self._source_check_thread = thread
        self._source_check_worker = worker

    def _do_start_source_emulator_update_at_index(self, index: int) -> None:
        emulators = self._normalize_emulators(self._emulators())
        if index < 0 or index >= len(emulators):
            return

        emulator = emulators[index]
        source_entry = self._source_download_entry_for_emulator_name(
            emulator.get("name", ""),
            emulator,
        )
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

    def _on_source_version_check_finished_slot(self, bundle: object) -> None:
        installed_tag = bundle.get("installed_tag", "") if isinstance(bundle, dict) else ""
        available_tag = bundle.get("available_tag", "") if isinstance(bundle, dict) else ""
        error_msg = bundle.get("error", "") if isinstance(bundle, dict) else str(bundle)
        index = getattr(self, "_source_check_index", 0)
        self._on_source_version_check_finished(installed_tag, available_tag, error_msg, index)

    def _on_source_version_check_finished(
        self, installed_tag: str, available_tag: str, error_msg: str, index: int
    ) -> None:
        print(
            f"[DEBUG] _on_source_version_check_finished called: installed={installed_tag} "
            f"available={available_tag} error={error_msg} index={index}"
        )
        if error_msg:
            print("[DEBUG] Branch: error")
            QMessageBox.warning(self, "Version Check Failed", f"Could not check for updates:\n{error_msg}")
            return

        installed_display = installed_tag if installed_tag and installed_tag.casefold() != "latest" else "unknown"
        if available_tag == "direct":
            available_display = "Unknown (direct source)"
        else:
            available_display = available_tag

        if (
            installed_tag
            and available_tag
            and installed_tag != "latest"
            and available_tag != "direct"
            and installed_tag.casefold() == available_tag.casefold()
        ):
            print("[DEBUG] Branch: no updates")
            QMessageBox.information(self, "No Updates Available", f"Already up to date ({available_display}).")
            return

        emulator_name = getattr(self, "_source_check_emulator_name", "emulator")
        print("[DEBUG] Branch: show update dialog")
        print("[DEBUG] _show_update_dialog() fired")
        try:
            reply = QMessageBox.question(
                self,
                "Update Available",
                f"Update {emulator_name}?\n\nInstalled: {installed_display}\nAvailable: {available_display}",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No,
            )
            print(f"[DEBUG] QMessageBox.question returned: {reply}")
            if reply == QMessageBox.StandardButton.Yes:
                self._do_start_source_emulator_update_at_index(index)
        except Exception as e:
            import traceback
            print(f"[DEBUG] Exception in update dialog: {e}")
            traceback.print_exc()

    def _extract_emulator_archive(self, emulator_name: str, archive_path: Path) -> tuple[str, str]:
        library_path = self._library_path_dir()
        if library_path is None:
            QMessageBox.warning(self, "Validation", "Set a Library Path in Settings before adding an emulator archive.")
            return "", ""

        extract_target_dir = emulator_install_directory(library_path, emulator_name)
        try:
            extract_archive_into_directory(archive_path, extract_target_dir)
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

        _config_exclusion_list = self.config.get("tv_guide_button_exclusion_list", [])
        if not isinstance(_config_exclusion_list, list):
            _config_exclusion_list = []
        _config_exclusion_lower = {e.lower() for e in _config_exclusion_list if isinstance(e, str)}
        _opt_outs_raw = self.config.get("tv_guide_button_default_opt_outs", [])
        _opt_outs_lower = {e.lower() for e in _opt_outs_raw if isinstance(e, str)} if isinstance(_opt_outs_raw, list) else set()
        _existing_name_lower = existing_entry.get("name", "").strip().lower() if existing_entry else ""
        _is_guide_default = _existing_name_lower in _TV_GUIDE_DEFAULT_EXCLUSIONS
        _is_guide_excluded = (
            (_is_guide_default and _existing_name_lower not in _opt_outs_lower)
            or (not _is_guide_default and _existing_name_lower in _config_exclusion_lower)
        )

        dialog = EmulatorConfigDialog(
            self,
            emulator=existing_entry,
            is_new_entry=selected_row < 0,
            save_strategy_values=["auto", "single_file", "folder"],
            guide_button_excluded=_is_guide_excluded,
            is_guide_button_default_locked=_is_guide_default,
        )
        if dialog.exec() != QDialog.DialogCode.Accepted:
            return

        entry = dialog.entry_payload()
        entry_name = entry.get("name", "").strip()
        if _is_guide_default:
            _opt_outs = self.config.get("tv_guide_button_default_opt_outs", [])
            if not isinstance(_opt_outs, list):
                _opt_outs = []
            if not dialog.guide_button_excluded():
                if entry_name.lower() not in {e.lower() for e in _opt_outs if isinstance(e, str)}:
                    _opt_outs = _opt_outs + [entry_name]
            else:
                _opt_outs = [e for e in _opt_outs if isinstance(e, str) and e.lower() != entry_name.lower()]
            self.config["tv_guide_button_default_opt_outs"] = _opt_outs
        else:
            _exclusion_list = self.config.get("tv_guide_button_exclusion_list", [])
            if not isinstance(_exclusion_list, list):
                _exclusion_list = []
            if dialog.guide_button_excluded():
                _entry_name_lower = entry_name.lower()
                _existing_in_list_lower = {e.lower() for e in _exclusion_list if isinstance(e, str)}
                if _entry_name_lower not in _TV_GUIDE_DEFAULT_EXCLUSIONS and _entry_name_lower not in _existing_in_list_lower:
                    _exclusion_list = _exclusion_list + [entry_name]
            else:
                _exclusion_list = [e for e in _exclusion_list if isinstance(e, str) and e.lower() != entry_name.lower()]
            self.config["tv_guide_button_exclusion_list"] = _exclusion_list

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
        existing_entry = emulators[target_index] if not is_new_manual_entry else {}
        if not isinstance(existing_entry, dict):
            existing_entry = {}

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
            source_id=str(existing_entry.get("source_id", "")),
            source_provider=str(existing_entry.get("source_provider", "")),
            source_owner=str(existing_entry.get("source_owner", "")),
            source_repo=str(existing_entry.get("source_repo", "")),
            source_release_tag=str(existing_entry.get("source_release_tag", "")),
        )
        entry = self._emulator_entry_with_source_identity(entry)

        if is_new_manual_entry:
            entry = apply_manual_emulator_profile_defaults(
                entry,
                self._emulator_autoprofiles(),
                emulator_profile_for_entry=emulator_profile_for_entry,
                normalize_save_strategy_value=self._normalize_save_strategy_value,
            )

        self.config["emulators"] = self._normalize_emulators(
            upsert_emulator_entry(emulators, entry, target_index)
        )

        if is_new_manual_entry:
            profile = self._emulator_profile_for_entry(entry)
            if isinstance(profile, dict):
                defaults, core_defaults = assign_profile_platform_defaults(
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
            self._show_toast({"message": f"Added emulator '{name}'.", "level": "success"})

    def _remove_emulator_at_index(self, index: int) -> None:
        emulators = self._emulators()
        if index < 0 or index >= len(emulators):
            return

        emulator_to_remove = emulators[index]
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
            process = subprocess.Popen(
                command,
                cwd=str(emulator_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if sys.platform == "win32" else 0,
            )
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

    def _is_duckstation_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "duckstation", emulator=emulator)

    def _trigger_rpcs3_firmware_download_background(
        self, emulator_entry: dict, path_text: str
    ) -> None:
        if rpcs3_pup_path(path_text) is not None:
            return

        firmware_dirs = self._resolved_firmware_directories(emulator_entry)
        if not firmware_dirs:
            return

        platform_id: int | None = None
        platform_ids = getattr(self, "server_platform_ids", {})
        for key, val in platform_ids.items():
            if isinstance(key, str) and ("playstation 3" in key.lower() or key.lower() == "ps3"):
                if isinstance(val, int):
                    platform_id = val
                    break

        api_get = getattr(self, "_api_get", None) if platform_id is not None else None
        api_get_bytes = getattr(self, "_api_get_bytes", None) if platform_id is not None else None

        firmware_game: dict[str, str] = {"title": "PS3 Firmware", "platform": "PlayStation 3"}
        entry_id = self._create_download_entry(firmware_game, "downloading")
        self._firmware_download_entry_id = entry_id
        self.active_download_count += 1
        self.active_download_bytes = 0
        self.active_download_total = 0
        self.active_download_speed_bps = 0.0
        self._update_download_status_ui()

        def _worker() -> None:
            def _progress(downloaded: int, total: int, speed: float) -> None:
                self._firmware_download_progress.emit({"downloaded": downloaded, "total": total, "speed": speed})

            warnings = download_ps3_firmware_direct(firmware_dirs, skip_existing=True, progress_callback=_progress)
            if warnings and platform_id is not None:
                try:
                    install_platform_firmware(
                        api_get,
                        api_get_bytes,
                        platform_id,
                        firmware_dirs,
                        skip_existing=True,
                    )
                except Exception:
                    pass
            error = warnings[0] if warnings else ""
            self._firmware_download_done.emit(error)
            self._emulator_refresh_requested.emit()

        t = threading.Thread(target=_worker, daemon=True)
        t.start()

    def _trigger_firmware_install_for_source_emulator(self, emulator_name: str) -> None:
        """Download and install firmware for a freshly installed source emulator.

        RPCS3 is skipped — its firmware is already handled by
        _trigger_rpcs3_firmware_download_background.  All other emulators
        that have firmware_directories entries have their server firmware
        fetched into those directories in a background thread.

        Only call this on a fresh install (not a source update).
        """
        if self._is_rpcs3_emulator_name(emulator_name):
            return

        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None:
            return

        firmware_dirs = self._resolved_firmware_directories(emulator_entry)
        if not firmware_dirs:
            return

        profile = self._emulator_profile_for_entry(emulator_entry)
        if not isinstance(profile, dict):
            return

        platform_ids = getattr(self, "server_platform_ids", {})
        if profile.get("all_platforms") and self._is_retroarch_emulator_name(emulator_name):
            platform_id_list = [
                v
                for p, v in platform_ids.items()
                if isinstance(v, int) and self._retroarch_cores_for_platform(p)
            ]
        elif profile.get("all_platforms"):
            platform_id_list = [v for v in platform_ids.values() if isinstance(v, int)]
        else:
            platform_keywords_raw = profile.get("platform_keywords", [])
            platform_keywords = (
                [str(k).strip() for k in platform_keywords_raw if isinstance(k, str) and str(k).strip()]
                if isinstance(platform_keywords_raw, list)
                else []
            )
            if not platform_keywords:
                return
            matching_platforms = self._matching_platforms_for_emulator_keywords(platform_keywords)
            platform_id_list = [
                platform_ids[p]
                for p in matching_platforms
                if isinstance(platform_ids.get(p), int)
            ]
        if not platform_id_list:
            return

        api_get = getattr(self, "_api_get", None)
        api_get_bytes = getattr(self, "_api_get_bytes", None)
        if not callable(api_get) or not callable(api_get_bytes):
            return

        captured_firmware_dirs = list(firmware_dirs)

        def _worker() -> None:
            for platform_id in platform_id_list:
                try:
                    install_platform_firmware(
                        api_get,
                        api_get_bytes,
                        platform_id,
                        captured_firmware_dirs,
                        skip_existing=True,
                    )
                except Exception:
                    pass
            self._emulator_refresh_requested.emit()

        threading.Thread(target=_worker, daemon=True).start()

    def _is_retroarch_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return self._emulator_matches_tokens(emulator_name, "retroarch", emulator=emulator)

