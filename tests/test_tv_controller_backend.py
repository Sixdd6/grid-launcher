from __future__ import annotations

import os
import sys
import time
import unittest
from unittest.mock import MagicMock, patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtCore import QCoreApplication

_app = QCoreApplication.instance() or QCoreApplication(sys.argv)


class _StubAppBackend:
    def __init__(self, exclusion_list: list[str]) -> None:
        self.tvGuideExclusionList = exclusion_list


class _StubGameBackend:
    def __init__(self, emulator_name: str) -> None:
        self._active_session: dict = {"emulator_name": emulator_name}


class ControllerBackendTests(unittest.TestCase):

    def _make_backend(self, exclusion_list=None, emulator_name=""):
        from rom_mate.tv.bridge.controller import ControllerBackend
        app_backend = _StubAppBackend(exclusion_list or [])
        game_backend = _StubGameBackend(emulator_name)
        return ControllerBackend(app_backend=app_backend, game_backend=game_backend)

    # ------------------------------------------------------------------
    # Guide button suppression
    # ------------------------------------------------------------------

    def test_guide_button_suppressed_for_excluded_emulator(self):
        backend = self._make_backend(exclusion_list=["RPCS3"], emulator_name="RPCS3")
        self.assertTrue(backend.should_suppress_guide_button())

    def test_guide_button_allowed_for_non_excluded_emulator(self):
        backend = self._make_backend(exclusion_list=["RPCS3"], emulator_name="RetroArch")
        self.assertFalse(backend.should_suppress_guide_button())

    def test_guide_button_exclusion_is_case_insensitive(self):
        backend = self._make_backend(exclusion_list=["Cemu"], emulator_name="cemu")
        self.assertTrue(backend.should_suppress_guide_button())

    def test_guide_button_exclusion_is_case_insensitive_reversed(self):
        backend = self._make_backend(exclusion_list=["cemu"], emulator_name="Cemu")
        self.assertTrue(backend.should_suppress_guide_button())

    def test_guide_button_allowed_when_no_game_running(self):
        backend = self._make_backend(exclusion_list=["RPCS3"], emulator_name="")
        self.assertFalse(backend.should_suppress_guide_button())

    def test_guide_button_allowed_when_no_backends(self):
        from rom_mate.tv.bridge.controller import ControllerBackend
        backend = ControllerBackend()
        self.assertFalse(backend.should_suppress_guide_button())

    def test_guide_button_allowed_when_exclusion_list_empty(self):
        backend = self._make_backend(exclusion_list=[], emulator_name="RPCS3")
        self.assertFalse(backend.should_suppress_guide_button())

    # ------------------------------------------------------------------
    # Button event mapping
    # ------------------------------------------------------------------

    def test_button_press_emits_correct_navigation_event(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_SOUTH", 1.0)
        self.assertEqual(emitted, ["confirm"])

    def test_button_release_does_not_emit(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_SOUTH", 0.0)
        self.assertEqual(emitted, [])

    def test_dpad_up_emits_up(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_DPAD_UP", 1.0)
        self.assertEqual(emitted, ["up"])

    def test_dpad_down_emits_down(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_DPAD_DOWN", 1.0)
        self.assertEqual(emitted, ["down"])

    def test_btn_east_emits_back(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_EAST", 1.0)
        self.assertEqual(emitted, ["back"])

    def test_btn_tl_emits_tab_prev(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_TL", 1.0)
        self.assertEqual(emitted, ["tab_prev"])

    def test_btn_tr_emits_tab_next(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_TR", 1.0)
        self.assertEqual(emitted, ["tab_next"])

    def test_guide_button_emits_when_not_suppressed(self):
        backend = self._make_backend(exclusion_list=[], emulator_name="")
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_MODE", 1.0)
        self.assertEqual(emitted, ["guide_button"])

    def test_guide_button_suppressed_does_not_emit(self):
        backend = self._make_backend(exclusion_list=["RPCS3"], emulator_name="RPCS3")
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_MODE", 1.0)
        self.assertEqual(emitted, [])

    def test_unknown_button_code_does_not_emit(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("BTN_UNKNOWN_XYZ", 1.0)
        self.assertEqual(emitted, [])

    def test_emit_navigation_emits_signal(self):
        backend = self._make_backend()
        received = []
        backend.navigationEvent.connect(lambda ev: received.append(ev))
        backend.emitNavigation("confirm")
        self.assertEqual(received, ["confirm"])

    def test_emit_navigation_empty_string(self):
        backend = self._make_backend()
        received = []
        backend.navigationEvent.connect(lambda ev: received.append(ev))
        backend.emitNavigation("")
        self.assertEqual(received, [""])

    # ------------------------------------------------------------------
    # Axis dead-zone filtering
    # ------------------------------------------------------------------

    def test_axis_below_dead_zone_does_not_emit(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("ABS_X", 0.1)   # below 0.3 threshold
        self.assertEqual(emitted, [])

    def test_axis_above_dead_zone_positive_emits_right(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("ABS_X", 0.8)
        self.assertEqual(emitted, ["right"])

    def test_axis_above_dead_zone_negative_emits_left(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("ABS_X", -0.8)
        self.assertEqual(emitted, ["left"])

    def test_axis_y_positive_emits_down(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("ABS_Y", 0.9)
        self.assertEqual(emitted, ["down"])

    def test_axis_y_negative_emits_up(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("ABS_Y", -0.9)
        self.assertEqual(emitted, ["up"])

    def test_axis_center_clears_repeat_state(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        # Push axis right, then center it
        backend._on_raw_event("ABS_X", 0.9)
        backend._on_raw_event("ABS_X", 0.0)
        # Push right again — should fire immediately (no lingering repeat timer)
        backend._on_raw_event("ABS_X", 0.9)
        self.assertEqual(emitted, ["right", "right"])

    def test_axis_repeat_suppressed_within_interval(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        # Fire once, then immediately again in the same direction
        backend._on_raw_event("ABS_X", 0.9)
        # Manually set last_fire to now so the repeat interval hasn't elapsed
        now = time.monotonic()
        backend._axis_state["ABS_X"] = ("right", now)
        backend._on_raw_event("ABS_X", 0.9)
        self.assertEqual(emitted, ["right"])  # only one emission

    def test_axis_direction_change_fires_immediately(self):
        backend = self._make_backend()
        emitted = []
        backend.navigationEvent.connect(lambda e: emitted.append(e))
        backend._on_raw_event("ABS_X", 0.9)   # right
        # Force state to look like it just fired right
        now = time.monotonic()
        backend._axis_state["ABS_X"] = ("right", now)
        # Now push left — direction changed, should fire immediately
        backend._on_raw_event("ABS_X", -0.9)
        self.assertEqual(emitted, ["right", "left"])

    # ------------------------------------------------------------------
    # Event classification
    # ------------------------------------------------------------------

    def test_classify_button_event(self):
        from rom_mate.tv.bridge.controller import ControllerBackend
        backend = ControllerBackend()
        self.assertEqual(backend._classify_event_type("BTN_SOUTH"), "button")

    def test_classify_axis_event(self):
        from rom_mate.tv.bridge.controller import ControllerBackend
        backend = ControllerBackend()
        self.assertEqual(backend._classify_event_type("ABS_X"), "axis")

    def test_classify_unknown_event(self):
        from rom_mate.tv.bridge.controller import ControllerBackend
        backend = ControllerBackend()
        self.assertEqual(backend._classify_event_type("SYN_REPORT"), "unknown")

    def test_gamepad_poll_thread_run_returns_when_pygame_missing(self):
        from rom_mate.tv.bridge.controller import _GamepadPollThread

        original_import = __import__

        def _import_with_missing_pygame(name, globals=None, locals=None, fromlist=(), level=0):
            if name == "pygame":
                raise ImportError("pygame not installed")
            return original_import(name, globals, locals, fromlist, level)

        thread = _GamepadPollThread()
        with patch("builtins.__import__", side_effect=_import_with_missing_pygame):
            thread.run()

    def test_xinput_poll_thread_is_install_active_defaults_to_false(self):
        from rom_mate.tv.bridge.controller import _XInputPollThread

        thread = _XInputPollThread()
        self.assertFalse(thread._is_install_active())

    def test_xinput_poll_thread_uses_provided_callable(self):
        from rom_mate.tv.bridge.controller import _XInputPollThread

        thread = _XInputPollThread(is_install_active=lambda: True)
        self.assertTrue(thread._is_install_active())


class TestControllerGuideButton(unittest.TestCase):
    def test_guide_button_calls_request_pause_when_session_active(self):
        from rom_mate.tv.bridge.controller import ControllerBackend

        app_backend = MagicMock()
        app_backend.tvGuideExclusionList = []

        game_backend = MagicMock()
        game_backend.isSessionActive = True
        game_backend.activeEmulatorName = "RetroArch"
        game_backend.requestPause = MagicMock()

        backend = ControllerBackend(app_backend=app_backend, game_backend=game_backend)
        emitted: list[str] = []
        backend.navigationEvent.connect(lambda name: emitted.append(name))

        self.assertFalse(backend.should_suppress_guide_button())

        backend._handle_button("BTN_MODE")

        game_backend.requestPause.assert_called_once_with()
        self.assertEqual(emitted, [])

    def test_should_suppress_guide_button_casefold_match(self):
        from rom_mate.tv.bridge.controller import ControllerBackend

        app_backend = MagicMock()
        app_backend.tvGuideExclusionList = ["RPCS3", "RetroArch"]

        game_backend = MagicMock()
        game_backend.activeEmulatorName = "retroarch"

        backend = ControllerBackend(app_backend=app_backend, game_backend=game_backend)

        self.assertTrue(backend.should_suppress_guide_button())

    def test_should_not_suppress_guide_button_for_non_matching_emulator(self):
        from rom_mate.tv.bridge.controller import ControllerBackend

        app_backend = MagicMock()
        app_backend.tvGuideExclusionList = ["RPCS3"]

        game_backend = MagicMock()
        game_backend.activeEmulatorName = "Dolphin"

        backend = ControllerBackend(app_backend=app_backend, game_backend=game_backend)

        self.assertFalse(backend.should_suppress_guide_button())


if __name__ == "__main__":
    unittest.main()
