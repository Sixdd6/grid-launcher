"""Tests for Discover tab functionality."""

import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import unittest
from unittest.mock import Mock, call, patch

from grid_launcher.server.discover import (
    DiscoverCache,
    client_filter_games,
    fetch_all_games,
    fetch_games_by_platform,
    fetch_highly_rated_games,
    fetch_new_games,
    fetch_recommendations,
    fetch_server_platforms,
    fetch_short_games,
    filter_games_by_installed,
    filter_unexplored_platforms,
    genre_stats_from_games,
    get_top_genres_from_games,
    load_watchlist,
    normalize_discover_item,
    record_discover_event,
    save_watchlist,
)


class TestDiscoverCache(unittest.TestCase):
    """Test DiscoverCache class."""

    def setUp(self) -> None:
        self.cache = DiscoverCache(ttl=10)

    def test_cache_set_and_get(self) -> None:
        data = {"games": [{"id": 1, "title": "Game 1"}]}
        self.cache.set_section("test", data)
        result = self.cache.get_section("test")
        self.assertEqual(result, data)

    def test_cache_expiration(self) -> None:
        import time
        cache = DiscoverCache(ttl=0)
        data = {"games": [{"id": 1}]}
        cache.set_section("test", data)
        time.sleep(0.01)
        result = cache.get_section("test")
        self.assertIsNone(result)

    def test_cache_force_refresh(self) -> None:
        data = {"games": [{"id": 1}]}
        self.cache.set_section("test", data)
        result = self.cache.get_section("test", force_refresh=True)
        self.assertIsNone(result)

    def test_cache_invalidation(self) -> None:
        data = {"games": [{"id": 1}]}
        self.cache.set_section("test", data)
        self.cache.invalidate_section("test")
        result = self.cache.get_section("test")
        self.assertIsNone(result)

    def test_installed_games_filter(self) -> None:
        games = [
            {"name": "Game A", "title": "Game A"},
            {"name": "Game B", "title": "Game B"},
        ]
        self.cache.set_installed_games(games)
        self.assertIn("game a", self.cache.installed_game_keys)
        self.assertIn("game b", self.cache.installed_game_keys)

    def test_is_stale(self) -> None:
        self.assertTrue(self.cache.is_stale("nonexistent"))
        self.cache.set_section("test", {"games": []})
        self.assertFalse(self.cache.is_stale("test"))

    def test_clear_cache(self) -> None:
        self.cache.set_section("test1", {"games": []})
        self.cache.set_section("test2", {"games": []})
        self.cache.clear()
        self.assertIsNone(self.cache.get_section("test1"))
        self.assertIsNone(self.cache.get_section("test2"))


class TestDiscoverFiltering(unittest.TestCase):
    """Test discover filtering functions."""

    def test_filter_games_by_installed(self) -> None:
        games = [
            {"title": "Game A", "rating": 4.5},
            {"title": "Game B", "rating": 3.5},
            {"title": "Game C", "rating": 4.0},
        ]
        installed = {"game a", "game c"}
        result = filter_games_by_installed(games, installed)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["title"], "Game B")

    def test_filter_games_empty_installed(self) -> None:
        games = [{"title": "Game A"}, {"title": "Game B"}]
        result = filter_games_by_installed(games, set())
        self.assertEqual(len(result), 2)

    def test_filter_games_non_dict_entries(self) -> None:
        games = [{"title": "Game A"}, None, "bad", {"title": "Game B"}]
        result = filter_games_by_installed(games, set())  # type: ignore[arg-type]
        self.assertEqual(len(result), 2)

    def test_get_top_genres_from_games(self) -> None:
        all_games = [
            {"genres": "Action,Adventure", "rating": 4.5},
            {"genres": "Action,Platformer", "rating": 4.0},
            {"genres": "RPG,Adventure", "rating": 3.5},
            {"genres": "Puzzle", "rating": 3.0},
        ]
        installed_games = [{"genres": "Action,Adventure"}]
        result = get_top_genres_from_games(all_games, installed_games, top_n=3)
        self.assertEqual(len(result), 3)
        self.assertIn("Action", result)
        self.assertIn("Adventure", result)


class TestNormalizeDiscoverItem(unittest.TestCase):
    """Test normalize_discover_item field mapping."""

    def test_basic_fields(self) -> None:
        item = {
            "id": 42,
            "name": "My Game",
            "platform_display_name": "SNES",
            "url_cover": "http://example.com/cover.jpg",
            "rating": 4.5,
            "summary": "A great game.",
        }
        result = normalize_discover_item(item)
        self.assertEqual(result["title"], "My Game")
        self.assertEqual(result["platform"], "SNES")
        self.assertEqual(result["cover_url"], "http://example.com/cover.jpg")
        self.assertEqual(result["rom_id"], "42")
        self.assertEqual(result["rating"], "4.5")
        self.assertEqual(result["description"], "A great game.")

    def test_cover_fallback_order(self) -> None:
        item = {"id": 1, "path_cover_large": "http://large.jpg"}
        result = normalize_discover_item(item)
        self.assertEqual(result["cover_url"], "http://large.jpg")

    def test_genres_list_of_dicts(self) -> None:
        item = {"id": 1, "genres": [{"name": "Action"}, {"name": "RPG"}]}
        result = normalize_discover_item(item)
        self.assertIn("Action", result["genres"])
        self.assertIn("RPG", result["genres"])

    def test_missing_fields_default_to_empty(self) -> None:
        result = normalize_discover_item({"id": 5})
        self.assertEqual(result["title"], "")
        self.assertEqual(result["cover_url"], "")
        self.assertEqual(result["rating"], "")

    def test_boolean_like_fields_are_strings(self) -> None:
        result = normalize_discover_item({"id": 1})
        self.assertEqual(result["ps4_has_update"], "false")
        self.assertEqual(result["update_available"], "false")


