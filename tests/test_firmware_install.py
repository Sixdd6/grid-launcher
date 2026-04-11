from __future__ import annotations

import io
import shutil
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import Mock, patch

from rom_mate.library.firmware_install import (
    download_firmware_bytes,
    fetch_platform_firmware,
    install_platform_firmware,
    resolve_firmware_targets,
)
from rom_mate.emulator.retroarch import retroarch_core_firmware_metadata


class FirmwareRoutingTests(unittest.TestCase):
    def test_resolve_plain_path_accepts_all(self) -> None:
        targets = [Path("/bios")]

        result = resolve_firmware_targets("anything.bin", targets)

        self.assertEqual(result, [Path("/bios")])

    def test_resolve_tuple_match_hit(self) -> None:
        targets = [(Path("/gc/usa"), ["ntsc"])]

        result = resolve_firmware_targets("gc_ntsc.zip", targets)

        self.assertEqual(result, [Path("/gc/usa")])

    def test_resolve_tuple_match_miss(self) -> None:
        targets = [(Path("/gc/usa"), ["pal"])]

        result = resolve_firmware_targets("gc_ntsc.zip", targets)

        self.assertEqual(result, [])

    def test_resolve_first_match_wins(self) -> None:
        targets = [
            (Path("/gc/jap"), ["ntsc_j", "jap"]),
            (Path("/gc/usa"), ["ntsc"]),
        ]

        result = resolve_firmware_targets("gc_ntsc_j.zip", targets)

        self.assertEqual(result, [Path("/gc/jap")])

    def test_resolve_case_insensitive(self) -> None:
        targets = [(Path("/gc/usa"), ["ntsc"])]

        result = resolve_firmware_targets("GC_NTSC.ZIP", targets)

        self.assertEqual(result, [Path("/gc/usa")])

    def test_resolve_no_routed_match_returns_empty(self) -> None:
        targets = [
            (Path("/gc/jap"), ["ntsc_j", "jap"]),
            (Path("/gc/usa"), ["ntsc", "usa"]),
        ]

        result = resolve_firmware_targets("gc_unknown.zip", targets)

        self.assertEqual(result, [])

    def test_resolve_mixed_plain_and_routed(self) -> None:
        targets = [Path("/shared"), (Path("/gc/usa"), ["ntsc"])]

        result_hit = resolve_firmware_targets("gc_ntsc.zip", targets)
        result_miss = resolve_firmware_targets("gc_pal.zip", targets)

        self.assertEqual(result_hit, [Path("/shared"), Path("/gc/usa")])
        self.assertEqual(result_miss, [Path("/shared")])


