from __future__ import annotations

import os
import unittest
from unittest.mock import MagicMock

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication

from rom_mate.tv.widgets.components.game_wall import GameWall


def _make_games(count: int) -> list[dict]:
    return [{"title": f"Game {i}", "cover_url": ""} for i in range(count)]


class GameWallBatchingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self) -> None:
        self._mock_loader = MagicMock()
        self._mock_loader.create_batch.return_value = 1
        self._wall = GameWall(cover_loader=self._mock_loader, columns=4)

    def test_all_cards_in_grid_immediately(self) -> None:
        self._wall.set_games(_make_games(100))

        self.assertEqual(len(self._wall._cards), 100)
        self.assertEqual(self._wall._grid.count(), 100)

    def test_visible_row_zero_populated_immediately(self) -> None:
        # In offscreen mode, viewport().height() == 0.
        # visible_bottom = 0 + 0 = 0.
        # Row 0: row_top=0, row_bottom=480.
        # Condition row_bottom <= visible_top → 480 <= 0 → False.
        # Condition row_top > visible_bottom → 0 > 0 → False.
        # → Row 0 is NOT skipped → gets populated immediately.
        self._wall.set_games(_make_games(10))

        self.assertIn(0, self._wall._populated)

    def test_second_set_games_resets_state(self) -> None:
        self._wall.set_games(_make_games(50))
        self._wall.set_games(_make_games(10))

        self.assertEqual(len(self._wall._cards), 10)
        self.assertLessEqual(len(self._wall._populated), 10)
        self.assertEqual(self._wall._grid.count(), 10)

    def test_cover_batch_cancelled_on_second_call(self) -> None:
        self._wall.set_games(_make_games(5))
        first_batch_id = self._mock_loader.create_batch.return_value

        self._wall.set_games(_make_games(5))

        self._mock_loader.cancel_batch.assert_called()
        self._mock_loader.cancel_batch.assert_called_with(first_batch_id)

    def test_render_timer_id_removed(self) -> None:
        self.assertFalse(hasattr(self._wall, "_render_timer_id"))


if __name__ == "__main__":
    unittest.main()


if __name__ == "__main__":
    unittest.main()
