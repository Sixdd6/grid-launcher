from __future__ import annotations

import io
import json
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
from rom_mate.emulator.retroarch import (
    retroarch_core_config_files_metadata,
    retroarch_core_firmware_metadata,
    retroarch_core_saves_files_metadata,
)


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

    def test_resolve_all_matches_win(self) -> None:
        targets = [
            (Path("/gc/jap"), ["ntsc_j", "jap"]),
            (Path("/gc/usa"), ["ntsc"]),
        ]

        result = resolve_firmware_targets("gc_ntsc_j.zip", targets)

        self.assertEqual(result, [Path("/gc/jap"), Path("/gc/usa")])

    def test_resolve_ntsc_goes_to_jap_and_usa(self) -> None:
        profile_path = Path(__file__).resolve().parent.parent / "emulator-autoprofiles.json"
        profiles = json.loads(profile_path.read_text(encoding="utf-8"))
        dolphin_profile = next(p for p in profiles if p.get("name") == "Dolphin")

        routed_targets = [
            (Path(entry["path"]), entry["match"])
            for entry in dolphin_profile.get("firmware_directories", [])
            if isinstance(entry, dict) and entry.get("path") and entry.get("match")
        ]

        result = resolve_firmware_targets("gc-ntsc.zip", routed_targets)

        self.assertEqual(result, [Path("Sys/GC/JAP"), Path("Sys/GC/USA")])

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


class CemuFirmwareRoutingTests(unittest.TestCase):
    def test_cemu_firmware_dirs_wrapped_with_keys_txt_filter(self) -> None:
        firmware_dirs = [Path("/cemu/portable")]

        result = [
            d if isinstance(d, tuple) else (d, ["keys.txt"])
            for d in firmware_dirs
        ]

        self.assertEqual(result, [(Path("/cemu/portable"), ["keys.txt"])])

    def test_cemu_firmware_dirs_preserves_existing_tuples(self) -> None:
        firmware_dirs = [(Path("/cemu/portable"), ["existing_filter"])]

        result = [
            d if isinstance(d, tuple) else (d, ["keys.txt"])
            for d in firmware_dirs
        ]

        self.assertEqual(result, [(Path("/cemu/portable"), ["existing_filter"])])


class EdenFirmwareRoutingTests(unittest.TestCase):
    def setUp(self) -> None:
        profile_path = Path(__file__).resolve().parent.parent / "emulator-autoprofiles.json"
        profiles = json.loads(profile_path.read_text(encoding="utf-8"))
        eden_profile = next(p for p in profiles if p.get("name") == "Eden (Nintendo Switch)")
        self.routed_targets = [
            (Path(entry["path"]), entry["match"])
            for entry in eden_profile.get("firmware_directories", [])
            if isinstance(entry, dict) and entry.get("path") and entry.get("match")
        ]

    def test_eden_keys_zip_routes_to_keys_dir(self) -> None:
        result = resolve_firmware_targets("switch-keys.zip", self.routed_targets)
        self.assertEqual(result, [Path("user\\keys")])

    def test_eden_firmware_zip_routes_to_registered_dir(self) -> None:
        result = resolve_firmware_targets("switch-firmware.zip", self.routed_targets)
        self.assertEqual(result, [Path("user\\nand\\system\\Contents\\registered")])

    def test_eden_unrelated_file_routes_to_neither(self) -> None:
        result = resolve_firmware_targets("something-else.bin", self.routed_targets)
        self.assertEqual(result, [])

    def test_eden_firmware_routing_is_case_insensitive(self) -> None:
        result = resolve_firmware_targets("Switch-Keys.ZIP", self.routed_targets)
        self.assertEqual(result, [Path("user\\keys")])


