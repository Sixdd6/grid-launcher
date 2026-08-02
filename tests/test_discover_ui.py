"""Tests for Discover tab UI behaviour (pills, badges, cache/offline renders, watchlist)."""

import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import importlib.util
import tempfile
import time
import unittest
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from PySide6.QtCore import QSize
from PySide6.QtGui import QColor, QIcon
from PySide6.QtWidgets import QApplication, QPushButton, QWidget

from grid_launcher.server.discover import DiscoverCache, load_watchlist
from grid_launcher.ui.theme import (
    theme_colors,
    theme_stylesheet,
    themed_svg_icon,
    themed_svg_pixmap,
)
from grid_launcher.ui.discover import (
    DiscoverCarouselSection,
    DiscoverGenreSection,
    DiscoverPageWidget,
)
from grid_launcher.ui.game_views import make_game_card


def _load_main_module() -> Any:
    module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
    spec = importlib.util.spec_from_file_location("grid_launcher_main_for_discover_ui_tests", module_path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _widget_pixels(widget: QWidget) -> list[int]:
    """Flat list of ARGB pixel values for a rendered widget (used for icon-visibility checks)."""
    image = widget.grab().toImage()
    return [image.pixel(x, y) for y in range(image.height()) for x in range(image.width())]


def _iso(seconds_ago: float) -> str:
    return datetime.fromtimestamp(time.time() - seconds_ago, tz=timezone.utc).isoformat()


def _game(
    title: str,
    rom_id: str = "1",
    genres: str = "",
    platform: str = "",
    rating: str = "",
    created_at: str = "",
) -> dict[str, str]:
    return {
        "title": title, "platform": platform, "genres": genres, "rom_id": rom_id,
        "cover_url": "", "rating": rating, "description": "", "created_at": created_at,
        "update_available": "false",
    }


class _StubWindow:
    """Lightweight stand-in for MainWindow's game-card / discover callbacks."""

    def __init__(self, library_games: list[dict] | None = None) -> None:
        self.library_games = library_games or []
        self.watchlist_games: dict[str, dict] = {}
        self.opened: list[tuple[dict, str]] = []
        self.events: list[tuple[str, str, str]] = []
        self.navigated_platforms: list[str | None] = []
        self.navigated_genres: list[str] = []

    def _theme_color(self, role: str, fallback: str) -> str:
        return fallback

    def _open_game_details(self, game: dict, source: str) -> None:
        self.opened.append((game, source))

    def _queue_game_cover_load(self, game: dict, label: Any) -> None:
        pass

    def _make_game_card(self, game: dict, source: str, show_added_date: bool = False) -> QWidget:
        return make_game_card(self, game, source, show_added_date)

    def _clear_layout(self, layout: Any) -> None:
        pass

    def record_discover_event(self, event: str, section_id: str, rom_id: str) -> None:
        self.events.append((event, section_id, rom_id))

    def toggle_watchlist(self, game: dict) -> None:
        rom_id = str(game.get("rom_id", "") or "")
        if rom_id in self.watchlist_games:
            del self.watchlist_games[rom_id]
        else:
            self.watchlist_games[rom_id] = dict(game)

    def is_watchlisted(self, rom_id: str) -> bool:
        return rom_id in self.watchlist_games

    def navigate_to_server_platform(self, platform_display_name: str | None) -> None:
        self.navigated_platforms.append(platform_display_name)

    def navigate_to_server_genre(self, genre: str) -> None:
        self.navigated_genres.append(genre)


class _FastCardWindow(_StubWindow):
    """Stub whose cards are cheap placeholders (used when badges are not under test)."""

    def _make_game_card(self, game: dict, source: str, show_added_date: bool = False) -> QWidget:
        return QWidget()


class _ThreadStub:
    def __init__(self, running: bool) -> None:
        self._running = running

    def isRunning(self) -> bool:
        return self._running


class _MainStub(_FastCardWindow):
    """Binds the unbound MainWindow discover helpers to a lightweight window."""

    def __init__(
        self,
        module: Any,
        cache: DiscoverCache,
        page: DiscoverPageWidget | None = None,
        ui_state: dict | None = None,
        watchlist_path: str | None = None,
    ) -> None:
        super().__init__()
        self.module = module
        self.discover_cache = cache
        self.discover_page = page
        self._preferred_platforms: set[str] = set()
        self._ui_state = ui_state or {}
        self._watchlist_path = watchlist_path or os.path.join(tempfile.gettempdir(), "unused_watchlist.json")
        self.saved_to_disk: list[Path] = []
        self.server_connected = True
        self._discover_load_thread: Any = None
        self.refresh_calls: list[bool] = []
        self.switched_pages: list[int] = []
        self.server_search_input: Any = None
        self.server_platforms_list: Any = None

        cache.save_to_disk = self._record_save_to_disk  # type: ignore[method-assign]

    def _record_save_to_disk(self, path: Any) -> None:
        self.saved_to_disk.append(path)

    def _load_discover_ui_state(self) -> dict:
        return dict(self._ui_state)

    def _discover_cache_file(self) -> Path:
        return Path(self._watchlist_path).with_name("discover_cache.json")

    def _watchlist_file(self) -> Path:
        return Path(self._watchlist_path)

    def _server_connected(self) -> bool:
        return self.server_connected

    def _refresh_discover_data(self, force_refresh: bool = True) -> None:
        self.refresh_calls.append(force_refresh)

    def _switch_page(self, index: int) -> None:
        self.switched_pages.append(index)

    # --- real MainWindow implementations bound to this stub ---

    def _discover_result_from_cache(self) -> dict:
        return self.module.MainWindow._discover_result_from_cache(self)

    def _discover_cached_refresh_time(self) -> float:
        return self.module.MainWindow._discover_cached_refresh_time(self)

    def _render_discover_result(self, result: dict, from_cache: bool) -> None:
        self.module.MainWindow._render_discover_result(self, result, from_cache)

    def _render_discover_from_cache(self) -> bool:
        return self.module.MainWindow._render_discover_from_cache(self)

    def _render_discover_offline(self) -> None:
        self.module.MainWindow._render_discover_offline(self)

    def _auto_refresh_stale_discover(self) -> None:
        self.module.MainWindow._auto_refresh_stale_discover(self)

    def navigate_to_server_genre(self, genre: str) -> None:
        self.module.MainWindow.navigate_to_server_genre(self, genre)

    def toggle_watchlist(self, game: dict) -> None:
        self.module.MainWindow.toggle_watchlist(self, game)

    def is_watchlisted(self, rom_id: str) -> bool:
        return self.module.MainWindow.is_watchlisted(self, rom_id)

    def _watchlist_section_games(self) -> list[dict]:
        return self.module.MainWindow._watchlist_section_games(self)

    def _hydrate_watchlist_entries(self, games: list[dict]) -> None:
        self.module.MainWindow._hydrate_watchlist_entries(self, games)


class _CardMainStub(_MainStub):
    """MainWindow-bound stub that builds real game cards (needed for star-icon checks)."""

    def _make_game_card(self, game: dict, source: str, show_added_date: bool = False) -> QWidget:
        return make_game_card(self, game, source, show_added_date)


class _QtTestCase(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])


