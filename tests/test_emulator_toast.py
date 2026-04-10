from __future__ import annotations

import importlib.util
import os
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication, QComboBox, QLineEdit, QListWidget


class _SaveEmulatorToastStubWindow:
    def __init__(self) -> None:
        self.config = {
            "emulators": [],
            "default_emulators": {},
            "default_retroarch_cores": {},
        }
        self.emulator_name_input = QLineEdit("DuckStation")
        self.emulator_path_input = QLineEdit("C:/Emulators/duckstation.exe")
        self.emulator_args_input = QLineEdit("%rom%")
        self.emulator_save_strategy_input = QComboBox()
        self.emulator_save_strategy_input.addItems(["auto", "single_file", "folder"])
        self.emulator_save_strategy_input.setCurrentText("auto")
        self.emulator_ignore_files_input = QLineEdit("")
        self.emulator_ignore_extensions_input = QLineEdit("")
        self.emulator_save_paths_input = QLineEdit("")
        self.emulator_state_paths_input = QLineEdit("")
        self.emulator_list = QListWidget()
        self.toast_calls: list[tuple[str, str]] = []
        self.saved_configs: list[dict[str, object]] = []

    def _normalize_save_strategy_value(self, value: object) -> str:
        text = str(value).strip().casefold()
        return text if text in {"auto", "single_file", "folder"} else "auto"

    def _emulators(self) -> list[dict[str, str]]:
        value = self.config.get("emulators", [])
        return value if isinstance(value, list) else []

    def _ensure_emulator_sync_settings(self, name: str, path: str) -> None:
        return None

    def _emulator_autoprofiles(self) -> dict[str, object]:
        return {}

    def _emulator_profile_for_entry(self, entry: dict[str, str]) -> object:
        return None

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

    def _default_assignable_server_platforms(self) -> list[str]:
        return []

    def _installed_retroarch_cores_for_platform(self, platform: str, emulator_name: str) -> list[str]:
        return []

    def _matching_platforms_for_emulator_keywords(self, name: str) -> list[str]:
        return []

    def _dolphin_variant_label_for_game(self, game: dict[str, str]) -> str:
        return ""

    def _dolphin_target_platforms_for_variant(self, variant: str) -> list[str]:
        return []

    def _normalize_emulators(self, value: object) -> list[dict[str, str]]:
        if isinstance(value, list):
            return [item for item in value if isinstance(item, dict)]
        return []

    def _refresh_emulator_views(self) -> None:
        return None

    def _clear_emulator_selection(self) -> None:
        return None

    def _save_config(self, config: dict[str, object]) -> None:
        self.saved_configs.append(dict(config))

    def _show_toast(self, message: str, level: str = "info") -> None:
        self.toast_calls.append((message, level))


class EmulatorAddToastTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    @staticmethod
    def _load_main_module():
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_emulator_toast_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_save_emulator_shows_success_toast_for_new_manual_add(self) -> None:
        module = self._load_main_module()
        window = _SaveEmulatorToastStubWindow()

        module.MainWindow._save_emulator(window)

        self.assertEqual(len(window.toast_calls), 1)


if __name__ == "__main__":
    unittest.main()
