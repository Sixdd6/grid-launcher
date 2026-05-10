from __future__ import annotations

from typing import Any

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QFontMetrics, QPaintEvent, QPainter
from PySide6.QtWidgets import QFrame, QHBoxLayout, QLabel, QScrollArea, QVBoxLayout, QWidget

from rom_mate.tv.widgets import theme


class NativeExecPickerDialog(QWidget):
    def __init__(self, game_backend: Any, app_backend: Any, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._game_backend = game_backend
        self._app_backend = app_backend

        self._rom_id = ""
        self._candidates: list[dict[str, Any]] = []
        self._selected_index = -1
        self._cursor_index = 0

        self.setVisible(False)

        self._panel = QFrame(self)
        self._panel.setFixedSize(640, 520)
        self._panel.setStyleSheet(
            f"background: {theme.PANEL}; border: 1px solid {theme.BORDER_INACTIVE}; border-radius: 12px;"
        )

        panel_layout = QVBoxLayout(self._panel)
        panel_layout.setContentsMargins(20, 18, 20, 18)
        panel_layout.setSpacing(10)

        title = QLabel("Game Executable", self._panel)
        title.setStyleSheet(f"color: {theme.TEXT_PRIMARY}; font-size: 20px; font-weight: 700;")
        panel_layout.addWidget(title)

        subtitle = QLabel("Select the executable to use when launching", self._panel)
        subtitle.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 13px;")
        panel_layout.addWidget(subtitle)

        self._scroll = QScrollArea(self._panel)
        self._scroll.setWidgetResizable(True)
        self._scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._scroll.setFrameShape(QFrame.Shape.NoFrame)

        self._list_container = QWidget(self._scroll)
        self._list_layout = QVBoxLayout(self._list_container)
        self._list_layout.setContentsMargins(0, 0, 0, 0)
        self._list_layout.setSpacing(6)
        self._list_layout.setAlignment(Qt.AlignmentFlag.AlignTop)

        self._scroll.setWidget(self._list_container)
        panel_layout.addWidget(self._scroll, 1)

        self._close_button = QLabel("Close", self._panel)
        self._close_button.setFixedHeight(42)
        self._close_button.setAlignment(Qt.AlignmentFlag.AlignCenter)
        panel_layout.addWidget(self._close_button)

    def show_for_game(self, rom_id: str, candidates: list, current_path: str = "") -> None:
        self._rom_id = str(rom_id or "")
        self._candidates = [item for item in (candidates or []) if isinstance(item, dict)]
        self._cursor_index = 0
        self._selected_index = -1

        current = str(current_path or "").strip()
        if current:
            for idx, candidate in enumerate(self._candidates):
                if str(candidate.get("path", "")).strip() == current:
                    self._selected_index = idx
                    self._cursor_index = idx
                    break

        self._rebuild_ui()
        self.setVisible(True)
        self.raise_()
        self._app_backend.setUiOverlayActive(True)

    def close_overlay(self) -> None:
        self.setVisible(False)
        self._app_backend.setUiOverlayActive(False)

    def handle_nav(self, direction: str) -> None:
        if not self.isVisible():
            return

        max_idx = len(self._candidates)
        if direction == "up":
            self._cursor_index = max(0, self._cursor_index - 1)
            self._rebuild_ui()
            return

        if direction == "down":
            self._cursor_index = min(max_idx, self._cursor_index + 1)
            self._rebuild_ui()
            return

        if direction == "confirm":
            if self._cursor_index == max_idx:
                self.close_overlay()
                return

            if 0 <= self._cursor_index < len(self._candidates):
                candidate = self._candidates[self._cursor_index]
                exe_path = str(candidate.get("path", "") or "")
                self._game_backend.saveNativeExecutable({"rom_id": self._rom_id, "exe_path": exe_path})
                self._selected_index = self._cursor_index
                self._rebuild_ui()
            return

        if direction == "back":
            self.close_overlay()

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        panel_rect = self._panel.frameGeometry()
        panel_rect.moveCenter(self.rect().center())
        self._panel.move(panel_rect.topLeft())

    def paintEvent(self, event: QPaintEvent) -> None:
        _ = event
        painter = QPainter(self)
        painter.fillRect(self.rect(), QColor(0, 0, 0, 204))

    def _rebuild_ui(self) -> None:
        while self._list_layout.count() > 0:
            item = self._list_layout.takeAt(0)
            widget = item.widget()
            if widget is None:
                continue
            widget.setParent(None)
            widget.deleteLater()

        for idx, candidate in enumerate(self._candidates):
            self._list_layout.addWidget(self._build_candidate_row(idx, candidate))

        close_focused = self._cursor_index == len(self._candidates)
        close_border = theme.ACCENT if close_focused else theme.BORDER_INACTIVE
        close_bg = "rgba(255, 121, 198, 0.25)" if close_focused else "rgba(68, 71, 90, 0.35)"
        self._close_button.setStyleSheet(
            f"color: {theme.TEXT_PRIMARY}; background: {close_bg}; border: 1px solid {close_border};"
            "border-radius: 8px; font-size: 14px; font-weight: 700;"
        )

    def _build_candidate_row(self, idx: int, candidate: dict[str, Any]) -> QWidget:
        row = QFrame(self._list_container)
        row.setFixedHeight(52)

        focused = self._cursor_index == idx
        border = f"2px solid {theme.ACCENT}" if focused else "1px solid transparent"
        row.setStyleSheet(
            f"background: rgba(40, 42, 54, 0.78); border: {border}; border-radius: 8px;"
        )

        layout = QHBoxLayout(row)
        layout.setContentsMargins(12, 0, 12, 0)
        layout.setSpacing(10)

        selected = self._selected_index == idx
        circle = QLabel("●" if selected else "○", row)
        circle_color = theme.ACCENT if selected else theme.BORDER_INACTIVE
        circle.setStyleSheet(f"color: {circle_color}; font-size: 16px;")
        circle.setFixedWidth(20)
        layout.addWidget(circle)

        raw_label = str(candidate.get("label") or candidate.get("path") or "")
        font = row.font()
        font.setPixelSize(13)
        metrics = QFontMetrics(font)
        elided = metrics.elidedText(raw_label, Qt.TextElideMode.ElideMiddle, 520)

        name_label = QLabel(elided, row)
        name_label.setFont(font)
        name_label.setStyleSheet(f"color: {theme.TEXT_PRIMARY};")
        layout.addWidget(name_label, 1)

        return row