class GenrePillTests(_QtTestCase):

    def _section(self, see_all=None) -> DiscoverGenreSection:
        games_by_genre = {
            "Action": [_game("A", "1")],
            "RPG": [_game("B", "2"), _game("C", "3")],
            "Puzzle": [],
        }
        return DiscoverGenreSection(
            "genres", ["Action", "RPG", "Puzzle"], games_by_genre, _FastCardWindow(), see_all
        )

    def test_default_first_genre_renders_carousel(self) -> None:
        section = self._section()
        self.assertEqual(section.selected_genre, "Action")
        self.assertIsNotNone(section.carousel_section)
        self.assertEqual(section.carousel_section.title, "Top Action Games")
        self.assertEqual(len(section.carousel_section.game_cards), 1)

    def test_click_rebuilds_carousel_for_clicked_genre(self) -> None:
        section = self._section()
        section.genre_buttons["RPG"].click()
        self.assertEqual(section.selected_genre, "RPG")
        self.assertEqual(section.carousel_section.title, "Top RPG Games")
        self.assertEqual(len(section.carousel_section.game_cards), 2)

    def test_click_checks_only_clicked_pill(self) -> None:
        section = self._section()
        section.genre_buttons["RPG"].click()
        self.assertTrue(section.genre_buttons["RPG"].isChecked())
        self.assertFalse(section.genre_buttons["Action"].isChecked())
        self.assertFalse(section.genre_buttons["Puzzle"].isChecked())

    def test_click_emits_selected_genre(self) -> None:
        section = self._section()
        emitted: list[str] = []
        section.games_selected.connect(emitted.append)
        section.genre_buttons["RPG"].click()
        self.assertEqual(emitted, ["RPG"])

    def test_empty_genre_click_clears_carousel(self) -> None:
        section = self._section()
        section.genre_buttons["Puzzle"].click()
        self.assertIsNone(section.carousel_section)
        self.assertEqual(section.selected_genre, "Puzzle")

    def test_returning_to_populated_genre_rebuilds(self) -> None:
        section = self._section()
        section.genre_buttons["Puzzle"].click()
        section.genre_buttons["Action"].click()
        self.assertIsNotNone(section.carousel_section)
        self.assertEqual(section.carousel_section.title, "Top Action Games")

    def test_set_genre_stats_updates_labels_only(self) -> None:
        section = self._section()
        carousel_before = section.carousel_section
        section.set_genre_stats({"Action": (120, 3), "RPG": (40, 0)})
        self.assertEqual(section.genre_buttons["Action"].text(), "Action (120 / 3)")
        self.assertEqual(section.genre_buttons["RPG"].text(), "RPG (40)")
        self.assertEqual(section.genre_buttons["Puzzle"].text(), "Puzzle")
        self.assertIs(section.carousel_section, carousel_before)
        self.assertEqual(section.selected_genre, "Action")


class RefreshButtonTests(_QtTestCase):

    def test_click_does_not_leak_checked_argument(self) -> None:
        page = DiscoverPageWidget(_FastCardWindow())
        calls: list[tuple] = []
        page.set_refresh_callback(lambda *args, **kwargs: calls.append((args, kwargs)))
        page.refresh_button.click()
        self.assertEqual(calls, [((), {})])

    def test_window_side_force_refresh_default_preserved(self) -> None:
        page = DiscoverPageWidget(_FastCardWindow())
        seen: list[bool] = []

        def refresh(force_refresh: bool = True) -> None:
            seen.append(force_refresh)

        page.set_refresh_callback(refresh)
        page.refresh_button.click()
        self.assertEqual(seen, [True])

    def test_rebinding_callback_replaces_previous(self) -> None:
        page = DiscoverPageWidget(_FastCardWindow())
        calls: list[str] = []
        page.set_refresh_callback(lambda: calls.append("old"))
        page.set_refresh_callback(lambda: calls.append("new"))
        page.refresh_button.click()
        self.assertEqual(calls, ["new"])


