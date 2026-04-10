from __future__ import annotations

import importlib.util
import os
import unittest
from pathlib import Path
from unittest.mock import patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QApplication, QComboBox, QLabel, QLineEdit, QScrollArea

from rom_mate.ui.game_views import make_game_card, update_details_action_buttons
from rom_mate.ui.theme import apply_theme_inline_styles


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
    ) -> None:
        self.current_details_game = {"title": "Test Game", "platform": platform}
        self.current_details_cloud_mode = "overview"
        self.details_title_label = None
        self.details_cover_label = None
        self.details_platform_label = None
        self.details_rating_label = None
        self.details_description_label = None
        self.details_primary_button = _StubButton()
        self.details_config_button = _StubButton()
        self.details_details_button = _StubButton()
        self.details_manage_saves_button = _StubButton()
        self.details_manage_states_button = _StubButton()
        self.details_secondary_button = _StubButton()
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

    def _open_config_folder(self) -> None:
        return None


class _EmulatorsPageStubWindow:
    def __init__(self) -> None:
        self.emulator_list = None
        self.emulator_name_input = None
        self.emulator_path_input = None
        self.emulator_args_input = None
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


class _DetailsPageStubWindow:
    def __init__(self) -> None:
        self.details_content_frame = None
        self.details_center_stack = None
        self.details_cover_label = None
        self.details_title_label = None
        self.details_platform_label = None
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
        self.details_secondary_button = None
        self.details_screenshot_labels = []
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

    def _perform_game_secondary_action(self) -> None:
        return None

    def _perform_current_cloud_upload_action(self) -> None:
        return None

    def _theme_color(self, role: str, fallback: str) -> str:
        return fallback

    def _show_details_overview(self) -> None:
        self.overview_calls += 1

    def _update_details_layout_metrics(self) -> None:
        self.layout_calls += 1


class GameViewCloudToggleTests(unittest.TestCase):
    def test_toggle_details_cloud_mode_switches_views_before_loading_records(self) -> None:
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_tests", module_path)
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


class EmulatorsPageLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_emulator_layout_tests", module_path)
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


class GameDetailsPageLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_details_tests", module_path)
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


class SettingsPageLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_settings_tests", module_path)
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


if __name__ == "__main__":
    unittest.main()
