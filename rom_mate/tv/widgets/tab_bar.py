from PySide6.QtCore import QRect, Qt, Signal
from PySide6.QtGui import QColor, QMouseEvent, QPaintEvent, QPainter
from PySide6.QtWidgets import QWidget

from rom_mate.tv.widgets import theme


class ViewTabBar(QWidget):
    tabChanged = Signal(object)

    _TAB_LABELS = ["Home", "Library", "Server"]

    def __init__(self, parent=None) -> None:
        super().__init__(parent)
        self._current_index = 0
        self.setFixedHeight(56)

    @property
    def current_index(self) -> int:
        return self._current_index

    def set_current_index(self, idx: int) -> None:
        clamped = max(0, min(len(self._TAB_LABELS) - 1, int(idx)))
        if clamped == self._current_index:
            return
        self._current_index = clamped
        self.update()
        self.tabChanged.emit(self._current_index)

    def _tab_rect(self, index: int) -> QRect:
        width = self.width()
        count = len(self._TAB_LABELS)
        tab_width = width // count if count > 0 else 0
        x = index * tab_width
        if index == count - 1:
            tab_width = width - x
        return QRect(x, 0, tab_width, self.height())

    def paintEvent(self, event: QPaintEvent) -> None:
        painter = QPainter(self)
        painter.fillRect(self.rect(), QColor(theme.PANEL))

        for index, label in enumerate(self._TAB_LABELS):
            rect = self._tab_rect(index)
            text_color = theme.TEXT_PRIMARY if index == self._current_index else theme.TEXT_SECONDARY
            painter.setPen(QColor(text_color))
            painter.drawText(rect, Qt.AlignmentFlag.AlignHCenter | Qt.AlignmentFlag.AlignVCenter, label)

            if index == self._current_index:
                underline = QRect(rect.left(), rect.bottom() - 2, rect.width(), 3)
                painter.fillRect(underline, QColor(theme.ACCENT))

    def mousePressEvent(self, event: QMouseEvent) -> None:
        if event.button() != Qt.MouseButton.LeftButton:
            super().mousePressEvent(event)
            return

        for index in range(len(self._TAB_LABELS)):
            if self._tab_rect(index).contains(event.position().toPoint()):
                self.set_current_index(index)
                return

        super().mousePressEvent(event)
