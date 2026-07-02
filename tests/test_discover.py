"""Tests for Discover tab functionality."""

import unittest
from unittest.mock import Mock, patch

from rom_mate.server.discover import (
    DiscoverCache,
    fetch_all_games,
    filter_games_by_installed,
    get_top_genres_from_games,
    normalize_discover_item,
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

    @patch("rom_mate.server.discover.api_get_json")
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

    @patch("rom_mate.server.discover.api_get_json")
    def test_fetch_all_games_api_error_returns_empty(self, mock_api: Mock) -> None:
        mock_api.side_effect = Exception("connection refused")
        games, genres = fetch_all_games("http://test", "token")
        self.assertEqual(games, [])
        self.assertEqual(genres, [])

    @patch("rom_mate.server.discover.api_get_json")
    def test_fetch_all_games_non_dict_response(self, mock_api: Mock) -> None:
        mock_api.return_value = "unexpected string"
        games, genres = fetch_all_games("http://test", "token")
        self.assertEqual(games, [])
        self.assertEqual(genres, [])

    @patch("rom_mate.server.discover.api_get_json")
    def test_fetch_all_games_no_filter_values(self, mock_api: Mock) -> None:
        mock_api.return_value = {"items": [{"id": 1, "name": "Solo"}]}
        games, genres = fetch_all_games("http://test", "token")
        self.assertEqual(len(games), 1)
        self.assertEqual(genres, [])

    @patch("rom_mate.server.discover.api_get_json")
    def test_fetch_all_games_genre_dicts(self, mock_api: Mock) -> None:
        mock_api.return_value = {
            "items": [],
            "filter_values": {"genres": [{"name": "Puzzle"}, {"name": "Strategy"}]},
        }
        _, genres = fetch_all_games("http://test", "token")
        self.assertIn("Puzzle", genres)
        self.assertIn("Strategy", genres)


if __name__ == "__main__":
    unittest.main()

