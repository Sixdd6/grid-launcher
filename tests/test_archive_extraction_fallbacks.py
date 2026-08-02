from __future__ import annotations

import os
import unittest
from pathlib import Path
from unittest.mock import patch

import tempfile

import grid_launcher.library.archive_preparation as archive_preparation
from grid_launcher.library.archive_preparation import (
    _extract_7z_with_fallbacks,
    _run_extractor_process,
)


class RunExtractorProcessEnvTests(unittest.TestCase):
    def test_passes_cleaned_environment_to_subprocess(self) -> None:
        captured: dict[str, object] = {}

        class _Result:
            returncode = 0
            stderr = ""

        def fake_run(command, **kwargs):
            captured.update(kwargs)
            return _Result()

        env = {
            "LD_LIBRARY_PATH": "/bundle/_internal",
            "LD_LIBRARY_PATH_ORIG": "/usr/lib64",
        }
        with patch.dict(os.environ, env, clear=False):
            with patch("grid_launcher.library.archive_preparation.subprocess.run", fake_run):
                _run_extractor_process(["7z", "x", "archive.7z"], failure_message="failed")

        passed_env = captured.get("env")
        self.assertIsInstance(passed_env, dict)
        self.assertEqual(passed_env.get("LD_LIBRARY_PATH"), "/usr/lib64")


@unittest.skipIf(os.name == "nt", "fallback chain behaves differently on Windows")
class ExtractFallbackChainTests(unittest.TestCase):
    def _run_failing_chain(self, extracted_dir: Path, archive: Path) -> OSError:
        missing_bundled = extracted_dir / "no-such-7z.exe"
        with patch.object(archive_preparation, "_BUNDLED_7Z_PATH", missing_bundled):
            with patch("grid_launcher.library.archive_preparation.shutil.which", return_value=None):
                with patch.object(archive_preparation, "_KNOWN_7Z_PATHS", []):
                    with patch(
                        "grid_launcher.library.archive_preparation._try_py7zr",
                        return_value="BCJ2 filter is not supported by py7zr",
                    ):
                        with self.assertRaises(OSError) as raised:
                            _extract_7z_with_fallbacks(archive, extracted_dir)
        return raised.exception

    def test_error_reports_real_python_fallback_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            extracted_dir = Path(temp_dir) / "game"
            extracted_dir.mkdir()
            archive = Path(temp_dir) / "game.7z"
            archive.write_bytes(b"not-a-real-archive")
            error = self._run_failing_chain(extracted_dir, archive)
        self.assertIn("BCJ2 filter is not supported by py7zr", str(error))

    def test_dead_end_does_not_wipe_extracted_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            extracted_dir = Path(temp_dir) / "game"
            extracted_dir.mkdir()
            partial_file = extracted_dir / "data.pak"
            partial_file.write_text("partial-extraction")
            archive = Path(temp_dir) / "game.7z"
            archive.write_bytes(b"not-a-real-archive")
            self._run_failing_chain(extracted_dir, archive)
            self.assertTrue(partial_file.exists())

    def test_error_reports_system_7z_failure_instead_of_not_found(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            extracted_dir = Path(temp_dir) / "game"
            extracted_dir.mkdir()
            archive = Path(temp_dir) / "game.7z"
            archive.write_bytes(b"not-a-real-archive")
            missing_bundled = extracted_dir / "no-such-7z.exe"

            def fake_run_extractor(command, *, failure_message):
                raise OSError("version `CXXABI_1.3.15' not found")

            with patch.object(archive_preparation, "_BUNDLED_7Z_PATH", missing_bundled):
                with patch(
                    "grid_launcher.library.archive_preparation.shutil.which",
                    side_effect=lambda cmd: "/usr/bin/7z" if cmd == "7z" else None,
                ):
                    with patch.object(archive_preparation, "_KNOWN_7Z_PATHS", []):
                        with patch(
                            "grid_launcher.library.archive_preparation._run_extractor_process",
                            fake_run_extractor,
                        ):
                            with patch(
                                "grid_launcher.library.archive_preparation._try_py7zr",
                                return_value="__py7zr_unavailable__",
                            ):
                                with self.assertRaises(OSError) as raised:
                                    _extract_7z_with_fallbacks(archive, extracted_dir)

        message = str(raised.exception)
        self.assertIn("CXXABI_1.3.15", message)
        self.assertNotIn("7-Zip was not found", message)


if __name__ == "__main__":
    unittest.main()
