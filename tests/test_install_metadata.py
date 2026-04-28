from __future__ import annotations

import unittest

from rom_mate.library.install_metadata import hydrate_install_game_metadata, sync_install_metadata_to_details_game


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


if __name__ == "__main__":
    unittest.main()
