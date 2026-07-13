from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from grid_launcher.background import InstallDownloadWorker
from grid_launcher.emulator import emulator_install_directory
from grid_launcher.ui.mixins.install_mixin import InstallMixin


class _StubWindow(InstallMixin):
    def __init__(self, library_path: Path, archive_name: str) -> None:
        self._library_path = library_path
        self._archive_name = archive_name
        self.install_in_progress = False
        self.install_finalize_in_progress = False
        self.install_pending_game: dict | None = None
        self.install_finalize_game: dict | None = None
        self.install_queue: list[dict] = []
        self.active_download_entry_id = ""
        self.active_download_count = 0
        self.active_download_bytes = 0
        self.active_download_total = 0
        self.active_download_speed_bps = 0.0
        self.install_thread = None
        self.install_worker = None

    def _install_block_reason_for_game(self, game: dict[str, str]) -> str:
        return ""

    def _library_path_dir(self) -> Path:
        return self._library_path

    def _archive_name_for_game(self, game: dict[str, str]) -> str:
        return self._archive_name

    def _game_key(self, game: dict[str, str]) -> str:
        return "stub-key"

    def _create_download_entry(self, game: dict[str, str], status: str) -> str:
        return "entry-1"

    def _set_download_entry_status(self, entry_id: str, status: str) -> None:
        pass

    def _update_download_status_ui(self) -> None:
        pass

    def _update_details_action_buttons(self) -> None:
        pass

    def _debug_prints_enabled(self) -> bool:
        return False


def _run_install(library_path: Path, archive_name: str, source_metadata: dict) -> Path:
    window = _StubWindow(library_path, archive_name)
    game = {"title": "Test Emulator", "_source_metadata": source_metadata}
    with patch("grid_launcher.ui.mixins.install_mixin.InstallDownloadWorker") as mock_worker_cls, \
            patch("grid_launcher.ui.mixins.install_mixin.QThread") as mock_thread_cls:
        result = window._start_async_source_emulator_install(game)
    assert result is True
    mock_worker_cls.assert_called_once()
    return mock_worker_cls.call_args.args[2]


class EmulatorInstallSubfolderTests(unittest.TestCase):
    def test_appimage_downloads_into_emulator_subfolder(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            library_path = Path(temp_dir)
            archive_path = _run_install(
                library_path,
                "DuckStation-x64.AppImage",
                {"provider": "github", "supplemental_downloads": []},
            )

        expected_dir = emulator_install_directory(library_path, "DuckStation-x64")
        self.assertEqual(archive_path, expected_dir / "DuckStation-x64.AppImage")
        self.assertEqual(archive_path.parent, library_path / "Emulators" / "DuckStation-x64")

    def test_archive_downloads_into_emulator_subfolder(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            library_path = Path(temp_dir)
            archive_path = _run_install(
                library_path,
                "PCSX2.zip",
                {"provider": "github", "supplemental_downloads": []},
            )

        expected_dir = emulator_install_directory(library_path, "PCSX2")
        self.assertEqual(archive_path, expected_dir / "PCSX2.zip")
        self.assertEqual(archive_path.parent, library_path / "Emulators" / "PCSX2")

    def test_subfolder_created_before_download_begins(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            library_path = Path(temp_dir)
            archive_path = _run_install(
                library_path,
                "PPSSPP.zip",
                {"provider": "github", "supplemental_downloads": []},
            )

            expected_dir = emulator_install_directory(library_path, "PPSSPP")
            self.assertTrue(expected_dir.exists())
            self.assertTrue(expected_dir.is_dir())
            self.assertEqual(archive_path.parent, expected_dir)

    def test_download_not_placed_at_emulators_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            library_path = Path(temp_dir)
            archive_path = _run_install(
                library_path,
                "cemu.zip",
                {"provider": "github", "supplemental_downloads": []},
            )

        self.assertNotEqual(archive_path.parent, library_path / "Emulators")
        self.assertEqual(archive_path.parent.name, "cemu")

    def test_supplemental_archives_land_in_emulator_subfolder(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            library_path = Path(temp_dir)
            source_metadata = {
                "provider": "github",
                "supplemental_downloads": [
                    {"asset_name": "firmware.zip"},
                    {"asset_name": "keys.zip"},
                ],
            }
            archive_path = _run_install(library_path, "Eden.zip", source_metadata)

        expected_dir = emulator_install_directory(library_path, "Eden")
        worker = InstallDownloadWorker(
            "", {}, archive_path, source_metadata=source_metadata
        )
        first = worker._supplemental_archive_path(archive_path, 1, "firmware.zip")
        second = worker._supplemental_archive_path(archive_path, 2, "keys.zip")

        self.assertEqual(first.parent, expected_dir)
        self.assertEqual(second.parent, expected_dir)
        self.assertEqual(first.name, "Eden-supplemental-1.zip")
        self.assertEqual(second.name, "Eden-supplemental-2.zip")


if __name__ == "__main__":
    unittest.main()
