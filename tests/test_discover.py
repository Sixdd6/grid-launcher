"""Tests for Discover tab functionality."""

import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import unittest
from typing import Any
from unittest.mock import Mock, call, patch

from grid_launcher.server.view import filter_server_games
from grid_launcher.server.discover import (
    DiscoverCache,
    client_filter_games,
    fetch_all_games,
    fetch_games_by_platform,
    fetch_genre_totals,
    fetch_highly_rated_games,
    fetch_new_games,
    fetch_recommendations,
    fetch_server_platforms,
    fetch_short_games,
    filter_games_by_installed,
    filter_unexplored_platforms,
    genre_stats_from_games,
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

    def test_concurrent_set_section_keeps_all_writes(self) -> None:
        from concurrent.futures import ThreadPoolExecutor

        def write(index: int) -> None:
            self.cache.set_section(f"section{index}", {"games": [index]})
            self.cache.is_stale(f"section{index}")
            self.cache.get_section(f"section{index}")

        with ThreadPoolExecutor(max_workers=8) as executor:
            for future in [executor.submit(write, i) for i in range(50)]:
                future.result()

        for i in range(50):
            self.assertEqual(self.cache.get_section(f"section{i}"), {"games": [i]})


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

    def test_created_at_passed_through(self) -> None:
        result = normalize_discover_item({"id": 1, "created_at": "2024-05-01T12:00:00.000000"})
        self.assertEqual(result["created_at"], "2024-05-01T12:00:00.000000")

    def test_created_at_missing_is_empty(self) -> None:
        self.assertEqual(normalize_discover_item({"id": 1})["created_at"], "")


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
        mock_api.return_value = {"items": [{"id": 7, "name": "Top Game", "rating": 4.5}]}
        games = fetch_highly_rated_games("http://test", "token")
        self.assertEqual(games[0]["title"], "Top Game")
        params = mock_api.call_args[0][3]
        self.assertEqual(params["order_by"], "average_rating")
        self.assertEqual(params["order_dir"], "desc")

    @patch("grid_launcher.server.discover.api_get_json")
    def test_api_error_returns_empty(self, mock_api: Mock) -> None:
        mock_api.side_effect = Exception("boom")
        self.assertEqual(fetch_highly_rated_games("http://test", "token"), [])

    @patch("grid_launcher.server.discover.api_get_json")
    def test_ratings_below_threshold_excluded(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [
                {"id": 1, "name": "Great", "rating": 4.0},
                {"id": 2, "name": "Okay", "rating": 3.9},
                {"id": 3, "name": "Best", "rating": 5},
            ]
        }
        titles = [g["title"] for g in fetch_highly_rated_games("http://test", "token")]
        self.assertEqual(titles, ["Great", "Best"])

    @patch("grid_launcher.server.discover.api_get_json")
    def test_missing_or_zero_rating_excluded(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [
                {"id": 1, "name": "No Rating"},
                {"id": 2, "name": "Zero", "rating": 0},
                {"id": 3, "name": "Unparseable", "rating": "n/a"},
            ]
        }
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

    def test_totals_override_sample_counts(self) -> None:
        all_games = [{"genres": "Action, RPG"}, {"genres": "Action"}]
        installed_games = [{"genres": "Action"}]
        result = genre_stats_from_games(all_games, installed_games, {"Action": 512, "RPG": 30})
        self.assertEqual(result["Action"], (512, 1))
        self.assertEqual(result["RPG"], (30, 0))

    def test_totals_include_genres_missing_from_sample(self) -> None:
        result = genre_stats_from_games([{"genres": "Action"}], [], {"Puzzle": 7})
        self.assertEqual(result["Puzzle"], (7, 0))
        self.assertEqual(result["Action"], (1, 0))


class TestFetchGenreTotals(unittest.TestCase):
    """Test fetch_genre_totals with mocked api_get_json."""

    @patch("grid_launcher.server.discover.api_get_json")
    def test_returns_total_per_genre(self, mock_api: Mock) -> None:
        mock_api.side_effect = [{"total": 120, "items": []}, {"total": 8, "items": []}]
        totals = fetch_genre_totals("http://test", "token", ["Action", "RPG"])
        self.assertEqual(totals, {"Action": 120, "RPG": 8})
        self.assertEqual(mock_api.call_count, 2)

    @patch("grid_launcher.server.discover.api_get_json")
    def test_request_params(self, mock_api: Mock) -> None:
        mock_api.return_value = {"total": 1, "items": []}
        fetch_genre_totals("http://test", "token", ["Action"])
        params = mock_api.call_args[0][3]
        self.assertEqual(params["genres"], ["Action"])
        self.assertEqual(params["limit"], 1)
        self.assertEqual(params["with_filter_values"], "false")

    @patch("grid_launcher.server.discover.api_get_json")
    def test_failing_genre_omitted(self, mock_api: Mock) -> None:
        mock_api.side_effect = [Exception("boom"), {"total": 5, "items": []}]
        totals = fetch_genre_totals("http://test", "token", ["Action", "RPG"])
        self.assertEqual(totals, {"RPG": 5})

    @patch("grid_launcher.server.discover.api_get_json")
    def test_non_dict_or_missing_total_omitted(self, mock_api: Mock) -> None:
        mock_api.side_effect = ["bad", {"items": []}]
        totals = fetch_genre_totals("http://test", "token", ["Action", "RPG"])
        self.assertEqual(totals, {})

    def test_empty_genres_returns_empty(self) -> None:
        self.assertEqual(fetch_genre_totals("http://test", "token", []), {})


class TestWatchlistPersistence(unittest.TestCase):

    def setUp(self) -> None:
        import tempfile
        fd, self.path = tempfile.mkstemp(suffix=".json")
        os.close(fd)
        os.unlink(self.path)

    def tearDown(self) -> None:
        if os.path.exists(self.path):
            os.unlink(self.path)

    def test_load_missing_file_returns_empty_dict(self) -> None:
        self.assertEqual(load_watchlist(self.path), {})

    def test_roundtrip_keeps_game_dicts(self) -> None:
        entries = {
            "1": {"title": "Game One", "platform": "SNES", "rom_id": "1"},
            "2": {"title": "Game Two", "platform": "PS1", "rom_id": "2"},
        }
        save_watchlist(self.path, entries)
        self.assertEqual(load_watchlist(self.path), entries)

    def test_load_corrupt_file_returns_empty_dict(self) -> None:
        with open(self.path, "w", encoding="utf-8") as fh:
            fh.write("not json")
        self.assertEqual(load_watchlist(self.path), {})

    def test_load_non_container_json_returns_empty_dict(self) -> None:
        with open(self.path, "w", encoding="utf-8") as fh:
            fh.write('"just a string"')
        self.assertEqual(load_watchlist(self.path), {})

    def test_load_skips_non_dict_values(self) -> None:
        with open(self.path, "w", encoding="utf-8") as fh:
            fh.write('{"1": {"title": "Ok"}, "2": "bad"}')
        self.assertEqual(load_watchlist(self.path), {"1": {"title": "Ok"}})

    def test_save_writes_mapping(self) -> None:
        import json
        save_watchlist(self.path, {"7": {"title": "Seven", "rom_id": "7"}})
        with open(self.path, "r", encoding="utf-8") as fh:
            result = json.load(fh)
        self.assertEqual(result, {"7": {"title": "Seven", "rom_id": "7"}})

    def test_old_format_list_loads_as_ids_without_card_data(self) -> None:
        with open(self.path, "w", encoding="utf-8") as fh:
            fh.write('["12", "34"]')
        loaded = load_watchlist(self.path)
        self.assertEqual(set(loaded), {"12", "34"})
        self.assertEqual(loaded["12"], {})
        self.assertEqual(loaded["34"], {})


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

    def test_add_watchlist_section_rebuilds_in_place(self) -> None:
        page = self._page()
        page.add_watchlist_section([_discover_game("Old", "1")])
        page.add_carousel_section("later", "Later", [])
        page.add_watchlist_section([_discover_game("New", "2")])
        self.assertEqual(page.content_layout.indexOf(page.sections["watchlist"]), 0)
        self.assertEqual(page.content_layout.indexOf(page.sections["later"]), 1)

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


class TestDiscoverGenreSection(unittest.TestCase):
    """Test genre pill selection rebuilding the carousel."""

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def _section(self):
        from grid_launcher.ui.discover import DiscoverGenreSection
        games_by_genre = {
            "Action": [_discover_game("A", "1")],
            "RPG": [_discover_game("B", "2"), _discover_game("C", "3")],
        }
        return DiscoverGenreSection("genres", ["Action", "RPG"], games_by_genre, _MockWindow())

    def test_first_genre_carousel_rendered_on_init(self) -> None:
        section = self._section()
        self.assertEqual(section.selected_genre, "Action")
        self.assertIsNotNone(section.carousel_section)
        self.assertEqual(len(section.carousel_section.game_cards), 1)

    def test_clicking_pill_rebuilds_carousel_for_clicked_genre(self) -> None:
        section = self._section()
        section.genre_buttons["RPG"].click()
        self.assertEqual(section.selected_genre, "RPG")
        self.assertEqual(section.carousel_section.title, "Top RPG Games")
        self.assertEqual(len(section.carousel_section.game_cards), 2)
        self.assertTrue(section.genre_buttons["RPG"].isChecked())
        self.assertFalse(section.genre_buttons["Action"].isChecked())

    def test_genre_without_games_clears_carousel(self) -> None:
        from grid_launcher.ui.discover import DiscoverGenreSection
        section = DiscoverGenreSection(
            "genres", ["Action", "Puzzle"], {"Action": [_discover_game("A")]}, _MockWindow()
        )
        section.genre_buttons["Puzzle"].click()
        self.assertIsNone(section.carousel_section)
        self.assertEqual(section.selected_genre, "Puzzle")

    def test_set_genre_stats_only_updates_labels(self) -> None:
        section = self._section()
        carousel_before = section.carousel_section
        section.set_genre_stats({"Action": (120, 3), "RPG": (40, 0)})
        self.assertEqual(section.genre_buttons["Action"].text(), "Action (120 / 3)")
        self.assertEqual(section.genre_buttons["RPG"].text(), "RPG (40)")
        self.assertIs(section.carousel_section, carousel_before)
        self.assertEqual(section.selected_genre, "Action")


class TestDiscoverPageRefreshCallback(unittest.TestCase):

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def test_refresh_click_calls_callback_without_checked_arg(self) -> None:
        from grid_launcher.ui.discover import DiscoverPageWidget
        page = DiscoverPageWidget(_MockWindow())
        calls: list[tuple] = []
        page.set_refresh_callback(lambda *args: calls.append(args))
        page.refresh_button.click()
        self.assertEqual(calls, [()])

    def test_set_refresh_callback_twice_calls_only_latest(self) -> None:
        from grid_launcher.ui.discover import DiscoverPageWidget
        page = DiscoverPageWidget(_MockWindow())
        calls: list[str] = []
        page.set_refresh_callback(lambda: calls.append("first"))
        page.set_refresh_callback(lambda: calls.append("second"))
        page.refresh_button.click()
        self.assertEqual(calls, ["second"])


class TestDiscoverLoadWorker(unittest.TestCase):
    """Test the parallelized DiscoverLoadWorker.run()."""

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def _worker(self, library_games=None):
        from grid_launcher.background.workers import DiscoverLoadWorker
        return DiscoverLoadWorker(
            "http://test",
            "token",
            DiscoverCache(ttl=3600),
            True,
            library_games=library_games if library_games is not None else [],
        )

    @patch("grid_launcher.server.discover.fetch_genre_totals")
    @patch("grid_launcher.server.discover.fetch_games_by_genre")
    @patch("grid_launcher.server.discover.fetch_games_by_platform")
    @patch("grid_launcher.server.discover.fetch_server_platforms")
    @patch("grid_launcher.server.discover.fetch_recommendations")
    @patch("grid_launcher.server.discover.fetch_highly_rated_games")
    @patch("grid_launcher.server.discover.fetch_new_games")
    @patch("grid_launcher.server.discover.fetch_short_games")
    def test_emits_finished_with_all_sections(
        self,
        mock_short: Mock,
        mock_new: Mock,
        mock_rated: Mock,
        mock_recs: Mock,
        mock_platforms: Mock,
        mock_platform_games: Mock,
        mock_genre_games: Mock,
        mock_totals: Mock,
    ) -> None:
        mock_short.return_value = ([_discover_game("Short")], ["Action", "RPG"])
        mock_new.return_value = [_discover_game("New", "2")]
        mock_rated.return_value = [_discover_game("Rated", "3")]
        mock_recs.return_value = [_discover_game("Rec", "4")]
        mock_platforms.return_value = [
            {"id": 9, "name": "n64", "display_name": "N64", "rom_count": 30}
        ]
        mock_platform_games.return_value = [_discover_game("Platform Game", "5")]
        mock_genre_games.return_value = [_discover_game("Genre Game", "6")]
        mock_totals.return_value = {"Action": 120, "RPG": 40}

        worker = self._worker(library_games=[{"name": f"g{i}"} for i in range(20)])
        results: list[dict] = []
        errors: list[str] = []
        worker.finished.connect(results.append)
        worker.error.connect(errors.append)
        worker.run()

        self.assertEqual(errors, [])
        self.assertEqual(len(results), 1)
        result = results[0]
        for key in (
            "short_games",
            "new_games",
            "highly_rated",
            "recommendations",
            "platforms",
            "genres",
            "genre_totals",
        ):
            self.assertIn(key, result)
        self.assertEqual(result["genre_totals"], {"Action": 120, "RPG": 40})
        self.assertEqual(result["platforms"][0]["display_name"], "N64")
        self.assertEqual(worker.cache.get_section("genre_totals"), {"totals": {"Action": 120, "RPG": 40}})

    @patch("grid_launcher.server.discover.fetch_genre_totals")
    @patch("grid_launcher.server.discover.fetch_games_by_genre")
    @patch("grid_launcher.server.discover.fetch_server_platforms")
    @patch("grid_launcher.server.discover.fetch_highly_rated_games")
    @patch("grid_launcher.server.discover.fetch_new_games")
    @patch("grid_launcher.server.discover.fetch_short_games")
    def test_one_failing_section_does_not_kill_siblings(
        self,
        mock_short: Mock,
        mock_new: Mock,
        mock_rated: Mock,
        mock_platforms: Mock,
        mock_genre_games: Mock,
        mock_totals: Mock,
    ) -> None:
        mock_short.return_value = ([_discover_game("Short")], ["Action"])
        mock_new.side_effect = Exception("boom")
        mock_rated.return_value = [_discover_game("Rated", "3")]
        mock_platforms.return_value = []
        mock_genre_games.return_value = [_discover_game("Genre Game", "6")]
        mock_totals.side_effect = Exception("totals down")

        worker = self._worker()
        results: list[dict] = []
        worker.finished.connect(results.append)
        worker.run()

        self.assertEqual(len(results), 1)
        self.assertIn("highly_rated", results[0])
        self.assertIn("genres", results[0])
        self.assertNotIn("new_games", results[0])
        self.assertNotIn("genre_totals", results[0])

    @patch("grid_launcher.server.discover.fetch_server_platforms")
    @patch("grid_launcher.server.discover.fetch_highly_rated_games")
    @patch("grid_launcher.server.discover.fetch_new_games")
    @patch("grid_launcher.server.discover.fetch_short_games")
    def test_emits_error_when_everything_fails(
        self,
        mock_short: Mock,
        mock_new: Mock,
        mock_rated: Mock,
        mock_platforms: Mock,
    ) -> None:
        mock_short.side_effect = Exception("server down")
        mock_new.return_value = []
        mock_rated.return_value = []
        mock_platforms.return_value = []

        worker = self._worker()
        results: list[dict] = []
        errors: list[str] = []
        worker.finished.connect(results.append)
        worker.error.connect(errors.append)
        worker.run()

        self.assertEqual(results, [])
        self.assertEqual(errors, ["server down"])


class TestFilterServerGamesGenre(unittest.TestCase):
    """Test genre matching added to filter_server_games."""

    def _games(self) -> list[dict]:
        return [
            {"title": "A", "platform": "SNES", "genres": "Action, Adventure"},
            {"title": "B", "platform": "PS1", "genres": "Puzzle"},
            {"title": "C", "platform": "PS1"},
        ]

    def test_genre_substring_match(self) -> None:
        result = filter_server_games(self._games(), "Adven")
        self.assertEqual([g["title"] for g in result], ["A"])

    def test_genre_match_is_case_insensitive(self) -> None:
        result = filter_server_games(self._games(), "pUzZlE")
        self.assertEqual([g["title"] for g in result], ["B"])

    def test_non_matching_genre_returns_empty(self) -> None:
        self.assertEqual(filter_server_games(self._games(), "Racing"), [])

    def test_title_and_platform_matching_still_work(self) -> None:
        self.assertEqual(len(filter_server_games(self._games(), "PS1")), 2)
        self.assertEqual(len(filter_server_games(self._games(), "B")), 1)

    def test_genres_as_list_supported(self) -> None:
        games = [{"title": "A", "platform": "SNES", "genres": ["Action", "Shooter"]}]
        self.assertEqual(len(filter_server_games(games, "shooter")), 1)


def _load_main_module() -> Any:
    import importlib.util
    from pathlib import Path
    module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
    spec = importlib.util.spec_from_file_location("grid_launcher_main_for_discover_tests", module_path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class _CacheStub:
    """Minimal stand-in for MainWindow when exercising cache-only helpers."""

    def __init__(self, module: Any, cache: DiscoverCache) -> None:
        self.module = module
        self.discover_cache = cache

    def _discover_result_from_cache(self) -> dict:
        return self.module.MainWindow._discover_result_from_cache(self)


class TestDiscoverResultFromCache(unittest.TestCase):
    """Test the cache-only result assembly used for offline / cold-start renders."""

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])
        cls.module = _load_main_module()

    def _warm_cache(self) -> DiscoverCache:
        cache = DiscoverCache(ttl=3600)
        cache.set_section("short_games", {"games": [_discover_game("Short", "1")], "genres": ["Action"]})
        cache.set_section("new_games", {"games": [_discover_game("New", "2")]})
        cache.set_section("highly_rated", {"games": [_discover_game("Rated", "3")]})
        cache.set_section("recommendations", {"games": [_discover_game("Rec", "4")]})
        cache.set_section("platforms_list", {"platforms": [{"id": 9, "display_name": "N64", "games": []}]})
        cache.set_section(
            "genres",
            {"genres": ["Action"], "games_by_genre": {"Action": [_discover_game("Genre", "5")]}},
        )
        cache.set_section("genre_totals", {"totals": {"Action": 120}})
        return cache

    def test_returns_expected_result_shape(self) -> None:
        window = _CacheStub(self.module, self._warm_cache())
        result = window._discover_result_from_cache()
        self.assertEqual(result["short_games"]["games"][0]["title"], "Short")
        self.assertEqual(result["new_games"]["games"][0]["title"], "New")
        self.assertEqual(result["highly_rated"]["games"][0]["title"], "Rated")
        self.assertEqual(result["recommendations"]["games"][0]["title"], "Rec")
        self.assertEqual(result["platforms"][0]["display_name"], "N64")
        self.assertEqual(result["genres"]["genres"], ["Action"])
        self.assertEqual(result["genre_totals"], {"Action": 120})

    def test_empty_cache_returns_empty_result(self) -> None:
        window = _CacheStub(self.module, DiscoverCache(ttl=3600))
        self.assertEqual(window._discover_result_from_cache(), {})

    def test_stale_entries_still_returned(self) -> None:
        cache = self._warm_cache()
        cache.ttl = 0
        window = _CacheStub(self.module, cache)
        self.assertIn("short_games", window._discover_result_from_cache())

    def test_empty_sections_omitted(self) -> None:
        cache = DiscoverCache(ttl=3600)
        cache.set_section("short_games", {"games": []})
        cache.set_section("new_games", {"games": [_discover_game("New", "2")]})
        result = _CacheStub(self.module, cache)._discover_result_from_cache()
        self.assertNotIn("short_games", result)
        self.assertIn("new_games", result)


