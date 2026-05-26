from __future__ import annotations

import os
import unittest
from unittest.mock import MagicMock

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication

from rom_mate.cover.details import update_details_layout_metrics
from rom_mate.ui.game_views import open_game_details


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
        self._stylesheet: str = ""

    def setFixedSize(self, width: int, height: int) -> None:
        self.fixed_size = (width, height)

    def setMaximumWidth(self, width: int) -> None:
        self.maximum_width = width

    def styleSheet(self) -> str:
        return self._stylesheet

    def setStyleSheet(self, s: str) -> None:
        self._stylesheet = s


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
        self.details_title_label = _StubLabel()
        self.details_title_label.setStyleSheet("font-size: 30px; font-weight: 700; color: #f8f8f2;")

        _meta_label = _StubLabel()
        _meta_label.setStyleSheet("font-size: 14px; color: #f8f8f2;")
        self.details_metadata_scalable_labels: list[tuple[_StubLabel, str]] = [
            (self.details_title_label, self.details_title_label.styleSheet()),
            (_meta_label, _meta_label.styleSheet()),
        ]
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
    def setUp(self) -> None:
        self.window = _StubWindow(content_width=1200, content_height=700, window_width=1920, window_height=1080)

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

    def test_screenshot_labels_receive_max_width_not_fixed_size(self) -> None:
        window = _StubWindow(
            content_width=1480,
            content_height=760,
            window_width=1600,
            window_height=900,
        )

        update_details_layout_metrics(window)

        for label in window.details_screenshot_labels:
            self.assertIsNone(label.fixed_size)
            self.assertIsNotNone(label.maximum_width)

    def test_font_scale_decreases_at_720p(self) -> None:
        self.window._window_height = 720
        update_details_layout_metrics(self.window)
        ss = self.window.details_title_label.styleSheet()
        import re

        m = re.search(r"font-size:\s*(\d+)px", ss)
        self.assertIsNotNone(m)
        self.assertLessEqual(int(m.group(1)), 24)

    def test_font_scale_unchanged_at_1080p(self) -> None:
        self.window._window_height = 1080
        update_details_layout_metrics(self.window)
        ss = self.window.details_title_label.styleSheet()
        import re

        m = re.search(r"font-size:\s*(\d+)px", ss)
        self.assertIsNotNone(m)
        self.assertEqual(int(m.group(1)), 30)

    def test_font_scale_increases_at_1440p(self) -> None:
        self.window._window_height = 1440
        update_details_layout_metrics(self.window)
        ss = self.window.details_title_label.styleSheet()
        import re

        m = re.search(r"font-size:\s*(\d+)px", ss)
        self.assertIsNotNone(m)
        self.assertGreater(int(m.group(1)), 30)
        self.assertLessEqual(int(m.group(1)), 75)


class _StubDetailsOpenWindow:
    def __init__(self) -> None:
        self.current_details_game = None
        self.current_details_source = ""
        self.details_title_label = None
        self.details_cover_label = None
        self.details_platform_label = None
        self.details_genres_label = None
        self.details_genres_layout = None
        self.details_genres_group = None
        self.details_regions_label = None
        self.details_filesize_label = None
        self.details_version_label = None
        self.details_rating_label = None
        self.details_description_label = None
        self.details_companies_group = MagicMock()
        self.details_companies_label = MagicMock()
        self.details_release_date_group = MagicMock()
        self.details_release_date_label = MagicMock()
        self.details_languages_group = MagicMock()
        self.details_languages_label = MagicMock()
        self.stack = MagicMock()
        self.nav_buttons: list = []
        self.install_in_progress = False
        self.install_finalize_in_progress = False
        self.install_pending_game = None
        self.install_finalize_game = None

    def _queue_game_cover_load(self, game: dict, label: object) -> None:
        pass

    def _update_details_screenshots(self, game: dict) -> None:
        pass

    def _update_details_action_buttons(self) -> None:
        pass

    def _update_details_layout_metrics(self) -> None:
        pass

    def _show_details_overview(self) -> None:
        pass

    def _is_emulators_platform(self, game: dict) -> bool:
        return False

    def _is_game_installed(self, game: dict) -> bool:
        return True

    def _install_block_reason_for_game(self, game: dict) -> str:
        return ""

    def _is_game_install_queued(self, game: dict) -> bool:
        return False

    def _game_key(self, game: dict) -> tuple:
        return (game.get("title", ""), game.get("platform", ""))

    def _is_native_executable_platform(self, game: dict) -> bool:
        return False

    def _resolved_emulator_entry_for_game(self, game: dict) -> tuple:
        return ("", None)

    def _is_rpcs3_emulator_name(self, name: str) -> bool:
        return False

    def _details_cloud_mode_supported(self, game: dict, save_type: str) -> bool:
        return False

    def _details_cloud_button_text(self, game: dict, save_type: str) -> str:
        return ""


class DetailsNewFieldsVisibilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def _make_window(self) -> _StubDetailsOpenWindow:
        return _StubDetailsOpenWindow()

    def test_companies_group_shown_when_present(self) -> None:
        window = self._make_window()
        game = {"title": "Test", "platform": "PS1", "companies": "Capcom"}

        open_game_details(window, game, "library")

        window.details_companies_group.setVisible.assert_called_with(True)

    def test_companies_group_hidden_when_absent(self) -> None:
        window = self._make_window()
        game = {"title": "Test", "platform": "PS1"}

        open_game_details(window, game, "library")

        window.details_companies_group.setVisible.assert_called_with(False)

    def test_release_date_group_shown_when_present(self) -> None:
        window = self._make_window()
        game = {"title": "Test", "platform": "PS1", "first_release_date": "1995-01-01"}

        open_game_details(window, game, "library")

        window.details_release_date_group.setVisible.assert_called_with(True)

    def test_languages_group_shown_when_present(self) -> None:
        window = self._make_window()
        game = {"title": "Test", "platform": "PS1", "languages": "English, French"}

        open_game_details(window, game, "library")

        window.details_languages_group.setVisible.assert_called_with(True)

if __name__ == "__main__":
    unittest.main()
