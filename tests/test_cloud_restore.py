from __future__ import annotations

import io
import tempfile
import unittest
import zipfile
from pathlib import Path

from grid_launcher.library.cloud_restore import (
    latest_server_records_by_slot,
    relative_timestamp_text,
    restore_single_save_payload,
    restore_single_state_payload,
    save_record_timestamp,
    sort_server_records_by_recency,
)
from grid_launcher.library.cloud_sync import cloud_sync_candidates_for_game


class CloudRestoreTests(unittest.TestCase):
    def test_relative_timestamp_text_uses_human_readable_ranges(self) -> None:
        self.assertEqual(relative_timestamp_text(0, now=1_000), "Unknown")
        self.assertEqual(relative_timestamp_text(995, now=1_000), "just now")
        self.assertEqual(relative_timestamp_text(940, now=1_000), "1 minute ago")
        self.assertEqual(relative_timestamp_text(1_000 - (3 * 3_600), now=1_000), "3 hours ago")
        self.assertEqual(relative_timestamp_text(1_000 - (2 * 86_400), now=1_000), "2 days ago")

    def test_sort_server_records_by_recency_prefers_newest_timestamp_then_id(self) -> None:
        records = [
            {"id": "4", "updated_at": "2026-04-08T10:00:00Z"},
            {"id": "9", "updated_at": "2026-04-08T10:00:00Z"},
            {"id": "2", "updated_at": "2026-04-07T09:00:00Z"},
        ]

        ordered = sort_server_records_by_recency(records, timestamp_fn=lambda item: 1 if item["id"] == "2" else 2)

        self.assertEqual([item["id"] for item in ordered], ["9", "4", "2"])

    def test_latest_server_records_by_slot_keeps_newest_entry_per_slot(self) -> None:
        records = [
            {"id": "1", "emulator": "Redream", "slot": "vmu0", "updated_at": "2026-04-08T09:00:00Z"},
            {"id": "2", "emulator": "Redream", "slot": "vmu0", "updated_at": "2026-04-08T10:00:00Z"},
            {"id": "3", "emulator": "Redream", "slot": "vmu1", "updated_at": "2026-04-08T08:30:00Z"},
            {"id": "4", "emulator": "Other", "slot": "vmu0", "updated_at": "2026-04-08T11:00:00Z"},
        ]

        grouped = latest_server_records_by_slot(records, "Redream", save_record_timestamp)

        self.assertEqual([item["id"] for item in grouped], ["2", "3"])

    def test_restore_single_save_payload_prefers_exact_candidate_filename(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            save_root = Path(temp_dir) / "saves"
            core_dir = save_root / "Snes9x"
            core_dir.mkdir(parents=True)

            rtc_path = core_dir / "Chrono Trigger.rtc"
            rtc_path.write_bytes(b"rtc-old")
            srm_path = core_dir / "Chrono Trigger.srm"
            srm_path.write_bytes(b"srm-old")

            restored = restore_single_save_payload(
                [save_root],
                {"file_name": "Chrono Trigger.srm"},
                b"srm-new",
                [rtc_path, srm_path],
                "Chrono Trigger.srm",
            )

            self.assertEqual(restored, srm_path)
            self.assertEqual(srm_path.read_bytes(), b"srm-new")
            self.assertEqual(rtc_path.read_bytes(), b"rtc-old")

    def test_cloud_sync_candidates_for_game_matches_redream_numeric_savestate_names(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_root = Path(temp_dir) / "states"
            state_root.mkdir(parents=True)
            state_file = state_root / "Sonic Adventure.0.sav"
            state_file.write_bytes(b"state")

            candidates = cloud_sync_candidates_for_game(
                {"title": "Sonic Adventure", "platform": "Dreamcast", "rom_file_name": "Sonic Adventure.chd"},
                [state_root],
                "state",
                lambda game: {"sonicadventure", "sonic", "adventure"},
                lambda path: path.name.casefold().endswith(".0.sav"),
            )

            self.assertEqual(candidates, [state_file])

    def test_cloud_sync_candidates_for_game_falls_back_to_latest_redream_hash_group(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_root = Path(temp_dir) / "states"
            state_root.mkdir(parents=True)

            older_state = state_root / "A1B2C3D4.0.sav"
            older_state.write_bytes(b"older")
            latest_state = state_root / "D4A53E48.0.sav"
            latest_state.write_bytes(b"latest")
            companion_state = state_root / "D4A53E48.1.sav"
            companion_state.write_bytes(b"slot-1")

            older_mtime = 1_000_000
            latest_mtime = older_mtime + 60
            older_state.touch()
            latest_state.touch()
            companion_state.touch()
            import os
            os.utime(older_state, (older_mtime, older_mtime))
            os.utime(latest_state, (latest_mtime, latest_mtime))
            os.utime(companion_state, (latest_mtime, latest_mtime))

            candidates = cloud_sync_candidates_for_game(
                {"title": "Jet Grind Radio", "platform": "Dreamcast", "rom_file_name": "Jet Grind Radio (USA).chd"},
                [state_root],
                "state",
                lambda game: {"jetgrindradio", "jet", "grind", "radio"},
                lambda path: path.name.casefold().endswith(".sav"),
            )

            self.assertEqual(candidates, [latest_state, companion_state])

    def test_restore_single_state_payload_keeps_nested_candidate_folder(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_root = Path(temp_dir) / "states"
            core_dir = state_root / "Snes9x"
            core_dir.mkdir(parents=True)

            existing_state = core_dir / "Chrono Trigger.state1"
            existing_state.write_bytes(b"slot-1")

            restored = restore_single_state_payload(
                [state_root],
                {"file_name": "Chrono Trigger.state.auto"},
                b"auto-state",
                [existing_state],
                "Chrono Trigger.state",
            )

            expected_path = core_dir / "Chrono Trigger.state.auto"
            self.assertEqual(restored, expected_path)
            self.assertEqual(expected_path.read_bytes(), b"auto-state")

    def test_restore_single_state_payload_writes_screenshot_sidecar(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_root = Path(temp_dir) / "states"
            core_dir = state_root / "Snes9x"
            core_dir.mkdir(parents=True)
            existing_state = core_dir / "Chrono Trigger.state1"
            existing_state.write_bytes(b"slot-1")

            restored = restore_single_state_payload(
                [state_root],
                {"file_name": "Chrono Trigger.state.auto"},
                b"auto-state",
                [existing_state],
                "Chrono Trigger.state",
                screenshot_bytes=b"\x89PNG\r\n\x1a\n",
                screenshot_extension=".png",
            )

            expected_path = core_dir / "Chrono Trigger.state.auto"
            sidecar_path = Path(str(expected_path) + ".png")
            self.assertEqual(restored, expected_path)
            self.assertTrue(expected_path.exists())
            self.assertTrue(sidecar_path.exists())
            self.assertEqual(sidecar_path.read_bytes(), b"\x89PNG\r\n\x1a\n")

    def test_restore_single_state_payload_omits_sidecar_when_no_screenshot(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_root = Path(temp_dir) / "states"
            core_dir = state_root / "Snes9x"
            core_dir.mkdir(parents=True)
            existing_state = core_dir / "Chrono Trigger.state1"
            existing_state.write_bytes(b"slot-1")

            restored = restore_single_state_payload(
                [state_root],
                {"file_name": "Chrono Trigger.state.auto"},
                b"auto-state",
                [existing_state],
                "Chrono Trigger.state",
                screenshot_bytes=None,
            )

            expected_path = core_dir / "Chrono Trigger.state.auto"
            sidecar_path = Path(str(expected_path) + ".png")
            self.assertEqual(restored, expected_path)
            self.assertTrue(expected_path.exists())
            self.assertFalse(sidecar_path.exists())

    def test_restore_single_state_payload_uses_custom_screenshot_extension(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_root = Path(temp_dir) / "states"
            core_dir = state_root / "Snes9x"
            core_dir.mkdir(parents=True)
            existing_state = core_dir / "Chrono Trigger.state1"
            existing_state.write_bytes(b"slot-1")

            restored = restore_single_state_payload(
                [state_root],
                {"file_name": "Chrono Trigger.state.auto"},
                b"auto-state",
                [existing_state],
                "Chrono Trigger.state",
                screenshot_bytes=b"fake",
                screenshot_extension=".jpg",
            )

            expected_path = core_dir / "Chrono Trigger.state.auto"
            jpg_sidecar_path = Path(str(expected_path) + ".jpg")
            png_sidecar_path = Path(str(expected_path) + ".png")
            self.assertEqual(restored, expected_path)
            self.assertTrue(jpg_sidecar_path.exists())
            self.assertFalse(png_sidecar_path.exists())

    def test_restore_single_state_payload_no_sidecar_for_zip_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_root = Path(temp_dir) / "states"
            core_dir = state_root / "Snes9x"
            core_dir.mkdir(parents=True)
            existing_state = core_dir / "Chrono Trigger.state1"
            existing_state.write_bytes(b"slot-1")

            payload_stream = io.BytesIO()
            with zipfile.ZipFile(payload_stream, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
                archive.writestr("Chrono Trigger.state2", b"new-slot")

            restored = restore_single_state_payload(
                [state_root],
                {"file_name": "Chrono Trigger.state.zip"},
                payload_stream.getvalue(),
                [existing_state],
                "Chrono Trigger.state",
                screenshot_bytes=b"\x89PNG\r\n\x1a\n",
            )

            expected_extracted_path = core_dir / "Chrono Trigger.state2"
            sidecar_path = Path(str(core_dir / "Chrono Trigger.state.zip") + ".png")
            self.assertEqual(restored, core_dir)
            self.assertTrue(expected_extracted_path.exists())
            self.assertFalse(sidecar_path.exists())

    def test_restore_single_state_payload_unpacks_zip_archive_into_matching_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            state_root = Path(temp_dir) / "states"
            core_dir = state_root / "Snes9x"
            core_dir.mkdir(parents=True)

            existing_slot = core_dir / "Chrono Trigger.state1"
            existing_auto = core_dir / "Chrono Trigger.state.auto"
            existing_slot.write_bytes(b"old-slot")
            existing_auto.write_bytes(b"old-auto")

            payload_stream = io.BytesIO()
            with zipfile.ZipFile(payload_stream, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
                archive.writestr("Chrono Trigger.state1", b"new-slot")
                archive.writestr("Chrono Trigger.state.auto", b"new-auto")

            restored = restore_single_state_payload(
                [state_root],
                {"file_name": "Chrono Trigger.state.zip"},
                payload_stream.getvalue(),
                [existing_slot, existing_auto],
                "Chrono Trigger.state",
            )

            self.assertEqual(restored, core_dir)
            self.assertEqual(existing_slot.read_bytes(), b"new-slot")
            self.assertEqual(existing_auto.read_bytes(), b"new-auto")


if __name__ == "__main__":
    unittest.main()
