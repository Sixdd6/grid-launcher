from __future__ import annotations

import unittest

from rom_mate.server.catalog import games_from_rom_items
from rom_mate.server.details_cache import rom_file_name_from_payload
from rom_mate.server.metadata import normalize_rating_to_five


class ServerCatalogFileNameTests(unittest.TestCase):
    def test_rom_file_name_from_payload_combines_fs_name_and_fs_extension(self) -> None:
        payload = {
            "fs_name": "Katamari Forever (USA)",
            "fs_extension": "7z",
        }

        self.assertEqual(
            rom_file_name_from_payload(payload),
            "Katamari Forever (USA).7z",
        )

    def test_rom_file_name_from_payload_uses_full_fs_name_when_extension_included(self) -> None:
        payload = {
            "fs_name": "Game.iso",
        }

        self.assertEqual(rom_file_name_from_payload(payload), "Game.iso")

    def test_rom_file_name_from_payload_uses_files_first_entry_for_folder_backed_rom(self) -> None:
        payload = {
            "fs_name": "BLUS30336",
            "fs_extension": "",
            "files": [
                {
                    "file_name": "BLUS30336 (USA).7z",
                }
            ],
        }

        self.assertEqual(
            rom_file_name_from_payload(payload),
            "BLUS30336 (USA).7z",
        )

    def test_rom_file_name_from_payload_folder_backed_rom_falls_back_to_fs_name_when_files_empty(self) -> None:
        payload = {
            "fs_name": "BLUS30336",
            "fs_extension": "",
            "files": [],
        }

        self.assertEqual(rom_file_name_from_payload(payload), "BLUS30336")

    def test_catalog_rom_file_name_combines_name_and_ext(self) -> None:
        games, _ = games_from_rom_items(
            [
                {
                    "id": 101,
                    "name": "Katamari Forever",
                    "fs_name": "Katamari Forever (USA)",
                    "fs_extension": "7z",
                    "platform_display_name": "PS3",
                }
            ],
            "PS3",
            cover_url_from_payload=lambda payload: "",
            screenshot_urls_from_payload=lambda payload: [],
        )

        self.assertEqual(len(games), 1)
        self.assertEqual(games[0]["rom_file_name"], "Katamari Forever (USA).7z")

    def test_catalog_rom_file_name_uses_full_name_when_ext_present(self) -> None:
        games, _ = games_from_rom_items(
            [
                {
                    "id": 102,
                    "name": "Game",
                    "fs_name": "Game.iso",
                    "platform_display_name": "PS2",
                }
            ],
            "PS2",
            cover_url_from_payload=lambda payload: "",
            screenshot_urls_from_payload=lambda payload: [],
        )

        self.assertEqual(len(games), 1)
        self.assertEqual(games[0]["rom_file_name"], "Game.iso")

    def test_catalog_folder_rom_sets_nested_file_name_without_overwriting_rom_file_name(self) -> None:
        games, _ = games_from_rom_items(
            [
                {
                    "id": "1898",
                    "fs_name": "Katamari Forever",
                    "fs_extension": "",
                    "files": [
                        {
                            "id": 2588,
                            "file_name": "Katamari Forever (USA).7z",
                            "file_path": "roms/ps3/Katamari Forever",
                        }
                    ],
                    "platforms": [{"name": "PlayStation 3", "slug": "ps3"}],
                    "platform_display_name": "PlayStation 3",
                    "name": "Katamari Forever",
                }
            ],
            "PlayStation 3",
            cover_url_from_payload=lambda payload: "",
            screenshot_urls_from_payload=lambda payload: [],
        )

        self.assertEqual(len(games), 1)
        self.assertEqual(games[0]["rom_file_name"], "Katamari Forever")
        self.assertEqual(games[0]["rom_nested_file_name"], "Katamari Forever (USA).7z")

    def test_catalog_multifile_folder_rom_sets_base_file_id(self) -> None:
        games, _ = games_from_rom_items(
            [
                {
                    "id": "1900",
                    "name": "DiRT 4",
                    "fs_name": "DiRT 4",
                    "fs_extension": "",
                    "platform_display_name": "PS4",
                    "files": [
                        {"id": 2590, "file_name": "DiRT 4.7z", "category": None},
                        {"id": 2591, "file_name": "DiRT 4 Update.pkg", "category": "update"},
                        {"id": 2592, "file_name": "DiRT 4 DLC 1.pkg", "category": "dlc"},
                        {"id": 2593, "file_name": "DiRT 4 DLC 2.pkg", "category": "dlc"},
                    ],
                }
            ],
            "PS4",
            cover_url_from_payload=lambda payload: "",
            screenshot_urls_from_payload=lambda payload: [],
        )

        self.assertEqual(len(games), 1)
        self.assertEqual(games[0]["rom_base_file_id"], "2590")

    def test_catalog_single_file_folder_rom_leaves_base_file_id_empty(self) -> None:
        games, _ = games_from_rom_items(
            [
                {
                    "id": "1901",
                    "name": "Single File Folder ROM",
                    "fs_name": "Single File Folder ROM",
                    "fs_extension": "",
                    "platform_display_name": "PS4",
                    "files": [
                        {"id": 3001, "file_name": "Single File Folder ROM.7z", "category": None}
                    ],
                }
            ],
            "PS4",
            cover_url_from_payload=lambda payload: "",
            screenshot_urls_from_payload=lambda payload: [],
        )

        self.assertEqual(len(games), 1)
        self.assertEqual(games[0]["rom_base_file_id"], "")

    def test_catalog_metadata_prefers_launchbox_then_falls_back_to_other_sources(self) -> None:
        games, _ = games_from_rom_items(
            [
                {
                    "id": 200,
                    "name": "Priority Test",
                    "platform_display_name": "PS2",
                    "summary": "Summary fallback",
                    "launchbox_metadata": {
                        "description": "LaunchBox description",
                        "genres": ["Action"],
                        "rating": "90/100",
                    },
                    "ss_metadata": {
                        "description": "ScreenScraper description",
                        "genres": ["Platformer"],
                        "regions": ["USA"],
                        "rating": "7/10",
                    },
                    "igdb_metadata": {
                        "genres": ["Adventure"],
                        "regions": ["Europe"],
                    },
                    "moby_metadata": {
                        "genres": ["Puzzle"],
                        "regions": ["Japan"],
                    },
                    "fs_size_bytes": 1536,
                }
            ],
            "PS2",
            cover_url_from_payload=lambda payload: "",
            screenshot_urls_from_payload=lambda payload: [],
        )

        self.assertEqual(len(games), 1)
        game = games[0]
        self.assertEqual(game["description"], "LaunchBox description")
        self.assertEqual(game["rating"], "4.5/5")
        self.assertEqual(game["genres"], "Action, Platformer, Adventure, Puzzle")
        self.assertEqual(game["regions"], "USA, Europe, Japan")
        self.assertEqual(game["filesize_bytes"], "1536")

    def test_catalog_metadata_falls_back_when_higher_priority_missing(self) -> None:
        games, _ = games_from_rom_items(
            [
                {
                    "id": 201,
                    "name": "Fallback Test",
                    "platform_display_name": "PS1",
                    "summary": "Summary fallback",
                    "launchbox_metadata": {},
                    "ss_metadata": {
                        "description": "ScreenScraper description",
                        "rating": "80%",
                        "regions": ["USA"],
                    },
                    "igdb_metadata": {
                        "genres": ["RPG"],
                    },
                }
            ],
            "PS1",
            cover_url_from_payload=lambda payload: "",
            screenshot_urls_from_payload=lambda payload: [],
        )

        self.assertEqual(len(games), 1)
        game = games[0]
        self.assertEqual(game["description"], "ScreenScraper description")
        self.assertEqual(game["rating"], "4.0/5")
        self.assertEqual(game["genres"], "RPG")
        self.assertEqual(game["regions"], "USA")


class RatingNormalizationTests(unittest.TestCase):
    def test_normalize_rating_to_five_handles_common_scales(self) -> None:
        self.assertEqual(normalize_rating_to_five("9/10"), 4.5)
        self.assertEqual(normalize_rating_to_five("80%"), 4.0)
        self.assertEqual(normalize_rating_to_five("4.2/5"), 4.2)
        self.assertEqual(normalize_rating_to_five(88), 4.4)

    def test_normalize_rating_to_five_returns_none_for_invalid_values(self) -> None:
        self.assertIsNone(normalize_rating_to_five(""))
        self.assertIsNone(normalize_rating_to_five("not a rating"))
        self.assertIsNone(normalize_rating_to_five(-1))


if __name__ == "__main__":
    unittest.main()
