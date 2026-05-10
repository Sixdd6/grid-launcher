from __future__ import annotations

from typing import Any

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QHBoxLayout, QLabel, QScrollArea, QVBoxLayout, QWidget

from rom_mate.tv.widgets import theme


def _button_stylesheet() -> str:
    return (
        "QPushButton {"
        f"background: {theme.TERTIARY};"
        f"color: {theme.TEXT_PRIMARY};"
        f"border: 1px solid {theme.BORDER_INACTIVE};"
        "border-radius: 4px;"
        "padding: 6px 16px;"
        "font-size: 14px;"
        "}"
        "QPushButton:hover {"
        f"background: {theme.ACCENT};"
        f"color: {theme.BG};"
        f"border: 1px solid {theme.ACCENT};"
        "}"
        "QPushButton:focus {"
        f"background: {theme.ACCENT};"
        f"color: {theme.BG};"
        f"border: 1px solid {theme.ACCENT};"
        "outline: none;"
        "}"
    )


class _SettingRow(QWidget):
    def __init__(self, label: str, value: str = "", parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setFixedHeight(56)
        self._label = QLabel(label, self)
        self._value = QLabel(value, self)
        self._value.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)

        layout = QHBoxLayout(self)
        layout.setContentsMargins(20, 0, 20, 0)
        layout.setSpacing(16)
        layout.addWidget(self._label, 1)
        layout.addWidget(self._value, 0)

        self._label.setStyleSheet(f"color: {theme.TEXT_PRIMARY}; font-size: 15px;")
        self._value.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 14px;")
        self.set_focused(False)

    def set_label(self, label: str) -> None:
        self._label.setText(label)

    def set_value(self, value: str) -> None:
        self._value.setText(value)

    def set_focused(self, focused: bool) -> None:
        if focused:
            self.setStyleSheet(
                f"background: {theme.TERTIARY}; border: 1px solid {theme.ACCENT}; border-left: 3px solid {theme.ACCENT};"
            )
            return
        self.setStyleSheet(
            f"background: {theme.PANEL}; border: 1px solid {theme.BORDER_INACTIVE}; border-left: 3px solid transparent;"
        )


