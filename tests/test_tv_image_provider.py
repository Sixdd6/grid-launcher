from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtCore import QBuffer, QIODevice
from PySide6.QtGui import QColor, QImage
from PySide6.QtWidgets import QApplication

_app = QApplication.instance() or QApplication(sys.argv)


class TestCoverLoader(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.cache_dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def _make_loader(self, cover_url_map=None):
        from grid_launcher.tv.widgets.cover_loader import CoverLoader

        return CoverLoader(str(self.cache_dir), "test-token", "", cover_url_map or {})

    def _make_png_bytes(self) -> bytes:
        src = QImage(1, 1, QImage.Format.Format_RGB32)
        src.fill(QColor("red"))
        buf = QBuffer()
        buf.open(QIODevice.OpenModeFlag.WriteOnly)
        src.save(buf, "PNG")
        png_bytes = bytes(buf.data())
        buf.close()
        return png_bytes

    def test_load_pixmap_returns_image_from_cached_path(self):
        png_bytes = self._make_png_bytes()

        cache_file = self.cache_dir / "cover.png"
        cache_file.write_bytes(png_bytes)

        loader = self._make_loader({"http://example.com/cover.jpg": str(cache_file)})
        pixmap = loader.load_pixmap("http://example.com/cover.jpg")
        self.assertIsNotNone(pixmap)
        self.assertFalse(pixmap.isNull())

    def test_load_pixmap_returns_none_for_empty_url(self):
        loader = self._make_loader()
        self.assertIsNone(loader.load_pixmap(""))

    def test_load_pixmap_returns_none_for_none_url(self):
        loader = self._make_loader()
        self.assertIsNone(loader.load_pixmap(None))

    @patch("grid_launcher.tv.widgets.cover_loader.urllib.request.urlopen")
    def test_load_pixmap_fetches_over_http_when_not_cached(self, mock_urlopen):
        png_bytes = self._make_png_bytes()
        mock_response = Mock()
        mock_response.__enter__ = lambda s: s
        mock_response.__exit__ = Mock(return_value=False)
        mock_response.read.return_value = png_bytes
        mock_urlopen.return_value = mock_response

        loader = self._make_loader()
        pixmap = loader.load_pixmap("http://example.com/cover.jpg")

        self.assertIsNotNone(pixmap)
        self.assertFalse(pixmap.isNull())
        mock_urlopen.assert_called_once()

    @patch("grid_launcher.tv.widgets.cover_loader.urllib.request.urlopen")
    def test_load_pixmap_returns_none_when_http_fetch_fails(self, mock_urlopen):
        import urllib.error

        mock_urlopen.side_effect = urllib.error.URLError("network error")
        loader = self._make_loader()

        self.assertIsNone(loader.load_pixmap("http://example.com/cover.jpg"))
        mock_urlopen.assert_called_once()

    @patch("grid_launcher.tv.widgets.cover_loader.urllib.request.urlopen")
    def test_load_pixmap_stale_cached_entry_falls_back_to_http(self, mock_urlopen):
        png_bytes = self._make_png_bytes()
        mock_response = Mock()
        mock_response.__enter__ = lambda s: s
        mock_response.__exit__ = Mock(return_value=False)
        mock_response.read.return_value = png_bytes
        mock_urlopen.return_value = mock_response

        missing_file = self.cache_dir / "missing.png"
        loader = self._make_loader({"http://example.com/cover.jpg": str(missing_file)})

        pixmap = loader.load_pixmap("http://example.com/cover.jpg")
        self.assertIsNotNone(pixmap)
        self.assertFalse(pixmap.isNull())
        mock_urlopen.assert_called_once()

    @patch("grid_launcher.tv.widgets.cover_loader.urllib.request.urlopen")
    def test_load_pixmap_returns_none_for_unknown_key_without_network(self, mock_urlopen):
        import urllib.error

        mock_urlopen.side_effect = urllib.error.URLError("unreachable")
        loader = self._make_loader()
        self.assertIsNone(loader.load_pixmap("http://localhost:19999/nonexistent.jpg"))
        mock_urlopen.assert_called_once()

    def test_does_not_import_loader(self):
        import sys

        sys.modules.pop("grid_launcher.cover.loader", None)
        sys.modules.pop("grid_launcher.tv.widgets.cover_loader", None)
        import grid_launcher.tv.widgets.cover_loader  # noqa: F401

        self.assertNotIn("grid_launcher.cover.loader", sys.modules)


if __name__ == "__main__":
    unittest.main()
