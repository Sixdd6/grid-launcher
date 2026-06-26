from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Callable

from PySide6.QtCore import QEvent, Qt, QSize, QTimer, Signal
from PySide6.QtGui import QPixmap, QWheelEvent
from rom_mate.cover.details import _apply_font_scale
from rom_mate.ui.theme import themed_svg_pixmap
from PySide6.QtWidgets import (
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QScrollArea,
    QSizePolicy,
    QVBoxLayout,
    QWidget,
)

from rom_mate.tv.widgets import theme
from rom_mate.tv.widgets.components.cloud_saves_overlay import CloudSavesOverlay
from rom_mate.tv.widgets.components.controls_bar import ControlHint
from rom_mate.tv.widgets.components.fanart_background import FanartBackground
from rom_mate.tv.widgets.components.install_progress_bar import InstallProgressBar
from rom_mate.tv.widgets.components.native_exec_picker import NativeExecPickerDialog
from rom_mate.tv.widgets.components.nav_scroll_area import NavScrollArea


_ASSETS = Path(__file__).resolve().parents[4] / "assets" / "retroarch-assets"
_SVG_ASSETS_ROOT = Path(__file__).resolve().parents[4] / "assets"
_TV_FONT_REF_HEIGHT = 1080
_TV_FONT_MAX_SCALE = 2.5


_DETAILS_HINTS: list[ControlHint] = [
    ControlHint("Confirm", "input_BTN-D", "Enter"),
    ControlHint("Back", "input_BTN-R", "Backspace"),
    ControlHint("Navigate", "input_DPAD-U", "Arrows"),
]
_LIGHTBOX_HINTS: list[ControlHint] = [
    ControlHint("Close", "input_BTN-R", "Backspace"),
    ControlHint("Navigate", "input_DPAD-U", "←↑↓→"),
]


