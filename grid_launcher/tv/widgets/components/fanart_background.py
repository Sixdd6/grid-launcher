from __future__ import annotations

from PySide6.QtCore import Property, QEasingCurve, QPropertyAnimation, QTimer, Qt
from PySide6.QtGui import QColor, QPainter, QPaintEvent, QPixmap
from PySide6.QtWidgets import (
    QGraphicsBlurEffect,
    QGraphicsPixmapItem,
    QGraphicsScene,
    QWidget,
)

from grid_launcher.tv.widgets import theme
from grid_launcher.tv.widgets.cover_loader import CoverLoader


def _blur_pixmap(pixmap: QPixmap, radius: float = 4.0) -> QPixmap:
    if pixmap.isNull():
        return pixmap

    scene = QGraphicsScene()
    item = QGraphicsPixmapItem(pixmap)
    blur = QGraphicsBlurEffect()
    blur.setBlurRadius(radius)
    blur.setBlurHints(QGraphicsBlurEffect.BlurHint.PerformanceHint)
    item.setGraphicsEffect(blur)
    scene.addItem(item)

    result = QPixmap(pixmap.size())
    result.fill(Qt.GlobalColor.transparent)

    painter = QPainter(result)
    scene.render(painter)
    painter.end()
    return result


class FanartBackground(QWidget):
    def __init__(self, cover_loader: CoverLoader, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._cover_loader = cover_loader
        self._urls: list[str] = []
        self._current_index = 0

        self._front_pixmap: QPixmap | None = None
        self._back_pixmap: QPixmap | None = None
        self._back_opacity_val: float = 0.0
        self._anim: QPropertyAnimation | None = None

        self.setAttribute(Qt.WidgetAttribute.WA_OpaquePaintEvent, False)
        self.setAutoFillBackground(False)

        self._cycle_timer = QTimer(self)
        self._cycle_timer.setInterval(5000)
        self._cycle_timer.timeout.connect(self._advance_url)

    def _get_back_opacity(self) -> float:
        return self._back_opacity_val

    def _set_back_opacity(self, value: float) -> None:
        self._back_opacity_val = float(value)
        self.update()

    back_opacity = Property(float, _get_back_opacity, _set_back_opacity)

    def set_urls(self, url_list: list[str]) -> None:
        self._urls = [str(u).strip() for u in (url_list or []) if str(u).strip()]
        self._current_index = 0
        self._back_pixmap = None
        self._back_opacity_val = 0.0
        if self._anim and self._anim.state() == QPropertyAnimation.State.Running:
            self._anim.stop()
        self._cycle_timer.stop()
        if not self._urls:
            self.update()
            return
        self._cover_loader.load_async(self._urls[0], self._on_pixmap_loaded)
        if len(self._urls) > 1:
            self._cycle_timer.start()

    def _advance_url(self) -> None:
        if not self._urls:
            self._cycle_timer.stop()
            return
        self._current_index = (self._current_index + 1) % len(self._urls)
        self._cover_loader.load_async(self._urls[self._current_index], self._on_pixmap_loaded)

    def _on_pixmap_loaded(self, pixmap: QPixmap | None) -> None:
        # Safety check: verify widget is still part of the widget tree before calling update()
        if self.parent() is None:
            # Widget has been orphaned from the tree, don't try to update it
            return
        
        if pixmap is None or pixmap.isNull():
            return
        blurred = _blur_pixmap(pixmap)
        if self._front_pixmap is None:
            self._front_pixmap = blurred
            self.update()
            return
        self._back_pixmap = blurred
        self._back_opacity_val = 0.0
        if self._anim and self._anim.state() == QPropertyAnimation.State.Running:
            self._anim.stop()
        anim = QPropertyAnimation(self, b"back_opacity", self)
        anim.setDuration(1000)
        anim.setStartValue(0.0)
        anim.setEndValue(1.0)
        anim.setEasingCurve(QEasingCurve.Type.InOutQuad)
        anim.finished.connect(self._swap_pixmaps)
        self._anim = anim
        self._anim.start()

    def _swap_pixmaps(self) -> None:
        if self._back_pixmap is not None:
            self._front_pixmap = self._back_pixmap
        self._back_pixmap = None
        self._back_opacity_val = 0.0
        self.update()

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        rect = self.rect()

        painter.fillRect(rect, QColor(theme.BG))

        if self._front_pixmap and not self._front_pixmap.isNull():
            scaled = self._front_pixmap.scaled(
                rect.size(),
                Qt.AspectRatioMode.KeepAspectRatioByExpanding,
                Qt.TransformationMode.SmoothTransformation,
            )
            x = (rect.width() - scaled.width()) // 2
            y = (rect.height() - scaled.height()) // 2
            painter.setOpacity(1.0)
            painter.drawPixmap(x, y, scaled)

        if self._back_pixmap and not self._back_pixmap.isNull() and self._back_opacity_val > 0.0:
            scaled = self._back_pixmap.scaled(
                rect.size(),
                Qt.AspectRatioMode.KeepAspectRatioByExpanding,
                Qt.TransformationMode.SmoothTransformation,
            )
            x = (rect.width() - scaled.width()) // 2
            y = (rect.height() - scaled.height()) // 2
            painter.setOpacity(self._back_opacity_val)
            painter.drawPixmap(x, y, scaled)

        painter.setOpacity(1.0)
        painter.fillRect(rect, QColor(0, 0, 0, 178))
