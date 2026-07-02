from __future__ import annotations

from PySide6.QtCore import QRect, Qt, Signal
from PySide6.QtGui import QColor, QPainter, QPaintEvent, QPixmap
from PySide6.QtWidgets import QWidget

from grid_launcher.tv.widgets import theme
from grid_launcher.tv.widgets.cover_loader import CoverLoader


class PlatformCard(QWidget):
    selected = Signal(object)

    def __init__(self, cover_loader: CoverLoader, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._cover_loader = cover_loader
        self._platform: dict = {}
        self._name = ""
        self._pixmap: QPixmap | None = None
        self._title_height = 36
        self._focused = False
        self.setFixedSize(280, 190)
        self.setFocusPolicy(Qt.FocusPolicy.NoFocus)

    def set_platform(self, platform_dict: dict) -> None:
        self._platform = platform_dict or {}
        self._name = str(self._platform.get("name") or "")
        self._pixmap = None
        self.update()
        logo = (
            str(self._platform.get("local_logo_path") or "").strip()
            or str(self._platform.get("logo_url") or "").strip()
        )
        self._cover_loader.load_async(logo, self.set_pixmap)

    def set_pixmap(self, pixmap: QPixmap | None) -> None:
        # Safety check: verify widget is still part of the widget tree before calling update()
        # Calling update() on an orphaned widget causes it to become a top-level window
        if self.parent() is None:
            # Widget has been orphaned from the tree, don't try to update it
            return
        
        if pixmap is None or pixmap.isNull():
            self._pixmap = None
        else:
            self._pixmap = pixmap
        self.update()

    def set_focused(self, focused: bool) -> None:
        if self._focused != focused:
            self._focused = focused
            self.update()

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        rect = self.rect().adjusted(1, 1, -1, -1)
        focused = self._focused
        inset = 0 if focused else 2
        card_rect = rect.adjusted(inset, inset, -inset, -inset)
        radius = 8
        border_width = 2 if focused else 1
        border_color = QColor(theme.ACCENT if focused else theme.BORDER_INACTIVE)

        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QColor(theme.PANEL))
        painter.drawRoundedRect(card_rect, radius, radius)

        image_rect = QRect(
            card_rect.left() + border_width,
            card_rect.top() + border_width,
            card_rect.width() - (border_width * 2),
            card_rect.height() - self._title_height - border_width,
        )
        painter.fillRect(image_rect, QColor(theme.BG))

        if self._pixmap is not None and not self._pixmap.isNull() and image_rect.width() > 0 and image_rect.height() > 0:
            scaled = self._pixmap.scaled(
                image_rect.size(),
                Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            )
            pix_x = image_rect.left() + (image_rect.width() - scaled.width()) // 2
            pix_y = image_rect.top() + (image_rect.height() - scaled.height()) // 2
            target_rect = QRect(pix_x, pix_y, scaled.width(), scaled.height())
            source_rect = QRect(0, 0, scaled.width(), scaled.height())
            painter.drawPixmap(target_rect, scaled, source_rect)

        title_rect = QRect(
            card_rect.left() + border_width,
            card_rect.bottom() - self._title_height - border_width + 1,
            card_rect.width() - (border_width * 2),
            self._title_height,
        )
        painter.fillRect(title_rect, QColor(0, 0, 0, 160))
        painter.setPen(QColor(theme.TEXT_PRIMARY))
        font = painter.font()
        font.setPixelSize(13)
        painter.setFont(font)
        painter.drawText(title_rect.adjusted(8, 0, -8, 0), Qt.AlignmentFlag.AlignCenter, self._name)

        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.setPen(border_color)
        painter.drawRoundedRect(card_rect, radius, radius)
