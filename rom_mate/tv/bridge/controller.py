from __future__ import annotations

import sys
import time
from typing import TYPE_CHECKING, Any

from PySide6.QtCore import QObject, QThread, Qt, Signal, Slot

if TYPE_CHECKING:
    pass

_DEAD_ZONE = 0.3
_REPEAT_INTERVAL = 0.2  # seconds between repeated nav events from held axis


# ---------------------------------------------------------------------------
# XInput constants (Windows only)
# ---------------------------------------------------------------------------
_XINPUT_GAMEPAD_DPAD_UP = 0x0001
_XINPUT_GAMEPAD_DPAD_DOWN = 0x0002
_XINPUT_GAMEPAD_DPAD_LEFT = 0x0004
_XINPUT_GAMEPAD_DPAD_RIGHT = 0x0008
_XINPUT_GAMEPAD_START = 0x0010
_XINPUT_GAMEPAD_BACK = 0x0020
_XINPUT_GAMEPAD_LEFT_THUMB = 0x0040
_XINPUT_GAMEPAD_RIGHT_THUMB = 0x0080
_XINPUT_GAMEPAD_LEFT_SHOULDER = 0x0100
_XINPUT_GAMEPAD_RIGHT_SHOULDER = 0x0200
_XINPUT_GAMEPAD_A = 0x1000
_XINPUT_GAMEPAD_B = 0x2000
_XINPUT_GAMEPAD_X = 0x4000
_XINPUT_GAMEPAD_Y = 0x8000
_XINPUT_TRIGGER_THRESHOLD = 30  # 0-255; treat above this as pressed
_XINPUT_STICK_MAX = 32767.0

# Map each XInput digital button bit -> event code (same names as _BUTTON_MAP)
_XINPUT_BUTTON_BITS: list[tuple[int, str]] = [
    (_XINPUT_GAMEPAD_DPAD_UP, "BTN_DPAD_UP"),
    (_XINPUT_GAMEPAD_DPAD_DOWN, "BTN_DPAD_DOWN"),
    (_XINPUT_GAMEPAD_DPAD_LEFT, "BTN_DPAD_LEFT"),
    (_XINPUT_GAMEPAD_DPAD_RIGHT, "BTN_DPAD_RIGHT"),
    (_XINPUT_GAMEPAD_START, "BTN_START"),
    (_XINPUT_GAMEPAD_BACK, "BTN_SELECT"),
    (_XINPUT_GAMEPAD_LEFT_THUMB, "BTN_THUMBL"),
    (_XINPUT_GAMEPAD_RIGHT_THUMB, "BTN_THUMBR"),
    (_XINPUT_GAMEPAD_LEFT_SHOULDER, "BTN_TL"),
    (_XINPUT_GAMEPAD_RIGHT_SHOULDER, "BTN_TR"),
    (_XINPUT_GAMEPAD_A, "BTN_SOUTH"),
    (_XINPUT_GAMEPAD_B, "BTN_EAST"),
    (_XINPUT_GAMEPAD_X, "BTN_WEST"),
    (_XINPUT_GAMEPAD_Y, "BTN_NORTH"),
]


# ---------------------------------------------------------------------------
# Windows XInput poll thread
# ---------------------------------------------------------------------------


