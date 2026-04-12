"""Tests for Xbox 360 / Xenia install support."""
from __future__ import annotations

import struct
import tempfile
import unittest
from pathlib import Path

from rom_mate.emulator.selection import is_xbox360_platform
from rom_mate.emulator.xenia import apply_xenia_content_without_ui, _read_stfs_header


def _make_stfs_header(title_id: int, content_type: int) -> bytes:
    """Build a minimal STFS header blob for testing."""
    header = bytearray(0x368)
    header[0:4] = b"CON "
    struct.pack_into(">I", header, 0x344, content_type)
    struct.pack_into(">I", header, 0x360, title_id)
    return bytes(header)


class XBox360PlatformDetectionTests(unittest.TestCase):
    def test_detects_xbox_360(self) -> None:
        self.assertTrue(is_xbox360_platform({"platform": "Xbox 360"}))

    def test_detects_xbox360_no_space(self) -> None:
        self.assertTrue(is_xbox360_platform({"platform": "Xbox360"}))

    def test_detects_microsoft_xbox_360(self) -> None:
        self.assertTrue(is_xbox360_platform({"platform": "Microsoft Xbox 360"}))

    def test_rejects_original_xbox(self) -> None:
        self.assertFalse(is_xbox360_platform({"platform": "Xbox"}))

    def test_rejects_xbox_one(self) -> None:
        self.assertFalse(is_xbox360_platform({"platform": "Xbox One"}))

    def test_rejects_playstation(self) -> None:
        self.assertFalse(is_xbox360_platform({"platform": "PlayStation 3"}))

    def test_rejects_empty(self) -> None:
        self.assertFalse(is_xbox360_platform({"platform": ""}))