class TestDiscoverAPI(unittest.TestCase):
    """Test API fetch functions with mocked api_get_json."""

    @patch("grid_launcher.server.discover.api_get_json")
    def test_fetch_all_games_returns_games_and_genres(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [
                {"id": 1, "name": "Game 1"},
                {"id": 2, "name": "Game 2"},
            ],
            "filter_values": {
                "genres": ["Action", "RPG"],
            },
        }
        games, genres = fetch_all_games("http://test", "token", limit=20)
        self.assertEqual(len(games), 2)
        self.assertEqual(games[0]["title"], "Game 1")
        self.assertIn("Action", genres)
        self.assertIn("RPG", genres)
        mock_api.assert_called_once()
        call_params = mock_api.call_args[0][3]
        self.assertEqual(call_params["with_filter_values"], "true")
        self.assertEqual(call_params["with_char_index"], "false")
        # Ensure no order_by param that would cause 422
        self.assertNotIn("order_by", call_params)

    @patch("grid_launcher.server.discover.api_get_json")
    def test_fetch_all_games_api_error_returns_empty(self, mock_api: Mock) -> None:
        mock_api.side_effect = Exception("connection refused")
        games, genres = fetch_all_games("http://test", "token")
        self.assertEqual(games, [])
        self.assertEqual(genres, [])

    @patch("grid_launcher.server.discover.api_get_json")
    def test_fetch_all_games_non_dict_response(self, mock_api: Mock) -> None:
        mock_api.return_value = "unexpected string"
        games, genres = fetch_all_games("http://test", "token")
        self.assertEqual(games, [])
        self.assertEqual(genres, [])

    @patch("grid_launcher.server.discover.api_get_json")
    def test_fetch_all_games_no_filter_values(self, mock_api: Mock) -> None:
        mock_api.return_value = {"items": [{"id": 1, "name": "Solo"}]}
        games, genres = fetch_all_games("http://test", "token")
        self.assertEqual(len(games), 1)
        self.assertEqual(genres, [])

    @patch("grid_launcher.server.discover.api_get_json")
    def test_fetch_all_games_genre_dicts(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [],
            "filter_values": {"genres": [{"name": "Puzzle"}, {"name": "Strategy"}]},
        }
        _, genres = fetch_all_games("http://test", "token")
        self.assertIn("Puzzle", genres)
        self.assertIn("Strategy", genres)


class TestDiscoverCacheDiskPersistence(unittest.TestCase):
    """Test DiscoverCache save_to_disk / load_from_disk."""

    def setUp(self) -> None:
        import tempfile
        fd, self.path = tempfile.mkstemp(suffix=".json")
        import os
        os.close(fd)
        os.unlink(self.path)

    def tearDown(self) -> None:
        import os
        if os.path.exists(self.path):
            os.unlink(self.path)

    def test_save_and_load(self) -> None:
        cache = DiscoverCache(ttl=10)
        cache.set_section("trending", {"games": [{"id": 1, "title": "Game 1"}]})
        cache.save_to_disk(self.path)

        fresh = DiscoverCache(ttl=10)
        fresh.load_from_disk(self.path)
        self.assertEqual(
            fresh.cache["trending"]["data"],
            {"games": [{"id": 1, "title": "Game 1"}]},
        )

    def test_load_nonexistent_file(self) -> None:
        cache = DiscoverCache(ttl=10)
        cache.load_from_disk(self.path + ".missing")
        self.assertEqual(cache.cache, {})

    def test_load_corrupt_file(self) -> None:
        with open(self.path, "w", encoding="utf-8") as fh:
            fh.write("not valid json {{{")
        cache = DiscoverCache(ttl=10)
        cache.load_from_disk(self.path)
        self.assertEqual(cache.cache, {})

    def test_load_stale_does_not_overwrite_fresh(self) -> None:
        cache = DiscoverCache(ttl=10)
        cache.set_section("A", {"games": ["fresh"]})

        disk = DiscoverCache(ttl=10)
        disk.set_section("A", {"games": ["stale"]})
        disk.save_to_disk(self.path)

        cache.load_from_disk(self.path)
        self.assertEqual(cache.cache["A"]["data"], {"games": ["fresh"]})

    def test_load_max_age_skips_old_entry(self) -> None:
        import json
        import time
        from pathlib import Path
        Path(self.path).write_text(
            json.dumps(
                {"old": {"data": {"games": []}, "timestamp": time.time() - 8 * 86400}}
            ),
            encoding="utf-8",
        )
        cache = DiscoverCache(ttl=10)
        cache.load_from_disk(self.path, max_age=7 * 86400)
        self.assertNotIn("old", cache.cache)

    def test_load_max_age_keeps_fresh_entry(self) -> None:
        import json
        import time
        from pathlib import Path
        Path(self.path).write_text(
            json.dumps(
                {"fresh": {"data": {"games": []}, "timestamp": time.time() - 3600}}
            ),
            encoding="utf-8",
        )
        cache = DiscoverCache(ttl=10)
        cache.load_from_disk(self.path, max_age=7 * 86400)
        self.assertIn("fresh", cache.cache)

    def test_load_no_max_age_keeps_old_entry(self) -> None:
        import json
        import time
        from pathlib import Path
        Path(self.path).write_text(
            json.dumps(
                {"old": {"data": {"games": []}, "timestamp": time.time() - 8 * 86400}}
            ),
            encoding="utf-8",
        )
        cache = DiscoverCache(ttl=10)
        cache.load_from_disk(self.path)
        self.assertIn("old", cache.cache)