class FirmwareInstallTests(unittest.TestCase):
    def test_no_target_dirs_returns_no_warnings(self) -> None:
        mock_json_fn = Mock()
        mock_bytes_fn = Mock()

        result = install_platform_firmware(mock_json_fn, mock_bytes_fn, 19, [])

        self.assertEqual(result, [])
        mock_json_fn.assert_not_called()

    def test_server_returns_empty_list_no_warnings(self) -> None:
        mock_json_fn = Mock(return_value=[])
        mock_bytes_fn = Mock()

        with tempfile.TemporaryDirectory() as temp_dir:
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [Path(temp_dir)],
            )

        self.assertEqual(result, [])
        mock_bytes_fn.assert_not_called()

    def test_single_firmware_downloaded_and_written(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock(return_value=b"FIRMWAREDATA")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
            )
            dest_path = target_dir / "gc-ntsc-12-101.bin"

            self.assertEqual(result, [])
            self.assertTrue(dest_path.exists())
            self.assertEqual(dest_path.read_bytes(), b"FIRMWAREDATA")

    def test_skip_existing_firmware_when_file_present(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock(return_value=b"NEWDATA")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            dest_path = target_dir / "gc-ntsc-12-101.bin"
            dest_path.write_bytes(b"ORIGINAL")

            install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
                skip_existing=True,
            )

            mock_bytes_fn.assert_called_once()
            self.assertEqual(dest_path.read_bytes(), b"ORIGINAL")

    def test_zip_archive_extracted_to_target_dir(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc_ntsc.zip"}])
        zip_buffer = io.BytesIO()
        with zipfile.ZipFile(zip_buffer, "w") as zf:
            zf.writestr("IPL.bin", b"IPLDATA")
        mock_bytes_fn = Mock(return_value=zip_buffer.getvalue())

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
            )

            self.assertEqual(result, [])
            self.assertTrue((target_dir / "IPL.bin").exists())
            self.assertEqual((target_dir / "IPL.bin").read_bytes(), b"IPLDATA")

    def test_zip_archive_skips_existing_extracted_file(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc_ntsc.zip"}])
        zip_buffer = io.BytesIO()
        with zipfile.ZipFile(zip_buffer, "w") as zf:
            zf.writestr("IPL.bin", b"IPLDATA")
        mock_bytes_fn = Mock(return_value=zip_buffer.getvalue())

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            existing_file = target_dir / "IPL.bin"
            existing_file.write_bytes(b"ORIGINAL")

            install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
                skip_existing=True,
            )

            mock_bytes_fn.assert_called_once()
            self.assertEqual(existing_file.read_bytes(), b"ORIGINAL")

    def test_zip_archive_skips_macosx_metadata(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc_ntsc.zip"}])
        zip_buffer = io.BytesIO()
        with zipfile.ZipFile(zip_buffer, "w") as zf:
            zf.writestr("__MACOSX/._IPL.bin", b"META")
            zf.writestr("IPL.bin", b"IPLDATA")
        mock_bytes_fn = Mock(return_value=zip_buffer.getvalue())

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
            )

            self.assertEqual(result, [])
            self.assertTrue((target_dir / "IPL.bin").exists())
            self.assertFalse((target_dir / "._IPL.bin").exists())

    def test_7z_archive_calls_extract_archive_into_directory(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc_ntsc.7z"}])
        mock_bytes_fn = Mock(return_value=b"fakearchivecontent")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            with patch("rom_mate.library.archive_preparation.extract_archive_into_directory") as mock_extract:
                result = install_platform_firmware(
                    mock_json_fn,
                    mock_bytes_fn,
                    19,
                    [target_dir],
                )

            self.assertEqual(result, [])
            mock_extract.assert_called_once()
            call_args = mock_extract.call_args[0]
            self.assertEqual(call_args[0].suffix, ".7z")
            self.assertNotEqual(call_args[1], target_dir)

    def test_rar_archive_calls_extract_archive_into_directory(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc_ntsc.rar"}])
        mock_bytes_fn = Mock(return_value=b"fakearchivecontent")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            with patch("rom_mate.library.archive_preparation.extract_archive_into_directory") as mock_extract:
                result = install_platform_firmware(
                    mock_json_fn,
                    mock_bytes_fn,
                    19,
                    [target_dir],
                )

            self.assertEqual(result, [])
            mock_extract.assert_called_once()
            call_args = mock_extract.call_args[0]
            self.assertEqual(call_args[0].suffix, ".rar")
            self.assertNotEqual(call_args[1], target_dir)

    def test_non_zip_file_written_directly(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock(return_value=b"RAWDATA")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
            )

            self.assertEqual(result, [])
            self.assertEqual((target_dir / "gc-ntsc-12-101.bin").read_bytes(), b"RAWDATA")

    def test_bad_zip_appends_warning(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc_ntsc.zip"}])
        mock_bytes_fn = Mock(return_value=b"PK\x03\x04this-is-not-a-valid-archive")

        with tempfile.TemporaryDirectory() as temp_dir:
            with patch("zipfile.is_zipfile", return_value=True):
                result = install_platform_firmware(
                    mock_json_fn,
                    mock_bytes_fn,
                    19,
                    [Path(temp_dir)],
                )

        self.assertTrue(result)
        self.assertIn("Failed to extract firmware archive gc_ntsc.zip", result[0])

    def test_overwrite_existing_firmware_when_skip_false(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock(return_value=b"NEWDATA")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            dest_path = target_dir / "gc-ntsc-12-101.bin"
            dest_path.write_bytes(b"ORIGINAL")

            install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
                skip_existing=False,
            )

            mock_bytes_fn.assert_called_once()
            self.assertEqual(dest_path.read_bytes(), b"NEWDATA")

    def test_multiple_target_dirs_all_receive_the_file(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock(return_value=b"FIRMWAREDATA")

        with tempfile.TemporaryDirectory() as temp_dir_one, tempfile.TemporaryDirectory() as temp_dir_two:
            target_one = Path(temp_dir_one)
            target_two = Path(temp_dir_two)

            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_one, target_two],
            )

            self.assertEqual(result, [])
            self.assertEqual((target_one / "gc-ntsc-12-101.bin").read_bytes(), b"FIRMWAREDATA")
            self.assertEqual((target_two / "gc-ntsc-12-101.bin").read_bytes(), b"FIRMWAREDATA")

    def test_routed_firmware_zip_to_correct_region_dir(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc_ntsc.zip"}])
        zip_buffer = io.BytesIO()
        with zipfile.ZipFile(zip_buffer, "w") as zf:
            zf.writestr("IPL.bin", b"IPLDATA")
        mock_bytes_fn = Mock(return_value=zip_buffer.getvalue())

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            jap_dir = root / "JAP"
            usa_dir = root / "USA"
            eur_dir = root / "EUR"

            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [
                    (jap_dir, ["ntsc_j", "ntsc-j", "jap", "jpn"]),
                    (usa_dir, ["ntsc", "usa"]),
                    (eur_dir, ["pal", "eur"]),
                ],
            )

            self.assertEqual(result, [])
            self.assertFalse((jap_dir / "IPL.bin").exists())
            self.assertTrue((usa_dir / "IPL.bin").exists())
            self.assertFalse((eur_dir / "IPL.bin").exists())

    def test_routed_firmware_no_match_silently_skipped(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc_unknown.zip"}])
        mock_bytes_fn = Mock(return_value=b"unused")

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            jap_dir = root / "JAP"
            usa_dir = root / "USA"
            eur_dir = root / "EUR"

            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [
                    (jap_dir, ["ntsc_j", "ntsc-j", "jap", "jpn"]),
                    (usa_dir, ["ntsc", "usa"]),
                    (eur_dir, ["pal", "eur"]),
                ],
            )

            self.assertEqual(result, [])
            mock_bytes_fn.assert_not_called()
            self.assertFalse(jap_dir.exists())
            self.assertFalse(usa_dir.exists())
            self.assertFalse(eur_dir.exists())

    def test_firmware_downloaded_once_for_multiple_dirs(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock(return_value=b"FIRMWAREDATA")

        with tempfile.TemporaryDirectory() as temp_dir_one, tempfile.TemporaryDirectory() as temp_dir_two:
            target_one = Path(temp_dir_one)
            target_two = Path(temp_dir_two)

            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_one, target_two],
            )

            self.assertEqual(result, [])
            mock_bytes_fn.assert_called_once_with("/api/firmware/3/content/gc-ntsc-12-101.bin")
            self.assertEqual((target_one / "gc-ntsc-12-101.bin").read_bytes(), b"FIRMWAREDATA")
            self.assertEqual((target_two / "gc-ntsc-12-101.bin").read_bytes(), b"FIRMWAREDATA")

    def test_download_error_appends_warning_not_exception(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock(side_effect=OSError("disk error"))

        with tempfile.TemporaryDirectory() as temp_dir:
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [Path(temp_dir)],
            )

        self.assertTrue(result)
        self.assertIn("gc-ntsc-12-101.bin", result[0])

    def test_fetch_error_appends_warning_not_exception(self) -> None:
        mock_json_fn = Mock(side_effect=Exception("connection refused"))
        mock_bytes_fn = Mock()

        with tempfile.TemporaryDirectory() as temp_dir:
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [Path(temp_dir)],
            )

        self.assertTrue(result)

    def test_invalid_record_missing_id_skipped(self) -> None:
        mock_json_fn = Mock(return_value=[{"file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock()

        with tempfile.TemporaryDirectory() as temp_dir:
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [Path(temp_dir)],
            )

        mock_bytes_fn.assert_not_called()
        self.assertEqual(result, [])

    def test_invalid_record_blank_filename_skipped(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": ""}])
        mock_bytes_fn = Mock()

        with tempfile.TemporaryDirectory() as temp_dir:
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [Path(temp_dir)],
            )

        mock_bytes_fn.assert_not_called()
        self.assertEqual(result, [])

    def test_target_dir_created_if_not_existing(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock(return_value=b"FIRMWAREDATA")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir) / "bios" / "subdir"
            self.assertFalse(target_dir.exists())

            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
            )

            self.assertEqual(result, [])
            self.assertTrue(target_dir.exists())
            self.assertEqual((target_dir / "gc-ntsc-12-101.bin").read_bytes(), b"FIRMWAREDATA")

    def test_mkdir_failure_appends_warning(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "gc-ntsc-12-101.bin"}])
        mock_bytes_fn = Mock(return_value=b"FIRMWAREDATA")

        with tempfile.TemporaryDirectory() as temp_dir:
            with patch("pathlib.Path.mkdir", side_effect=OSError("permission denied")):
                result = install_platform_firmware(
                    mock_json_fn,
                    mock_bytes_fn,
                    19,
                    [Path(temp_dir) / "bios"],
                )

        self.assertTrue(result)
        self.assertIn("Could not create firmware directory", result[0])

    def test_fetch_platform_firmware_calls_correct_path_and_params(self) -> None:
        mock_fn = Mock(return_value=[{"id": 1, "file_name": "test.bin"}])

        result = fetch_platform_firmware(mock_fn, 19)

        mock_fn.assert_called_once_with("/api/firmware", {"platform_id": 19})
        self.assertEqual(result, [{"id": 1, "file_name": "test.bin"}])

    def test_download_firmware_bytes_calls_correct_path(self) -> None:
        mock_fn = Mock(return_value=b"data")

        result = download_firmware_bytes(mock_fn, 42, "scph5501.bin")

        mock_fn.assert_called_once_with("/api/firmware/42/content/scph5501.bin")
        self.assertEqual(result, b"data")

    def test_fetch_platform_firmware_returns_empty_on_non_list(self) -> None:
        mock_fn = Mock(return_value={})

        result = fetch_platform_firmware(mock_fn, 19)

        self.assertEqual(result, [])

    def test_7z_archive_preserves_existing_files_in_target(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "firmware.7z"}])
        mock_bytes_fn = Mock(return_value=b"archivedata")

        def fake_extract(archive_path, staging_dir):
            (staging_dir / "IPL.bin").write_bytes(b"NEWBIOS")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            existing_file = target_dir / "existing.bin"
            existing_file.write_bytes(b"KEEPME")

            with patch("rom_mate.library.archive_preparation.extract_archive_into_directory", side_effect=fake_extract):
                install_platform_firmware(
                    mock_json_fn, mock_bytes_fn, 19, [target_dir],
                )

            self.assertTrue(existing_file.exists())
            self.assertEqual(existing_file.read_bytes(), b"KEEPME")
            self.assertTrue((target_dir / "IPL.bin").exists())
            self.assertEqual((target_dir / "IPL.bin").read_bytes(), b"NEWBIOS")

    def test_7z_archive_skip_existing_honored(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "firmware.7z"}])
        mock_bytes_fn = Mock(return_value=b"archivedata")

        def fake_extract(archive_path, staging_dir):
            (staging_dir / "IPL.bin").write_bytes(b"NEWDATA")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            (target_dir / "IPL.bin").write_bytes(b"ORIGINAL")

            with patch("rom_mate.library.archive_preparation.extract_archive_into_directory", side_effect=fake_extract):
                install_platform_firmware(
                    mock_json_fn, mock_bytes_fn, 19, [target_dir],
                    skip_existing=True,
                )

            self.assertEqual((target_dir / "IPL.bin").read_bytes(), b"ORIGINAL")

    def test_7z_archive_overwrites_when_skip_false(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "firmware.7z"}])
        mock_bytes_fn = Mock(return_value=b"archivedata")

        def fake_extract(archive_path, staging_dir):
            (staging_dir / "IPL.bin").write_bytes(b"NEWDATA")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            (target_dir / "IPL.bin").write_bytes(b"ORIGINAL")

            with patch("rom_mate.library.archive_preparation.extract_archive_into_directory", side_effect=fake_extract):
                install_platform_firmware(
                    mock_json_fn, mock_bytes_fn, 19, [target_dir],
                    skip_existing=False,
                )

            self.assertEqual((target_dir / "IPL.bin").read_bytes(), b"NEWDATA")

    def test_7z_archive_skips_macosx_and_ds_store(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "firmware.7z"}])
        mock_bytes_fn = Mock(return_value=b"archivedata")

        def fake_extract(archive_path, staging_dir):
            (staging_dir / "IPL.bin").write_bytes(b"BIOSDATA")
            macosx_dir = staging_dir / "__MACOSX"
            macosx_dir.mkdir()
            (macosx_dir / "._IPL.bin").write_bytes(b"JUNK")
            (staging_dir / ".DS_Store").write_bytes(b"JUNK")

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)

            with patch("rom_mate.library.archive_preparation.extract_archive_into_directory", side_effect=fake_extract):
                install_platform_firmware(
                    mock_json_fn, mock_bytes_fn, 19, [target_dir],
                )

            self.assertTrue((target_dir / "IPL.bin").exists())
            self.assertFalse((target_dir / "._IPL.bin").exists())
            self.assertFalse((target_dir / ".DS_Store").exists())


