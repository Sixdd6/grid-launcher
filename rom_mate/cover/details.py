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
    current_details_cloud_mode: str
    details_content_frame: Any | None
    details_cover_label: QLabel | None
    details_description_label: QLabel | None
    details_screenshot_labels: list[QLabel]
    details_screenshots_panel: Any | None
    details_screenshots_scroll: Any | None
    cover_cache: dict[str, QPixmap | None]

    def width(self) -> int:
        ...

    def height(self) -> int:
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
    content_width = max(content_width, 640)

    content_height = window.details_content_frame.height()
    if content_height <= 0:
        content_height = window.height() - 180
    content_height = max(content_height, 420)

    cover_aspect_ratio = 1.35
    cover_max_height = max(320, min(680, int(content_height * 0.78)))
    cover_width = max(
        220,
        min(
            720,
            int(content_width * 0.32),
            int(cover_max_height / cover_aspect_ratio),
        ),
    )
    cover_height = max(300, min(cover_max_height, int(cover_width * cover_aspect_ratio)))
    if window.details_cover_label is not None:
        window.details_cover_label.setFixedSize(cover_width, cover_height)

    screenshot_aspect_ratio = 0.62
    screenshot_max_height = max(96, min(240, int(content_height * 0.24)))
    screenshot_width = max(
        160,
        min(
            420,
            int(content_width * 0.19),
            int(screenshot_max_height / screenshot_aspect_ratio),
        ),
    )
    screenshot_height = max(90, min(screenshot_max_height, int(screenshot_width * screenshot_aspect_ratio)))

    compact_cloud_layout = window.current_details_cloud_mode != "overview" and (
        content_width < 1360
        or content_height < 640
        or window.width() <= 1280
        or window.height() <= 720
    )
    minimum_center_width = 620 if window.current_details_cloud_mode != "overview" else 420
    screenshots_reserved_width = screenshot_width + 84
    show_screenshots = (not compact_cloud_layout) and (
        content_width >= (cover_width + minimum_center_width + screenshots_reserved_width)
    )

    screenshots_panel = getattr(window, "details_screenshots_panel", None)
    if screenshots_panel is not None:
        screenshots_panel.setVisible(show_screenshots)
    elif window.details_screenshots_scroll is not None:
        window.details_screenshots_scroll.setVisible(show_screenshots)

    if window.details_screenshots_scroll is not None:
        window.details_screenshots_scroll.setFixedWidth(screenshot_width + 28)
    for label in window.details_screenshot_labels:
        label.setFixedSize(screenshot_width, screenshot_height)

    if window.details_description_label is not None:
        description_width = max(280, min(1200, int(content_width * 0.42)))
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
