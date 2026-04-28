from __future__ import annotations

import hashlib
import threading
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urlparse
from urllib.request import Request, urlopen

from PySide6.QtCore import QSize
from PySide6.QtGui import QImage
from PySide6.QtQuick import QQuickImageProvider


class CoverImageProvider(QQuickImageProvider):
    """
    Serves cover images to QML via image://covers/<encoded_cover_url>.

    Resolution order:
    1. Check cover_url_to_cached_path map (pre-built from installed_games).
    2. Scan image_cache_dir for any file whose stem starts with a
       slug derived from the cover URL's path component.
    3. HTTP fetch from cover_url with auth token; store result in image_cache_dir.
    4. Return null QImage on failure.

    Does NOT import rom_mate.cover.cache or rom_mate.cover.loader.
    """

    def __init__(
        self,
        image_cache_dir: Path,
        api_token: str,
        server_url: str,
        cover_url_to_cached_path: dict[str, str],
    ) -> None:
        super().__init__(QQuickImageProvider.ImageType.Image)
        self._image_cache_dir = image_cache_dir
        self._api_token = api_token
        self._server_url = server_url.rstrip("/")
        self._cover_url_to_cached_path = cover_url_to_cached_path.copy()
        self._lock = threading.Lock()
        self._fetch_sem = threading.BoundedSemaphore(4)

    def requestImage(self, image_id: str, size: QSize, requested_size: QSize) -> QImage:
        cover_url = image_id.strip()
        if not cover_url:
            return QImage()

        # 1. Direct map lookup
        with self._lock:
            cached_path = self._cover_url_to_cached_path.get(cover_url, "")

        if cached_path:
            path = Path(cached_path)
            if path.is_file():
                img = QImage(str(path))
                if not img.isNull():
                    return img

        # 2. HTTP fetch with auth token
        return self._fetch_and_cache(cover_url)

    def _fetch_and_cache(self, cover_url: str) -> QImage:
        acquired = self._fetch_sem.acquire(blocking=True, timeout=30)
        if not acquired:
            return QImage()
        try:
            try:
                headers: dict[str, str] = {"Accept": "image/*", "User-Agent": "rom-mate/1.0"}
                if self._api_token and self._server_url:
                    parsed_server = urlparse(self._server_url)
                    parsed_cover = urlparse(cover_url)
                    if parsed_cover.netloc == parsed_server.netloc:
                        headers["Authorization"] = f"Bearer {self._api_token}"
                elif self._api_token and not self._server_url:
                    # No server_url configured; preserve original behavior.
                    headers["Authorization"] = f"Bearer {self._api_token}"
                request = Request(cover_url, headers=headers, method="GET")
                with urlopen(request, timeout=15) as response:
                    payload = response.read()
            except (HTTPError, URLError, OSError, ValueError):
                return QImage()

            img = QImage()
            if not img.loadFromData(payload):
                return QImage()

            # Attempt to persist to cache dir
            try:
                self._image_cache_dir.mkdir(parents=True, exist_ok=True)
                url_hash = hashlib.sha1(cover_url.encode()).hexdigest()
                ext = Path(urlparse(cover_url).path).suffix or ".png"
                cache_file = self._image_cache_dir / f"{url_hash}{ext}"
                if not cache_file.exists():
                    cache_file.write_bytes(payload)
                with self._lock:
                    self._cover_url_to_cached_path[cover_url] = str(cache_file)
            except OSError:
                pass

            return img
        finally:
            self._fetch_sem.release()
