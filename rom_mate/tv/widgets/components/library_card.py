from __future__ import annotations

from PySide6.QtCore import QRect, QSize, Qt, Signal
from PySide6.QtGui import QColor, QPainter, QPaintEvent, QPixmap
from PySide6.QtWidgets import QWidget

from rom_mate.tv.widgets import theme
from rom_mate.ui.theme import themed_svg_pixmap


class LibraryCard(QWidget):
    selected = Signal(object)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._game: dict = {}
        self._title = ""
        self._platform = ""
        self._pixmap: QPixmap | None = None
        self._is_favorite = False
        self._has_saves = False
        self._title_height = 34
        self.resize(200, 300)
        self.setFocusPolicy(Qt.FocusPolicy.NoFocus)

    def set_game(self, game_dict: dict) -> None:
        self._game = game_dict or {}
        self._title = str(self._game.get("name") or self._game.get("title") or "")
        self._platform = str(self._game.get("platform") or "")
        self._is_favorite = self._game.get("is_favorite") == "true"
        self._has_saves = self._game.get("has_saves") == "true"
        self._pixmap = None
        self.update()
        self.setVisible(bool(self._game))

    def set_pixmap(self, pixmap: QPixmap | None) -> None:
        # Safety check: verify widget is still part of the widget tree before calling update()
        if self.parent() is None:
            # Widget has been orphaned from the tree, don't try to update it
            return
        
        if pixmap is None or pixmap.isNull():
            self._pixmap = None
        else:
            self._pixmap = pixmap
        self.update()

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        rect = self.rect().adjusted(1, 1, -1, -1)
        inset = 2
        card_rect = rect.adjusted(inset, inset, -inset, -inset)
        radius = 8
        border_width = 1
        border_color = QColor(theme.BORDER_INACTIVE)

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
            target_rect = QRect(pix_x, pix_y, scaled.width(), scaled.height())
            source_rect = QRect(0, 0, scaled.width(), scaled.height())
            painter.drawPixmap(target_rect, scaled, source_rect)

        if self._is_favorite:
            pix = themed_svg_pixmap("svg/favorite", QColor(theme.ACCENT), size=QSize(20, 20))
            painter.drawPixmap(cover_rect.right() - 24, cover_rect.top() + 8, pix)
            painter.setPen(Qt.PenStyle.NoPen)
            painter.setBrush(Qt.BrushStyle.NoBrush)

        if self._has_saves:
            cloud_rect = QRect(cover_rect.left() + 6, cover_rect.bottom() - 24, 20, 20)
            painter.setBrush(QColor(30, 31, 41, 220))
            painter.setPen(QColor(theme.PURPLE))
            painter.drawRoundedRect(cloud_rect, 10, 10)
            painter.drawText(cloud_rect, Qt.AlignmentFlag.AlignCenter, "☁")

        strip_rect = QRect(
            card_rect.left() + border_width,
            card_rect.bottom() - self._title_height - border_width + 1,
            card_rect.width() - (border_width * 2),
            self._title_height,
        )
        painter.fillRect(strip_rect, QColor(0, 0, 0, 160))

        text_font = painter.font()
        text_font.setPixelSize(12)
        painter.setFont(text_font)
        painter.setPen(QColor(theme.TEXT_PRIMARY))
        title_rect = strip_rect.adjusted(8, 0, -8, -13)
        painter.drawText(
            title_rect,
            Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft,
            self._title,
        )

        if self._platform:
            chip_font = painter.font()
            chip_font.setPixelSize(10)
            painter.setFont(chip_font)
            chip_text = self._platform
            metrics = painter.fontMetrics()
            chip_width = min(strip_rect.width() - 16, metrics.horizontalAdvance(chip_text) + 12)
            chip_rect = QRect(strip_rect.left() + 8, strip_rect.bottom() - 16, chip_width, 14)
            painter.setBrush(QColor(theme.TERTIARY))
            painter.setPen(Qt.PenStyle.NoPen)
            painter.drawRoundedRect(chip_rect, 6, 6)
            painter.setPen(QColor(theme.TEXT_PRIMARY))
            painter.drawText(chip_rect.adjusted(4, 0, -4, 0), Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft, chip_text)

        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.setPen(border_color)
        painter.drawRoundedRect(card_rect, radius, radius)