class CacheRenderTests(_QtTestCase):
    @classmethod
    def setUpClass(cls) -> None:
        super().setUpClass()
        cls.module = _load_main_module()

    def _warm_cache(self) -> DiscoverCache:
        cache = DiscoverCache(ttl=3600)
        cache.set_section("short_games", {"games": [_game("Short", "1", platform="SNES")], "genres": ["Action"]})
        cache.set_section("new_games", {"games": [_game("New", "2", platform="SNES", created_at=_iso(3600))]})
        cache.set_section("highly_rated", {"games": [_game("Rated", "3", rating="4.5")]})
        cache.set_section("recommendations", {"games": [_game("Rec", "4")]})
        cache.set_section(
            "platforms_list",
            {"platforms": [{"id": 9, "display_name": "N64", "games": [_game("Plat", "5", platform="N64")]}]},
        )
        cache.set_section(
            "genres",
            {"genres": ["Action"], "games_by_genre": {"Action": [_game("Genre", "6", genres="Action")]}},
        )
        cache.set_section("genre_totals", {"totals": {"Action": 120}})
        return cache

    def _stub(self, cache: DiscoverCache) -> _MainStub:
        window = _MainStub(self.module, cache)
        window.discover_page = DiscoverPageWidget(window)
        return window

    def test_result_from_cache_has_expected_shape(self) -> None:
        window = self._stub(self._warm_cache())
        result = window._discover_result_from_cache()
        self.assertEqual(result["short_games"]["games"][0]["title"], "Short")
        self.assertEqual(result["new_games"]["games"][0]["title"], "New")
        self.assertEqual(result["highly_rated"]["games"][0]["title"], "Rated")
        self.assertEqual(result["recommendations"]["games"][0]["title"], "Rec")
        self.assertEqual(result["platforms"][0]["display_name"], "N64")
        self.assertEqual(result["genres"]["genres"], ["Action"])
        self.assertEqual(result["genre_totals"], {"Action": 120})

    def test_result_from_empty_cache_is_empty(self) -> None:
        window = self._stub(DiscoverCache(ttl=3600))
        self.assertEqual(window._discover_result_from_cache(), {})

    def test_render_from_cache_builds_all_sections(self) -> None:
        window = self._stub(self._warm_cache())
        window._render_discover_result(window._discover_result_from_cache(), from_cache=True)
        sections = window.discover_page.sections
        for section_id in ("short_games", "new_games", "highly_rated", "recommendations", "platform_9", "genres", "watchlist"):
            self.assertIn(section_id, sections)

    def test_render_from_cache_does_not_save_to_disk(self) -> None:
        window = self._stub(self._warm_cache())
        window._render_discover_result(window._discover_result_from_cache(), from_cache=True)
        self.assertEqual(window.saved_to_disk, [])

    def test_render_live_result_saves_to_disk(self) -> None:
        window = self._stub(self._warm_cache())
        window._render_discover_result(window._discover_result_from_cache(), from_cache=False)
        self.assertEqual(len(window.saved_to_disk), 1)

    def test_render_from_cache_helper_returns_true_and_sets_label(self) -> None:
        window = self._stub(self._warm_cache())
        self.assertTrue(window._render_discover_from_cache())
        self.assertFalse(window.discover_page.is_loading)
        self.assertIn("Updated", window.discover_page.last_refresh_label.text())

    def test_render_from_empty_cache_returns_false(self) -> None:
        window = self._stub(DiscoverCache(ttl=3600))
        self.assertFalse(window._render_discover_from_cache())
        self.assertEqual(window.discover_page.sections, {})

    def test_hidden_sections_are_skipped(self) -> None:
        window = self._stub(self._warm_cache())
        window._ui_state = {"hidden_sections": ["new_games", "highly_rated"]}
        window._render_discover_result(window._discover_result_from_cache(), from_cache=True)
        self.assertNotIn("new_games", window.discover_page.sections)
        self.assertIn("short_games", window.discover_page.sections)

    def test_genre_stats_applied_from_result_totals(self) -> None:
        window = self._stub(self._warm_cache())
        window._render_discover_result(window._discover_result_from_cache(), from_cache=True)
        genre_section = window.discover_page.sections["genres"]
        self.assertEqual(genre_section.genre_buttons["Action"].text(), "Action (120)")

    def test_platform_see_all_navigates_to_platform(self) -> None:
        window = self._stub(self._warm_cache())
        window._render_discover_result(window._discover_result_from_cache(), from_cache=True)
        window.discover_page.sections["platform_9"].see_all_callback()
        self.assertEqual(window.navigated_platforms, ["N64"])

    def test_genre_see_all_navigates_to_genre(self) -> None:
        window = self._stub(self._warm_cache())
        window._render_discover_result(window._discover_result_from_cache(), from_cache=True)
        window.discover_page.sections["genres"].carousel_section.see_all_callback()
        self.assertEqual(window.switched_pages, [1])


