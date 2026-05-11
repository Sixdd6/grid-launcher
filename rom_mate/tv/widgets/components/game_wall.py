from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import QFrame, QGridLayout, QHBoxLayout, QScrollArea, QWidget

from rom_mate.tv.widgets.components.game_card import GameCard
from rom_mate.tv.widgets.components.scrollbar import TvScrollBar
from rom_mate.tv.widgets.cover_loader import CoverLoader


class GameWall(QWidget):
    game_selected = Signal(object)

    def __init__(self, cover_loader: CoverLoader, columns: int = 4, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._cover_loader = cover_loader
        self._columns = max(1, int(columns))
        self._games: list[dict] = []
        self._cards: list[GameCard] = []
        self._current_idx = 0

        root_layout = QHBoxLayout(self)
        root_layout.setContentsMargins(40, 40, 16, 40)
        root_layout.setSpacing(12)

        self._scroll_area = QScrollArea(self)
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
        self._games = [game for game in (games or []) if isinstance(game, dict)]
        self._current_idx = 0
        self._clear_grid()

        self._cards = []
        for idx, game in enumerate(self._games):
            card = GameCard(self._container)
            card.set_game(game)
            self._cover_loader.load_async(str(game.get("cover_url", "") or ""), card.set_pixmap)
            card.selected.connect(self.game_selected.emit)

            row = idx // self._columns
            col = idx % self._columns
            self._grid.addWidget(card, row, col)
            self._cards.append(card)

        if self._cards:
            self._focus_card(0)

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

    def _focus_card(self, idx: int) -> None:
        if idx < 0 or idx >= len(self._cards):
            return
        self._current_idx = idx
        card = self._cards[idx]
        card.setFocus()
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

    def _clear_grid(self, delete_cards: bool = True) -> None:
        while self._grid.count() > 0:
            item = self._grid.takeAt(0)
            widget = item.widget()
            if widget is None:
                continue
            if delete_cards:
                widget.setParent(None)
                widget.deleteLater()

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        new_cols = self._compute_columns()
        if new_cols != self._columns:
            self._columns = new_cols
            self._rebuild_grid()