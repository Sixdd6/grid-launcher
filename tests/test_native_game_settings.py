import os
import sys
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication

from grid_launcher.ui.dialogs import NativeGameSettingsDialog


def _make_dialog(available_compat_tools, existing_compat_tool=""):
    return NativeGameSettingsDialog(
        None,
        game_title="Test Game",
        install_dir=Path("/games/test"),
        executable_candidates=[Path("/games/test/game.exe")],
        available_compat_tools=available_compat_tools,
        existing_compat_tool=existing_compat_tool,
    )


@unittest.skipIf(sys.platform == "win32", "Linux-only dialog fields")
class NativeGameSettingsDialogTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def test_dialog_no_compat_tools_returns_empty(self):
        dialog = _make_dialog([])
        self.assertEqual(dialog.selected_compat_tool_path(), "")

    def test_dialog_with_wine_entry(self):
        dialog = _make_dialog(
            [
                {"name": "None", "type": "", "path": ""},
                {"name": "Wine (system)", "type": "wine", "path": "wine"},
            ]
        )
        dialog.compat_tool_combo.setCurrentIndex(1)
        self.assertEqual(dialog.selected_compat_tool_path(), "wine")

    def test_dialog_preselects_existing_compat_tool(self):
        dialog = _make_dialog(
            [
                {"name": "None", "type": "", "path": ""},
                {"name": "GE-Proton", "type": "proton", "path": "/path/to/GE-Proton"},
            ],
            existing_compat_tool="/path/to/GE-Proton",
        )
        self.assertEqual(dialog.selected_compat_tool_path(), "/path/to/GE-Proton")

    def test_dialog_none_selection_returns_empty(self):
        dialog = _make_dialog(
            [
                {"name": "None", "type": "", "path": ""},
                {"name": "Wine (system)", "type": "wine", "path": "wine"},
            ]
        )
        dialog.compat_tool_combo.setCurrentIndex(0)
        self.assertEqual(dialog.selected_compat_tool_path(), "")


if __name__ == "__main__":
    unittest.main()
