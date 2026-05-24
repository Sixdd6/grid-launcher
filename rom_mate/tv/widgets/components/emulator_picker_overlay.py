from __future__ import annotations

from collections.abc import Callable
from typing import Any

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QPaintEvent, QPainter
from PySide6.QtWidgets import QFrame, QLabel, QVBoxLayout, QWidget

from rom_mate.tv.widgets.components.nav_scroll_area import NavScrollArea

from rom_mate.tv.widgets import theme


class EmulatorPickerOverlay(QWidget):
    def __init__(self, app_backend: Any, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._app_backend = app_backend

        self._names: list[str] = []
        self._on_select: Callable[[str], None] | None = None
        self._cursor_index = 0
        self._row_widgets: list[QWidget] = []

        self.setVisible(False)
        self.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)

        self._panel = QFrame(self)
        self._panel.setFixedSize(520, 440)
        self._panel.setStyleSheet(
            "QFrame {"
            f"background: {theme.PANEL};"
            f"border: 1px solid {theme.BORDER_INACTIVE};"
            "border-radius: 12px;"
            "}"
        )

        panel_layout = QVBoxLayout(self._panel)
        panel_layout.setContentsMargins(16, 16, 16, 16)
        panel_layout.setSpacing(10)

        self._title = QLabel("Select Emulator to Exclude", self._panel)
        self._title.setStyleSheet(
            f"color: {theme.TEXT_PRIMARY}; font-size: 16px; font-weight: 700; border: none;"
        )
        panel_layout.addWidget(self._title)

        self._scroll = NavScrollArea(self._panel)
        self._scroll.setWidgetResizable(True)
        self._scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._scroll.setFrameShape(QFrame.Shape.NoFrame)
        self._scroll.setStyleSheet(
            "NavScrollArea { border: none; background: transparent; } "
            "NavScrollArea > QWidget > QWidget { border: none; background: transparent; }"
        )

        self._list_container = QWidget(self._scroll)
        self._list_container.setStyleSheet(f"background: {theme.PANEL}; border: none;")

        self._list_layout = QVBoxLayout(self._list_container)
        self._list_layout.setContentsMargins(0, 0, 0, 0)
        self._list_layout.setSpacing(4)
        self._list_layout.setAlignment(Qt.AlignmentFlag.AlignTop)

        self._scroll.setWidget(self._list_container)
        panel_layout.addWidget(self._scroll, 1)

        self.resize(self.parentWidget().size() if self.parentWidget() is not None else self.size())

    def show_picker(self, names: list, on_select) -> None:
        self._names = [str(name) for name in (names or [])]
        self._on_select = on_select if callable(on_select) else None
        self._cursor_index = 0
        self._rebuild_ui()
        self._app_backend.setUiOverlayActive(True)
        self.setVisible(True)
        self.raise_()

    def close_overlay(self) -> None:
        self.setVisible(False)
        self._app_backend.setUiOverlayActive(False)

    def handle_nav(self, direction: str) -> None:
        if not self.isVisible():
            return

        max_index = len(self._names)

        if direction == "up":
            self._cursor_index = max(0, self._cursor_index - 1)
            self._refresh_styles()
            self._scroll_to_cursor()
            return

        if direction == "down":
            self._cursor_index = min(max_index, self._cursor_index + 1)
            self._refresh_styles()
            self._scroll_to_cursor()
            return

        if direction == "confirm":
            if self._cursor_index == len(self._names):
                self.close_overlay()
                return

            if 0 <= self._cursor_index < len(self._names):
                if self._on_select is not None:
                    self._on_select(self._names[self._cursor_index])
                self.close_overlay()
            return

        if direction == "back":
            self.close_overlay()

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        parent = self.parentWidget()
        if parent is not None:
            self.resize(parent.size())
        panel_rect = self._panel.frameGeometry()
        panel_rect.moveCenter(self.rect().center())
        self._panel.move(panel_rect.topLeft())

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        painter.fillRect(self.rect(), QColor(0, 0, 0, 204))

    def _rebuild_ui(self) -> None:
        self._row_widgets = []
        while self._list_layout.count() > 0:
            item = self._list_layout.takeAt(0)
            widget = item.widget()
            if widget is None:
                continue
            widget.setParent(None)
            widget.deleteLater()

        for idx, name in enumerate(self._names):
            row = self._build_row(name, focused=self._cursor_index == idx)
            self._list_layout.addWidget(row)
            self._row_widgets.append(row)

        close_row = self._build_row("Close", focused=self._cursor_index == len(self._names))
        self._list_layout.addWidget(close_row)
        self._row_widgets.append(close_row)

    def _refresh_styles(self) -> None:
        for idx, row in enumerate(self._row_widgets):
            is_close = idx == len(self._names)
            focused = self._cursor_index == idx
            text = "Close" if is_close else self._names[idx]
            style = (
                f"background: {theme.ACCENT}; color: {theme.PANEL}; border-radius: 6px; margin: 2px 8px;"
                if focused
                else f"background: {theme.TERTIARY}; color: {theme.TEXT_PRIMARY}; border-radius: 6px; margin: 2px 8px;"
            )
            row.setStyleSheet(style)

    def _build_row(self, text: str, focused: bool) -> QWidget:
        row = QWidget(self._list_container)
        row.setFixedHeight(48)

        style = (
            f"background: {theme.ACCENT}; color: {theme.PANEL}; border-radius: 6px; margin: 2px 8px;"
            if focused
            else f"background: {theme.TERTIARY}; color: {theme.TEXT_PRIMARY}; border-radius: 6px; margin: 2px 8px;"
        )
        row.setStyleSheet(style)

        layout = QVBoxLayout(row)
        layout.setContentsMargins(16, 0, 16, 0)
        layout.setSpacing(0)

        label = QLabel(str(text or ""), row)
        label.setAlignment(Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft)
        label.setStyleSheet("border: none; background: transparent;")
        layout.addWidget(label)

        return row

    def _scroll_to_cursor(self) -> None:
        if not self._row_widgets:
            return
        if self._cursor_index < 0 or self._cursor_index >= len(self._row_widgets):
            return
        self._scroll.ensureWidgetVisible(self._row_widgets[self._cursor_index], 0, 8)
