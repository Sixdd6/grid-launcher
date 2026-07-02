from __future__ import annotations

import json
import unittest

from grid_launcher.library.install_metadata import (
    apply_windows_game_json_to_game,
    hydrate_install_game_metadata,
    parse_windows_game_json,
    sync_install_metadata_to_details_game,
)


class InstallMetadataTests(unittest.TestCase):
    @staticmethod
    def _game_key(game: dict[str, str]) -> tuple[str, str]:
        return (
            game.get("title", "").strip().casefold(),
            game.get("platform", "").strip().casefold(),
        )

    @staticmethod
    def _rom_id_key(game: dict[str, str]) -> str:
        return game.get("rom_id", "").strip().casefold()

    def test_hydrate_install_game_metadata_refreshes_stale_screenshot_urls_from_server_payload(self) -> None:
        game = {
            "title": "Demo Game",
            "platform": "PS2",
            "screenshot_urls": "\n".join(
                [
                    "https://img.example/old-box-front.jpg",
                    "https://img.example/old-fanart.jpg",
                ]
            ),
        }
        corrected_urls = [
            "https://img.example/new-screenshot-1.jpg",
            "https://img.example/new-screenshot-2.jpg",
        ]

        hydrate_install_game_metadata(
            game,
            "rom-1",
            server_games_by_platform={},
            server_rom_payloads={
                "rom-1": {
                    "launchbox_metadata": {
                        "images": [
                            {"type": "Screenshot - Gameplay", "url": corrected_urls[0]},
                            {"type": "Screenshot - Game Title", "url": corrected_urls[1]},
                            {"type": "Box - Front", "url": "https://img.example/new-box-front.jpg"},
                        ]
                    }
                }
            },
            game_key=self._game_key,
            rom_id_key=self._rom_id_key,
            fetch_server_rom_payload=lambda rom_id: None,
            resolved_cover_url_for_game=lambda value: "",
            cover_url_from_rom_payload=lambda payload: "",
            screenshot_urls_from_rom_payload=lambda payload: corrected_urls,
        )

        self.assertEqual(game.get("screenshot_urls", ""), "\n".join(corrected_urls))

    def test_sync_install_metadata_to_details_game_applies_refreshed_screenshot_urls(self) -> None:
        details_game = {
            "title": "Demo Game",
            "platform": "PS2",
            "screenshot_urls": "https://img.example/stale-box-front.jpg",
        }
        install_game = {
            "title": "Demo Game",
            "platform": "PS2",
            "screenshot_urls": "\n".join(
                [
                    "https://img.example/new-screenshot-1.jpg",
                    "https://img.example/new-screenshot-2.jpg",
                ]
            ),
        }

        sync_install_metadata_to_details_game(
            details_game,
            install_game,
            game_key=self._game_key,
        )

        self.assertEqual(
            details_game.get("screenshot_urls", ""),
            "\n".join(
                [
                    "https://img.example/new-screenshot-1.jpg",
                    "https://img.example/new-screenshot-2.jpg",
                ]
            ),
        )

    def test_sync_install_metadata_to_details_game_copies_new_details_fields(self) -> None:
        details_game = {
            "title": "Demo Game",
            "platform": "PS2",
        }
        install_game = {
            "title": "Demo Game",
            "platform": "PS2",
            "rating": "4.0/5",
            "description": "Updated description",
            "genres": "Action, Adventure",
            "regions": "USA",
            "filesize_bytes": "1024",
        }

        sync_install_metadata_to_details_game(
            details_game,
            install_game,
            game_key=self._game_key,
        )

        self.assertEqual(details_game.get("rating"), "4.0/5")
        self.assertEqual(details_game.get("description"), "Updated description")
        self.assertEqual(details_game.get("genres"), "Action, Adventure")
        self.assertEqual(details_game.get("regions"), "USA")
        self.assertEqual(details_game.get("filesize_bytes"), "1024")

    def test_hydrate_install_game_metadata_copies_release_year_from_server_game(self) -> None:
        game = {
            "title": "Demo Game",
            "platform": "PS2",
            "release_year": "",
        }

        hydrate_install_game_metadata(
            game,
            "rom-2",
            server_games_by_platform={
                "PS2": [
                    {
                        "title": "Demo Game",
                        "platform": "PS2",
                        "rom_id": "rom-2",
                        "release_year": "1999",
                    }
                ]
            },
            server_rom_payloads={},
            game_key=self._game_key,
            rom_id_key=self._rom_id_key,
            fetch_server_rom_payload=lambda rom_id: None,
            resolved_cover_url_for_game=lambda value: "",
            cover_url_from_rom_payload=lambda payload: "",
            screenshot_urls_from_rom_payload=lambda payload: [],
        )

        self.assertEqual(game.get("release_year"), "1999")

    def test_sync_install_metadata_to_details_game_copies_release_year(self) -> None:
        details_game = {
            "title": "Demo Game",
            "platform": "PS2",
            "release_year": "",
        }
        install_game = {
            "title": "Demo Game",
            "platform": "PS2",
            "release_year": "2001",
        }

        sync_install_metadata_to_details_game(
            details_game,
            install_game,
            game_key=self._game_key,
        )

        self.assertEqual(details_game.get("release_year"), "2001")

    def test_sync_propagates_new_metadata_fields(self) -> None:
        details_game = {
            "title": "Demo Game",
            "platform": "PS2",
        }
        install_game = {
            "title": "Demo Game",
            "platform": "PS2",
            "revision": "v1.2",
            "languages": "English, French",
            "tags": "RPG, Action",
            "companies": "Capcom, Inafune",
            "fanart_url": "https://example.com/fanart.jpg",
            "first_release_date": "1995-01-01",
        }

        sync_install_metadata_to_details_game(
            details_game,
            install_game,
            game_key=self._game_key,
        )

        self.assertEqual(details_game.get("revision"), "v1.2")
        self.assertEqual(details_game.get("languages"), "English, French")
        self.assertEqual(details_game.get("tags"), "RPG, Action")
        self.assertEqual(details_game.get("companies"), "Capcom, Inafune")
        self.assertEqual(details_game.get("fanart_url"), "https://example.com/fanart.jpg")
        self.assertEqual(details_game.get("first_release_date"), "1995-01-01")


