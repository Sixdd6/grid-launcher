from __future__ import annotations

import os
import sys
import unittest
from unittest.mock import MagicMock, patch

os.environ["QT_QPA_PLATFORM"] = "offscreen"

from PySide6.QtWidgets import QApplication

from rom_mate.tv.widgets.views.library_view import LibraryView


def _make_view(games: list[dict] | None = None) -> LibraryView:
    app_backend = MagicMock()
    app_backend.libraryGames = games or []
    app_backend.libraryGamesChanged = MagicMock()
    cover_loader = MagicMock()
    return LibraryView(
        app_backend,
        MagicMock(),
        MagicMock(),
        MagicMock(),
        MagicMock(),
        cover_loader,
        lambda w: None,
        lambda: None,
    )


def _fake_games(n: int) -> list[dict]:
    return [{"name": f"Game {i}", "cover_url": ""} for i in range(n)]


class LibraryViewCarouselTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication(sys.argv)

    def _resize_view(self, view: LibraryView, width: int, height: int) -> None:
        view.resize(width, height)
        view.show()
        self.app.processEvents()

    def test_initial_idx_is_zero(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        self.assertEqual(view._current_idx, 0)

    def test_empty_games_shows_empty_label(self):
        view = _make_view([])
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        self.assertTrue(view._empty_label.isVisible())

    def test_non_empty_games_hides_empty_label(self):
        view = _make_view(_fake_games(3))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        self.assertFalse(view._empty_label.isVisible())

    def test_handle_nav_right_increments_idx(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        with patch("rom_mate.tv.widgets.views.library_view.QParallelAnimationGroup.start"):
            view.handle_nav("right")

        self.assertEqual(view._current_idx, 1)

    def test_handle_nav_left_at_boundary_does_nothing(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._current_idx = 0

        with patch("rom_mate.tv.widgets.views.library_view.QParallelAnimationGroup.start"):
            view.handle_nav("left")

        self.assertEqual(view._current_idx, 0)
        self.assertFalse(view._anim_blocked)

    def test_handle_nav_right_at_boundary_does_nothing(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._current_idx = 4

        with patch("rom_mate.tv.widgets.views.library_view.QParallelAnimationGroup.start"):
            view.handle_nav("right")

        self.assertEqual(view._current_idx, 4)
        self.assertFalse(view._anim_blocked)

    def test_handle_nav_blocked_during_anim(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._anim_blocked = True

        view.handle_nav("right")

        self.assertEqual(view._current_idx, 0)

    def test_handle_nav_up_is_noop(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        view.handle_nav("up")

        self.assertEqual(view._current_idx, 0)

    def test_handle_nav_down_is_noop(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        view.handle_nav("down")

        self.assertEqual(view._current_idx, 0)

    def test_handle_nav_confirm_emits_game_selected(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        emitted: list[dict] = []
        view.game_selected.connect(lambda g: emitted.append(g))

        view.handle_nav("confirm")

        self.assertEqual(len(emitted), 1)
        self.assertEqual(emitted[0], view._games[0])

    def test_on_slide_finished_clears_blocked(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._anim_blocked = True

        view._on_nav_finished("right")

        self.assertFalse(view._anim_blocked)
        self.assertIsNone(view._nav_anim)

    def test_pool_center_card_not_dimmed(self):
        view = _make_view(_fake_games(9))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._bind_pool()

        self.assertFalse(view._pool[5]._dimmed)

    def test_pool_side_cards_are_dimmed(self):
        view = _make_view(_fake_games(9))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._bind_pool()

        self.assertTrue(view._pool[0]._dimmed)
        self.assertTrue(view._pool[10]._dimmed)

    def test_place_strip_y_at_anchor(self):
        view = _make_view(_fake_games(3))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        self.assertEqual(view._strip_container.y(), int(1080 * 0.67))

    def test_grow_center_card_starts_anim(self):
        view = _make_view(_fake_games(5))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._card_scale_anim = None

        view._grow_center_card()

        self.assertIsNotNone(view._card_scale_anim)

    def test_fanart_fills_view(self):
        view = _make_view(_fake_games(3))
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        self.assertEqual(view._fanart.geometry(), view.rect())

    def test_refresh_loads_games_from_backend(self):
        view = _make_view([])
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._app_backend.libraryGames = _fake_games(3)

        view._refresh()

        self.assertEqual(len(view._games), 3)


if __name__ == "__main__":
    unittest.main()
