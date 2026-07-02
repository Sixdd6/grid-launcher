from __future__ import annotations

import logging
from PySide6.QtCore import QTimer, Qt
from PySide6.QtGui import QColor, QFont, QPainter, QPaintEvent
from PySide6.QtWidgets import QWidget

from grid_launcher.tv.widgets import theme
from grid_launcher.tv.widgets.components.controls_bar import ControlHint

logger = logging.getLogger(__name__)


class PauseWindow(QWidget):
    CONTROL_HINTS: list[ControlHint] = [
        ControlHint("Select", "input_DPAD-U", "↑↓"),
        ControlHint("Confirm", "input_BTN-D", "Enter"),
        ControlHint("Resume", "input_BTN-R", "Backspace"),
    ]
    def __init__(self, pause_backend, parent=None):
        logger.debug(f"PauseWindow.__init__() START: parent={parent.__class__.__name__ if parent else None}")
        super().__init__(parent)
        self._pause_backend = pause_backend
        self._current_index = 0

        # CRITICAL: Hide BEFORE setting window flags to prevent Qt from briefly showing the window
        # on Windows when flags are changed
        logger.debug(f"PauseWindow: pre-hiding before setWindowFlags")
        self.hide()
        
        logger.debug(f"PauseWindow: calling setWindowFlags")
        print(f"[PAUSE] Before setWindowFlags: visible={self.isVisible()}, parent={self.parent()}")
        self.setWindowFlags(
            Qt.WindowType.Window
            | Qt.WindowType.FramelessWindowHint
            | Qt.WindowType.WindowStaysOnTopHint
            | Qt.WindowType.Tool
        )
        print(f"[PAUSE] After setWindowFlags: visible={self.isVisible()}, parent={self.parent()}")
        logger.debug(f"PauseWindow: setWindowFlags complete, setting transparent attribute")
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)

        logger.debug(f"PauseWindow: connecting signals")
        self._pause_backend.visibleChanged.connect(self._on_visible_changed)
        self._pause_backend.gameTitleChanged.connect(self.update)
        self._pause_backend.emulatorNameChanged.connect(self.update)

        # Final hide to ensure widget stays hidden
        logger.debug(f"PauseWindow: final hide() to ensure hidden")
        print(f"[PAUSE] Before final hide(): visible={self.isVisible()}, parent={self.parent()}")
        self.hide()
        print(f"[PAUSE] After final hide(): visible={self.isVisible()}, parent={self.parent()}")
        logger.debug(f"PauseWindow.__init__() END")

    def _on_visible_changed(self) -> None:
        # Safety check: only show if a game session is actually active
        # This prevents stale visible state from showing the pause window during normal browsing
        game_backend = getattr(self._pause_backend, "_game_backend", None)
        is_session_active = game_backend is not None and bool(getattr(game_backend, "isSessionActive", False))
        
        if self._pause_backend.visible and is_session_active:
            self._current_index = 0
            self.show()
            self.activateWindow()
            self.raise_()
            self.update()
            return
        
        # Hide if not visible or no active session
        self.hide()
        logger.debug(f"PauseWindow: hiding (visible={self._pause_backend.visible}, session_active={is_session_active})")
        self.hide()

    def show_on_screen(self, screen) -> None:
        if screen is None:
            return
        logger.info(f"PauseWindow.show_on_screen() called")
        self.setGeometry(screen.geometry())
        self.show()

    def show(self) -> None:
        logger.info(f"PauseWindow.show() called")
        import traceback
        logger.debug(f"Call stack: {traceback.format_stack()[-3]}")
        super().show()

    def hide(self) -> None:
        logger.info(f"PauseWindow.hide() called")
        super().hide()

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing, True)

        painter.fillRect(self.rect(), QColor(0, 0, 0, 200))

        panel_w = 400
        panel_h = 280
        panel_x = int((self.width() - panel_w) / 2)
        panel_y = int((self.height() - panel_h) / 2)

        panel_rect = self.rect().adjusted(panel_x, panel_y, -(self.width() - panel_x - panel_w), -(self.height() - panel_y - panel_h))
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(theme.PANEL))
        painter.drawRoundedRect(panel_rect, 14, 14)

        title_rect = panel_rect.adjusted(24, 34, -24, -(panel_h - 72))
        painter.setPen(QColor(theme.TEXT_SECONDARY))
        title_font = QFont()
        title_font.setPixelSize(16)
        painter.setFont(title_font)
        painter.drawText(title_rect, int(Qt.AlignmentFlag.AlignHCenter | Qt.AlignmentFlag.AlignVCenter), self._pause_backend.gameTitle)

        emulator_rect = panel_rect.adjusted(24, 64, -24, -(panel_h - 96))
        emulator_font = QFont()
        emulator_font.setPixelSize(13)
        painter.setFont(emulator_font)
        painter.drawText(emulator_rect, int(Qt.AlignmentFlag.AlignHCenter | Qt.AlignmentFlag.AlignVCenter), self._pause_backend.emulatorName)

        button_w = 240
        button_h = 44
        gap = 12
        total_h = (button_h * 2) + gap
        buttons_top = panel_y + int((panel_h - total_h) / 2) + 44
        button_x = int((self.width() - button_w) / 2)

        actions = self._pause_backend.actions
        for idx, action in enumerate(actions[:2]):
            y = buttons_top + idx * (button_h + gap)
            btn_rect = self.rect().adjusted(button_x, y, -(self.width() - button_x - button_w), -(self.height() - y - button_h))

            focused = idx == self._current_index
            if focused:
                painter.setBrush(QColor(theme.ACCENT))
                text_color = QColor(theme.PANEL)
            else:
                painter.setBrush(QColor(theme.TERTIARY))
                text_color = QColor(theme.TEXT_PRIMARY)

            painter.setPen(Qt.PenStyle.NoPen)
            painter.drawRoundedRect(btn_rect, 8, 8)

            text_font = QFont()
            text_font.setPixelSize(15)
            text_font.setBold(focused)
            painter.setFont(text_font)
            painter.setPen(text_color)
            painter.drawText(btn_rect, int(Qt.AlignmentFlag.AlignCenter), action)

    def handle_nav(self, direction: str) -> None:
        if direction == "up":
            self._current_index = max(0, self._current_index - 1)
            self.update()
            return
        if direction == "down":
            self._current_index = min(1, self._current_index + 1)
            self.update()
            return
        if direction == "confirm":
            self._trigger_action()
            return
        if direction == "back":
            self._pause_backend.resumeGame()

    def _trigger_action(self) -> None:
        if self._current_index == 0:
            QTimer.singleShot(150, self._pause_backend.resumeGame)
            return
        if self._current_index == 1:
            self._pause_backend.quitGame()

    def keyPressEvent(self, event) -> None:
        key = event.key()
        if key == Qt.Key.Key_Up:
            self.handle_nav("up")
            return
        if key == Qt.Key.Key_Down:
            self.handle_nav("down")
            return
        if key in (Qt.Key.Key_Return, Qt.Key.Key_Enter):
            self.handle_nav("confirm")
            return
        if key in (Qt.Key.Key_Escape, Qt.Key.Key_Backspace):
            self.handle_nav("back")
            return
        super().keyPressEvent(event)