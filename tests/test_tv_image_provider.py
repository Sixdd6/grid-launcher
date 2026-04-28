from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

from PySide6.QtCore import QCoreApplication, QSize
from PySide6.QtGui import QImage

_app = QCoreApplication.instance() or QCoreApplication(sys.argv)


class TestCoverImageProviderCacheHit(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.cache_dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def _make_provider(self, cover_url_map=None):
        from rom_mate.tv.bridge.image_provider import CoverImageProvider
        return CoverImageProvider(self.cache_dir, "test-token", "", cover_url_map or {})

    def test_requestImage_returns_image_from_cached_path(self):
        # Generate a valid 1x1 red PNG using QImage.
        from PySide6.QtCore import QBuffer, QIODevice
        from PySide6.QtGui import QColor

        src = QImage(1, 1, QImage.Format.Format_RGB32)
        src.fill(QColor("red"))
        buf = QBuffer()
        buf.open(QIODevice.OpenModeFlag.WriteOnly)
        src.save(buf, "PNG")
        png_bytes = bytes(buf.data())
        buf.close()

        cache_file = self.cache_dir / "cover.png"
        cache_file.write_bytes(png_bytes)

        provider = self._make_provider({"http://example.com/cover.jpg": str(cache_file)})
        null_size = QSize()
        img = provider.requestImage("http://example.com/cover.jpg", null_size, null_size)
        self.assertFalse(img.isNull())

    def test_requestImage_returns_null_for_empty_id(self):
        provider = self._make_provider()
        null_size = QSize()
        img = provider.requestImage("", null_size, null_size)
        self.assertTrue(img.isNull())

    def test_requestImage_returns_null_for_unknown_key_without_network(self):
        # No cached path, no network (unreachable URL)
        provider = self._make_provider()
        null_size = QSize()
        img = provider.requestImage("http://localhost:19999/nonexistent.jpg", null_size, null_size)
        self.assertTrue(img.isNull())

    def test_does_not_import_loader(self):
        import rom_mate.tv.bridge.image_provider  # noqa: F401
        import sys
        self.assertNotIn("rom_mate.cover.loader", sys.modules)


if __name__ == "__main__":
    unittest.main()
