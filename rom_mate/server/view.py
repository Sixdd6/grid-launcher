from __future__ import annotations

from typing import Any, Callable, Protocol

from PySide6.QtWidgets import QGridLayout, QListWidget, QLineEdit, QPushButton, QScrollArea


class ServerViewWindowProtocol(Protocol):
    server_connected: bool
    server_games_grid: QGridLayout | None
    server_games_scroll: QScrollArea | None
    server_games_by_platform: dict[str, list[dict[str, str]]]
    server_platform_ids: dict[str, int]
    server_rom_payloads: dict[str, dict[str, Any]]
    server_search_input: QLineEdit | None
    server_search_clear_button: QPushButton | None
    server_platforms_list: QListWidget | None

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


def clear_server_connection_data(window: ServerViewWindowProtocol) -> None:
    window.server_connected = False
    window.server_platform_ids = {}
    window.server_games_by_platform = {}
    window.server_rom_payloads = {}
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

    games = window.server_games_by_platform.get(platform, [])
    query = ""
    if window.server_search_input is not None:
        query = window.server_search_input.text().strip().casefold()
    games = filter_server_games(games, query)

    window._clear_layout(window.server_games_grid)
    columns = window._grid_columns_for_width(window.server_games_scroll, window.server_games_grid)
    for i, game in enumerate(games):
        card = window._make_game_card(game, "server")
        row = i // columns
        col = i % columns
        window.server_games_grid.addWidget(card, row, col)