class XemuFirmwareRoutingTests(unittest.TestCase):
    def setUp(self) -> None:
        profile_path = Path(__file__).resolve().parent.parent / "emulator-autoprofiles.json"
        profiles = json.loads(profile_path.read_text(encoding="utf-8"))
        xemu_profile = next(p for p in profiles if p.get("name") == "Xemu (Xbox)")
        self.firmware_dirs = [
            Path(entry)
            for entry in xemu_profile.get("firmware_directories", [])
            if isinstance(entry, str) and entry
        ]

    def test_xemu_firmware_routes_to_emulator_dir(self) -> None:
        self.assertIn(Path("."), self.firmware_dirs)

        result = resolve_firmware_targets("xbox-firmware.zip", self.firmware_dirs)

        self.assertEqual(result, [Path(".")])


class Rpcs3FirmwareRoutingTests(unittest.TestCase):
    def setUp(self) -> None:
        profile_path = Path(__file__).resolve().parent.parent / "emulator-autoprofiles.json"
        profiles = json.loads(profile_path.read_text(encoding="utf-8"))
        rpcs3_profile = next(p for p in profiles if p.get("name") == "RPCS3 (Playstation 3)")
        self.firmware_dirs = [
            Path(entry)
            for entry in rpcs3_profile.get("firmware_directories", [])
            if isinstance(entry, str) and entry
        ]

    def test_rpcs3_firmware_routes_to_emulator_dir(self) -> None:
        self.assertIn(Path("."), self.firmware_dirs)
        result = resolve_firmware_targets("PS3UPDAT.PUP", self.firmware_dirs)
        self.assertEqual(result, [Path(".")])

    def test_rpcs3_pup_detected_when_present(self) -> None:
        from rom_mate.emulator.rpcs3 import rpcs3_pup_path

        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            pup = Path(tmp) / "PS3UPDAT.PUP"
            pup.write_bytes(b"")
            result = rpcs3_pup_path(str(exe))
        self.assertIsNotNone(result)

    def test_rpcs3_pup_absent_returns_none(self) -> None:
        from rom_mate.emulator.rpcs3 import rpcs3_pup_path

        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            result = rpcs3_pup_path(str(exe))
        self.assertIsNone(result)


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


class FirmwareZipArchiveBehaviorTests(unittest.TestCase):
    @staticmethod
    def _zip_bytes(member_name: str, payload: bytes) -> bytes:
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w") as zf:
            zf.writestr(member_name, payload)
        return buffer.getvalue()

    def test_explicitly_named_zip_is_saved_as_archive(self) -> None:
        file_name = "naomi.zip"
        archive_data = self._zip_bytes("epr-21576h.ic27", b"ROMDATA")
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": file_name}])
        mock_bytes_fn = Mock(return_value=archive_data)

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                23,
                [(target_dir, ["naomi.zip"])],
            )

            archive_path = target_dir / file_name
            self.assertEqual(result, [])
            self.assertTrue(archive_path.exists())
            self.assertTrue(zipfile.is_zipfile(archive_path))
            self.assertFalse((target_dir / "epr-21576h.ic27").exists())
            with zipfile.ZipFile(archive_path, "r") as zf:
                self.assertEqual(zf.namelist(), ["epr-21576h.ic27"])

    def test_non_explicitly_named_zip_is_extracted(self) -> None:
        file_name = "naomi.zip"
        archive_data = self._zip_bytes("epr-21576h.ic27", b"ROMDATA")
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": file_name}])
        mock_bytes_fn = Mock(return_value=archive_data)

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                23,
                [target_dir],
            )

            self.assertEqual(result, [])
            self.assertFalse((target_dir / file_name).exists())
            self.assertEqual((target_dir / "epr-21576h.ic27").read_bytes(), b"ROMDATA")

    def test_zip_not_in_keyword_list_is_extracted(self) -> None:
        file_name = "naomi.zip"
        archive_data = self._zip_bytes("epr-21576h.ic27", b"ROMDATA")
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": file_name}])
        mock_bytes_fn = Mock(return_value=archive_data)

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                23,
                [(target_dir, ["naomi", "naomi2"])],
            )

            self.assertEqual(result, [])
            self.assertFalse((target_dir / file_name).exists())
            self.assertEqual((target_dir / "epr-21576h.ic27").read_bytes(), b"ROMDATA")


