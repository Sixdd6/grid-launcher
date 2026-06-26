from __future__ import annotations

from typing import Any

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QPainter
from PySide6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QVBoxLayout,
    QWidget,
)

from rom_mate.tv.widgets.components.controls_bar import ControlHint
from rom_mate.tv.widgets.components.nav_scroll_area import NavScrollArea

from rom_mate.tv.widgets.components.emulator_picker_overlay import EmulatorPickerOverlay
from rom_mate.tv.widgets import theme


_PANEL_ALT = "#44454f"


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
        self.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
        self.setFixedHeight(56)
        self._is_alt = False
        self._label = QLabel(label, self)
        self._value = QLabel(value, self)
        self._value.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)

        layout = QHBoxLayout(self)
        layout.setContentsMargins(20, 0, 20, 0)
        layout.setSpacing(16)
        layout.addWidget(self._label, 1)
        layout.addWidget(self._value, 0)

        self._label.setStyleSheet(
            f"color: {theme.TEXT_PRIMARY}; font-size: 15px; background: transparent; border: none;"
        )
        self._value.setStyleSheet(
            f"color: {theme.TEXT_SECONDARY}; font-size: 14px; background: transparent; border: none;"
        )
        self.set_focused(False)

    def set_label(self, label: str) -> None:
        self._label.setText(label)

    def set_value(self, value: str) -> None:
        self._value.setText(value)

    def set_alt_row(self, alt: bool) -> None:
        self._is_alt = alt
        self.set_focused(False)

    def set_focused(self, focused: bool) -> None:
        if focused:
            self.setStyleSheet(
                f"background: {theme.TERTIARY};"
                f"border: 2px solid {theme.ACCENT};"
                "border-radius: 8px;"
            )
            return
        bg = _PANEL_ALT if self._is_alt else theme.PANEL
        self.setStyleSheet(
            f"background: {bg};"
            f"border: 1px solid {theme.BORDER_INACTIVE};"
            "border-radius: 8px;"
        )


