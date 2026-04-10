from __future__ import annotations

import json
import tempfile
import unittest
import zipfile
from pathlib import Path

from rom_mate.core.config import normalize_installed_games
from rom_mate.library.archive_preparation import (
    apply_ps4_content_archive_without_ui,
    extract_archive_into_directory,
    extracted_dir_for_archive_path,
)
from rom_mate.library.install_registry import build_installed_game_record


class PS4ContentApplyTests(unittest.TestCase):
    def _write_ps4_content_zip(self, zip_path: Path, title_id: str, file_map: dict[str, bytes]) -> None:
        with zipfile.ZipFile(zip_path, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
            for relative_path, payload in file_map.items():
                archive.writestr(f"{title_id}/{relative_path}", payload)

    def test_apply_ps4_content_archive_merges_into_existing_title_dir_and_tracks_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            installed_root = root / "installed"
            title_dir = installed_root / "CUSA12345"
            title_dir.mkdir(parents=True)

            existing_file = title_dir / "sce_sys" / "param.sfo"
            existing_file.parent.mkdir(parents=True)
            existing_file.write_bytes(b"old")

            archive_path = root / "update.zip"
            self._write_ps4_content_zip(
                archive_path,
                "CUSA12345",
                {
                    "sce_sys/param.sfo": b"new",
                    "patch/data.bin": b"patch",
                },
            )

            updated_game, warning_text = apply_ps4_content_archive_without_ui(
                {
                    "title": "Demo Game",
                    "platform": "PS4",
                    "ps4_game_id": "CUSA12345",
                    "extracted_dir": str(installed_root),
                },
                archive_path,
                content_kind="update",
                extracted_dir_for_archive_path=extracted_dir_for_archive_path,
                extract_archive_into_directory=extract_archive_into_directory,
            )

            self.assertEqual(warning_text, "")
            self.assertIsNotNone(updated_game)
            assert updated_game is not None
            self.assertEqual(existing_file.read_bytes(), b"new")
            self.assertEqual((title_dir / "patch" / "data.bin").read_bytes(), b"patch")
            self.assertFalse(archive_path.exists())

            metadata = json.loads(updated_game.get("ps4_content", "[]"))
            self.assertEqual(len(metadata), 1)
            self.assertEqual(metadata[0]["kind"], "update")
            self.assertEqual(metadata[0]["title_id"], "CUSA12345")
            self.assertEqual(metadata[0]["archive_name"], "update.zip")
            self.assertTrue(metadata[0].get("applied_at", "").isdigit())

    def test_apply_ps4_content_archive_fails_on_title_id_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            installed_root = root / "installed"
            title_dir = installed_root / "CUSA12345"
            title_dir.mkdir(parents=True)
            (title_dir / "eboot.bin").write_bytes(b"base")

            archive_path = root / "dlc.zip"
            self._write_ps4_content_zip(
                archive_path,
                "CUSA99999",
                {
                    "dlc/content.pkg": b"dlc",
                },
            )

            updated_game, error_text = apply_ps4_content_archive_without_ui(
                {
                    "title": "Demo Game",
                    "platform": "PlayStation 4",
                    "ps4_game_id": "CUSA12345",
                    "extracted_dir": str(installed_root),
                },
                archive_path,
                content_kind="dlc",
                extracted_dir_for_archive_path=extracted_dir_for_archive_path,
                extract_archive_into_directory=extract_archive_into_directory,
            )

            self.assertIsNone(updated_game)
            self.assertIn("title ID mismatch", error_text)
            self.assertIn("expected CUSA12345", error_text)
            self.assertEqual((title_dir / "eboot.bin").read_bytes(), b"base")

    def test_build_installed_record_and_config_normalization_keep_ps4_content_metadata(self) -> None:
        record = build_installed_game_record(
            {
                "title": "Demo Game",
                "platform": "PS4",
                "ps4_game_id": "cusa12345",
                "ps4_content": " [{\"kind\":\"update\"}] ",
            },
            Path("demo.zip"),
            resolved_cover_url="",
            cached_cover_path="",
        )
        self.assertEqual(record["ps4_content"], "[{\"kind\":\"update\"}]")

        normalized = normalize_installed_games(
            [record],
            lambda game: (game.get("title", ""), game.get("platform", "")),
        )
        self.assertEqual(len(normalized), 1)
        self.assertEqual(normalized[0]["ps4_game_id"], "CUSA12345")
        self.assertEqual(normalized[0]["ps4_content"], "[{\"kind\":\"update\"}]")


if __name__ == "__main__":
    unittest.main()
