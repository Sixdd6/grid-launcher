from __future__ import annotations

from PySide6.QtCore import QRect, Qt, Signal
from PySide6.QtGui import QColor, QFontMetrics, QKeyEvent, QPainter, QPainterPath, QPaintEvent, QPen, QPixmap
from PySide6.QtWidgets import QWidget

from rom_mate.tv.widgets import theme


class HomeCard(QWidget):
    selected = Signal(object)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._game: dict = {}
        self._pixmap: QPixmap | None = None
        self._title = ""
        self._platform = ""
        self._year = ""
        self._genre = ""
        self._installed = False
        self.setFixedSize(780, 260)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)

    def set_game(self, game_dict: dict) -> None:
        self._game = game_dict or {}
        self._title = str(self._game.get("name") or self._game.get("title") or "")
        self._platform = str(self._game.get("platform") or "")
        self._year = str(self._game.get("release_year") or "")

        raw_genres = self._game.get("genres")
        if isinstance(raw_genres, str):
            genre = raw_genres.split(",", 1)[0].strip()
        elif isinstance(raw_genres, list) and raw_genres:
            genre = str(raw_genres[0]).strip()
        else:
            genre = ""
        self._genre = genre

        self._installed = bool(self._game.get("local_path"))
        self._pixmap = None
        self.update()

    def set_pixmap(self, pixmap: QPixmap | None) -> None:
        if pixmap is None or pixmap.isNull():
            self._pixmap = None
        else:
            self._pixmap = pixmap
        self.update()

    def focusInEvent(self, event) -> None:
        super().focusInEvent(event)
        self.update()

    def focusOutEvent(self, event) -> None:
        super().focusOutEvent(event)
        self.update()

    def keyPressEvent(self, event: QKeyEvent) -> None:
        if event.key() in (Qt.Key.Key_Return, Qt.Key.Key_Enter):
            self.selected.emit(self._game)
            event.accept()
            return
        super().keyPressEvent(event)

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        card_rect = self.rect().adjusted(1, 1, -1, -1)
        focused = self.hasFocus()
        border_width = 6 if focused else 2
        border_color = QColor(theme.ACCENT if focused else theme.BORDER_INACTIVE)
        radius = 16

        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(theme.PANEL))
        painter.drawRoundedRect(card_rect, radius, radius)

        cover_rect = QRect(card_rect.left(), card_rect.top(), 240, card_rect.height())
        cover_path = QPainterPath()
        left = float(cover_rect.left())
        top = float(cover_rect.top())
        right = float(cover_rect.right() + 1)
        bottom = float(cover_rect.bottom() + 1)
        curve = float(radius)
        cover_path.moveTo(left + curve, top)
        cover_path.lineTo(right, top)
        cover_path.lineTo(right, bottom)
        cover_path.lineTo(left + curve, bottom)
        cover_path.quadTo(left, bottom, left, bottom - curve)
        cover_path.lineTo(left, top + curve)
        cover_path.quadTo(left, top, left + curve, top)
        cover_path.closeSubpath()
        painter.fillPath(cover_path, QColor(theme.TERTIARY))

        if self._pixmap is not None and not self._pixmap.isNull():
            margin = 12
            inner_rect = cover_rect.adjusted(margin, margin, -margin, -margin)
            if inner_rect.width() > 0 and inner_rect.height() > 0:
                scaled = self._pixmap.scaled(
                    inner_rect.size(),
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.SmoothTransformation,
                )
                pix_x = inner_rect.left() + (inner_rect.width() - scaled.width()) // 2
                pix_y = inner_rect.top() + (inner_rect.height() - scaled.height()) // 2
                painter.drawPixmap(QRect(pix_x, pix_y, scaled.width(), scaled.height()), scaled)

        content_x = card_rect.left() + 240 + 20
        content_right = self.width() - 20
        content_width = max(0, content_right - content_x)

        title_font = painter.font()
        title_font.setPixelSize(26)
        title_font.setBold(True)
        painter.setFont(title_font)
        painter.setPen(QColor(theme.TEXT_PRIMARY))
        title_metrics = QFontMetrics(title_font)
        title_text = title_metrics.elidedText(self._title, Qt.TextElideMode.ElideRight, max(0, content_width))
        title_rect = QRect(content_x, card_rect.top() + 24, max(0, content_width), 40)
        painter.drawText(title_rect, Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter, title_text)

        badge_font = painter.font()
        badge_font.setPixelSize(20)
        badge_font.setBold(True)
        painter.setFont(badge_font)
        badge_metrics = QFontMetrics(badge_font)
        badge_text = self._platform
        max_badge_text = max(0, content_width - 12)
        badge_text = badge_metrics.elidedText(badge_text, Qt.TextElideMode.ElideRight, max_badge_text)
        badge_width = min(content_width, badge_metrics.horizontalAdvance(badge_text) + 12)
        badge_height = 40
        badge_rect = QRect(content_x, card_rect.top() + 96, max(0, badge_width), badge_height)
        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(theme.TERTIARY))
        painter.drawRoundedRect(badge_rect, badge_height / 2.0, badge_height / 2.0)
        painter.setPen(QColor(theme.PURPLE))
        painter.drawText(badge_rect, Qt.AlignmentFlag.AlignCenter, badge_text)

        meta_font = painter.font()
        meta_font.setPixelSize(22)
        meta_font.setBold(False)
        painter.setFont(meta_font)
        painter.setPen(QColor(theme.TEXT_SECONDARY))
        meta_metrics = QFontMetrics(meta_font)
        parts = [part for part in (self._year, self._genre) if part]
        meta_text = " • ".join(parts)
        meta_text = meta_metrics.elidedText(meta_text, Qt.TextElideMode.ElideRight, max(0, content_width))
        meta_rect = QRect(content_x, card_rect.top() + 156, max(0, content_width), 32)
        painter.drawText(meta_rect, Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter, meta_text)

        status_font = painter.font()
        status_font.setPixelSize(20)
        status_font.setBold(True)
        painter.setFont(status_font)
        painter.setPen(QColor(theme.SUCCESS if self._installed else theme.TEXT_SECONDARY))
        status_text = "Installed" if self._installed else "Not Installed"
        status_rect = QRect(content_x, card_rect.top() + 208, max(0, content_width), 28)
        painter.drawText(status_rect, Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter, status_text)

        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.setPen(QPen(border_color, border_width))
        painter.drawRoundedRect(card_rect, radius, radius)
