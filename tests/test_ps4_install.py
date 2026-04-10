from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from rom_mate.core.config import merge_config_with_defaults, normalize_installed_games
from rom_mate.library.archive_preparation import (
    _is_ps4_platform,
    prepare_installed_game_without_ui,
    select_extracted_launch_file,
)


class PS4InstallTests(unittest.TestCase):
    def test_is_ps4_platform_detects_common_labels(self) -> None:
        self.assertTrue(_is_ps4_platform({"platform": "PlayStation 4"}))
        self.assertTrue(_is_ps4_platform({"platform": "PS4"}))
        self.assertTrue(_is_ps4_platform({"platform": "Sony Playstation-4"}))
        self.assertFalse(_is_ps4_platform({"platform": "PlayStation 3"}))

    def test_select_extracted_launch_file_prefers_eboot_under_title_id_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            extracted_dir = Path(temp_dir) / "game"
            real_dir = extracted_dir / "CUSA12345"
            decoy_dir = extracted_dir / "extras"
            real_dir.mkdir(parents=True)
            decoy_dir.mkdir(parents=True)

            real_eboot = real_dir / "eboot.bin"
            decoy_eboot = decoy_dir / "eboot.bin"
            real_eboot.write_bytes(b"real")
            decoy_eboot.write_bytes(b"decoy")

            selected = select_extracted_launch_file(
                {"platform": "PS4"},
                extracted_dir,
                Path(temp_dir) / "CUSA12345.zip",
                is_ps3_platform=lambda game: False,
            )

        self.assertEqual(selected, real_eboot)

    def test_prepare_installed_game_without_ui_sets_ps4_game_id(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "CUSA54321.zip"
            archive_path.write_bytes(b"placeholder")
            extracted_dir = Path(temp_dir) / "extract"
            launch_file = extracted_dir / "CUSA54321" / "eboot.bin"
            launch_file.parent.mkdir(parents=True)
            launch_file.write_bytes(b"boot")

            prepared, warning_text = prepare_installed_game_without_ui(
                {"title": "Test PS4 Game", "platform": "PlayStation 4"},
                archive_path,
                configure_ps3_links=False,
                should_extract_archive_for_game=lambda game, path: True,
                extract_archive_for_game=lambda game, path, install_progress_callback: (launch_file, extracted_dir),
                is_ps3_platform=lambda game: False,
                configure_ps3_install_links=lambda game, path: [],
                update_rpcs3_games_yml_for_install=lambda game, path, links: "",
            )

        self.assertIsNotNone(prepared)
        self.assertEqual(warning_text, "")
        assert prepared is not None
        self.assertEqual(prepared.get("ps4_game_id"), "CUSA54321")
        self.assertEqual(prepared.get("extracted_path"), str(launch_file))

    def test_merge_config_with_defaults_normalizes_installed_ps4_game_id(self) -> None:
        defaults = {
            "installed_games": [],
            "emulators": [],
            "default_emulators": {},
            "default_retroarch_cores": {},
            "cloud_sync_state": {},
            "first_run_completed": False,
        }
        content = {
            "installed_games": [
                {
                    "title": " Demo PS4 ",
                    "platform": " PS4 ",
                    "ps4_game_id": " cusa00001 ",
                }
            ]
        }

        merged = merge_config_with_defaults(
            defaults,
            content,
            normalize_emulators=lambda value: [],
            normalize_default_emulators=lambda value: {},
            normalize_default_retroarch_cores=lambda value: {},
            normalize_installed_games=lambda value: normalize_installed_games(
                value,
                lambda game: (game.get("title", ""), game.get("platform", "")),
            ),
            normalize_cloud_sync_state=lambda value: {},
        )

        installed = merged["installed_games"]
        self.assertEqual(len(installed), 1)
        self.assertEqual(installed[0]["title"], "Demo PS4")
        self.assertEqual(installed[0]["platform"], "PS4")
        self.assertEqual(installed[0]["ps4_game_id"], "CUSA00001")


if __name__ == "__main__":
    unittest.main()