class _XInputPollThread(QThread):
    """
    Polls XInput1_4.dll directly for up to 4 controllers.
    Emits edge-triggered button events and normalised axis events.
    Guide button is NOT polled here (XInputGetState doesn't expose it);
    it is handled separately by _PygameGuidePollThread.
    """

    event_received = Signal(str, float)  # event_code, state_value

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._running = True

    def stop(self) -> None:
        self._running = False

    def run(self) -> None:
        try:
            import ctypes
            import ctypes.wintypes as wt
        except ImportError:
            print("[TV Controller] ctypes not available - XInput polling disabled", file=sys.stderr)
            return

        # Load XInput DLL
        xinput = None
        for dll_name in ("XInput1_4.dll", "XInput9_1_0.dll", "XInput1_3.dll"):
            try:
                xinput = ctypes.WinDLL(dll_name)
                break
            except OSError:
                continue
        if xinput is None:
            print("[TV Controller] XInput DLL not found", file=sys.stderr)
            return

        class _Gamepad(ctypes.Structure):
            _fields_ = [
                ("wButtons", wt.WORD),
                ("bLeftTrigger", wt.BYTE),
                ("bRightTrigger", wt.BYTE),
                ("sThumbLX", wt.SHORT),
                ("sThumbLY", wt.SHORT),
                ("sThumbRX", wt.SHORT),
                ("sThumbRY", wt.SHORT),
            ]

        class _State(ctypes.Structure):
            _fields_ = [
                ("dwPacketNumber", wt.DWORD),
                ("Gamepad", _Gamepad),
            ]

        XInputGetState = xinput.XInputGetState
        XInputGetState.argtypes = [wt.DWORD, ctypes.POINTER(_State)]
        XInputGetState.restype = wt.DWORD
        ERROR_SUCCESS = 0
        ERROR_NOT_CONNECTED = 1167

        # Track previous button state for edge detection (per controller slot)
        prev_buttons: dict[int, int] = {}
        prev_lt: dict[int, bool] = {}
        prev_rt: dict[int, bool] = {}
        prev_axes: dict[int, dict[str, float]] = {}
        last_axis_emit_times: dict[int, dict[str, float]] = {}

        print("[TV Controller] XInput poll thread started", file=sys.stderr)

        while self._running:
            for user_index in range(4):
                state = _State()
                ret = XInputGetState(user_index, ctypes.byref(state))
                if ret == ERROR_NOT_CONNECTED:
                    prev_buttons.pop(user_index, None)
                    continue
                if ret != ERROR_SUCCESS:
                    continue

                gp = state.Gamepad
                buttons = gp.wButtons
                old_buttons = prev_buttons.get(user_index, 0)

                # Edge-triggered digital buttons
                for bit, code in _XINPUT_BUTTON_BITS:
                    was = bool(old_buttons & bit)
                    now = bool(buttons & bit)
                    if now and not was:
                        self.event_received.emit(code, 1.0)
                    elif was and not now:
                        self.event_received.emit(code, 0.0)

                prev_buttons[user_index] = buttons

                # Triggers: treat as button press/release
                lt_pressed = gp.bLeftTrigger > _XINPUT_TRIGGER_THRESHOLD
                rt_pressed = gp.bRightTrigger > _XINPUT_TRIGGER_THRESHOLD
                if lt_pressed != prev_lt.get(user_index, False):
                    self.event_received.emit("BTN_TL2", 1.0 if lt_pressed else 0.0)
                    prev_lt[user_index] = lt_pressed
                if rt_pressed != prev_rt.get(user_index, False):
                    self.event_received.emit("BTN_TR2", 1.0 if rt_pressed else 0.0)
                    prev_rt[user_index] = rt_pressed

                # Analog sticks: emit on value change OR periodically when held above dead zone
                _AXIS_MIN_DELTA = 0.02
                _AXIS_REPEAT_INTERVAL = _REPEAT_INTERVAL  # reuse the module-level 0.2s constant
                _AXIS_DEAD_ZONE = _DEAD_ZONE  # reuse the module-level 0.3 constant
                now_t = time.monotonic()
                axes_now = {
                    "ABS_X":  gp.sThumbLX / _XINPUT_STICK_MAX,
                    "ABS_Y":  -gp.sThumbLY / _XINPUT_STICK_MAX,
                    "ABS_RX": gp.sThumbRX / _XINPUT_STICK_MAX,
                    "ABS_RY": -gp.sThumbRY / _XINPUT_STICK_MAX,
                }
                old_axes = prev_axes.get(user_index, {})
                last_axis_emit = last_axis_emit_times.get(user_index, {})
                for axis_code, val in axes_now.items():
                    changed = abs(val - old_axes.get(axis_code, 999.0)) > _AXIS_MIN_DELTA
                    held_above_dead_zone = abs(val) > _AXIS_DEAD_ZONE
                    repeat_due = (now_t - last_axis_emit.get(axis_code, 0.0)) >= _AXIS_REPEAT_INTERVAL
                    if changed or (held_above_dead_zone and repeat_due):
                        self.event_received.emit(axis_code, val)
                        last_axis_emit[axis_code] = now_t
                prev_axes[user_index] = axes_now
                last_axis_emit_times[user_index] = last_axis_emit

            time.sleep(0.016)  # ~60 Hz


# ---------------------------------------------------------------------------
# Pygame guide-button-only poll thread (Windows helper)
# ---------------------------------------------------------------------------


class _PygameGuidePollThread(QThread):
    """
    Minimal pygame thread that watches ONLY for the guide button (joystick
    button 10 on 8BitDo / SDL HID path). Used on Windows alongside
    _XInputPollThread which cannot see the guide button via XInput API.
    """

    event_received = Signal(str, float)

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._running = True

    def stop(self) -> None:
        self._running = False

    def run(self) -> None:
        try:
            import os
            import pygame
        except ImportError:
            return

        os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
        os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
        os.environ["SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS"] = "1"

        pygame.init()
        pygame.joystick.init()

        joysticks: list[Any] = []
        for i in range(pygame.joystick.get_count()):
            js = pygame.joystick.Joystick(i)
            js.init()
            joysticks.append(js)

        print(f"[TV Controller] pygame guide-thread: {len(joysticks)} joystick(s)", file=sys.stderr)

        while self._running:
            try:
                pygame.event.pump()
                for event in pygame.event.get():
                    if event.type == pygame.JOYBUTTONDOWN and event.button == 10:
                        self.event_received.emit("BTN_MODE", 1.0)
                    elif event.type == pygame.JOYBUTTONUP and event.button == 10:
                        self.event_received.emit("BTN_MODE", 0.0)
            except Exception:
                pass
            time.sleep(0.016)

        try:
            pygame.quit()
        except Exception:
            pass


