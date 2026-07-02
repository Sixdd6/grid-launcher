from __future__ import annotations

import json
import unittest

from grid_launcher.server.catalog import games_from_rom_items


class ServerCatalogPs4MetadataTests(unittest.TestCase):
    def test_games_from_rom_items_sets_ps4_category_metadata_and_flags(self) -> None:
        rom_items = [
            {
                "id": 10,
                "name": "Example PS4 Game",
                "fs_name": "Example PS4 Game.pkg",
                "platform_display_name": "PlayStation 4",
                "summary": "An example title.",
                "files": [
                    {"id": 101, "category": "game"},
                    {"id": 102, "category": "update"},
                    {"id": 103, "category": "dlc"},
                    {"id": 104, "category": "DLC"},
                    {"id": 105, "category": None},
                    {"id": "invalid", "category": "update"},
                    "bad-entry",
                ],
            }
        ]

        games, rom_payloads = games_from_rom_items(
            rom_items,
            "PS4",
            cover_url_from_payload=lambda payload: "https://covers.example/game.png",
            screenshot_urls_from_payload=lambda payload: ["https://img.example/1.png", "https://img.example/2.png"],
        )

        self.assertEqual(len(games), 1)
        game = games[0]
        self.assertEqual(game["title"], "Example PS4 Game")
        self.assertEqual(game["platform"], "PlayStation 4")
        self.assertEqual(game["rom_id"], "10")
        self.assertEqual(game["ps4_has_update"], "true")
        self.assertEqual(game["ps4_has_dlc"], "true")

        file_ids_by_category = json.loads(game["ps4_file_ids_by_category"])
        self.assertEqual(
            file_ids_by_category,
            {
                "dlc": [103, 104],
                "game": [101, 105],
                "update": [102],
            },
        )
        self.assertEqual(rom_payloads["10"]["name"], "Example PS4 Game")

    def test_games_from_rom_items_keeps_non_ps4_with_safe_defaults(self) -> None:
        rom_items = [
            {
                "id": 20,
                "name": "Non-PS4 Game",
                "platform_display_name": "Nintendo Switch",
                "fs_name": "Non-PS4 Game.nsp",
                "files": [
                    {"id": 201, "category": "update"},
                    {"id": 202, "category": "dlc"},
                ],
            }
        ]

        games, _ = games_from_rom_items(
            rom_items,
            "Nintendo Switch",
            cover_url_from_payload=lambda payload: "",
            screenshot_urls_from_payload=lambda payload: [],
        )

        self.assertEqual(len(games), 1)
        game = games[0]
        self.assertEqual(game["platform"], "Nintendo Switch")
        self.assertEqual(game["ps4_has_update"], "false")
        self.assertEqual(game["ps4_has_dlc"], "false")
        self.assertEqual(game["ps4_file_ids_by_category"], "{}")


if __name__ == "__main__":
    unittest.main()
