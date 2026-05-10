from __future__ import annotations

from collections.abc import Callable

from PySide6.QtCore import Qt
from PySide6.QtGui import QShowEvent
from PySide6.QtWidgets import QFrame, QGridLayout, QHBoxLayout, QLabel, QScrollArea, QStackedWidget, QVBoxLayout, QWidget

from rom_mate.cover.utils import resolve_cover_url
from rom_mate.tv.widgets import theme
from rom_mate.tv.widgets.components.game_wall import GameWall
from rom_mate.tv.widgets.components.platform_card import PlatformCard
from rom_mate.tv.widgets.components.scrollbar import TvScrollBar
from rom_mate.tv.widgets.cover_loader import CoverLoader


class ServerView(QWidget):
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
        self._columns = 4
        self._platforms: list[dict] = []
        self._platform_cards: list[PlatformCard] = []
        self._current_platform_idx = 0
        self._selected_platform = ""
        self._loading_games = False

        self.setStyleSheet(f"background: {theme.BG};")

        root_layout = QVBoxLayout(self)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        self._stack = QStackedWidget(self)
        root_layout.addWidget(self._stack)

        self._platform_page = QWidget(self)
        platform_root_layout = QVBoxLayout(self._platform_page)
        platform_root_layout.setContentsMargins(0, 0, 0, 0)
        platform_root_layout.setSpacing(0)

        platform_content = QWidget(self._platform_page)
        platform_content_layout = QHBoxLayout(platform_content)
        platform_content_layout.setContentsMargins(40, 40, 16, 40)
        platform_content_layout.setSpacing(12)

        self._platform_scroll = QScrollArea(platform_content)
        self._platform_scroll.setWidgetResizable(True)
        self._platform_scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._platform_scroll.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self._platform_scroll.setFrameShape(QFrame.Shape.NoFrame)

        self._platform_container = QWidget(self._platform_scroll)
        self._platform_grid = QGridLayout(self._platform_container)
        self._platform_grid.setContentsMargins(0, 0, 0, 0)
        self._platform_grid.setHorizontalSpacing(16)
        self._platform_grid.setVerticalSpacing(20)
        self._platform_grid.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)
        self._platform_scroll.setWidget(self._platform_container)
        platform_content_layout.addWidget(self._platform_scroll, 1)

        self._platform_scrollbar = TvScrollBar(Qt.Orientation.Vertical, platform_content)
        platform_content_layout.addWidget(self._platform_scrollbar, 0, Qt.AlignmentFlag.AlignVCenter)
        platform_root_layout.addWidget(platform_content, 1)

        self._platform_empty_label = QLabel("Connect to a RomM server to browse games", self._platform_page)
        self._platform_empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._platform_empty_label.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 18px;")
        platform_root_layout.addWidget(self._platform_empty_label, 1)

        self._game_page = QWidget(self)
        game_page_layout = QVBoxLayout(self._game_page)
        game_page_layout.setContentsMargins(0, 0, 0, 0)
        game_page_layout.setSpacing(0)

        self._game_wall = GameWall(self._cover_loader, columns=6, parent=self._game_page)
        game_page_layout.addWidget(self._game_wall)

        self._loading_label = QLabel("Loading...", self._game_page)
        self._loading_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._loading_label.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 20px;")
        self._loading_label.hide()

        self._stack.addWidget(self._platform_page)
        self._stack.addWidget(self._game_page)

        pbar = self._platform_scroll.verticalScrollBar()
        pbar.rangeChanged.connect(self._platform_scrollbar.set_range)
        pbar.valueChanged.connect(self._platform_scrollbar.set_value)
        self._platform_scrollbar.set_range(pbar.minimum(), pbar.maximum())
        self._platform_scrollbar.set_value(pbar.value())

        self._game_wall.game_selected.connect(self._on_game_selected)
        self._app_backend.platformsChanged.connect(self._refresh_platforms)
        self._app_backend.serverGamesChanged.connect(self._on_server_games_changed)

        self._refresh_platforms()

    def activate(self) -> None:
        self._refresh_platforms()
        if self._stack.currentIndex() == 0:
            self._focus_first_platform()
            return
        self._game_wall.focus_first()

    def handle_nav(self, direction: str) -> None:
        if self._stack.currentIndex() == 1:
            if direction == "back":
                self._selected_platform = ""
                self._loading_games = False
                self._stack.setCurrentIndex(0)
                self._sync_platform_empty_state()
                self._focus_first_platform()
                return
            self._game_wall.handle_nav(direction)
            return

        if not self._platform_cards:
            return

        if direction == "confirm":
            self._on_platform_selected(self._platforms[self._current_platform_idx])
            return

        next_idx = self._current_platform_idx
        if direction == "up":
            next_idx -= self._columns
        elif direction == "down":
            next_idx += self._columns
        elif direction == "left":
            next_idx -= 1
        elif direction == "right":
            next_idx += 1
        else:
            return

        next_idx = max(0, min(len(self._platform_cards) - 1, next_idx))
        if next_idx != self._current_platform_idx:
            self._focus_platform_card(next_idx)

    def _refresh_platforms(self) -> None:
        details = getattr(self._app_backend, "platformDetails", [])
        if not isinstance(details, list):
            details = []

        server_url = str(getattr(self._app_backend, "serverUrl", "") or "")
        normalized: list[dict] = []
        for item in details:
            if not isinstance(item, dict):
                continue
            name = str(item.get("name", "") or "").strip()
            if not name:
                continue
            url_logo = resolve_cover_url(item.get("url_logo"), server_url)
            normalized.append(
                {
                    **item,
                    "name": name,
                    "logo_url": url_logo,
                }
            )

        self._platforms = normalized
        self._current_platform_idx = 0
        self._clear_platform_grid()
        self._platform_cards = []

        for idx, platform in enumerate(self._platforms):
            card = PlatformCard(self._cover_loader, self._platform_container)
            card.set_platform(platform)
            card.selected.connect(self._on_platform_selected)
            row = idx // self._columns
            col = idx % self._columns
            self._platform_grid.addWidget(card, row, col)
            self._platform_cards.append(card)

        if self._stack.currentIndex() == 0:
            self._sync_platform_empty_state()

    def _sync_platform_empty_state(self) -> None:
        show_empty = len(self._platform_cards) == 0
        self._platform_empty_label.setVisible(show_empty)
        self._platform_scroll.setVisible(not show_empty)
        self._platform_scrollbar.setVisible(not show_empty)

    def _on_platform_selected(self, platform_payload: object) -> None:
        if not isinstance(platform_payload, dict):
            return
        platform_name = str(platform_payload.get("name", "") or "").strip()
        if not platform_name:
            return

        self._selected_platform = platform_name
        self._stack.setCurrentIndex(1)

        games = list(self._app_backend.serverGamesForPlatform(platform_name) or [])
        self._game_wall.set_games(games)

        self._loading_games = len(games) == 0
        self._sync_loading_state()

        self._app_backend.loadPlatformGames(platform_name)
        if games:
            self._game_wall.focus_first()

    def _on_server_games_changed(self, platform_label: object) -> None:
        label = str(platform_label or "")
        if not label or label != self._selected_platform:
            return

        games = list(self._app_backend.serverGamesForPlatform(label) or [])
        self._game_wall.set_games(games)
        self._loading_games = False
        self._sync_loading_state()
        if self._stack.currentIndex() == 1:
            self._game_wall.focus_first()

    def _on_game_selected(self, game: object) -> None:
        if not isinstance(game, dict):
            return
        from rom_mate.tv.widgets.views.details_view import DetailsView

        pop_callback = self._pop_view_callback
        if pop_callback is None:
            pop_callback = lambda: self._push_view_callback(None)  # type: ignore[arg-type]

        details_view = DetailsView(
            game,
            self._app_backend,
            self._cloud_backend,
            self._game_backend,
            self._pause_backend,
            self._controller_backend,
            self._cover_loader,
            pop_callback,
        )
        self._push_view_callback(details_view)

    def _focus_first_platform(self) -> None:
        if not self._platform_cards:
            return
        self._focus_platform_card(0)

    def _focus_platform_card(self, idx: int) -> None:
        if idx < 0 or idx >= len(self._platform_cards):
            return
        self._current_platform_idx = idx
        card = self._platform_cards[idx]
        card.setFocus()
        self._platform_scroll.ensureWidgetVisible(card)

    def _sync_loading_state(self) -> None:
        if self._loading_games:
            self._loading_label.show()
            self._game_wall.hide()
            return
        self._loading_label.hide()
        self._game_wall.show()

    def _clear_platform_grid(self) -> None:
        while self._platform_grid.count() > 0:
            item = self._platform_grid.takeAt(0)
            widget = item.widget()
            if widget is None:
                continue
            widget.setParent(None)
            widget.deleteLater()

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        self._loading_label.setGeometry(self._game_page.rect())

    def showEvent(self, event: QShowEvent) -> None:
        super().showEvent(event)
        self.activate()