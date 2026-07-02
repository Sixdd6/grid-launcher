from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QPaintEvent, QPainter
from PySide6.QtWidgets import QSizePolicy, QWidget

from grid_launcher.tv.widgets import theme


class InstallProgressBar(QWidget):
    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._progress = 0.0
        self.setFixedHeight(6)
        self.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)

    def set_progress(self, value: float) -> None:
        clamped = max(0.0, min(1.0, float(value or 0.0)))
        if abs(clamped - self._progress) < 0.0001:
            return
        self._progress = clamped
        self.update()

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        rect = self.rect()
        if rect.width() <= 0 or rect.height() <= 0:
            return

        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(theme.BORDER_INACTIVE))
        painter.drawRoundedRect(rect, 3, 3)

        fill_width = int(rect.width() * self._progress)
        if fill_width <= 0:
            return
        fill_rect = rect.adjusted(0, 0, -(rect.width() - fill_width), 0)
        painter.setBrush(QColor(theme.ACCENT))
        painter.drawRoundedRect(fill_rect, 3, 3)
