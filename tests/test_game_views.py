from __future__ import annotations

import importlib.util
import os
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QApplication, QAbstractItemView, QComboBox, QFrame, QLabel, QLineEdit, QPushButton, QScrollArea

from grid_launcher.ui.game_views import make_game_card, open_game_details, update_details_action_buttons
from grid_launcher.ui.mixins.emulator_ui_mixin import EmulatorUIMixin
from grid_launcher.ui.theme import apply_theme_inline_styles


class _StubButton:
    def __init__(self) -> None:
        self.text = ""
        self.visible: bool | None = None
        self.enabled: bool | None = None
        self.checked: bool | None = None
        self.tooltip = ""

    def setText(self, value: str) -> None:
        self.text = value

    def setVisible(self, value: bool) -> None:
        self.visible = value

    def setEnabled(self, value: bool) -> None:
        self.enabled = value

    def setChecked(self, value: bool) -> None:
        self.checked = value

    def setToolTip(self, value: str) -> None:
        self.tooltip = value


class _StubWindow:
    def __init__(
        self,
        *,
        platform: str,
        save_visible: bool,
        state_visible: bool,
        save_label: str = "Manage Saves",
        game_overrides: dict[str, str] | None = None,
    ) -> None:
        self.current_details_game = {"title": "Test Game", "platform": platform}
        if isinstance(game_overrides, dict):
            self.current_details_game.update({key: str(value) for key, value in game_overrides.items()})
        self.current_details_cloud_mode = "overview"
        self.details_title_label = None
        self.details_cover_label = None
        self.details_platform_label = None
        self.details_genres_label = None
        self.details_regions_label = None
        self.details_filesize_label = None
        self.details_genres_label = None
        self.details_regions_label = None
        self.details_filesize_label = None
        self.details_version_label = None
        self.details_rating_label = None
        self.details_description_label = None
        self.details_companies_group = None
        self.details_companies_label = None
        self.details_release_date_group = None
        self.details_release_date_label = None
        self.details_languages_group = None
        self.details_languages_label = None
        self.details_primary_button = _StubButton()
        self.details_config_button = _StubButton()
        self.details_details_button = _StubButton()
        self.details_manage_saves_button = _StubButton()
        self.details_manage_states_button = _StubButton()
        self.details_secondary_button = _StubButton()
        self.details_update_button = _StubButton()
        self.stack = None
        self.nav_buttons: list[_StubButton] = []
        self.install_in_progress = False
        self.install_finalize_in_progress = False
        self.install_pending_game = None
        self.install_finalize_game = None
        self._save_visible = save_visible
        self._state_visible = state_visible
        self._save_label = save_label

    def _queue_game_cover_load(self, game: dict[str, str], label: object) -> None:
        return None

    def _update_details_screenshots(self, game: dict[str, str]) -> None:
        return None

    def _update_details_action_buttons(self) -> None:
        return None

    def _update_details_layout_metrics(self) -> None:
        return None

    def _show_details_overview(self) -> None:
        self.current_details_cloud_mode = "overview"

    def _is_emulators_platform(self, game: dict[str, str]) -> bool:
        return game.get("platform", "").strip().casefold() == "emulators"

    def _is_game_installed(self, game: dict[str, str]) -> bool:
        return True

    def _install_block_reason_for_game(self, game: dict[str, str]) -> str:
        return ""

    def _is_game_install_queued(self, game: dict[str, str]) -> bool:
        return False

    def _game_key(self, game: dict[str, str]) -> tuple[str, str]:
        return (game.get("title", ""), game.get("platform", ""))

    def _is_native_executable_platform(self, game: dict[str, str]) -> bool:
        return False

    def _resolved_emulator_entry_for_game(self, game: dict[str, str]) -> tuple[str, dict[str, str] | None]:
        return ("Shared Emulator", {"name": "Shared Emulator"})

    def _is_rpcs3_emulator_name(self, emulator_name: str) -> bool:
        return False

    def _details_cloud_mode_supported(self, game: dict[str, str], save_type: str) -> bool:
        return self._save_visible if save_type == "save" else self._state_visible

    def _details_cloud_button_text(self, game: dict[str, str], save_type: str) -> str:
        if save_type == "save":
            return self._save_label
        return "Manage States"


class _CardStubWindow:
    def __init__(self) -> None:
        self.queued_game: dict[str, str] | None = None
        self.queued_label: QLabel | None = None

    def _open_game_details(self, game: dict[str, str], source: str) -> None:
        return None

    def _queue_game_cover_load(self, game: dict[str, str], label: QLabel) -> None:
        self.queued_game = game
        self.queued_label = label

    def _theme_color(self, role: str, fallback: str) -> str:
        return fallback


class GameCardStyleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def test_make_game_card_uses_transparent_cover_background(self) -> None:
        window = _CardStubWindow()
        game = {
            "title": "Test Game",
            "platform": "Dreamcast",
            "rating": "5/5",
            "description": "A test game.",
        }

        card = make_game_card(window, game, "library")

        labels = card.findChildren(QLabel)
        self.assertGreaterEqual(len(labels), 3)
        cover = labels[0]
        self.assertIs(window.queued_label, cover)
        self.assertIn("background-color: transparent", cover.styleSheet())

    def test_make_game_card_shows_update_indicator_when_update_available(self) -> None:
        window = _CardStubWindow()
        game = {
            "title": "Test Game",
            "platform": "Windows",
            "rating": "5/5",
            "description": "A test game.",
            "update_available": "true",
        }

        card = make_game_card(window, game, "library")

        update_indicator = card.findChild(QLabel, "gameCardUpdateIndicator")
        self.assertIsNotNone(update_indicator)
        assert update_indicator is not None
        self.assertEqual(update_indicator.text(), "Update Available")
        self.assertFalse(update_indicator.isHidden())

    def test_apply_theme_inline_styles_keeps_details_cover_background_transparent(self) -> None:
        label = QLabel()

        apply_theme_inline_styles({"window": "#101010", "border": "#202020"}, details_cover_label=label)

        self.assertIn("background-color: transparent", label.styleSheet())


class GameViewCloudButtonTests(unittest.TestCase):
    def test_update_details_action_buttons_hides_cloud_buttons_for_emulator_entries(self) -> None:
        window = _StubWindow(platform="Emulators", save_visible=False, state_visible=False)

        update_details_action_buttons(window)

        self.assertFalse(window.details_manage_saves_button.visible)
        self.assertFalse(window.details_manage_states_button.visible)

    def test_update_details_action_buttons_shows_emulator_saves_for_shared_platforms(self) -> None:
        window = _StubWindow(
            platform="Dreamcast",
            save_visible=True,
            state_visible=True,
            save_label="Emulator Saves",
        )

        update_details_action_buttons(window)

        self.assertTrue(window.details_manage_saves_button.visible)
        self.assertEqual(window.details_manage_saves_button.text, "Emulator Saves")
        self.assertTrue(window.details_manage_states_button.visible)

    def test_update_details_action_buttons_shows_emulator_saves_for_shared_emulator_entries(self) -> None:
        window = _StubWindow(
            platform="Emulators",
            save_visible=True,
            state_visible=False,
            save_label="Emulator Saves",
        )

        update_details_action_buttons(window)

        self.assertTrue(window.details_manage_saves_button.visible)
        self.assertEqual(window.details_manage_saves_button.text, "Emulator Saves")
        self.assertFalse(window.details_manage_states_button.visible)

    def test_update_details_action_buttons_shows_update_action_when_installed_game_has_update(self) -> None:
        window = _StubWindow(
            platform="PlayStation 4",
            save_visible=False,
            state_visible=False,
            game_overrides={"ps4_has_update": "true"},
        )

        update_details_action_buttons(window)

        self.assertEqual(window.details_secondary_button.text, "Uninstall")
        self.assertTrue(window.details_secondary_button.visible)
        self.assertTrue(window.details_secondary_button.enabled)
        self.assertTrue(window.details_update_button.text.startswith("Update"))
        self.assertTrue(window.details_update_button.visible)
        self.assertTrue(window.details_update_button.enabled)

    def test_update_details_action_buttons_keeps_uninstall_action_when_no_update_available(self) -> None:
        window = _StubWindow(
            platform="PlayStation 4",
            save_visible=False,
            state_visible=False,
            game_overrides={"ps4_has_update": "false"},
        )

        update_details_action_buttons(window)

        self.assertEqual(window.details_secondary_button.text, "Uninstall")
        self.assertTrue(window.details_secondary_button.visible)
        self.assertTrue(window.details_secondary_button.enabled)
        self.assertFalse(window.details_update_button.visible)
        self.assertFalse(window.details_update_button.enabled)

    def test_update_details_action_buttons_keeps_emulator_entries_hidden_even_with_update_flag(self) -> None:
        window = _StubWindow(
            platform="Emulators",
            save_visible=False,
            state_visible=False,
            game_overrides={"ps4_has_update": "true"},
        )

        update_details_action_buttons(window)

        self.assertFalse(window.details_secondary_button.visible)
        self.assertFalse(window.details_update_button.visible)


