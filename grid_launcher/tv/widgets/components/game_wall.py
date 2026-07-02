from __future__ import annotations

from collections.abc import Callable

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import QFrame, QGridLayout, QHBoxLayout, QWidget

from grid_launcher.tv.widgets.components.game_card import GameCard
from grid_launcher.tv.widgets.components.nav_scroll_area import NavScrollArea
from grid_launcher.tv.widgets.components.scrollbar import TvScrollBar
from grid_launcher.tv.widgets.cover_loader import CoverLoader


class GameWall(QWidget):
    game_selected = Signal(object)

    def __init__(self, cover_loader: CoverLoader, columns: int = 4, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._cover_loader = cover_loader
        self._columns = max(1, int(columns))
        self._games: list[dict] = []
        self._cards: list[GameCard] = []
        self._current_idx = 0
        self._current_batch_id = None  # Track current batch for callback cancellation
        self._populated: set[int] = set()
        self._scroll_handler: Callable | None = None

        root_layout = QHBoxLayout(self)
        root_layout.setContentsMargins(40, 40, 16, 40)
        root_layout.setSpacing(12)

        self._scroll_area = NavScrollArea(self)
        self._scroll_area.setWidgetResizable(True)
        self._scroll_area.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._scroll_area.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._scroll_area.setFrameShape(QFrame.Shape.NoFrame)

        self._container = QWidget(self._scroll_area)
        self._grid = QGridLayout(self._container)
        self._grid.setContentsMargins(0, 0, 0, 0)
        self._grid.setHorizontalSpacing(16)
        self._grid.setVerticalSpacing(18)
        self._grid.setAlignment(Qt.AlignmentFlag.AlignHCenter | Qt.AlignmentFlag.AlignTop)

        self._scroll_area.setWidget(self._container)
        root_layout.addWidget(self._scroll_area, 1)

        self._scrollbar = TvScrollBar(Qt.Orientation.Vertical, self)
        root_layout.addWidget(self._scrollbar, 0, Qt.AlignmentFlag.AlignVCenter)

        vbar = self._scroll_area.verticalScrollBar()
        vbar.rangeChanged.connect(self._scrollbar.set_range)
        vbar.valueChanged.connect(self._scrollbar.set_value)
        self._scrollbar.set_range(vbar.minimum(), vbar.maximum())
        self._scrollbar.set_value(vbar.value())

    def set_games(self, games: list[dict]) -> None:
        vbar = self._scroll_area.verticalScrollBar()
        if self._scroll_handler is not None:
            try:
                vbar.valueChanged.disconnect(self._scroll_handler)
            except RuntimeError:
                pass
            self._scroll_handler = None

        if self._current_batch_id is not None:
            self._cover_loader.cancel_batch(self._current_batch_id)
        self._current_batch_id = self._cover_loader.create_batch()

        self._games = [game for game in (games or []) if isinstance(game, dict)]
        self._populated = set()
        self._current_idx = 0
        self._clear_grid()
        self._cards = []

        for i, game in enumerate(self._games):
            card = GameCard(self._container)
            card.selected.connect(self.game_selected.emit)
            self._grid.addWidget(card, i // self._columns, i % self._columns)
            self._cards.append(card)

        if self._cards:
            self._focus_card(0)

        vbar.valueChanged.connect(self._upgrade_visible_tv_cards)
        self._scroll_handler = self._upgrade_visible_tv_cards
        self._upgrade_visible_tv_cards()

    def _upgrade_visible_tv_cards(self) -> None:
        if len(self._populated) == len(self._games):
            return

        vbar = self._scroll_area.verticalScrollBar()
        visible_top = vbar.value()
        visible_bottom = visible_top + self._scroll_area.viewport().height()
        card_h = 480
        v_spacing = 18
        row_h = card_h + v_spacing

        min_row = max(0, visible_top // row_h)
        max_row = max(0, visible_bottom // row_h) + 1

        for row in range(min_row, max_row + 1):
            for col in range(self._columns):
                idx = row * self._columns + col
                if idx >= len(self._cards) or idx in self._populated:
                    continue
                self._populated.add(idx)
                card = self._cards[idx]
                game = self._games[idx]
                card.set_game(game)
                self._cover_loader.load_async(
                    str(game.get("cover_url", "") or ""),
                    card.set_pixmap,
                    batch_id=self._current_batch_id,
                )

        if len(self._populated) == len(self._games) and self._scroll_handler is not None:
            try:
                self._scroll_area.verticalScrollBar().valueChanged.disconnect(self._scroll_handler)
            except RuntimeError:
                pass
            self._scroll_handler = None

    def handle_nav(self, direction: str) -> None:
        if not self._cards:
            return

        next_idx = self._current_idx
        if direction == "up":
            next_idx -= self._columns
        elif direction == "down":
            next_idx += self._columns
        elif direction == "left":
            next_idx -= 1
        elif direction == "right":
            next_idx += 1
        elif direction == "confirm":
            self.game_selected.emit(self._games[self._current_idx])
            return
        else:
            return

        next_idx = max(0, min(len(self._cards) - 1, next_idx))
        if next_idx != self._current_idx:
            self._focus_card(next_idx)

    def focus_first(self) -> None:
        if not self._cards:
            return
        self._focus_card(0)

    def refocus(self) -> None:
        if not self._cards:
            return
        self._focus_card(self._current_idx)

    def _focus_card(self, idx: int) -> None:
        if idx < 0 or idx >= len(self._cards):
            return
        if 0 <= self._current_idx < len(self._cards) and self._current_idx != idx:
            self._cards[self._current_idx].set_focused(False)
        self._current_idx = idx
        card = self._cards[idx]
        card.set_focused(True)
        self._scroll_area.ensureWidgetVisible(card)

    def _compute_columns(self) -> int:
        usable = self.width() - 80 - 24
        result = usable // (280 + 16)
        return max(1, result)

    def _rebuild_grid(self) -> None:
        self._clear_grid(delete_cards=False)
        for idx, card in enumerate(self._cards):
            row = idx // self._columns
            col = idx % self._columns
            self._grid.addWidget(card, row, col)
        self._upgrade_visible_tv_cards()  # populate newly visible cards after re-layout

    def _clear_grid(self, delete_cards: bool = True) -> None:
        while self._grid.count() > 0:
            item = self._grid.takeAt(0)
            widget = item.widget()
            if widget is None:
                continue
            if delete_cards:
                widget.hide()
                widget.setParent(None)
                widget.deleteLater()

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        new_cols = self._compute_columns()
        if new_cols != self._columns:
            self._columns = new_cols
            self._rebuild_grid()