if __name__ == "__main__":
    unittest.main()


class RetroArchCoreFirmwareMetadataTests(unittest.TestCase):
    """Tests for retroarch_core_firmware_metadata()."""

    SAMPLE_ENTRIES = [
        {
            "core_file": "flycast_libretro.dll",
            "platforms": ["Sega Dreamcast"],
            "firmware": {
                "needs_bios": False,
                "subdirectory": "dc",
                "files": ["dc_boot.bin", "dc_flash.bin"],
            },
        },
        {
            "core_file": "mednafen_psx_hw_libretro.dll",
            "platforms": ["Sony PlayStation"],
            "firmware": {
                "needs_bios": True,
                "subdirectory": None,
                "files": ["scph5500.bin", "scph5501.bin", "scph5502.bin"],
            },
        },
        {
            "core_file": "snes9x_libretro.dll",
            "platforms": ["Nintendo SNES"],
        },
        {
            "core_file": "dolphin_libretro.dll",
            "platforms": ["Nintendo GameCube"],
        },
    ]

    def test_core_with_subdirectory_returns_metadata(self) -> None:
        result = retroarch_core_firmware_metadata("flycast", self.SAMPLE_ENTRIES)

        self.assertIsNotNone(result)
        self.assertEqual(result["subdirectory"], "dc")
        self.assertFalse(result["needs_bios"])
        self.assertIn("dc_boot.bin", result["files"])

    def test_core_with_null_subdirectory_returns_metadata(self) -> None:
        result = retroarch_core_firmware_metadata("mednafen_psx_hw", self.SAMPLE_ENTRIES)

        self.assertIsNotNone(result)
        self.assertIsNone(result["subdirectory"])
        self.assertTrue(result["needs_bios"])

    def test_core_without_firmware_key_returns_none(self) -> None:
        result = retroarch_core_firmware_metadata("snes9x", self.SAMPLE_ENTRIES)

        self.assertIsNone(result)

    def test_skipped_core_returns_none(self) -> None:
        result = retroarch_core_firmware_metadata("dolphin", self.SAMPLE_ENTRIES)

        self.assertIsNone(result)

    def test_unknown_core_id_returns_none(self) -> None:
        result = retroarch_core_firmware_metadata("totally_fake_core", self.SAMPLE_ENTRIES)

        self.assertIsNone(result)

    def test_empty_entries_returns_none(self) -> None:
        result = retroarch_core_firmware_metadata("flycast", [])

        self.assertIsNone(result)

    def test_empty_core_id_returns_none(self) -> None:
        result = retroarch_core_firmware_metadata("", self.SAMPLE_ENTRIES)

        self.assertIsNone(result)