class TestSetInstalledPlatformNames(unittest.TestCase):
    """Test DiscoverCache.set_installed_platform_names."""

    def test_basic(self) -> None:
        cache = DiscoverCache()
        cache.set_installed_platform_names(
            [{"platform": "SNES"}, {"platform": "Nintendo 64"}]
        )
        self.assertIn("snes", cache.installed_platform_names)
        self.assertIn("nintendo 64", cache.installed_platform_names)

    def test_missing_platform_key_ignored(self) -> None:
        cache = DiscoverCache()
        cache.set_installed_platform_names([{"platform": "SNES"}, {"title": "no plat"}])
        self.assertEqual(cache.installed_platform_names, {"snes"})

    def test_non_dict_entries_ignored(self) -> None:
        cache = DiscoverCache()
        cache.set_installed_platform_names(
            [{"platform": "SNES"}, None, "bad", 42]  # type: ignore[list-item]
        )
        self.assertEqual(cache.installed_platform_names, {"snes"})


class TestFetchNewGames(unittest.TestCase):
    """Test fetch_new_games with mocked api_get_json."""

    @patch("grid_launcher.server.discover.api_get_json")
    def test_returns_normalized_games(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [{"id": 1, "name": "New Game"}],
        }
        games = fetch_new_games("http://test", "token")
        self.assertEqual(len(games), 1)
        self.assertEqual(games[0]["title"], "New Game")
        params = mock_api.call_args[0][3]
        self.assertEqual(params["order_by"], "created_at")
        self.assertEqual(params["order_dir"], "desc")

    @patch("grid_launcher.server.discover.api_get_json")
    def test_api_error_returns_empty(self, mock_api: Mock) -> None:
        mock_api.side_effect = Exception("boom")
        self.assertEqual(fetch_new_games("http://test", "token"), [])

    @patch("grid_launcher.server.discover.api_get_json")
    def test_empty_items_returns_empty(self, mock_api: Mock) -> None:
        mock_api.return_value = {"items": []}
        self.assertEqual(fetch_new_games("http://test", "token"), [])


class TestFetchHighlyRatedGames(unittest.TestCase):
    """Test fetch_highly_rated_games with mocked api_get_json."""

    @patch("grid_launcher.server.discover.api_get_json")
    def test_returns_normalized_games(self, mock_api: Mock) -> None:
        mock_api.return_value = {"items": [{"id": 7, "name": "Top Game"}]}
        games = fetch_highly_rated_games("http://test", "token")
        self.assertEqual(games[0]["title"], "Top Game")
        params = mock_api.call_args[0][3]
        self.assertEqual(params["order_by"], "average_rating")
        self.assertEqual(params["order_dir"], "desc")

    @patch("grid_launcher.server.discover.api_get_json")
    def test_api_error_returns_empty(self, mock_api: Mock) -> None:
        mock_api.side_effect = Exception("boom")
        self.assertEqual(fetch_highly_rated_games("http://test", "token"), [])


class TestFetchServerPlatforms(unittest.TestCase):
    """Test fetch_server_platforms with mocked api_get_json."""

    @patch("grid_launcher.server.discover.api_get_json")
    def test_returns_list_of_dicts(self, mock_api: Mock) -> None:
        mock_api.return_value = [{"id": 1, "name": "SNES"}, {"id": 2, "name": "N64"}]
        result = fetch_server_platforms("http://test", "token")
        self.assertEqual(len(result), 2)
        self.assertEqual(result[0]["name"], "SNES")

    @patch("grid_launcher.server.discover.api_get_json")
    def test_filters_non_dicts(self, mock_api: Mock) -> None:
        mock_api.return_value = ["bad", {"id": 1}, None]
        result = fetch_server_platforms("http://test", "token")
        self.assertEqual(result, [{"id": 1}])

    @patch("grid_launcher.server.discover.api_get_json")
    def test_api_error_returns_empty(self, mock_api: Mock) -> None:
        mock_api.side_effect = Exception("boom")
        self.assertEqual(fetch_server_platforms("http://test", "token"), [])

    @patch("grid_launcher.server.discover.api_get_json")
    def test_non_list_response_returns_empty(self, mock_api: Mock) -> None:
        mock_api.return_value = {"data": []}
        self.assertEqual(fetch_server_platforms("http://test", "token"), [])


