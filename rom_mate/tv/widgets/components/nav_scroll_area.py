from __future__ import annotations

from PySide6.QtCore import QEvent, Qt
from PySide6.QtWidgets import QScrollArea


# Keys that drive TV-mode navigation — scroll areas must not consume these.
_NAV_KEYS = frozenset(
    {
        Qt.Key.Key_Up,
        Qt.Key.Key_Down,
        Qt.Key.Key_Left,
        Qt.Key.Key_Right,
        Qt.Key.Key_Return,
        Qt.Key.Key_Enter,
        Qt.Key.Key_Escape,
        Qt.Key.Key_Backspace,
        Qt.Key.Key_End,
        Qt.Key.Key_PageDown,
    }
)


class NavScrollArea(QScrollArea):
    """QScrollArea that does not consume TV-mode navigation key events.

    ``QAbstractScrollArea`` installs its own event filter on its viewport
    and handles arrow keys there (scrolling the content).  This subclass
    overrides that filter so navigation keys are ignored and propagate up
    the widget hierarchy to ``TVWindow.keyPressEvent``.
    """

    def eventFilter(self, watched: object, event: object) -> bool:
        if (
            watched is self.viewport()
            and isinstance(event, QEvent)
            and event.type() == QEvent.Type.KeyPress
            and event.key() in _NAV_KEYS  # type: ignore[attr-defined]
        ):
            event.ignore()
            return False  # don't consume; let viewport widget receive it
        return super().eventFilter(watched, event)

    def keyPressEvent(self, event) -> None:  # type: ignore[override]
        if event.key() in _NAV_KEYS:
            event.ignore()  # propagate to parent (ultimately TVWindow)
        else:
            super().keyPressEvent(event)