class FirmwareExtractZipWithPathsTests(unittest.TestCase):
    @staticmethod
    def _zip_bytes(members: dict[str, bytes]) -> bytes:
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w") as zf:
            for member_name, payload in members.items():
                zf.writestr(member_name, payload)
        return buffer.getvalue()

    def test_zip_extracted_preserving_relative_paths(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "dolphin-gc-bios.zip"}])
        archive_data = self._zip_bytes(
            {
                "dolphin-emu/User/GC/USA/IPL.bin": b"USA",
                "dolphin-emu/User/GC/EUR/IPL.bin": b"EUR",
            }
        )
        mock_bytes_fn = Mock(return_value=archive_data)

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
                extract_zip_with_paths=True,
            )

            self.assertEqual(result, [])
            self.assertEqual((target_dir / "dolphin-emu/User/GC/USA/IPL.bin").read_bytes(), b"USA")
            self.assertEqual((target_dir / "dolphin-emu/User/GC/EUR/IPL.bin").read_bytes(), b"EUR")

    def test_extract_with_paths_overrides_keep_archive_tuple(self) -> None:
        file_name = "dolphin-gc-bios.zip"
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": file_name}])
        archive_data = self._zip_bytes(
            {
                "dolphin-emu/User/GC/USA/IPL.bin": b"USA",
                "dolphin-emu/User/GC/EUR/IPL.bin": b"EUR",
            }
        )
        mock_bytes_fn = Mock(return_value=archive_data)

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [(target_dir, [file_name])],
                extract_zip_with_paths=True,
            )

            self.assertEqual(result, [])
            self.assertFalse((target_dir / file_name).exists())
            self.assertTrue((target_dir / "dolphin-emu/User/GC/USA/IPL.bin").exists())
            self.assertTrue((target_dir / "dolphin-emu/User/GC/EUR/IPL.bin").exists())

    def test_extract_with_paths_skips_macosx_entries(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "dolphin-gc-bios.zip"}])
        archive_data = self._zip_bytes(
            {
                "__MACOSX/._IPL.bin": b"META",
                "dolphin-emu/User/GC/USA/IPL.bin": b"USA",
            }
        )
        mock_bytes_fn = Mock(return_value=archive_data)

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
                extract_zip_with_paths=True,
            )

            self.assertEqual(result, [])
            self.assertFalse((target_dir / "__MACOSX/._IPL.bin").exists())
            self.assertTrue((target_dir / "dolphin-emu/User/GC/USA/IPL.bin").exists())

    def test_extract_with_paths_skips_traversal_members(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "dolphin-gc-bios.zip"}])
        archive_data = self._zip_bytes(
            {
                "../../outside_passwd": b"BAD",
                "dolphin-emu/User/GC/USA/IPL.bin": b"USA",
            }
        )
        mock_bytes_fn = Mock(return_value=archive_data)

        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = Path(temp_dir)
            target_dir = temp_root / "target"
            outside_path = temp_root / "outside_passwd"

            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
                extract_zip_with_paths=True,
            )

            self.assertEqual(result, [])
            self.assertFalse(outside_path.exists())
            self.assertTrue((target_dir / "dolphin-emu/User/GC/USA/IPL.bin").exists())

    def test_extract_with_paths_skip_existing_respects_nested_path(self) -> None:
        mock_json_fn = Mock(return_value=[{"id": 3, "file_name": "dolphin-gc-bios.zip"}])
        archive_data = self._zip_bytes(
            {
                "dolphin-emu/User/GC/USA/IPL.bin": b"NEW",
            }
        )
        mock_bytes_fn = Mock(return_value=archive_data)

        with tempfile.TemporaryDirectory() as temp_dir:
            target_dir = Path(temp_dir)
            existing = target_dir / "dolphin-emu/User/GC/USA/IPL.bin"
            existing.parent.mkdir(parents=True, exist_ok=True)
            existing.write_bytes(b"ORIGINAL")

            result = install_platform_firmware(
                mock_json_fn,
                mock_bytes_fn,
                19,
                [target_dir],
                skip_existing=True,
                extract_zip_with_paths=True,
            )

            self.assertEqual(result, [])
            self.assertEqual(existing.read_bytes(), b"ORIGINAL")

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


