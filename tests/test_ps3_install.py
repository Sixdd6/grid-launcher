from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from rom_mate.library.archive_preparation import prepare_installed_game_without_ui


class PS3InstallTests(unittest.TestCase):
    def test_prepare_installed_game_without_ui_uses_extracted_dir_as_extracted_path_for_ps3(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "game.7z"
            archive_path.write_bytes(b"placeholder")
            extracted_dir = Path(temp_dir) / "game"

            def fake_extract(archive: Path, destination: Path, install_progress_callback=None) -> None:
                del archive
                del install_progress_callback
                destination.mkdir(parents=True, exist_ok=True)
                (destination / "PS3_GAME").mkdir()
                (destination / "PS3_DISC.SFB").write_bytes(b"disc")

            with patch("rom_mate.library.archive_preparation.extract_archive_into_directory", side_effect=fake_extract):
                prepared, warning_text = prepare_installed_game_without_ui(
                    {"title": "Test PS3 Game", "platform": "PlayStation 3"},
                    archive_path,
                    configure_ps3_links=False,
                    should_extract_archive_for_game=lambda game, path: True,
                    extract_archive_for_game=lambda game, path, install_progress_callback: (_ for _ in ()).throw(
                        AssertionError("extract_archive_for_game should not be used for PS3 installs")
                    ),
                    is_ps3_platform=lambda game: True,
                    configure_ps3_install_links=lambda game, path: [],
                    update_rpcs3_games_yml_for_install=lambda game, path, links: "",
                )

        self.assertIsNotNone(prepared)
        self.assertEqual(warning_text, "")
        assert prepared is not None
        self.assertEqual(prepared.get("extracted_dir"), str(extracted_dir))
        self.assertEqual(prepared.get("extracted_path"), str(extracted_dir))


if __name__ == "__main__":
    unittest.main()
