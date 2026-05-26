from __future__ import annotations

import unittest
from unittest.mock import patch

from rom_mate.server.pcgamingwiki import (
    PCGamingWikiError,
    fetch_page_id_by_title,
    fetch_windows_save_paths,
    parse_windows_save_paths,
)


class PCGamingWikiTests(unittest.TestCase):
    def test_parse_windows_save_paths_typical_wikitext(self) -> None:
        wikitext = "{{Game data/saves|Windows|{{p|appdata}}\\MyGame\\saves}}"

        paths = parse_windows_save_paths(wikitext)

        self.assertIn("%APPDATA%\\MyGame\\saves", paths)

    def test_parse_windows_save_paths_no_windows_entry(self) -> None:
        wikitext = "{{Game data/saves|Linux|{{p|userprofile}}/.local/share/MyGame/saves}}"

        paths = parse_windows_save_paths(wikitext)

        self.assertEqual(paths, [])

    def test_parse_windows_save_paths_wildcard_stripped(self) -> None:
        wikitext = "{{Game data/saves|Windows|{{p|userprofile}}\\Documents\\Game\\*.sav}}"

        paths = parse_windows_save_paths(wikitext)

        self.assertEqual(paths, ["%USERPROFILE%\\Documents\\Game"])

    def test_parse_windows_save_paths_drm_path_excluded(self) -> None:
        wikitext = "{{Game data/saves|Windows|{{p|steam}}\\userdata\\saves}}"

        paths = parse_windows_save_paths(wikitext)

        self.assertEqual(paths, [])

    def test_parse_windows_save_paths_multiple_paths(self) -> None:
        wikitext = (
            "{{Game data/saves|Windows|"
            "{{p|appdata}}\\MyGame\\saves|"
            "{{p|localappdata}}\\MyGame\\saves|"
            "{{p|userprofile\\documents}}\\MyGame\\saves"
            "}}"
        )

        paths = parse_windows_save_paths(wikitext)

        self.assertIn("%APPDATA%\\MyGame\\saves", paths)
        self.assertIn("%LOCALAPPDATA%\\MyGame\\saves", paths)
        self.assertIn("%USERPROFILE%\\Documents\\MyGame\\saves", paths)

    @patch("rom_mate.server.pcgamingwiki._fetch_json")
    def test_fetch_page_id_by_title_exact_match(self, mock_fetch_json) -> None:
        mock_fetch_json.return_value = {
            "batchcomplete": "",
            "query": {
                "pages": {
                    "12345": {"pageid": 12345, "ns": 0, "title": "Stardew Valley"}
                }
            },
        }

        page_id = fetch_page_id_by_title("Stardew Valley")

        self.assertEqual(page_id, 12345)

    @patch("rom_mate.server.pcgamingwiki._fetch_json_value")
    @patch("rom_mate.server.pcgamingwiki._fetch_json")
    def test_fetch_page_id_by_title_opensearch_fallback(self, mock_fetch_json, mock_fetch_json_value) -> None:
        mock_fetch_json.side_effect = [
            {
                "batchcomplete": "",
                "query": {"pages": {"-1": {"ns": 0, "title": "Stardew Valley", "missing": ""}}},
            },
            {
                "batchcomplete": "",
                "query": {
                    "pages": {
                        "54321": {"pageid": 54321, "ns": 0, "title": "Stardew Valley"}
                    }
                },
            },
        ]
        mock_fetch_json_value.return_value = [
            "Stardew Valley",
            ["Stardew Valley"],
            [],
            ["https://www.pcgamingwiki.com/wiki/Stardew_Valley"],
        ]

        page_id = fetch_page_id_by_title("Stardew Valley")

        self.assertEqual(page_id, 54321)

    @patch("rom_mate.server.pcgamingwiki._fetch_json")
    def test_fetch_windows_save_paths_full_round_trip(self, mock_fetch_json) -> None:
        mock_fetch_json.side_effect = [
            {
                "batchcomplete": "",
                "query": {
                    "pages": {
                        "12345": {"pageid": 12345, "ns": 0, "title": "My Game"}
                    }
                },
            },
            {
                "parse": {
                    "wikitext": {
                        "*": "{{Game data/saves|Windows|{{p|appdata}}\\MyGame\\saves}}"
                    }
                }
            },
        ]

        paths = fetch_windows_save_paths("My Game")

        self.assertEqual(paths, ["%APPDATA%\\MyGame\\saves"])

    def test_parse_windows_save_paths_note_annotation_stripped(self) -> None:
        wikitext = (
            "{{Game data/saves|Windows|{{p|userprofile\\Documents}}\\Square Enix\\Batman Arkham Asylum GOTY\\SaveData\\{{note|name=Game of the Year Edition}}}}"
        )

        paths = parse_windows_save_paths(wikitext)

        self.assertIn("%USERPROFILE%\\Documents\\Square Enix\\Batman Arkham Asylum GOTY\\SaveData", paths)

    def test_parse_windows_save_paths_batman_arkham_asylum(self) -> None:
        wikitext = (
            "{{Game data/saves|Windows|{{p|userprofile\\Documents}}\\Eidos\\Batman Arkham Asylum\\SaveData\\|{{p|userprofile\\Documents}}\\Square Enix\\Batman Arkham Asylum GOTY\\SaveData\\{{note|name=Game of the Year Edition}}}}"
        )

        paths = parse_windows_save_paths(wikitext)

        self.assertIn("%USERPROFILE%\\Documents\\Eidos\\Batman Arkham Asylum\\SaveData", paths)
        self.assertIn("%USERPROFILE%\\Documents\\Square Enix\\Batman Arkham Asylum GOTY\\SaveData", paths)

    @patch("rom_mate.server.pcgamingwiki._fetch_json")
    def test_fetch_windows_save_paths_http_error_raises(self, mock_fetch_json) -> None:
        mock_fetch_json.side_effect = PCGamingWikiError("HTTP failure")

        with self.assertRaises(PCGamingWikiError):
            fetch_windows_save_paths("Game")


if __name__ == "__main__":
    unittest.main()