class _ToggleStubStack:
    def __init__(self) -> None:
        self.index: int | None = None

    def setCurrentIndex(self, value: int) -> None:
        self.index = value


class _ToggleStubWindow:
    def __init__(self) -> None:
        self.current_details_cloud_mode = "overview"
        self.current_details_game = {"title": "Test Game", "platform": "Dreamcast"}
        self.details_details_button = _StubButton()
        self.details_manage_saves_button = _StubButton()
        self.details_manage_states_button = _StubButton()
        self.details_center_stack = _ToggleStubStack()
        self.refresh_calls = 0
        self.layout_calls = 0
        self.loading_calls = 0

    def _show_details_overview(self) -> None:
        self.current_details_cloud_mode = "overview"

    def _details_cloud_mode_supported(self, game: dict[str, str], save_type: str) -> bool:
        return True

    def _details_cloud_button_visible(self, save_type: str) -> bool:
        return True

    def _show_details_cloud_loading_state(self, save_type: str) -> None:
        self.loading_calls += 1
        self.details_center_stack.setCurrentIndex(1)

    def _refresh_details_cloud_panel(self) -> None:
        self.refresh_calls += 1

    def _update_details_layout_metrics(self) -> None:
        self.layout_calls += 1


class _SettingsPageStubWindow:
    def __init__(self) -> None:
        self.config = {
            "server_url": "https://example.test",
            "api_token": "secret-token",
            "library_path": "C:/Games",
            "theme": "system",
            "retroachievements_token": "",
        }
        self.server_url_input = None
        self.api_token_input = None
        self.library_path_input = None
        self.theme_input = None
        self.debug_prints_checkbox = None
        self.auto_cloud_download_checkbox = None
        self.auto_cloud_upload_checkbox = None
        self.auto_cloud_skip_local_newer_checkbox = None
        self.settings_status_label = None

    def _make_section_title(self, text: str) -> QLabel:
        return QLabel(text)

    def _normalized_theme_choice(self, value: object) -> str:
        return str(value).strip().casefold() if isinstance(value, str) and value else "system"

    def _debug_prints_enabled(self) -> bool:
        return True

    def _auto_cloud_save_download_enabled(self) -> bool:
        return True

    def _auto_cloud_save_upload_enabled(self) -> bool:
        return True

    def _auto_cloud_skip_download_if_local_newer(self) -> bool:
        return True

    def _theme_color(self, role: str, fallback: str) -> str:
        return fallback

    def _connect_from_settings(self) -> None:
        return None

    def _disconnect_from_server(self) -> None:
        return None

    def _browse_library_path(self) -> None:
        return None

    def _on_theme_selection_changed(self, value: str) -> None:
        return None

    def _save_settings(self) -> None:
        return None

    def _ra_login_clicked(self):
        pass

    def _ra_clear_credentials(self):
        pass

    def _on_ra_login_finished(self, username, token, error):
        pass

    def _open_config_folder(self) -> None:
        return None


class _EmulatorsPageStubWindow:
    def __init__(self) -> None:
        self.config = {
            "emulators": [],
        }
        self.emulator_list = None
        self.emulator_name_input = None
        self.emulator_path_input = None
        self.emulator_args_input = None
        self.save_emulator_button = None
        self.emulator_save_strategy_input = None
        self.emulator_ignore_files_input = None
        self.emulator_ignore_extensions_input = None
        self.emulator_save_paths_input = None
        self.emulator_state_paths_input = None
        self.default_platform_combo = None
        self.default_emulator_combo = None
        self.default_core_combo = None
        self.default_mapping_list = None

    def _make_section_title(self, text: str) -> QLabel:
        return QLabel(text)

    def _emulators(self) -> list[dict[str, str]]:
        value = self.config.get("emulators", [])
        return value if isinstance(value, list) else []

    def _load_emulator_from_selection(self, row: int) -> None:
        return None

    def _launch_selected_emulator(self) -> None:
        return None

    def _browse_emulator_path(self) -> None:
        return None

    def _save_emulator(self) -> None:
        return None

    def _clear_emulator_selection(self) -> None:
        return None

    def _remove_emulator(self) -> None:
        return None

    def _on_default_platform_changed(self, value: str) -> None:
        return None

    def _refresh_retroarch_core_options(self, value: str = "") -> None:
        return None

    def _set_default_emulator(self) -> None:
        return None


