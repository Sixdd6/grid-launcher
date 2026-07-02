from __future__ import annotations

from PySide6.QtCore import Property, QEasingCurve, QPropertyAnimation, QRect, Qt, Signal
from PySide6.QtGui import QColor, QPainter, QPaintEvent, QPixmap
from PySide6.QtWidgets import QWidget

from grid_launcher.tv.widgets import theme


class GameCard(QWidget):
    selected = Signal(object)
    scale = Property(float, lambda self: self._scale_val, lambda self, value: self._set_scale(value))

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._game: dict = {}
        self._title = ""
        self._pixmap: QPixmap | None = None
        self._title_height = 36
        self._focus_scale = 1.05
        self._scale_val = 1.0
        self._scale_anim: QPropertyAnimation | None = None
        self._focused = False
        self.setFixedSize(296, 480)
        self.setFocusPolicy(Qt.FocusPolicy.NoFocus)

    def _set_scale(self, value: float) -> None:
        self._scale_val = float(value)
        self.update()

    def _animate_scale(self, target: float, easing: QEasingCurve.Type) -> None:
        if self._scale_anim is not None and self._scale_anim.state() == QPropertyAnimation.State.Running:
            self._scale_anim.stop()
        anim = QPropertyAnimation(self, b"scale", self)
        anim.setDuration(120)
        anim.setStartValue(self._scale_val)
        anim.setEndValue(float(target))
        anim.setEasingCurve(easing)
        self._scale_anim = anim
        self._scale_anim.start()

    def set_game(self, game_dict: dict) -> None:
        self._game = game_dict or {}
        self._title = str(self._game.get("name") or self._game.get("title") or "")
        self._pixmap = None
        self.update()

    def set_pixmap(self, pixmap: QPixmap | None) -> None:
        # Guard: calling update() on an orphaned widget causes it to become a top-level window
        try:
            if self.parent() is None:
                return
            if pixmap is None or pixmap.isNull():
                self._pixmap = None
            else:
                self._pixmap = pixmap
            self.update()
        except RuntimeError:
            pass

    def set_focused(self, focused: bool) -> None:
        if self._focused != focused:
            self._focused = focused
            if focused:
                self._animate_scale(self._focus_scale, QEasingCurve.Type.OutQuad)
            else:
                self._animate_scale(1.0, QEasingCurve.Type.InQuad)

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        if self._scale_val != 1.0:
            cx = self.width() / 2.0
            cy = self.height() / 2.0
            painter.translate(cx, cy)
            painter.scale(self._scale_val, self._scale_val)
            painter.translate(-cx, -cy)

        rect = self.rect().adjusted(8, 12, -8, -12)
        focused = self._focused
        inset = 0 if focused else 2
        card_rect = rect.adjusted(inset, inset, -inset, -inset)
        radius = 8
        border_width = 2 if focused else 1
        border_color = QColor(theme.ACCENT if focused else theme.BORDER_INACTIVE)

        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(theme.PANEL))
        painter.drawRoundedRect(card_rect, radius, radius)

        cover_rect = QRect(
            card_rect.left() + border_width,
            card_rect.top() + border_width,
            card_rect.width() - (border_width * 2),
            card_rect.height() - self._title_height - border_width,
        )
        painter.fillRect(cover_rect, QColor(theme.BG))

        if self._pixmap is not None and not self._pixmap.isNull() and cover_rect.width() > 0 and cover_rect.height() > 0:
            scaled = self._pixmap.scaled(
                cover_rect.size(),
                Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            )
            pix_x = cover_rect.left() + (cover_rect.width() - scaled.width()) // 2
            pix_y = cover_rect.top() + (cover_rect.height() - scaled.height()) // 2
            painter.drawPixmap(pix_x, pix_y, scaled)

        strip_rect = QRect(
            card_rect.left() + border_width,
            card_rect.bottom() - self._title_height - border_width + 1,
            card_rect.width() - (border_width * 2),
            self._title_height,
        )
        painter.fillRect(strip_rect, QColor(0, 0, 0, 160))

        painter.setPen(QColor(theme.TEXT_PRIMARY))
        font = painter.font()
        font.setPixelSize(12)
        painter.setFont(font)
        text_rect = strip_rect.adjusted(8, 0, -8, 0)
        painter.drawText(
            text_rect,
            Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft,
            self._title,
        )

        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.setPen(border_color)
        painter.drawRoundedRect(card_rect, radius, radius)
