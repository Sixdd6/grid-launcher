from __future__ import annotations

import unittest

from rom_mate.server.catalog import games_from_rom_items
from rom_mate.server.details_cache import rom_file_name_from_payload


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


if __name__ == "__main__":
    unittest.main()
