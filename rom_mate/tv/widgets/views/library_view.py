from __future__ import annotations

from collections.abc import Callable

from PySide6.QtCore import QEasingCurve, QParallelAnimationGroup, QPropertyAnimation, QRect, Qt, Signal
from PySide6.QtGui import QColor, QLinearGradient, QPainter, QShowEvent
from PySide6.QtWidgets import QLabel, QWidget

from rom_mate.tv.widgets import theme
from rom_mate.tv.widgets.components.fanart_background import FanartBackground
from rom_mate.tv.widgets.components.library_card import LibraryCard
from rom_mate.tv.widgets.cover_loader import CoverLoader

_ROW_ANCHOR_RATIO = 0.67
_CARD_W = 200
_CARD_H = 300
_CARD_GAP = 24
_POOL_SIZE = 11
_FOCUS_SCALE = 1.20
_Y_BUFFER = 60
_SHRINK_ANIM_MS = 120
_SLIDE_DURATION_MS = 260
_SCALE_ANIM_MS = 190


class LibraryView(QWidget):
    game_selected = Signal(object)

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
        self._games: list[dict] = []
        self._current_idx: int = 0
        self._anim_blocked: bool = False
        self._card_scale_anim: QPropertyAnimation | None = None
        self._first_slot: int = 0
        self._nav_anim: QParallelAnimationGroup | None = None

        self.setStyleSheet(f"background: {theme.BG};")
        self._fanart = FanartBackground(cover_loader, self)

        self._strip_container = QWidget(self)
        self._strip_container.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)

        self._pool: list[LibraryCard] = []
        for i in range(_POOL_SIZE):
            card = LibraryCard(self._strip_container)
            card.setGeometry(i * (_CARD_W + _CARD_GAP), _Y_BUFFER, _CARD_W, _CARD_H)
            card.selected.connect(self.game_selected.emit)
            self._pool.append(card)

        self._empty_label = QLabel("No installed games", self)
        self._empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._empty_label.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 22px;")
        self._empty_label.hide()

        self._fanart.lower()
        self._strip_container.raise_()

        self._app_backend.libraryGamesChanged.connect(self._refresh)
        self.game_selected.connect(self._on_game_selected)
        self._refresh()

    def _card_at_slot(self, j: int) -> LibraryCard:
        return self._pool[(self._first_slot + j) % _POOL_SIZE]

    def _center_card(self) -> LibraryCard:
        return self._card_at_slot(_POOL_SIZE // 2)

    def _place_strip(self) -> None:
        if self.height() == 0:
            return
        w = self.width()
        h = self.height()
        strip_top = int(h * _ROW_ANCHOR_RATIO)
        strip_w = _POOL_SIZE * (_CARD_W + _CARD_GAP)
        strip_h = _CARD_H + 80
        center_slot = _POOL_SIZE // 2
        center_slot_center_x = center_slot * (_CARD_W + _CARD_GAP) + _CARD_W // 2
        container_x = w // 2 - center_slot_center_x
        self._strip_container.setGeometry(container_x, strip_top, strip_w, strip_h)

    def _bind_pool(self) -> None:
        if self._card_scale_anim is not None and self._card_scale_anim.state() == QPropertyAnimation.State.Running:
            self._card_scale_anim.stop()
            self._card_scale_anim = None
        center_slot = _POOL_SIZE // 2
        stride = _CARD_W + _CARD_GAP
        for j in range(_POOL_SIZE):
            card = self._card_at_slot(j)
            card.setGeometry(j * stride, _Y_BUFFER, _CARD_W, _CARD_H)
            game_idx = self._current_idx - center_slot + j
            if 0 <= game_idx < len(self._games):
                card.set_game(self._games[game_idx])
                self._cover_loader.load_async(
                    str(self._games[game_idx].get("cover_url", "") or ""),
                    card.set_pixmap,
                )
            else:
                card.set_game({})
            card.set_dimmed(j != center_slot)

    def _grow_center_card(self) -> None:
        if not self._games:
            return
        center = self._center_card()
        scaled_w = int(_CARD_W * _FOCUS_SCALE)
        scaled_h = int(_CARD_H * _FOCUS_SCALE)
        slot_x = (_POOL_SIZE // 2) * (_CARD_W + _CARD_GAP)
        start = QRect(slot_x, _Y_BUFFER, _CARD_W, _CARD_H)
        end_x = slot_x + (_CARD_W - scaled_w) // 2
        end_y = _Y_BUFFER + _CARD_H - scaled_h
        end = QRect(end_x, end_y, scaled_w, scaled_h)
        anim = QPropertyAnimation(center, b"geometry", self)
        anim.setDuration(_SCALE_ANIM_MS)
        anim.setStartValue(start)
        anim.setEndValue(end)
        anim.setEasingCurve(QEasingCurve.Type.OutQuint)
        self._card_scale_anim = anim
        anim.start()

    def _update_fanart(self) -> None:
        if not self._games or self._current_idx < 0 or self._current_idx >= len(self._games):
            self._fanart.set_urls([])
            return
        game = self._games[self._current_idx]
        raw = game.get("screenshot_urls") or game.get("url_screenshots") or ""
        if isinstance(raw, list):
            parsed_urls = [str(u).strip() for u in raw if str(u).strip()]
        else:
            parsed_urls = [u.strip() for u in str(raw).splitlines() if u.strip()]
        self._fanart.set_urls(parsed_urls)

    def activate(self) -> None:
        self._refresh()
        self.focus_first()

    def handle_nav(self, direction: str) -> None:
        if direction in ("up", "down"):
            return

        if direction == "left":
            if self._anim_blocked or not self._games:
                return
            new_idx = max(0, self._current_idx - 1)
            if new_idx == self._current_idx:
                return
            self._current_idx = new_idx
            self._update_fanart()
            self._start_nav_anim(direction)
        elif direction == "right":
            if self._anim_blocked or not self._games:
                return
            new_idx = min(len(self._games) - 1, self._current_idx + 1)
            if new_idx == self._current_idx:
                return
            self._current_idx = new_idx
            self._update_fanart()
            self._start_nav_anim(direction)
        elif direction == "confirm":
            if not self._games:
                return
            self.game_selected.emit(self._games[self._current_idx])

    def focus_first(self) -> None:
        self._current_idx = 0
        self._bind_pool()
        self._update_fanart()
        self._place_strip()
        self._grow_center_card()

    def _refresh(self) -> None:
        self._games = [g for g in (getattr(self._app_backend, "libraryGames", []) or []) if isinstance(g, dict)]
        self._current_idx = 0
        if not self._games:
            self._empty_label.show()
            self._strip_container.hide()
            self._fanart.set_urls([])
            return
        self._empty_label.hide()
        self._strip_container.show()
        self._bind_pool()
        self._update_fanart()
        self._place_strip()
        self._grow_center_card()

    def _start_nav_anim(self, direction: str) -> None:
        self._anim_blocked = True
        if self._card_scale_anim is not None and self._card_scale_anim.state() == QPropertyAnimation.State.Running:
            self._card_scale_anim.stop()
            self._card_scale_anim = None

        center = self._center_card()
        slot_x = (_POOL_SIZE // 2) * (_CARD_W + _CARD_GAP)
        base_rect = QRect(slot_x, _Y_BUFFER, _CARD_W, _CARD_H)
        current_rect = center.geometry()

        if current_rect == base_rect:
            self._complete_nav_anim(direction)
            return

        shrink = QPropertyAnimation(center, b"geometry", self)
        shrink.setDuration(_SHRINK_ANIM_MS)
        shrink.setStartValue(current_rect)
        shrink.setEndValue(base_rect)
        shrink.setEasingCurve(QEasingCurve.Type.InCubic)
        shrink.finished.connect(lambda: self._complete_nav_anim(direction))
        self._card_scale_anim = shrink
        shrink.start()

    def _complete_nav_anim(self, direction: str) -> None:
        if self.height() == 0:
            self._anim_blocked = False
            return

        self._card_scale_anim = None
        stride = _CARD_W + _CARD_GAP
        center_slot = _POOL_SIZE // 2
        scaled_w = int(_CARD_W * _FOCUS_SCALE)
        scaled_h = int(_CARD_H * _FOCUS_SCALE)
        grow_slot_x = center_slot * stride
        grow_end_x = grow_slot_x + (_CARD_W - scaled_w) // 2
        grow_end_y = _Y_BUFFER + _CARD_H - scaled_h
        grown_rect = QRect(grow_end_x, grow_end_y, scaled_w, scaled_h)

        if direction == "right":
            delta = -1
            new_center_slot = center_slot + 1
        else:
            delta = 1
            new_center_slot = center_slot - 1

        group = QParallelAnimationGroup(self)

        for j in range(_POOL_SIZE):
            card = self._card_at_slot(j)
            start_rect = QRect(j * stride, _Y_BUFFER, _CARD_W, _CARD_H)
            if j == new_center_slot:
                end_rect = grown_rect
                duration = _SCALE_ANIM_MS
                easing = QEasingCurve.Type.OutQuint
            else:
                end_rect = QRect((j + delta) * stride, _Y_BUFFER, _CARD_W, _CARD_H)
                duration = _SLIDE_DURATION_MS
                easing = QEasingCurve.Type.OutCubic

            anim = QPropertyAnimation(card, b"geometry", self)
            anim.setDuration(duration)
            anim.setStartValue(start_rect)
            anim.setEndValue(end_rect)
            anim.setEasingCurve(easing)
            group.addAnimation(anim)

        group.finished.connect(lambda: self._on_nav_finished(direction))
        self._nav_anim = group
        group.start()

    def _on_nav_finished(self, direction: str) -> None:
        self._anim_blocked = False
        self._nav_anim = None
        stride = _CARD_W + _CARD_GAP
        center_slot = _POOL_SIZE // 2

        if direction == "right":
            recycle_card = self._card_at_slot(0)
            self._first_slot = (self._first_slot + 1) % _POOL_SIZE
            recycle_card.move(10 * stride, _Y_BUFFER)
            game_idx = self._current_idx + center_slot
            if 0 <= game_idx < len(self._games):
                recycle_card.set_game(self._games[game_idx])
                self._cover_loader.load_async(
                    str(self._games[game_idx].get("cover_url", "") or ""),
                    recycle_card.set_pixmap,
                )
            else:
                recycle_card.set_game({})
            recycle_card.set_dimmed(True)
        else:
            recycle_card = self._card_at_slot(10)
            self._first_slot = (self._first_slot - 1 + _POOL_SIZE) % _POOL_SIZE
            recycle_card.move(0, _Y_BUFFER)
            game_idx = self._current_idx - center_slot
            if 0 <= game_idx < len(self._games):
                recycle_card.set_game(self._games[game_idx])
                self._cover_loader.load_async(
                    str(self._games[game_idx].get("cover_url", "") or ""),
                    recycle_card.set_pixmap,
                )
            else:
                recycle_card.set_game({})
            recycle_card.set_dimmed(True)

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        self._fanart.setGeometry(self.rect())
        self._empty_label.setGeometry(self.rect())
        if self._nav_anim is not None:
            self._nav_anim.stop()
            self._nav_anim = None
            self._anim_blocked = False
        if self._card_scale_anim is not None:
            self._card_scale_anim.stop()
            self._card_scale_anim = None
        self._bind_pool()
        self._place_strip()

    def paintEvent(self, event) -> None:
        super().paintEvent(event)
        h = self.height()
        w = self.width()
        if h == 0:
            return
        painter = QPainter(self)
        grad = QLinearGradient(0, h * 0.55, 0, h)
        grad.setColorAt(0.0, QColor(13, 13, 13, 0))
        grad.setColorAt(1.0, QColor(13, 13, 13, 220))
        painter.fillRect(0, int(h * 0.55), w, h - int(h * 0.55), grad)
        painter.end()

    def showEvent(self, event: QShowEvent) -> None:
        super().showEvent(event)
        self._refresh()
        self.focus_first()

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