# ---------------------------------------------------------------------------
# Full pygame poll thread (non-Windows fallback)
# ---------------------------------------------------------------------------

# pygame button index -> event code name
_PYGAME_BUTTON_MAP: dict[int, str] = {
    0: "BTN_SOUTH",
    1: "BTN_EAST",
    2: "BTN_WEST",
    3: "BTN_NORTH",
    4: "BTN_TL",
    5: "BTN_TR",
    6: "BTN_SELECT",
    7: "BTN_START",
    8: "BTN_THUMBL",
    9: "BTN_THUMBR",
    10: "BTN_MODE",
}

_PYGAME_AXIS_MAP: dict[int, str] = {
    0: "ABS_X",
    1: "ABS_Y",
    2: "ABS_Z",
    3: "ABS_RX",
    4: "ABS_RY",
    5: "ABS_RZ",
}


class _GamepadPollThread(QThread):
    """Full pygame joystick poll thread - used on non-Windows platforms."""

    event_received = Signal(str, float)

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._running = True

    def stop(self) -> None:
        self._running = False

    def run(self) -> None:
        try:
            import os
            import pygame
        except ImportError:
            print("[TV Controller] WARNING: pygame not installed.", file=sys.stderr)
            return

        os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
        os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
        os.environ["SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS"] = "1"

        pygame.init()
        pygame.joystick.init()

        joysticks: dict[int, Any] = {}
        for i in range(pygame.joystick.get_count()):
            js = pygame.joystick.Joystick(i)
            js.init()
            joysticks[i] = js

        count = pygame.joystick.get_count()
        print(f"[TV Controller] pygame OK. Joysticks: {count}", file=sys.stderr)
        for js in joysticks.values():
            print(f"[TV Controller]  - {js.get_name()}", file=sys.stderr)

        while self._running:
            try:
                pygame.event.pump()
                for event in pygame.event.get():
                    if event.type == pygame.JOYDEVICEADDED:
                        js = pygame.joystick.Joystick(event.device_index)
                        js.init()
                        joysticks[event.device_index] = js
                    elif event.type == pygame.JOYDEVICEREMOVED:
                        joysticks.pop(event.instance_id, None)
                    elif event.type == pygame.JOYBUTTONDOWN:
                        code = _PYGAME_BUTTON_MAP.get(event.button)
                        if code:
                            self.event_received.emit(code, 1.0)
                    elif event.type == pygame.JOYBUTTONUP:
                        code = _PYGAME_BUTTON_MAP.get(event.button)
                        if code:
                            self.event_received.emit(code, 0.0)
                    elif event.type == pygame.JOYAXISMOTION:
                        code = _PYGAME_AXIS_MAP.get(event.axis)
                        if code:
                            self.event_received.emit(code, float(event.value))
                    elif event.type == pygame.JOYHATMOTION:
                        x, y = event.value
                        self.event_received.emit("ABS_HAT0X", float(x))
                        self.event_received.emit("ABS_HAT0Y", float(-y))
            except Exception:
                pass
            time.sleep(0.008)

        try:
            pygame.quit()
        except Exception:
            pass


