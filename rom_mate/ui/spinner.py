from PySide6.QtCore import QEvent, Qt, QTimer
from PySide6.QtGui import QColor, QPainter, QPen, QPalette
from PySide6.QtWidgets import QWidget


class LoadingSpinnerWidget(QWidget):
    """Animated arc spinner overlay. Parent it to the widget it should cover."""

    def __init__(
        self,
        parent: QWidget,
        *,
        diameter: int = 48,
        stroke_width: int = 5,
    ) -> None:
        super().__init__(parent)
        self._stroke_width = stroke_width
        self._angle: int = 0
        self.setFixedSize(diameter, diameter)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating, True)
        self._timer = QTimer(self)
        self._timer.setInterval(16)  # ~60 fps
        self._timer.timeout.connect(self._tick)
        parent.installEventFilter(self)
        self.hide()

    # ------------------------------------------------------------------
    def showEvent(self, event) -> None:
        super().showEvent(event)
        self._reposition()
        self._timer.start()

    def hideEvent(self, event) -> None:
        super().hideEvent(event)
        self._timer.stop()

    def eventFilter(self, watched, event) -> bool:
        if watched is self.parentWidget() and event.type() == QEvent.Type.Resize:
            self._reposition()
        return super().eventFilter(watched, event)

    # ------------------------------------------------------------------
    def _reposition(self) -> None:
        parent = self.parentWidget()
        if parent is None:
            return
        x = (parent.width() - self.width()) // 2
        y = (parent.height() - self.height()) // 2
        self.move(x, y)

    def _tick(self) -> None:
        self._angle = (self._angle + 6) % 360
        self.update()

    def paintEvent(self, event) -> None:
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        color: QColor = self.palette().color(QPalette.ColorRole.PlaceholderText)
        pen = QPen(color, self._stroke_width, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap)
        painter.setPen(pen)
        margin = self._stroke_width // 2 + 1
        rect = self.rect().adjusted(margin, margin, -margin, -margin)
        # drawArc uses 1/16th-degree units; 270° arc, start angle rotates each tick
        painter.drawArc(rect, self._angle * 16, 270 * 16)