class OfflineNoticeTests(_QtTestCase):
    @classmethod
    def setUpClass(cls) -> None:
        super().setUpClass()
        cls.module = _load_main_module()

    def _page(self) -> DiscoverPageWidget:
        page = DiscoverPageWidget(_FastCardWindow())
        page.show()
        return page

    def test_notice_hidden_by_default(self) -> None:
        self.assertFalse(self._page().offline_label.isVisible())

    def test_set_notice_with_timestamp_mentions_age(self) -> None:
        page = self._page()
        page.set_offline_notice(time.time() - 2 * 86400)
        self.assertTrue(page.offline_label.isVisible())
        self.assertIn("cached results from 2 days ago", page.offline_label.text())

    def test_set_notice_without_timestamp_says_no_cached_data(self) -> None:
        page = self._page()
        page.set_offline_notice(None)
        self.assertTrue(page.offline_label.isVisible())
        self.assertIn("no cached data", page.offline_label.text())

    def test_clear_notice_hides_label(self) -> None:
        page = self._page()
        page.set_offline_notice(None)
        page.clear_offline_notice()
        self.assertFalse(page.offline_label.isVisible())

    def test_offline_with_empty_cache_shows_no_cached_data(self) -> None:
        window = _MainStub(self.module, DiscoverCache(ttl=3600))
        window.discover_page = DiscoverPageWidget(window)
        window.discover_page.show()
        window._render_discover_offline()
        self.assertEqual(window.discover_page.sections, {})
        self.assertIn("no cached data", window.discover_page.offline_label.text())

    def test_offline_with_warm_cache_renders_sections_and_age(self) -> None:
        cache = DiscoverCache(ttl=3600)
        cache.set_section("short_games", {"games": [_game("Short", "1")]})
        window = _MainStub(self.module, cache)
        window.discover_page = DiscoverPageWidget(window)
        window.discover_page.show()
        window._render_discover_offline()
        self.assertIn("short_games", window.discover_page.sections)
        self.assertIn("Offline", window.discover_page.offline_label.text())
        self.assertIn("cached results from", window.discover_page.offline_label.text())

    def test_offline_does_not_hit_network_or_save(self) -> None:
        cache = DiscoverCache(ttl=3600)
        cache.set_section("short_games", {"games": [_game("Short", "1")]})
        window = _MainStub(self.module, cache)
        window.discover_page = DiscoverPageWidget(window)
        window._render_discover_offline()
        self.assertEqual(window.saved_to_disk, [])


class GameCardBadgeTests(_QtTestCase):

    def _card(self, game: dict, source: str = "discover", show_added_date: bool = False, library=None):
        window = _StubWindow(library_games=library or [])
        card = make_game_card(window, game, source, show_added_date)
        card.show()
        return card

    def test_new_badge_visible_for_recent_created_at(self) -> None:
        card = self._card(_game("Fresh", "1", created_at=_iso(60)))
        self.assertTrue(card.new_badge.isVisible())

    def test_new_badge_hidden_for_month_old_created_at(self) -> None:
        card = self._card(_game("Old", "1", created_at=_iso(30 * 86400)))
        self.assertFalse(card.new_badge.isVisible())

    def test_new_badge_hidden_without_created_at(self) -> None:
        card = self._card(_game("Unknown", "1"))
        self.assertFalse(card.new_badge.isVisible())

    def test_rating_badge_visible_with_value(self) -> None:
        card = self._card(_game("Rated", "1", rating="4.5"))
        self.assertTrue(card.rating_badge.isVisible())
        self.assertIn("4.5", card.rating_badge.text())

    def test_rating_badge_hidden_for_empty_rating(self) -> None:
        card = self._card(_game("Plain", "1", rating=""))
        self.assertFalse(card.rating_badge.isVisible())

    def test_rating_badge_hidden_for_na_rating(self) -> None:
        card = self._card(_game("Plain", "1", rating="N/A"))
        self.assertFalse(card.rating_badge.isVisible())

    def test_rating_badge_hidden_for_zero_rating(self) -> None:
        card = self._card(_game("Plain", "1", rating="0"))
        self.assertFalse(card.rating_badge.isVisible())

    def test_installed_badge_visible_when_in_library(self) -> None:
        game = _game("Owned", "77", platform="SNES")
        card = self._card(game, library=[_game("Owned", "77", platform="SNES")])
        self.assertTrue(card.installed_badge.isVisible())

    def test_installed_badge_hidden_when_not_in_library(self) -> None:
        game = _game("Not Owned", "77", platform="SNES")
        card = self._card(game, library=[_game("Something Else", "88", platform="SNES")])
        self.assertFalse(card.installed_badge.isVisible())

    def test_installed_badge_hidden_for_empty_library(self) -> None:
        card = self._card(_game("Solo", "1"))
        self.assertFalse(card.installed_badge.isVisible())

    def test_added_date_label_only_when_requested(self) -> None:
        game = _game("Dated", "1", created_at=_iso(3 * 86400))
        with_date = self._card(game, show_added_date=True)
        self.assertTrue(hasattr(with_date, "added_date_label"))
        self.assertIn("3 days ago", with_date.added_date_label.text())
        self.assertFalse(hasattr(self._card(game), "added_date_label"))

    def test_added_date_label_hidden_without_created_at(self) -> None:
        card = self._card(_game("Undated", "1"), show_added_date=True)
        self.assertFalse(card.added_date_label.isVisible())

    def test_bookmark_button_toggles_watchlist(self) -> None:
        window = _StubWindow()
        card = make_game_card(window, _game("Save Me", "42"), "discover")
        card.show()
        button = card.bookmark_btn
        self.assertEqual(button.text(), "")
        self.assertFalse(button.icon().isNull())
        outline_key = button.icon().cacheKey()
        outline_pixels = _widget_pixels(button)

        button.click()
        self.assertIn("42", window.watchlist_games)
        self.assertEqual(button.text(), "")
        self.assertFalse(button.icon().isNull())
        filled_key = button.icon().cacheKey()
        self.assertNotEqual(filled_key, outline_key)
        self.assertNotEqual(_widget_pixels(button), outline_pixels)

        button.click()
        self.assertNotIn("42", window.watchlist_games)
        self.assertEqual(button.text(), "")
        self.assertFalse(button.icon().isNull())
        self.assertNotEqual(button.icon().cacheKey(), filled_key)

    def test_non_discover_source_has_no_badges(self) -> None:
        card = self._card(_game("Library Game", "1", rating="4.5", created_at=_iso(60)), source="library")
        for attribute in ("new_badge", "rating_badge", "installed_badge", "bookmark_btn", "added_date_label"):
            self.assertFalse(hasattr(card, attribute), attribute)