class _WatchlistStub:
    """Minimal stand-in for MainWindow when exercising watchlist helpers."""

    def __init__(self, module: Any, path: str, entries: dict) -> None:
        self.module = module
        self.path = path
        self.watchlist_games = entries
        self.discover_page = None

    def _watchlist_file(self):
        from pathlib import Path
        return Path(self.path)

    def _watchlist_section_games(self) -> list[dict]:
        return self.module.MainWindow._watchlist_section_games(self)

    def _hydrate_watchlist_entries(self, games: list[dict]) -> None:
        self.module.MainWindow._hydrate_watchlist_entries(self, games)

    def toggle_watchlist(self, game: dict) -> None:
        self.module.MainWindow.toggle_watchlist(self, game)

    def is_watchlisted(self, rom_id: str) -> bool:
        return self.module.MainWindow.is_watchlisted(self, rom_id)


class TestWindowWatchlistStore(unittest.TestCase):
    """Test the window-side watchlist store (full game dicts keyed by rom_id)."""

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])
        cls.module = _load_main_module()

    def setUp(self) -> None:
        import tempfile
        fd, self.path = tempfile.mkstemp(suffix=".json")
        os.close(fd)
        os.unlink(self.path)

    def tearDown(self) -> None:
        if os.path.exists(self.path):
            os.unlink(self.path)

    def test_toggle_stores_full_game_dict(self) -> None:
        window = _WatchlistStub(self.module, self.path, {})
        window.toggle_watchlist(_discover_game("Saved", "42", "Action", "SNES"))
        self.assertEqual(window.watchlist_games["42"]["title"], "Saved")
        self.assertTrue(window.is_watchlisted("42"))
        self.assertEqual(load_watchlist(self.path)["42"]["platform"], "SNES")

    def test_toggle_twice_removes_entry(self) -> None:
        window = _WatchlistStub(self.module, self.path, {})
        game = _discover_game("Saved", "42")
        window.toggle_watchlist(game)
        window.toggle_watchlist(game)
        self.assertEqual(window.watchlist_games, {})
        self.assertEqual(load_watchlist(self.path), {})

    def test_toggle_without_rom_id_ignored(self) -> None:
        window = _WatchlistStub(self.module, self.path, {})
        window.toggle_watchlist({"title": "No Id"})
        self.assertEqual(window.watchlist_games, {})

    def test_section_games_skip_id_only_entries(self) -> None:
        window = _WatchlistStub(
            self.module, self.path, {"1": {}, "2": _discover_game("Has Data", "2")}
        )
        titles = [g["title"] for g in window._watchlist_section_games()]
        self.assertEqual(titles, ["Has Data"])

    def test_hydrate_fills_id_only_entry_and_saves(self) -> None:
        window = _WatchlistStub(self.module, self.path, {"7": {}})
        window._hydrate_watchlist_entries([_discover_game("Resurfaced", "7"), _discover_game("Other", "8")])
        self.assertEqual(window.watchlist_games["7"]["title"], "Resurfaced")
        self.assertNotIn("8", window.watchlist_games)
        self.assertEqual(load_watchlist(self.path)["7"]["title"], "Resurfaced")

    def test_hydrate_does_not_overwrite_existing_card_data(self) -> None:
        window = _WatchlistStub(self.module, self.path, {"7": _discover_game("Original", "7")})
        window._hydrate_watchlist_entries([_discover_game("Newer", "7")])
        self.assertEqual(window.watchlist_games["7"]["title"], "Original")

    def test_hydrate_without_matches_writes_nothing(self) -> None:
        window = _WatchlistStub(self.module, self.path, {"7": {}})
        window._hydrate_watchlist_entries([_discover_game("Unrelated", "9")])
        self.assertFalse(os.path.exists(self.path))


