from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from grid_launcher.emulator.launch import prepare_emulator_launch_command
from grid_launcher.ui.mixins.details_view_mixin import DetailsViewMixin


class _StubWindow:
    def __init__(
        self,
        *,
        current_game: dict[str, str],
        is_installed: bool,
        installed_record: dict[str, str] | None,
        firmware_result: str = "",
        firmware_exception: Exception | None = None,
    ) -> None:
        self.current_details_game = current_game
        self._is_installed = is_installed
        self._installed_record = installed_record
        self._firmware_result = firmware_result
        self._firmware_exception = firmware_exception

        self.call_order: list[str] = []
        self.firmware_call_args: tuple[dict[str, str], dict[str, str]] | None = None
        self.launch_called = False
        self.install_started = False
        self.firmware_called = False

    def _is_game_installed(self, game: dict[str, str]) -> bool:
        return self._is_installed

    def _installed_game_record(self, game: dict[str, str]) -> dict[str, str] | None:
        return self._installed_record

    def _auto_sync_before_launch(self, game: dict[str, str]) -> None:
        self.call_order.append("auto_sync")

    def _install_firmware_for_game_without_ui(
        self,
        game: dict[str, str],
        firmware_filename_map: dict[str, str],
    ) -> str:
        self.call_order.append("install_firmware")
        self.firmware_called = True
        self.firmware_call_args = (game, firmware_filename_map)
        if self._firmware_exception is not None:
            raise self._firmware_exception
        return self._firmware_result

    def _launch_installed_game(self, game: dict[str, str]) -> None:
        self.call_order.append("launch")
        self.launch_called = True

    def _start_async_install(self, game: dict[str, str]) -> None:
        self.call_order.append("start_async_install")
        self.install_started = True

    def _resolved_emulator_entry_for_game(
        self,
        game: dict[str, str],
    ) -> tuple[str, dict[str, str] | None]:
        return "", None


class PerformGameActionFirmwareTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_firmware_launch_tests", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load grid-launcher.py for tests.")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        cls.module = module

    def test_perform_game_action_calls_firmware_install_before_launch(self) -> None:
        current_game = {"title": "Test Game", "platform": "gc"}
        installed_game = {"title": "Installed Test Game", "platform": "gc"}
        window = _StubWindow(
            current_game=current_game,
            is_installed=True,
            installed_record=installed_game,
            firmware_result="",
        )

        type(self).module.MainWindow._perform_game_action(window)

        self.assertEqual(window.firmware_call_args, (installed_game, {}))
        self.assertLess(window.call_order.index("install_firmware"), window.call_order.index("launch"))

    def test_perform_game_action_still_launches_on_firmware_warnings(self) -> None:
        current_game = {"title": "Test Game", "platform": "gc"}
        installed_game = {"title": "Installed Test Game", "platform": "gc"}
        window = _StubWindow(
            current_game=current_game,
            is_installed=True,
            installed_record=installed_game,
            firmware_result="Missing BIOS file",
        )

        type(self).module.MainWindow._perform_game_action(window)

        self.assertTrue(window.launch_called)

    def test_perform_game_action_skips_firmware_when_not_installed(self) -> None:
        current_game = {"title": "Test Game", "platform": "gc"}
        window = _StubWindow(
            current_game=current_game,
            is_installed=False,
            installed_record=None,
        )

        type(self).module.MainWindow._perform_game_action(window)

        self.assertFalse(window.firmware_called)
        self.assertTrue(window.install_started)

    def test_perform_game_action_firmware_exception_does_not_block_launch(self) -> None:
        current_game = {"title": "Test Game", "platform": "gc"}
        installed_game = {"title": "Installed Test Game", "platform": "gc"}
        window = _StubWindow(
            current_game=current_game,
            is_installed=True,
            installed_record=installed_game,
            firmware_exception=Exception("Network error"),
        )

        type(self).module.MainWindow._perform_game_action(window)

        self.assertTrue(window.launch_called)


