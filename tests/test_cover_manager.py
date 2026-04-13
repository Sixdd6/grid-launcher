from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from rom_mate.cover.manager import MAX_CACHED_COVER_BYTES, queue_game_cover_load


class _StubWindow:
    def __init__(self, cached_cover_path: Path | None, cover_url: str = "https://example.com/cover.png") -> None:
        self.cover_cache: dict[str, object | None] = {}
        self._cached_cover_path = cached_cover_path
        self._cover_url = cover_url
        self.applied: list[tuple[object, object | None]] = []
        self.queued: list[tuple[str, object]] = []

    def _cached_cover_path_from_game(self, game: dict[str, str]) -> Path | None:
        return self._cached_cover_path

    def _cached_cover_cache_key(self, cached_cover_path: Path) -> str:
        return f"file:{cached_cover_path}"

    def _resolved_cover_url_for_game(self, game: dict[str, str]) -> str:
        return self._cover_url

    def _apply_cover_to_label(self, label: object, pixmap: object | None) -> None:
        self.applied.append((label, pixmap))

    def _queue_cover_load(self, cover_url: str, label: object) -> None:
        self.queued.append((cover_url, label))

    def _path_key(self, path: Path) -> str:
        return str(path)


class CoverManagerTests(unittest.TestCase):
    def test_queue_game_cover_load_does_not_decode_cached_file_synchronously(self) -> None:
        with TemporaryDirectory() as tmpdir:
            cached_cover_path = Path(tmpdir) / "cached-cover.png"
            cached_cover_path.write_bytes(b"not-an-image")
            window = _StubWindow(cached_cover_path)
            label = object()

            with patch("rom_mate.cover.manager.QPixmap", side_effect=AssertionError("unexpected sync decode")):
                queue_game_cover_load(window, {"title": "Test Game", "platform": "Test Platform"}, label)

            self.assertEqual(window.applied, [])
            self.assertEqual(len(window.queued), 2)
            self.assertTrue(window.queued[0][0].startswith("file:"))
            self.assertEqual(window.queued[1], ("https://example.com/cover.png", label))

    def test_queue_game_cover_load_queues_cached_file_url_and_remote_without_sync_disk_check(self) -> None:
        with TemporaryDirectory() as tmpdir:
            cached_cover_path = Path(tmpdir) / "cached-cover.png"
            cached_cover_path.write_bytes(b"0" * (MAX_CACHED_COVER_BYTES + 1))
            window = _StubWindow(cached_cover_path)
            label = object()

            queue_game_cover_load(window, {"title": "Test Game", "platform": "Test Platform"}, label)

            self.assertEqual(window.applied, [])
            self.assertEqual(len(window.queued), 2)
            self.assertTrue(window.queued[0][0].startswith("file:"))
            self.assertEqual(window.queued[1], ("https://example.com/cover.png", label))


if __name__ == "__main__":
    unittest.main()
