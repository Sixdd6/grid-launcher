from __future__ import annotations

import hashlib
import threading
from collections.abc import Callable
from pathlib import Path
from urllib.parse import urlparse

import urllib.error
import urllib.request
from PySide6.QtCore import QTimer
from PySide6.QtGui import QImage, QPixmap
from PySide6.QtWidgets import QApplication


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


class CoverLoader:
    def __init__(
        self,
        image_cache_dir: str,
        api_token: str,
        server_url: str,
        cover_url_to_cached_path: dict,
    ):
        self._image_cache_dir = Path(image_cache_dir)
        self._api_token = api_token
        self._server_url = server_url.rstrip("/")
        self._lock = threading.Lock()
        self._cache: dict[str, str] = {
            str(k): str(v)
            for k, v in cover_url_to_cached_path.items()
            if isinstance(k, str) and isinstance(v, str) and v
        }

    def load_pixmap(self, url: str) -> QPixmap | None:
        raw = self._load_bytes((url or "").strip())
        if not raw:
            return None
        pixmap = QPixmap()
        if not pixmap.loadFromData(raw):
            return None
        return pixmap

    def load_async(self, url: str, callback: Callable[[QPixmap | None], None]) -> None:
        target_url = (url or "").strip()

        def _worker() -> None:
            raw = self._load_bytes(target_url)

            def _deliver() -> None:
                if not raw:
                    callback(None)
                    return
                pixmap = QPixmap()
                if not pixmap.loadFromData(raw):
                    callback(None)
                    return
                callback(pixmap)

            _app = QApplication.instance()
            if _app is not None:
                QTimer.singleShot(0, _app, _deliver)
            else:
                _deliver()

        thread = threading.Thread(target=_worker, daemon=True)
        thread.start()

    def _load_bytes(self, cover_url: str) -> bytes | None:
        if not cover_url:
            return None

        with self._lock:
            cached = self._cache.get(cover_url)

        if cached is not None:
            if not cached:
                return None
            cached_bytes = self._read_image_bytes(Path(cached))
            if cached_bytes is not None:
                return cached_bytes
            with self._lock:
                self._cache.pop(cover_url, None)

        # Handle local file:// URIs (used for retroarch platform assets)
        if cover_url.startswith("file://"):
            try:
                from urllib.request import url2pathname
                from urllib.parse import urlparse as _urlparse

                parsed = _urlparse(cover_url)
                file_path = Path(url2pathname(parsed.path))
                data = self._read_image_bytes(file_path)
                return data
            except Exception:
                return None

        url_hash = hashlib.sha1(cover_url.encode()).hexdigest()
        for ext in (".jpg", ".jpeg", ".png", ".webp", ".gif"):
            candidate = self._image_cache_dir / f"{url_hash}{ext}"
            candidate_bytes = self._read_image_bytes(candidate)
            if candidate_bytes is None:
                continue
            with self._lock:
                self._cache[cover_url] = str(candidate)
            return candidate_bytes

        headers: dict[str, str] = {"Accept": "image/*", "User-Agent": "rom-mate/1.0"}
        if self._api_token and self._server_url:
            parsed_server = urlparse(self._server_url)
            parsed_cover = urlparse(cover_url)
            if parsed_cover.netloc == parsed_server.netloc:
                headers["Authorization"] = f"Bearer {self._api_token}"
        elif self._api_token and not self._server_url:
            headers["Authorization"] = f"Bearer {self._api_token}"

        try:
            req = urllib.request.Request(cover_url, headers=headers)
            with urllib.request.urlopen(req, timeout=15) as response:
                payload = response.read()
        except (urllib.error.URLError, OSError, ValueError):
            with self._lock:
                self._cache[cover_url] = ""
            return None

        if not self._is_image_payload(payload):
            with self._lock:
                self._cache[cover_url] = ""
            return None

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

        return payload

    def _read_image_bytes(self, path: Path) -> bytes | None:
        if not path.is_file():
            return None
        try:
            data = path.read_bytes()
        except OSError:
            return None
        if not self._is_image_payload(data):
            return None
        return data

    @staticmethod
    def _is_image_payload(data: bytes) -> bool:
        if not data:
            return False
        image = QImage()
        return image.loadFromData(data)