class ThemedIconTests(_QtTestCase):
    """Regression guards for the SVG-icon migration (no Discover UI depends on font glyphs)."""

    ICON_ASSETS = (
        "svg/star.svg",
        "svg/star-outline.svg",
        "svg/chevron-down.svg",
        "svg/chevron-right.svg",
        "svg/chevron-up.svg",
        "svg/check.svg",
        "svg/config.svg",
    )

    # objectName -> fixed square size, mirroring the real Discover widgets.
    GLYPH_ONLY_BUTTONS = (
        ("gameCardBookmark", 24),
        ("discoverSectionToggle", 24),
        ("discoverPrefsButton", 28),
    )

    def _icon_button(self, object_name: str, size: int, with_icon: bool) -> QPushButton:
        button = QPushButton()
        button.setObjectName(object_name)
        button.setFixedSize(size, size)
        button.setIconSize(QSize(16, 16))
        button.setIcon(
            themed_svg_icon("svg/star-outline.svg", QColor("#ffb86c"), size=(16, 16))
            if with_icon
            else QIcon()
        )
        button.setStyleSheet(theme_stylesheet(theme_colors("dark")))
        button.show()
        return button

    def test_asset_files_exist(self) -> None:
        svg_dir = Path(__file__).resolve().parents[1] / "assets" / "svg"
        for asset in self.ICON_ASSETS:
            with self.subTest(asset=asset):
                self.assertTrue((svg_dir / Path(asset).name).is_file())

    def test_assets_render_non_null_icon_and_pixmap(self) -> None:
        for asset in self.ICON_ASSETS:
            with self.subTest(asset=asset):
                self.assertFalse(themed_svg_icon(asset, QColor("#f8f8f2"), size=(16, 16)).isNull())
                self.assertFalse(themed_svg_pixmap(asset, QColor("#f8f8f2"), size=QSize(16, 16)).isNull())

    def test_glyph_only_buttons_render_icon_under_app_stylesheet(self) -> None:
        # Guards the QSS override: the global `QPushButton { padding: 8px 14px }`
        # rule used to clip these fixed-size buttons to an empty content box.
        for object_name, size in self.GLYPH_ONLY_BUTTONS:
            with self.subTest(object_name=object_name):
                with_icon = _widget_pixels(self._icon_button(object_name, size, True))
                without_icon = _widget_pixels(self._icon_button(object_name, size, False))
                self.assertGreater(len(set(with_icon)), 1)
                self.assertNotEqual(with_icon, without_icon)

    def test_stylesheet_overrides_default_padding_for_glyph_only_buttons(self) -> None:
        # The global `QPushButton { padding: 8px 14px }` rule clips the content box of
        # these 24-28px buttons to nothing, so the override rule must stay in place.
        stylesheet = theme_stylesheet(theme_colors("dark"))
        selector = ",\n".join(
            f"        QPushButton#{object_name}" for object_name, _ in self.GLYPH_ONLY_BUTTONS
        )
        self.assertIn(f"{selector} {{\n            padding: 2px;", stylesheet)

    def test_collapse_toggle_uses_icon_that_changes_with_state(self) -> None:
        section = DiscoverCarouselSection("s1", "Title", [_game("A", "1")], _FastCardWindow())
        section.show()
        button = section._toggle_btn
        self.assertEqual(button.text(), "")
        self.assertFalse(button.icon().isNull())
        expanded_key = button.icon().cacheKey()

        section.toggle_collapsed()
        self.assertTrue(section.collapsed)
        self.assertFalse(button.icon().isNull())
        collapsed_key = button.icon().cacheKey()
        self.assertNotEqual(collapsed_key, expanded_key)

        section.toggle_collapsed()
        self.assertFalse(section.collapsed)
        self.assertNotEqual(button.icon().cacheKey(), collapsed_key)

    def test_prefs_button_uses_icon_without_text(self) -> None:
        page = DiscoverPageWidget(_FastCardWindow())
        page.show()
        self.assertEqual(page.prefs_button.text(), "")
        self.assertFalse(page.prefs_button.icon().isNull())

    def test_rating_badge_is_numeric_text_plus_star_icon(self) -> None:
        window = _StubWindow()
        card = make_game_card(window, _game("Rated", "1", rating="4.5"), "discover")
        card.show()
        self.assertEqual(card.rating_badge.text(), "4.5")
        self.assertNotIn("⭐", card.rating_badge.text())
        self.assertNotIn("★", card.rating_badge.text())
        self.assertFalse(card.rating_star_icon.pixmap().isNull())
        self.assertTrue(card.rating_star_icon.isVisible())
        self.assertEqual(card.rating_star_icon.isVisible(), card.rating_badge.isVisible())

    def test_rating_star_icon_hidden_without_rating(self) -> None:
        window = _StubWindow()
        card = make_game_card(window, _game("Plain", "1", rating=""), "discover")
        card.show()
        self.assertEqual(card.rating_badge.text(), "")
        self.assertFalse(card.rating_badge.isVisible())
        self.assertFalse(card.rating_star_icon.isVisible())

    def test_installed_badge_uses_check_pixmap_without_text(self) -> None:
        window = _StubWindow(library_games=[_game("Owned", "77", platform="SNES")])
        card = make_game_card(window, _game("Owned", "77", platform="SNES"), "discover")
        card.show()
        self.assertEqual(card.installed_badge.text(), "")
        self.assertFalse(card.installed_badge.pixmap().isNull())