class _EmulatorRowsStubWindow:
    def __init__(self, *, source_available: bool = False) -> None:
        self.config = {
            "emulators": [
                {
                    "name": "DuckStation",
                    "path": "C:/Emulators/duckstation.exe",
                    "args": "%rom%",
                    "save_strategy": "auto",
                    "ignore_files": "",
                    "ignore_extensions": "",
                    "save_paths": "",
                    "state_paths": "",
                }
            ],
            "default_emulators": {},
            "default_retroarch_cores": {},
        }
        self.emulator_list = None
        self.default_platform_combo = None
        self.default_mapping_list = None
        self.source_available = source_available
        self._retroarch_core_ids_cache: dict[str, set[str]] = {}
        self._platform_default_emulator_cache: dict[str, str] = {}
        self._platform_available_emulator_cache: dict[str, str] = {}
        self.server_platform_ids: dict[str, int] = {}
        self.library_games: list[dict[str, str]] = []

    def _emulators(self) -> list[dict[str, str]]:
        value = self.config.get("emulators", [])
        return value if isinstance(value, list) else []

    def _normalize_emulators(self, value: object) -> list[dict[str, str]]:
        if isinstance(value, list):
            return [item for item in value if isinstance(item, dict)]
        return []

    def _default_assignable_server_platforms(self) -> list[str]:
        return []

    def _normalize_default_emulators(self, value: object) -> dict[str, str]:
        if isinstance(value, dict):
            return {str(key): str(item) for key, item in value.items()}
        return {}

    def _normalize_default_retroarch_cores(self, value: object) -> dict[str, str]:
        if isinstance(value, dict):
            return {str(key): str(item) for key, item in value.items()}
        return {}

    def _is_retroarch_emulator_name(self, emulator_name: str) -> bool:
        return emulator_name.strip().casefold() == "retroarch"

    def _is_azahar_emulator_name(
        self,
        emulator_name: str,
        emulator: dict[str, str] | None = None,
    ) -> bool:
        del emulator
        return emulator_name.strip().casefold() == "azahar"

    def _is_eden_emulator_name(
        self,
        emulator_name: str,
        emulator: dict[str, str] | None = None,
    ) -> bool:
        del emulator
        return emulator_name.strip().casefold() == "eden"

    def _is_xemu_emulator_name(
        self,
        emulator_name: str,
        emulator: dict[str, str] | None = None,
    ) -> bool:
        del emulator
        normalized_name = emulator_name.strip().casefold()
        return any(token in normalized_name for token in ("xemu", "xemu.exe"))

    def _is_duckstation_emulator_name(
        self,
        emulator_name: str,
        emulator: dict[str, str] | None = None,
    ) -> bool:
        del emulator
        normalized_name = emulator_name.strip().casefold()
        return any(token in normalized_name for token in ("duckstation", "duckstation.exe"))

    def _is_rpcs3_emulator_name(self, emulator_name: str) -> bool:
        normalized_name = emulator_name.strip().casefold()
        return any(token in normalized_name for token in ("rpcs3", "rpcs3.exe"))

    def _on_default_platform_changed(self, platform: str) -> None:
        return None

    def _warm_emulator_platform_caches(self) -> None:
        return None

    def _remove_emulator_at_index(self, index: int) -> None:
        return None

    def _source_download_entry_for_emulator_name(
        self,
        emulator_name: str,
        emulator: dict[str, str] | None = None,
    ) -> dict[str, str] | None:
        del emulator
        if not self.source_available:
            return None
        return {
            "name": emulator_name,
            "provider": "github",
            "owner": "stenzek",
            "repo": "duckstation",
            "release_tag": "latest",
            "source_id": "stenzek/duckstation",
            "source_metadata": {
                "provider": "github",
                "owner": "stenzek",
                "repo": "duckstation",
                "release_tag": "latest",
            },
        }

    def _start_source_emulator_update_at_index(self, index: int) -> None:
        return None


