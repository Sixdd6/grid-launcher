from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import MagicMock, patch

from rom_mate.core.config import normalize_installed_games
from rom_mate.library.archive_preparation import (
    _BUNDLED_7Z_PATH,
    apply_ps4_content_archive_without_ui,
    extract_archive_into_directory,
    extracted_dir_for_archive_path,
)
from rom_mate.library.install_registry import build_installed_game_record


class PS4ContentApplyTests(unittest.TestCase):
    def _write_ps4_content_zip(self, zip_path: Path, title_id: str, file_map: dict[str, bytes]) -> None:
        with zipfile.ZipFile(zip_path, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
            for relative_path, payload in file_map.items():
                archive.writestr(f"{title_id}/{relative_path}", payload)

    def test_apply_ps4_content_archive_merges_into_existing_title_dir_and_tracks_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            installed_root = root / "installed"
            title_dir = installed_root / "CUSA12345"
            title_dir.mkdir(parents=True)

            existing_file = title_dir / "sce_sys" / "param.sfo"
            existing_file.parent.mkdir(parents=True)
            existing_file.write_bytes(b"old")

            archive_path = root / "update.zip"
            self._write_ps4_content_zip(
                archive_path,
                "CUSA12345",
                {
                    "sce_sys/param.sfo": b"new",
                    "patch/data.bin": b"patch",
                },
            )

            updated_game, warning_text = apply_ps4_content_archive_without_ui(
                {
                    "title": "Demo Game",
                    "platform": "PS4",
                    "ps4_game_id": "CUSA12345",
                    "extracted_dir": str(installed_root),
                },
                archive_path,
                content_kind="update",
                extracted_dir_for_archive_path=extracted_dir_for_archive_path,
                extract_archive_into_directory=extract_archive_into_directory,
            )

            self.assertEqual(warning_text, "")
            self.assertIsNotNone(updated_game)
            assert updated_game is not None
            self.assertEqual(existing_file.read_bytes(), b"new")
            self.assertEqual((title_dir / "patch" / "data.bin").read_bytes(), b"patch")
            self.assertFalse(archive_path.exists())

            metadata = json.loads(updated_game.get("ps4_content", "[]"))
            self.assertEqual(len(metadata), 1)
            self.assertEqual(metadata[0]["kind"], "update")
            self.assertEqual(metadata[0]["title_id"], "CUSA12345")
            self.assertEqual(metadata[0]["archive_name"], "update.zip")
            self.assertTrue(metadata[0].get("applied_at", "").isdigit())

    def test_apply_ps4_content_archive_fails_on_title_id_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            installed_root = root / "installed"
            title_dir = installed_root / "CUSA12345"
            title_dir.mkdir(parents=True)
            (title_dir / "eboot.bin").write_bytes(b"base")

            archive_path = root / "dlc.zip"
            self._write_ps4_content_zip(
                archive_path,
                "CUSA99999",
                {
                    "dlc/content.pkg": b"dlc",
                },
            )

            updated_game, error_text = apply_ps4_content_archive_without_ui(
                {
                    "title": "Demo Game",
                    "platform": "PlayStation 4",
                    "ps4_game_id": "CUSA12345",
                    "extracted_dir": str(installed_root),
                },
                archive_path,
                content_kind="dlc",
                extracted_dir_for_archive_path=extracted_dir_for_archive_path,
                extract_archive_into_directory=extract_archive_into_directory,
            )

            self.assertIsNone(updated_game)
            self.assertIn("title ID mismatch", error_text)
            self.assertIn("expected CUSA12345", error_text)
            self.assertEqual((title_dir / "eboot.bin").read_bytes(), b"base")

    def test_build_installed_record_and_config_normalization_keep_ps4_content_metadata(self) -> None:
        record = build_installed_game_record(
            {
                "title": "Demo Game",
                "platform": "PS4",
                "ps4_game_id": "cusa12345",
                "ps4_content": " [{\"kind\":\"update\"}] ",
            },
            Path("demo.zip"),
            resolved_cover_url="",
            cached_cover_path="",
        )
        self.assertEqual(record["ps4_content"], "[{\"kind\":\"update\"}]")

        normalized = normalize_installed_games(
            [record],
            lambda game: (game.get("title", ""), game.get("platform", "")),
        )
        self.assertEqual(len(normalized), 1)
        self.assertEqual(normalized[0]["ps4_game_id"], "CUSA12345")
        self.assertEqual(normalized[0]["ps4_content"], "[{\"kind\":\"update\"}]")

    def test_extract_archive_into_directory_supports_7z(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path = root / "pcsx2.7z"
            archive_path.write_bytes(b"7z-test")

            extracted_dir = root / "extract"
            progress_updates: list[tuple[int, int]] = []
            expected_extracted = extracted_dir / "PCSX2" / "pcsx2-qt.exe"

            def fake_extract(_archive_path: Path, out_dir: Path) -> None:
                target = out_dir / "PCSX2"
                target.mkdir(parents=True, exist_ok=True)
                (target / "pcsx2-qt.exe").write_bytes(b"pcsx2-binary")

            with patch("rom_mate.library.archive_preparation._extract_7z_with_fallbacks", side_effect=fake_extract):
                extract_archive_into_directory(
                    archive_path,
                    extracted_dir,
                    install_progress_callback=lambda installed, total: progress_updates.append((installed, total)),
                )

            self.assertTrue(expected_extracted.exists())
            self.assertEqual(expected_extracted.read_bytes(), b"pcsx2-binary")
            self.assertGreaterEqual(len(progress_updates), 2)
            self.assertEqual(progress_updates[0], (0, 0))
            self.assertGreater(progress_updates[-1][0], 0)
            self.assertEqual(progress_updates[-1][0], progress_updates[-1][1])

    def test_rar_routes_through_7z_fallbacks(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path = root / "duckstation.rar"
            archive_path.write_bytes(b"rar-test")
            extracted_dir = root / "extract"

            with patch("rom_mate.library.archive_preparation._extract_7z_with_fallbacks") as mock_extract:
                extract_archive_into_directory(archive_path, extracted_dir)

            mock_extract.assert_called_once_with(archive_path, extracted_dir)

    def test_system_7z_tried_first(self) -> None:
        from rom_mate.library.archive_preparation import _extract_7z_with_fallbacks

        archive = Path("/fake/test.7z")
        out_dir = Path("/fake/out")
        call_order = []

        def fake_run(cmd, **kwargs):
            call_order.append(cmd[0])
            return MagicMock(returncode=0, stderr="")

        bundled_path = MagicMock()
        bundled_path.exists.return_value = False
        bundled_path.__str__.return_value = "C:/bundled/7z.exe"

        with patch("rom_mate.library.archive_preparation._BUNDLED_7Z_PATH", bundled_path), \
             patch("subprocess.run", side_effect=fake_run), \
             patch("rom_mate.library.archive_preparation._ensure_full_7z", return_value=None):
            _extract_7z_with_fallbacks(archive, out_dir)

        self.assertTrue(any(c in ("7z", "7za", "7zz") for c in call_order))
        self.assertEqual(call_order[0], call_order[0])

    def test_portable_7z_downloaded_and_used_as_last_resort(self) -> None:
        from rom_mate.library.archive_preparation import _extract_7z_with_fallbacks

        archive = Path("/fake/test.7z")
        out_dir = Path("/fake/out")
        full_7z_path = Path("C:/tools/7zz.exe")

        def fake_run(cmd, **kwargs):
            if cmd[0] == str(full_7z_path):
                return MagicMock(returncode=0, stderr="")
            raise FileNotFoundError

        bundled_path = MagicMock()
        bundled_path.exists.return_value = False
        bundled_path.__str__.return_value = "C:/bundled/7z.exe"

        with patch("rom_mate.library.archive_preparation._BUNDLED_7Z_PATH", bundled_path), \
             patch("subprocess.run", side_effect=fake_run), \
             patch("rom_mate.library.archive_preparation._ensure_full_7z", return_value=full_7z_path), \
             patch("shutil.rmtree"):
            _extract_7z_with_fallbacks(archive, out_dir)

    def test_bundled_7z_used_when_available(self) -> None:
        from rom_mate.library.archive_preparation import _BUNDLED_7Z_PATH, _extract_7z_with_fallbacks

        archive = Path("/fake/test.7z")
        out_dir = Path("/fake/out")
        seen_commands: list[list[str]] = []

        def fake_run(cmd, **kwargs):
            seen_commands.append(cmd)
            return MagicMock(returncode=0, stderr="")

        bundled_path = MagicMock()
        bundled_path.exists.return_value = True
        bundled_path.__str__.return_value = str(_BUNDLED_7Z_PATH)

        with patch("rom_mate.library.archive_preparation._BUNDLED_7Z_PATH", bundled_path), \
             patch("subprocess.run", side_effect=fake_run):
            _extract_7z_with_fallbacks(archive, out_dir)

        self.assertGreaterEqual(len(seen_commands), 1)
        self.assertEqual(seen_commands[0][0], str(_BUNDLED_7Z_PATH))

    def test_portable_7z_reused_when_already_downloaded(self) -> None:
        from rom_mate.library.archive_preparation import _ensure_portable_7z, _PORTABLE_7ZR_PATH

        with patch("pathlib.Path.exists", return_value=True):
            result = _ensure_portable_7z()

        if result is not None:
            self.assertEqual(result, _PORTABLE_7ZR_PATH)


class TestExtractArchiveIntoDirectory(unittest.TestCase):
    @unittest.skipUnless(_BUNDLED_7Z_PATH.exists(), "Bundled 7z.exe not available")
    def test_bundled_7z_extracts_real_7z_archive(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            test_file = Path("hello.txt")
            (root / test_file).write_bytes(b"Hello from 7z")

            archive_path = root / "smoke.7z"
            subprocess.run(
                [str(_BUNDLED_7Z_PATH), "a", str(archive_path), str(test_file), "-y"],
                cwd=root,
                check=True,
                capture_output=True,
                text=True,
            )

            extract_dir = root / "extract"
            extract_dir.mkdir(parents=True, exist_ok=True)

            extract_archive_into_directory(archive_path, extract_dir)

            extracted_file = extract_dir / "hello.txt"
            self.assertTrue(extracted_file.exists())
            self.assertEqual(extracted_file.read_bytes(), b"Hello from 7z")


class TestEnsureFullSevenZip(unittest.TestCase):
    def test_returns_path_when_7zz_already_exists(self) -> None:
        from rom_mate.library.archive_preparation import _ensure_full_7z, _PORTABLE_7ZZ_PATH

        with patch("sys.platform", "win32"), \
             patch("pathlib.Path.exists", return_value=True), \
             patch("rom_mate.library.archive_preparation._ensure_portable_7z") as mock_bootstrap:
            result = _ensure_full_7z()

        self.assertEqual(result, _PORTABLE_7ZZ_PATH)
        mock_bootstrap.assert_not_called()

    def test_returns_none_on_non_windows(self) -> None:
        from rom_mate.library.archive_preparation import _ensure_full_7z

        with patch("sys.platform", "linux"):
            result = _ensure_full_7z()

        self.assertIsNone(result)

    def test_returns_none_when_7zr_unavailable(self) -> None:
        from rom_mate.library.archive_preparation import _ensure_full_7z

        with patch("sys.platform", "win32"), \
             patch("pathlib.Path.exists", return_value=False), \
             patch("rom_mate.library.archive_preparation._ensure_portable_7z", return_value=None):
            result = _ensure_full_7z()

        self.assertIsNone(result)

    def test_downloads_extra_archive_and_extracts_7zz(self) -> None:
        from rom_mate.library.archive_preparation import _ensure_full_7z, _PORTABLE_7ZZ_PATH

        with patch("sys.platform", "win32"), \
                         patch("pathlib.Path.exists", side_effect=[False, True, True]), \
             patch("rom_mate.library.archive_preparation._ensure_portable_7z", return_value=Path("C:/tools/7zr.exe")), \
                         patch("rom_mate.library.archive_preparation.urllib.request.urlopen", return_value=MagicMock(read=MagicMock(return_value=b""))), \
                         patch("pathlib.Path.write_bytes", return_value=0), \
               patch("pathlib.Path.mkdir"), \
             patch("rom_mate.library.archive_preparation.subprocess.run", return_value=MagicMock(returncode=0, stderr="")):
            result = _ensure_full_7z()

        self.assertEqual(result, _PORTABLE_7ZZ_PATH)

    def test_moves_x64_7zz_to_root_and_cleans_up_extra_files(self) -> None:
        import rom_mate.library.archive_preparation as archive_preparation

        with tempfile.TemporaryDirectory() as temp_dir:
            tools_dir = Path(temp_dir) / "tools"
            portable_7zz_path = tools_dir / "7zz.exe"
            extra_names = (
                "7za.exe",
                "7zS.sfx",
                "7zSD.sfx",
                "readme.txt",
                "History.txt",
                "License.txt",
                "7-ZipFar.dll",
                "7zS2.sfx",
                "7zS2con.sfx",
            )

            def fake_extract(*_args, **_kwargs):
                x64_dir = tools_dir / "x64"
                x64_dir.mkdir(parents=True, exist_ok=True)
                (x64_dir / "7zz.exe").write_bytes(b"portable-7zz")
                for name in extra_names:
                    (tools_dir / name).write_bytes(b"extra")
                return MagicMock(returncode=0, stderr="")

            with patch("sys.platform", "win32"), \
                 patch.object(archive_preparation, "_APP_TOOLS_DIR", tools_dir), \
                 patch.object(archive_preparation, "_PORTABLE_7ZZ_PATH", portable_7zz_path), \
                 patch("rom_mate.library.archive_preparation._ensure_portable_7z", return_value=Path("C:/tools/7zr.exe")), \
                 patch("rom_mate.library.archive_preparation.urllib.request.urlretrieve"), \
                 patch("rom_mate.library.archive_preparation.subprocess.run", side_effect=fake_extract):
                result = archive_preparation._ensure_full_7z()

            self.assertEqual(result, portable_7zz_path)
            self.assertTrue(portable_7zz_path.exists())
            self.assertFalse((tools_dir / "x64").exists())
            for name in extra_names:
                self.assertFalse((tools_dir / name).exists())

    def test_returns_none_when_extraction_fails(self) -> None:
        from rom_mate.library.archive_preparation import _ensure_full_7z

        with patch("sys.platform", "win32"), \
             patch("pathlib.Path.exists", return_value=False), \
             patch("rom_mate.library.archive_preparation._ensure_portable_7z", return_value=Path("C:/tools/7zr.exe")), \
             patch("rom_mate.library.archive_preparation.urllib.request.urlretrieve"), \
               patch("pathlib.Path.mkdir"), \
             patch("rom_mate.library.archive_preparation.subprocess.run", return_value=MagicMock(returncode=1, stderr="boom")):
            result = _ensure_full_7z()

        self.assertIsNone(result)

    def test_returns_none_when_subprocess_raises(self) -> None:
        from rom_mate.library.archive_preparation import _ensure_full_7z

        with patch("sys.platform", "win32"), \
             patch("pathlib.Path.exists", return_value=False), \
             patch("rom_mate.library.archive_preparation._ensure_portable_7z", return_value=Path("C:/tools/7zr.exe")), \
             patch("rom_mate.library.archive_preparation.urllib.request.urlretrieve"), \
               patch("pathlib.Path.mkdir"), \
             patch("rom_mate.library.archive_preparation.subprocess.run", side_effect=OSError("extract failed")):
            result = _ensure_full_7z()

        self.assertIsNone(result)

    def test_cleans_up_temp_file_on_failure(self) -> None:
        from rom_mate.library.archive_preparation import _ensure_full_7z

        with patch("sys.platform", "win32"), \
             patch("pathlib.Path.exists", return_value=False), \
             patch("rom_mate.library.archive_preparation._ensure_portable_7z", return_value=Path("C:/tools/7zr.exe")), \
             patch("rom_mate.library.archive_preparation.urllib.request.urlretrieve"), \
               patch("pathlib.Path.mkdir"), \
             patch("rom_mate.library.archive_preparation.subprocess.run", side_effect=OSError("extract failed")), \
             patch("pathlib.Path.unlink") as mock_unlink:
            result = _ensure_full_7z()

        self.assertIsNone(result)
        self.assertTrue(mock_unlink.called)


if __name__ == "__main__":
    unittest.main()