class SeeAllTests(_QtTestCase):

    def test_genre_section_see_all_receives_selected_genre(self) -> None:
        calls: list[str] = []
        section = DiscoverGenreSection(
            "genres", ["Action", "RPG"],
            {"Action": [_game("A", "1")], "RPG": [_game("B", "2")]},
            _FastCardWindow(), calls.append,
        )
        section.carousel_section.see_all_callback()
        self.assertEqual(calls, ["Action"])

    def test_genre_section_see_all_follows_pill_selection(self) -> None:
        calls: list[str] = []
        section = DiscoverGenreSection(
            "genres", ["Action", "RPG"],
            {"Action": [_game("A", "1")], "RPG": [_game("B", "2")]},
            _FastCardWindow(), calls.append,
        )
        section.genre_buttons["RPG"].click()
        section.carousel_section.see_all_callback()
        self.assertEqual(calls, ["RPG"])

    def test_genre_section_without_callback_has_no_see_all(self) -> None:
        section = DiscoverGenreSection(
            "genres", ["Action"], {"Action": [_game("A", "1")]}, _FastCardWindow(), None
        )
        self.assertIsNone(section.carousel_section.see_all_callback)

    def test_carousel_see_all_button_triggers_callback(self) -> None:
        from PySide6.QtWidgets import QPushButton
        calls: list[str] = []
        section = DiscoverCarouselSection(
            "s1", "Title", [_game("A", "1")], _FastCardWindow(), lambda: calls.append("clicked")
        )
        buttons = [b for b in section.findChildren(QPushButton) if "See All" in b.text()]
        self.assertEqual(len(buttons), 1)
        buttons[0].click()
        self.assertEqual(calls, ["clicked"])

    def test_carousel_without_callback_has_no_see_all_button(self) -> None:
        from PySide6.QtWidgets import QPushButton
        section = DiscoverCarouselSection("s1", "Title", [_game("A", "1")], _FastCardWindow())
        buttons = [b for b in section.findChildren(QPushButton) if "See All" in b.text()]
        self.assertEqual(buttons, [])


class LastRefreshTickTests(_QtTestCase):

    def _page(self) -> DiscoverPageWidget:
        page = DiscoverPageWidget(_FastCardWindow())
        page.show()
        return page

    def test_tick_updates_text_for_aged_timestamp(self) -> None:
        page = self._page()
        page.update_last_refresh_time(time.time())
        self.assertEqual(page.last_refresh_label.text(), "Updated just now")
        page._last_refresh_time = time.time() - 605
        page._tick_last_refresh_label()
        self.assertEqual(page.last_refresh_label.text(), "Updated 10 minutes ago")

    def test_tick_updates_to_hours(self) -> None:
        page = self._page()
        page._last_refresh_time = time.time() - 2 * 3600
        page._tick_last_refresh_label()
        self.assertEqual(page.last_refresh_label.text(), "Updated 2 hours ago")

    def test_tick_without_refresh_time_is_noop(self) -> None:
        page = self._page()
        page._tick_last_refresh_label()
        self.assertEqual(page.last_refresh_label.text(), "")
        self.assertFalse(page.last_refresh_label.isVisible())

    def test_update_with_zero_hides_label(self) -> None:
        page = self._page()
        page.update_last_refresh_time(time.time())
        page.update_last_refresh_time(0)
        self.assertFalse(page.last_refresh_label.isVisible())