class DetailsView(QWidget):
    controlHintsChanged = Signal()

    @property
    def CONTROL_HINTS(self) -> list[ControlHint]:
        return _LIGHTBOX_HINTS if self._lightbox.isVisible() else _DETAILS_HINTS

    def __init__(
        self,
        game_dict: dict,
        app_backend,
        cloud_backend,
        game_backend,
        pause_backend,
        controller_backend,
        cover_loader,
        pop_callback: Callable[[], None],
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self._game = dict(game_dict or {})
        self._app_backend = app_backend
        self._cloud_backend = cloud_backend
        self._game_backend = game_backend
        self._pause_backend = pause_backend
        self._controller_backend = controller_backend
        self._cover_loader = cover_loader
        self._pop_callback = pop_callback

        self._focused_column = 0
        self._button_index = 0
        self._shot_index = 0
        self._shot_cards: list[QLabel] = []
        self._shot_pixmaps: list[QPixmap | None] = []
        self._buttons: list[dict[str, str]] = []
        self._install_progress = 0.0
        self._install_speed = 0.0
        self._metadata_loading = False
        self._installed_local_path = ""
        self._cover_pixmap: QPixmap | None = None
        self._current_font_scale: float = 1.0
        self._scalable_labels: list[tuple[QLabel, str]] = []

        self.setStyleSheet(f"background: {theme.BG};")

        self._fanart = FanartBackground(self._cover_loader, self)

        root_layout = QVBoxLayout(self)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        self._content = QWidget(self)
        content_layout = QVBoxLayout(self._content)
        content_layout.setContentsMargins(0, 0, 0, 0)
        content_layout.setSpacing(0)

        self._header = QFrame(self._content)
        self._header.setFixedHeight(48)
        self._header.setStyleSheet("background: rgba(30, 31, 41, 0.6);")
        header_layout = QHBoxLayout(self._header)
        header_layout.setContentsMargins(16, 0, 16, 0)
        header_layout.setSpacing(12)

        self._back_label = QWidget(self._header)
        _back_hlayout = QHBoxLayout(self._back_label)
        _back_hlayout.setContentsMargins(0, 0, 0, 0)
        _back_hlayout.setSpacing(4)
        _arrow_pix = themed_svg_pixmap("svg/back", theme.TEXT_SECONDARY, size=QSize(16, 16))
        _back_icon_lbl = QLabel(self._back_label)
        _back_icon_lbl.setPixmap(_arrow_pix)
        _back_icon_lbl.setFixedSize(QSize(16, 16))
        _back_icon_lbl.setStyleSheet("background: transparent;")
        _back_hlayout.addWidget(_back_icon_lbl)
        self._back_text_lbl = QLabel("Back", self._back_label)
        self._back_text_lbl.setStyleSheet(
            f"color: {theme.TEXT_SECONDARY}; font-size: 14px; font-weight: 700;"
        )
        self._scalable_labels.append((self._back_text_lbl, self._back_text_lbl.styleSheet()))
        _back_hlayout.addWidget(self._back_text_lbl)
        header_layout.addWidget(self._back_label)

        self._header_title = QLabel(self._game_title(), self._header)
        self._header_title.setStyleSheet(f"color: {theme.TEXT_PRIMARY}; font-size: 16px; font-weight: 700;")
        self._scalable_labels.append((self._header_title, self._header_title.styleSheet()))
        header_layout.addWidget(self._header_title, 1)

        content_layout.addWidget(self._header)

        self._main = QWidget(self._content)
        main_layout = QHBoxLayout(self._main)
        main_layout.setContentsMargins(20, 16, 20, 16)
        main_layout.setSpacing(14)

        self._left_col = QFrame(self._main)
        self._left_col.setStyleSheet("background: transparent;")
        left_layout = QVBoxLayout(self._left_col)
        left_layout.setContentsMargins(0, 0, 0, 0)
        left_layout.setSpacing(10)

        self._cover_label = QLabel(self._left_col)
        self._cover_label.setMinimumHeight(360)
        self._cover_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._cover_label.setStyleSheet(
            "background: rgba(30, 31, 41, 0.85); border: none; border-radius: 8px;"
        )
        self._cover_label.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
        left_layout.addWidget(self._cover_label, 1)

        self._install_text = QLabel("", self._left_col)
        self._install_text.setStyleSheet(f"color: {theme.TEXT_PRIMARY}; font-size: 13px;")
        self._scalable_labels.append((self._install_text, self._install_text.styleSheet()))
        self._install_text.setVisible(False)
        left_layout.addWidget(self._install_text)

        self._progress_bar = InstallProgressBar(self._left_col)
        self._progress_bar.setVisible(False)
        left_layout.addWidget(self._progress_bar)

        self._button_container = QWidget(self._left_col)
        self._button_layout = QVBoxLayout(self._button_container)
        self._button_layout.setContentsMargins(0, 4, 0, 0)
        self._button_layout.setSpacing(8)
        left_layout.addWidget(self._button_container)

        self._center_col = QFrame(self._main)
        self._center_col.setStyleSheet(f"background: rgba(30, 31, 41, 0.82); border-radius: 10px;")
        center_layout = QVBoxLayout(self._center_col)
        center_layout.setContentsMargins(14, 12, 14, 12)
        center_layout.setSpacing(10)

        self._title_label = QLabel(self._game_title(), self._center_col)
        self._title_label.setWordWrap(True)
        self._title_label.setStyleSheet(f"color: {theme.SUCCESS}; font-size: 28px; font-weight: 700;")
        self._scalable_labels.append((self._title_label, self._title_label.styleSheet()))
        center_layout.addWidget(self._title_label)

        self._desc_scroll = NavScrollArea(self._center_col)
        self._desc_scroll.setWidgetResizable(True)
        self._desc_scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._desc_scroll.setFrameShape(QFrame.Shape.NoFrame)

        self._desc_content = QWidget(self._desc_scroll)
        desc_layout = QVBoxLayout(self._desc_content)
        desc_layout.setContentsMargins(0, 0, 0, 0)

        self._desc_label = QLabel(self._description_text(), self._desc_content)
        self._desc_label.setWordWrap(True)
        self._desc_label.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)
        self._desc_label.setStyleSheet(f"color: {theme.TEXT_PRIMARY}; font-size: 14px;")
        self._scalable_labels.append((self._desc_label, self._desc_label.styleSheet()))
        desc_layout.addWidget(self._desc_label)
        desc_layout.addStretch(1)

        self._desc_scroll.setWidget(self._desc_content)
        center_layout.addWidget(self._desc_scroll, 1)

        self._metadata_panel = QFrame(self._center_col)
        self._metadata_panel.setStyleSheet("background: transparent; border: none;")
        meta_layout = QGridLayout(self._metadata_panel)
        meta_layout.setContentsMargins(10, 8, 10, 8)
        meta_layout.setHorizontalSpacing(16)
        meta_layout.setVerticalSpacing(10)
        self._metadata_labels: dict[str, QLabel] = {}

        fields = [
            ("platform", "PLATFORM"), ("released", "RELEASED"), ("by", "BY"),
            ("version", "VERSION"), ("size", "SIZE"), ("rating", "RATING"),
            ("region", "REGION"), ("languages", "LANGUAGES"), ("genres", "GENRES"),
        ]
        for index, (key, display_name) in enumerate(fields):
            grid_row = index // 3
            grid_col = index % 3

            cell_widget = QWidget(self._metadata_panel)
            cell_widget.setStyleSheet("background: transparent; border: none;")
            cell_layout = QVBoxLayout(cell_widget)
            cell_layout.setContentsMargins(0, 0, 0, 0)
            cell_layout.setSpacing(2)

            name_lbl = QLabel(display_name, cell_widget)
            name_lbl.setStyleSheet(
                f"color: {theme.TEXT_SECONDARY}; font-size: 11px; font-weight: 700;"
                " background: transparent; border: none;"
            )
            self._scalable_labels.append((name_lbl, name_lbl.styleSheet()))

            value_lbl = QLabel("—", cell_widget)
            value_lbl.setWordWrap(True)
            value_lbl.setStyleSheet(
                f"color: {theme.TEXT_PRIMARY}; font-size: 13px;"
                " background: transparent; border: none;"
            )
            self._scalable_labels.append((value_lbl, value_lbl.styleSheet()))

            cell_layout.addWidget(name_lbl)
            cell_layout.addWidget(value_lbl)
            cell_layout.addStretch()

            meta_layout.addWidget(cell_widget, grid_row, grid_col)
            self._metadata_labels[key] = value_lbl

        center_layout.addWidget(self._metadata_panel)

        self._right_col = QFrame(self._main)
        self._right_col.setStyleSheet(f"background: rgba(30, 31, 41, 0.82); border-radius: 10px;")
        right_layout = QVBoxLayout(self._right_col)
        right_layout.setContentsMargins(12, 12, 12, 12)
        right_layout.setSpacing(10)

        self._shots_header = QLabel("Screenshots", self._right_col)
        self._shots_header.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 13px; font-weight: 700;")
        self._scalable_labels.append((self._shots_header, self._shots_header.styleSheet()))
        right_layout.addWidget(self._shots_header)

        self._shots_scroll = NavScrollArea(self._right_col)
        self._shots_scroll.setWidgetResizable(True)
        self._shots_scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._shots_scroll.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._shots_scroll.setFrameShape(QFrame.Shape.NoFrame)

        self._shots_container = QWidget(self._shots_scroll)
        self._shots_layout = QVBoxLayout(self._shots_container)
        self._shots_layout.setContentsMargins(4, 4, 4, 4)
        self._shots_layout.setSpacing(10)
        self._shots_layout.setAlignment(Qt.AlignmentFlag.AlignTop)

        self._shots_scroll.setWidget(self._shots_container)
        self._shots_scroll.viewport().installEventFilter(self)

        _arrow_style = (
            "background: rgba(0,0,0,0.6); border-radius: 16px;"
            f" color: {theme.TEXT_PRIMARY}; font-size: 13px;"
        )
        self._shot_up_arrow = QLabel("▲", self._shots_scroll.viewport())
        self._shot_up_arrow.setFixedSize(32, 32)
        self._shot_up_arrow.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._shot_up_arrow.setStyleSheet(_arrow_style)
        self._shot_up_arrow.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self._shot_up_arrow.hide()

        self._shot_down_arrow = QLabel("▼", self._shots_scroll.viewport())
        self._shot_down_arrow.setFixedSize(32, 32)
        self._shot_down_arrow.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._shot_down_arrow.setStyleSheet(_arrow_style)
        self._shot_down_arrow.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self._shot_down_arrow.hide()

        right_layout.addWidget(self._shots_scroll, 1)

        main_layout.addWidget(self._left_col, 1)
        main_layout.addWidget(self._center_col, 2)
        main_layout.addWidget(self._right_col, 1)
        content_layout.addWidget(self._main, 1)

        root_layout.addWidget(self._content)

        self._status_banner = QLabel("", self)
        self._status_banner.setFixedHeight(40)
        self._status_banner.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._status_banner.setVisible(False)
        self._status_timer = QTimer(self)
        self._status_timer.setSingleShot(True)
        self._status_timer.timeout.connect(self._status_banner.hide)

        self._cloud_overlay = CloudSavesOverlay(self._cloud_backend, self._app_backend, self)
        self._native_picker = NativeExecPickerDialog(self._game_backend, self._app_backend, self)

        # Lightbox overlay for enlarged screenshot view
        self._lightbox = QWidget(self)
        self._lightbox.setStyleSheet("background: rgba(0, 0, 0, 0.88);")
        self._lightbox.hide()
        lb_layout = QVBoxLayout(self._lightbox)
        lb_layout.setContentsMargins(0, 0, 0, 0)
        lb_layout.setSpacing(0)

        self._lightbox_image = QLabel(self._lightbox)
        self._lightbox_image.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._lightbox_image.setStyleSheet("background: transparent;")
        lb_layout.addWidget(self._lightbox_image, 1)

        self._lightbox_counter = QLabel("", self._lightbox)
        self._lightbox_counter.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._lightbox_counter.setFixedHeight(28)
        self._lightbox_counter.setStyleSheet(
            f"color: {theme.TEXT_PRIMARY}; font-size: 14px; font-weight: 700; background: transparent;"
        )
        self._scalable_labels.append((self._lightbox_counter, self._lightbox_counter.styleSheet()))
        lb_layout.addWidget(self._lightbox_counter)

        self._lightbox_hint = QWidget(self._lightbox)
        self._lightbox_hint.setFixedHeight(32)
        self._lightbox_hint.setStyleSheet("background: transparent;")
        _hint_layout = QHBoxLayout(self._lightbox_hint)
        _hint_layout.setContentsMargins(0, 0, 0, 0)
        _hint_layout.setSpacing(0)
        _hint_layout.addStretch()

        _btn_r_pix = QPixmap(str(_ASSETS / "input_BTN-R.png")).scaled(
            QSize(20, 20), Qt.AspectRatioMode.KeepAspectRatio,
            Qt.TransformationMode.SmoothTransformation,
        )
        _btn_r_lbl = QLabel(self._lightbox_hint)
        _btn_r_lbl.setPixmap(_btn_r_pix)
        _btn_r_lbl.setFixedSize(QSize(20, 20))
        _btn_r_lbl.setStyleSheet("background: transparent;")
        _hint_layout.addWidget(_btn_r_lbl)
        _hint_layout.addSpacing(4)
        _backspace_badge = QLabel("Backspace", self._lightbox_hint)
        _backspace_badge.setAlignment(Qt.AlignmentFlag.AlignCenter)
        _backspace_badge.setStyleSheet(
            f"color: {theme.TEXT_PRIMARY}; background: {theme.TERTIARY};"
            f"border: 1px solid {theme.BORDER_INACTIVE}; border-radius: 4px;"
            f"font-size: 10px; padding: 1px 6px;"
        )
        _hint_layout.addWidget(_backspace_badge)
        _hint_layout.addSpacing(8)
        _close_lbl = QLabel("Close", self._lightbox_hint)
        _close_lbl.setStyleSheet(
            f"color: {theme.TEXT_SECONDARY}; font-size: 12px; background: transparent;"
        )
        _hint_layout.addWidget(_close_lbl)

        _hint_layout.addSpacing(32)

        for _i, _dpad_file in enumerate(
            ("input_DPAD-U.png", "input_DPAD-D.png", "input_DPAD-L.png", "input_DPAD-R.png")
        ):
            if _i > 0:
                _hint_layout.addSpacing(4)
            _dp_pix = QPixmap(str(_ASSETS / _dpad_file)).scaled(
                QSize(20, 20), Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            )
            _dp_lbl = QLabel(self._lightbox_hint)
            _dp_lbl.setPixmap(_dp_pix)
            _dp_lbl.setFixedSize(QSize(20, 20))
            _dp_lbl.setStyleSheet("background: transparent;")
            _hint_layout.addWidget(_dp_lbl)
        _hint_layout.addSpacing(4)
        _arrows_badge = QLabel("←↑↓→", self._lightbox_hint)
        _arrows_badge.setAlignment(Qt.AlignmentFlag.AlignCenter)
        _arrows_badge.setStyleSheet(
            f"color: {theme.TEXT_PRIMARY}; background: {theme.TERTIARY};"
            f"border: 1px solid {theme.BORDER_INACTIVE}; border-radius: 4px;"
            f"font-size: 10px; padding: 1px 6px;"
        )
        _hint_layout.addWidget(_arrows_badge)
        _hint_layout.addSpacing(8)
        _nav_lbl = QLabel("Navigate", self._lightbox_hint)
        _nav_lbl.setStyleSheet(
            f"color: {theme.TEXT_SECONDARY}; font-size: 12px; background: transparent;"
        )
        _hint_layout.addWidget(_nav_lbl)

        _hint_layout.addStretch()
        lb_layout.addWidget(self._lightbox_hint)

        self._game_backend.installProgress.connect(self._on_install_progress)
        self._game_backend.installComplete.connect(self._on_install_complete)
        self._game_backend.uninstallComplete.connect(self._on_uninstall_complete)
        self._game_backend.launchError.connect(self._on_launch_error)
        self._game_backend.sessionStarted.connect(self._on_session_started)
        self._game_backend.sessionEnded.connect(self._on_session_ended)
        self._game_backend.nativeExecPickerNeeded.connect(self._on_native_exec_picker_needed)

        self._app_backend.romMetadataFetchStarted.connect(self._on_rom_metadata_fetch_started)
        self._app_backend.romMetadataReady.connect(self._on_rom_metadata_ready)
        self._app_backend.favoriteToggleComplete.connect(self._on_favorite_toggle_complete)

        self._refresh_fanart()
        self._refresh_screenshots()
        self._refresh_ui()
        QTimer.singleShot(0, self._update_font_scale)

        self._cover_loader.load_async(str(self._game.get("cover_url", "") or ""), self._on_cover_loaded)
        self._app_backend.logHandleDiag("details-open")
        self._app_backend.fetchRomMetadata(json.dumps(self._game))

    def handle_nav(self, direction: str) -> None:
        if self._lightbox.isVisible():
            if direction in ("back", "confirm"):
                self._close_lightbox()
            elif direction in ("left", "up"):
                if self._shot_index > 0:
                    self._shot_index -= 1
                    self._update_lightbox_image()
            elif direction in ("right", "down"):
                if self._shot_index < len(self._shot_pixmaps) - 1:
                    self._shot_index += 1
                    self._update_lightbox_image()
            return

        if self._cloud_overlay.isVisible():
            self._cloud_overlay.handle_nav(direction)
            return

        if self._native_picker.isVisible():
            self._native_picker.handle_nav(direction)
            return

        if direction == "back":
            self._pop_callback()
            return

        if direction == "left":
            if self._focused_column == 2:
                self._focused_column = 0
                self._shot_index = 0
                self._refresh_ui()
            return

        if direction == "right":
            if self._focused_column == 0:
                self._focused_column = 2
                self._shot_index = 0
                self._refresh_ui()
            return

        if direction == "up":
            if self._focused_column == 0:
                self._button_index = max(0, self._button_index - 1)
                self._refresh_ui()
                return
            if self._shot_cards:
                self._shot_index = max(0, self._shot_index - 1)
                self._sync_shot_focus()
                self._shots_scroll.ensureWidgetVisible(self._shot_cards[self._shot_index], 4, 4)
            return

        if direction == "down":
            if self._focused_column == 0:
                max_button = max(0, len(self._buttons) - 1)
                self._button_index = min(max_button, self._button_index + 1)
                self._refresh_ui()
                return
            if self._shot_cards:
                self._shot_index = min(len(self._shot_cards) - 1, self._shot_index + 1)
                self._sync_shot_focus()
                self._shots_scroll.ensureWidgetVisible(self._shot_cards[self._shot_index], 4, 4)
            return

        if direction == "confirm":
            if self._focused_column == 0:
                self._trigger_action()
            elif self._focused_column == 2 and self._shot_cards:
                self._open_lightbox()

    def eventFilter(self, watched: object, event: object) -> bool:
        if watched is self._shots_scroll.viewport():
            if isinstance(event, QWheelEvent):
                delta = event.angleDelta().y()
                if delta != 0:
                    if self._focused_column != 2:
                        self._focused_column = 2
                        self._refresh_ui()
                    self.handle_nav("up" if delta > 0 else "down")
                return True
            if isinstance(event, QEvent) and event.type() == QEvent.Type.Resize:
                self._reposition_shot_arrows()
        return super().eventFilter(watched, event)

    def _reposition_shot_arrows(self) -> None:
        vp = self._shots_scroll.viewport()
        w, h = vp.width(), vp.height()
        size = 32
        x = (w - size) // 2
        self._shot_up_arrow.move(x, 8)
        self._shot_down_arrow.move(x, h - size - 8)

    def _update_shot_arrows(self) -> None:
        self._reposition_shot_arrows()
        n = len(self._shot_cards)
        self._shot_up_arrow.setVisible(n > 0 and self._shot_index > 0)
        self._shot_down_arrow.setVisible(n > 0 and self._shot_index < n - 1)
        if self._shot_up_arrow.isVisible():
            self._shot_up_arrow.raise_()
        if self._shot_down_arrow.isVisible():
            self._shot_down_arrow.raise_()

    def intercepts_back(self) -> bool:
        return self._lightbox.isVisible()

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        rect = self.rect()
        self._fanart.setGeometry(rect)
        self._cloud_overlay.setGeometry(rect)
        self._native_picker.setGeometry(rect)
        self._lightbox.setGeometry(rect)
        self._update_lightbox_image()
        self._status_banner.setGeometry(16, 56, max(120, rect.width() - 32), 40)
        self._update_cover_pixmap()
        self._update_font_scale()

    def _update_font_scale(self) -> None:
        h = self.height()
        if h <= 0:
            return
        scale = max(720 / _TV_FONT_REF_HEIGHT, min(_TV_FONT_MAX_SCALE, h / _TV_FONT_REF_HEIGHT))
        self._current_font_scale = scale
        for label, base_stylesheet in self._scalable_labels:
            label.setStyleSheet(_apply_font_scale(base_stylesheet, scale))

    def _refresh_ui(self) -> None:
        self._header_title.setText(self._game_title())
        self._title_label.setText(self._game_title())
        self._desc_label.setText(self._description_text())

        installed_entry = self._get_installed_entry()
        installed = installed_entry is not None
        self._installed_local_path = str((installed_entry or {}).get("local_path", "") or "")
        connected = bool(getattr(self._app_backend, "isConnected", False))
        native_pc = self._is_native_pc_game()
        installing = bool(getattr(self._game_backend, "isInstallActive", False))

        self._buttons = self._build_buttons(installed, connected, native_pc, installing)
        self._button_index = min(self._button_index, max(0, len(self._buttons) - 1))
        self._rebuild_buttons()

        if installing:
            percent = int(round(self._install_progress * 100.0))
            kbps = max(0.0, self._install_speed / 1024.0)
            self._install_text.setText(f"Installing... {percent}%  {kbps:.0f} KB/s")
            self._progress_bar.set_progress(self._install_progress)
            self._install_text.setVisible(True)
            self._progress_bar.setVisible(True)
        else:
            self._install_text.setVisible(False)
            self._progress_bar.setVisible(False)

        self._update_metadata_panel()
        self._sync_column_focus_style()
        self._sync_shot_focus()

        self._fanart.lower()
        self._content.raise_()
        self._status_banner.raise_()
        if self._cloud_overlay.isVisible():
            self._cloud_overlay.raise_()
        if self._native_picker.isVisible():
            self._native_picker.raise_()
        if self._lightbox.isVisible():
            self._lightbox.raise_()

    def _build_buttons(self, installed: bool, connected: bool, native_pc: bool, installing: bool) -> list[dict[str, str]]:
        buttons: list[dict[str, str]] = []

        if installing:
            buttons.append({"id": "cancel", "icon": "svg/trashcan", "label": "Cancel"})
        elif installed:
            buttons.append({"id": "play", "icon": "svg/play", "label": "Play"})
        else:
            buttons.append({"id": "install", "icon": "svg/plus", "label": "Install"})

        if installed:
            buttons.append({"id": "uninstall", "icon": "svg/trashcan", "label": "Uninstall"})

        if installed and native_pc:
            buttons.append({"id": "native_exec", "icon": "svg/config", "label": "Change Executable"})

        if connected:
            buttons.append({"id": "cloud", "icon": "svg/cloud", "label": "Cloud Saves"})

            is_favorite = self._boolish(self._game.get("is_favorite", False))
            fav_label = "Remove from Favorites" if is_favorite else "Add to Favorites"
            buttons.append({"id": "favorite", "icon": "svg/star", "label": fav_label})

        return buttons

    def _rebuild_buttons(self) -> None:
        while self._button_layout.count() > 0:
            item = self._button_layout.takeAt(0)
            widget = item.widget()
            if widget is None:
                continue
            widget.hide()
            widget.setParent(None)
            widget.deleteLater()

        for idx, item in enumerate(self._buttons):
            focused = self._focused_column == 0 and idx == self._button_index
            bg = theme.ACCENT if focused else "rgba(68, 71, 90, 0.5)"
            fg = theme.PANEL if focused else theme.TEXT_PRIMARY

            row = QWidget(self._button_container)
            row.setObjectName("buttonRow")
            row.setFixedHeight(44)
            if focused:
                row.setStyleSheet(
                    f"QWidget#buttonRow {{ background: {bg}; border: 1px solid {theme.ACCENT}; border-radius: 8px; }}"
                )
            else:
                row.setStyleSheet(
                    f"QWidget#buttonRow {{ background: {bg}; border: none; border-radius: 8px; }}"
                )

            row_layout = QHBoxLayout(row)
            row_layout.setContentsMargins(12, 0, 12, 0)
            row_layout.setSpacing(8)

            icon_file = item.get("icon", "")
            if icon_file:
                if icon_file.endswith(".png"):
                    icon_pix = QPixmap(str(_ASSETS / icon_file)).scaled(
                        QSize(20, 20),
                        Qt.AspectRatioMode.KeepAspectRatio,
                        Qt.TransformationMode.SmoothTransformation,
                    )
                else:
                    icon_pix = themed_svg_pixmap(icon_file, fg, size=QSize(20, 20))
                icon_lbl = QLabel(row)
                icon_lbl.setPixmap(icon_pix)
                icon_lbl.setFixedSize(QSize(20, 20))
                icon_lbl.setStyleSheet("background: transparent;")
                row_layout.addWidget(icon_lbl)

            text_lbl = QLabel(item["label"], row)
            text_lbl.setStyleSheet(
                _apply_font_scale(
                    f"color: {fg}; font-size: 14px; font-weight: 700; background: transparent;",
                    self._current_font_scale,
                )
            )
            row_layout.addWidget(text_lbl, 1)

            self._button_layout.addWidget(row)

        self._button_layout.addStretch(1)

    def _sync_column_focus_style(self) -> None:
        self._left_col.setStyleSheet(
            "background: transparent; border: none; border-radius: 10px;"
        )
        self._center_col.setStyleSheet(
            "background: rgba(30, 31, 41, 0.82); border: none; border-radius: 10px;"
        )
        self._right_col.setStyleSheet(
            "background: rgba(30, 31, 41, 0.82); border: none; border-radius: 10px;"
        )

    def _sync_shot_focus(self) -> None:
        focused_in_shots = self._focused_column == 2
        for idx, card in enumerate(self._shot_cards):
            if focused_in_shots and idx == self._shot_index:
                card.setProperty("shotFocused", True)
                card.setStyleSheet(
                    f"background: rgba(30, 31, 41, 0.9); border: 2px solid {theme.ACCENT}; border-radius: 8px;"
                )
            else:
                card.setProperty("shotFocused", False)
                card.setStyleSheet(
                    "background: rgba(30, 31, 41, 0.9); border: 2px solid transparent; border-radius: 8px;"
                )
        self._update_shot_arrows()

    def _trigger_action(self) -> None:
        if not self._buttons:
            return
        if self._button_index < 0 or self._button_index >= len(self._buttons):
            return

        action_id = self._buttons[self._button_index]["id"]
        if action_id == "play":
            self._game_backend.launchGame(self._game)
            return

        if action_id == "install":
            if not bool(getattr(self._app_backend, "isConnected", False)):
                self._show_banner("Not connected to server.", success=False)
                return
            self._game_backend.installGame(self._game)
            return

        if action_id == "cancel":
            self._game_backend.cancelInstall()
            return

        if action_id == "uninstall":
            self._game_backend.uninstallGame(self._game)
            return

        if action_id == "native_exec":
            rom_id = self._rom_id()
            if not rom_id:
                self._show_banner("Game is missing ROM id.", success=False)
                return
            candidates = self._game_backend.getNativeExecutableCandidates(rom_id)
            if not isinstance(candidates, list) or not candidates:
                self._show_banner("No native executables found.", success=False)
                return
            current = str(self._game.get("native_executable_path", "") or "")
            self._native_picker.show_for_game(rom_id, candidates, current)
            return

        if action_id == "cloud":
            self._cloud_overlay.show_for_game(self._game, "save")
            return

        if action_id == "favorite":
            rom_id = self._rom_id()
            if rom_id:
                self._app_backend.toggleFavorite(rom_id)

    def _on_cover_loaded(self, pixmap: QPixmap | None) -> None:
        # Safety check: verify widget is still part of the widget tree before updating UI
        if self.parent() is None:
            return
        
        if pixmap is None or pixmap.isNull():
            self._cover_pixmap = None
            self._cover_label.setText("No Cover")
            self._cover_label.setStyleSheet(
                f"background: rgba(30, 31, 41, 0.85); border: none;"
                f"border-radius: 8px; color: {theme.TEXT_SECONDARY};"
            )
            return
        self._cover_pixmap = pixmap
        self._update_cover_pixmap()

    def _update_cover_pixmap(self) -> None:
        if self._cover_pixmap is None or self._cover_pixmap.isNull():
            return
        size = self._cover_label.size()
        if size.width() <= 0 or size.height() <= 0:
            return
        scaled = self._cover_pixmap.scaled(
            size,
            Qt.AspectRatioMode.KeepAspectRatio,
            Qt.TransformationMode.SmoothTransformation,
        )
        self._cover_label.setPixmap(scaled)

    def _update_lightbox_image(self) -> None:
        if not self._lightbox.isVisible():
            return
        total = len(self._shot_pixmaps)
        if self._shot_index < 0 or self._shot_index >= total:
            self._lightbox_image.clear()
            self._lightbox_counter.setText("")
            return
        self._lightbox_counter.setText(f"{self._shot_index + 1} of {total}")
        pixmap = self._shot_pixmaps[self._shot_index]
        if pixmap is None or pixmap.isNull():
            self._lightbox_image.clear()
            return
        available = self._lightbox_image.size()
        if available.width() <= 0 or available.height() <= 0:
            return
        scaled = pixmap.scaled(
            available,
            Qt.AspectRatioMode.KeepAspectRatio,
            Qt.TransformationMode.SmoothTransformation,
        )
        self._lightbox_image.setPixmap(scaled)

    def _open_lightbox(self) -> None:
        self._lightbox.setGeometry(self.rect())
        self._lightbox.show()
        self._lightbox.raise_()
        self.controlHintsChanged.emit()
        QTimer.singleShot(0, self._update_lightbox_image)

    def _close_lightbox(self) -> None:
        self._lightbox.hide()
        self.controlHintsChanged.emit()

    def _refresh_fanart(self) -> None:
        urls = self._screenshot_urls()
        self._fanart.set_urls(urls)

    def _refresh_screenshots(self) -> None:
        self._shot_cards: list[QLabel] = []
        self._shot_pixmaps = []
        while self._shots_layout.count() > 0:
            item = self._shots_layout.takeAt(0)
            widget = item.widget()
            if widget is None:
                continue
            widget.hide()
            widget.setParent(None)
            widget.deleteLater()

        urls = self._screenshot_urls()
        if not urls:
            self._shots_header.setText("No screenshots available")
            label = QLabel("No screenshots available", self._shots_container)
            label.setAlignment(Qt.AlignmentFlag.AlignCenter)
            label.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 13px;")
            self._shots_layout.addWidget(label)
            self._shots_layout.addStretch(1)
            return

        self._shots_header.setText("Screenshots")
        for url in urls:
            card_index = len(self._shot_cards)
            self._shot_pixmaps.append(None)

            card = QLabel(self._shots_container)
            card.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
            card.setAlignment(Qt.AlignmentFlag.AlignCenter)
            card.setStyleSheet(
                "background: rgba(30, 31, 41, 0.9); border: 2px solid transparent; border-radius: 8px;"
            )
            self._shots_layout.addWidget(card)
            self._shot_cards.append(card)

            def _set_shot(pixmap: QPixmap | None, target: QLabel = card, idx: int = card_index) -> None:
                if target.parent() is None:
                    return
                if pixmap is None or pixmap.isNull():
                    target.setText("Image unavailable")
                    target.setStyleSheet(
                        f"background: rgba(30, 31, 41, 0.9); border: 2px solid transparent;"
                        f"border-radius: 8px; color: {theme.TEXT_SECONDARY};"
                    )
                    return
                if idx < len(self._shot_pixmaps):
                    self._shot_pixmaps[idx] = pixmap
                card_w = max(1, target.width() or (self._right_col.width() - 24))
                card_h = max(60, int(card_w * pixmap.height() / pixmap.width())) if pixmap.width() > 0 else 90
                target.setFixedHeight(card_h)
                scaled = pixmap.scaled(
                    QSize(card_w, card_h),
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.SmoothTransformation,
                )
                target.setPixmap(scaled)

            self._cover_loader.load_async(url, _set_shot)

        self._shots_layout.addStretch(1)
        self._sync_shot_focus()

    def _update_metadata_panel(self) -> None:
        values = {
            "platform": str(self._game.get("platform", "") or "-"),
            "released": self._released_text(),
            "by": str(self._game.get("companies", "") or "-"),
            "version": str(self._game.get("revision", "") or "-"),
            "size": self._size_text(),
            "rating": self._rating_text(),
            "region": str(self._game.get("region", "") or "-"),
            "languages": str(self._game.get("languages", "") or "-"),
            "genres": str(self._game.get("genres", "") or "-"),
        }
        for key, value in values.items():
            label = self._metadata_labels.get(key)
            if label is None:
                continue
            label.setText(value)

    def _show_banner(self, text: str, success: bool) -> None:
        if not text:
            self._status_banner.hide()
            return
        bg = theme.SUCCESS if success else theme.ERROR
        fg = theme.PANEL
        self._status_banner.setText(text)
        self._status_banner.setStyleSheet(
            f"background: {bg}; color: {fg}; border-radius: 8px; font-size: 14px; font-weight: 700;"
        )
        self._status_banner.show()
        self._status_banner.raise_()
        self._status_timer.stop()
        self._status_timer.start(4000)

    def _scroll_area_by(self, area: QScrollArea, delta: int) -> None:
        bar = area.verticalScrollBar()
        bar.setValue(bar.value() + int(delta))

    def _get_installed_entry(self) -> dict | None:
        rom_id = self._rom_id()
        if not rom_id:
            return None
        games = getattr(self._app_backend, "libraryGames", []) or []
        for game in games:
            if not isinstance(game, dict):
                continue
            game_rom = str(game.get("rom_id") or game.get("id") or "").strip()
            if game_rom == rom_id:
                local = str(game.get("local_path", "") or "").strip()
                if local:
                    return dict(game)
        return None

    def _is_native_pc_game(self) -> bool:
        platform = str(self._game.get("platform", "") or "")
        return platform in ("Windows", "Windows 9x")

    def _rom_id(self) -> str:
        return str(self._game.get("rom_id") or self._game.get("id") or "").strip()

    def _game_title(self) -> str:
        return str(self._game.get("name") or self._game.get("title") or "Game Details")

    def _description_text(self) -> str:
        desc = str(self._game.get("description", "") or "").strip()
        if not desc:
            if self._metadata_loading:
                return "Loading metadata..."
            return "No description available."
        return desc

    def _released_text(self) -> str:
        value = str(self._game.get("first_release_date") or self._game.get("release_year") or "").strip()
        return value or "-"

    def _size_text(self) -> str:
        raw = self._game.get("filesize_bytes", "")
        try:
            size = float(raw)
        except (TypeError, ValueError):
            return "-"
        units = ["B", "KB", "MB", "GB", "TB"]
        idx = 0
        while size >= 1024.0 and idx < len(units) - 1:
            size /= 1024.0
            idx += 1
        if idx == 0:
            return f"{int(size)} {units[idx]}"
        return f"{size:.1f} {units[idx]}"

    def _rating_text(self) -> str:
        raw = self._game.get("rating", "")
        try:
            rating = float(raw)
        except (TypeError, ValueError):
            return "-"
        stars_value = rating if rating <= 5.0 else (rating / 20.0)
        stars_value = max(0.0, min(5.0, stars_value))
        stars = "*" * int(round(stars_value))
        if not stars:
            stars = "-"
        return stars

    def _screenshot_urls(self) -> list[str]:
        raw = self._game.get("screenshot_urls", "")
        if isinstance(raw, str):
            return [line.strip() for line in raw.split("\n") if line.strip()]
        if isinstance(raw, list):
            return [str(item).strip() for item in raw if str(item).strip()]
        return []

    @staticmethod
    def _boolish(value: Any) -> bool:
        if isinstance(value, bool):
            return value
        if isinstance(value, str):
            return value.strip().lower() in {"1", "true", "yes", "on"}
        if isinstance(value, (int, float)):
            return bool(value)
        return False

    def _on_install_progress(self, bundle: object) -> None:
        if not isinstance(bundle, dict):
            return
        downloaded = float(bundle.get("downloaded", 0) or 0)
        total = float(bundle.get("total", 0) or 0)
        speed = float(bundle.get("speed", 0) or 0)
        self._install_progress = (downloaded / total) if total > 0 else 0.0
        self._install_speed = speed
        self._refresh_ui()

    def _on_install_complete(self, bundle: object) -> None:
        success = bool(bundle.get("success", False)) if isinstance(bundle, dict) else False
        message = str(bundle.get("message", "")) if isinstance(bundle, dict) else "Install complete."
        game = bundle.get("game") if isinstance(bundle, dict) else None
        if success and isinstance(game, dict):
            self._game.update(game)
        self._show_banner(message, success)
        self._refresh_ui()

    def _on_uninstall_complete(self, bundle: object) -> None:
        success = bool(bundle.get("success", False)) if isinstance(bundle, dict) else False
        message = str(bundle.get("message", "")) if isinstance(bundle, dict) else "Uninstall complete."
        if success:
            self._game.pop("local_path", None)
            self._game.pop("extracted_path", None)
            self._game.pop("archive_path", None)
        self._show_banner(message, success)
        self._refresh_ui()

    def _on_launch_error(self, message: object) -> None:
        self._show_banner(str(message or "Launch failed."), success=False)

    def _on_session_started(self, emulator_name: object) -> None:
        name = str(emulator_name or "")
        text = f"Launched with {name}" if name else "Game launched"
        self._show_banner(text, success=True)

    def _on_session_ended(self, emulator_name: object) -> None:
        _ = emulator_name
        self._show_banner("Session ended", success=True)

    def _on_native_exec_picker_needed(self, candidates: object) -> None:
        if not isinstance(candidates, list) or not candidates:
            self._show_banner("No native executable candidates found.", success=False)
            return
        current = str(self._game.get("native_executable_path", "") or "")
        self._native_picker.show_for_game(self._rom_id(), candidates, current)

    def _on_rom_metadata_fetch_started(self, rom_id: object) -> None:
        if str(rom_id or "").strip() != self._rom_id():
            return
        self._metadata_loading = True
        self._refresh_ui()

    def _on_rom_metadata_ready(self, bundle: object) -> None:
        if not isinstance(bundle, dict):
            return
        rom_id = str(bundle.get("rom_id", "") or "").strip()
        if rom_id != self._rom_id():
            return

        metadata_json = str(bundle.get("metadata_json", "") or "")
        try:
            metadata = json.loads(metadata_json) if metadata_json else {}
        except (json.JSONDecodeError, TypeError):
            metadata = {}

        if isinstance(metadata, dict):
            self._game.update({k: v for k, v in metadata.items() if v is not None})

        self._metadata_loading = False
        self._refresh_fanart()
        self._refresh_screenshots()
        self._refresh_ui()

    def _on_favorite_toggle_complete(self, bundle: object) -> None:
        if not isinstance(bundle, dict):
            return
        rom_id = str(bundle.get("rom_id", "") or "").strip()
        if rom_id != self._rom_id():
            return
        self._game["is_favorite"] = bool(bundle.get("is_now_favorite", False))
        self._refresh_ui()
