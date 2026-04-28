from __future__ import annotations

import unittest
from unittest.mock import patch

from rom_mate.background.workers import RomDetailWorker
from rom_mate.ui.mixins.details_view_mixin import DetailsViewMixin


class _StubAchievementsButton:
    def __init__(self) -> None:
        self.visible: bool | None = None

    def setVisible(self, value: bool) -> None:
        self.visible = value


class _StubDetailsWindow(DetailsViewMixin):
    def __init__(self) -> None:
        self.config = {"api_token": "token"}
        self._cloud_emulator_entry_cache: dict[str, object] = {}
        self._current_details_game: dict[str, str] | None = None
        self.details_achievements_button = _StubAchievementsButton()
        self.started_lookup_args: tuple[str, str, str] | None = None

    def _enrich_game_for_details(self, game: dict) -> None:
        return

    def _server_base_url(self) -> str:
        return "https://romm.example"

    def _start_rom_detail_lookup(self, rom_id: str, base_url: str, api_token: str) -> None:
        self.started_lookup_args = (rom_id, base_url, api_token)


class RomDetailWorkerTests(unittest.TestCase):
    def test_worker_emits_payload_on_success(self) -> None:
        worker = RomDetailWorker("https://romm.example", "token", "123")
        results: list[tuple[str, dict, str]] = []
        worker.finished.connect(lambda rom_id, payload, error: results.append((rom_id, payload, error)))

        payload = {"id": 123, "name": "Test Rom"}
        with patch("rom_mate.background.workers.api_get_json", return_value=payload) as mock_api_get:
            worker.run()

        mock_api_get.assert_called_once_with("https://romm.example", "token", "/api/roms/123")
        self.assertEqual(results, [("123", payload, "")])

    def test_worker_emits_error_on_exception(self) -> None:
        worker = RomDetailWorker("https://romm.example", "token", "321")
        results: list[tuple[str, dict, str]] = []
        worker.finished.connect(lambda rom_id, payload, error: results.append((rom_id, payload, error)))

        with patch("rom_mate.background.workers.api_get_json", side_effect=ValueError("boom")):
            worker.run()

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][0], "321")
        self.assertEqual(results[0][1], {})
        self.assertTrue(results[0][2])

    def test_open_game_details_starts_lookup_when_server_base_url_present(self) -> None:
        window = _StubDetailsWindow()
        game = {
            "rom_id": "123",
            "title": "Test",
            "genres": "",
            "regions": "",
            "filesize_bytes": "",
            "rating": "",
        }

        with patch("rom_mate.ui.mixins.details_view_mixin.open_game_details"), patch(
            "rom_mate.server.retroachievements.resolve_ra_game_id", return_value=None
        ):
            window._open_game_details(game, "library")

        self.assertEqual(window.started_lookup_args, ("123", "https://romm.example", "token"))

    def test_on_rom_detail_loaded_applies_new_fields(self) -> None:
        window = _StubDetailsWindow()
        window.current_details_game = {
            "rom_id": "123",
            "title": "Test",
        }
        window.current_details_source = "library"
        payload = {
            "revision": "v2.0",
            "languages": ["English", "French"],
            "tags": [{"name": "RPG"}, {"name": "Action"}],
            "igdb_metadata": {
                "companies": ["Capcom"],
                "first_release_date": 788918400,
            },
            "ss_metadata": {
                "fanart_url": "https://example.com/fanart.jpg",
            },
        }

        with patch("rom_mate.ui.mixins.details_view_mixin.open_game_details"):
            window._on_rom_detail_loaded("123", payload, "")

        game = window.current_details_game
        self.assertEqual(game.get("revision"), "v2.0")
        self.assertEqual(game.get("languages"), "English, French")
        self.assertEqual(game.get("tags"), "RPG, Action")
        self.assertEqual(game.get("companies"), "Capcom")
        self.assertEqual(game.get("fanart_url"), "https://example.com/fanart.jpg")
        self.assertTrue(game.get("first_release_date"))


if __name__ == "__main__":
    unittest.main()
