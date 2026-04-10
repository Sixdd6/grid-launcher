from __future__ import annotations

import importlib.util
import os
import unittest
from pathlib import Path
from unittest.mock import patch

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


class _AutoConfigureRetroArchFallbackWindow:
    def __init__(self) -> None:
        self.config = {
            "emulators": [],
            "default_emulators": {},
            "default_retroarch_cores": {},
        }
        self.saved_configs: list[dict[str, object]] = []

    def _is_emulators_platform(self, game: dict[str, str]) -> bool:
        return True

    def _select_emulator_executable_path(self, game: dict[str, str], archive_path: Path) -> str:
        return "C:/Emulators/RetroArch/retroarch.exe"

    def _emulator_profile_for_game(self, game: dict[str, str], executable_path: str) -> dict[str, object]:
        return {"name": "RetroArch (Multi-System)", "platform_keywords": ["playstation"]}

    def _normalize_emulators(self, value: object) -> list[dict[str, str]]:
        if isinstance(value, list):
            return [item for item in value if isinstance(item, dict)]
        return []

    def _emulators(self) -> list[dict[str, str]]:
        value = self.config.get("emulators", [])
        return value if isinstance(value, list) else []

    def _normalize_default_emulators(self, value: object) -> dict[str, str]:
        if isinstance(value, dict):
            return {str(key): str(item) for key, item in value.items()}
        return {}

    def _normalize_default_retroarch_cores(self, value: object) -> dict[str, str]:
        if isinstance(value, dict):
            return {str(key): str(item) for key, item in value.items()}
        return {}

    def _auto_configured_emulator_name(self, base_name: str, game: dict[str, str]) -> str:
        return base_name

    def _normalize_save_strategy_value(self, value: object) -> str:
        text = str(value).strip().casefold()
        return text if text in {"auto", "single_file", "folder"} else "auto"

    def _is_retroarch_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
        return "retroarch" in emulator_name.casefold()

    def _default_assignable_server_platforms(self) -> list[str]:
        return ["Sony PlayStation"]

    def _matching_platforms_for_emulator_keywords(self, emulator_name: str) -> list[str]:
        return ["Sony PlayStation"]

    def _dolphin_variant_label_for_game(self, game: dict[str, str]) -> str:
        return ""

    def _dolphin_target_platforms_for_variant(self, variant: str) -> list[str]:
        return []

    def _retroarch_cores_for_platform(self, platform: str) -> list[str]:
        if platform.casefold() == "sony playstation":
            return ["pcsx_rearmed_libretro.dll"]
        return []

    def _retroarch_installed_core_ids_for_emulator(self, emulator_name: str) -> set[str]:
        return set()

    def _refresh_emulator_views(self) -> None:
        return None

    def _save_config(self, config: dict[str, object]) -> None:
        self.saved_configs.append(dict(config))

    def _ensure_emulator_sync_settings(self, name: str, path: str) -> None:
        return None


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

    def test_auto_configure_retroarch_assigns_platforms_when_emulator_not_yet_in_config(self) -> None:
        module = self._load_main_module()
        window = _AutoConfigureRetroArchFallbackWindow()
        game = {"title": "RetroArch", "platform": "Emulators"}

        def _fake_resolve_auto_configure(*args, **kwargs):
            installed_for_platform = kwargs["installed_retroarch_cores_for_platform"]
            installed = installed_for_platform("Sony PlayStation", "RetroArch (Multi-System)")
            self.assertEqual(installed, ["pcsx_rearmed_libretro.dll"])
            return (
                [{"name": "RetroArch (Multi-System)", "path": args[1]}],
                {"Sony PlayStation": "RetroArch (Multi-System)"},
                {"Sony PlayStation": "pcsx_rearmed_libretro.dll"},
            )

        with patch.object(module, "resolve_installed_retroarch_core_ids", return_value={"pcsx_rearmed_libretro.dll"}) as mock_ids:
            with patch.object(module, "resolve_auto_configure_emulator_settings", side_effect=_fake_resolve_auto_configure):
                applied = module.MainWindow._auto_configure_installed_emulator(window, game, Path("retroarch.zip"))

        self.assertTrue(applied)
        self.assertEqual(window.config["default_emulators"].get("Sony PlayStation"), "RetroArch (Multi-System)")
        self.assertEqual(window.config["default_retroarch_cores"].get("Sony PlayStation"), "pcsx_rearmed_libretro.dll")
        self.assertEqual(window.config["emulators"][0].get("name"), "RetroArch (Multi-System)")
        mock_ids.assert_called_once_with("C:/Emulators/RetroArch/retroarch.exe")


if __name__ == "__main__":
    unittest.main()