class TestFilterUnexploredPlatforms(unittest.TestCase):
    """Test filter_unexplored_platforms."""

    def test_excludes_installed_platforms(self) -> None:
        platforms = [
            {"display_name": "SNES", "rom_count": 100},
            {"display_name": "N64", "rom_count": 50},
        ]
        result = filter_unexplored_platforms(platforms, {"snes"}, max_platforms=10)
        names = [p["display_name"] for p in result]
        self.assertNotIn("SNES", names)
        self.assertIn("N64", names)

    def test_excludes_zero_rom_count(self) -> None:
        platforms = [
            {"display_name": "SNES", "rom_count": 0},
            {"display_name": "N64", "rom_count": 5},
        ]
        result = filter_unexplored_platforms(platforms, set(), max_platforms=10)
        names = [p["display_name"] for p in result]
        self.assertNotIn("SNES", names)
        self.assertIn("N64", names)

    def test_sorted_by_rom_count_desc(self) -> None:
        platforms = [
            {"display_name": "A", "rom_count": 10},
            {"display_name": "B", "rom_count": 50},
            {"display_name": "C", "rom_count": 30},
        ]
        result = filter_unexplored_platforms(platforms, set(), max_platforms=10)
        self.assertEqual([p["display_name"] for p in result], ["B", "C", "A"])

    def test_max_platforms_capped(self) -> None:
        platforms = [
            {"display_name": f"P{i}", "rom_count": i + 1} for i in range(10)
        ]
        result = filter_unexplored_platforms(platforms, set(), max_platforms=3)
        self.assertEqual(len(result), 3)

    def test_empty_installed_names(self) -> None:
        platforms = [
            {"display_name": "SNES", "rom_count": 100},
            {"display_name": "N64", "rom_count": 50},
        ]
        result = filter_unexplored_platforms(platforms, set(), max_platforms=10)
        self.assertEqual(len(result), 2)

    def test_name_match_also_excluded(self) -> None:
        platforms = [
            {"name": "snes", "rom_count": 100},
            {"name": "n64", "rom_count": 50},
        ]
        result = filter_unexplored_platforms(platforms, {"snes"}, max_platforms=10)
        names = [p["name"] for p in result]
        self.assertNotIn("snes", names)
        self.assertIn("n64", names)


class TestFetchGamesByPlatform(unittest.TestCase):
    """Test fetch_games_by_platform with mocked api_get_json."""

    @patch("grid_launcher.server.discover.api_get_json")
    def test_passes_platform_id(self, mock_api: Mock) -> None:
        mock_api.return_value = {"items": [{"id": 1, "name": "PlatGame"}]}
        games = fetch_games_by_platform("http://test", "token", 123)
        self.assertEqual(games[0]["title"], "PlatGame")
        params = mock_api.call_args[0][3]
        self.assertEqual(params["platform_ids"], [123])

    @patch("grid_launcher.server.discover.api_get_json")
    def test_api_error_returns_empty(self, mock_api: Mock) -> None:
        mock_api.side_effect = Exception("boom")
        self.assertEqual(fetch_games_by_platform("http://test", "token", 123), [])


class TestFetchRecommendations(unittest.TestCase):
    """Test fetch_recommendations with mocked fetch_games_by_genre."""

    def test_returns_empty_for_empty_library(self) -> None:
        result = fetch_recommendations("url", "token", [], set())
        self.assertEqual(result, [])

    @patch("grid_launcher.server.discover.fetch_games_by_genre")
    def test_deduplicates_by_rom_id(self, mock_fetch: Mock) -> None:
        mock_fetch.return_value = [{"title": "Game X", "rom_id": "1"}]
        library_games = [
            {"genres": "Action, RPG"},
            {"genres": "RPG"},
        ]
        result = fetch_recommendations("url", "token", library_games, set())
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["rom_id"], "1")

    @patch("grid_launcher.server.discover.fetch_games_by_genre")
    def test_filters_installed_games(self, mock_fetch: Mock) -> None:
        mock_fetch.return_value = [{"title": "Installed Game", "rom_id": "5"}]
        library_games = [{"genres": "Action"}]
        result = fetch_recommendations(
            "url", "token", library_games, {"installed game"}
        )
        self.assertEqual(result, [])

    @patch("grid_launcher.server.discover.fetch_games_by_genre")
    def test_api_error_returns_empty(self, mock_fetch: Mock) -> None:
        mock_fetch.side_effect = Exception("network error")
        library_games = [{"genres": "Action"}]
        result = fetch_recommendations("url", "token", library_games, set())
        self.assertEqual(result, [])

    @patch("grid_launcher.server.discover.fetch_games_by_genre")
    def test_preferred_platforms_filters_results(self, mock_fetch: Mock) -> None:
        mock_fetch.return_value = [
            {"title": "SNES Game", "rom_id": "1", "platform": "SNES", "genres": "Action"},
            {"title": "PS1 Game", "rom_id": "2", "platform": "PS1", "genres": "Action"},
        ]
        library_games = [{"genres": "Action"} for _ in range(5)]
        result = fetch_recommendations(
            "url", "token", library_games, set(), preferred_platforms={"SNES"}
        )
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["platform"], "SNES")

    @patch("grid_launcher.server.discover.fetch_games_by_genre")
    def test_empty_preferred_platforms_returns_all(self, mock_fetch: Mock) -> None:
        mock_fetch.return_value = [
            {"title": "SNES Game", "rom_id": "1", "platform": "SNES", "genres": "Action"},
            {"title": "PS1 Game", "rom_id": "2", "platform": "PS1", "genres": "Action"},
        ]
        library_games = [{"genres": "Action"} for _ in range(5)]
        result = fetch_recommendations(
            "url", "token", library_games, set(), preferred_platforms=set()
        )
        self.assertEqual(len(result), 2)

    @patch("grid_launcher.server.discover.fetch_games_by_genre")
    def test_none_preferred_platforms_returns_all(self, mock_fetch: Mock) -> None:
        mock_fetch.return_value = [
            {"title": "SNES Game", "rom_id": "1", "platform": "SNES", "genres": "Action"},
            {"title": "PS1 Game", "rom_id": "2", "platform": "PS1", "genres": "Action"},
        ]
        library_games = [{"genres": "Action"} for _ in range(5)]
        result = fetch_recommendations(
            "url", "token", library_games, set(), preferred_platforms=None
        )
        self.assertEqual(len(result), 2)


