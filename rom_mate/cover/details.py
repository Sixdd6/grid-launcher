from __future__ import annotations

from typing import Any, Callable, Protocol

from PySide6.QtGui import QPixmap
from PySide6.QtWidgets import QLabel


def resolved_cover_url_for_game(
    game: dict[str, str],
    server_rom_payloads: dict[str, Any],
    *,
    resolve_cover_url: Callable[[Any], str],
    cover_url_from_rom_payload: Callable[[dict[str, Any]], str],
) -> str:
    cover_url_value = game.get("cover_url", "")
    if isinstance(cover_url_value, str):
        resolved = resolve_cover_url(cover_url_value)
        if resolved:
            return resolved

    rom_id_value = game.get("rom_id", "")
    rom_id = rom_id_value.strip() if isinstance(rom_id_value, str) else ""
    if not rom_id:
        return ""

    payload = server_rom_payloads.get(rom_id)
    if isinstance(payload, dict):
        return cover_url_from_rom_payload(payload)
    return ""


class CoverDetailsWindowProtocol(Protocol):
    current_details_game: dict[str, str] | None
    details_content_frame: Any | None
    details_cover_label: QLabel | None
    details_description_label: QLabel | None
    details_screenshot_labels: list[QLabel]
    details_screenshots_scroll: Any | None
    cover_cache: dict[str, QPixmap | None]

    def width(self) -> int:
        ...

    def _screenshot_urls_from_game(self, game: dict[str, str]) -> list[str]:
        ...

    def _queue_cover_load(self, cover_url: str, label: QLabel) -> None:
        ...

    def _queue_game_cover_load(self, game: dict[str, str], label: QLabel) -> None:
        ...

    def _apply_cover_to_label(self, label: QLabel, pixmap: QPixmap | None) -> None:
        ...

    def _rescale_details_media_for_current_sizes(self) -> None:
        ...


def update_details_screenshots(window: CoverDetailsWindowProtocol, game: dict[str, str]) -> None:
    if not window.details_screenshot_labels:
        return

    screenshot_urls = window._screenshot_urls_from_game(game)
    for index, label in enumerate(window.details_screenshot_labels):
        label.clear()
        if index < len(screenshot_urls):
            label.setVisible(True)
            window._queue_cover_load(screenshot_urls[index], label)
        else:
            label.setVisible(False)


def update_details_layout_metrics(window: CoverDetailsWindowProtocol) -> None:
    if window.details_content_frame is None:
        return

    content_width = window.details_content_frame.width()
    if content_width <= 0:
        content_width = window.width() - 64
    content_width = max(content_width, 900)

    cover_width = max(300, min(780, int(content_width * 0.36)))
    cover_height = max(400, min(1040, int(cover_width * 1.35)))
    if window.details_cover_label is not None:
        window.details_cover_label.setFixedSize(cover_width, cover_height)

    screenshot_width = max(210, min(520, int(content_width * 0.25)))
    screenshot_height = max(118, min(320, int(screenshot_width * 0.62)))
    if window.details_screenshots_scroll is not None:
        window.details_screenshots_scroll.setFixedWidth(screenshot_width + 28)
    for label in window.details_screenshot_labels:
        label.setFixedSize(screenshot_width, screenshot_height)

    if window.details_description_label is not None:
        description_width = max(420, min(1600, int(content_width * 0.66)))
        window.details_description_label.setMaximumWidth(description_width)

    window._rescale_details_media_for_current_sizes()


def rescale_details_media_for_current_sizes(window: CoverDetailsWindowProtocol) -> None:
    game = window.current_details_game
    if game is None:
        return

    if window.details_cover_label is not None:
        window._queue_game_cover_load(game, window.details_cover_label)

    screenshot_urls = window._screenshot_urls_from_game(game)
    for index, label in enumerate(window.details_screenshot_labels):
        if index >= len(screenshot_urls):
            continue
        screenshot_url = screenshot_urls[index].strip()
        if not screenshot_url:
            continue
        if screenshot_url in window.cover_cache:
            window._apply_cover_to_label(label, window.cover_cache[screenshot_url])
        else:
            window._queue_cover_load(screenshot_url, label)