class TestDiscoverOfflineNotice(unittest.TestCase):

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def _page(self):
        from grid_launcher.ui.discover import DiscoverPageWidget
        page = DiscoverPageWidget(_MockWindow())
        page.show()
        return page

    def test_notice_hidden_by_default(self) -> None:
        self.assertFalse(self._page().offline_label.isVisible())

    def test_notice_with_timestamp_mentions_cache_age(self) -> None:
        import time
        page = self._page()
        page.set_offline_notice(time.time() - 2 * 86400)
        self.assertTrue(page.offline_label.isVisible())
        self.assertIn("cached results from 2 days ago", page.offline_label.text())

    def test_notice_without_timestamp_says_no_cached_data(self) -> None:
        page = self._page()
        page.set_offline_notice(None)
        self.assertTrue(page.offline_label.isVisible())
        self.assertIn("no cached data", page.offline_label.text())

    def test_clear_hides_notice(self) -> None:
        page = self._page()
        page.set_offline_notice(None)
        page.clear_offline_notice()
        self.assertFalse(page.offline_label.isVisible())


class TestLastRefreshLabelTick(unittest.TestCase):
    """Test the minute timer slot that re-renders the "Updated ..." label."""

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def _page(self):
        from grid_launcher.ui.discover import DiscoverPageWidget
        page = DiscoverPageWidget(_MockWindow())
        page.show()
        return page

    def test_tick_updates_text_for_aged_timestamp(self) -> None:
        import time
        page = self._page()
        page.update_last_refresh_time(time.time())
        self.assertEqual(page.last_refresh_label.text(), "Updated just now")
        page._last_refresh_time = time.time() - 605
        page._tick_last_refresh_label()
        self.assertEqual(page.last_refresh_label.text(), "Updated 10 minutes ago")

    def test_tick_without_refresh_time_leaves_label_hidden(self) -> None:
        page = self._page()
        page._tick_last_refresh_label()
        self.assertFalse(page.last_refresh_label.isVisible())


