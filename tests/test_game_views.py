from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch

from rom_mate.ui.game_views import update_details_action_buttons


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


if __name__ == "__main__":
    unittest.main()
