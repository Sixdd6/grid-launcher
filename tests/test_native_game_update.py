from __future__ import annotations

import shutil
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import MagicMock

from rom_mate.library.archive_preparation import (
    merge_archive_into_directory,
    prepare_native_game_update_without_ui,
)
from rom_mate.library.install_registry import build_installed_game_record


class TestMergeArchiveIntoDirectory(unittest.TestCase):
    def _make_zip(self, zip_path: Path, files: dict[str, str]) -> None:
        with zipfile.ZipFile(zip_path, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
            for relative_path, content in files.items():
                archive.writestr(relative_path, content)

    def test_merge_overwrites_archive_files(self) -> None:
        root = Path(tempfile.mkdtemp())
        try:
            target_dir = root / "game"
            target_dir.mkdir(parents=True)
            (target_dir / "game.exe").write_text("old", encoding="utf-8")

            archive_path = root / "update.zip"
            self._make_zip(archive_path, {"game.exe": "new"})

            temp_dir = root / "merge-temp"
            merge_archive_into_directory(archive_path, target_dir, temp_dir)

            self.assertEqual((target_dir / "game.exe").read_text(encoding="utf-8"), "new")
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_merge_preserves_non_archive_files(self) -> None:
        root = Path(tempfile.mkdtemp())
        try:
            target_dir = root / "game"
            target_dir.mkdir(parents=True)
            (target_dir / "game.exe").write_text("old", encoding="utf-8")
            (target_dir / "save.sav").write_text("user-save", encoding="utf-8")

            archive_path = root / "update.zip"
            self._make_zip(archive_path, {"game.exe": "new"})

            temp_dir = root / "merge-temp"
            merge_archive_into_directory(archive_path, target_dir, temp_dir)

            self.assertEqual((target_dir / "game.exe").read_text(encoding="utf-8"), "new")
            self.assertTrue((target_dir / "save.sav").exists())
            self.assertEqual((target_dir / "save.sav").read_text(encoding="utf-8"), "user-save")
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_merge_creates_new_subdirectory(self) -> None:
        root = Path(tempfile.mkdtemp())
        try:
            target_dir = root / "game"
            target_dir.mkdir(parents=True)
            (target_dir / "game.exe").write_text("old", encoding="utf-8")

            archive_path = root / "update.zip"
            self._make_zip(
                archive_path,
                {
                    "game.exe": "new",
                    "data/textures.pak": "pak-data",
                },
            )

            temp_dir = root / "merge-temp"
            merge_archive_into_directory(archive_path, target_dir, temp_dir)

            self.assertTrue((target_dir / "data" / "textures.pak").exists())
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_merge_cleans_temp_dir_on_success(self) -> None:
        root = Path(tempfile.mkdtemp())
        try:
            target_dir = root / "game"
            target_dir.mkdir(parents=True)
            (target_dir / "game.exe").write_text("old", encoding="utf-8")

            archive_path = root / "update.zip"
            self._make_zip(archive_path, {"game.exe": "new"})

            temp_dir = root / "merge-temp"
            merge_archive_into_directory(archive_path, target_dir, temp_dir)

            self.assertFalse(temp_dir.exists())
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_merge_cleans_temp_dir_on_failure(self) -> None:
        root = Path(tempfile.mkdtemp())
        try:
            target_dir = root / "game"
            target_dir.mkdir(parents=True)

            archive_path = root / "missing.zip"
            temp_dir = root / "merge-temp"

            with self.assertRaises(Exception):
                merge_archive_into_directory(archive_path, target_dir, temp_dir)

            self.assertFalse(temp_dir.exists())
        finally:
            shutil.rmtree(root, ignore_errors=True)


class TestPrepareNativeGameUpdateWithoutUI(unittest.TestCase):
    def _make_zip(self, zip_path: Path, files: dict[str, str]) -> None:
        with zipfile.ZipFile(zip_path, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
            for relative_path, content in files.items():
                archive.writestr(relative_path, content)

    def test_returns_none_when_extracted_dir_missing(self) -> None:
        root = Path(tempfile.mkdtemp())
        try:
            archive_path = root / "update.zip"
            self._make_zip(archive_path, {"game.exe": "new"})

            installed_game = {"title": "Game", "extracted_dir": "", "extracted_path": ""}
            result, error_text = prepare_native_game_update_without_ui(
                installed_game,
                {},
                archive_path,
                temp_dir_for_game=MagicMock(return_value=root / "temp"),
                select_extracted_launch_file=MagicMock(return_value=None),
            )

            self.assertIsNone(result)
            self.assertTrue(isinstance(error_text, str) and error_text.strip())
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_returns_none_when_extracted_dir_not_on_disk(self) -> None:
        root = Path(tempfile.mkdtemp())
        try:
            archive_path = root / "update.zip"
            self._make_zip(archive_path, {"game.exe": "new"})

            missing_dir = root / "not-installed"
            installed_game = {
                "title": "Game",
                "extracted_dir": str(missing_dir),
                "extracted_path": "",
            }
            result, error_text = prepare_native_game_update_without_ui(
                installed_game,
                {},
                archive_path,
                temp_dir_for_game=MagicMock(return_value=root / "temp"),
                select_extracted_launch_file=MagicMock(return_value=None),
            )

            self.assertIsNone(result)
            self.assertTrue(isinstance(error_text, str) and error_text.strip())
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_preserves_native_executable_path(self) -> None:
        root = Path(tempfile.mkdtemp())
        try:
            target_dir = root / "game"
            target_dir.mkdir(parents=True)
            (target_dir / "game.exe").write_text("old", encoding="utf-8")

            archive_path = root / "update.zip"
            self._make_zip(archive_path, {"game.exe": "new"})

            installed_game = {
                "title": "Game",
                "extracted_dir": str(target_dir),
                "extracted_path": str(target_dir / "game.exe"),
                "native_executable_path": "/custom/path/game.exe",
            }

            updated_game, error_text = prepare_native_game_update_without_ui(
                installed_game,
                {},
                archive_path,
                temp_dir_for_game=MagicMock(return_value=target_dir.parent / "temp"),
                select_extracted_launch_file=MagicMock(return_value=target_dir / "game.exe"),
            )

            self.assertEqual(error_text, "")
            self.assertIsNotNone(updated_game)
            assert updated_game is not None
            self.assertEqual(updated_game.get("native_executable_path"), "/custom/path/game.exe")
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_updates_server_metadata_fields(self) -> None:
        root = Path(tempfile.mkdtemp())
        try:
            target_dir = root / "game"
            target_dir.mkdir(parents=True)
            (target_dir / "game.exe").write_text("old", encoding="utf-8")

            archive_path = root / "update.zip"
            self._make_zip(archive_path, {"game.exe": "new"})

            installed_game = {
                "title": "Game",
                "extracted_dir": str(target_dir),
                "extracted_path": str(target_dir / "game.exe"),
            }
            update_game = {
                "rom_file_name": "game-v2.zip",
                "server_updated_at": "2026-01-01T00:00:00",
            }

            updated_game, error_text = prepare_native_game_update_without_ui(
                installed_game,
                update_game,
                archive_path,
                temp_dir_for_game=MagicMock(return_value=target_dir.parent / "temp"),
                select_extracted_launch_file=MagicMock(return_value=target_dir / "game.exe"),
            )

            self.assertEqual(error_text, "")
            self.assertIsNotNone(updated_game)
            assert updated_game is not None
            self.assertEqual(updated_game.get("rom_file_name"), "game-v2.zip")
            self.assertEqual(updated_game.get("server_updated_at"), "2026-01-01T00:00:00")
        finally:
            shutil.rmtree(root, ignore_errors=True)


class TestBuildInstalledGameRecordNativeExecutablePath(unittest.TestCase):
    def test_build_installed_game_record_preserves_native_executable_path(self) -> None:
        record = build_installed_game_record(
            {
                "title": "Game",
                "platform": "Windows",
                "native_executable_path": "/path/to/game.exe",
            },
            Path("game.zip"),
            resolved_cover_url="",
            cached_cover_path="",
        )

        self.assertEqual(record.get("native_executable_path"), "/path/to/game.exe")


if __name__ == "__main__":
    unittest.main()