class RetroArchFirmwareDirectoryRoutingTests(unittest.TestCase):
    """Tests for subdirectory routing logic used in firmware install."""

    def test_subdirectory_appended_to_plain_path(self) -> None:
        dirs = [Path("/retroarch/system")]
        subdirectory = "dc"

        result = [d / subdirectory for d in dirs]

        self.assertEqual(result, [Path("/retroarch/system/dc")])

    def test_null_subdirectory_leaves_dirs_unchanged(self) -> None:
        dirs = [Path("/retroarch/system")]
        subdirectory = None

        if isinstance(subdirectory, str) and subdirectory.strip():
            result = [d / subdirectory for d in dirs]
        else:
            result = list(dirs)

        self.assertEqual(result, [Path("/retroarch/system")])

    def test_subdirectory_appended_to_tuple_entry(self) -> None:
        dirs = [(Path("/retroarch/system"), ["ntsc"])]
        subdirectory = "dc"

        result = [
            (d[0] / subdirectory, d[1]) if isinstance(d, tuple) else d / subdirectory
            for d in dirs
        ]

        self.assertEqual(result, [(Path("/retroarch/system/dc"), ["ntsc"])])

    def test_nested_subdirectory_path(self) -> None:
        dirs = [Path("/retroarch/system")]
        subdirectory = "ep128emu/rom"

        result = [d / subdirectory for d in dirs]

        self.assertEqual(result, [Path("/retroarch/system/ep128emu/rom")])
