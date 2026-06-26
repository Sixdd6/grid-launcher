from __future__ import annotations

from typing import Any

from PySide6.QtCore import QTimer, Qt
from PySide6.QtGui import QColor, QPaintEvent, QPainter
from PySide6.QtWidgets import QFrame, QHBoxLayout, QLabel, QVBoxLayout, QWidget

from rom_mate.tv.widgets.components.nav_scroll_area import NavScrollArea

from rom_mate.tv.widgets import theme


class CloudSavesOverlay(QWidget):
    def __init__(self, cloud_backend: Any, app_backend: Any, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._cloud_backend = cloud_backend
        self._app_backend = app_backend

        self._game: dict[str, Any] = {}
        self._save_type = "save"
        self._slots: list[dict[str, Any]] = []
        self._current_index = 0
        self._action_mode = 0
        self._loading = False
        self._uploading = False
        self._error_text = ""
        self._status_text = ""
        self._status_success = True

        self.setVisible(False)
        self.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)

        self._panel = QFrame(self)
        self._panel.setFixedSize(700, 560)
        self._panel.setStyleSheet(
            f"background: {theme.PANEL}; border: 1px solid {theme.BORDER_INACTIVE}; border-radius: 12px;"
        )

        panel_layout = QVBoxLayout(self._panel)
        panel_layout.setContentsMargins(20, 18, 20, 18)
        panel_layout.setSpacing(10)

        self._title_label = QLabel("Cloud Saves", self._panel)
        self._title_label.setStyleSheet(f"color: {theme.PURPLE}; font-size: 18px; font-weight: 700;")
        panel_layout.addWidget(self._title_label)

        self._game_label = QLabel("", self._panel)
        self._game_label.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 13px;")
        panel_layout.addWidget(self._game_label)

        self._status_label = QLabel("", self._panel)
        self._status_label.setFixedHeight(36)
        self._status_label.setAlignment(Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft)
        self._status_label.setVisible(False)
        self._status_label.setStyleSheet("padding: 0 10px; border-radius: 8px;")
        panel_layout.addWidget(self._status_label)

        self._scroll = NavScrollArea(self._panel)
        self._scroll.setWidgetResizable(True)
        self._scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._scroll.setFrameShape(QFrame.Shape.NoFrame)

        self._list_container = QWidget(self._scroll)
        self._list_layout = QVBoxLayout(self._list_container)
        self._list_layout.setContentsMargins(0, 0, 0, 0)
        self._list_layout.setSpacing(8)
        self._list_layout.setAlignment(Qt.AlignmentFlag.AlignTop)

        self._scroll.setWidget(self._list_container)
        panel_layout.addWidget(self._scroll, 1)

        self._status_timer = QTimer(self)
        self._status_timer.setSingleShot(True)
        self._status_timer.timeout.connect(self._clear_status)

        self._cloud_backend.slotsLoaded.connect(self._on_slots_loaded)
        self._cloud_backend.slotsError.connect(self._on_slots_error)
        self._cloud_backend.restoreComplete.connect(self._on_restore_complete)
        self._cloud_backend.deleteComplete.connect(self._on_delete_complete)
        self._cloud_backend.uploadComplete.connect(self._on_upload_complete)

    def show_for_game(self, game_dict: dict, save_type: str = "save") -> None:
        self._game = dict(game_dict or {})
        self._save_type = str(save_type or "save")
        self._slots = []
        self._current_index = 0
        self._action_mode = 0
        self._loading = True
        self._uploading = False
        self._error_text = ""
        self._clear_status()

        game_name = str(self._game.get("name") or self._game.get("title") or "")
        self._game_label.setText(game_name)

        self._rebuild_ui()
        self.setVisible(True)
        self.raise_()
        self._app_backend.setUiOverlayActive(True)
        self._cloud_backend.loadSlotsForGame({"game": self._game, "save_type": self._save_type})

    def close_overlay(self) -> None:
        self.setVisible(False)
        self._app_backend.setUiOverlayActive(False)

    def handle_nav(self, direction: str) -> None:
        if not self.isVisible():
            return

        if direction == "up":
            self._current_index = max(0, self._current_index - 1)
            self._action_mode = 0
            self._rebuild_ui()
            return

        if direction == "down":
            self._current_index = min(len(self._slots), self._current_index + 1)
            self._action_mode = 0
            self._rebuild_ui()
            return

        if direction == "left":
            if self._current_index > 0 and self._action_mode > 0:
                self._action_mode -= 1
                self._rebuild_ui()
            return

        if direction == "right":
            if self._current_index > 0:
                self._action_mode = min(2, self._action_mode + 1)
                self._rebuild_ui()
            return

        if direction == "confirm":
            if self._current_index == 0:
                if not self._uploading:
                    self._uploading = True
                    self._rebuild_ui()
                    self._cloud_backend.uploadSave({"game": self._game, "save_type": self._save_type})
                return

            slot_idx = self._current_index - 1
            if slot_idx < 0 or slot_idx >= len(self._slots):
                return
            slot = self._slots[slot_idx]
            save_id = str(slot.get("id", "")).strip()
            if not save_id:
                return

            if self._action_mode == 0:
                self._action_mode = 1
                self._rebuild_ui()
                return

            if self._action_mode == 1:
                self._cloud_backend.restoreSlot(
                    {"game": self._game, "save_id": save_id, "save_type": self._save_type}
                )
                return

            if self._action_mode == 2:
                self._cloud_backend.deleteSlot({"save_id": save_id, "save_type": self._save_type})
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
            widget.hide()
            widget.setParent(None)
            widget.deleteLater()

        upload_row = self._build_upload_row()
        self._list_layout.addWidget(upload_row)

        if self._loading:
            self._list_layout.addWidget(self._build_state_label("Loading saves..."))
            return

        if self._error_text:
            self._list_layout.addWidget(self._build_state_label(self._error_text))
            return

        if not self._slots:
            self._list_layout.addWidget(self._build_state_label("No cloud saves found."))
            return

        for idx, slot in enumerate(self._slots):
            row = self._build_slot_row(idx + 1, slot)
            self._list_layout.addWidget(row)

    def _build_upload_row(self) -> QWidget:
        row = QFrame(self._list_container)
        row.setFixedHeight(56)
        focused = self._current_index == 0
        row_bg = "rgba(255, 121, 198, 0.25)" if focused else "rgba(68, 71, 90, 0.35)"
        row_border = theme.ACCENT if focused else theme.BORDER_INACTIVE
        row.setStyleSheet(
            f"background: {row_bg}; border: 1px solid {row_border}; border-radius: 8px;"
        )

        layout = QHBoxLayout(row)
        layout.setContentsMargins(12, 0, 12, 0)

        title = "Upload New Save" if not self._uploading else "Uploading..."
        label = QLabel(title, row)
        label.setStyleSheet(f"color: {theme.TEXT_PRIMARY}; font-size: 14px; font-weight: 600;")
        layout.addWidget(label)
        layout.addStretch(1)
        return row

    def _build_slot_row(self, row_index: int, slot: dict[str, Any]) -> QWidget:
        row = QFrame(self._list_container)
        row.setFixedHeight(72)

        focused_row = self._current_index == row_index
        row_bg = "rgba(255, 121, 198, 0.18)" if focused_row else "rgba(40, 42, 54, 0.72)"
        row_border = theme.ACCENT if focused_row else theme.BORDER_INACTIVE
        row.setStyleSheet(
            f"background: {row_bg}; border: 1px solid {row_border}; border-radius: 8px;"
        )

        layout = QHBoxLayout(row)
        layout.setContentsMargins(12, 8, 12, 8)
        layout.setSpacing(12)

        meta_layout = QVBoxLayout()
        timestamp = str(slot.get("timestamp_text", "") or "")
        file_name = str(slot.get("file_name", "") or "")
        emulator = str(slot.get("emulator", "") or "")

        title = QLabel(timestamp or "Unknown date", row)
        title.setStyleSheet(f"color: {theme.TEXT_PRIMARY}; font-size: 14px; font-weight: 700;")
        meta_layout.addWidget(title)

        subtitle = QLabel(file_name or emulator or "Save Slot", row)
        subtitle.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 12px;")
        meta_layout.addWidget(subtitle)

        layout.addLayout(meta_layout, 1)

        if focused_row:
            actions = QHBoxLayout()
            actions.setSpacing(8)
            actions.addWidget(self._action_chip("Restore", self._action_mode == 1, theme.ACCENT))
            actions.addWidget(self._action_chip("Delete", self._action_mode == 2, theme.ERROR))
            layout.addLayout(actions)

        return row

    def _action_chip(self, text: str, selected: bool, color_hex: str) -> QLabel:
        label = QLabel(text, self._list_container)
        bg = color_hex if selected else "rgba(68, 71, 90, 0.9)"
        fg = theme.PANEL if selected else theme.TEXT_PRIMARY
        label.setStyleSheet(
            f"background: {bg}; color: {fg}; border-radius: 6px; padding: 4px 10px; font-size: 12px; font-weight: 700;"
        )
        return label

    def _build_state_label(self, text: str) -> QWidget:
        frame = QFrame(self._list_container)
        frame.setFixedHeight(200)
        layout = QVBoxLayout(frame)
        layout.setContentsMargins(0, 0, 0, 0)

        label = QLabel(text, frame)
        label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        label.setWordWrap(True)
        label.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 14px;")
        layout.addStretch(1)
        layout.addWidget(label)
        layout.addStretch(1)
        return frame

    def _show_status(self, text: str, success: bool) -> None:
        self._status_text = str(text or "")
        self._status_success = bool(success)
        if not self._status_text:
            self._status_label.setVisible(False)
            return
        bg = "rgba(80, 250, 123, 0.25)" if success else "rgba(255, 85, 85, 0.25)"
        fg = theme.SUCCESS if success else theme.ERROR
        self._status_label.setText(self._status_text)
        self._status_label.setStyleSheet(
            f"padding: 0 10px; border-radius: 8px; background: {bg}; color: {fg}; font-size: 13px; font-weight: 600;"
        )
        self._status_label.setVisible(True)
        self._status_timer.stop()
        self._status_timer.start(4000)

    def _clear_status(self) -> None:
        self._status_text = ""
        self._status_label.setVisible(False)

    def _reload_slots(self) -> None:
        if not self._game:
            return
        self._loading = True
        self._error_text = ""
        self._rebuild_ui()
        self._cloud_backend.loadSlotsForGame({"game": self._game, "save_type": self._save_type})

    def _on_slots_loaded(self, bundle: object) -> None:
        if not isinstance(bundle, dict):
            return
        if str(bundle.get("save_type", "")) != self._save_type:
            return
        slots = bundle.get("slots", [])
        self._slots = [slot for slot in (slots or []) if isinstance(slot, dict)]
        self._loading = False
        self._error_text = ""
        self._current_index = min(self._current_index, len(self._slots))
        self._action_mode = 0
        self._rebuild_ui()

    def _on_slots_error(self, bundle: object) -> None:
        if not isinstance(bundle, dict):
            return
        if str(bundle.get("save_type", "")) != self._save_type:
            return
        self._loading = False
        self._error_text = str(bundle.get("error", "") or "Could not load saves.")
        self._rebuild_ui()

    def _on_restore_complete(self, bundle: object) -> None:
        success = bool(bundle.get("success", False)) if isinstance(bundle, dict) else False
        message = str(bundle.get("message", "")) if isinstance(bundle, dict) else ""
        self._show_status(message, success)
        if success:
            self._reload_slots()

    def _on_delete_complete(self, bundle: object) -> None:
        success = bool(bundle.get("success", False)) if isinstance(bundle, dict) else False
        message = str(bundle.get("message", "")) if isinstance(bundle, dict) else ""
        self._show_status(message, success)
        if success:
            self._reload_slots()

    def _on_upload_complete(self, bundle: object) -> None:
        self._uploading = False
        success = bool(bundle.get("success", False)) if isinstance(bundle, dict) else False
        message = str(bundle.get("message", "")) if isinstance(bundle, dict) else ""
        self._show_status(message, success)
        self._rebuild_ui()
        if success:
            self._reload_slots()