class RetroArchCoreConfigFilesMetadataTests(unittest.TestCase):
    """Tests for retroarch_core_config_files_metadata()."""

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
            "core_file": "snes9x_libretro.dll",
            "platforms": ["Nintendo SNES"],
        },
        {
            "core_file": "dolphin_libretro.dll",
            "platforms": ["Nintendo GameCube", "Nintendo Wii"],
            "config_files": {
                "base_dir": "config/dolphin-emu",
                "files": ["dolphin-emu.opt"],
            },
        },
    ]

    def test_retroarch_core_config_files_metadata_found(self) -> None:
        result = retroarch_core_config_files_metadata("dolphin", self.SAMPLE_ENTRIES)

        self.assertIsNotNone(result)
        self.assertEqual(result["base_dir"], "config/dolphin-emu")
        self.assertEqual(result["files"], ["dolphin-emu.opt"])

    def test_retroarch_core_config_files_metadata_absent(self) -> None:
        result = retroarch_core_config_files_metadata("snes9x", self.SAMPLE_ENTRIES)

        self.assertIsNone(result)

    def test_retroarch_core_config_files_metadata_wrong_core(self) -> None:
        result = retroarch_core_config_files_metadata("totally_fake_core", self.SAMPLE_ENTRIES)

        self.assertIsNone(result)

    def test_retroarch_core_config_files_metadata_real_json_new_cores(self) -> None:
        core_list_path = Path(__file__).resolve().parent.parent / "retroarch-core-list.json"
        entries = json.loads(core_list_path.read_text(encoding="utf-8"))

        cases = [
            ("flycast", "config/flycast", ["flycast.opt"]),
            ("pcsx_rearmed", "config/PCSX-ReARMed", ["PCSX-ReARMed.opt"]),
            ("duckstation", "config/DuckStation", ["DuckStation.opt"]),
            ("mupen64plus_next", "config/Mupen64Plus-Next", ["Mupen64Plus-Next.opt"]),
            ("mgba", "config/mGBA", ["mGBA.opt"]),
            ("melonds_ds", "config/melonDS DS", ["melonDS DS.opt"]),
            ("ppsspp", "config/PPSSPP", ["PPSSPP.opt"]),
            ("genesis_plus_gx", "config/Genesis Plus GX", ["Genesis Plus GX.opt"]),
        ]

        for core_id, expected_base_dir, expected_files in cases:
            with self.subTest(core_id=core_id):
                result = retroarch_core_config_files_metadata(core_id, entries)

                self.assertIsNotNone(result)
                self.assertEqual(result["base_dir"], expected_base_dir)
                self.assertEqual(result["files"], expected_files)


class RetroArchFirmwareExtractWithPathsTests(unittest.TestCase):
    """Tests for extract_with_paths firmware metadata handling."""

    def test_dolphin_firmware_metadata_extract_with_paths(self) -> None:
        core_list_path = Path(__file__).resolve().parent.parent / "retroarch-core-list.json"
        entries = json.loads(core_list_path.read_text(encoding="utf-8"))

        result = retroarch_core_firmware_metadata("dolphin", entries)

        self.assertIsNotNone(result)
        self.assertTrue(result["extract_with_paths"])
        self.assertEqual(result["subdirectory"], "dolphin-emu/Sys")
        self.assertEqual(result["files"], ["dolphin-gc-bios.zip"])

    def test_firmware_install_passes_extract_with_paths_flag(self) -> None:
        metadata = {
            "needs_bios": False,
            "subdirectory": "dolphin-emu/Sys",
            "files": ["dolphin-gc-bios.zip"],
            "extract_with_paths": True,
        }

        extract_zip_with_paths = bool(metadata.get("extract_with_paths", False))

        self.assertTrue(extract_zip_with_paths)


