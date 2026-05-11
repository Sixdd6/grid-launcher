from __future__ import annotations

from collections.abc import Callable

from PySide6.QtCore import QEasingCurve, QParallelAnimationGroup, QPropertyAnimation, QRect, Qt, QTimer
from PySide6.QtGui import QColor, QLinearGradient, QPainter, QShowEvent
from PySide6.QtWidgets import QWidget

from rom_mate.tv.widgets import theme
from rom_mate.tv.widgets.components.fanart_background import FanartBackground
from rom_mate.tv.widgets.components.game_row import GameRow
from rom_mate.tv.widgets.cover_loader import CoverLoader


_ROW_STRIP_HEIGHT = 384
_ROW_ANCHOR_RATIO = 0.70
_ANIM_DURATION_MS = 300


class HomeView(QWidget):
    def __init__(
        self,
        app_backend,
        cloud_backend,
        game_backend,
        pause_backend,
        controller_backend,
        cover_loader: CoverLoader,
        push_view_callback: Callable[[QWidget], None],
        pop_view_callback: Callable[[], None] | None = None,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self._app_backend = app_backend
        self._cloud_backend = cloud_backend
        self._game_backend = game_backend
        self._pause_backend = pause_backend
        self._controller_backend = controller_backend
        self._cover_loader = cover_loader
        self._push_view_callback = push_view_callback
        self._pop_view_callback = pop_view_callback
        self._active_row = 0
        self._did_focus_initial = False
        self._row_anim: QParallelAnimationGroup | None = None
        self._anim_blocked = False

        self.setObjectName("home_view")
        self.setStyleSheet(f"QWidget#home_view {{ background: {theme.BG}; }}")

        self._fanart = FanartBackground(self._cover_loader, self)

        self._rows_container = QWidget(self)
        self._rows_container.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)

        self._rows: list[GameRow] = [
            GameRow("Continue Playing", self._cover_loader, self._rows_container),
            GameRow("Favorites", self._cover_loader, self._rows_container),
            GameRow("New Additions", self._cover_loader, self._rows_container),
            GameRow("Highly Rated", self._cover_loader, self._rows_container),
        ]

        self._rows[0].game_selected.connect(self._on_game_selected)
        self._rows[1].game_selected.connect(self._on_game_selected)
        self._rows[2].game_selected.connect(self._on_game_selected)
        self._rows[3].game_selected.connect(self._on_game_selected)

        self._rows[0].active_game_changed.connect(self._on_active_game_changed)
        self._rows[1].active_game_changed.connect(self._on_active_game_changed)
        self._rows[2].active_game_changed.connect(self._on_active_game_changed)
        self._rows[3].active_game_changed.connect(self._on_active_game_changed)

        self._library_row_debounce = QTimer(self)
        self._library_row_debounce.setSingleShot(True)
        self._library_row_debounce.setInterval(80)
        self._library_row_debounce.timeout.connect(self._do_refresh_library_row)

        self._favorites_row_debounce = QTimer(self)
        self._favorites_row_debounce.setSingleShot(True)
        self._favorites_row_debounce.setInterval(80)
        self._favorites_row_debounce.timeout.connect(self._do_refresh_favorites_row)

        self._new_additions_row_debounce = QTimer(self)
        self._new_additions_row_debounce.setSingleShot(True)
        self._new_additions_row_debounce.setInterval(80)
        self._new_additions_row_debounce.timeout.connect(self._do_refresh_new_additions_row)

        self._highly_rated_row_debounce = QTimer(self)
        self._highly_rated_row_debounce.setSingleShot(True)
        self._highly_rated_row_debounce.setInterval(80)
        self._highly_rated_row_debounce.timeout.connect(self._do_refresh_highly_rated_row)

        self._app_backend.libraryGamesChanged.connect(self._refresh_library_row)
        self._app_backend.favoritesGamesChanged.connect(self._refresh_favorites_row)
        self._app_backend.newAdditionsGamesChanged.connect(self._refresh_new_additions_row)
        self._app_backend.highlyRatedGamesChanged.connect(self._refresh_highly_rated_row)

        self._refresh_library_row()
        self._refresh_favorites_row()
        self._refresh_new_additions_row()
        self._refresh_highly_rated_row()

        self._fanart.lower()
        self._rows_container.raise_()
        self._place_rows()

    def _place_rows(self) -> None:
        if self.height() == 0:
            return
        w = self.width()
        strip_y = int(round(self.height() * _ROW_ANCHOR_RATIO))
        for i, row in enumerate(self._rows):
            if i == self._active_row:
                row.setGeometry(0, strip_y, w, _ROW_STRIP_HEIGHT)
            elif i < self._active_row:
                row.setGeometry(0, -_ROW_STRIP_HEIGHT, w, _ROW_STRIP_HEIGHT)
            else:
                row.setGeometry(0, self.height(), w, _ROW_STRIP_HEIGHT)
            row.setVisible(True)

    def handle_nav(self, direction: str) -> None:
        if direction == "up":
            if self._anim_blocked:
                return
            new_index = max(0, self._active_row - 1)
            if new_index == self._active_row:
                return
            screen_x = self._rows[self._active_row].focused_card_screen_x()
            self._animate_to_row(new_index, direction, screen_x)
            return

        if direction == "down":
            if self._anim_blocked:
                return
            new_index = min(len(self._rows) - 1, self._active_row + 1)
            if new_index == self._active_row:
                return
            screen_x = self._rows[self._active_row].focused_card_screen_x()
            self._animate_to_row(new_index, direction, screen_x)
            return

        if direction in ("left", "right", "confirm"):
            self._rows[self._active_row].handle_nav(direction)

    def _animate_to_row(self, new_index: int, direction: str, screen_x: int) -> None:
        self._anim_blocked = True
        w = self.width()
        strip_y = int(round(self.height() * _ROW_ANCHOR_RATIO))
        h = self.height()

        current_row = self._rows[self._active_row]
        incoming_row = self._rows[new_index]

        if direction == "up":
            current_end = QRect(0, h, w, _ROW_STRIP_HEIGHT)
            incoming_start = QRect(0, -_ROW_STRIP_HEIGHT, w, _ROW_STRIP_HEIGHT)
        else:
            current_end = QRect(0, -_ROW_STRIP_HEIGHT, w, _ROW_STRIP_HEIGHT)
            incoming_start = QRect(0, h, w, _ROW_STRIP_HEIGHT)
        incoming_end = QRect(0, strip_y, w, _ROW_STRIP_HEIGHT)

        incoming_row.setGeometry(incoming_start)

        current_anim = QPropertyAnimation(current_row, b"geometry", self)
        current_anim.setStartValue(current_row.geometry())
        current_anim.setEndValue(current_end)
        current_anim.setDuration(_ANIM_DURATION_MS)
        current_anim.setEasingCurve(QEasingCurve.Type.OutCubic)

        incoming_anim = QPropertyAnimation(incoming_row, b"geometry", self)
        incoming_anim.setStartValue(incoming_start)
        incoming_anim.setEndValue(incoming_end)
        incoming_anim.setDuration(_ANIM_DURATION_MS)
        incoming_anim.setEasingCurve(QEasingCurve.Type.OutCubic)

        group = QParallelAnimationGroup(self)
        group.addAnimation(current_anim)
        group.addAnimation(incoming_anim)

        self._active_row = new_index
        group.finished.connect(lambda: self._on_row_anim_finished(screen_x))

        self._row_anim = group
        group.start()

    def _on_row_anim_finished(self, screen_x: int) -> None:
        self._anim_blocked = False
        self._row_anim = None
        self._place_rows()
        active_row = self._rows[self._active_row]
        if screen_x >= 0:
            active_row.focus_nearest_to_screen_x(screen_x)
        else:
            active_row.focus_first_card()

    def focus_default_row(self) -> None:
        self._active_row = 0
        self._rows[0].focus_first_card()
        self._place_rows()
        self._did_focus_initial = True

    def _refresh_library_row(self) -> None:
        self._library_row_debounce.start()

    def _do_refresh_library_row(self) -> None:
        library_games = list(getattr(self._app_backend, "libraryGames", []) or [])
        recent = list(reversed(library_games))[:20]
        self._rows[0].set_games(recent)

    def _refresh_favorites_row(self) -> None:
        self._favorites_row_debounce.start()

    def _do_refresh_favorites_row(self) -> None:
        self._rows[1].set_games(list(getattr(self._app_backend, "favoritesGames", []) or []))

    def _refresh_new_additions_row(self) -> None:
        self._new_additions_row_debounce.start()

    def _do_refresh_new_additions_row(self) -> None:
        self._rows[2].set_games(list(getattr(self._app_backend, "newAdditionsGames", []) or []))

    def _refresh_highly_rated_row(self) -> None:
        self._highly_rated_row_debounce.start()

    def _do_refresh_highly_rated_row(self) -> None:
        self._rows[3].set_games(list(getattr(self._app_backend, "highlyRatedGames", []) or []))

    def _on_active_game_changed(self, game_dict: object) -> None:
        game = game_dict if isinstance(game_dict, dict) else {}
        raw = game.get("screenshot_urls") or game.get("url_screenshots") or ""
        if isinstance(raw, list):
            parsed_urls = [str(u).strip() for u in raw if str(u).strip()]
        else:
            parsed_urls = [u.strip() for u in str(raw).splitlines() if u.strip()]
        self._fanart.set_urls(parsed_urls)

    def _on_game_selected(self, game_dict: object) -> None:
        if not isinstance(game_dict, dict):
            return
        from rom_mate.tv.widgets.views.details_view import DetailsView

        pop_callback = self._pop_view_callback
        if pop_callback is None:
            pop_callback = lambda: self._push_view_callback(None)  # type: ignore[arg-type]

        details_view = DetailsView(
            game_dict,
            self._app_backend,
            self._cloud_backend,
            self._game_backend,
            self._pause_backend,
            self._controller_backend,
            self._cover_loader,
            pop_callback,
        )
        self._push_view_callback(details_view)

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        rect = self.rect()
        self._fanart.setGeometry(rect)
        self._rows_container.setGeometry(rect)
        if self._row_anim is not None:
            self._row_anim.stop()
            self._row_anim = None
            self._anim_blocked = False
        self._place_rows()

    def paintEvent(self, event) -> None:
        super().paintEvent(event)
        h = self.height()
        w = self.width()
        if h == 0:
            return
        painter = QPainter(self)
        grad = QLinearGradient(0, h - 600, 0, h)
        grad.setColorAt(0.0, QColor(13, 13, 13, 0))
        grad.setColorAt(1.0, QColor(13, 13, 13, 220))
        painter.fillRect(0, h - 600, w, 600, grad)
        self._draw_dots(painter)
        painter.end()

    def _draw_dots(self, painter: QPainter) -> None:
        dot_count = len(self._rows)
        inactive_h = 6
        active_h = 16
        dot_w = 6
        spacing = 12

        heights = [active_h if i == self._active_row else inactive_h for i in range(dot_count)]
        total_h = sum(heights) + spacing * (dot_count - 1)
        y = (self.height() - total_h) // 2
        x = self.width() - 48 - dot_w

        painter.setPen(Qt.PenStyle.NoPen)
        for i in range(dot_count):
            item_h = heights[i]
            if i == self._active_row:
                painter.setBrush(QColor(232, 232, 232, 255))
                painter.drawRoundedRect(QRect(x, y, dot_w, item_h), 3, 3)
            else:
                painter.setBrush(QColor(255, 255, 255, 77))
                painter.drawEllipse(QRect(x, y, dot_w, item_h))
            y += item_h + spacing

    def showEvent(self, event: QShowEvent) -> None:
        super().showEvent(event)
        if not self._did_focus_initial:
            self.focus_default_row()
        else:
            self._rows[self._active_row].refocus()
