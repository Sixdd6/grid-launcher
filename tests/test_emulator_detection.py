from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from rom_mate.emulator.detection import detect_linux_emulators


class EmulatorDetectionTests(unittest.TestCase):
    @patch("rom_mate.emulator.detection.sys.platform", "win32")
    def test_returns_empty_on_windows(self) -> None:
        self.assertEqual(detect_linux_emulators(), [])

    @patch("rom_mate.emulator.detection.sys.platform", "linux")
    @patch("rom_mate.emulator.detection.Path.is_file", return_value=False)
    @patch("rom_mate.emulator.detection.Path.exists", return_value=False)
    @patch("rom_mate.emulator.detection.subprocess.run")
    def test_detects_flatpak_by_subprocess(
        self,
        mock_run: MagicMock,
        _mock_exists: MagicMock,
        _mock_is_file: MagicMock,
    ) -> None:
        mock_run.return_value = MagicMock(stdout="net.pcsx2.PCSX2\n", returncode=0)

        result = detect_linux_emulators()

        self.assertEqual(len(result), 1)
        entry = result[0]
        self.assertEqual(entry.get("slug"), "pcsx2")
        self.assertEqual(entry.get("path"), "/usr/bin/flatpak")
        self.assertEqual(entry.get("args"), "run net.pcsx2.PCSX2 %rom%")
        self.assertEqual(entry.get("autodetected"), "true")

    @patch("rom_mate.emulator.detection.sys.platform", "linux")
    @patch("rom_mate.emulator.detection.subprocess.run", side_effect=OSError("flatpak unavailable"))
    @patch("rom_mate.emulator.detection.Path.is_file", return_value=True)
    @patch("rom_mate.emulator.detection.Path.exists", return_value=True)
    def test_detects_system_binary(
        self,
        _mock_exists: MagicMock,
        _mock_is_file: MagicMock,
        _mock_run: MagicMock,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_binary = Path(temp_dir) / "retroarch"
            temp_binary.write_text("#!/bin/sh\n", encoding="utf-8")

            result = detect_linux_emulators()

        retroarch_entries = [entry for entry in result if entry.get("slug") == "retroarch"]
        self.assertEqual(len(retroarch_entries), 1)
        self.assertEqual(retroarch_entries[0].get("path"), "/usr/bin/retroarch")
        self.assertEqual(retroarch_entries[0].get("args"), "%rom%")

    @patch("rom_mate.emulator.detection.sys.platform", "linux")
    @patch(
        "rom_mate.emulator.detection.subprocess.run",
        side_effect=subprocess.TimeoutExpired("flatpak", 5),
    )
    @patch("rom_mate.emulator.detection.Path.is_file", return_value=True)
    @patch("rom_mate.emulator.detection.Path.exists", return_value=True)
    def test_subprocess_failure_returns_system_results(
        self,
        _mock_exists: MagicMock,
        _mock_is_file: MagicMock,
        _mock_run: MagicMock,
    ) -> None:
        result = detect_linux_emulators()

        dolphin_entries = [entry for entry in result if entry.get("slug") == "dolphin"]
        self.assertEqual(len(dolphin_entries), 1)

    @patch("rom_mate.emulator.detection.sys.platform", "linux")
    @patch("rom_mate.emulator.detection.Path.is_file", return_value=True)
    @patch("rom_mate.emulator.detection.Path.exists", return_value=True)
    @patch("rom_mate.emulator.detection.subprocess.run")
    def test_dedup_prefers_system_binary(
        self,
        mock_run: MagicMock,
        _mock_exists: MagicMock,
        _mock_is_file: MagicMock,
    ) -> None:
        mock_run.return_value = MagicMock(stdout="net.pcsx2.PCSX2\n", returncode=0)

        result = detect_linux_emulators()

        pcsx2_entries = [entry for entry in result if entry.get("slug") == "pcsx2"]
        self.assertEqual(len(pcsx2_entries), 1)
        self.assertEqual(pcsx2_entries[0].get("path"), "/usr/bin/pcsx2-qt")

    @patch("rom_mate.emulator.detection.sys.platform", "linux")
    @patch("rom_mate.emulator.detection.Path.is_file", return_value=True)
    @patch("rom_mate.emulator.detection.Path.exists", return_value=True)
    @patch("rom_mate.emulator.detection.subprocess.run")
    def test_all_entries_have_autodetected_true(
        self,
        mock_run: MagicMock,
        _mock_exists: MagicMock,
        _mock_is_file: MagicMock,
    ) -> None:
        mock_run.return_value = MagicMock(
            stdout="net.pcsx2.PCSX2\norg.ppsspp.PPSSPP\n",
            returncode=0,
        )

        result = detect_linux_emulators()

        self.assertTrue(result)
        self.assertTrue(all(entry.get("autodetected") == "true" for entry in result))


if __name__ == "__main__":
    unittest.main()
