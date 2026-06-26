from __future__ import annotations

import math
from typing import Any, Callable, Protocol

from PySide6.QtWidgets import QGridLayout, QListWidget, QLineEdit, QPushButton, QScrollArea, QWidget

from rom_mate.ui.spinner import LoadingSpinnerWidget


class ServerViewWindowProtocol(Protocol):
    server_connected: bool
    server_games_grid: QGridLayout | None
    server_games_scroll: QScrollArea | None
    server_loading_spinner: "LoadingSpinnerWidget | None"
    server_games_by_platform: dict[str, list[dict[str, str]]]
    server_platform_ids: dict[str, int]
    server_rom_payloads: dict[str, dict[str, Any]]
    server_search_input: QLineEdit | None
    server_search_clear_button: QPushButton | None
    server_platforms_list: QListWidget | None
    _server_platforms_loading: set[str]
    _server_render_generation: int
    _server_render_platform: str
    _server_scroll_handler: "Callable | None"
    _server_pending_rows: "dict[int, list[int]]"

    def _load_server_games(self, platform_label: str) -> None:
        ...

    def _render_server_games(self, platform: str) -> None:
        ...

    def _clear_layout(self, layout: QGridLayout) -> None:
        ...

    def _grid_columns_for_width(self, scroll: QScrollArea, grid: QGridLayout) -> int:
        ...

    def _make_game_card(self, game: dict[str, str], source: str):
        ...

    def _refresh_emulator_views(self) -> None:
        ...

    def _resize_server_platform_list(self) -> None:
        ...


class _ServerGamePlaceholder(QWidget):
    """Lightweight placeholder occupying a grid cell until it scrolls into view."""
    def __init__(self, game_data: dict, parent=None) -> None:
        super().__init__(parent)
        self.game_data = game_data
        self.setFixedSize(180, 250)
        self.setStyleSheet("background-color: #3a3a4a; border-radius: 4px;")


def clear_server_connection_data(window: ServerViewWindowProtocol) -> None:
    window.server_connected = False
    window.server_platform_ids = {}
    window.server_games_by_platform = {}
    window.server_rom_payloads = {}
    window._server_platforms_loading.clear()
    if window.server_platforms_list is not None:
        window.server_platforms_list.clear()
        window._resize_server_platform_list()
    window._refresh_emulator_views()
    if window.server_games_grid is not None:
        window._clear_layout(window.server_games_grid)


def populate_server_platforms(
    window: ServerViewWindowProtocol,
    payload: Any,
    resolve_platform_ids: Callable[[Any], dict[str, int]],
) -> None:
    if window.server_platforms_list is None:
        return

    platform_ids = resolve_platform_ids(payload)
    if not platform_ids:
        window.server_platforms_list.clear()
        window.server_platform_ids = {}
        window._refresh_emulator_views()
        return

    window.server_platforms_list.clear()
    window.server_platform_ids = platform_ids
    window.server_games_by_platform = {}
    window.server_rom_payloads = {}

    for display_label in window.server_platform_ids:
        window.server_platforms_list.addItem(display_label)

    if window.server_platforms_list.count() > 0:
        window.server_platforms_list.setCurrentRow(0)
    window._resize_server_platform_list()
    window._refresh_emulator_views()


def filter_server_games(games: list[dict[str, str]], query: str) -> list[dict[str, str]]:
    normalized_query = query.strip().casefold()
    if not normalized_query:
        return games

    filtered_games: list[dict[str, str]] = []
    for game in games:
        title = game.get("title", "").casefold()
        game_platform = game.get("platform", "").casefold()
        if normalized_query in title or normalized_query in game_platform:
            filtered_games.append(game)
    return filtered_games


def on_server_platform_selected(window: ServerViewWindowProtocol, platform_label: str) -> None:
    if not platform_label:
        return
    if platform_label not in window.server_games_by_platform:
        window._load_server_games(platform_label)
    window._render_server_games(platform_label)


