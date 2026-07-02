from __future__ import annotations

import shutil
import tempfile
import unittest
from pathlib import Path

from grid_launcher.library.install_cleanup import remove_game_files
from grid_launcher.library.install_paths import (
    candidate_archive_paths_for_game,
    candidate_extracted_dirs_for_game,
)


class TestNativeGameDirPathResolution(unittest.TestCase):
    def setUp(self) -> None:
        self._temp_dirs: list[str] = []

    def tearDown(self) -> None:
        for temp_dir in self._temp_dirs:
            shutil.rmtree(temp_dir, ignore_errors=True)

    def _make_temp_dir(self) -> Path:
        temp_dir = tempfile.mkdtemp()
        self._temp_dirs.append(temp_dir)
        return Path(temp_dir)

    def test_candidate_archive_paths_includes_native_game_dir(self) -> None:
        native_game_dir = self._make_temp_dir()
        game = {"native_game_dir": str(native_game_dir)}

        candidates = candidate_archive_paths_for_game(
            game,
            platform_library_dir=lambda entry: None,
            archive_name_for_game=lambda entry: "game.zip",
            library_path_dir=lambda: None,
        )

        self.assertIn(native_game_dir / "game.zip", candidates)

    def test_candidate_archive_paths_without_native_game_dir(self) -> None:
        platform_library = self._make_temp_dir()
        game: dict[str, str] = {}

        candidates = candidate_archive_paths_for_game(
            game,
            platform_library_dir=lambda entry: platform_library,
            archive_name_for_game=lambda entry: "game.zip",
            library_path_dir=lambda: None,
        )

        self.assertIn(platform_library / "game.zip", candidates)
        self.assertNotIn(None, candidates)

    def test_candidate_extracted_dirs_includes_native_game_dir(self) -> None:
        native_game_dir = self._make_temp_dir()
        game = {"native_game_dir": str(native_game_dir)}
        archive_paths = [Path("/library/platform/game.zip")]

        candidates = candidate_extracted_dirs_for_game(
            game,
            archive_paths,
            extracted_dir_for_archive_path=lambda path: path.with_suffix(""),
        )

        self.assertIn(native_game_dir / "game", candidates)

    def test_candidate_extracted_dirs_without_native_game_dir(self) -> None:
        game: dict[str, str] = {}
        archive_paths = [Path("/library/platform/game.zip")]

        candidates = candidate_extracted_dirs_for_game(
            game,
            archive_paths,
            extracted_dir_for_archive_path=lambda path: path.with_suffix(""),
        )

        self.assertIn(Path("/library/platform/game"), candidates)


class TestRemoveGameFilesNativeLayout(unittest.TestCase):
    def setUp(self) -> None:
        self._temp_dirs: list[str] = []

    def tearDown(self) -> None:
        for temp_dir in self._temp_dirs:
            shutil.rmtree(temp_dir, ignore_errors=True)

    def _make_temp_dir(self) -> Path:
        temp_dir = tempfile.mkdtemp()
        self._temp_dirs.append(temp_dir)
        return Path(temp_dir)

    def test_removes_native_game_dir_entirely(self) -> None:
        native_game_dir = self._make_temp_dir()
        game_folder = native_game_dir / "Some Game"
        game_folder.mkdir()
        (game_folder / "game.exe").write_bytes(b"exe")
        prefix_dir = native_game_dir / "prefix"
        prefix_dir.mkdir()
        (prefix_dir / "drive_c").mkdir()

        game = {"native_game_dir": str(native_game_dir)}

        remove_game_files(
            game,
            is_ps3_platform=lambda entry: False,
            is_native_executable_platform=lambda entry: True,
            candidate_extracted_dirs_for_game=lambda entry: [],
            remove_directory_tree=shutil.rmtree,
            candidate_archive_paths_for_game=lambda entry: [],
        )

        self.assertFalse(native_game_dir.exists())

    def test_falls_back_to_extracted_dir_when_no_native_game_dir(self) -> None:
        extracted_dir = self._make_temp_dir()
        (extracted_dir / "game.exe").write_bytes(b"exe")

        game: dict[str, str] = {}

        remove_game_files(
            game,
            is_ps3_platform=lambda entry: False,
            is_native_executable_platform=lambda entry: True,
            candidate_extracted_dirs_for_game=lambda entry: [extracted_dir],
            remove_directory_tree=shutil.rmtree,
            candidate_archive_paths_for_game=lambda entry: [],
        )

        self.assertFalse(extracted_dir.exists())

    def test_no_error_when_native_game_dir_missing_from_disk(self) -> None:
        missing_dir = Path(tempfile.gettempdir()) / "grid_launcher_nonexistent_native_dir"
        self.assertFalse(missing_dir.exists())

        game = {"native_game_dir": str(missing_dir)}

        remove_game_files(
            game,
            is_ps3_platform=lambda entry: False,
            is_native_executable_platform=lambda entry: True,
            candidate_extracted_dirs_for_game=lambda entry: [],
            remove_directory_tree=shutil.rmtree,
            candidate_archive_paths_for_game=lambda entry: [],
        )


if __name__ == "__main__":
    unittest.main()
