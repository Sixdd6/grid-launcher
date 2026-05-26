from __future__ import annotations

import json
import os
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

from rom_mate.library.cloud_sync import cemu_save_directories_for_game, cloud_sync_candidates_for_game
from rom_mate.library.cloud_transfer import (
    appended_image_sidecar_path,
    grouped_file_upload_jobs,
    normalize_manual_save_path,
    ppsspp_state_upload_jobs,
    resolve_native_save_dir,
    retroarch_state_upload_jobs,
    session_screenshot_path,
    screenshot_download_candidate_paths,
    zip_directory_for_upload,
    zip_native_save_dirs_for_upload,
    zip_selected_files_for_upload,
)


class CloudTransferTests(unittest.TestCase):
    def test_resolve_native_save_dir_returns_expanded_when_no_windows_documents(self) -> None:
        raw_path = "%USERPROFILE%\\Documents\\Game\\saves"
        expansions = {
            "%USERPROFILE%\\Documents\\Game\\saves": "C:\\Users\\TestUser\\Documents\\Game\\saves",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = resolve_native_save_dir(raw_path, windows_documents=None)

        self.assertEqual(result, Path("C:\\Users\\TestUser\\Documents\\Game\\saves"))

    def test_resolve_native_save_dir_no_redirection_returns_standard_expansion(self) -> None:
        raw_path = "%USERPROFILE%\\Documents\\Game\\saves"
        windows_documents = Path("C:\\Users\\TestUser\\Documents")
        expansions = {
            "%USERPROFILE%": "C:\\Users\\TestUser",
            "%USERPROFILE%\\Documents\\Game\\saves": "C:\\Users\\TestUser\\Documents\\Game\\saves",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = resolve_native_save_dir(raw_path, windows_documents)

        self.assertEqual(result, Path("C:\\Users\\TestUser\\Documents\\Game\\saves"))

    def test_resolve_native_save_dir_redirected_documents_uses_shell_path(self) -> None:
        raw_path = "%USERPROFILE%\\Documents\\Square Enix\\Batman GOTY\\SaveData"
        windows_documents = Path("Y:\\Users\\TestUser\\Documents")
        expansions = {
            "%USERPROFILE%": "C:\\Users\\TestUser",
            "%USERPROFILE%\\Documents\\Square Enix\\Batman GOTY\\SaveData": "C:\\Users\\TestUser\\Documents\\Square Enix\\Batman GOTY\\SaveData",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = resolve_native_save_dir(raw_path, windows_documents)

        self.assertEqual(
            result,
            Path("Y:\\Users\\TestUser\\Documents\\Square Enix\\Batman GOTY\\SaveData"),
        )

    def test_resolve_native_save_dir_non_documents_path_unaffected_by_redirection(self) -> None:
        raw_path = "%APPDATA%\\Game\\saves"
        windows_documents = Path("Y:\\Users\\TestUser\\Documents")
        expansions = {
            "%USERPROFILE%": "C:\\Users\\TestUser",
            "%APPDATA%\\Game\\saves": "C:\\Users\\TestUser\\AppData\\Roaming\\Game\\saves",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = resolve_native_save_dir(raw_path, windows_documents)

        self.assertEqual(result, Path("C:\\Users\\TestUser\\AppData\\Roaming\\Game\\saves"))

    def test_normalize_manual_save_path_appdata_roaming(self) -> None:
        expansions = {
            "%APPDATA%": "C:\\Users\\TestUser\\AppData\\Roaming",
            "%LOCALAPPDATA%": "C:\\Users\\TestUser\\AppData\\Local",
            "%USERPROFILE%": "C:\\Users\\TestUser",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = normalize_manual_save_path("C:\\Users\\TestUser\\AppData\\Roaming\\SomeGame\\saves")

        self.assertEqual(result, "%APPDATA%\\SomeGame\\saves")

    def test_normalize_manual_save_path_appdata_local(self) -> None:
        expansions = {
            "%APPDATA%": "C:\\Users\\TestUser\\AppData\\Roaming",
            "%LOCALAPPDATA%": "C:\\Users\\TestUser\\AppData\\Local",
            "%USERPROFILE%": "C:\\Users\\TestUser",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = normalize_manual_save_path("C:\\Users\\TestUser\\AppData\\Local\\SomeGame\\saves")

        self.assertEqual(result, "%LOCALAPPDATA%\\SomeGame\\saves")

    def test_normalize_manual_save_path_appdata_locallow(self) -> None:
        expansions = {
            "%APPDATA%": "C:\\Users\\TestUser\\AppData\\Roaming",
            "%LOCALAPPDATA%": "C:\\Users\\TestUser\\AppData\\Local",
            "%USERPROFILE%": "C:\\Users\\TestUser",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = normalize_manual_save_path("C:\\Users\\TestUser\\AppData\\LocalLow\\Paralives\\MySaves.mod")

        self.assertEqual(result, "%USERPROFILE%\\AppData\\LocalLow\\Paralives\\MySaves.mod")

    def test_normalize_manual_save_path_documents(self) -> None:
        expansions = {
            "%APPDATA%": "C:\\Users\\TestUser\\AppData\\Roaming",
            "%LOCALAPPDATA%": "C:\\Users\\TestUser\\AppData\\Local",
            "%USERPROFILE%": "C:\\Users\\TestUser",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = normalize_manual_save_path("C:\\Users\\TestUser\\Documents\\MyGame\\saves")

        self.assertEqual(result, "%USERPROFILE%\\Documents\\MyGame\\saves")

    def test_normalize_manual_save_path_other_userprofile_subpath(self) -> None:
        expansions = {
            "%APPDATA%": "C:\\Users\\TestUser\\AppData\\Roaming",
            "%LOCALAPPDATA%": "C:\\Users\\TestUser\\AppData\\Local",
            "%USERPROFILE%": "C:\\Users\\TestUser",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = normalize_manual_save_path("C:\\Users\\TestUser\\Saved Games\\SomeGame")

        self.assertEqual(result, "%USERPROFILE%\\Saved Games\\SomeGame")

    def test_normalize_manual_save_path_unrecognized_path_unchanged(self) -> None:
        expansions = {
            "%APPDATA%": "C:\\Users\\TestUser\\AppData\\Roaming",
            "%LOCALAPPDATA%": "C:\\Users\\TestUser\\AppData\\Local",
            "%USERPROFILE%": "C:\\Users\\TestUser",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = normalize_manual_save_path("D:\\GameSaves\\SomeGame")

        self.assertEqual(result, "D:\\GameSaves\\SomeGame")

    def test_normalize_manual_save_path_forward_slashes_normalized(self) -> None:
        expansions = {
            "%APPDATA%": "C:\\Users\\TestUser\\AppData\\Roaming",
            "%LOCALAPPDATA%": "C:\\Users\\TestUser\\AppData\\Local",
            "%USERPROFILE%": "C:\\Users\\TestUser",
        }

        with patch(
            "rom_mate.library.cloud_transfer.os.path.expandvars",
            side_effect=lambda value: expansions.get(value, value),
        ):
            result = normalize_manual_save_path("C:/Users/TestUser/AppData/Roaming/SomeGame/saves")

        self.assertEqual(result, "%APPDATA%\\SomeGame\\saves")

    def test_zip_directory_for_upload_skips_os_metadata_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            save_dir = Path(temp_dir) / "ULUS12345"
            save_dir.mkdir()
            (save_dir / "DATA.BIN").write_text("save-data", encoding="utf-8")
            (save_dir / "ICON0.PNG").write_bytes(b"\x89PNG\r\n\x1a\n")
            (save_dir / "Thumbs.db").write_bytes(b"not-an-image")
            (save_dir / "desktop.ini").write_text("cache", encoding="utf-8")

            archive_path = zip_directory_for_upload(save_dir, "Test Game")
            try:
                with zipfile.ZipFile(archive_path) as archive:
                    members = set(archive.namelist())
            finally:
                archive_path.unlink(missing_ok=True)

        self.assertIn("ULUS12345/DATA.BIN", members)
        self.assertIn("ULUS12345/ICON0.PNG", members)
        self.assertNotIn("ULUS12345/Thumbs.db", members)
        self.assertNotIn("ULUS12345/desktop.ini", members)

    def test_zip_native_save_dirs_skips_unreadable_directory(self) -> None:
        _ConcreteBase = type(Path())

        class _FailingPath(_ConcreteBase):
            def rglob(self, pattern):
                raise PermissionError("access denied")

        with tempfile.TemporaryDirectory() as temp_dir_a, tempfile.TemporaryDirectory() as temp_dir_b:
            dir_a = Path(temp_dir_a)
            (dir_a / "save.dat").write_text("save", encoding="utf-8")
            failing_dir_b = _FailingPath(temp_dir_b)
            dir_map = [
                ("%APPDATA%\\DirB", failing_dir_b),
                ("%APPDATA%\\DirA", dir_a),
            ]

            archive_path = None
            try:
                archive_path, total_files, manifest = zip_native_save_dirs_for_upload(dir_map, "TestGame")

                self.assertEqual(total_files, 1)
                self.assertEqual(manifest, {"1": "%APPDATA%\\DirA"})

                with zipfile.ZipFile(archive_path) as archive:
                    members = set(archive.namelist())

                self.assertIn("1/save.dat", members)
                self.assertFalse(any(member.startswith("0/") for member in members))
            finally:
                if archive_path is not None:
                    archive_path.unlink(missing_ok=True)

    def test_zip_native_save_dirs_skips_locked_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            save_dir = Path(temp_dir)
            locked_file = save_dir / "locked.sav"
            good_file = save_dir / "good.sav"
            locked_file.write_text("locked", encoding="utf-8")
            good_file.write_text("good", encoding="utf-8")
            dir_map = [("%APPDATA%\\DirA", save_dir)]

            original_write = zipfile.ZipFile.write

            def _write(self, filename, arcname=None, *args, **kwargs):
                if str(arcname or filename).endswith("locked.sav"):
                    raise PermissionError("locked")
                return original_write(self, filename, arcname, *args, **kwargs)

            archive_path = None
            try:
                with patch("zipfile.ZipFile.write", side_effect=_write, autospec=True):
                    archive_path, total_files, _ = zip_native_save_dirs_for_upload(dir_map, "TestGame")

                self.assertEqual(total_files, 1)

                with zipfile.ZipFile(archive_path) as archive:
                    members = set(archive.namelist())

                self.assertIn("0/good.sav", members)
                self.assertNotIn("0/locked.sav", members)
            finally:
                if archive_path is not None:
                    archive_path.unlink(missing_ok=True)

    def test_zip_native_save_dirs_all_dirs_fail_returns_zero_files_and_empty_manifest(self) -> None:
        _ConcreteBase = type(Path())

        class _FailingPath(_ConcreteBase):
            def rglob(self, pattern):
                raise PermissionError("access denied")

        with tempfile.TemporaryDirectory() as temp_dir:
            failing_path = _FailingPath(temp_dir)
            dir_map = [("%APPDATA%\\DirA", failing_path)]

            archive_path = None
            try:
                archive_path, total_files, manifest = zip_native_save_dirs_for_upload(dir_map, "TestGame")

                self.assertEqual(total_files, 0)
                self.assertEqual(manifest, {})

                with zipfile.ZipFile(archive_path) as archive:
                    self.assertIn("_rom_mate_dirs.json", archive.namelist())
                    parsed_manifest = json.loads(archive.read("_rom_mate_dirs.json").decode("utf-8"))

                self.assertEqual(parsed_manifest, {})
            finally:
                if archive_path is not None:
                    archive_path.unlink(missing_ok=True)

    def test_ppsspp_state_upload_jobs_uses_supported_image_sidecars_only(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_dir = Path(temp_dir) / "states"
            state_dir.mkdir()
            state_file = state_dir / "ULUS12345_1.ppst"
            state_file.write_text("state", encoding="utf-8")
            screenshot_file = state_dir / "ULUS12345_1.png"
            screenshot_file.write_bytes(b"\x89PNG\r\n\x1a\n")
            (state_dir / "Thumbs.db").write_bytes(b"not-an-image")

            jobs = ppsspp_state_upload_jobs(["ULUS12345"], [state_dir], "stateFile")

        self.assertEqual(len(jobs), 1)
        display_name, files = jobs[0]
        self.assertEqual(display_name, "ULUS12345_1.ppst")
        self.assertEqual(files["stateFile"].name, "ULUS12345_1.ppst")
        self.assertEqual(files["screenshotFile"].name, screenshot_file.name)

    def test_appended_image_sidecar_path_finds_png_appended_to_full_filename(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_path = Path(temp_dir) / "game.state1"
            screenshot_path = Path(temp_dir) / "game.state1.png"
            state_path.write_text("", encoding="utf-8")
            screenshot_path.write_bytes(b"")

            found = appended_image_sidecar_path(state_path)

        self.assertEqual(found, screenshot_path)

        with tempfile.TemporaryDirectory() as temp_dir:
            state_path = Path(temp_dir) / "game.state1"
            replaced_extension_screenshot = Path(temp_dir) / "game.png"
            state_path.write_text("", encoding="utf-8")
            replaced_extension_screenshot.write_bytes(b"")

            found = appended_image_sidecar_path(state_path)

        self.assertIsNone(found)

    def test_screenshot_download_candidate_paths_returns_ordered_candidates(self) -> None:
        candidates = screenshot_download_candidate_paths({
            "download_path": "a/b.png",
            "file_path": "c/d.png",
            "full_path": "e/f.png",
        })

        self.assertEqual(candidates, ["a/b.png", "c/d.png", "e/f.png"])

    def test_screenshot_download_candidate_paths_skips_blank_and_missing_keys(self) -> None:
        candidates = screenshot_download_candidate_paths({
            "download_path": "",
            "full_path": "x/y.png",
        })

        self.assertEqual(candidates, ["x/y.png"])

    def test_screenshot_download_candidate_paths_returns_empty_for_empty_record(self) -> None:
        candidates = screenshot_download_candidate_paths({})

        self.assertEqual(candidates, [])

    def test_retroarch_state_upload_jobs_attaches_appended_png_sidecar(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_file = Path(temp_dir) / "Chrono Trigger.state1"
            screenshot_file = Path(temp_dir) / "Chrono Trigger.state1.png"
            state_file.write_text("state", encoding="utf-8")
            screenshot_file.write_bytes(b"")

            jobs, temporary_archives = retroarch_state_upload_jobs([state_file], "stateFile")

        self.assertEqual(temporary_archives, [])
        self.assertEqual(len(jobs), 1)
        _, files = jobs[0]
        self.assertEqual(files["stateFile"], state_file)
        self.assertEqual(files["screenshotFile"], screenshot_file)

    def test_retroarch_state_upload_jobs_omits_screenshotfile_when_no_sidecar(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_file = Path(temp_dir) / "game.state"
            state_file.write_text("state", encoding="utf-8")

            jobs, temporary_archives = retroarch_state_upload_jobs([state_file], "stateFile")

        self.assertEqual(temporary_archives, [])
        self.assertEqual(len(jobs), 1)
        _, files = jobs[0]
        self.assertIn("stateFile", files)
        self.assertNotIn("screenshotFile", files)

    def test_retroarch_state_upload_jobs_separate_jobs_per_slot(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state1_file = Path(temp_dir) / "game.state1"
            state1_screenshot = Path(temp_dir) / "game.state1.png"
            state2_file = Path(temp_dir) / "game.state2"
            state2_screenshot = Path(temp_dir) / "game.state2.png"
            state1_file.write_text("state1", encoding="utf-8")
            state1_screenshot.write_bytes(b"")
            state2_file.write_text("state2", encoding="utf-8")
            state2_screenshot.write_bytes(b"")

            jobs, temporary_archives = retroarch_state_upload_jobs([state1_file, state2_file], "stateFile")

        self.assertEqual(temporary_archives, [])
        self.assertEqual(len(jobs), 2)
        self.assertEqual(jobs[0][1]["stateFile"], state1_file)
        self.assertEqual(jobs[0][1]["screenshotFile"], state1_screenshot)
        self.assertEqual(jobs[1][1]["stateFile"], state2_file)
        self.assertEqual(jobs[1][1]["screenshotFile"], state2_screenshot)

    def test_retroarch_state_upload_jobs_ignores_non_image_sidecar(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_file = Path(temp_dir) / "game.state"
            non_image_sidecar = Path(temp_dir) / "game.state.txt"
            state_file.write_text("state", encoding="utf-8")
            non_image_sidecar.write_text("metadata", encoding="utf-8")

            jobs, temporary_archives = retroarch_state_upload_jobs([state_file], "stateFile")

        self.assertEqual(temporary_archives, [])
        self.assertEqual(len(jobs), 1)
        _, files = jobs[0]
        self.assertNotIn("screenshotFile", files)

    def test_retroarch_state_upload_jobs_screenshot_in_files_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_file = Path(temp_dir) / "game.state1"
            screenshot_file = Path(temp_dir) / "game.state1.png"
            state_file.write_text("state", encoding="utf-8")
            screenshot_file.write_bytes(b"")

            jobs, temporary_archives = retroarch_state_upload_jobs([state_file], "stateFile")

        self.assertEqual(temporary_archives, [])
        self.assertEqual(len(jobs), 1)
        _, files = jobs[0]
        self.assertEqual(files["screenshotFile"], screenshot_file)

    def test_retroarch_state_upload_jobs_returns_two_tuple_per_job(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_file = Path(temp_dir) / "game.state1"
            state_file.write_text("state", encoding="utf-8")

            jobs, temporary_archives = retroarch_state_upload_jobs([state_file], "stateFile")

        self.assertEqual(temporary_archives, [])
        self.assertEqual(len(jobs), 1)
        self.assertEqual(len(jobs[0]), 2)

    def test_grouped_file_upload_jobs_archives_multiple_files_into_one_upload(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            save_dir = Path(temp_dir) / "saves"
            save_dir.mkdir()
            save_file = save_dir / "Chrono Trigger.srm"
            rtc_file = save_dir / "Chrono Trigger.rtc"
            save_file.write_text("save", encoding="utf-8")
            rtc_file.write_text("rtc", encoding="utf-8")

            jobs, temporary_archives = grouped_file_upload_jobs(
                [save_file, rtc_file],
                "saveFile",
                lambda files: zip_selected_files_for_upload(files, "Chrono Trigger"),
            )

            self.assertEqual(len(jobs), 1)
            display_name, payload = jobs[0]
            self.assertIn("Chrono Trigger", display_name)
            archive_path = payload["saveFile"]
            self.assertEqual(archive_path.suffix, ".zip")
            with zipfile.ZipFile(archive_path) as archive:
                members = set(archive.namelist())
            self.assertEqual(members, {"Chrono Trigger.rtc", "Chrono Trigger.srm"})

            for archive_path in temporary_archives:
                archive_path.unlink(missing_ok=True)

    def test_grouped_file_upload_jobs_keeps_distinct_state_slots_separate(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_dir = Path(temp_dir) / "states"
            state_dir.mkdir()
            slot_file = state_dir / "Chrono Trigger.state1"
            auto_file = state_dir / "Chrono Trigger.state.auto"
            slot_file.write_text("slot", encoding="utf-8")
            auto_file.write_text("auto", encoding="utf-8")

            jobs, temporary_archives = grouped_file_upload_jobs(
                [slot_file, auto_file],
                "stateFile",
                lambda files: zip_selected_files_for_upload(files, "Chrono Trigger"),
            )

            self.assertEqual(len(jobs), 2)
            self.assertEqual(temporary_archives, [])
            self.assertCountEqual(
                [payload["stateFile"].name for _, payload in jobs],
                ["Chrono Trigger.state1", "Chrono Trigger.state.auto"],
            )

    def test_cloud_sync_candidates_for_save_accepts_explicit_file_paths_from_profiles(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            memory_card = Path(temp_dir) / "Card A.raw"
            memory_card.write_text("memory-card", encoding="utf-8")

            game = {
                "title": "F-Zero GX",
                "rom_file_name": "F-Zero GX.iso",
            }

            candidates = cloud_sync_candidates_for_game(
                game,
                [memory_card],
                "save",
                lambda item: {"f-zero gx", "fzerogx"},
                lambda file_path: ".state" in file_path.name.casefold(),
            )

        self.assertEqual(candidates, [memory_card])

    def test_cemu_save_directories_for_game_selects_nested_user_folders(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            save_root = Path(temp_dir) / "mlc01" / "usr" / "save"
            persistent_dir = save_root / "00050000" / "1010ED00" / "user" / "80000001"
            common_dir = save_root / "00050000" / "1010ED00" / "user" / "common"
            unrelated_dir = save_root / "00050000" / "1010EE00" / "user" / "80000001"

            persistent_dir.mkdir(parents=True)
            common_dir.mkdir(parents=True)
            unrelated_dir.mkdir(parents=True)

            (persistent_dir / "progress.dat").write_text("save-data", encoding="utf-8")
            (common_dir / "settings.dat").write_text("common-data", encoding="utf-8")
            (unrelated_dir / "other.dat").write_text("other-game", encoding="utf-8")

            game = {
                "title": "The Legend of Zelda: Breath of the Wild",
                "title_id": "000500001010ED00",
            }

            candidates = cemu_save_directories_for_game(
                game,
                [save_root],
                lambda item: {"000500001010ED00", "00050000", "1010ED00"},
                lambda root, **kwargs: max(
                    (path.stat().st_mtime for path in root.rglob("*") if path.is_file()),
                    default=0.0,
                ),
            )

        self.assertCountEqual(candidates, [persistent_dir, common_dir])
        self.assertNotIn(unrelated_dir, candidates)

    def test_cloud_sync_candidates_for_state_filters_to_matching_rom_name(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_dir = Path(temp_dir) / "states"
            state_dir.mkdir()
            matching_state = state_dir / "Sonic The Hedgehog.state1"
            matching_auto_state = state_dir / "Sonic The Hedgehog.state.auto"
            unrelated_state = state_dir / "Streets of Rage.state1"

            matching_state.write_text("slot-state", encoding="utf-8")
            matching_auto_state.write_text("auto-state", encoding="utf-8")
            unrelated_state.write_text("other-game", encoding="utf-8")

            game = {
                "title": "Sonic The Hedgehog",
                "rom_file_name": "Sonic The Hedgehog.zip",
            }

            candidates = cloud_sync_candidates_for_game(
                game,
                [state_dir],
                "state",
                lambda item: {"sonic the hedgehog", "sonicthehedgehog"},
                lambda file_path: ".state" in file_path.name.casefold(),
            )

        self.assertEqual([path.name for path in candidates], [
            "Sonic The Hedgehog.state.auto",
            "Sonic The Hedgehog.state1",
        ])

    def test_cloud_sync_candidates_for_state_allows_common_name_variants_only(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_dir = Path(temp_dir) / "states"
            state_dir.mkdir()
            matching_dash_variant = state_dir / "Sonic-The-Hedgehog.state2"
            matching_compact_variant = state_dir / "SonicTheHedgehog.state.auto"
            unrelated_variant = state_dir / "Sonic Spinball.state1"
            sequel_variant = state_dir / "SonicTheHedgehog2.state3"

            matching_dash_variant.write_text("dash-variant", encoding="utf-8")
            matching_compact_variant.write_text("compact-variant", encoding="utf-8")
            unrelated_variant.write_text("other-title", encoding="utf-8")
            sequel_variant.write_text("sequel-title", encoding="utf-8")

            game = {
                "title": "Sonic The Hedgehog",
                "rom_file_name": "Sonic The Hedgehog.zip",
            }

            candidates = cloud_sync_candidates_for_game(
                game,
                [state_dir],
                "state",
                lambda item: {"sonic the hedgehog", "sonicthehedgehog"},
                lambda file_path: ".state" in file_path.name.casefold(),
            )

        self.assertCountEqual(
            [path.name for path in candidates],
            [
                "SonicTheHedgehog.state.auto",
                "Sonic-The-Hedgehog.state2",
            ],
        )


class TestSessionScreenshotPath(unittest.TestCase):
    def test_returns_none_when_no_directories(self) -> None:
        result = session_screenshot_path([], (0.0, 9999999999.0))
        self.assertIsNone(result)

    def test_returns_none_when_session_window_is_none(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            img = Path(temp_dir) / "shot.png"
            img.write_bytes(b"\x89PNG\r\n\x1a\n")
            result = session_screenshot_path([Path(temp_dir)], None)
        self.assertIsNone(result)

    def test_returns_none_when_no_images_in_window(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            img = Path(temp_dir) / "shot.png"
            img.write_bytes(b"\x89PNG\r\n\x1a\n")
            past_time = 1000.0
            os.utime(img, (past_time, past_time))
            result = session_screenshot_path([Path(temp_dir)], (2000.0, 3000.0))
        self.assertIsNone(result)

    def test_returns_image_within_session_window(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            img = Path(temp_dir) / "shot.png"
            img.write_bytes(b"\x89PNG\r\n\x1a\n")
            window_start = 1000.0
            window_end = 9000.0
            os.utime(img, (5000.0, 5000.0))
            result = session_screenshot_path([Path(temp_dir)], (window_start, window_end))
        self.assertEqual(result, img)

    def test_returns_most_recent_image_when_multiple_within_window(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            earlier = Path(temp_dir) / "earlier.png"
            later = Path(temp_dir) / "later.png"
            earlier.write_bytes(b"\x89PNG\r\n\x1a\n")
            later.write_bytes(b"\x89PNG\r\n\x1a\n")
            os.utime(earlier, (2000.0, 2000.0))
            os.utime(later, (4000.0, 4000.0))
            result = session_screenshot_path([Path(temp_dir)], (1000.0, 9000.0))
        self.assertEqual(result, later)

    def test_ignores_non_image_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            txt = Path(temp_dir) / "notes.txt"
            txt.write_text("not an image", encoding="utf-8")
            os.utime(txt, (5000.0, 5000.0))
            result = session_screenshot_path([Path(temp_dir)], (1000.0, 9000.0))
        self.assertIsNone(result)

    def test_scans_subdirectories_recursively(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            subdir = Path(temp_dir) / "GameID"
            subdir.mkdir()
            img = subdir / "shot.png"
            img.write_bytes(b"\x89PNG\r\n\x1a\n")
            os.utime(img, (5000.0, 5000.0))
            result = session_screenshot_path([Path(temp_dir)], (1000.0, 9000.0))
        self.assertEqual(result, img)

    def test_skips_blocked_basenames(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            img = Path(temp_dir) / "blocked.png"
            img.write_bytes(b"\x89PNG\r\n\x1a\n")
            os.utime(img, (5000.0, 5000.0))
            result = session_screenshot_path(
                [Path(temp_dir)],
                (1000.0, 9000.0),
                blocked_basenames={"blocked.png"},
            )
        self.assertIsNone(result)

    def test_skips_missing_directories_gracefully(self) -> None:
        missing = Path("/nonexistent/screenshot/dir/that/does/not/exist")
        result = session_screenshot_path([missing], (1000.0, 9000.0))
        self.assertIsNone(result)

    def test_supports_jpg_extension(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            img = Path(temp_dir) / "shot.jpg"
            img.write_bytes(b"\xff\xd8\xff")
            os.utime(img, (5000.0, 5000.0))
            result = session_screenshot_path([Path(temp_dir)], (1000.0, 9000.0))
        self.assertEqual(result, img)

    def test_supports_webp_extension(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            img = Path(temp_dir) / "shot.webp"
            img.write_bytes(b"RIFF")
            os.utime(img, (5000.0, 5000.0))
            result = session_screenshot_path([Path(temp_dir)], (1000.0, 9000.0))
        self.assertEqual(result, img)

    def test_supports_bmp_extension(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            img = Path(temp_dir) / "shot.bmp"
            img.write_bytes(b"BM")
            os.utime(img, (5000.0, 5000.0))
            result = session_screenshot_path([Path(temp_dir)], (1000.0, 9000.0))
        self.assertEqual(result, img)


if __name__ == "__main__":
    unittest.main()
