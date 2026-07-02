from __future__ import annotations

import os
import unittest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication, QGridLayout, QLabel, QScrollArea, QWidget

from grid_launcher.server.view import (
    _ServerGamePlaceholder,
    _upgrade_visible_server_cards,
    render_server_games,
)


def _make_games(count: int) -> list[dict[str, str]]:
    return [{"title": f"Game {i}", "platform": "TestPlatform"} for i in range(count)]


class _StubWindow:
    def __init__(self) -> None:
        self._server_render_generation: int = 0
        self._server_render_platform: str = ""
        self._server_platforms_loading: set[str] = set()
        self._server_scroll_handler = None
        self._server_pending_rows: dict = {}
        self.server_games_by_platform: dict[str, list[dict[str, str]]] = {}
        self.server_loading_spinner = None
        self.server_search_input = None
        # Grid must have a parent widget so addWidget() reparents placeholders
        # and they are not garbage-collected between creation and assertion.
        self._grid_container = QWidget()
        self.server_games_grid: QGridLayout = QGridLayout(self._grid_container)
        self.server_games_scroll: QScrollArea = QScrollArea()

    def _clear_layout(self, layout: QGridLayout) -> None:
        while layout.count():
            item = layout.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.setParent(None)
                widget.deleteLater()

    def _grid_columns_for_width(self, scroll: QScrollArea, grid: QGridLayout) -> int:
        return 4

    def _make_game_card(self, game: dict[str, str], source: str) -> QLabel:
        return QLabel(game.get("title", ""))


class ServerGamesRenderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self) -> None:
        self._window = _StubWindow()
        self._window.server_games_by_platform = {"TestPlatform": _make_games(100)}

    def test_all_placeholders_added_immediately(self) -> None:
        render_server_games(self._window, "TestPlatform")

        self.assertEqual(self._window.server_games_grid.count(), 100)
        first_widget = self._window.server_games_grid.itemAt(0).widget()
        self.assertIsInstance(first_widget, _ServerGamePlaceholder)

    def test_row_stretch_set_correctly(self) -> None:
        # 100 games at 4 columns → 25 rows → setRowStretch(25, 1)
        render_server_games(self._window, "TestPlatform")

        self.assertEqual(self._window.server_games_grid.rowStretch(25), 1)

    def test_upgrade_replaces_visible_placeholder(self) -> None:
        games = _make_games(100)
        self._window.server_games_by_platform = {"TestPlatform": games}
        render_server_games(self._window, "TestPlatform")

        _upgrade_visible_server_cards(
            self._window, games, 4, self._window._server_render_generation,
            visible_top=0, visible_bottom=600,
        )

        widget = self._window.server_games_grid.itemAtPosition(0, 0).widget()
        self.assertNotIsInstance(widget, _ServerGamePlaceholder)

    def test_stale_generation_is_no_op(self) -> None:
        games = _make_games(100)
        self._window.server_games_by_platform = {"TestPlatform": games}
        render_server_games(self._window, "TestPlatform")

        old_gen = self._window._server_render_generation
        self._window._server_render_generation += 1

        _upgrade_visible_server_cards(
            self._window, games, 4, old_gen,
            visible_top=0, visible_bottom=9999,
        )

        # Row 2 (row_top=532) is beyond the initial handler's visible range (478px),
        # so it starts as a placeholder. The stale call must leave it unchanged.
        widget = self._window.server_games_grid.itemAtPosition(2, 0).widget()
        self.assertIsInstance(widget, _ServerGamePlaceholder)

    def test_scroll_handler_replaced_on_re_render(self) -> None:
        games = _make_games(20)
        self._window.server_games_by_platform = {"TestPlatform": games}

        render_server_games(self._window, "TestPlatform")
        self.assertIsNotNone(self._window._server_scroll_handler)

        render_server_games(self._window, "TestPlatform")
        self.assertIsNotNone(self._window._server_scroll_handler)

        # Fresh placeholders — grid count equals game count
        self.assertEqual(self._window.server_games_grid.count(), len(games))


if __name__ == "__main__":
    unittest.main()
