from __future__ import annotations

import io
import tempfile
import unittest
import zipfile
from pathlib import Path

from rom_mate.library.cloud_restore import (
    relative_timestamp_text,
    restore_single_save_payload,
    restore_single_state_payload,
    sort_server_records_by_recency,
)


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
