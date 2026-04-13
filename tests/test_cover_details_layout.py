from __future__ import annotations

import unittest

from rom_mate.cover.details import update_details_layout_metrics


class _StubFrame:
    def __init__(self, width: int, height: int) -> None:
        self._width = width
        self._height = height

    def width(self) -> int:
        return self._width

    def height(self) -> int:
        return self._height


class _StubLabel:
    def __init__(self) -> None:
        self.fixed_size: tuple[int, int] | None = None
        self.maximum_width: int | None = None

    def setFixedSize(self, width: int, height: int) -> None:
        self.fixed_size = (width, height)

    def setMaximumWidth(self, width: int) -> None:
        self.maximum_width = width


class _StubScroll:
    def __init__(self) -> None:
        self.fixed_width: int | None = None

    def setFixedWidth(self, width: int) -> None:
        self.fixed_width = width


class _StubWidget:
    def __init__(self) -> None:
        self.visible = True

    def setVisible(self, value: bool) -> None:
        self.visible = value


class _StubWindow:
    def __init__(
        self,
        *,
        content_width: int,
        content_height: int,
        window_width: int,
        window_height: int,
        cloud_mode: str = "overview",
    ) -> None:
        self.details_content_frame = _StubFrame(content_width, content_height)
        self.details_cover_label = _StubLabel()
        self.details_description_label = _StubLabel()
        self.details_screenshot_labels = [_StubLabel() for _ in range(5)]
        self.details_screenshots_scroll = _StubScroll()
        self.details_screenshots_panel = _StubWidget()
        self.current_details_game = None
        self.current_details_cloud_mode = cloud_mode
        self.cover_cache: dict[str, object | None] = {}
        self._window_width = window_width
        self._window_height = window_height
        self.rescale_calls = 0

    def width(self) -> int:
        return self._window_width

    def height(self) -> int:
        return self._window_height

    def _screenshot_urls_from_game(self, game: dict[str, str]) -> list[str]:
        return []

    def _queue_cover_load(self, cover_url: str, label: _StubLabel) -> None:
        return None

    def _queue_game_cover_load(self, game: dict[str, str], label: _StubLabel) -> None:
        return None

    def _apply_cover_to_label(self, label: _StubLabel, pixmap: object | None) -> None:
        return None

    def _rescale_details_media_for_current_sizes(self) -> None:
        self.rescale_calls += 1


class CoverDetailsLayoutMetricsTests(unittest.TestCase):
    def test_update_details_layout_metrics_limits_cover_height_for_720p_windows(self) -> None:
        window = _StubWindow(content_width=1050, content_height=520, window_width=1280, window_height=720)

        update_details_layout_metrics(window)

        assert window.details_cover_label.fixed_size is not None
        self.assertLessEqual(window.details_cover_label.fixed_size[1], 440)

    def test_update_details_layout_metrics_shrinks_below_old_min_width_floor(self) -> None:
        window = _StubWindow(content_width=760, content_height=580, window_width=900, window_height=720)

        update_details_layout_metrics(window)

        assert window.details_cover_label.fixed_size is not None
        self.assertLess(window.details_cover_label.fixed_size[0], 300)
        assert window.details_description_label.maximum_width is not None
        self.assertLess(window.details_description_label.maximum_width, 420)

    def test_update_details_layout_metrics_hides_screenshots_for_tight_cloud_layouts(self) -> None:
        window = _StubWindow(
            content_width=1050,
            content_height=520,
            window_width=1280,
            window_height=720,
            cloud_mode="save",
        )

        update_details_layout_metrics(window)

        self.assertFalse(window.details_screenshots_panel.visible)

    def test_update_details_layout_metrics_hides_screenshots_for_720p_sized_cloud_layouts(self) -> None:
        window = _StubWindow(
            content_width=1230,
            content_height=600,
            window_width=1280,
            window_height=720,
            cloud_mode="save",
        )

        update_details_layout_metrics(window)

        self.assertFalse(window.details_screenshots_panel.visible)

    def test_update_details_layout_metrics_keeps_screenshots_visible_when_space_allows(self) -> None:
        window = _StubWindow(
            content_width=1480,
            content_height=760,
            window_width=1600,
            window_height=900,
            cloud_mode="overview",
        )

        update_details_layout_metrics(window)

        self.assertTrue(window.details_screenshots_panel.visible)


if __name__ == "__main__":
    unittest.main()
