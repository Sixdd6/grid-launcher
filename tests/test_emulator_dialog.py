from __future__ import annotations

import os
import unittest
from unittest.mock import patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication, QFileDialog, QLabel, QPushButton

from rom_mate.ui.dialogs import EmulatorConfigDialog


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


if __name__ == "__main__":
    unittest.main()
