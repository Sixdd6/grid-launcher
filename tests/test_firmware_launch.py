from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


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
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_firmware_launch_tests", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load rom-mate.py for tests.")
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


if __name__ == "__main__":
    unittest.main()