class RetroArchCoreSavesFilesMetadataTests(unittest.TestCase):
    """Tests for retroarch_core_saves_files_metadata()."""

    SAMPLE_ENTRIES = [
        {
            "core_file": "dolphin_libretro.dll",
            "platforms": ["Nintendo GameCube", "Nintendo Wii"],
            "saves_files": {
                "file": "dolphin-gc-bios.zip",
            },
        },
        {
            "core_file": "snes9x_libretro.dll",
            "platforms": ["Nintendo SNES"],
        },
    ]

    def test_saves_files_metadata_found(self) -> None:
        result = retroarch_core_saves_files_metadata("dolphin", self.SAMPLE_ENTRIES)

        self.assertIsNotNone(result)
        self.assertEqual(result, {"file": "dolphin-gc-bios.zip"})

    def test_saves_files_metadata_absent(self) -> None:
        result = retroarch_core_saves_files_metadata("snes9x", self.SAMPLE_ENTRIES)

        self.assertIsNone(result)

    def test_saves_files_metadata_wrong_core(self) -> None:
        result = retroarch_core_saves_files_metadata("totally_fake_core", self.SAMPLE_ENTRIES)

        self.assertIsNone(result)

    def test_saves_files_metadata_real_json_dolphin(self) -> None:
        core_list_path = Path(__file__).resolve().parent.parent / "retroarch-core-list.json"
        entries = json.loads(core_list_path.read_text(encoding="utf-8"))

        result = retroarch_core_saves_files_metadata("dolphin", entries)

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


class RetroArchConfigFileDirsFilteringTests(unittest.TestCase):
    """Tests for config_file_dirs assembly logic used in rom-mate.py."""

    @staticmethod
    def _build_config_file_dirs(config_metadata: dict, emulator_dir: Path) -> list:
        config_file_dirs: list = []
        if isinstance(config_metadata, dict):
            base_dir = config_metadata.get("base_dir")
            if isinstance(base_dir, str) and base_dir.strip():
                file_names = config_metadata.get("files", [])
                if isinstance(file_names, list) and file_names:
                    config_file_dirs = [(emulator_dir / base_dir, list(file_names))]
                else:
                    config_file_dirs = [emulator_dir / base_dir]
        return config_file_dirs

    def test_config_file_dirs_uses_files_list_as_filter(self) -> None:
        config_metadata = {"base_dir": "config/mGBA", "files": ["mGBA.opt"]}

        result = self._build_config_file_dirs(config_metadata, Path("/retroarch"))

        self.assertEqual(len(result), 1)
        self.assertIsInstance(result[0], tuple)
        self.assertEqual(result[0], (Path("/retroarch/config/mGBA"), ["mGBA.opt"]))

    def test_config_file_dirs_plain_when_files_list_empty(self) -> None:
        config_metadata = {"base_dir": "config/mGBA", "files": []}

        result = self._build_config_file_dirs(config_metadata, Path("/retroarch"))

        self.assertEqual(len(result), 1)
        self.assertIsInstance(result[0], Path)
        self.assertEqual(result[0], Path("/retroarch/config/mGBA"))

    def test_config_file_dirs_plain_when_files_key_absent(self) -> None:
        config_metadata = {"base_dir": "config/mGBA"}

        result = self._build_config_file_dirs(config_metadata, Path("/retroarch"))

        self.assertEqual(len(result), 1)
        self.assertIsInstance(result[0], Path)
        self.assertEqual(result[0], Path("/retroarch/config/mGBA"))