class _SourceDownloadDialogStubWindow:
    def __init__(self) -> None:
        self._sanitizer_calls: list[tuple[str, str]] = []
        self.install_game: dict[str, str] | None = None

    def _emulators(self) -> list[dict[str, str]]:
        return []

    def _normalize_emulators(self, value: object) -> list[dict[str, str]]:
        if isinstance(value, list):
            return [item for item in value if isinstance(item, dict)]
        return []

    def _available_source_download_emulator_entries(
        self,
        query: str = "",
        installed_emulator_names: list[str] | None = None,
    ) -> list[dict[str, str]]:
        return [
            {
                "name": "DuckStation",
                "provider": "github",
                "owner": "stenzek",
                "repo": "duckstation",
                "release_tag": "latest",
                "source_id": "stenzek/duckstation",
                "source_metadata": {
                    "provider": "github",
                    "owner": "stenzek",
                    "repo": "duckstation",
                    "release_tag": "latest",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name": "duckstation-windows-x64-release.zip",
                        }
                    ],
                },
            }
        ]

    def _sanitize_path_component(self, value: str, fallback: str) -> str:
        self._sanitizer_calls.append((value, fallback))
        normalized = "-".join(value.strip().split())
        return normalized or fallback

    def _start_async_install(self, game: dict[str, str]) -> None:
        self.install_game = game

    def _build_source_emulator_install_game(self, selected: dict[str, str], install_mode: str) -> dict[str, str]:
        source_metadata = selected.get("source_metadata", {})
        source_release_tag = selected.get("release_tag", "latest")
        return {
            "title": selected.get("name", "Emulator"),
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


class _SourceUpdateActionStubWindow:
    def __init__(self) -> None:
        self.install_game: dict[str, str] | None = None
        self.config = {
            "emulators": [
                {
                    "name": "DuckStation",
                    "path": "C:/Emulators/duckstation.exe",
                }
            ]
        }

    def _emulators(self) -> list[dict[str, str]]:
        value = self.config.get("emulators", [])
        return value if isinstance(value, list) else []

    def _normalize_emulators(self, value: object) -> list[dict[str, str]]:
        if isinstance(value, list):
            return [item for item in value if isinstance(item, dict)]
        return []

    def _emulator_source_installs(self) -> dict[str, dict[str, str]]:
        value = self.config.get("emulator_source_installs", {})
        return value if isinstance(value, dict) else {}

    def _source_download_entry_for_emulator_name(
        self,
        emulator_name: str,
        emulator: dict[str, str] | None = None,
    ) -> dict[str, str] | None:
        del emulator
        return {
            "name": emulator_name,
            "provider": "github",
            "owner": "stenzek",
            "repo": "duckstation",
            "release_tag": "latest",
            "source_id": "stenzek/duckstation",
            "source_metadata": {
                "provider": "github",
                "owner": "stenzek",
                "repo": "duckstation",
                "release_tag": "latest",
            },
        }

    def _build_source_emulator_install_game(self, selected: dict[str, str], install_mode: str) -> dict[str, str]:
        return {
            "title": selected.get("name", "Emulator"),
            "platform": "Emulators",
            "_install_mode": install_mode,
            "_source_id": selected.get("source_id", ""),
            "_source_release_tag": selected.get("release_tag", "latest"),
            "_source_metadata": selected.get("source_metadata", {}),
        }

    def _start_async_install(self, game: dict[str, str]) -> None:
        self.install_game = game


class _RenamedSourceLookupStubWindow(EmulatorUIMixin):
    def __init__(self) -> None:
        self.config = {
            "emulator_source_installs": {
                "dolphin-emu/dolphin": {
                    "name": "Dolphin",
                    "provider": "direct",
                    "owner": "dolphin-emu",
                    "repo": "dolphin",
                    "release_tag": "latest",
                    "installed_at": "2026-04-10T00:00:00Z",
                }
            }
        }

    def _available_source_download_emulator_entries(
        self,
        query: str = "",
        installed_emulator_names: list[str] | None = None,
    ) -> list[dict[str, str]]:
        del query, installed_emulator_names
        return [
            {
                "name": "Dolphin",
                "provider": "direct",
                "owner": "dolphin-emu",
                "repo": "dolphin",
                "release_tag": "latest",
                "source_id": "dolphin-emu/dolphin",
                "source_metadata": {
                    "provider": "direct",
                    "owner": "dolphin-emu",
                    "repo": "dolphin",
                    "release_tag": "latest",
                    "download_url": "https://example.test/dolphin.7z",
                },
            }
        ]

    def _emulator_source_installs(self) -> dict[str, dict[str, str]]:
        value = self.config.get("emulator_source_installs", {})
        return value if isinstance(value, dict) else {}

    def _emulator_profile_for_entry(self, emulator: dict[str, str]) -> dict[str, object] | None:
        path = str(emulator.get("path", "")).casefold()
        if path.endswith("dolphin.exe"):
            return {
                "name": "Dolphin",
                "source": {
                    "provider": "direct",
                    "owner": "dolphin-emu",
                    "repo": "dolphin",
                    "release_tag": "latest",
                    "download_url": "https://example.test/dolphin.7z",
                },
            }
        return None

class _DetailsPageStubWindow:
    def __init__(self) -> None:
        self.details_content_frame = None
        self.details_center_stack = None
        self.details_cover_label = None
        self.details_title_label = None
        self.details_platform_label = None
        self.details_version_label = None
        self.details_rating_label = None
        self.details_description_label = None
        self.details_cloud_title_label = None
        self.details_cloud_status_label = None
        self.details_cloud_empty_label = None
        self.details_cloud_upload_button = None
        self.details_cloud_list_layout = None
        self.details_details_button = None
        self.details_manage_saves_button = None
        self.details_manage_states_button = None
        self.details_primary_button = None
        self.details_config_button = None
        self.details_ps4_content_button = None
        self.details_xbox360_content_button = None
        self.details_secondary_button = None
        self.details_update_button = None
        self.details_screenshot_labels = []
        self.details_metadata_scalable_labels = []
        self.details_screenshots_panel = None
        self.details_screenshots_scroll = None
        self.overview_calls = 0
        self.layout_calls = 0

    def _return_from_details(self) -> None:
        return None

    def _perform_show_details_action(self) -> None:
        return None

    def _perform_manage_saves_action(self) -> None:
        return None

    def _perform_manage_states_action(self) -> None:
        return None

    def _perform_game_action(self) -> None:
        return None

    def _perform_game_config_action(self) -> None:
        return None

    def _perform_ps4_content_action(self) -> None:
        return None

    def _perform_xbox360_content_action(self) -> None:
        return None

    def _perform_game_secondary_action(self) -> None:
        return None

    def _perform_current_cloud_upload_action(self) -> None:
        return None

    def _open_achievements_panel(self) -> None:
        return None

    def _theme_color(self, role: str, fallback: str) -> str:
        return fallback

    def _show_details_overview(self) -> None:
        self.overview_calls += 1

    def _update_details_layout_metrics(self) -> None:
        self.layout_calls += 1


class GameViewCloudToggleTests(unittest.TestCase):
    def test_toggle_details_cloud_mode_switches_views_before_loading_records(self) -> None:
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        window = _ToggleStubWindow()
        scheduled: list[tuple[int, object]] = []

        with patch.object(module.QTimer, "singleShot", side_effect=lambda delay, callback: scheduled.append((delay, callback))):
            module.MainWindow._toggle_details_cloud_mode(window, "save")

        self.assertEqual(window.current_details_cloud_mode, "save")
        self.assertEqual(window.details_center_stack.index, 1)
        self.assertEqual(window.loading_calls, 1)
        self.assertEqual(window.refresh_calls, 0)
        self.assertEqual(window.layout_calls, 1)
        self.assertEqual(len(scheduled), 2)


class EmulatorSaveButtonLabelTests(unittest.TestCase):
    @staticmethod
    def _load_emulators_module():
        module_path = Path(__file__).resolve().parents[1] / "grid_launcher" / "ui" / "emulators.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_ui_emulators_for_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_save_button_label_returns_add_new_when_no_valid_row_selected(self) -> None:
        module = self._load_emulators_module()
        emulators = [{"name": "DuckStation"}]

        self.assertEqual(module.save_button_label(emulators, -1), "Add New")
        self.assertEqual(module.save_button_label(emulators, len(emulators)), "Add New")

    def test_save_button_label_returns_update_when_editing_existing_row(self) -> None:
        module = self._load_emulators_module()
        emulators = [
            {"name": "DuckStation"},
            {"name": "RetroArch"},
        ]

        self.assertEqual(module.save_button_label(emulators, 1), "Update")


class EmulatorSourceDownloadHelperTests(unittest.TestCase):
    @staticmethod
    def _load_emulators_module():
        module_path = Path(__file__).resolve().parents[1] / "grid_launcher" / "ui" / "emulators.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_ui_emulators_source_helpers_for_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_source_download_emulator_entries_returns_sorted_deduplicated_rows(self) -> None:
        module = self._load_emulators_module()
        autoprofiles = [
            {
                "name": "PPSSPP (Playstation Portable)",
                "source": {
                    "provider": "github-release",
                    "owner": "hrydgard",
                    "repo": "ppsspp",
                },
            },
            {
                "name": "DuckStation (Playstation 1)",
                "source": {
                    "provider": "github",
                    "owner": "stenzek",
                    "repo": "duckstation",
                    "release_tag": "latest",
                },
            },
            {
                "name": "duckstation (playstation 1)",
                "source": {
                    "provider": "github_release",
                    "owner": "Stenzek",
                    "repo": "DuckStation",
                },
            },
            {
                "name": "Missing Source",
            },
        ]

        rows = module.source_download_emulator_entries(autoprofiles)

        self.assertEqual([row["name"] for row in rows], [
            "DuckStation (Playstation 1)",
            "PPSSPP (Playstation Portable)",
        ])
        self.assertEqual(rows[0]["provider"], "github")
        self.assertEqual(rows[0]["source_id"], "stenzek/duckstation")
        self.assertEqual(rows[1]["release_tag"], "latest")

    def test_source_download_emulator_entries_preserves_source_metadata_windows_assets(self) -> None:
        module = self._load_emulators_module()
        autoprofiles = [
            {
                "name": "DuckStation (Playstation 1)",
                "source": {
                    "provider": "github",
                    "owner": "stenzek",
                    "repo": "duckstation",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name": "duckstation-windows-x64-release.zip",
                        }
                    ],
                },
            }
        ]

        rows = module.source_download_emulator_entries(autoprofiles)

        self.assertEqual(len(rows), 1)
        self.assertIn("source_metadata", rows[0])
        self.assertEqual(
            rows[0]["source_metadata"],
            {
                "provider": "github",
                "owner": "stenzek",
                "repo": "duckstation",
                "windows_assets": [
                    {
                        "arch": "x64",
                        "asset_name": "duckstation-windows-x64-release.zip",
                    }
                ],
            },
        )

    def test_filter_source_download_emulator_entries_supports_query_and_installed_name_filter(self) -> None:
        module = self._load_emulators_module()
        source_rows = [
            {
                "name": "DuckStation (Playstation 1)",
                "provider": "github",
                "owner": "stenzek",
                "repo": "duckstation",
                "release_tag": "latest",
                "source_id": "stenzek/duckstation",
            },
            {
                "name": "PPSSPP (Playstation Portable)",
                "provider": "github",
                "owner": "hrydgard",
                "repo": "ppsspp",
                "release_tag": "v1.19.0",
                "source_id": "hrydgard/ppsspp",
            },
        ]

        filtered = module.filter_source_download_emulator_entries(
            source_rows,
            query="github ppsspp",
            installed_emulator_names=["DuckStation (Playstation 1)"],
        )

        self.assertEqual(len(filtered), 1)
        self.assertEqual(filtered[0]["name"], "PPSSPP (Playstation Portable)")

    def test_available_source_download_emulator_entries_combines_listing_and_filtering(self) -> None:
        module = self._load_emulators_module()
        autoprofiles = [
            {
                "name": "DuckStation (Playstation 1)",
                "source": {
                    "provider": "github-release",
                    "owner": "stenzek",
                    "repo": "duckstation",
                },
            },
            {
                "name": "RPCS3 (Playstation 3)",
            },
        ]

        rows = module.available_source_download_emulator_entries(
            autoprofiles,
            query="duckstation",
            installed_emulator_names=[],
        )

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["source_id"], "stenzek/duckstation")

    def test_available_source_download_emulator_entries_excludes_renamed_installed_source_by_source_id(self) -> None:
        module = self._load_emulators_module()
        autoprofiles = [
            {
                "name": "Dolphin",
                "source": {
                    "provider": "direct",
                    "owner": "dolphin-emu",
                    "repo": "dolphin",
                },
            }
        ]

        rows = module.available_source_download_emulator_entries(
            autoprofiles,
            query="dolphin",
            installed_emulator_names=["Dolphin (GameCube + Wii)"],
            installed_source_ids=["dolphin-emu/dolphin"],
        )

        self.assertEqual(rows, [])

    def test_source_download_entries_excludes_win32_only_on_linux(self) -> None:
        module = self._load_emulators_module()
        autoprofiles = [
            {
                "name": "DuckStation (Playstation 1)",
                "source": {
                    "platforms": ["win32"],
                    "provider": "github",
                    "owner": "stenzek",
                    "repo": "duckstation",
                },
            },
            {
                "name": "RetroArch (Multi-System)",
                "source": {
                    "provider": "github",
                    "owner": "libretro",
                    "repo": "RetroArch",
                },
            },
        ]

        rows = module.source_download_emulator_entries(autoprofiles, current_platform="linux")

        self.assertEqual([row["name"] for row in rows], ["RetroArch (Multi-System)"])

    def test_source_download_entries_includes_win32_only_on_win32(self) -> None:
        module = self._load_emulators_module()
        autoprofiles = [
            {
                "name": "DuckStation (Playstation 1)",
                "source": {
                    "platforms": ["win32"],
                    "provider": "github",
                    "owner": "stenzek",
                    "repo": "duckstation",
                },
            },
            {
                "name": "RetroArch (Multi-System)",
                "source": {
                    "provider": "github",
                    "owner": "libretro",
                    "repo": "RetroArch",
                },
            },
        ]

        rows = module.source_download_emulator_entries(autoprofiles, current_platform="win32")

        self.assertEqual(
            [row["name"] for row in rows],
            ["DuckStation (Playstation 1)", "RetroArch (Multi-System)"],
        )

    def test_source_download_entries_no_platforms_field_always_included(self) -> None:
        module = self._load_emulators_module()
        autoprofiles = [
            {
                "name": "RetroArch (Multi-System)",
                "source": {
                    "provider": "github",
                    "owner": "libretro",
                    "repo": "RetroArch",
                },
            },
        ]

        rows = module.source_download_emulator_entries(autoprofiles, current_platform="linux")

        self.assertEqual([row["name"] for row in rows], ["RetroArch (Multi-System)"])


class EmulatorsPageLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_emulator_layout_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_build_emulators_page_wraps_content_in_scroll_area(self) -> None:
        module = self._load_main_module()
        window = _EmulatorsPageStubWindow()

        page = module.MainWindow._build_emulators_page(window)

        scroll_areas = page.findChildren(QScrollArea)
        self.assertEqual(len(scroll_areas), 1)
        self.assertTrue(scroll_areas[0].widgetResizable())
        self.assertEqual(scroll_areas[0].objectName(), "emulatorsScroll")

    def test_build_emulators_page_starts_with_add_new_save_button_label(self) -> None:
        module = self._load_main_module()
        window = _EmulatorsPageStubWindow()
        window.config["emulators"] = [{"name": "DuckStation", "path": "C:/Emulators/duckstation.exe"}]

        page = module.MainWindow._build_emulators_page(window)

        self.assertIsNotNone(page)
        self.assertIsNotNone(window.save_emulator_button)
        assert window.save_emulator_button is not None
        self.assertEqual(window.save_emulator_button.text(), "Add New")

    def test_build_emulators_page_exposes_add_new_emulator_button(self) -> None:
        module = self._load_main_module()
        window = _EmulatorsPageStubWindow()

        page = module.MainWindow._build_emulators_page(window)

        self.assertIsNotNone(page)
        add_buttons = [
            button
            for button in page.findChildren(QPushButton)
            if button.text().strip() == "Add New Emulator"
        ]
        self.assertEqual(len(add_buttons), 1)

    def test_build_emulators_page_exposes_source_download_button(self) -> None:
        module = self._load_main_module()
        window = _EmulatorsPageStubWindow()

        page = module.MainWindow._build_emulators_page(window)

        self.assertIsNotNone(page)
        source_buttons = [
            button
            for button in page.findChildren(QPushButton)
            if button.text().strip() == "Download Supported Emulator"
        ]
        self.assertEqual(len(source_buttons), 1)

    def test_open_source_emulator_download_dialog_builds_archive_name_without_crashing(self) -> None:
        module = self._load_main_module()
        window = _SourceDownloadDialogStubWindow()

        with patch.object(module.QInputDialog, "getItem", return_value=("[GitHub] DuckStation - stenzek/duckstation (latest)", True)):
            module.MainWindow._open_source_emulator_download_dialog(window)

        self.assertEqual(
            window._sanitizer_calls,
            [
                ("DuckStation", "emulator"),
                ("latest", "latest"),
            ],
        )
        self.assertIsNotNone(window.install_game)
        assert window.install_game is not None
        self.assertEqual(window.install_game.get("_archive_name_override"), "DuckStation-latest.zip")
        source_metadata = window.install_game.get("_source_metadata")
        self.assertIsInstance(source_metadata, dict)
        assert isinstance(source_metadata, dict)
        self.assertIn("windows_assets", source_metadata)
        self.assertEqual(
            source_metadata["windows_assets"],
            [
                {
                    "arch": "x64",
                    "asset_name": "duckstation-windows-x64-release.zip",
                }
            ],
        )

    def test_build_emulators_page_does_not_show_inline_emulator_details_panel(self) -> None:
        module = self._load_main_module()
        window = _EmulatorsPageStubWindow()

        page = module.MainWindow._build_emulators_page(window)

        self.assertIsNotNone(page)
        self.assertIsNone(page.findChild(QLabel, "Emulator Details"))
        self.assertNotIn(
            "Emulator Details",
            [label.text().strip() for label in page.findChildren(QLabel)],
        )

    def test_build_emulators_page_uses_non_selectable_installed_emulator_list(self) -> None:
        module = self._load_main_module()
        window = _EmulatorsPageStubWindow()

        page = module.MainWindow._build_emulators_page(window)

        self.assertIsNotNone(page)
        self.assertIsNotNone(window.emulator_list)
        assert window.emulator_list is not None
        self.assertEqual(window.emulator_list.selectionMode(), QAbstractItemView.SelectionMode.NoSelection)

    def test_build_emulators_page_enables_alternating_row_colors_for_installed_emulator_list(self) -> None:
        module = self._load_main_module()
        window = _EmulatorsPageStubWindow()

        page = module.MainWindow._build_emulators_page(window)

        self.assertIsNotNone(page)
        self.assertIsNotNone(window.emulator_list)
        assert window.emulator_list is not None
        self.assertTrue(window.emulator_list.alternatingRowColors())

    def test_refresh_emulator_views_uses_plain_name_label_and_icon_only_row_actions(self) -> None:
        module = self._load_main_module()
        window = _EmulatorRowsStubWindow()
        window.emulator_list = module.QListWidget()

        module.MainWindow._refresh_emulator_views(window)

        self.assertEqual(window.emulator_list.count(), 1)
        item = window.emulator_list.item(0)
        row_widget = window.emulator_list.itemWidget(item)
        self.assertIsNotNone(row_widget)
        assert row_widget is not None
        self.assertIsNotNone(row_widget.layout())

        name_labels = [label.text().strip() for label in row_widget.findChildren(QLabel)]
        action_buttons: list[QPushButton] = list(row_widget.findChildren(QPushButton))

        self.assertEqual(name_labels, [
            "DuckStation",
            "RetroAchievements: Configure login via Emulator Settings \u2192 Achievements (tokens are machine-encrypted)"
        ])
        self.assertEqual(len(action_buttons), 3)
        self.assertTrue(all(not button.text().strip() for button in action_buttons))

        action_order: list[str] = []
        for button in action_buttons:
            semantic_text = " ".join(
                [
                    button.objectName(),
                    button.toolTip(),
                    button.statusTip(),
                    button.whatsThis(),
                    button.accessibleName(),
                    button.accessibleDescription(),
                ]
            ).casefold()
            if "launch" in semantic_text:
                action_order.append("launch")
            elif "config" in semantic_text:
                action_order.append("config")
            elif "uninstall" in semantic_text or "remove" in semantic_text:
                action_order.append("uninstall")

        self.assertEqual(action_order, ["launch", "config", "uninstall"])

    def test_refresh_emulator_views_shows_source_update_action_for_source_capable_installed_emulator(self) -> None:
        module = self._load_main_module()
        window = _EmulatorRowsStubWindow(source_available=True)
        window.emulator_list = module.QListWidget()

        module.MainWindow._refresh_emulator_views(window)

        self.assertEqual(window.emulator_list.count(), 1)
        item = window.emulator_list.item(0)
        row_widget = window.emulator_list.itemWidget(item)
        self.assertIsNotNone(row_widget)
        assert row_widget is not None
        action_buttons = row_widget.findChildren(QPushButton)
        self.assertEqual(len(action_buttons), 4)

        source_buttons = [
            button
            for button in action_buttons
            if button.objectName() == "installedEmulatorSourceUpdateButton"
        ]
        self.assertEqual(len(source_buttons), 1)
        self.assertEqual(source_buttons[0].toolTip(), "Update from Source")

    def test_start_source_emulator_update_at_index_starts_source_update_install_mode(self) -> None:
        module = self._load_main_module()
        window = _SourceUpdateActionStubWindow()

        module.MainWindow._do_start_source_emulator_update_at_index(window, 0)

        self.assertIsNotNone(window.install_game)
        assert window.install_game is not None
        self.assertEqual(window.install_game.get("_install_mode"), "source_emulator_update")
        self.assertEqual(window.install_game.get("_source_id"), "stenzek/duckstation")

    def test_source_download_entry_lookup_for_renamed_emulator_uses_stable_source_identity(self) -> None:
        module = self._load_main_module()
        window = _RenamedSourceLookupStubWindow()
        emulator = {
            "name": "Dolphin (GameCube + Wii)",
            "path": "C:/Emulators/Dolphin/dolphin.exe",
        }

        source_entry = module.MainWindow._source_download_entry_for_emulator_name(
            window,
            emulator["name"],
            emulator,
        )

        self.assertIsNotNone(source_entry)
        assert source_entry is not None
        self.assertEqual(source_entry.get("source_id"), "dolphin-emu/dolphin")
        self.assertEqual(source_entry.get("name"), "Dolphin (GameCube + Wii)")

    def test_apply_source_supplemental_archives_preserves_main_emulator_files(self) -> None:
        module = self._load_main_module()
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            install_dir = root / "RetroArch"
            install_dir.mkdir()
            (install_dir / "retroarch.exe").write_text("main-exe", encoding="utf-8")
            (install_dir / "retroarch.cfg").write_text("config", encoding="utf-8")

            archive_path = root / "retroarch.7z"
            archive_path.write_text("placeholder", encoding="utf-8")
            supplemental_path = root / "retroarch-supplemental-1.zip"
            with zipfile.ZipFile(supplemental_path, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
                archive.writestr("cores/flycast_libretro.dll", "core-data")

            game = {
                "_source_metadata": {
                    "supplemental_downloads": [
                        {
                            "asset_name": "RetroArch_cores.zip",
                        }
                    ]
                }
            }
            installed_game = {
                "extracted_dir": str(install_dir),
                "extracted_path": str(install_dir / "retroarch.exe"),
            }

            module.MainWindow._apply_source_supplemental_archives_without_ui(
                object(),
                game,
                archive_path,
                installed_game,
            )

            self.assertTrue((install_dir / "retroarch.exe").exists())
            self.assertTrue((install_dir / "retroarch.cfg").exists())
            self.assertTrue((install_dir / "cores" / "flycast_libretro.dll").exists())

    def test_build_emulators_page_locks_text_inputs_to_readable_height(self) -> None:
        module = self._load_main_module()
        window = _EmulatorsPageStubWindow()

        page = module.MainWindow._build_emulators_page(window)

        widgets = [
            window.emulator_name_input,
            window.emulator_path_input,
            window.emulator_args_input,
            window.emulator_save_strategy_input,
            window.emulator_ignore_files_input,
            window.emulator_ignore_extensions_input,
            window.emulator_save_paths_input,
            window.emulator_state_paths_input,
            window.default_platform_combo,
            window.default_emulator_combo,
            window.default_core_combo,
        ]
        self.assertIsNotNone(page)
        self.assertTrue(all(isinstance(widget, (QLineEdit, QComboBox)) for widget in widgets))
        for widget in widgets:
            assert widget is not None
            self.assertGreaterEqual(widget.minimumHeight(), 32)
            self.assertEqual(widget.minimumHeight(), widget.maximumHeight())


class _CoreDropdownStubWindow:
    def __init__(self) -> None:
        self.config = {"default_retroarch_cores": {"PlayStation 2": "pcsx2"}}
        self.default_platform_combo = QComboBox()
        self.default_platform_combo.addItem("PlayStation 2")
        self.default_platform_combo.setCurrentText("PlayStation 2")
        self.default_emulator_combo = QComboBox()
        self.default_emulator_combo.addItem("PCSX2")
        self.default_emulator_combo.setCurrentText("PCSX2")
        self.default_core_combo = QComboBox()

    def _is_retroarch_emulator_name(self, emulator_name: str) -> bool:
        return emulator_name.strip().casefold() == "retroarch"

    def _normalize_default_retroarch_cores(self, value: object) -> dict[str, str]:
        if isinstance(value, dict):
            return {str(key): str(item) for key, item in value.items()}
        return {}

    def _installed_retroarch_cores_for_platform(self, platform: str, emulator_name: str) -> list[str]:
        return ["pcsx2"]


class CoreDropdownBehaviorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_core_dropdown_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_refresh_retroarch_core_options_shows_na_placeholder_for_non_retroarch_selection(self) -> None:
        module = self._load_main_module()
        window = _CoreDropdownStubWindow()

        module.MainWindow._refresh_retroarch_core_options(window)

        self.assertFalse(window.default_core_combo.isEnabled())
        self.assertEqual(window.default_core_combo.count(), 1)
        self.assertEqual(window.default_core_combo.itemText(0), "N/A")


class GameDetailsPageLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_details_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_build_game_details_page_wraps_metadata_section_in_scroll_area(self) -> None:
        module = self._load_main_module()
        window = _DetailsPageStubWindow()

        page = module.MainWindow._build_game_details_page(window)

        self.assertIsNotNone(page)
        overview_scroll = page.findChild(QScrollArea, "detailsOverviewScroll")
        self.assertIsNotNone(overview_scroll)
        assert overview_scroll is not None
        self.assertTrue(overview_scroll.widgetResizable())
        self.assertEqual(overview_scroll.verticalScrollBarPolicy(), Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        self.assertEqual(overview_scroll.horizontalScrollBarPolicy(), Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self.assertEqual(overview_scroll.widget().objectName(), "detailsOverviewContent")
        self.assertIsNotNone(window.details_description_label)
        assert window.details_description_label is not None
        self.assertTrue(overview_scroll.widget().isAncestorOf(window.details_description_label))
        self.assertEqual(window.overview_calls, 1)
        self.assertEqual(window.layout_calls, 1)


class _OpenDetailsStackStub:
    # Mirrors the real MainWindow.stack, which has 7 pages (library, server,
    # discover, downloads, emulators, settings, game_details) since the
    # Discover tab was added, so `count() - 1` resolves to the details page.
    def __init__(self) -> None:
        self.index: int | None = None
        self.widget_count = 7

    def count(self) -> int:
        return self.widget_count

    def setCurrentIndex(self, value: int) -> None:
        self.index = value


class _OpenDetailsButtonStub:
    def __init__(self) -> None:
        self.checked = True

    def setChecked(self, value: bool) -> None:
        self.checked = value


class _OpenDetailsWindowStub:
    def __init__(self, game: dict[str, str]) -> None:
        self.current_details_game = None
        self.current_details_source = ""
        self.current_details_cloud_mode = "overview"
        self.details_title_label = QLabel()
        self.details_cover_label = QLabel()
        self.details_platform_label = QLabel()
        self.details_genres_label = QLabel()
        self.details_regions_label = QLabel()
        self.details_filesize_label = QLabel()
        self.details_version_label = QLabel()
        self.details_rating_label = QLabel()
        self.details_description_label = QLabel()
        self.details_primary_button = None
        self.details_config_button = None
        self.details_details_button = None
        self.details_manage_saves_button = None
        self.details_manage_states_button = None
        self.details_ps4_content_button = None
        self.details_secondary_button = None
        self.details_update_button = None
        self.details_companies_group = None
        self.details_companies_label = None
        self.details_release_date_group = None
        self.details_release_date_label = None
        self.details_languages_group = None
        self.details_languages_label = None
        self.stack = _OpenDetailsStackStub()
        self.nav_buttons = [_OpenDetailsButtonStub(), _OpenDetailsButtonStub()]
        self.install_in_progress = False
        self.install_finalize_in_progress = False
        self.install_pending_game = None
        self.install_finalize_game = None
        self._game = game
        self.screenshot_updates = 0
        self.action_updates = 0
        self.layout_updates = 0

    def _queue_game_cover_load(self, game: dict[str, str], label: QLabel) -> None:
        return None

    def _update_details_screenshots(self, game: dict[str, str]) -> None:
        self.screenshot_updates += 1

    def _update_details_action_buttons(self) -> None:
        self.action_updates += 1

    def _update_details_layout_metrics(self) -> None:
        self.layout_updates += 1

    def _show_details_overview(self) -> None:
        self.current_details_cloud_mode = "overview"

    def _format_size(self, size_bytes: float) -> str:
        if size_bytes >= 1024.0:
            return f"{size_bytes / 1024.0:.1f} KB"
        return f"{int(size_bytes)} B"

    def _is_emulators_platform(self, game: dict[str, str]) -> bool:
        return False

    def _is_game_installed(self, game: dict[str, str]) -> bool:
        return True

    def _install_block_reason_for_game(self, game: dict[str, str]) -> str:
        return ""

    def _is_game_install_queued(self, game: dict[str, str]) -> bool:
        return False

    def _game_key(self, game: dict[str, str]) -> tuple[str, str]:
        return (game.get("title", ""), game.get("platform", ""))

    def _is_native_executable_platform(self, game: dict[str, str]) -> bool:
        return False

    def _details_ps4_content_button_text(self, game: dict[str, str]) -> str:
        return ""

    def _ps4_content_install_block_reason(self, game: dict[str, str]) -> str:
        return ""

    def _resolved_emulator_entry_for_game(self, game: dict[str, str]) -> tuple[str, dict[str, str] | None]:
        return ("", None)

    def _details_cloud_mode_supported(self, game: dict[str, str], save_type: str) -> bool:
        return False

    def _details_cloud_button_text(self, game: dict[str, str], save_type: str) -> str:
        return ""

    def _is_rpcs3_emulator_name(self, emulator_name: str) -> bool:
        return False


class GameDetailsVersionLabelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def test_open_game_details_shows_windows_version_label_when_rom_file_tag_exists(self) -> None:
        game = {
            "title": "Native Game",
            "platform": "Windows",
            "rating": "4/5",
            "description": "Native title.",
            "rom_file_name": "native-game (v01234).zip",
        }
        window = _OpenDetailsWindowStub(game)

        open_game_details(window, game, "library")

        self.assertEqual(window.details_version_label.text(), "Version: v01234")
        self.assertTrue(window.details_version_label.isVisible())

    def test_open_game_details_hides_version_label_for_non_windows_platform(self) -> None:
        game = {
            "title": "Console Game",
            "platform": "PlayStation 2",
            "rating": "4/5",
            "description": "Console title.",
            "rom_file_name": "console-game (v01234).zip",
        }
        window = _OpenDetailsWindowStub(game)

        open_game_details(window, game, "library")

        self.assertEqual(window.details_version_label.text(), "")
        self.assertFalse(window.details_version_label.isVisible())

    def test_open_game_details_shows_new_metadata_rows_when_values_present(self) -> None:
        game = {
            "title": "Console Game",
            "platform": "PlayStation 2",
            "rating": "4.2/5",
            "description": "Console title.",
            "genres": "Action, Adventure",
            "regions": "USA, Europe",
            "filesize_bytes": "2048",
            "rom_file_name": "console-game (v01234).zip",
        }
        window = _OpenDetailsWindowStub(game)

        open_game_details(window, game, "library")

        self.assertTrue(window.details_platform_label.isVisible())
        self.assertEqual(window.details_genres_label.text(), "Genres: Action, Adventure")
        self.assertTrue(window.details_genres_label.isVisible())
        self.assertEqual(window.details_regions_label.text(), "Regions: USA, Europe")
        self.assertTrue(window.details_regions_label.isVisible())
        self.assertEqual(window.details_filesize_label.text(), "Filesize: 2.0 KB")
        self.assertTrue(window.details_filesize_label.isVisible())
        self.assertEqual(window.details_rating_label.text(), "Rating: 4.2/5")
        self.assertTrue(window.details_rating_label.isVisible())

    def test_open_game_details_hides_new_metadata_rows_when_values_missing(self) -> None:
        game = {
            "title": "Console Game",
            "platform": "",
            "rating": "",
            "description": "",
            "genres": "",
            "regions": "",
            "filesize_bytes": "",
            "rom_file_name": "console-game (v01234).zip",
        }
        window = _OpenDetailsWindowStub(game)

        open_game_details(window, game, "library")

        self.assertFalse(window.details_platform_label.isVisible())
        self.assertFalse(window.details_genres_label.isVisible())
        self.assertFalse(window.details_regions_label.isVisible())
        self.assertFalse(window.details_filesize_label.isVisible())
        self.assertFalse(window.details_rating_label.isVisible())
        self.assertFalse(window.details_description_label.isVisible())


class _MainWindowVersionTextStub:
    def __init__(
        self,
        module,
        *,
        installed_game: dict[str, str] | None = None,
        server_game: dict[str, str] | None = None,
    ) -> None:
        self._module = module
        self._installed_game = installed_game
        self._server_game = server_game

    def _is_windows_pc_platform(self, game: dict[str, str]) -> bool:
        return self._module.MainWindow._is_windows_pc_platform(self, game)

    def _is_emulators_platform(self, game: dict[str, str]) -> bool:
        return False

    def _installed_game_record(self, game: dict[str, str]) -> dict[str, str] | None:
        return self._installed_game

    def _server_game_for_identity(self, game: dict[str, str], rom_id: str) -> dict[str, str] | None:
        return self._server_game

    def _format_version_tag_for_ui(self, version_tag: object) -> str:
        return self._module.MainWindow._format_version_tag_for_ui(self, version_tag)


class MainWindowVersionFormattingTests(unittest.TestCase):
    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_version_format_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_details_version_label_text_supports_semver_tags(self) -> None:
        module = self._load_main_module()
        game = {
            "title": "A Little to the Left",
            "platform": "Windows",
            "rom_file_name": "A Little to the Left (v3.6.0) (2022) (W_P).7z",
        }
        window = _MainWindowVersionTextStub(module)

        label_text = module.MainWindow._details_version_label_text_for_game(window, game)

        self.assertEqual(label_text, "Version: v3.6.0")

    def test_details_update_button_text_supports_semver_tags(self) -> None:
        module = self._load_main_module()
        installed_game = {
            "title": "A Little to the Left",
            "platform": "Windows",
            "rom_id": "1",
            "rom_file_name": "A Little to the Left (v3.6.0) (2022) (W_P).7z",
        }
        server_game = {
            "title": "A Little to the Left",
            "platform": "Windows",
            "rom_id": "1",
            "rom_file_name": "A Little to the Left (v3.7.0) (2022) (W_P).7z",
        }
        window = _MainWindowVersionTextStub(module, installed_game=installed_game, server_game=server_game)

        button_text = module.MainWindow._details_update_button_text_for_game(window, installed_game)

        self.assertEqual(button_text, "Update to v3.7.0")

    def test_details_update_button_text_preserves_numeric_zero_padding(self) -> None:
        module = self._load_main_module()
        installed_game = {
            "title": "Native Game",
            "platform": "Windows",
            "rom_id": "1",
            "rom_file_name": "native-game (v01234).zip",
        }
        server_game = {
            "title": "Native Game",
            "platform": "Windows",
            "rom_id": "1",
            "rom_file_name": "native-game (v01235).zip",
        }
        window = _MainWindowVersionTextStub(module, installed_game=installed_game, server_game=server_game)

        button_text = module.MainWindow._details_update_button_text_for_game(window, installed_game)

        self.assertEqual(button_text, "Update to v01235")


class TestBuildAchievementsPanel(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def test_empty_achievements_shows_empty_label(self):
        from grid_launcher.ui.game_views import build_achievements_panel

        panel = build_achievements_panel([])
        labels = panel.findChildren(QLabel)
        object_names = [label.objectName() for label in labels]
        self.assertIn("achievementsEmptyLabel", object_names)

    def test_single_achievement_renders_row(self):
        from grid_launcher.ui.game_views import build_achievements_panel

        achievement = {
            "id": 1,
            "title": "First Blood",
            "description": "Kill an enemy",
            "points": 5,
            "badge_name": "12345",
            "date_earned": "",
        }
        panel = build_achievements_panel([achievement], load_image_fn=lambda url, lbl: None)
        rows = panel.findChildren(QFrame, "achievementRow")
        self.assertGreaterEqual(len(rows), 1)

    def test_earned_achievement_shows_checkmark(self):
        from grid_launcher.ui.game_views import build_achievements_panel

        achievement = {
            "id": 2,
            "title": "Done",
            "description": "Complete",
            "points": 10,
            "badge_name": "99999",
            "date_earned": "2024-01-01",
        }
        panel = build_achievements_panel([achievement], load_image_fn=lambda url, lbl: None)
        earned_labels = panel.findChildren(QLabel, "achievementEarned")
        self.assertGreaterEqual(len(earned_labels), 1)
        self.assertEqual(earned_labels[0].text(), "✓")

    def test_earned_achievement_shows_date(self):
        from grid_launcher.ui.game_views import build_achievements_panel

        ach = {
            "id": 2,
            "title": "Done",
            "description": "Complete",
            "points": 10,
            "badge_name": "b",
            "date_earned": "2024-01-01 12:00:00",
        }
        panel = build_achievements_panel([ach], load_image_fn=lambda u, l: None)
        date_labels = panel.findChildren(QLabel, "achievementDate")
        self.assertGreaterEqual(len(date_labels), 1)
        self.assertTrue(date_labels[0].text().startswith("Unlocked:"))
        self.assertGreater(len(date_labels[0].text()), len("Unlocked: "))

    def test_locked_achievement_no_date(self):
        from grid_launcher.ui.game_views import build_achievements_panel

        ach = {
            "id": 3,
            "title": "Locked",
            "description": "Not done",
            "points": 5,
            "badge_name": "c",
            "date_earned": "",
        }
        panel = build_achievements_panel([ach], load_image_fn=lambda u, l: None)
        date_labels = panel.findChildren(QLabel, "achievementDate")
        self.assertEqual(len(date_labels), 0)

    def test_badge_image_label_created_per_achievement(self):
        from grid_launcher.ui.game_views import build_achievements_panel

        calls = []

        def fake_loader(url, lbl):
            calls.append((url, lbl))

        ach = {
            "id": 1,
            "title": "T",
            "description": "D",
            "points": 5,
            "badge_name": "54321",
            "date_earned": "",
        }

        build_achievements_panel([ach], load_image_fn=fake_loader)
        self.assertEqual(len(calls), 3)
        self.assertIn("54321_lock", calls[0][0])


class SettingsPageLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_settings_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_build_settings_page_wraps_content_in_scroll_area(self) -> None:
        module = self._load_main_module()
        window = _SettingsPageStubWindow()

        page = module.MainWindow._build_settings_page(window)

        scroll_areas = page.findChildren(QScrollArea)
        self.assertEqual(len(scroll_areas), 1)
        self.assertTrue(scroll_areas[0].widgetResizable())
        self.assertEqual(scroll_areas[0].objectName(), "settingsScroll")

    def test_build_settings_page_locks_text_inputs_to_readable_height(self) -> None:
        module = self._load_main_module()
        window = _SettingsPageStubWindow()

        page = module.MainWindow._build_settings_page(window)

        widgets = [
            window.server_url_input,
            window.api_token_input,
            window.library_path_input,
            window.theme_input,
        ]
        self.assertIsNotNone(page)
        self.assertTrue(all(isinstance(widget, (QLineEdit, QComboBox)) for widget in widgets))
        for widget in widgets:
            assert widget is not None
            self.assertGreaterEqual(widget.minimumHeight(), 32)
            self.assertEqual(widget.minimumHeight(), widget.maximumHeight())


class _MixinSourceDownloadStub:
    def __init__(self, autoprofiles: list[dict]) -> None:
        self._autoprofiles = autoprofiles
        self.config = {"emulator_autoprofiles": autoprofiles}

    def _emulators(self):
        return []

    def _normalize_emulators(self, emulators):
        return list(emulators)

    def _emulator_autoprofiles(self):
        return self._autoprofiles

    def _emulator_profile_for_entry(self, emulator):
        return {}


class SourceEntryLabelTests(unittest.TestCase):
    @staticmethod
    def _load_emulators_module():
        module_path = Path(__file__).resolve().parents[1] / "grid_launcher" / "ui" / "emulators.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_ui_emulators_source_label_for_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_appimage_entry_returns_appimage(self) -> None:
        module = self._load_emulators_module()
        entry = {"source_metadata": {"asset_patterns": ["SomeEmulator-x86_64.AppImage"]}}

        self.assertEqual(module.source_entry_label(entry), "[AppImage]")

    def test_appimage_in_platform_override_returns_appimage(self) -> None:
        module = self._load_emulators_module()
        entry = {"source_metadata": {"platform_overrides": {"linux": {"asset_patterns": ["app.AppImage"]}}}}

        self.assertEqual(module.source_entry_label(entry), "[AppImage]")

    def test_github_release_returns_github(self) -> None:
        module = self._load_emulators_module()
        entry = {"provider": "github-release", "source_metadata": {"asset_patterns": ["app.zip"]}}

        self.assertEqual(module.source_entry_label(entry), "[GitHub]")

    def test_gitea_without_appimage_returns_github(self) -> None:
        module = self._load_emulators_module()
        entry = {"provider": "gitea", "source_metadata": {"asset_patterns": ["app.zip"]}}

        self.assertEqual(module.source_entry_label(entry), "[GitHub]")


if __name__ == "__main__":
    unittest.main()