class _MockWindow:
    def _open_game_details(self, game, source):
        pass

    def _theme_color(self, role, fallback):
        return fallback

    def _make_game_card(self, game, source):
        from PySide6.QtWidgets import QWidget
        return QWidget()

    def _clear_layout(self, layout):
        pass


class TestUpdateLastRefreshTime(unittest.TestCase):
    """Test DiscoverPageWidget.update_last_refresh_time formatting."""

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self) -> None:
        from grid_launcher.ui.discover import DiscoverPageWidget
        self.page = DiscoverPageWidget(_MockWindow(), None)
        self.page.show()

    def test_formats_just_now(self) -> None:
        import time
        self.page.update_last_refresh_time(time.time())
        self.assertEqual(self.page.last_refresh_label.text(), "Updated just now")
        self.assertTrue(self.page.last_refresh_label.isVisible())

    def test_formats_minutes_ago(self) -> None:
        import time
        self.page.update_last_refresh_time(time.time() - 305)
        self.assertIn("5 minutes ago", self.page.last_refresh_label.text())

    def test_formats_hours_ago(self) -> None:
        import time
        self.page.update_last_refresh_time(time.time() - 7200)
        self.assertIn("2 hours ago", self.page.last_refresh_label.text())

    def test_formats_days_ago(self) -> None:
        import time
        self.page.update_last_refresh_time(time.time() - 3 * 86400)
        self.assertIn("3 days ago", self.page.last_refresh_label.text())

    def test_hidden_when_ts_zero(self) -> None:
        self.page.update_last_refresh_time(0)
        self.assertFalse(self.page.last_refresh_label.isVisible())


class TestCollapseToggle(unittest.TestCase):
    """Test collapse toggle behavior on carousel sections and the page."""

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def test_carousel_toggle_hides_content(self) -> None:
        from grid_launcher.ui.discover import DiscoverCarouselSection
        section = DiscoverCarouselSection("s1", "Title", [], _MockWindow())
        section.show()
        self.assertTrue(section._content_scroll.isVisible())
        section.toggle_collapsed()
        self.assertFalse(section._content_scroll.isVisible())

    def test_carousel_toggle_shows_again(self) -> None:
        from grid_launcher.ui.discover import DiscoverCarouselSection
        section = DiscoverCarouselSection("s1", "Title", [], _MockWindow())
        section.show()
        section.toggle_collapsed()
        section.toggle_collapsed()
        self.assertTrue(section._content_scroll.isVisible())

    def test_carousel_apply_collapsed_no_signal(self) -> None:
        from grid_launcher.ui.discover import DiscoverCarouselSection
        section = DiscoverCarouselSection("s1", "Title", [], _MockWindow())
        listener = Mock()
        section.collapsed_changed.connect(listener)
        section.apply_collapsed(True)
        listener.assert_not_called()
        self.assertTrue(section.collapsed)

    def test_carousel_toggle_emits_signal(self) -> None:
        from grid_launcher.ui.discover import DiscoverCarouselSection
        section = DiscoverCarouselSection("s1", "Title", [], _MockWindow())
        listener = Mock()
        section.collapsed_changed.connect(listener)
        section.toggle_collapsed()
        self.assertTrue(listener.called)
        self.assertEqual(listener.call_args, call("s1", True))

    def test_page_tracks_collapse_state(self) -> None:
        from grid_launcher.ui.discover import DiscoverPageWidget
        page = DiscoverPageWidget(_MockWindow())
        page.add_carousel_section("s1", "Title", [])
        page.sections["s1"].toggle_collapsed()
        self.assertTrue(page._collapsed_states.get("s1"))