class _TabSelectorRow(QWidget):
    _TABS = ["home", "library", "server"]
    _LABELS = ["Home", "Library", "Server"]

    def __init__(self, current_tab: str, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
        self.setFixedHeight(64)
        self._cursor = self._tab_to_idx(current_tab)
        self._is_alt = False

        outer = QHBoxLayout(self)
        outer.setContentsMargins(20, 8, 20, 8)
        outer.setSpacing(16)

        label = QLabel("Default Tab")
        label.setStyleSheet(
            f"color: {theme.TEXT_PRIMARY}; font-size: 15px; background: transparent; border: none;"
        )
        outer.addWidget(label, 0)

        self._pill_container = QWidget()
        pill_layout = QHBoxLayout(self._pill_container)
        pill_layout.setContentsMargins(0, 0, 0, 0)
        pill_layout.setSpacing(8)
        self._pill_container.setStyleSheet("background: transparent;")

        self._pills: list[QLabel] = []
        for tab_label in self._LABELS:
            pill = QLabel(tab_label)
            pill.setAlignment(Qt.AlignmentFlag.AlignCenter)
            pill.setFixedHeight(36)
            pill.setMinimumWidth(80)
            pill.setStyleSheet(
                f"color: {theme.TEXT_SECONDARY}; background: {theme.TERTIARY};"
                "border-radius: 18px; padding: 0 16px; font-size: 14px; border: 2px solid transparent;"
            )
            pill_layout.addWidget(pill)
            self._pills.append(pill)

        outer.addWidget(self._pill_container, 1, Qt.AlignmentFlag.AlignRight)

        self._row_focused = False
        self._refresh_pills(current_tab)
        self.set_focused(False)

    def _tab_to_idx(self, tab: str) -> int:
        try:
            return self._TABS.index(str(tab))
        except ValueError:
            return 0

    def _refresh_pills(self, active_tab: str) -> None:
        active_idx = self._tab_to_idx(active_tab)
        for i, pill in enumerate(self._pills):
            is_active = i == active_idx
            is_cursor = i == self._cursor
            if is_active:
                bg = theme.ACCENT
                fg = theme.PANEL
                border = theme.ACCENT
            elif is_cursor and self._row_focused:
                bg = theme.TERTIARY
                fg = theme.TEXT_PRIMARY
                border = theme.TEXT_PRIMARY
            else:
                bg = theme.TERTIARY
                fg = theme.TEXT_SECONDARY
                border = "transparent"
            pill.setStyleSheet(
                f"color: {fg}; background: {bg};"
                f"border-radius: 18px; padding: 0 16px; font-size: 14px;"
                f"border: 2px solid {border};"
            )

    def set_focused(self, focused: bool) -> None:
        self._row_focused = focused
        if focused:
            self.setStyleSheet(
                f"background: {theme.TERTIARY}; border: 2px solid {theme.ACCENT}; border-radius: 8px;"
            )
            return
        bg = _PANEL_ALT if self._is_alt else theme.PANEL
        self.setStyleSheet(
            f"background: {bg}; border: 1px solid transparent; border-radius: 8px;"
        )

    def update_active(self, active_tab: str) -> None:
        self._refresh_pills(active_tab)

    def move_cursor(self, direction: str) -> None:
        step = -1 if direction == "left" else 1
        self._cursor = (self._cursor + step) % len(self._TABS)

    def cursor_tab(self) -> str:
        return self._TABS[self._cursor]

    def set_alt_row(self, alt: bool) -> None:
        self._is_alt = alt
        self.set_focused(False)

    def sync_cursor_to(self, active_tab: str) -> None:
        self._cursor = self._tab_to_idx(active_tab)


class _ToggleSwitch(QWidget):
    def __init__(self, checked: bool = False, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._checked = checked
        self.setFixedSize(56, 30)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)

    def set_checked(self, checked: bool) -> None:
        self._checked = checked
        self.update()

    def paintEvent(self, event) -> None:  # noqa: ARG002
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        pill_color = QColor(theme.ACCENT) if self._checked else QColor(theme.BORDER_INACTIVE)
        painter.setBrush(pill_color)
        painter.setPen(Qt.PenStyle.NoPen)
        painter.drawRoundedRect(0, 1, 56, 28, 14, 14)
        circle_color = QColor(theme.TEXT_PRIMARY)
        painter.setBrush(circle_color)
        x = 30 if self._checked else 2
        painter.drawEllipse(x, 3, 24, 24)


class _ToggleRow(QWidget):
    def __init__(self, label: str, checked: bool = False, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
        self.setFixedHeight(56)
        self._is_alt = False
        layout = QHBoxLayout(self)
        layout.setContentsMargins(20, 0, 20, 0)
        layout.setSpacing(16)

        lbl = QLabel(label)
        lbl.setStyleSheet(
            f"color: {theme.TEXT_PRIMARY}; font-size: 15px; background: transparent; border: none;"
        )
        layout.addWidget(lbl, 1)

        self._toggle = _ToggleSwitch(checked)
        layout.addWidget(self._toggle, 0)

        self.set_focused(False)

    def set_checked(self, checked: bool) -> None:
        self._toggle.set_checked(checked)

    def set_alt_row(self, alt: bool) -> None:
        self._is_alt = alt
        self.set_focused(False)

    def set_focused(self, focused: bool) -> None:
        if focused:
            self.setStyleSheet(
                f"background: {theme.TERTIARY}; border: 2px solid {theme.ACCENT}; border-radius: 8px;"
            )
            return
        bg = _PANEL_ALT if self._is_alt else theme.PANEL
        self.setStyleSheet(
            f"background: {bg}; border: 1px solid {theme.BORDER_INACTIVE}; border-radius: 8px;"
        )


class _ActionButton(QFrame):
    def __init__(self, label: str, parent=None):
        super().__init__(parent)
        self.setFrameShape(QFrame.Shape.NoFrame)
        self.setFixedHeight(56)
        layout = QHBoxLayout(self)
        layout.setContentsMargins(16, 0, 16, 0)

        self._label = QLabel(label)
        self._label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._label.setStyleSheet(
            f"color: {theme.TEXT_PRIMARY}; font-size: 15px; font-weight: 600; background: transparent; border: none;"
        )
        layout.addWidget(self._label)

        self.set_focused(False)

    def set_focused(self, focused: bool) -> None:
        if focused:
            self.setStyleSheet(
                f"background: {theme.TERTIARY};"
                f"border: 2px solid {theme.ACCENT};"
                "border-radius: 8px;"
            )
        else:
            self.setStyleSheet(
                f"background: {theme.TERTIARY};"
                f"border: 1px solid {theme.TEXT_SECONDARY};"
                "border-radius: 8px;"
            )


class SettingsView(QWidget):
    _HOME_TABS = ["home", "library", "server"]
    _ACTION_GROUP = frozenset({"back", "desktop", "exit"})

    CONTROL_HINTS: list[ControlHint] = [
        ControlHint("Confirm", "input_BTN-D", "Enter"),
        ControlHint("Back", "input_BTN-R", "Backspace"),
        ControlHint("Navigate", "input_DPAD-U", "Arrows"),
    ]

    def __init__(self, app_backend: Any, pop_callback, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._app_backend = app_backend
        self._pop_callback = pop_callback
        self._focus_index = 0
        self._focus_rows: list[dict[str, Any]] = []

        self.setStyleSheet(f"background: {theme.BG};")

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

        self._scroll = NavScrollArea(self)
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
        self._emulator_picker = EmulatorPickerOverlay(self._app_backend, parent=self)
        self._emulator_picker.resize(self.size())

        self._app_backend.homeViewTabChanged.connect(self._on_home_view_tab_changed)
        self._app_backend.exclusionDataChanged.connect(self._on_exclusion_list_changed)
        self._app_backend.autoSyncChanged.connect(self._on_auto_sync_changed)

    def handle_nav(self, direction: object) -> None:
        nav = str(direction or "")
        if self._emulator_picker and self._emulator_picker.isVisible():
            self._emulator_picker.handle_nav(nav)
            return

        current_type = self._focus_rows[self._focus_index]["type"] if self._focus_rows else None
        in_action_group = current_type in self._ACTION_GROUP

        if nav == "up":
            if in_action_group:
                return
            first_after = next(
                (i for i, r in enumerate(self._focus_rows) if r["type"] not in self._ACTION_GROUP), 0
            )
            if self._focus_index == first_after:
                self._set_focus_index(0)
            else:
                self._set_focus_index(self._focus_index - 1)
            return

        if nav == "down":
            if in_action_group:
                first_after = next(
                    (i for i, r in enumerate(self._focus_rows) if r["type"] not in self._ACTION_GROUP), None
                )
                if first_after is not None:
                    self._set_focus_index(first_after)
                return
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
                widget.hide()
                widget.setParent(None)
                widget.deleteLater()

        self._focus_rows = []

        action_row_container = QWidget(self._content)
        action_row_layout = QHBoxLayout(action_row_container)
        action_row_layout.setContentsMargins(24, 8, 24, 8)
        action_row_layout.setSpacing(8)

        back_row = _ActionButton("< Back")
        self._add_focus_row("back", back_row, target_layout=action_row_layout)
        action_row_layout.setStretch(action_row_layout.indexOf(back_row), 1)

        desktop_row = _ActionButton("Return to Desktop Mode")
        self._add_focus_row("desktop", desktop_row, target_layout=action_row_layout)
        action_row_layout.setStretch(action_row_layout.indexOf(desktop_row), 1)

        exit_row = _ActionButton("Exit")
        self._add_focus_row("exit", exit_row, target_layout=action_row_layout)
        action_row_layout.setStretch(action_row_layout.indexOf(exit_row), 1)

        self._content_layout.addWidget(action_row_container)

        general_section = QLabel("General", self._content)
        general_section.setStyleSheet(
            f"color: {theme.PURPLE}; font-size: 20px; font-weight: 700; padding: 16px 24px 8px 24px;"
        )
        self._content_layout.addWidget(general_section)

        card_wrapper = QWidget(self._content)
        card_wrapper_layout = QHBoxLayout(card_wrapper)
        card_wrapper_layout.setContentsMargins(24, 8, 24, 8)
        card_wrapper_layout.setSpacing(0)

        settings_card = QWidget(card_wrapper)
        settings_card.setObjectName("settings_card")
        settings_card.setStyleSheet(
            "QWidget#settings_card {"
            f"background: {theme.PANEL};"
            f"border: 1px solid {theme.BORDER_INACTIVE};"
            "border-radius: 8px;"
            "}"
        )
        card_layout = QVBoxLayout(settings_card)
        card_layout.setContentsMargins(0, 0, 0, 0)
        card_layout.setSpacing(0)

        home_tab_row = _TabSelectorRow(self._home_tab_display().lower())
        self._add_focus_row("home_tab", home_tab_row, target_layout=card_layout)
        auto_sync_row = _ToggleRow("Auto Cloud Sync", bool(getattr(self._app_backend, "isAutoSync", False)))
        self._add_focus_row("auto_sync", auto_sync_row, target_layout=card_layout)
        home_tab_row.set_alt_row(False)
        auto_sync_row.set_alt_row(True)

        card_wrapper_layout.addWidget(settings_card)
        self._content_layout.addWidget(card_wrapper)

        section = QLabel("Guide Button Emulator Exclusions", self._content)
        section.setStyleSheet(
            f"color: {theme.PURPLE}; font-size: 20px; font-weight: 700; padding: 16px 24px 8px 24px;"
        )
        self._content_layout.addWidget(section)

        excl_card_wrapper = QWidget(self._content)
        excl_card_wrapper_layout = QHBoxLayout(excl_card_wrapper)
        excl_card_wrapper_layout.setContentsMargins(24, 8, 24, 8)
        excl_card_wrapper_layout.setSpacing(0)

        excl_card = QWidget(excl_card_wrapper)
        excl_card.setObjectName("excl_card")
        excl_card.setStyleSheet(
            "QWidget#excl_card {"
            f"background: {theme.PANEL};"
            f"border: 1px solid {theme.BORDER_INACTIVE};"
            "border-radius: 8px;"
            "}"
        )
        excl_card_layout = QVBoxLayout(excl_card)
        excl_card_layout.setContentsMargins(0, 0, 0, 0)
        excl_card_layout.setSpacing(0)

        add_excl_btn = _SettingRow("+ Add Exclusion")
        add_excl_btn.set_alt_row(False)
        add_excl_btn.setFixedWidth(220)

        add_excl_container = QWidget()
        add_excl_container.setStyleSheet("background: transparent; border: none;")
        add_excl_container.setFixedHeight(64)
        add_excl_outer = QHBoxLayout(add_excl_container)
        add_excl_outer.setContentsMargins(12, 10, 12, 10)
        add_excl_outer.setSpacing(0)
        add_excl_outer.addWidget(add_excl_btn, 0)
        add_excl_outer.addStretch(1)

        excl_card_layout.addWidget(add_excl_container)
        self._focus_rows.append({"type": "add_exclusion", "name": "", "widget": add_excl_btn})

        exclusions = list(getattr(self._app_backend, "tvGuideExclusionList", []) or [])
        for i, name in enumerate(exclusions):
            row = _SettingRow(str(name), "x Remove")
            row.set_alt_row((i + 1) % 2 == 1)
            self._add_focus_row("remove_exclusion", row, str(name), target_layout=excl_card_layout)

        excl_card_wrapper_layout.addWidget(excl_card)
        self._content_layout.addWidget(excl_card_wrapper)

        self._content_layout.addStretch(1)

        if not self._focus_rows:
            self._focus_index = 0
            return
        self._set_focus_index(min(self._focus_index, len(self._focus_rows) - 1))

    def _add_focus_row(
        self,
        row_type: str,
        widget: QWidget,
        name: str = "",
        target_layout: QVBoxLayout | QHBoxLayout | None = None,
    ) -> None:
        if target_layout is not None:
            target_layout.addWidget(widget)
        else:
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
            if entry["type"] == "home_tab":
                active = str(getattr(self._app_backend, "homeViewTab", "home") or "home")
                row_widget.update_active(active)
        focused_widget = self._focus_rows[self._focus_index]["widget"]
        self._scroll.ensureWidgetVisible(focused_widget, 24, 24)

    def _handle_horizontal(self, direction: str) -> None:
        if not self._focus_rows:
            return
        entry = self._focus_rows[self._focus_index]
        row_type = entry["type"]

        if row_type in self._ACTION_GROUP:
            action_indices = [i for i, r in enumerate(self._focus_rows) if r["type"] in self._ACTION_GROUP]
            pos = action_indices.index(self._focus_index)
            step = -1 if direction == "left" else 1
            new_pos = (pos + step) % len(action_indices)
            self._set_focus_index(action_indices[new_pos])
            return

        if row_type == "home_tab":
            entry["widget"].move_cursor(direction)
            entry["widget"].update_active(entry["widget"].cursor_tab())
            return

    def _activate_focused(self) -> None:
        if not self._focus_rows:
            return
        entry = self._focus_rows[self._focus_index]
        row_type = entry["type"]

        if row_type == "back":
            self._pop_callback()
            return

        if row_type == "home_tab":
            tab = entry["widget"].cursor_tab()
            self._app_backend.setHomeViewTab(tab)
            return

        if row_type == "auto_sync":
            self._app_backend.setAutoSync(not bool(getattr(self._app_backend, "isAutoSync", False)))
            return

        if row_type == "desktop":
            self._app_backend.requestDesktopMode()
            return

        if row_type == "exit":
            self._app_backend.requestQuit()
            return

        if row_type == "add_exclusion":
            self._open_emulator_picker()
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

    def _open_emulator_picker(self) -> None:
        names = list(getattr(self._app_backend, "availableEmulatorNames", []) or [])
        names = [str(n).strip() for n in names if str(n).strip()]
        if not names:
            return
        self._emulator_picker.show_picker(names, on_select=self._app_backend.addExclusionEntry)

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        if hasattr(self, "_emulator_picker") and self._emulator_picker:
            self._emulator_picker.resize(self.size())

    def _on_home_view_tab_changed(self, _value: object) -> None:
        for entry in self._focus_rows:
            if entry["type"] == "home_tab":
                active = str(getattr(self._app_backend, "homeViewTab", "home") or "home")
                entry["widget"].update_active(active)
                entry["widget"].sync_cursor_to(active)
                break

    def _on_auto_sync_changed(self, _value: object) -> None:
        for entry in self._focus_rows:
            if entry["type"] == "auto_sync":
                entry["widget"].set_checked(bool(getattr(self._app_backend, "isAutoSync", False)))
                break

    def _on_exclusion_list_changed(self, _payload: object = None) -> None:
        self._rebuild_rows()
