from __future__ import annotations

import os
import unittest
from unittest.mock import patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication, QFileDialog, QLabel, QListWidget, QPushButton

from rom_mate.ui.dialogs import EmulatorConfigDialog
from rom_mate.ui.mixins.emulator_ui_mixin import EmulatorUIMixin


class _EmulatorRowsStubWindow(EmulatorUIMixin):
    def __init__(self) -> None:
        self.config = {
            "emulators": [
                {
                    "name": "Auto Detected",
                    "path": "/usr/bin/retroarch",
                    "args": "%rom%",
                    "save_strategy": "auto",
                    "autodetected": "true",
                },
                {
                    "name": "Manual Entry",
                    "path": "/opt/emulators/manual",
                    "args": "%rom%",
                    "save_strategy": "auto",
                },
            ],
            "default_emulators": {},
            "default_retroarch_cores": {},
        }
        self.emulator_list = QListWidget()
        self.default_platform_combo = None
        self.default_mapping_list = None
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

    def _is_azahar_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        del emulator
        return emulator_name.strip().casefold() == "azahar"

    def _is_eden_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        del emulator
        return emulator_name.strip().casefold() == "eden"

    def _is_xemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        del emulator
        return emulator_name.strip().casefold() == "xemu"

    def _is_rpcs3_emulator_name(self, emulator_name: str) -> bool:
        return emulator_name.strip().casefold() == "rpcs3"

    def _on_default_platform_changed(self, platform: str) -> None:
        return None

    def _warm_emulator_platform_caches(self) -> None:
        return None

    def _source_download_entry_for_emulator_name(
        self,
        emulator_name: str,
        emulator: dict[str, str] | None = None,
    ) -> dict[str, str] | None:
        del emulator_name, emulator
        return None

    def _launch_emulator_at_index(self, row: int) -> None:
        return None

    def _open_emulator_config_dialog_for_row(self, row: int) -> None:
        return None

    def _remove_emulator_at_index(self, row: int) -> None:
        return None

    def _start_source_emulator_update_at_index(self, index: int) -> None:
        return None


class EmulatorConfigDialogAddNewTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def _new_dialog(self) -> EmulatorConfigDialog:
        return EmulatorConfigDialog(None, is_new_entry=True)

    def _edit_dialog(self) -> EmulatorConfigDialog:
        return EmulatorConfigDialog(
            None,
            emulator={
                "name": "Existing Emulator",
                "path": r"C:\Emulators\Existing\existing.exe",
                "args": "%rom%",
                "save_strategy": "auto",
            },
            is_new_entry=False,
        )

    def test_add_new_dialog_hides_archive_path_field(self) -> None:
        dialog = self._new_dialog()

        label_texts = [
            label.text().strip()
            for label in dialog.findChildren(QLabel)
            if label.text().strip()
        ]
        self.assertNotIn("Archive Path", label_texts)

    def test_add_new_dialog_name_field_has_predictive_suggestions(self) -> None:
        dialog = self._new_dialog()

        completer = dialog.emulator_name_input.completer()
        self.assertIsNotNone(completer, "Expected Name field to provide predictive suggestions")

        assert completer is not None
        completer.setCompletionPrefix("duck")
        completion_model = completer.completionModel()
        suggestions = [
            str(completion_model.index(row, 0).data()).strip()
            for row in range(completion_model.rowCount())
        ]
        self.assertIn("DuckStation (Playstation 1)", suggestions)

    def test_selecting_name_suggestion_autofills_related_fields(self) -> None:
        dialog = self._new_dialog()

        completer = dialog.emulator_name_input.completer()
        self.assertIsNotNone(completer, "Expected Name field completer to be wired")

        assert completer is not None
        suggestion = "DuckStation (Playstation 1)"
        completer.activated.emit(suggestion)

        self.assertEqual(dialog.emulator_name_input.text(), suggestion)
        self.assertEqual(dialog.emulator_args_input.text(), '-fullscreen -batch "%rom%"')
        self.assertEqual(dialog.emulator_save_strategy_input.currentText(), "single_file")
        self.assertIn("memcards", dialog.emulator_save_paths_input.text().casefold())

    def test_edit_dialog_name_field_does_not_enable_predictive_suggestions(self) -> None:
        dialog = self._edit_dialog()

        self.assertIsNone(dialog.emulator_name_input.completer())

    def test_single_browse_button_routes_executable_or_archive_to_matching_payload_fields(self) -> None:
        dialog = self._new_dialog()

        browse_buttons = [
            button for button in dialog.findChildren(QPushButton) if button.text().strip() == "Browse..."
        ]
        self.assertEqual(len(browse_buttons), 1, "Expected one unified browse button")

        label_texts = [
            label.text().strip()
            for label in dialog.findChildren(QLabel)
            if label.text().strip()
        ]
        self.assertNotIn("Archive Path", label_texts)

        assert browse_buttons
        browse_button = browse_buttons[0]

        cases = [
            (
                r"C:\Emulators\DuckStation\duckstation-qt-x64-ReleaseLTCG.exe",
                {"path": r"C:\Emulators\DuckStation\duckstation-qt-x64-ReleaseLTCG.exe", "archive_path": ""},
            ),
            (
                r"C:\Downloads\DuckStation.zip",
                {"path": "", "archive_path": r"C:\Downloads\DuckStation.zip"},
            ),
        ]

        for selected_path, expected in cases:
            with self.subTest(selected_path=selected_path):
                dialog.emulator_path_input.clear()
                dialog.emulator_archive_path_input.clear()
                with patch.object(
                    QFileDialog,
                    "getOpenFileName",
                    return_value=(selected_path, "All Files (*)"),
                ):
                    browse_button.click()
                payload = dialog.entry_payload()
                self.assertEqual(payload.get("path", ""), expected["path"])
                self.assertEqual(payload.get("archive_path", ""), expected["archive_path"])

    def test_selecting_archive_populates_visible_path_field(self) -> None:
        dialog = self._new_dialog()

        archive_path = "/some/emulator.zip"
        dialog._route_selected_path(archive_path)

        self.assertEqual(dialog.emulator_path_input.text(), archive_path)
        self.assertEqual(dialog.emulator_archive_path_input.text(), archive_path)
        self.assertEqual(dialog.entry_payload()["path"], "")


class EmulatorConfigDialogGuideButtonTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def test_guide_button_checkbox_unchecked_by_default_for_new_entry(self) -> None:
        dialog = EmulatorConfigDialog(None, is_new_entry=True)

        self.assertFalse(dialog.guide_button_excluded())

    def test_guide_button_checkbox_checked_when_excluded_flag_set(self) -> None:
        dialog = EmulatorConfigDialog(None, guide_button_excluded=True)

        self.assertTrue(dialog.guide_button_excluded())

    def test_guide_button_default_checkbox_is_enabled(self) -> None:
        dialog = EmulatorConfigDialog(
            None,
            guide_button_excluded=True,
            is_guide_button_default_locked=True,
        )

        self.assertTrue(dialog.guide_button_checkbox.isEnabled())

    def test_guide_button_default_checkbox_can_be_unchecked(self) -> None:
        dialog = EmulatorConfigDialog(
            None,
            guide_button_excluded=True,
            is_guide_button_default_locked=True,
        )

        dialog.guide_button_checkbox.setChecked(False)
        self.assertFalse(dialog.guide_button_excluded())

    def test_guide_button_default_tooltip_set(self) -> None:
        dialog = EmulatorConfigDialog(
            None,
            guide_button_excluded=True,
            is_guide_button_default_locked=True,
        )

        self.assertIn("by default", dialog.guide_button_checkbox.toolTip())

    def test_guide_button_checkbox_not_in_entry_payload(self) -> None:
        dialog = EmulatorConfigDialog(None, is_new_entry=True)

        self.assertNotIn("guide_button_excluded", dialog.entry_payload())


class EmulatorListAutodetectedTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def test_autodetected_emulator_uninstall_button_disabled(self) -> None:
        window = _EmulatorRowsStubWindow()

        window._refresh_emulator_views()

        autodetected_button = None
        manual_button = None
        for row in range(window.emulator_list.count()):
            item = window.emulator_list.item(row)
            row_widget = window.emulator_list.itemWidget(item)
            self.assertIsNotNone(row_widget)
            assert row_widget is not None

            labels = [label.text().strip() for label in row_widget.findChildren(QLabel) if label.text().strip()]
            uninstall_button = next(
                button
                for button in row_widget.findChildren(QPushButton)
                if button.objectName() == "installedEmulatorUninstallButton"
            )
            if "Auto Detected" in labels:
                autodetected_button = uninstall_button
            elif "Manual Entry" in labels:
                manual_button = uninstall_button

        self.assertIsNotNone(autodetected_button)
        self.assertIsNotNone(manual_button)
        assert autodetected_button is not None
        assert manual_button is not None
        self.assertFalse(autodetected_button.isEnabled())
        self.assertIn("cannot remove", autodetected_button.toolTip().casefold())
        self.assertTrue(manual_button.isEnabled())


if __name__ == "__main__":
    unittest.main()