class TestClientFilterGames(unittest.TestCase):

    def test_empty_filters_returns_all_games(self) -> None:
        games = [
            {"title": "A", "genres": "Action", "platform": "SNES"},
            {"title": "B", "genres": "RPG", "platform": "PS1"},
        ]
        result = client_filter_games(games, set(), set())
        self.assertEqual(len(result), 2)

    def test_genre_filter_matches_substring(self) -> None:
        games = [
            {"title": "A", "genres": "Action, RPG", "platform": "SNES"},
            {"title": "B", "genres": "Puzzle", "platform": "PS1"},
        ]
        result = client_filter_games(games, {"Action"}, set())
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["title"], "A")

    def test_platform_filter_case_insensitive(self) -> None:
        games = [
            {"title": "A", "genres": "Action", "platform": "SNES"},
            {"title": "B", "genres": "RPG", "platform": "PS1"},
        ]
        result = client_filter_games(games, set(), {"snes"})
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["platform"], "SNES")

    def test_combined_genre_and_platform(self) -> None:
        games = [
            {"title": "A", "genres": "Action", "platform": "SNES"},
            {"title": "B", "genres": "Action", "platform": "PS1"},
            {"title": "C", "genres": "RPG", "platform": "SNES"},
        ]
        result = client_filter_games(games, {"Action"}, {"SNES"})
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["title"], "A")

    def test_non_matching_filter_returns_empty(self) -> None:
        games = [{"title": "A", "genres": "Action", "platform": "SNES"}]
        result = client_filter_games(games, {"RPG"}, set())
        self.assertEqual(result, [])


class TestGenreStatsFromGames(unittest.TestCase):

    def test_counts_total_correctly(self) -> None:
        all_games = [{"genres": "Action, RPG"}, {"genres": "Action"}]
        result = genre_stats_from_games(all_games, [])
        self.assertEqual(result["Action"], (2, 0))
        self.assertEqual(result["RPG"], (1, 0))

    def test_counts_installed_correctly(self) -> None:
        all_games = [{"genres": "Action"}, {"genres": "RPG"}]
        installed_games = [{"genres": "Action"}]
        result = genre_stats_from_games(all_games, installed_games)
        self.assertEqual(result["Action"], (1, 1))
        self.assertEqual(result["RPG"], (1, 0))

    def test_empty_games_returns_empty_dict(self) -> None:
        self.assertEqual(genre_stats_from_games([], []), {})

    def test_empty_genre_string_skipped(self) -> None:
        all_games = [{"genres": ""}, {"genres": "Action"}]
        result = genre_stats_from_games(all_games, [])
        self.assertIn("Action", result)
        self.assertNotIn("", result)


class TestWatchlistPersistence(unittest.TestCase):

    def setUp(self) -> None:
        import tempfile
        fd, self.path = tempfile.mkstemp(suffix=".json")
        os.close(fd)
        os.unlink(self.path)

    def tearDown(self) -> None:
        if os.path.exists(self.path):
            os.unlink(self.path)

    def test_load_missing_file_returns_empty_set(self) -> None:
        self.assertEqual(load_watchlist(self.path), set())

    def test_roundtrip(self) -> None:
        save_watchlist(self.path, {"rom1", "rom2"})
        self.assertEqual(load_watchlist(self.path), {"rom1", "rom2"})

    def test_load_corrupt_file_returns_empty_set(self) -> None:
        with open(self.path, "w", encoding="utf-8") as fh:
            fh.write("not json")
        self.assertEqual(load_watchlist(self.path), set())

    def test_load_non_list_json_returns_empty_set(self) -> None:
        with open(self.path, "w", encoding="utf-8") as fh:
            fh.write('{"key": "value"}')
        self.assertEqual(load_watchlist(self.path), set())

    def test_save_writes_sorted_list(self) -> None:
        import json
        save_watchlist(self.path, {"c", "a", "b"})
        with open(self.path, "r", encoding="utf-8") as fh:
            result = json.load(fh)
        self.assertEqual(result, ["a", "b", "c"])


class TestRecordDiscoverEvent(unittest.TestCase):

    def setUp(self) -> None:
        import tempfile
        fd, self.path = tempfile.mkstemp(suffix=".jsonl")
        os.close(fd)
        os.unlink(self.path)

    def tearDown(self) -> None:
        if os.path.exists(self.path):
            os.unlink(self.path)

    def test_writes_jsonl_line(self) -> None:
        import json
        record_discover_event(self.path, "card_opened", "all_games", "42")
        with open(self.path, "r", encoding="utf-8") as fh:
            lines = [ln for ln in fh.read().splitlines() if ln.strip()]
        self.assertEqual(len(lines), 1)
        data = json.loads(lines[0])
        self.assertEqual(data["event"], "card_opened")
        self.assertEqual(data["section_id"], "all_games")
        self.assertEqual(data["rom_id"], "42")
        self.assertIn("ts", data)

    def test_appends_multiple_lines(self) -> None:
        import json
        record_discover_event(self.path, "card_opened", "all_games", "1")
        record_discover_event(self.path, "card_opened", "all_games", "2")
        with open(self.path, "r", encoding="utf-8") as fh:
            lines = [ln for ln in fh.read().splitlines() if ln.strip()]
        self.assertEqual(len(lines), 2)
        self.assertTrue(all(json.loads(ln) for ln in lines))

    def test_skips_write_when_over_1mb(self) -> None:
        with open(self.path, "w", encoding="utf-8") as fh:
            fh.write("x" * 1_048_577)
        size_before = os.path.getsize(self.path)
        record_discover_event(self.path, "card_opened", "all_games", "42")
        size_after = os.path.getsize(self.path)
        self.assertEqual(size_after, size_before)
        self.assertGreater(size_after, 1_048_576)

    def test_ignores_error_on_bad_path(self) -> None:
        try:
            record_discover_event(
                "/nonexistent_dir/subdir/file.jsonl", "card_opened", "s", "1"
            )
        except Exception as exc:  # pragma: no cover
            self.fail(f"record_discover_event raised: {exc}")


