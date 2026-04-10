from __future__ import annotations

import importlib.util
import os
import types
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication, QLabel, QPushButton, QVBoxLayout, QWidget


class _RestoreEnabledNativeStub:
    def __init__(self) -> None:
        self.current_details_game = {"title": "Native Game", "platform": "Windows"}

    def _installed_game_record(self, game: dict[str, str]) -> dict[str, str] | None:
        return game

    def _resolved_emulator_entry_for_game(self, game: dict[str, str]):
        raise AssertionError("native_multi_dir should short-circuit before emulator resolution")


class _NativeCloudPanelStub:
    def __init__(self) -> None:
        self.details_cloud_status_label = QLabel()
        self.details_cloud_empty_label = QLabel()
        self.details_cloud_upload_button = QPushButton()
        self.details_cloud_list_host = QWidget()
        self.details_cloud_list_layout = QVBoxLayout(self.details_cloud_list_host)
        self.details_cloud_list_layout.setContentsMargins(0, 0, 0, 0)
        self.details_cloud_list_layout.setSpacing(0)
        self.details_cloud_request_id = 1
        self.details_cloud_request_context: dict[str, object] = {}
        self.current_details_cloud_mode = "save"
        self.current_details_game = {"title": "Native Game", "platform": "Windows"}
        self._pcgw_paths_cache = {
            "native::Native Game": ["%APPDATA%/NativeGame/Saves"],
            "native::Native Game__manual": ["D:/ManualSaves"],
        }
        self.worker_calls: list[tuple[str, str, str, str, str]] = []

    def _theme_color(self, name: str, fallback: str) -> str:
        return fallback

    def _pcgw_paths_for_game(self, game: dict[str, str]) -> list[str] | None:
        return ["%APPDATA%/NativeGame/Saves"]

    def _pcgw_cache_key(self, game: dict[str, str]) -> str:
        return "native::Native Game"

    def _pcgw_remove_path_for_game(self, game: dict[str, str], raw_path: str) -> None:
        return None

    def _pcgw_add_manual_path_for_game(self, game: dict[str, str], folder: str) -> None:
        return None

    def _refresh_details_cloud_panel(self) -> None:
        return None

    def _cloud_sync_rom_id_for_game(self, game: dict[str, str], **kwargs: object) -> str:
        return "rom-native-1"

    def _start_details_cloud_records_worker(
        self,
        rom_id: str,
        save_type: str,
        *,
        kind_label: str,
        upload_reason: str,
        emulator_name: str,
    ) -> None:
        self.worker_calls.append((rom_id, save_type, kind_label, upload_reason, emulator_name))

    def _is_native_executable_platform(self, game: dict[str, str]) -> bool:
        return True

    def _save_record_timestamp(self, record: dict[str, object]) -> float:
        return 100.0

    def _make_details_cloud_record_widget(self, record: dict[str, object], save_type: str) -> QWidget:
        return QLabel(f"record-{record.get('id', '?')}")


class NativeCloudPanelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_native_cloud_panel_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_details_cloud_restore_enabled_allows_native_multi_dir_records(self) -> None:
        module = self._load_main_module()
        window = _RestoreEnabledNativeStub()

        enabled, reason = module.MainWindow._details_cloud_restore_enabled(
            window,
            {"id": "1", "emulator": "native_multi_dir"},
            "save",
        )

        self.assertTrue(enabled)
        self.assertEqual(reason, "")

    def test_refresh_native_save_panel_starts_cloud_records_worker_and_renders_cloud_section(self) -> None:
        module = self._load_main_module()
        window = _NativeCloudPanelStub()
        window._native_save_paths_for_game = types.MethodType(module.MainWindow._native_save_paths_for_game, window)
        window._native_cloud_saves_section_label = types.MethodType(module.MainWindow._native_cloud_saves_section_label, window)
        window._render_native_save_path_section = types.MethodType(module.MainWindow._render_native_save_path_section, window)

        module.MainWindow._refresh_native_save_panel(window, window.current_details_game, "save")

        self.assertEqual(len(window.worker_calls), 1)
        self.assertEqual(window.worker_calls[0][0], "rom-native-1")
        self.assertEqual(window.worker_calls[0][1], "save")
        self.assertEqual(window.worker_calls[0][4], "native_multi_dir")

        section_widgets = [
            window.details_cloud_list_layout.itemAt(index).widget()
            for index in range(window.details_cloud_list_layout.count())
            if window.details_cloud_list_layout.itemAt(index).widget() is not None
        ]
        self.assertTrue(any(widget.objectName() == "detailsNativePathSection" for widget in section_widgets))
        self.assertTrue(any(isinstance(widget, QLabel) and widget.text() == "Cloud Saves" for widget in section_widgets))

    def test_on_details_cloud_records_loaded_readds_native_path_section_before_records(self) -> None:
        module = self._load_main_module()
        window = _NativeCloudPanelStub()
        window.details_cloud_request_id = 7
        window.details_cloud_request_context = {
            "request_id": 7,
            "save_type": "save",
            "kind_label": "saves",
            "upload_reason": "",
            "emulator_name": "native_multi_dir",
        }

        window._clear_layout_items = types.MethodType(module.MainWindow._clear_layout_items, window)
        window._native_save_paths_for_game = types.MethodType(module.MainWindow._native_save_paths_for_game, window)
        window._native_cloud_saves_section_label = types.MethodType(module.MainWindow._native_cloud_saves_section_label, window)
        window._render_native_save_path_section = types.MethodType(module.MainWindow._render_native_save_path_section, window)

        stale_label = QLabel("stale")
        window.details_cloud_list_layout.addWidget(stale_label)

        module.MainWindow._on_details_cloud_records_loaded(
            window,
            7,
            "save",
            [{"id": "42", "emulator": "native_multi_dir", "file_size_bytes": 12}],
            "",
        )

        rendered_widgets = [
            window.details_cloud_list_layout.itemAt(index).widget()
            for index in range(window.details_cloud_list_layout.count())
            if window.details_cloud_list_layout.itemAt(index).widget() is not None
        ]

        self.assertTrue(any(widget.objectName() == "detailsNativePathSection" for widget in rendered_widgets))
        self.assertTrue(any(isinstance(widget, QLabel) and widget.text() == "Cloud Saves" for widget in rendered_widgets))
        self.assertTrue(any(isinstance(widget, QLabel) and widget.text() == "record-42" for widget in rendered_widgets))


if __name__ == "__main__":
    unittest.main()
