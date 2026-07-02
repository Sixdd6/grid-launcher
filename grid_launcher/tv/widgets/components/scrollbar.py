from __future__ import annotations

from PySide6.QtCore import QTimer, Qt
from PySide6.QtGui import QColor, QPainter, QPaintEvent
from PySide6.QtWidgets import QWidget

from grid_launcher.tv.widgets import theme


class TvScrollBar(QWidget):
    def __init__(
        self,
        orientation: Qt.Orientation = Qt.Orientation.Vertical,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self._orientation = orientation
        self._min_val = 0
        self._max_val = 0
        self._value = 0
        self._recently_scrolled = False
        self._inactive_timer = QTimer(self)
        self._inactive_timer.setSingleShot(True)
        self._inactive_timer.setInterval(700)
        self._inactive_timer.timeout.connect(self._set_inactive)

        if self._orientation == Qt.Orientation.Vertical:
            self.setFixedWidth(6)
        else:
            self.setFixedHeight(6)

    def set_range(self, min_val: int, max_val: int) -> None:
        self._min_val = int(min_val)
        self._max_val = int(max_val)
        self.update()

    def set_value(self, val: int) -> None:
        self._value = int(val)
        self._recently_scrolled = True
        self._inactive_timer.start()
        self.update()

    def _set_inactive(self) -> None:
        self._recently_scrolled = False
        self.update()

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        track_rect = self.rect().adjusted(0, 0, -1, -1)
        radius = min(track_rect.width(), track_rect.height()) / 2
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(theme.BORDER_INACTIVE))
        painter.drawRoundedRect(track_rect, radius, radius)

        if self._orientation != Qt.Orientation.Vertical:
            return

        if self._max_val <= self._min_val:
            thumb_rect = track_rect
        else:
            total = self._max_val - self._min_val
            thumb_height = max(24, int(track_rect.height() * 0.22))
            available = max(0, track_rect.height() - thumb_height)
            clamped = min(max(self._value, self._min_val), self._max_val)
            ratio = (clamped - self._min_val) / float(total)
            y_pos = track_rect.top() + int(available * ratio)
            thumb_rect = track_rect.adjusted(0, y_pos - track_rect.top(), 0, -(track_rect.bottom() - (y_pos + thumb_height - 1)))

        thumb_color = theme.ACCENT if self._recently_scrolled else theme.TEXT_SECONDARY
        painter.setBrush(QColor(thumb_color))
        painter.drawRoundedRect(thumb_rect, radius, radius)