class AutoRefreshGateTests(_QtTestCase):
    @classmethod
    def setUpClass(cls) -> None:
        super().setUpClass()
        cls.module = _load_main_module()

    def _stub(self, age_seconds: float) -> _MainStub:
        cache = DiscoverCache(ttl=3600)
        cache.set_section("short_games", {"games": [_game("Short", "1")]})
        cache.cache["short_games"]["timestamp"] = time.time() - age_seconds
        window = _MainStub(self.module, cache)
        window.discover_page = DiscoverPageWidget(window)
        return window

    def test_refreshes_when_cache_is_a_week_old(self) -> None:
        window = self._stub(7 * 86400 + 60)
        window._auto_refresh_stale_discover()
        self.assertEqual(window.refresh_calls, [True])

    def test_no_refresh_when_cache_is_fresh(self) -> None:
        window = self._stub(6 * 86400)
        window._auto_refresh_stale_discover()
        self.assertEqual(window.refresh_calls, [])

    def test_no_refresh_when_server_disconnected(self) -> None:
        window = self._stub(8 * 86400)
        window.server_connected = False
        window._auto_refresh_stale_discover()
        self.assertEqual(window.refresh_calls, [])

    def test_no_refresh_while_worker_running(self) -> None:
        window = self._stub(8 * 86400)
        window._discover_load_thread = _ThreadStub(True)
        window._auto_refresh_stale_discover()
        self.assertEqual(window.refresh_calls, [])

    def test_refresh_when_worker_finished(self) -> None:
        window = self._stub(8 * 86400)
        window._discover_load_thread = _ThreadStub(False)
        window._auto_refresh_stale_discover()
        self.assertEqual(window.refresh_calls, [True])

    def test_no_refresh_without_discover_page(self) -> None:
        window = self._stub(8 * 86400)
        window.discover_page = None
        window._auto_refresh_stale_discover()
        self.assertEqual(window.refresh_calls, [])

    def test_no_refresh_with_empty_cache(self) -> None:
        window = _MainStub(self.module, DiscoverCache(ttl=3600))
        window.discover_page = DiscoverPageWidget(window)
        window._auto_refresh_stale_discover()
        self.assertEqual(window.refresh_calls, [])


class WatchlistUITests(_QtTestCase):
    @classmethod
    def setUpClass(cls) -> None:
        super().setUpClass()
        cls.module = _load_main_module()

    def setUp(self) -> None:
        fd, self.path = tempfile.mkstemp(suffix=".json")
        os.close(fd)
        os.unlink(self.path)

    def tearDown(self) -> None:
        if os.path.exists(self.path):
            os.unlink(self.path)

    def _stub(self, watchlist: dict | None = None) -> _MainStub:
        window = _MainStub(self.module, DiscoverCache(ttl=3600), watchlist_path=self.path)
        window.watchlist_games = watchlist if watchlist is not None else {}
        window.discover_page = DiscoverPageWidget(window)
        window.discover_page.add_carousel_section("short_games", "Short But Fun", [_game("Short", "1")])
        window.discover_page.add_watchlist_section(window._watchlist_section_games())
        return window

    def test_toggle_adds_game_and_rebuilds_watchlist_section(self) -> None:
        window = self._stub()
        other_before = window.discover_page.sections["short_games"]
        window.toggle_watchlist(_game("Saved", "42", platform="SNES"))
        self.assertEqual(window.watchlist_games["42"]["title"], "Saved")
        self.assertIsInstance(window.discover_page.sections["watchlist"], DiscoverCarouselSection)
        self.assertEqual(len(window.discover_page.sections["watchlist"].game_cards), 1)
        self.assertIs(window.discover_page.sections["short_games"], other_before)

    def test_toggle_persists_to_disk(self) -> None:
        window = self._stub()
        window.toggle_watchlist(_game("Saved", "42", platform="SNES"))
        self.assertEqual(load_watchlist(self.path)["42"]["platform"], "SNES")

    def test_toggle_twice_removes_and_shows_placeholder(self) -> None:
        from PySide6.QtWidgets import QLabel
        window = self._stub()
        game = _game("Saved", "42")
        window.toggle_watchlist(game)
        window.toggle_watchlist(game)
        self.assertEqual(window.watchlist_games, {})
        container = window.discover_page.sections["watchlist"]
        labels = list(container.findChildren(QLabel))
        self.assertTrue(any("No saved games" in label.text() for label in labels))

    def test_toggle_keeps_watchlist_section_position(self) -> None:
        window = self._stub()
        index_before = window.discover_page.content_layout.indexOf(
            window.discover_page.sections["watchlist"]
        )
        window.toggle_watchlist(_game("Saved", "42"))
        index_after = window.discover_page.content_layout.indexOf(
            window.discover_page.sections["watchlist"]
        )
        self.assertEqual(index_before, index_after)

    def test_old_format_entries_hidden_from_section(self) -> None:
        window = self._stub({"1": {}, "2": _game("Has Data", "2")})
        self.assertEqual([g["title"] for g in window._watchlist_section_games()], ["Has Data"])

    def test_hydrate_fills_and_persists_old_format_entry(self) -> None:
        window = self._stub({"7": {}})
        window._hydrate_watchlist_entries([_game("Resurfaced", "7"), _game("Unrelated", "8")])
        self.assertEqual(window.watchlist_games["7"]["title"], "Resurfaced")
        self.assertNotIn("8", window.watchlist_games)
        self.assertEqual(load_watchlist(self.path)["7"]["title"], "Resurfaced")

    def test_hydrated_entry_becomes_visible_in_section(self) -> None:
        window = self._stub({"7": {}})
        self.assertEqual(window._watchlist_section_games(), [])
        window._hydrate_watchlist_entries([_game("Resurfaced", "7")])
        window.discover_page.add_watchlist_section(window._watchlist_section_games())
        self.assertEqual(len(window.discover_page.sections["watchlist"].game_cards), 1)

    def test_hydrate_without_matches_writes_nothing(self) -> None:
        window = self._stub({"7": {}})
        window._hydrate_watchlist_entries([_game("Unrelated", "9")])
        self.assertFalse(os.path.exists(self.path))

    def test_render_result_hydrates_watchlist_from_sections(self) -> None:
        window = self._stub({"5": {}})
        window._render_discover_result(
            {"short_games": {"games": [_game("Hydrate Me", "5", platform="SNES")]}},
            from_cache=True,
        )
        self.assertEqual(window.watchlist_games["5"]["title"], "Hydrate Me")
        self.assertEqual(len(window.discover_page.sections["watchlist"].game_cards), 1)


