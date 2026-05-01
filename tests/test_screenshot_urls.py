from __future__ import annotations

import unittest

from rom_mate.cover.utils import resolve_cover_url, screenshot_urls_from_game, screenshot_urls_from_rom_payload


class ScreenshotUrlsTests(unittest.TestCase):
    @staticmethod
    def _resolver(value: object) -> str:
        return value if isinstance(value, str) else ""

    def test_launchbox_typed_images_keep_only_screenshot_types(self) -> None:
        payload = {
            "launchbox_metadata": {
                "images": [
                    {"type": "Box - Front", "url": "https://img.example/box-front.jpg"},
                    {"type": "Fanart - Background", "url": "https://img.example/fanart.jpg"},
                    {"type": "Clear Logo", "url": "https://img.example/logo.png"},
                    {"type": "Screenshot - Gameplay", "url": "https://img.example/shot-gameplay.jpg"},
                    {"type": "Screenshot - Game Title", "url": "https://img.example/shot-title.jpg"},
                ]
            }
        }

        urls = screenshot_urls_from_rom_payload(payload, self._resolver)

        self.assertEqual(
            urls,
            [
                "https://img.example/shot-gameplay.jpg",
                "https://img.example/shot-title.jpg",
            ],
        )

    def test_metadata_blocks_exclude_non_screenshot_fields(self) -> None:
        payload = {
            "gamelist_metadata": {
                "screenshot_url": "https://img.example/gamelist-shot.jpg",
                "title_screen_url": "https://img.example/gamelist-title.jpg",
                "image_url": "https://img.example/gamelist-box-art.jpg",
            },
            "ss_metadata": {
                "screenshot_url": "https://img.example/ss-shot.jpg",
                "title_screen_url": "https://img.example/ss-title.jpg",
                "fanart_url": "https://img.example/ss-fanart.jpg",
            },
        }

        urls = screenshot_urls_from_rom_payload(payload, self._resolver)

        self.assertEqual(
            urls,
            [
                "https://img.example/gamelist-shot.jpg",
                "https://img.example/gamelist-title.jpg",
                "https://img.example/ss-shot.jpg",
                "https://img.example/ss-title.jpg",
            ],
        )

    def test_screenshot_sources_exclude_non_screenshot_images_block_entries(self) -> None:
        payload = {
            "merged_screenshots": [
                "https://img.example/merged-shot-1.jpg",
                "https://img.example/merged-shot-2.jpg",
            ],
            "url_screenshots": [
                "https://img.example/list-shot-1.jpg",
                "https://img.example/list-shot-2.jpg",
            ],
            "images": [
                {"type": "Screenshot - Gameplay", "url": "https://img.example/images-shot-1.jpg"},
                {"type": "Box - Front", "url": "https://img.example/box-front.jpg"},
                {"type": "Fanart - Background", "url": "https://img.example/fanart.jpg"},
            ],
        }

        urls = screenshot_urls_from_rom_payload(payload, self._resolver)

        self.assertEqual(
            urls,
            [
                "https://img.example/merged-shot-1.jpg",
                "https://img.example/merged-shot-2.jpg",
                "https://img.example/list-shot-1.jpg",
                "https://img.example/list-shot-2.jpg",
                "https://img.example/images-shot-1.jpg",
            ],
        )

    def test_details_view_path_filters_stale_non_screenshot_url_strings(self) -> None:
        stale_raw_urls = "\n".join(
            [
                "https://img.example/box-front.jpg",
                "https://img.example/screenshot-gameplay.jpg",
                "https://img.example/fanart-background.jpg",
                "https://img.example/title-screen.jpg",
            ]
        )

        urls = screenshot_urls_from_game(stale_raw_urls)

        self.assertEqual(
            urls,
            [
                "https://img.example/screenshot-gameplay.jpg",
                "https://img.example/title-screen.jpg",
            ],
        )


class ResolveCoverUrlTests(unittest.TestCase):
    def test_external_url_rejected_when_base_url_set(self):
        from rom_mate.cover.utils import filter_to_server_host
        result = filter_to_server_host("https://neoclone.screenscraper.fr/img/123.jpg", "https://my-romm-server")
        self.assertEqual(result, "")

    def test_external_url_allowed_when_base_url_empty(self):
        from rom_mate.cover.utils import filter_to_server_host
        result = filter_to_server_host("https://neoclone.screenscraper.fr/img/123.jpg", "")
        self.assertEqual(result, "https://neoclone.screenscraper.fr/img/123.jpg")

    def test_same_host_url_allowed(self):
        from rom_mate.cover.utils import filter_to_server_host
        result = filter_to_server_host("https://my-romm-server/api/roms/123/cover", "https://my-romm-server")
        self.assertEqual(result, "https://my-romm-server/api/roms/123/cover")

    def test_relative_url_resolves_to_base_host_and_passes_filter(self):
        from rom_mate.cover.utils import resolve_cover_url, filter_to_server_host
        resolved = resolve_cover_url("/api/roms/123/cover", "https://my-romm-server")
        result = filter_to_server_host(resolved, "https://my-romm-server")
        self.assertEqual(result, "https://my-romm-server/api/roms/123/cover")

    def test_port_mismatch_rejected(self):
        from rom_mate.cover.utils import filter_to_server_host
        result = filter_to_server_host("https://my-romm-server:9090/img/cover.jpg", "https://my-romm-server:8080")
        self.assertEqual(result, "")

    def test_external_url_allowed_when_base_url_has_no_netloc(self):
        from rom_mate.cover.utils import filter_to_server_host
        result = filter_to_server_host("https://neoclone.screenscraper.fr/img/123.jpg", "not-a-url")
        self.assertEqual(result, "https://neoclone.screenscraper.fr/img/123.jpg")


if __name__ == "__main__":
    unittest.main()