class _StubPS3LaunchWindow(DetailsViewMixin):
    def __init__(self, *, ps3_dev_hdd0: Path | None, rpcs3_data_root: Path | None) -> None:
        self._ps3_dev_hdd0 = ps3_dev_hdd0
        self._rpcs3_data_root = rpcs3_data_root

    def _is_native_executable_platform(self, game: dict[str, str]) -> bool:
        del game
        return False

    def _default_emulator_name_for_platform(self, platform: str) -> str:
        del platform
        return "RPCS3"

    def _emulator_entry_by_name(self, name: str) -> dict[str, str] | None:
        del name
        return {"path": "/fake/rpcs3"}

    def _resolved_rom_path_for_game(self, game: dict[str, str]) -> str:
        del game
        return "/fake/rom"

    def _resolved_launch_arguments_for_game(self, game: dict[str, str]) -> list[str]:
        del game
        return []

    def _is_retroarch_emulator_name(self, name: str) -> bool:
        del name
        return False

    def _normalized_retroarch_core_args(self, game: dict[str, str], core_id: str) -> list[str]:
        del game, core_id
        return []

    def _is_rpcs3_emulator_name(self, name: str) -> bool:
        del name
        return True

    def _ps3_dev_hdd0_for_game(self, game: dict[str, str]) -> Path | None:
        del game
        return self._ps3_dev_hdd0

    def _rpcs3_data_root_for_game(self, game: dict[str, str]) -> Path | None:
        del game
        return self._rpcs3_data_root

    def _ensure_emulator_sync_settings(self, name: str, path: str) -> None:
        del name, path

    def _register_game_session_for_auto_upload(self, game: dict[str, str], process, emulator_name: str) -> None:
        del game, process, emulator_name

    def _warn_if_process_exited_early(self, process, command: list[str]) -> None:
        del process, command


class PS3CustomConfigCopyOnLaunchTests(unittest.TestCase):
    def test_copy_called_for_ps3_game_at_launch(self) -> None:
        self.assertTrue(callable(prepare_emulator_launch_command))
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            dev_hdd0 = tmp_path / "vfs" / "dev_hdd0"
            rpcs3_root = tmp_path / "rpcs3"
            window = _StubPS3LaunchWindow(ps3_dev_hdd0=dev_hdd0, rpcs3_data_root=rpcs3_root)
            game = {"platform": "PlayStation 3", "ps3_game_id": "BLUS30443", "title": "Test Game"}
            process = MagicMock()

            with (
                patch(
                    "grid_launcher.ui.mixins.details_view_mixin.prepare_emulator_launch_command",
                    return_value=("RPCS3", ["/fake/rpcs3", "/fake/rom"], None),
                ),
                patch("grid_launcher.ui.mixins.details_view_mixin.subprocess.Popen", return_value=process),
                patch("grid_launcher.ui.mixins.details_view_mixin.QTimer.singleShot"),
                patch("grid_launcher.ui.mixins.details_view_mixin.copy_ps3_custom_config_to_emulator") as copy_cfg,
            ):
                launched = window._launch_installed_game(game)

            self.assertTrue(launched)
            copy_cfg.assert_called_once_with(dev_hdd0.parent / "config", rpcs3_root)

    def test_copy_skipped_when_ps3_game_id_empty(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            dev_hdd0 = tmp_path / "vfs" / "dev_hdd0"
            rpcs3_root = tmp_path / "rpcs3"
            window = _StubPS3LaunchWindow(ps3_dev_hdd0=dev_hdd0, rpcs3_data_root=rpcs3_root)
            game = {"platform": "PlayStation 3", "ps3_game_id": "", "title": "Test Game"}
            process = MagicMock()

            with (
                patch(
                    "grid_launcher.ui.mixins.details_view_mixin.prepare_emulator_launch_command",
                    return_value=("RPCS3", ["/fake/rpcs3", "/fake/rom"], None),
                ),
                patch("grid_launcher.ui.mixins.details_view_mixin.subprocess.Popen", return_value=process),
                patch("grid_launcher.ui.mixins.details_view_mixin.QTimer.singleShot"),
                patch("grid_launcher.ui.mixins.details_view_mixin.copy_ps3_custom_config_to_emulator") as copy_cfg,
            ):
                launched = window._launch_installed_game(game)

            self.assertTrue(launched)
            copy_cfg.assert_not_called()

    def test_copy_skipped_when_rpcs3_data_root_is_none(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            dev_hdd0 = tmp_path / "vfs" / "dev_hdd0"
            window = _StubPS3LaunchWindow(ps3_dev_hdd0=dev_hdd0, rpcs3_data_root=None)
            game = {"platform": "PlayStation 3", "ps3_game_id": "BLUS30443", "title": "Test Game"}
            process = MagicMock()

            with (
                patch(
                    "grid_launcher.ui.mixins.details_view_mixin.prepare_emulator_launch_command",
                    return_value=("RPCS3", ["/fake/rpcs3", "/fake/rom"], None),
                ),
                patch("grid_launcher.ui.mixins.details_view_mixin.subprocess.Popen", return_value=process),
                patch("grid_launcher.ui.mixins.details_view_mixin.QTimer.singleShot"),
                patch("grid_launcher.ui.mixins.details_view_mixin.copy_ps3_custom_config_to_emulator") as copy_cfg,
            ):
                launched = window._launch_installed_game(game)

            self.assertTrue(launched)
            copy_cfg.assert_not_called()


if __name__ == "__main__":
    unittest.main()