class TestGenreSectionSeeAll(unittest.TestCase):

    @classmethod
    def setUpClass(cls) -> None:
        from PySide6.QtWidgets import QApplication
        cls.app = QApplication.instance() or QApplication([])

    def _section(self, callback):
        from grid_launcher.ui.discover import DiscoverGenreSection
        games_by_genre = {
            "Action": [_discover_game("A", "1")],
            "RPG": [_discover_game("B", "2")],
        }
        return DiscoverGenreSection(
            "genres", ["Action", "RPG"], games_by_genre, _MockWindow(), callback
        )

    def test_see_all_calls_callback_with_selected_genre(self) -> None:
        calls: list[str] = []
        section = self._section(calls.append)
        section.carousel_section.see_all_callback()
        self.assertEqual(calls, ["Action"])

    def test_see_all_follows_pill_selection(self) -> None:
        calls: list[str] = []
        section = self._section(calls.append)
        section.genre_buttons["RPG"].click()
        section.carousel_section.see_all_callback()
        self.assertEqual(calls, ["RPG"])

    def test_no_callback_leaves_carousel_without_see_all(self) -> None:
        section = self._section(None)
        self.assertIsNone(section.carousel_section.see_all_callback)

    def test_page_add_genre_section_threads_callback(self) -> None:
        from grid_launcher.ui.discover import DiscoverPageWidget
        calls: list[str] = []
        page = DiscoverPageWidget(_MockWindow())
        page.add_genre_section(
            ["Action"], {"Action": [_discover_game("A", "1")]}, calls.append
        )
        page.sections["genres"].carousel_section.see_all_callback()
        self.assertEqual(calls, ["Action"])


if __name__ == "__main__":
    unittest.main()

