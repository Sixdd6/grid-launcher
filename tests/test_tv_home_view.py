from __future__ import annotations

import os
import sys
import unittest
from unittest.mock import MagicMock, patch

os.environ["QT_QPA_PLATFORM"] = "offscreen"

from PySide6.QtWidgets import QApplication

from grid_launcher.tv.widgets.views.home_view import HomeView


class HomeViewAnimationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication(sys.argv)

    def _make_view(self) -> HomeView:
        app_backend = MagicMock()
        app_backend.libraryGames = []
        app_backend.favoritesGames = []
        app_backend.newAdditionsGames = []
        app_backend.highlyRatedGames = []
        app_backend.libraryGamesChanged = MagicMock()
        app_backend.favoritesGamesChanged = MagicMock()
        app_backend.newAdditionsGamesChanged = MagicMock()
        app_backend.highlyRatedGamesChanged = MagicMock()

        view = HomeView(
            app_backend,
            MagicMock(),
            MagicMock(),
            MagicMock(),
            MagicMock(),
            MagicMock(),
            lambda widget: None,
            lambda: None,
        )
        return view

    def _resize_view(self, view: HomeView, width: int, height: int) -> None:
        view.resize(width, height)
        view.show()
        self.app.processEvents()

    def test_initial_active_row_is_zero(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        self.assertEqual(view._active_row, 0)

    def test_place_rows_active_row_in_strip(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        self.assertEqual(view._rows[0].geometry().y(), 756)

    def test_place_rows_later_rows_below(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)

        self.assertEqual(view._rows[1].geometry().y(), 1080)
        self.assertEqual(view._rows[2].geometry().y(), 1080)
        self.assertEqual(view._rows[3].geometry().y(), 1080)

    def test_place_rows_on_resize(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        self._resize_view(view, 1280, 720)

        self.assertEqual(view._rows[0].geometry().y(), 504)

    def test_handle_nav_blocked_when_anim_in_progress(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._anim_blocked = True

        view.handle_nav("down")

        self.assertEqual(view._active_row, 0)
        self.assertEqual(view._pending_nav, "down")

    def test_pending_nav_queues_latest_direction(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._anim_blocked = True

        view.handle_nav("down")
        view.handle_nav("up")  # overwrites previous pending

        self.assertEqual(view._pending_nav, "up")

    def test_pending_nav_fires_on_anim_finished(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 0
        view._anim_blocked = True
        view._pending_nav = "down"

        with patch("grid_launcher.tv.widgets.views.home_view.QParallelAnimationGroup.start"):
            view._on_row_anim_finished(-1)

        self.assertIsNone(view._pending_nav)
        self.assertEqual(view._active_row, 1)

    def test_pending_nav_cleared_after_dispatch(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 0
        view._anim_blocked = True
        view._pending_nav = "down"

        with patch("grid_launcher.tv.widgets.views.home_view.QParallelAnimationGroup.start"):
            view._on_row_anim_finished(-1)

        # _pending_nav consumed; the new animation would have set _anim_blocked again
        self.assertIsNone(view._pending_nav)

    def test_handle_nav_up_at_boundary_does_nothing(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 0

        with patch("grid_launcher.tv.widgets.views.home_view.QParallelAnimationGroup.start"):
            view.handle_nav("up")

        self.assertEqual(view._active_row, 0)
        self.assertFalse(view._anim_blocked)

    def test_handle_nav_down_at_boundary_does_nothing(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 3

        with patch("grid_launcher.tv.widgets.views.home_view.QParallelAnimationGroup.start"):
            view.handle_nav("down")

        self.assertEqual(view._active_row, 3)
        self.assertFalse(view._anim_blocked)

    def test_on_row_anim_finished_clears_blocked(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._anim_blocked = True
        view._active_row = 1

        view._on_row_anim_finished(-1)

        self.assertFalse(view._anim_blocked)
        self.assertIsNone(view._row_anim)

    def test_row_up_arrow_hidden_at_row_zero(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 0
        view._place_rows()
        self.assertFalse(view._row_up_arrow.isVisible())

    def test_row_down_arrow_hidden_at_last_row(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 3
        view._place_rows()
        self.assertFalse(view._row_down_arrow.isVisible())

    def test_both_arrows_visible_at_middle_row(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 1
        view._place_rows()
        self.assertTrue(view._row_up_arrow.isVisible())
        self.assertTrue(view._row_down_arrow.isVisible())

    def test_row_up_arrow_x_centered(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 1
        view._place_rows()
        self.assertEqual(view._row_up_arrow.x(), 1920 // 2 - (32 * 2 + 8) // 2)

    def test_row_down_arrow_x_centered(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 1
        view._place_rows()
        self.assertEqual(view._row_down_arrow.x(), 1920 // 2 - (32 * 2 + 8) // 2 + 32 + 8)

    def test_both_arrows_at_title_y(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 1
        view._place_rows()
        expected_y = int(round(1080 * 0.70)) + 16
        self.assertEqual(view._row_up_arrow.y(), expected_y)
        self.assertEqual(view._row_down_arrow.y(), expected_y)

    def test_arrows_update_on_animate_start(self):
        view = self._make_view()
        self.addCleanup(view.deleteLater)
        self._resize_view(view, 1920, 1080)
        view._active_row = 0

        with patch("grid_launcher.tv.widgets.views.home_view.QParallelAnimationGroup.start"):
            view.handle_nav("down")

        self.assertTrue(view._row_up_arrow.isVisible())

if __name__ == "__main__":
    unittest.main()