class SettingsView(QWidget):
    _HOME_TABS = ["home", "library", "server"]

    def __init__(self, app_backend: Any, pop_callback, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._app_backend = app_backend
        self._pop_callback = pop_callback
        self._focus_index = 0
        self._picker_index = 0
        self._focus_rows: list[dict[str, Any]] = []

        self.setStyleSheet(
            f"background: {theme.BG};"
            f"{_button_stylesheet()}"
        )

        root_layout = QVBoxLayout(self)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        header = QWidget(self)
        header.setFixedHeight(56)
        header.setStyleSheet(f"background: {theme.PANEL};")
        header_layout = QHBoxLayout(header)
        header_layout.setContentsMargins(20, 0, 20, 0)
        title = QLabel("Settings", header)
        title.setStyleSheet(f"color: {theme.TEXT_PRIMARY}; font-size: 20px; font-weight: 700;")
        header_layout.addWidget(title)
        root_layout.addWidget(header)

        self._scroll = QScrollArea(self)
        self._scroll.setWidgetResizable(True)
        self._scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._scroll.setStyleSheet(
            f"QScrollArea {{ border: none; background: {theme.BG}; }}"
            f"QScrollArea > QWidget > QWidget {{ background: {theme.BG}; }}"
        )

        self._content = QWidget(self._scroll)
        self._content.setStyleSheet(f"background: {theme.BG};")
        self._content_layout = QVBoxLayout(self._content)
        self._content_layout.setContentsMargins(0, 8, 0, 16)
        self._content_layout.setSpacing(0)
        self._scroll.setWidget(self._content)

        root_layout.addWidget(self._scroll)

        self._rebuild_rows()

        self._app_backend.homeViewTabChanged.connect(self._on_home_view_tab_changed)
        self._app_backend.exclusionListChanged.connect(self._on_exclusion_list_changed)
        self._app_backend.exclusionDataChanged.connect(self._on_exclusion_list_changed)
        self._app_backend.autoSyncChanged.connect(self._on_auto_sync_changed)

    def handle_nav(self, direction: object) -> None:
        nav = str(direction or "")
        if nav == "up":
            self._set_focus_index(max(0, self._focus_index - 1))
            return
        if nav == "down":
            self._set_focus_index(min(len(self._focus_rows) - 1, self._focus_index + 1))
            return
        if nav in ("left", "right"):
            self._handle_horizontal(nav)
            return
        if nav == "confirm":
            self._activate_focused()
            return
        if nav == "back":
            self._pop_callback()

    def _rebuild_rows(self) -> None:
        while self._content_layout.count() > 0:
            item = self._content_layout.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.setParent(None)
                widget.deleteLater()

        self._focus_rows = []

        self._add_focus_row("back", _SettingRow("< Back"))
        self._add_focus_row("home_tab", _SettingRow("Default Tab", self._home_tab_display()))
        self._add_focus_row("auto_sync", _SettingRow("Auto Cloud Sync", self._auto_sync_display()))
        self._add_focus_row("desktop", _SettingRow("Return to Desktop Mode"))

        section = QLabel("Guide Button Emulator Exclusions", self._content)
        section.setStyleSheet(
            f"color: {theme.PURPLE}; font-size: 12px; font-weight: 700; padding: 14px 20px 10px 20px;"
        )
        self._content_layout.addWidget(section)

        self._add_focus_row("add_exclusion", _SettingRow("Add Exclusion", self._preview_emulator_name()))

        exclusions = list(getattr(self._app_backend, "tvGuideExclusionList", []) or [])
        for name in exclusions:
            row = _SettingRow(str(name), "x Remove")
            self._add_focus_row("remove_exclusion", row, str(name))

        self._content_layout.addStretch(1)

        if not self._focus_rows:
            self._focus_index = 0
            return
        self._set_focus_index(min(self._focus_index, len(self._focus_rows) - 1))

    def _add_focus_row(self, row_type: str, widget: _SettingRow, name: str = "") -> None:
        self._content_layout.addWidget(widget)
        self._focus_rows.append({"type": row_type, "name": name, "widget": widget})

    def _set_focus_index(self, index: int) -> None:
        if not self._focus_rows:
            self._focus_index = 0
            return
        clamped = max(0, min(len(self._focus_rows) - 1, int(index)))
        self._focus_index = clamped
        for idx, entry in enumerate(self._focus_rows):
            row_widget = entry["widget"]
            row_widget.set_focused(idx == self._focus_index)
        focused_widget = self._focus_rows[self._focus_index]["widget"]
        self._scroll.ensureWidgetVisible(focused_widget, 24, 24)

    def _handle_horizontal(self, direction: str) -> None:
        if not self._focus_rows:
            return
        entry = self._focus_rows[self._focus_index]
        row_type = entry["type"]

        if row_type == "home_tab":
            current = self._safe_home_tab()
            current_idx = self._HOME_TABS.index(current)
            step = -1 if direction == "left" else 1
            next_idx = (current_idx + step) % len(self._HOME_TABS)
            self._app_backend.setHomeViewTab(self._HOME_TABS[next_idx])
            return

        if row_type == "add_exclusion":
            names = self._available_emulator_names()
            if not names:
                return
            step = -1 if direction == "left" else 1
            self._picker_index = (self._picker_index + step) % len(names)
            entry["widget"].set_value(names[self._picker_index])

    def _activate_focused(self) -> None:
        if not self._focus_rows:
            return
        entry = self._focus_rows[self._focus_index]
        row_type = entry["type"]

        if row_type == "back":
            self._pop_callback()
            return

        if row_type == "home_tab":
            self._app_backend.setHomeViewTab(self._safe_home_tab())
            return

        if row_type == "auto_sync":
            self._app_backend.setAutoSync(not bool(getattr(self._app_backend, "isAutoSync", False)))
            return

        if row_type == "desktop":
            self._app_backend.requestDesktopMode()
            return

        if row_type == "add_exclusion":
            names = self._available_emulator_names()
            if not names:
                return
            self._picker_index = max(0, min(self._picker_index, len(names) - 1))
            self._app_backend.addExclusionEntry(names[self._picker_index])
            return

        if row_type == "remove_exclusion":
            name = str(entry.get("name", ""))
            if name:
                self._app_backend.removeExclusionEntry(name)

    def _safe_home_tab(self) -> str:
        value = str(getattr(self._app_backend, "homeViewTab", "home") or "home")
        if value not in self._HOME_TABS:
            return "home"
        return value

    def _home_tab_display(self) -> str:
        return self._safe_home_tab().capitalize()

    def _auto_sync_display(self) -> str:
        return "On" if bool(getattr(self._app_backend, "isAutoSync", False)) else "Off"

    def _available_emulator_names(self) -> list[str]:
        names = list(getattr(self._app_backend, "availableEmulatorNames", []) or [])
        normalized = [str(name).strip() for name in names if str(name).strip()]
        if not normalized:
            self._picker_index = 0
            return []
        self._picker_index = max(0, min(self._picker_index, len(normalized) - 1))
        return normalized

    def _preview_emulator_name(self) -> str:
        names = self._available_emulator_names()
        if not names:
            return "No emulators"
        return names[self._picker_index]

    def _on_home_view_tab_changed(self, _value: object) -> None:
        for entry in self._focus_rows:
            if entry["type"] == "home_tab":
                entry["widget"].set_value(self._home_tab_display())
                break

    def _on_auto_sync_changed(self, _value: object) -> None:
        for entry in self._focus_rows:
            if entry["type"] == "auto_sync":
                entry["widget"].set_value(self._auto_sync_display())
                break

    def _on_exclusion_list_changed(self, _payload: object) -> None:
        self._rebuild_rows()