def _discover_game(title: str, rom_id: str = "1", genres: str = "", platform: str = "") -> dict:
    return {
        "title": title, "platform": platform, "genres": genres, "rom_id": rom_id,
        "cover_url": "", "rating": "", "description": "", "regions": "", "languages": "",
        "companies": "", "release_year": "", "filesize_bytes": "", "revision": "", "tags": "",
        "fanart_url": "", "first_release_date": "", "server_updated_at": "",
        "rom_file_name": "", "rom_nested_file_name": "", "rom_base_file_id": "",
        "ra_id": "", "ps4_has_update": "false", "ps4_has_dlc": "false",
        "ps4_file_ids_by_category": "{}", "xbox360_has_update": "false",
        "xbox360_has_dlc": "false", "xbox360_file_ids_by_category": "{}",
        "update_available": "false", "screenshot_urls": "",
    }


class TestFilterPanel(unittest.TestCase):

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def _panel(self):
        from grid_launcher.ui.discover import DiscoverFilterPanel
        return DiscoverFilterPanel()

    def test_populate_creates_genre_buttons(self) -> None:
        panel = self._panel()
        panel.populate(["Action", "RPG"], [])
        self.assertIn("Action", panel._genre_checks)
        self.assertIn("RPG", panel._genre_checks)

    def test_populate_creates_platform_buttons(self) -> None:
        panel = self._panel()
        panel.populate([], ["SNES", "PS1"])
        self.assertIn("SNES", panel._platform_checks)

    def test_filters_changed_emitted_on_check(self) -> None:
        panel = self._panel()
        panel.populate(["Action"], [])
        listener = Mock()
        panel.filters_changed.connect(listener)
        panel._genre_checks["Action"].click()
        self.assertTrue(listener.called)
        self.assertIn("Action", listener.call_args[0][0])

    def test_clear_unchecks_all(self) -> None:
        panel = self._panel()
        panel.populate(["Action", "RPG"], ["SNES"])
        panel._genre_checks["Action"].setChecked(True)
        panel._genre_checks["RPG"].setChecked(True)
        panel._platform_checks["SNES"].setChecked(True)
        panel.clear()
        self.assertEqual(panel.selected_genres, set())
        self.assertEqual(panel.selected_platforms, set())
        self.assertFalse(any(btn.isChecked() for btn in panel._genre_checks.values()))
        self.assertFalse(any(btn.isChecked() for btn in panel._platform_checks.values()))

    def test_selected_genres_property(self) -> None:
        panel = self._panel()
        panel.populate(["Action", "RPG"], [])
        panel._genre_checks["Action"].click()
        self.assertEqual(panel.selected_genres, {"Action"})


class TestDiscoverCarouselUpdateGames(unittest.TestCase):

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def test_update_games_repopulates_cards(self) -> None:
        from grid_launcher.ui.discover import DiscoverCarouselSection
        section = DiscoverCarouselSection(
            "s1", "Title", [_discover_game("A")], _MockWindow()
        )
        self.assertEqual(len(section.game_cards), 1)
        section.update_games([_discover_game("B", "2"), _discover_game("C", "3")])
        self.assertEqual(len(section.game_cards), 2)

    def test_update_games_with_empty_hides_scroll(self) -> None:
        from grid_launcher.ui.discover import DiscoverCarouselSection
        section = DiscoverCarouselSection(
            "s1", "Title", [_discover_game("A")], _MockWindow()
        )
        section.show()
        section.update_games([])
        self.assertFalse(section._content_scroll.isVisible())

    def test_update_games_with_games_shows_scroll(self) -> None:
        from grid_launcher.ui.discover import DiscoverCarouselSection
        section = DiscoverCarouselSection("s1", "Title", [], _MockWindow())
        section.show()
        section.update_games([_discover_game("A", "")])
        self.assertTrue(section._content_scroll.isVisible())


class TestDiscoverPageWatchlistSection(unittest.TestCase):

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def _page(self):
        from grid_launcher.ui.discover import DiscoverPageWidget
        return DiscoverPageWidget(_MockWindow())

    def test_add_watchlist_section_with_games_appends_carousel(self) -> None:
        page = self._page()
        page.add_carousel_section("all_games", "Games", [])
        page.add_watchlist_section([_discover_game("WL Game")])
        self.assertIn("watchlist", page.sections)
        last_index = page.content_layout.count() - 1
        self.assertIs(page.content_layout.itemAt(last_index).widget(), page.sections["watchlist"])

    def test_add_watchlist_section_empty_shows_placeholder(self) -> None:
        from PySide6.QtWidgets import QLabel
        page = self._page()
        page.add_watchlist_section([])
        self.assertIn("watchlist", page.sections)
        container = page.sections["watchlist"]
        labels = list(container.findChildren(QLabel))
        self.assertTrue(any("No saved games" in lbl.text() for lbl in labels))

    def test_add_watchlist_section_replaces_existing(self) -> None:
        from PySide6.QtCore import QEvent
        from PySide6.QtWidgets import QApplication
        page = self._page()
        page.add_watchlist_section([_discover_game("Old", "1")])
        page.add_watchlist_section([_discover_game("New", "2")])
        QApplication.sendPostedEvents(None, QEvent.Type.DeferredDelete)
        self.app.processEvents()
        self.assertEqual(page.content_layout.count(), 1)
        self.assertEqual(len(page.sections), 1)


