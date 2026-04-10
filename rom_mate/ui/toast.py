from __future__ import annotations

from PySide6.QtCore import QEvent, QObject, QTimer, Qt
from PySide6.QtWidgets import QLabel, QVBoxLayout, QWidget


class ToastWidget(QWidget):
    """Small transient message widget that overlays a parent widget."""

    def __init__(
        self,
        parent: QWidget,
        *,
        duration_ms: int = 2400,
        bottom_margin: int = 24,
        horizontal_padding: int = 14,
        vertical_padding: int = 10,
        max_width: int = 480,
    ) -> None:
        super().__init__(parent)
        self._duration_ms = max(250, int(duration_ms))
        self._bottom_margin = max(0, int(bottom_margin))
        self._max_width = max(120, int(max_width))

        self.setObjectName("toastWidget")
        self.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating, True)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        self.setFocusPolicy(Qt.FocusPolicy.NoFocus)

        self._message_label = QLabel("")
        self._message_label.setObjectName("toastMessage")
        self._message_label.setWordWrap(True)
        self._message_label.setAlignment(Qt.AlignmentFlag.AlignCenter)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(horizontal_padding, vertical_padding, horizontal_padding, vertical_padding)
        layout.addWidget(self._message_label)

        self.setStyleSheet(
            """
            QWidget#toastWidget {
                background-color: rgba(28, 30, 40, 220);
                border: 1px solid rgba(255, 255, 255, 45);
                border-radius: 9px;
            }
            QLabel#toastMessage {
                color: #ffffff;
                font-size: 13px;
                font-weight: 600;
            }
            """
        )

        self._dismiss_timer = QTimer(self)
        self._dismiss_timer.setSingleShot(True)
        self._dismiss_timer.timeout.connect(self.hide)

        parent.installEventFilter(self)
        self.hide()

    def show_message(self, message: str, *, duration_ms: int | None = None) -> None:
        text = message.strip() if isinstance(message, str) else ""
        if not text:
            return

        self._message_label.setText(text)
        self._message_label.setMaximumWidth(self._max_width)
        self.adjustSize()
        self._reposition()
        self.show()
        self.raise_()

        timeout_ms = self._duration_ms if duration_ms is None else max(250, int(duration_ms))
        self._dismiss_timer.start(timeout_ms)

    def hide_now(self) -> None:
        self._dismiss_timer.stop()
        self.hide()

    def eventFilter(self, watched: QObject, event: QEvent) -> bool:
        if watched is self.parentWidget() and event.type() in {
            QEvent.Type.Resize,
            QEvent.Type.Move,
            QEvent.Type.Show,
            QEvent.Type.WindowStateChange,
        }:
            self._reposition()
        return super().eventFilter(watched, event)

    def _reposition(self) -> None:
        parent = self.parentWidget()
        if parent is None:
            return
        target_x = max(0, (parent.width() - self.width()) // 2)
        target_y = max(0, parent.height() - self.height() - self._bottom_margin)
        self.move(target_x, target_y)


def show_toast(parent: QWidget, message: str, *, duration_ms: int = 2400) -> ToastWidget:
    """Show a reusable toast attached to the parent widget and return it."""

    toast = getattr(parent, "_toast_widget", None)
    if not isinstance(toast, ToastWidget):
        toast = ToastWidget(parent)
        setattr(parent, "_toast_widget", toast)
    toast.show_message(message, duration_ms=duration_ms)
    return toast


__all__ = ["ToastWidget", "show_toast"]