class WatchlistCardSyncTests(_QtTestCase):
    """The same game can be on screen in several sections; every star must follow the toggle."""

    @classmethod
    def setUpClass(cls) -> None:
        super().setUpClass()
        cls.module = _load_main_module()

    def setUp(self) -> None:
        fd, self.path = tempfile.mkstemp(suffix=".json")
        os.close(fd)
        os.unlink(self.path)

    def tearDown(self) -> None:
        if os.path.exists(self.path):
            os.unlink(self.path)

    def _window(self, watchlist: dict | None = None) -> _CardMainStub:
        window = _CardMainStub(self.module, DiscoverCache(ttl=3600), watchlist_path=self.path)
        window.watchlist_games = watchlist if watchlist is not None else {}
        window.discover_page = DiscoverPageWidget(window)
        return window

    def _twin_cards(self, watchlist: dict | None = None) -> tuple[_CardMainStub, Any, Any]:
        window = self._window(watchlist)
        window.discover_page.add_carousel_section("short_games", "Short But Fun", [_game("Twin", "42")])
        window.discover_page.add_carousel_section(
            "highly_rated", "Highly Rated", [_game("Twin", "42"), _game("Other", "7")]
        )
        card_a = window.discover_page.sections["short_games"].game_cards[0]
        card_b = window.discover_page.sections["highly_rated"].game_cards[0]
        card_a.show()
        card_b.show()
        return window, card_a, card_b

    def _reference_pixels(self, watchlisted: bool) -> list[int]:
        window = _StubWindow()
        if watchlisted:
            window.watchlist_games["42"] = _game("Twin", "42")
        card = make_game_card(window, _game("Twin", "42"), "discover")
        card.show()
        return _widget_pixels(card.bookmark_btn)

    def test_toggle_on_updates_star_on_other_section_card(self) -> None:
        window, card_a, card_b = self._twin_cards()
        before_key = card_b.bookmark_btn.icon().cacheKey()

        card_a.bookmark_btn.click()

        self.assertIn("42", window.watchlist_games)
        self.assertNotEqual(card_b.bookmark_btn.icon().cacheKey(), before_key)
        self.assertEqual(_widget_pixels(card_b.bookmark_btn), self._reference_pixels(True))
        self.assertEqual(card_b.bookmark_btn.toolTip(), "Remove from watchlist")

    def test_toggle_off_updates_star_on_other_section_card(self) -> None:
        window, card_a, card_b = self._twin_cards({"42": _game("Twin", "42")})
        before_key = card_b.bookmark_btn.icon().cacheKey()

        card_a.bookmark_btn.click()

        self.assertNotIn("42", window.watchlist_games)
        self.assertNotEqual(card_b.bookmark_btn.icon().cacheKey(), before_key)
        self.assertEqual(_widget_pixels(card_b.bookmark_btn), self._reference_pixels(False))
        self.assertEqual(card_b.bookmark_btn.toolTip(), "Add to watchlist")

    def test_toggle_leaves_unrelated_card_untouched(self) -> None:
        window, card_a, _ = self._twin_cards()
        other = window.discover_page.sections["highly_rated"].game_cards[1]
        other.show()
        before_key = other.bookmark_btn.icon().cacheKey()

        card_a.bookmark_btn.click()

        self.assertEqual(other.bookmark_btn.icon().cacheKey(), before_key)
        self.assertEqual(other.bookmark_btn.toolTip(), "Add to watchlist")

    def test_sync_reaches_genre_section_cards(self) -> None:
        window = self._window()
        window.discover_page.add_genre_section(["Action"], {"Action": [_game("Twin", "42", genres="Action")]})
        card = window.discover_page.sections["genres"].carousel_section.game_cards[0]
        card.show()
        before_key = card.bookmark_btn.icon().cacheKey()

        window.toggle_watchlist(_game("Twin", "42", genres="Action"))

        self.assertNotEqual(card.bookmark_btn.icon().cacheKey(), before_key)
        self.assertEqual(card.bookmark_btn.toolTip(), "Remove from watchlist")

    def test_sync_without_matching_rom_id_is_a_no_op(self) -> None:
        window, _, card_b = self._twin_cards()
        before_key = card_b.bookmark_btn.icon().cacheKey()
        window.discover_page.sync_watchlist_state("999")
        self.assertEqual(card_b.bookmark_btn.icon().cacheKey(), before_key)

    def test_set_watchlisted_updates_icon_and_tooltip(self) -> None:
        card = make_game_card(_StubWindow(), _game("Twin", "42"), "discover")
        card.show()
        self.assertEqual(card.watchlist_rom_id, "42")
        outline_key = card.bookmark_btn.icon().cacheKey()

        card.set_watchlisted(True)
        self.assertNotEqual(card.bookmark_btn.icon().cacheKey(), outline_key)
        self.assertEqual(card.bookmark_btn.toolTip(), "Remove from watchlist")
        self.assertEqual(_widget_pixels(card.bookmark_btn), self._reference_pixels(True))

        card.set_watchlisted(False)
        self.assertEqual(card.bookmark_btn.toolTip(), "Add to watchlist")
        self.assertEqual(_widget_pixels(card.bookmark_btn), self._reference_pixels(False))


if __name__ == "__main__":
    unittest.main()