class ControllerBackend(QObject):
    """
    Translates gamepad hardware events into high-level navigation signals for QML.

    Emits:
        navigationEvent(str) — values: "up", "down", "left", "right",
                               "confirm", "back", "tab_prev", "tab_next",
                               "guide_button"
    """

    navigationEvent = Signal(str)

    # Button code → navigation event name
    _BUTTON_MAP: dict[str, str] = {
        "BTN_DPAD_UP": "up",
        "BTN_DPAD_DOWN": "down",
        "BTN_DPAD_LEFT": "left",
        "BTN_DPAD_RIGHT": "right",
        "BTN_SOUTH": "confirm",    # A / Cross
        "BTN_EAST": "back",        # B / Circle
        "BTN_TL": "tab_prev",      # Left Shoulder (LB)
        "BTN_TR": "tab_next",      # Right Shoulder (RB)
        "BTN_MODE": "guide_button",
    }

    # Axis code → (negative_event, positive_event)
    _AXIS_MAP: dict[str, tuple[str, str]] = {
        "ABS_X": ("left", "right"),
        "ABS_Y": ("up", "down"),   # note: Y+ is typically down
        "ABS_HAT0X": ("left", "right"),
        "ABS_HAT0Y": ("up", "down"),
    }

    def __init__(
        self,
        app_backend: Any | None = None,
        game_backend: Any | None = None,
        parent: QObject | None = None,
    ) -> None:
        super().__init__(parent)
        self._app_backend = app_backend
        self._game_backend = game_backend
        self._poll_thread: _GamepadPollThread | None = None
        self._guide_thread: Any = None
        # Axis repeat state: axis_code → (last_direction, last_fire_time)
        self._axis_state: dict[str, tuple[str, float]] = {}

    def start(self) -> None:
        """Start the gamepad polling thread(s)."""
        if self._poll_thread is not None and self._poll_thread.isRunning():
            return

        if sys.platform == "win32":
            # Windows: XInput for standard buttons + pygame for guide button
            self._poll_thread = _XInputPollThread(self)
            self._poll_thread.event_received.connect(
                self._on_raw_event, Qt.ConnectionType.QueuedConnection
            )
            self._poll_thread.start()

            self._guide_thread = _PygameGuidePollThread(self)
            self._guide_thread.event_received.connect(
                self._on_raw_event, Qt.ConnectionType.QueuedConnection
            )
            self._guide_thread.start()
        else:
            self._poll_thread = _GamepadPollThread(self)
            self._poll_thread.event_received.connect(
                self._on_raw_event, Qt.ConnectionType.QueuedConnection
            )
            self._poll_thread.start()

    def stop(self) -> None:
        """Stop the gamepad polling thread(s)."""
        if self._poll_thread is not None:
            self._poll_thread.stop()
            self._poll_thread.quit()
            self._poll_thread.wait(500)
            self._poll_thread = None
        guide = getattr(self, "_guide_thread", None)
        if guide is not None:
            guide.stop()
            guide.quit()
            guide.wait(500)
            self._guide_thread = None

    @Slot(str)
    def emitNavigation(self, event: str) -> None:
        """Called from QML keyboard shortcuts to emit a navigation event."""
        self.navigationEvent.emit(event)

    @Slot(result=int)
    def joystickCount(self) -> int:
        """Returns the number of detected joysticks (for diagnostics)."""
        try:
            import pygame
            if pygame.joystick.get_init():
                return pygame.joystick.get_count()
        except Exception:
            pass
        return -1

    # ------------------------------------------------------------------
    # Public query
    # ------------------------------------------------------------------

    def should_suppress_guide_button(self) -> bool:
        """
        Returns True if the currently active emulator is on the exclusion list.
        The Guide button should NOT emit 'guide_button' when this returns True
        (the emulator handles it natively).
        """
        if self._game_backend is None:
            return False

        active = getattr(self._game_backend, "activeEmulatorName", "")
        if not isinstance(active, str) or not active:
            session = getattr(self._game_backend, "_active_session", None)
            if isinstance(session, dict):
                session_value = session.get("emulator_name", "")
                active = session_value if isinstance(session_value, str) else ""
        if not active:
            return False

        exclusion_list = self._app_backend.tvGuideExclusionList if self._app_backend else []
        return any(active.casefold() == e.casefold() for e in exclusion_list if isinstance(e, str))

    # ------------------------------------------------------------------
    # Raw event handler
    # ------------------------------------------------------------------

    @Slot(str, float)
    def _on_raw_event(self, code: str, value: float) -> None:
        event_type = self._classify_event_type(code)

        if event_type == "button":
            if value == 1.0:  # button pressed (not released)
                self._handle_button(code)
        elif event_type == "axis":
            self._handle_axis(code, value)

    def _classify_event_type(self, code: str) -> str:
        if code in self._BUTTON_MAP:
            return "button"
        if code in self._AXIS_MAP:
            return "axis"
        return "unknown"

    def _handle_button(self, code: str) -> None:
        nav_event = self._BUTTON_MAP.get(code)
        if nav_event is None:
            return
        if nav_event == "guide_button" and self.should_suppress_guide_button():
            return
        if nav_event == "guide_button":
            game_backend = self._game_backend
            if game_backend is not None and bool(getattr(game_backend, "isSessionActive", False)):
                request_pause = getattr(game_backend, "requestPause", None)
                if callable(request_pause):
                    request_pause()
                return
        self.navigationEvent.emit(nav_event)

    def _handle_axis(self, code: str, value: float) -> None:
        neg_event, pos_event = self._AXIS_MAP.get(code, (None, None))
        if neg_event is None:
            return

        now = time.monotonic()

        if value < -_DEAD_ZONE:
            direction = neg_event
        elif value > _DEAD_ZONE:
            direction = pos_event
        else:
            # Axis returned to center — reset repeat state
            self._axis_state.pop(code, None)
            return

        last_direction, last_fire = self._axis_state.get(code, ("", 0.0))

        if direction != last_direction or (now - last_fire) >= _REPEAT_INTERVAL:
            self._axis_state[code] = (direction, now)
            self.navigationEvent.emit(direction)