def on_server_search_changed(window: ServerViewWindowProtocol, search_text: str) -> None:
    if window.server_search_clear_button is not None:
        window.server_search_clear_button.setVisible(bool(search_text.strip()))
    if window.server_platforms_list is None:
        return
    selected_item = window.server_platforms_list.currentItem()
    if selected_item is None:
        return
    window._render_server_games(selected_item.text())


def clear_server_search(window: ServerViewWindowProtocol) -> None:
    if window.server_search_input is None:
        return
    window.server_search_input.clear()


def render_server_games(window: ServerViewWindowProtocol, platform: str) -> None:
    if window.server_games_grid is None or window.server_games_scroll is None:
        return

    # Disconnect previous scroll handler
    if window._server_scroll_handler is not None:
        try:
            window.server_games_scroll.verticalScrollBar().valueChanged.disconnect(
                window._server_scroll_handler
            )
        except RuntimeError:
            pass
        window._server_scroll_handler = None

    for i in range(window.server_games_grid.rowCount()):
        window.server_games_grid.setRowStretch(i, 0)
    window._clear_layout(window.server_games_grid)

    if platform in window._server_platforms_loading:
        if window.server_loading_spinner is not None:
            window.server_loading_spinner.show()
            window.server_loading_spinner.raise_()
        return

    if window.server_loading_spinner is not None:
        window.server_loading_spinner.hide()

    games = window.server_games_by_platform.get(platform, [])
    query = ""
    if window.server_search_input is not None:
        query = window.server_search_input.text().strip().casefold()
    games = filter_server_games(games, query)

    columns = window._grid_columns_for_width(window.server_games_scroll, window.server_games_grid)
    window._server_render_generation += 1
    window._server_render_platform = platform
    generation = window._server_render_generation

    grid = window.server_games_grid
    pending: dict[int, list[int]] = {}
    for i, game in enumerate(games):
        row, col = divmod(i, columns)
        ph = _ServerGamePlaceholder(game_data=game)
        grid.addWidget(ph, row, col)
        pending.setdefault(row, []).append(col)
    window._server_pending_rows = pending

    if games:
        total_rows = math.ceil(len(games) / columns)
        grid.setRowStretch(total_rows, 1)

    def _handler(_value: int = 0) -> None:
        vb = window.server_games_scroll.verticalScrollBar()
        vh = window.server_games_scroll.viewport().height()
        _upgrade_visible_server_cards(
            window, games, columns, generation,
            visible_top=vb.value(),
            visible_bottom=vb.value() + vh,
        )

    window._server_scroll_handler = _handler
    window.server_games_scroll.verticalScrollBar().valueChanged.connect(_handler)
    _handler()


def _upgrade_visible_server_cards(
    window: ServerViewWindowProtocol,
    games: list[dict[str, str]],
    columns: int,
    generation: int,
    *,
    visible_top: int,
    visible_bottom: int,
) -> None:
    if window._server_render_generation != generation:
        return

    pending = window._server_pending_rows
    if not pending:
        return

    grid = window.server_games_grid
    top_margin = 8
    card_h = 250
    v_spacing = 12
    row_h = card_h + v_spacing

    min_row = max(0, (visible_top - top_margin) // row_h)
    max_row = max(0, (visible_bottom - top_margin) // row_h)

    done_rows: list[int] = []
    for row in range(min_row, max_row + 1):
        cols = pending.get(row)
        if cols is None:
            continue
        remaining: list[int] = []
        for col in cols:
            item = grid.itemAtPosition(row, col)
            if item is None:
                continue
            widget = item.widget()
            if not isinstance(widget, _ServerGamePlaceholder):
                continue
            game = widget.game_data
            grid.removeWidget(widget)
            widget.hide()
            widget.deleteLater()
            card = window._make_game_card(game, "server")
            grid.addWidget(card, row, col)
            # remaining is empty for this col — don't append
        if not remaining:
            done_rows.append(row)
        else:
            pending[row] = remaining

    for row in done_rows:
        pending.pop(row, None)

    if not pending and window._server_scroll_handler is not None:
        try:
            window.server_games_scroll.verticalScrollBar().valueChanged.disconnect(
                window._server_scroll_handler
            )
        except RuntimeError:
            pass
        window._server_scroll_handler = None
