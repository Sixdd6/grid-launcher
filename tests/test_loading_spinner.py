import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import unittest

from PySide6.QtWidgets import QApplication, QWidget

from rom_mate.ui.spinner import LoadingSpinnerWidget


class LoadingSpinnerTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def _make_spinner(self, diameter: int = 48) -> tuple[QWidget, LoadingSpinnerWidget]:
        parent = QWidget()
        parent.resize(200, 150)
        spinner = LoadingSpinnerWidget(parent, diameter=diameter)
        return parent, spinner

    def test_spinner_hidden_on_init(self) -> None:
        _parent, spinner = self._make_spinner()
        self.assertFalse(spinner.isVisible())
        self.assertFalse(spinner._timer.isActive())

    def test_timer_starts_on_show(self) -> None:
        parent, spinner = self._make_spinner()
        parent.show()
        spinner.show()
        self.assertTrue(spinner._timer.isActive())
        spinner.hide()

    def test_timer_stops_on_hide(self) -> None:
        parent, spinner = self._make_spinner()
        parent.show()
        spinner.show()
        spinner.hide()
        self.assertFalse(spinner._timer.isActive())

    def test_tick_advances_angle(self) -> None:
        _parent, spinner = self._make_spinner()
        spinner._angle = 0
        for _ in range(60):
            spinner._tick()
        self.assertEqual(spinner._angle, (6 * 60) % 360)

    def test_reposition_centers_within_parent(self) -> None:
        parent = QWidget()
        parent.resize(200, 150)
        spinner = LoadingSpinnerWidget(parent, diameter=48)
        spinner._reposition()
        self.assertEqual(spinner.x(), (200 - 48) // 2)
        self.assertEqual(spinner.y(), (150 - 48) // 2)

    def test_paint_does_not_raise(self) -> None:
        parent = QWidget()
        parent.resize(200, 150)
        spinner = LoadingSpinnerWidget(parent, diameter=48)
        parent.show()
        spinner.show()
        # grab() triggers paintEvent
        spinner.grab()
        spinner.hide()


if __name__ == "__main__":
    unittest.main()
