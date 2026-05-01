from __future__ import annotations

import unittest

from rom_mate.server.catalog import server_platform_details


class ServerPlatformDetailsTests(unittest.TestCase):
    def test_empty_payload_returns_empty_list(self) -> None:
        self.assertEqual(server_platform_details([]), [])

    def test_non_list_payload_returns_empty_list(self) -> None:
        self.assertEqual(server_platform_details(None), [])

    def test_valid_entry_returns_name_and_slug(self) -> None:
        payload = [
            {
                "id": 1,
                "name": "SNES",
                "slug": "snes",
                "rom_count": 5,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(isinstance(result[0]["name"], str) and result[0]["name"].strip())
        self.assertEqual(result[0]["slug"], "snes")

    def test_zero_rom_count_entry_is_excluded(self) -> None:
        payload = [
            {
                "id": 2,
                "name": "Empty Platform",
                "slug": "empty-platform",
                "rom_count": 0,
            }
        ]

        self.assertEqual(server_platform_details(payload), [])

    def test_known_slug_fills_static_metadata(self) -> None:
        payload = [
            {
                "id": 3,
                "name": "Super Nintendo",
                "slug": "snes",
                "rom_count": 10,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(isinstance(result[0]["manufacturer"], str) and result[0]["manufacturer"].strip())
        self.assertTrue(isinstance(result[0]["release_year"], str) and result[0]["release_year"].strip())
        self.assertTrue(isinstance(result[0]["player_count"], str) and result[0]["player_count"].strip())

    def test_unknown_slug_uses_empty_strings(self) -> None:
        payload = [
            {
                "id": 4,
                "name": "Unknown",
                "slug": "unknownxyz999",
                "rom_count": 3,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["manufacturer"], "")
        self.assertEqual(result[0]["release_year"], "")
        self.assertEqual(result[0]["player_count"], "")

    def test_known_slug_produces_local_logo_path(self) -> None:
        payload = [
            {
                "id": 8,
                "name": "SNES",
                "slug": "snes",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(isinstance(result[0]["local_logo_path"], str) and result[0]["local_logo_path"].strip())
        self.assertTrue(
            result[0]["local_logo_path"].endswith("Nintendo%20-%20Super%20Nintendo%20Entertainment%20System.png")
        )

    def test_unknown_slug_produces_empty_local_logo_path(self) -> None:
        payload = [
            {
                "id": 9,
                "name": "Unknown",
                "slug": "unknownxyz999",
                "rom_count": 3,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["local_logo_path"], "")

    def test_name_fallback_matches_playstation(self) -> None:
        payload = [
            {
                "id": 11,
                "name": "PlayStation",
                "slug": "playstation",
                "rom_count": 4,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(isinstance(result[0]["local_logo_path"], str) and result[0]["local_logo_path"].strip())
        self.assertTrue(result[0]["local_logo_path"].endswith("Sony%20-%20PlayStation.png"))

    def test_name_fallback_matches_playstation_2(self) -> None:
        payload = [
            {
                "id": 12,
                "name": "PlayStation 2",
                "slug": "playstation-2",
                "rom_count": 6,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(isinstance(result[0]["local_logo_path"], str) and result[0]["local_logo_path"].strip())
        self.assertTrue(result[0]["local_logo_path"].endswith("Sony%20-%20PlayStation%202.png"))

    def test_windows_9x_display_name_maps_to_windows9x_logo(self) -> None:
        payload = [
            {
                "id": 24,
                "name": "Windows",
                "display_name": "Windows 9x",
                "slug": "win",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("windows9x.png"))

    def test_windows_display_name_falls_back_to_slug(self) -> None:
        payload = [
            {
                "id": 25,
                "name": "Windows",
                "slug": "win",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("windows7.png"))

    def test_psx_slug_maps_to_playstation_logo(self) -> None:
        payload = [
            {
                "id": 13,
                "name": "PlayStation",
                "slug": "psx",
                "rom_count": 3,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("Sony%20-%20PlayStation.png"))

    def test_gamecube_slug_maps_to_gamecube_logo(self) -> None:
        payload = [
            {
                "id": 14,
                "name": "GameCube",
                "slug": "gamecube",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("Nintendo%20-%20GameCube.png"))

    def test_ngc_slug_maps_to_gamecube_logo(self) -> None:
        payload = [
            {
                "id": 18,
                "name": "GameCube",
                "slug": "ngc",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("Nintendo%20-%20GameCube.png"))

    def test_pcengine_slug_maps_to_turbografx16_logo(self) -> None:
        payload = [
            {
                "id": 15,
                "name": "PC Engine",
                "slug": "pcengine",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("NEC%20-%20PC%20Engine%20-%20TurboGrafx%2016.png"))

    def test_tg16_slug_maps_to_turbografx16_logo(self) -> None:
        payload = [
            {
                "id": 19,
                "name": "TurboGrafx-16",
                "slug": "tg16",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("NEC%20-%20PC%20Engine%20-%20TurboGrafx%2016.png"))

    def test_pcenginecd_slug_maps_to_turbografxcd_logo(self) -> None:
        payload = [
            {
                "id": 16,
                "name": "PC Engine CD",
                "slug": "pcenginecd",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("NEC%20-%20PC%20Engine%20CD%20-%20TurboGrafx-CD.png"))

    def test_turbografxcd_slug_maps_to_turbografxcd_logo(self) -> None:
        payload = [
            {
                "id": 20,
                "name": "TurboGrafx-CD",
                "slug": "turbografx-cd",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("NEC%20-%20PC%20Engine%20CD%20-%20TurboGrafx-CD.png"))

    def test_genesis_slug_maps_to_mega_drive_genesis_logo(self) -> None:
        payload = [
            {
                "id": 17,
                "name": "Genesis",
                "slug": "genesis",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("Sega%20-%20Mega%20Drive%20-%20Genesis.png"))

    def test_sega32_slug_maps_to_32x_logo(self) -> None:
        payload = [
            {
                "id": 21,
                "name": "Sega 32X",
                "slug": "sega32",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("Sega%20-%2032X.png"))

    def test_pico_slug_maps_to_pico8_logo(self) -> None:
        payload = [
            {
                "id": 22,
                "name": "PICO-8",
                "slug": "pico",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("Pico8.png"))

    def test_segapico_slug_maps_to_sega_pico_logo(self) -> None:
        payload = [
            {
                "id": 23,
                "name": "Sega Pico",
                "slug": "segapico",
                "rom_count": 2,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["local_logo_path"].endswith("Sega%20-%20PICO.png"))

    def test_url_logo_preserved_alongside_local_logo_path(self) -> None:
        payload = [
            {
                "id": 10,
                "name": "SNES",
                "slug": "snes",
                "rom_count": 5,
                "url_logo": "https://example.com/logo.png",
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertTrue(isinstance(result[0]["local_logo_path"], str) and result[0]["local_logo_path"].strip())
        self.assertTrue(
            result[0]["local_logo_path"].endswith("Nintendo%20-%20Super%20Nintendo%20Entertainment%20System.png")
        )
        self.assertEqual(result[0]["url_logo"], "https://example.com/logo.png")

    def test_display_name_preferred_over_name(self) -> None:
        payload = [
            {
                "id": 5,
                "name": "foo",
                "display_name": "Bar",
                "slug": "bar",
                "rom_count": 1,
            }
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["name"], "Bar")

    def test_result_sorted_by_name(self) -> None:
        payload = [
            {
                "id": 6,
                "name": "Zebra Platform",
                "slug": "zzz",
                "rom_count": 1,
            },
            {
                "id": 7,
                "name": "Apple Platform",
                "slug": "aaa",
                "rom_count": 1,
            },
        ]

        result = server_platform_details(payload)

        self.assertEqual(len(result), 2)
        self.assertTrue(result[0]["name"] < result[1]["name"])


if __name__ == "__main__":
    unittest.main()
