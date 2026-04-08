from __future__ import annotations

from typing import Any, Protocol

from PySide6.QtCore import QUrl, Qt
from PySide6.QtGui import QPixmap
from PySide6.QtNetwork import QNetworkReply, QNetworkRequest
from PySide6.QtWidgets import QLabel


class CoverLoaderWindowProtocol(Protocol):
    cover_cache: dict[str, QPixmap | None]
    cover_waiters: dict[str, list[QLabel]]
    cover_loading: set[str]
    cover_network: Any

    def _apply_cover_to_label(self, label: QLabel, pixmap: QPixmap | None) -> None:
        ...

    def _on_cover_reply(self, cover_url: str, reply: QNetworkReply) -> None:
        ...


def apply_cover_to_label(label: QLabel, pixmap: QPixmap | None) -> None:
    if pixmap is None or pixmap.isNull():
        return
    try:
        label.setText("")
        label.setPixmap(
            pixmap.scaled(
                label.size(),
                Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            )
        )
    except RuntimeError:
        return


def queue_cover_load(window: CoverLoaderWindowProtocol, cover_url: str, label: QLabel) -> None:
    normalized = cover_url.strip()
    if not normalized:
        return

    if normalized in window.cover_cache:
        window._apply_cover_to_label(label, window.cover_cache[normalized])
        return

    waiters = window.cover_waiters.setdefault(normalized, [])
    waiters.append(label)
    if normalized in window.cover_loading:
        return

    window.cover_loading.add(normalized)
    request = QNetworkRequest(QUrl(normalized))
    request.setRawHeader(b"Accept", b"image/*")
    reply = window.cover_network.get(request)
    reply.finished.connect(lambda url=normalized, rep=reply: window._on_cover_reply(url, rep))


def on_cover_reply(window: CoverLoaderWindowProtocol, cover_url: str, reply: QNetworkReply) -> None:
    pixmap: QPixmap | None = None
    if reply.error() == QNetworkReply.NetworkError.NoError:
        payload = bytes(reply.readAll())
        parsed = QPixmap()
        if payload and parsed.loadFromData(payload):
            pixmap = parsed

    window.cover_cache[cover_url] = pixmap
    window.cover_loading.discard(cover_url)
    waiters = window.cover_waiters.pop(cover_url, [])
    for label in waiters:
        window._apply_cover_to_label(label, pixmap)
    reply.deleteLater()
