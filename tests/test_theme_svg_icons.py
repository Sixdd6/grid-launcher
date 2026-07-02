from __future__ import annotations

import os
import unittest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtGui import QColor
from PySide6.QtWidgets import QApplication

from grid_launcher.ui.theme import themed_svg_icon, themed_svg_pixmap


class ThemedSvgIconTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def test_themed_svg_pixmap_tints_hardcoded_fill_asset(self) -> None:
        pixmap = themed_svg_pixmap("svg/plus", "#00cc88", size=(32, 32))

        self.assertFalse(pixmap.isNull())
        image = pixmap.toImage()
        expected = QColor("#00cc88")

        tinted_pixel_found = False
        for y in range(image.height()):
            for x in range(image.width()):
                pixel = image.pixelColor(x, y)
                if pixel.alpha() == 0:
                    continue
                if (
                    abs(pixel.red() - expected.red()) <= 20
                    and abs(pixel.green() - expected.green()) <= 20
                    and abs(pixel.blue() - expected.blue()) <= 20
                ):
                    tinted_pixel_found = True
                    break
            if tinted_pixel_found:
                break

        self.assertTrue(tinted_pixel_found)

    def test_themed_svg_icon_and_missing_asset_behavior(self) -> None:
        icon = themed_svg_icon("svg/controller", "#3498db", size=(20, 20))
        missing_pixmap = themed_svg_pixmap("does-not-exist.svg", "#ffffff")

        self.assertFalse(icon.isNull())
        self.assertTrue(missing_pixmap.isNull())


if __name__ == "__main__":
    unittest.main()
