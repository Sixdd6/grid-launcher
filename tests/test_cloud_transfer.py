from __future__ import annotations

import tempfile
import unittest
import zipfile
from pathlib import Path

from rom_mate.library.cloud_sync import cemu_save_directories_for_game, cloud_sync_candidates_for_game
from rom_mate.library.cloud_transfer import (
    grouped_file_upload_jobs,
    ppsspp_state_upload_jobs,
    zip_directory_for_upload,
    zip_selected_files_for_upload,
)


class CloudTransferTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
