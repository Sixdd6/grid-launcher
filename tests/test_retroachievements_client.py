from __future__ import annotations

import json
import unittest
from io import BytesIO
from unittest.mock import patch
from urllib.error import HTTPError

from grid_launcher.server.retroachievements import RetroAchievementsError, fetch_game_achievements, ra_login, resolve_ra_game_id


class _ResponseStub:
    def __init__(self, payload: bytes) -> None:
        self._payload = payload

    def read(self) -> bytes:
        return self._payload

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        return False


class RetroAchievementsClientTests(unittest.TestCase):
    @patch("grid_launcher.server.retroachievements.urlopen")
    def test_ra_login_success(self, mock_urlopen) -> None:
        mock_urlopen.return_value = _ResponseStub(
            json.dumps({"Success": True, "User": "testuser", "Token": "abc123"}).encode("utf-8")
        )

        result = ra_login("testuser", "password123")

        self.assertEqual(result, {"username": "testuser", "token": "abc123"})

    @patch("grid_launcher.server.retroachievements.urlopen")
    def test_ra_login_failure(self, mock_urlopen) -> None:
        mock_urlopen.return_value = _ResponseStub(
            json.dumps({"Success": False, "Error": "Invalid credentials"}).encode("utf-8")
        )

        with self.assertRaises(RetroAchievementsError):
            ra_login("bad", "wrong")

    def test_ra_login_empty_username(self) -> None:
        with self.assertRaises(ValueError):
            ra_login("", "password")

    def test_ra_login_empty_password(self) -> None:
        with self.assertRaises(ValueError):
            ra_login("user", "")

    @patch("grid_launcher.server.retroachievements.urlopen")
    def test_fetch_game_achievements_with_user_credentials(self, mock_urlopen) -> None:
        payload = {
            "Achievements": {
                "123": {
                    "ID": 123,
                    "Title": "First Steps",
                    "Description": "Complete the tutorial",
                    "Points": 5,
                    "BadgeName": "12345",
                    "DateEarned": "2026-04-10 12:00:00",
                }
            }
        }
        mock_urlopen.return_value = _ResponseStub(json.dumps(payload).encode("utf-8"))

        achievements = fetch_game_achievements(1000, "sam", "secret-key")

        self.assertEqual(
            achievements,
            [
                {
                    "id": 123,
                    "title": "First Steps",
                    "description": "Complete the tutorial",
                    "points": 5,
                    "badge_name": "12345",
                    "date_earned": "2026-04-10 12:00:00",
                }
            ],
        )
        called_url = mock_urlopen.call_args[0][0].full_url
        self.assertIn("API_GetGameInfoAndUserProgress.php", called_url)
        self.assertIn("u=sam", called_url)
        self.assertNotIn("z=", called_url)

    @patch("grid_launcher.server.retroachievements.urlopen")
    def test_fetch_game_achievements_public_fallback(self, mock_urlopen) -> None:
        mock_urlopen.return_value = _ResponseStub(json.dumps({"Achievements": {}}).encode("utf-8"))

        achievements = fetch_game_achievements(1000, "", "")

        called_url = mock_urlopen.call_args[0][0].full_url
        self.assertIn("API_GetGameExtended.php", called_url)
        self.assertNotIn("API_GetGameInfoAndUserProgress.php", called_url)
        self.assertNotIn("u=", called_url)
        self.assertEqual(achievements, [])

    @patch("grid_launcher.server.retroachievements.urlopen")
    def test_fetch_game_achievements_http_error(self, mock_urlopen) -> None:
        mock_urlopen.side_effect = HTTPError(
            "https://retroachievements.org/API/API_GetGameInfoAndUserProgress.php",
            500,
            "Server Error",
            None,
            BytesIO(b"error"),
        )

        with self.assertRaises(RetroAchievementsError):
            fetch_game_achievements(1000, "sam", "secret-key")

    def test_fetch_game_achievements_invalid_ra_id(self) -> None:
        with self.assertRaises(ValueError):
            fetch_game_achievements(0, "sam", "secret-key")

    def test_resolve_ra_game_id_from_game_dict(self) -> None:
        game_id = resolve_ra_game_id({"ra_id": "12345"}, "", "")
        self.assertEqual(game_id, 12345)

    def test_resolve_ra_game_id_missing(self) -> None:
        self.assertIsNone(resolve_ra_game_id({"ra_id": ""}, "", ""))
        self.assertIsNone(resolve_ra_game_id({"ra_id": None}, "", ""))


if __name__ == "__main__":
    unittest.main()