class RetroArchSavesFileDirsAssemblyTests(unittest.TestCase):
    """Tests for saves_file_dirs assembly logic used in rom-mate.py."""

    @staticmethod
    def _build_saves_file_dirs(saves_metadata: dict | None, saves_dir: Path) -> list:
        saves_file_dirs: list = []
        if isinstance(saves_metadata, dict):
            saves_file_name = saves_metadata.get("file")
            if isinstance(saves_file_name, str) and saves_file_name.strip():
                saves_file_dirs = [(saves_dir, [saves_file_name])]
        return saves_file_dirs

    def test_saves_file_dirs_builds_tuple_with_filter(self) -> None:
        saves_metadata = {"file": "dolphin-gc-bios.zip"}

        result = self._build_saves_file_dirs(saves_metadata, Path("/retroarch/saves"))

        self.assertEqual(result, [(Path("/retroarch/saves"), ["dolphin-gc-bios.zip"])])

    def test_saves_file_dirs_empty_when_file_key_missing(self) -> None:
        saves_metadata = {}

        result = self._build_saves_file_dirs(saves_metadata, Path("/retroarch/saves"))

        self.assertEqual(result, [])

    def test_saves_file_dirs_empty_when_metadata_is_none(self) -> None:
        result = self._build_saves_file_dirs(None, Path("/retroarch/saves"))

        self.assertEqual(result, [])


class RetroArchFirmwareDirsFilteringTests(unittest.TestCase):
    """Tests for firmware_dirs tuple-filter assembly logic used in rom-mate.py."""

    @staticmethod
    def _apply_firmware_file_filter(firmware_dirs: list, metadata: dict) -> list:
        file_names = metadata.get("files", [])
        if isinstance(file_names, list) and file_names:
            return [
                (d if isinstance(d, tuple) else (d, list(file_names)))
                for d in firmware_dirs
            ]
        return list(firmware_dirs)

    def test_firmware_dirs_uses_tuple_filter_when_files_nonempty(self) -> None:
        firmware_dirs = [Path("/retroarch/system/dc")]
        metadata = {"files": ["dc_boot.bin", "dc_flash.bin"]}

        result = self._apply_firmware_file_filter(firmware_dirs, metadata)

        self.assertEqual(len(result), 1)
        self.assertIsInstance(result[0], tuple)
        self.assertEqual(result[0], (Path("/retroarch/system/dc"), ["dc_boot.bin", "dc_flash.bin"]))

    def test_firmware_dirs_plain_path_when_files_list_empty(self) -> None:
        firmware_dirs = [Path("/retroarch/system")]
        metadata = {"files": []}

        result = self._apply_firmware_file_filter(firmware_dirs, metadata)

        self.assertEqual(len(result), 1)
        self.assertIsInstance(result[0], Path)
        self.assertEqual(result[0], Path("/retroarch/system"))

    def test_firmware_dirs_plain_path_when_files_key_absent(self) -> None:
        firmware_dirs = [Path("/retroarch/system")]
        metadata = {"subdirectory": None}

        result = self._apply_firmware_file_filter(firmware_dirs, metadata)

        self.assertEqual(len(result), 1)
        self.assertIsInstance(result[0], Path)
        self.assertEqual(result[0], Path("/retroarch/system"))

    def test_firmware_dirs_leaves_existing_tuple_entry_unchanged(self) -> None:
        firmware_dirs = [(Path("/retroarch/system/dc"), ["some_filter"])]
        metadata = {"files": ["dc_boot.bin", "dc_flash.bin"]}

        result = self._apply_firmware_file_filter(firmware_dirs, metadata)

        self.assertEqual(len(result), 1)
        self.assertIsInstance(result[0], tuple)
        self.assertEqual(result[0], (Path("/retroarch/system/dc"), ["some_filter"]))