class TestDiscoverPageFilterIntegration(unittest.TestCase):

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def _page(self):
        from grid_launcher.ui.discover import DiscoverPageWidget
        return DiscoverPageWidget(_MockWindow())

    def test_set_filter_options_populates_panel(self) -> None:
        page = self._page()
        page.set_filter_options(["Action", "RPG"], ["SNES"])
        self.assertIn("Action", page._filter_panel._genre_checks)

    def test_apply_filters_updates_carousel_sections(self) -> None:
        page = self._page()
        games_action = [_discover_game("A", "1", "Action", "SNES")]
        games_rpg = [_discover_game("B", "2", "RPG", "PS1")]
        page.add_carousel_section("s1", "Test", games_action + games_rpg)
        page._on_filters_changed({"Action"}, set())
        section = page.sections["s1"]
        self.assertEqual(len(section.game_cards), 1)

    def test_active_filters_stored_after_change(self) -> None:
        page = self._page()
        page._on_filters_changed({"RPG"}, {"PS1"})
        self.assertEqual(page._active_genre_filter, {"RPG"})
        self.assertEqual(page._active_platform_filter, {"PS1"})


class TestFetchShortGames(unittest.TestCase):
    @patch("grid_launcher.server.discover.api_get_json")
    def test_short_games_returned_first(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [
                {"id": 1, "name": "Game A", "hltb_metadata": {"main_story": 600}},
                {"id": 2, "name": "Game B", "hltb_metadata": {"main_story": 3600}},
                {"id": 3, "name": "Game C", "hltb_metadata": {"main_story": 1200}},
                {"id": 4, "name": "Game D", "hltb_metadata": {"main_story": 1201}},
            ]
        }
        games, _ = fetch_short_games("http://test", "token", limit=10)
        titles = [g["title"] for g in games]
        idx_a = titles.index("Game A")
        idx_c = titles.index("Game C")
        idx_b = titles.index("Game B")
        idx_d = titles.index("Game D")
        self.assertLess(idx_a, idx_b)
        self.assertLess(idx_a, idx_d)
        self.assertLess(idx_c, idx_b)
        self.assertLess(idx_c, idx_d)

    @patch("grid_launcher.server.discover.api_get_json")
    def test_games_over_threshold_still_included_when_short_pool_small(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [
                {"id": 1, "name": "Game A", "hltb_metadata": {"main_story": 3600}},
                {"id": 2, "name": "Game B", "hltb_metadata": {"main_story": 5000}},
            ]
        }
        games, _ = fetch_short_games("http://test", "token", limit=5)
        self.assertTrue(games)

    @patch("grid_launcher.server.discover.api_get_json")
    def test_zero_main_story_goes_to_other(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [
                {"id": 1, "name": "Game A", "hltb_metadata": {"main_story": 0}},
                {"id": 2, "name": "Game B", "hltb_metadata": {}},
            ]
        }
        games, _ = fetch_short_games("http://test", "token", limit=10)
        self.assertEqual(len(games), 2)

    @patch("grid_launcher.server.discover.api_get_json")
    def test_no_hltb_metadata_handled_gracefully(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [
                {"id": 1, "name": "Game A", "hltb_metadata": None},
                {"id": 2, "name": "Game B"},
            ]
        }
        games, _ = fetch_short_games("http://test", "token", limit=10)
        self.assertEqual(len(games), 2)

    @patch("grid_launcher.server.discover.api_get_json")
    def test_api_error_returns_empty(self, mock_api: Mock) -> None:
        mock_api.side_effect = Exception("connection refused")
        result = fetch_short_games("http://test", "token")
        self.assertEqual(result, ([], []))

    @patch("grid_launcher.server.discover.api_get_json")
    def test_genres_returned_alongside_games(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [
                {"id": 1, "name": "Game A", "hltb_metadata": {"main_story": 600}},
            ],
            "filter_values": {"genres": ["Action", "RPG"]},
        }
        _, genres = fetch_short_games("http://test", "token", limit=10)
        self.assertIn("Action", genres)
        self.assertIn("RPG", genres)

    @patch("grid_launcher.server.discover.api_get_json")
    def test_limit_respected(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [
                {"id": i, "name": f"Game {i}", "hltb_metadata": {"main_story": 300}}
                for i in range(30)
            ]
        }
        games, _ = fetch_short_games("http://test", "token", limit=5)
        self.assertLessEqual(len(games), 5)


if __name__ == "__main__":
    unittest.main()

