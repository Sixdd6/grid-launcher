from __future__ import annotations

import hashlib
import threading
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlparse
from urllib.request import Request, urlopen

from PySide6.QtCore import QSize
from PySide6.QtGui import QImage
from PySide6.QtQuick import QQuickImageProvider


def _ext_from_bytes(data: bytes) -> str:
    if data.startswith(b"\x89PNG\r\n\x1a\n"):
        return ".png"
    if data.startswith(b"\xff\xd8\xff"):
        return ".jpg"
    if data.startswith((b"GIF87a", b"GIF89a")):
        return ".gif"
    if data.startswith(b"BM"):
        return ".bmp"
    if len(data) >= 12 and data.startswith(b"RIFF") and data[8:12] == b"WEBP":
        return ".webp"
    return ""


_VALID_IMAGE_EXTS = {
    ".jpg",
    ".jpeg",
    ".png",
    ".webp",
    ".gif",
    ".bmp",
    ".tiff",
    ".tif",
    ".ico",
    ".avif",
}


class CoverImageProvider(QQuickImageProvider):

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
        self._lock = threading.Lock()
        # Values: non-empty str = disk path (success); "" = fetch failed (skip)
        self._cache: dict[str, str] = {
            k: v for k, v in cover_url_to_cached_path.items() if v
        }

    def requestImage(self, image_id: str, size: QSize, requested_size: QSize) -> QImage:
        url = image_id.strip()

        if not url:
            return QImage()

        with self._lock:
            cached = self._cache.get(url)

        if cached is not None:
            if cached:
                p = Path(cached)
                if p.is_file():
                    img = QImage(str(p))
                    if not img.isNull():
                        return img
                # Stale entry — cached file is gone. Remove and retry via HTTP.
                with self._lock:
                    self._cache.pop(url, None)
                return self._load(url)
            return QImage()

        return self._load(url)

    def _load(self, cover_url: str) -> QImage:
        """Fetch image from disk cache or HTTP. Never blocks Qt threads."""
        url_hash = hashlib.sha1(cover_url.encode()).hexdigest()

        # 1. Disk cache scan by URL hash (covers images from previous sessions)
        for ext in (".jpg", ".jpeg", ".png", ".webp", ".gif"):
            candidate = self._image_cache_dir / f"{url_hash}{ext}"
            if candidate.is_file():
                img = QImage(str(candidate))
                if not img.isNull():
                    with self._lock:
                        self._cache[cover_url] = str(candidate)
                    return img

        # 2. HTTP fetch
        try:
            headers: dict[str, str] = {"Accept": "image/*", "User-Agent": "rom-mate/1.0"}
            if self._api_token and self._server_url:
                parsed_server = urlparse(self._server_url)
                parsed_cover = urlparse(cover_url)
                if parsed_cover.netloc == parsed_server.netloc:
                    headers["Authorization"] = f"Bearer {self._api_token}"
            elif self._api_token and not self._server_url:
                headers["Authorization"] = f"Bearer {self._api_token}"
            request = Request(cover_url, headers=headers, method="GET")
            with urlopen(request, timeout=15) as resp:
                payload = resp.read()
        except (HTTPError, URLError, OSError, ValueError):
            with self._lock:
                self._cache[cover_url] = ""   # record failure, never retry
            return QImage()

        img = QImage()
        if not img.loadFromData(payload):
            with self._lock:
                self._cache[cover_url] = ""
            return QImage()

        # 3. Persist to disk
        try:
            self._image_cache_dir.mkdir(parents=True, exist_ok=True)
            ext = _ext_from_bytes(payload)
            if not ext:
                url_ext = Path(urlparse(cover_url).path).suffix.lower()
                ext = url_ext if url_ext in _VALID_IMAGE_EXTS else ".png"
            cache_file = self._image_cache_dir / f"{url_hash}{ext}"
            if not cache_file.exists():
                cache_file.write_bytes(payload)
            with self._lock:
                self._cache[cover_url] = str(cache_file)
        except OSError:
            pass

        return img