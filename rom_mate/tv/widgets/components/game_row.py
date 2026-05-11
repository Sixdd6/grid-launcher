from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import QFrame, QHBoxLayout, QLabel, QScrollArea, QVBoxLayout, QWidget

from rom_mate.tv.widgets import theme
from rom_mate.tv.widgets.components.home_card import HomeCard
from rom_mate.tv.widgets.cover_loader import CoverLoader


class GameRow(QWidget):
    game_selected = Signal(object)
    active_game_changed = Signal(object)

    def __init__(
        self,
        section_title: str,
        cover_loader: CoverLoader,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self._cover_loader = cover_loader
        self._focused_index = 0
        self._games: list[dict] = []
        self._cards: list[HomeCard] = []

        layout = QVBoxLayout(self)
        layout.setContentsMargins(96, 16, 96, 16)
        layout.setSpacing(16)

        self._title_label = QLabel(section_title)
        self._title_label.setStyleSheet(f"background: transparent; color: {theme.PURPLE}; font-size: 28px; font-weight: 700;")
        layout.addWidget(self._title_label)

        self._scroll_area = QScrollArea(self)
        self._scroll_area.setWidgetResizable(True)
        self._scroll_area.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._scroll_area.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._scroll_area.setFrameShape(QFrame.Shape.NoFrame)
        self._scroll_area.setStyleSheet("background: transparent; border: none;")
        self._scroll_area.viewport().setStyleSheet("background: transparent;")

        self._cards_container = QWidget(self._scroll_area)
        self._cards_container.setStyleSheet("background: transparent;")
        self._cards_layout = QHBoxLayout(self._cards_container)
        self._cards_layout.setContentsMargins(0, 0, 0, 0)
        self._cards_layout.setSpacing(32)
        self._cards_layout.addStretch(1)

        self._scroll_area.setWidget(self._cards_container)
        layout.addWidget(self._scroll_area)

    def set_games(self, games: list[dict]) -> None:
        # Preserve focused game identity and Qt focus ownership across refresh
        focused_key: str = ""
        if self._games and 0 <= self._focused_index < len(self._games):
            g = self._games[self._focused_index]
            focused_key = str(g.get("rom_id") or g.get("id") or g.get("name") or "").strip()

        self._games = [game for game in (games or []) if isinstance(game, dict)]

        # Try to restore the previously focused position by matching identity
        restored_index = 0
        if focused_key:
            for i, game in enumerate(self._games):
                key = str(game.get("rom_id") or game.get("id") or game.get("name") or "").strip()
                if key == focused_key:
                    restored_index = i
                    break
        self._focused_index = restored_index

        while self._cards_layout.count() > 1:
            item = self._cards_layout.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.hide()
                widget.setParent(None)
                widget.deleteLater()

        self._cards = []
        for game in self._games:
            card = HomeCard(self._cards_container)
            card.set_game(game)
            self._cover_loader.load_async(str(game.get("cover_url", "") or ""), card.set_pixmap)
            card.selected.connect(self.game_selected.emit)
            self._cards_layout.insertWidget(self._cards_layout.count() - 1, card)
            self._cards.append(card)

        if self._games:
            self.active_game_changed.emit(self._games[self._focused_index])

    def handle_nav(self, direction: str) -> None:
        if not self._cards:
            return

        if direction == "left":
            self._focused_index = max(0, self._focused_index - 1)
            self._focus_current_card()
            return

        if direction == "right":
            self._focused_index = min(len(self._cards) - 1, self._focused_index + 1)
            self._focus_current_card()
            return

        if direction == "confirm":
            self.game_selected.emit(self._games[self._focused_index])

    def focus_first_card(self) -> None:
        if not self._cards:
            return
        self._focused_index = 0
        self._focus_current_card()

    def refocus(self) -> None:
        """Re-apply Qt focus to the currently focused card without resetting the index."""
        if not self._cards:
            return
        self._focus_current_card()

    @property
    def focused_index(self) -> int:
        return self._focused_index

    def focus_at_index(self, idx: int) -> None:
        if not self._cards:
            return
        self._focused_index = max(0, min(len(self._cards) - 1, idx))
        self._focus_current_card()

    def focused_card_screen_x(self) -> int:
        """Return the screen x-center of the currently focused card, or -1 if no cards."""
        if not self._cards or self._focused_index >= len(self._cards):
            return -1
        card = self._cards[self._focused_index]
        return card.mapToGlobal(card.rect().center()).x()

    def focus_nearest_to_screen_x(self, screen_x: int) -> None:
        """Focus the card whose screen x-center is closest to screen_x."""
        if not self._cards:
            return
        best_idx = 0
        best_dist = float("inf")
        for i, card in enumerate(self._cards):
            cx = card.mapToGlobal(card.rect().center()).x()
            dist = abs(cx - screen_x)
            if dist < best_dist:
                best_dist = dist
                best_idx = i
        self._focused_index = best_idx
        self._focus_current_card()

    def _focus_current_card(self) -> None:
        if not self._cards:
            return
        card = self._cards[self._focused_index]
        card.setFocus()
        self._scroll_area.ensureWidgetVisible(card)
        self.active_game_changed.emit(self._games[self._focused_index])