class TestWindowsGameJson(unittest.TestCase):
    def test_parse_basic_game_json(self) -> None:
        data = json.dumps(
            {
                "name": "Test Game",
                "version": "1.5",
                "release_year": 2001,
                "tags": ["RPG", "Action"],
                "included_dlc": ["DLC1", "DLC2"],
            }
        ).encode()

        parsed = parse_windows_game_json(data)

        self.assertEqual(parsed.get("revision"), "1.5")
        self.assertEqual(parsed.get("first_release_date"), "2001")
        self.assertEqual(parsed.get("tags"), "RPG, Action")
        self.assertEqual(parsed.get("included_dlc"), json.dumps(["DLC1", "DLC2"]))
        self.assertEqual(parsed.get("name"), "Test Game")

    def test_parse_game_json_missing_fields(self) -> None:
        data = json.dumps({"name": "Test"}).encode()

        parsed = parse_windows_game_json(data)

        self.assertEqual(parsed.get("revision"), "")
        self.assertEqual(parsed.get("first_release_date"), "")
        self.assertEqual(parsed.get("tags"), "")
        self.assertEqual(parsed.get("included_dlc"), "[]")
        self.assertEqual(parsed.get("name"), "Test")

    def test_parse_game_json_invalid_json(self) -> None:
        parsed = parse_windows_game_json(b"this is not valid json {{{")

        self.assertEqual(parsed, {})

    def test_parse_game_json_not_a_dict(self) -> None:
        data = json.dumps([1, 2, 3]).encode()

        parsed = parse_windows_game_json(data)

        self.assertEqual(parsed, {})

    def test_parse_game_json_uses_year_field(self) -> None:
        data = json.dumps({"year": "2019"}).encode()

        parsed = parse_windows_game_json(data)

        self.assertEqual(parsed.get("first_release_date"), "2019")

    def test_parse_game_json_year_preferred_over_release_year(self) -> None:
        data = json.dumps({"year": "2025", "release_year": "2010"}).encode()

        parsed = parse_windows_game_json(data)

        self.assertEqual(parsed.get("first_release_date"), "2025")

    def test_apply_fills_empty_game_fields(self) -> None:
        game = {"revision": "", "tags": ""}
        parsed = {
            "revision": "1.5",
            "first_release_date": "2001",
            "tags": "RPG, Action",
            "included_dlc": json.dumps(["DLC1"]),
            "name": "Test Game",
        }

        apply_windows_game_json_to_game(game, parsed)

        self.assertEqual(game.get("revision"), "1.5")
        self.assertEqual(game.get("tags"), "RPG, Action")
        self.assertEqual(game.get("first_release_date"), "2001")

    def test_apply_preserves_existing_non_empty_fields(self) -> None:
        game = {"revision": "2.0"}
        parsed = {
            "revision": "1.5",
            "first_release_date": "",
            "tags": "",
            "included_dlc": "[]",
            "name": "Test Game",
        }

        apply_windows_game_json_to_game(game, parsed)

        self.assertEqual(game.get("revision"), "2.0")

    def test_apply_always_overwrites_included_dlc(self) -> None:
        game = {"included_dlc": json.dumps(["old"])}
        parsed = {
            "revision": "",
            "first_release_date": "",
            "tags": "",
            "included_dlc": json.dumps(["new"]),
            "name": "Test Game",
        }

        apply_windows_game_json_to_game(game, parsed)

        self.assertEqual(game.get("included_dlc"), json.dumps(["new"]))

    def test_apply_does_not_write_name(self) -> None:
        game: dict[str, str] = {}
        parsed = {
            "revision": "1.5",
            "first_release_date": "2001",
            "tags": "RPG",
            "included_dlc": "[]",
            "name": "Test Game",
        }

        apply_windows_game_json_to_game(game, parsed)

        self.assertNotIn("name", game)

    def test_apply_empty_parsed_does_nothing(self) -> None:
        game = {"revision": "1.0", "tags": "RPG", "included_dlc": json.dumps(["keep"])}

        apply_windows_game_json_to_game(game, {})

        self.assertEqual(game, {"revision": "1.0", "tags": "RPG", "included_dlc": json.dumps(["keep"])})


if __name__ == "__main__":
    unittest.main()
