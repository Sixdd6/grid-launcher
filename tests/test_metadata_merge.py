from __future__ import annotations

import unittest

from rom_mate.server.metadata import (
    details_metadata_from_item,
    format_rating_to_five,
    normalize_rating_to_five,
)


class MetadataTests(unittest.TestCase):
    def test_normalize_rating_to_five_handles_numeric_scales(self):
        self.assertEqual(normalize_rating_to_five(3), 3.0)
        self.assertEqual(normalize_rating_to_five(8), 4.0)
        self.assertEqual(normalize_rating_to_five(80), 4.0)

    def test_normalize_rating_to_five_handles_string_slash_and_percent_and_bare_numbers(self):
        self.assertEqual(normalize_rating_to_five("8/10"), 4.0)
        self.assertEqual(normalize_rating_to_five("80%"), 4.0)
        self.assertEqual(normalize_rating_to_five("3.5/5"), 3.5)
        self.assertEqual(normalize_rating_to_five("4.2"), 4.2)
        self.assertEqual(normalize_rating_to_five("7.5"), 3.8)

    def test_normalize_rating_to_five_handles_none_empty_negative_and_clamp(self):
        self.assertIsNone(normalize_rating_to_five(None))
        self.assertIsNone(normalize_rating_to_five(""))
        self.assertIsNone(normalize_rating_to_five(-1))
        self.assertEqual(normalize_rating_to_five(120), 5.0)

    def test_format_rating_to_five_formats_valid_values(self):
        self.assertEqual(format_rating_to_five("8/10"), "4.0/5")

    def test_format_rating_to_five_returns_empty_for_none_or_empty(self):
        self.assertEqual(format_rating_to_five(None), "")
        self.assertEqual(format_rating_to_five(""), "")

    def test_details_metadata_prefers_launchbox_description(self):
        item = {
            "summary": "Top-level summary",
            "launchbox_metadata": {"description": "LaunchBox description"},
            "ss_metadata": {"description": "ScreenScraper description"},
        }

        merged = details_metadata_from_item(item)

        self.assertEqual(merged["description"], "LaunchBox description")
        self.assertEqual(merged["description_source"], "launchbox_metadata")

    def test_details_metadata_falls_back_to_screenscraper_description(self):
        item = {
            "launchbox_metadata": {},
            "ss_metadata": {"overview": "ScreenScraper description"},
            "igdb_metadata": {"summary": "IGDB description"},
        }

        merged = details_metadata_from_item(item)

        self.assertEqual(merged["description"], "ScreenScraper description")
        self.assertEqual(merged["description_source"], "ss_metadata")

    def test_details_metadata_falls_back_to_igdb_description(self):
        item = {
            "launchbox_metadata": {},
            "ss_metadata": {},
            "igdb_metadata": {"plot": "IGDB description"},
            "moby_metadata": {"synopsis": "Moby description"},
        }

        merged = details_metadata_from_item(item)

        self.assertEqual(merged["description"], "IGDB description")
        self.assertEqual(merged["description_source"], "igdb_metadata")

    def test_details_metadata_uses_moby_description_as_lowest_priority_source(self):
        item = {
            "launchbox_metadata": {},
            "ss_metadata": {},
            "igdb_metadata": {},
            "moby_metadata": {"description": "Moby description"},
        }

        merged = details_metadata_from_item(item)

        self.assertEqual(merged["description"], "Moby description")
        self.assertEqual(merged["description_source"], "moby_metadata")

    def test_details_metadata_genres_prioritize_launchbox_and_merge_supplemental_sources(self):
        item = {
            "launchbox_metadata": {"genres": ["Action", "Adventure"]},
            "ss_metadata": {"genres": ["Action", "Platformer"]},
            "igdb_metadata": {"genres": ["RPG"]},
            "moby_metadata": {"genre": "Puzzle"},
        }

        merged = details_metadata_from_item(item)

        self.assertEqual(merged["genres"], "Action, Adventure, Platformer, RPG, Puzzle")

    def test_details_metadata_rating_prefers_launchbox_then_falls_back(self):
        launchbox_first = {
            "launchbox_metadata": {"rating": "90/100"},
            "ss_metadata": {"rating": "8/10"},
        }
        fallback = {
            "launchbox_metadata": {},
            "ss_metadata": {"score": "80%"},
        }

        merged_launchbox = details_metadata_from_item(launchbox_first)
        merged_fallback = details_metadata_from_item(fallback)

        self.assertEqual(merged_launchbox["rating"], "4.5/5")
        self.assertEqual(merged_launchbox["rating_source"], "launchbox_metadata")
        self.assertEqual(merged_fallback["rating"], "4.0/5")
        self.assertEqual(merged_fallback["rating_source"], "ss_metadata")

    def test_details_metadata_formats_filesize_bytes_from_positive_int(self):
        item = {"fs_size_bytes": 123456}

        merged = details_metadata_from_item(item)

        self.assertEqual(merged["filesize_bytes"], "123456")

    def test_details_metadata_uses_empty_filesize_for_absent_or_zero(self):
        merged_absent = details_metadata_from_item({})
        merged_zero = details_metadata_from_item({"fs_size_bytes": 0})

        self.assertEqual(merged_absent["filesize_bytes"], "")
        self.assertEqual(merged_zero["filesize_bytes"], "")

    def test_details_metadata_extracts_and_joins_regions(self):
        item = {
            "ss_metadata": {"regions": [{"name": "USA"}]},
            "igdb_metadata": {"countries": ["Europe"]},
            "moby_metadata": {"country": "Japan"},
        }

        merged = details_metadata_from_item(item)

        self.assertEqual(merged["regions"], "USA, Europe, Japan")

    def test_details_metadata_falls_back_to_top_level_fields_when_sources_missing(self):
        item = {
            "summary": "Top-level summary",
            "genres": ["Action", {"name": "Racing"}],
            "region": "USA",
            "rating": "7.5/10",
        }

        merged = details_metadata_from_item(item)

        self.assertEqual(merged["description"], "Top-level summary")
        self.assertEqual(merged["description_source"], "summary")
        self.assertEqual(merged["genres"], "")
        self.assertEqual(merged["regions"], "USA")
        self.assertEqual(merged["rating"], "3.8/5")
        self.assertEqual(merged["rating_source"], "rom")

    def test_details_metadata_uses_top_level_metadatum_genres_when_sources_have_none(self):
        item = {
            "launchbox_metadata": {},
            "ss_metadata": {},
            "igdb_metadata": {},
            "moby_metadata": {},
            "metadatum": {"genres": ["Action", "RPG"]},
        }

        merged = details_metadata_from_item(item)

        self.assertEqual(merged["genres"], "Action, RPG")

    def test_details_metadata_empty_item_returns_empty_fields(self):
        merged = details_metadata_from_item({})

        self.assertEqual(
            merged,
            {
                "description": "",
                "description_source": "",
                "genres": "",
                "regions": "",
                "rating": "",
                "rating_source": "",
                "release_year": "",
                "filesize_bytes": "",
                "revision": "",
                "languages": "",
                "tags": "",
                "fanart_url": "",
                "companies": "",
                "first_release_date": "",
            },
        )

    def test_revision_extracted(self):
        item = {"revision": "v1.1"}

        result = details_metadata_from_item(item)

        self.assertEqual(result["revision"], "v1.1")

    def test_languages_joined_from_list(self):
        item = {"languages": ["English", "French"]}

        result = details_metadata_from_item(item)

        self.assertEqual(result["languages"], "English, French")

    def test_languages_empty_when_not_list(self):
        item = {"languages": "English"}

        result = details_metadata_from_item(item)

        self.assertEqual(result["languages"], "")

    def test_tags_joined_from_string_dicts(self):
        item = {"tags": [{"name": "RPG"}, {"name": "Action"}]}

        result = details_metadata_from_item(item)

        self.assertEqual(result["tags"], "RPG, Action")

    def test_tags_joined_from_plain_strings(self):
        item = {"tags": ["RPG", "Action"]}

        result = details_metadata_from_item(item)

        self.assertEqual(result["tags"], "RPG, Action")

    def test_fanart_url_from_ss_metadata(self):
        item = {"ss_metadata": {"fanart_url": "https://example.com/fanart.jpg"}}

        result = details_metadata_from_item(item)

        self.assertEqual(result["fanart_url"], "https://example.com/fanart.jpg")

    def test_fanart_url_falls_back_to_gamelist_metadata(self):
        item = {
            "ss_metadata": {},
            "gamelist_metadata": {"fanart_url": "https://example.com/fanart2.jpg"},
        }

        result = details_metadata_from_item(item)

        self.assertEqual(result["fanart_url"], "https://example.com/fanart2.jpg")

    def test_companies_from_igdb_metadata(self):
        item = {"igdb_metadata": {"companies": ["Capcom", "Inafune"]}}

        result = details_metadata_from_item(item)

        self.assertEqual(result["companies"], "Capcom, Inafune")

    def test_first_release_date_from_epoch(self):
        item = {"igdb_metadata": {"first_release_date": 0}}

        result = details_metadata_from_item(item)

        self.assertEqual(result["first_release_date"], "1970-01-01")


if __name__ == "__main__":
    unittest.main()