class StfsHeaderParseTests(unittest.TestCase):
    def test_reads_title_id_and_content_type(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stfs_file = Path(tmp) / "TU000000"
            stfs_file.write_bytes(_make_stfs_header(0x545107FC, 0x000B0000))
            title_id, content_type = _read_stfs_header(stfs_file)
        self.assertEqual(title_id, "545107FC")
        self.assertEqual(content_type, "000B0000")

    def test_rejects_non_stfs_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            bad_file = Path(tmp) / "notSTFS"
            bad_file.write_bytes(b"\x00" * 0x368)
            title_id, content_type = _read_stfs_header(bad_file)
        self.assertEqual(title_id, "")
        self.assertEqual(content_type, "")

    def test_rejects_too_short_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            short_file = Path(tmp) / "short"
            short_file.write_bytes(b"CON " + b"\x00" * 10)
            title_id, content_type = _read_stfs_header(short_file)
        self.assertEqual(title_id, "")
        self.assertEqual(content_type, "")

    def test_accepts_live_magic(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stfs_file = Path(tmp) / "DEADBEEF"
            header = bytearray(_make_stfs_header(0xDEADBEEF, 0x00000002))
            header[0:4] = b"LIVE"
            stfs_file.write_bytes(bytes(header))
            title_id, _ = _read_stfs_header(stfs_file)
        self.assertEqual(title_id, "DEADBEEF")

    def test_accepts_pirs_magic(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stfs_file = Path(tmp) / "package"
            header = bytearray(_make_stfs_header(0x12345678, 0x000B0000))
            header[0:4] = b"PIRS"
            stfs_file.write_bytes(bytes(header))
            title_id, _ = _read_stfs_header(stfs_file)
        self.assertEqual(title_id, "12345678")


class XeniaContentInstallTests(unittest.TestCase):
    def test_places_title_update_under_correct_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            content_root = Path(tmp) / "content"
            stfs_file = Path(tmp) / "TU000000"
            stfs_file.write_bytes(_make_stfs_header(0x545107FC, 0x000B0000))

            result = apply_xenia_content_without_ui(stfs_file, content_root)

            self.assertEqual(result["error"], "")
            self.assertEqual(result["title_id"], "545107FC")
            self.assertEqual(result["content_type"], "000B0000")
            expected = content_root / "0000000000000000" / "545107FC" / "000B0000" / "TU000000"
            self.assertEqual(Path(result["destination"]).resolve(), expected.resolve())
            self.assertTrue(expected.exists())

    def test_places_dlc_under_correct_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            content_root = Path(tmp) / "content"
            stfs_file = Path(tmp) / "44530001"
            stfs_file.write_bytes(_make_stfs_header(0x545107FC, 0x00000002))

            result = apply_xenia_content_without_ui(stfs_file, content_root)

            self.assertEqual(result["error"], "")
            expected = content_root / "0000000000000000" / "545107FC" / "00000002" / "44530001"
            self.assertTrue(expected.exists())

    def test_rejects_non_stfs_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            content_root = Path(tmp) / "content"
            bad_file = Path(tmp) / "notSTFS"
            bad_file.write_bytes(b"\x00" * 0x400)

            result = apply_xenia_content_without_ui(bad_file, content_root)

        self.assertNotEqual(result["error"], "")
        self.assertEqual(result["title_id"], "")

    def test_rejects_missing_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            content_root = Path(tmp) / "content"
            result = apply_xenia_content_without_ui(Path(tmp) / "nonexistent", content_root)
        self.assertNotEqual(result["error"], "")

    def test_rejects_title_id_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            content_root = Path(tmp) / "content"
            stfs_file = Path(tmp) / "TU000000"
            stfs_file.write_bytes(_make_stfs_header(0x545107FC, 0x000B0000))

            result = apply_xenia_content_without_ui(stfs_file, content_root, expected_title_id="AAAABBBB")

        self.assertIn("mismatch", result["error"].casefold())

    def test_accepts_matching_expected_title_id(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            content_root = Path(tmp) / "content"
            stfs_file = Path(tmp) / "TU000000"
            stfs_file.write_bytes(_make_stfs_header(0x545107FC, 0x000B0000))

            result = apply_xenia_content_without_ui(stfs_file, content_root, expected_title_id="545107FC")

        self.assertEqual(result["error"], "")

    def test_creates_content_root_if_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            content_root = Path(tmp) / "deep" / "content"
            stfs_file = Path(tmp) / "TU000000"
            stfs_file.write_bytes(_make_stfs_header(0x12345678, 0x000B0000))

            result = apply_xenia_content_without_ui(stfs_file, content_root)

            self.assertEqual(result["error"], "")
            self.assertTrue(Path(result["destination"]).exists())


class XeniaContentArchiveTests(unittest.TestCase):
    """Tests for apply_xenia_content_archive_without_ui."""

    def _make_zip_with_stfs(self, zip_path: Path, stfs_files: list[tuple[str, bytes]]) -> None:
        import zipfile
        with zipfile.ZipFile(str(zip_path), "w") as zf:
            for name, data in stfs_files:
                zf.writestr(name, data)

    def test_installs_stfs_files_from_zip(self) -> None:
        from rom_mate.library.archive_preparation import apply_xenia_content_archive_without_ui
        import zipfile

        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            content_root = tmp_path / "content"
            zip_path = tmp_path / "content.zip"

            stfs_data = _make_stfs_header(0x545107FC, 0x000B0000)
            with zipfile.ZipFile(str(zip_path), "w") as zf:
                zf.writestr("TU000000", stfs_data)

            extract_dir = tmp_path / "extracted"

            def fake_extracted_dir(p: Path) -> Path:
                return extract_dir

            def fake_extract(archive: Path, dest: Path, cb: object) -> None:
                import zipfile as zf_mod
                dest.mkdir(parents=True, exist_ok=True)
                with zf_mod.ZipFile(str(archive)) as zf:
                    zf.extractall(str(dest))

            results, warning = apply_xenia_content_archive_without_ui(
                zip_path,
                content_root,
                extracted_dir_for_archive_path=fake_extracted_dir,
                extract_archive_into_directory=fake_extract,
            )

        self.assertEqual(warning, "")
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0]["title_id"], "545107FC")

    def test_returns_error_for_non_stfs_files_in_zip(self) -> None:
        from rom_mate.library.archive_preparation import apply_xenia_content_archive_without_ui
        import zipfile

        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            content_root = tmp_path / "content"
            zip_path = tmp_path / "content.zip"

            with zipfile.ZipFile(str(zip_path), "w") as zf:
                zf.writestr("notSTFS", b"\x00" * 0x400)

            extract_dir = tmp_path / "extracted"

            def fake_extracted_dir(p: Path) -> Path:
                return extract_dir

            def fake_extract(archive: Path, dest: Path, cb: object) -> None:
                import zipfile as zf_mod
                dest.mkdir(parents=True, exist_ok=True)
                with zf_mod.ZipFile(str(archive)) as zf:
                    zf.extractall(str(dest))

            results, warning = apply_xenia_content_archive_without_ui(
                zip_path,
                content_root,
                extracted_dir_for_archive_path=fake_extracted_dir,
                extract_archive_into_directory=fake_extract,
            )

        self.assertEqual(results, [])
        self.assertNotEqual(warning, "")
