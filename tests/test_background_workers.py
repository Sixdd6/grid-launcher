from __future__ import annotations

import json
import tempfile
import unittest
from io import BytesIO
from pathlib import Path
from unittest.mock import patch
from urllib.error import HTTPError

from rom_mate.background.workers import (
    DetailsCloudRecordsWorker,
    InstallDownloadWorker,
    PCGamingWikiWorker,
    RetroAchievementsWorker,
)
from rom_mate.emulator.source import EmulatorSourceResolutionError


class _StubWindow:
    def __init__(self) -> None:
        self.called: list[tuple[str, str]] = []

    def _server_save_records_for_rom(self, rom_id: str):
        self.called.append(("save", rom_id))
        return [{"id": "1", "file_name": "save-1.zip"}]

    def _server_state_records_for_rom(self, rom_id: str):
        self.called.append(("state", rom_id))
        return [{"id": "2", "file_name": "state-1.zip"}]


class _FailingWindow(_StubWindow):
    def _server_save_records_for_rom(self, rom_id: str):
        raise ValueError("boom")


class DetailsCloudRecordsWorkerTests(unittest.TestCase):
    def test_worker_fetches_save_records(self) -> None:
        window = _StubWindow()
        worker = DetailsCloudRecordsWorker(window, 7, "99", "save")
        results: list[tuple[int, str, object, str]] = []
        worker.finished.connect(lambda request_id, save_type, records, error: results.append((request_id, save_type, records, error)))

        worker.run()

        self.assertEqual(window.called, [("save", "99")])
        self.assertEqual(results, [(7, "save", [{"id": "1", "file_name": "save-1.zip"}], "")])

    def test_worker_emits_error_for_failed_requests(self) -> None:
        window = _FailingWindow()
        worker = DetailsCloudRecordsWorker(window, 9, "88", "save")
        results: list[tuple[int, str, object, str]] = []
        worker.finished.connect(lambda request_id, save_type, records, error: results.append((request_id, save_type, records, error)))

        worker.run()

        self.assertEqual(results, [(9, "save", [], "boom")])


class InstallDownloadWorkerTests(unittest.TestCase):
    class _ResponseStub:
        def __init__(self, payload: bytes, *, content_length: int | None = None) -> None:
            self._payload = payload
            self._offset = 0
            self.headers: dict[str, str] = {}
            if content_length is not None:
                self.headers["Content-Length"] = str(content_length)

        def read(self, size: int = -1) -> bytes:
            if size is None or size < 0:
                size = len(self._payload) - self._offset
            chunk = self._payload[self._offset : self._offset + size]
            self._offset += len(chunk)
            return chunk

        def __enter__(self):
            return self

        def __exit__(self, exc_type, exc, tb):
            return False

    def test_http_error_includes_status_reason_url_and_body(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "game.zip"
            worker = InstallDownloadWorker(
                "https://server.example/api/roms/1/content/game.zip",
                {"Accept": "*/*"},
                archive_path,
            )
            results: list[tuple[str, str]] = []
            worker.finished.connect(lambda path, error: results.append((path, error)))

            http_error = HTTPError(
                "https://server.example/api/roms/1/content/game.zip",
                403,
                "Forbidden",
                None,
                BytesIO(b'{"detail":"Token invalid for this ROM"}'),
            )
            with patch("rom_mate.background.workers.urlopen", side_effect=http_error):
                worker.run()

            self.assertEqual(len(results), 1)
            self.assertEqual(results[0][0], "")
            self.assertIn("HTTP 403 Forbidden", results[0][1])
            self.assertIn("url=https://server.example/api/roms/1/content/game.zip", results[0][1])
            self.assertIn("Token invalid for this ROM", results[0][1])

    def test_debug_logging_prints_url_and_error_details(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "game.zip"
            worker = InstallDownloadWorker(
                "https://server.example/api/roms/1/content/game.zip",
                {"Accept": "*/*"},
                archive_path,
                debug_enabled=True,
            )
            results: list[tuple[str, str]] = []
            worker.finished.connect(lambda path, error: results.append((path, error)))

            http_error = HTTPError(
                "https://server.example/api/roms/1/content/game.zip",
                401,
                "Unauthorized",
                None,
                BytesIO(b"access denied"),
            )
            with patch("rom_mate.background.workers.urlopen", side_effect=http_error):
                with patch("builtins.print") as mock_print:
                    worker.run()

            self.assertEqual(len(results), 1)
            printed_text = "\n".join(" ".join(str(item) for item in call.args) for call in mock_print.call_args_list)
            self.assertIn("[DEBUG][InstallDownload] url=https://server.example/api/roms/1/content/game.zip", printed_text)
            self.assertIn("[DEBUG][InstallDownload] error=HTTP 401 Unauthorized", printed_text)

    def test_source_metadata_download_uses_github_release_and_windows_asset_name(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "duckstation.zip"
            worker = InstallDownloadWorker(
                "",
                {},
                archive_path,
                source_metadata={
                    "provider": "github-release",
                    "owner": "stenzek",
                    "repo": "duckstation",
                    "release_tag": "latest",
                    "windows_arch": "x64",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name": "duckstation-windows-x64-release.zip",
                        }
                    ],
                },
            )
            results: list[tuple[str, str]] = []
            worker.finished.connect(lambda path, error: results.append((path, error)))

            release_payload = json.dumps(
                {
                    "tag_name": "v0.1.0",
                    "assets": [
                        {
                            "name": "duckstation-windows-x64-release.zip",
                            "browser_download_url": "https://example.test/duckstation.zip",
                            "state": "uploaded",
                        }
                    ],
                }
            ).encode("utf-8")
            archive_payload = b"zip-data"

            with patch(
                "rom_mate.background.workers.urlopen",
                side_effect=[
                    self._ResponseStub(release_payload),
                    self._ResponseStub(archive_payload, content_length=len(archive_payload)),
                ],
            ):
                worker.run()

            self.assertEqual(results, [(str(archive_path), "")])
            self.assertTrue(archive_path.exists())
            self.assertEqual(archive_path.read_bytes(), archive_payload)

    def test_source_metadata_download_error_cleans_partial_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "ppsspp.zip"
            archive_path.write_bytes(b"stale")
            worker = InstallDownloadWorker(
                "",
                {},
                archive_path,
                source_metadata={
                    "provider": "github-release",
                    "owner": "hrydgard",
                    "repo": "ppsspp",
                    "release_tag": "latest",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name": "PPSSPP-v1.17.1-Windows-x64.zip",
                        }
                    ],
                },
            )
            results: list[tuple[str, str]] = []
            worker.finished.connect(lambda path, error: results.append((path, error)))

            release_payload = json.dumps(
                {
                    "tag_name": "v1.0.0",
                    "assets": [
                        {
                            "name": "PPSSPP-v1.17.1-Windows-x64.zip",
                            "browser_download_url": "https://example.test/ppsspp.zip",
                            "state": "uploaded",
                        }
                    ],
                }
            ).encode("utf-8")
            download_error = HTTPError(
                "https://example.test/ppsspp.zip",
                404,
                "Not Found",
                None,
                BytesIO(b"missing"),
            )
            with patch(
                "rom_mate.background.workers.urlopen",
                side_effect=[
                    self._ResponseStub(release_payload),
                    download_error,
                ],
            ):
                worker.run()

            self.assertEqual(len(results), 1)
            self.assertEqual(results[0][0], "")
            self.assertIn("HTTP 404 Not Found", results[0][1])
            self.assertFalse(archive_path.exists())

    def test_source_metadata_windows_assets_mismatch_does_not_fallback_to_linux_asset(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "duckstation.zip"
            worker = InstallDownloadWorker(
                "",
                {},
                archive_path,
                source_metadata={
                    "provider": "github",
                    "owner": "stenzek",
                    "repo": "duckstation",
                    "release_tag": "latest",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name": "duckstation-windows-x64-release.zip",
                        }
                    ],
                },
            )

            release_payload = {
                "tag_name": "v0.1.0",
                "assets": [
                    {
                        "name": "duckstation-linux-x64.AppImage",
                        "browser_download_url": "https://example.test/duckstation-linux.AppImage",
                    },
                    {
                        "name": "duckstation-macos-universal.zip",
                        "browser_download_url": "https://example.test/duckstation-macos.zip",
                    },
                ],
            }

            with patch.object(worker, "_load_json", return_value=release_payload):
                with self.assertRaises(EmulatorSourceResolutionError) as raised:
                    worker._resolve_source_download(worker.source_metadata or {})

            message = str(raised.exception)
            self.assertIn("windows_assets", message)
            self.assertIn("duckstation-linux-x64.AppImage", message)
            self.assertIn("duckstation-macos-universal.zip", message)

    def test_source_metadata_xenia_canary_exact_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("xenia_canary.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "xenia-canary",
            "repo": "xenia-canary",
            "release_tag": "latest",
            "windows_assets": [
                {
                    "arch": "x64",
                    "asset_name": "xenia_canary_windows.zip",
                }
            ],
        }
        release_payload = {
            "tag_name": "canary_experimental",
            "assets": [
                {
                    "name": "xenia_canary_windows.zip",
                    "browser_download_url": "https://example.test/xenia_canary_windows.zip",
                },
                {
                    "name": "xenia_canary_windows_symbols.zip",
                    "browser_download_url": "https://example.test/xenia_canary_windows_symbols.zip",
                },
            ],
        }

        with patch.object(worker, "_load_json", return_value=release_payload):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "xenia_canary_windows.zip")
        self.assertEqual(resolved["download_url"], "https://example.test/xenia_canary_windows.zip")

    def test_source_metadata_cemu_regex_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("cemu.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "cemu-project",
            "repo": "Cemu",
            "release_tag": "latest",
            "windows_assets": [
                {
                    "arch": "x64",
                    "asset_name_regex": "^cemu-[0-9.]+-windows-x64\\.zip$",
                }
            ],
        }
        release_payload = {
            "tag_name": "v2.6",
            "assets": [
                {
                    "name": "cemu-2.6-windows-x64.zip",
                    "browser_download_url": "https://example.test/cemu-2.6-windows-x64.zip",
                },
                {
                    "name": "cemu-2.6-linux-x64.tar.xz",
                    "browser_download_url": "https://example.test/cemu-2.6-linux-x64.tar.xz",
                },
            ],
        }

        with patch.object(worker, "_load_json", return_value=release_payload):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "cemu-2.6-windows-x64.zip")
        self.assertEqual(resolved["download_url"], "https://example.test/cemu-2.6-windows-x64.zip")

    def test_source_metadata_xemu_resolves_x64_and_arm64_assets_by_windows_arch(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("xemu.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "xemu-project",
            "repo": "xemu",
            "release_tag": "latest",
            "windows_assets": [
                {
                    "arch": "x64",
                    "asset_name": "xemu-win-x86_64-release.zip",
                },
                {
                    "arch": "arm64",
                    "asset_name": "xemu-win-aarch64-release.zip",
                },
            ],
        }
        release_payload = {
            "tag_name": "v0.8.50",
            "assets": [
                {
                    "name": "xemu-win-aarch64-release.zip",
                    "browser_download_url": "https://example.test/xemu-win-aarch64-release.zip",
                },
                {
                    "name": "xemu-win-x86_64-release.zip",
                    "browser_download_url": "https://example.test/xemu-win-x86_64-release.zip",
                },
            ],
        }

        with patch.object(worker, "_load_json", return_value=release_payload):
            resolved_x64 = worker._resolve_source_download({**source_metadata, "windows_arch": "x64"})
            resolved_arm64 = worker._resolve_source_download({**source_metadata, "windows_arch": "arm64"})

        self.assertEqual(resolved_x64["asset_name"], "xemu-win-x86_64-release.zip")
        self.assertEqual(
            resolved_x64["download_url"],
            "https://example.test/xemu-win-x86_64-release.zip",
        )
        self.assertEqual(resolved_arm64["asset_name"], "xemu-win-aarch64-release.zip")
        self.assertEqual(
            resolved_arm64["download_url"],
            "https://example.test/xemu-win-aarch64-release.zip",
        )

    def test_source_metadata_shadps4_sdl_core_regex_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("shadps4-sdl.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "shadps4-emu",
            "repo": "shadPS4",
            "release_tag": "latest",
            "windows_assets": [
                {
                    "arch": "x64",
                    "asset_name_regex": "^shadps4-win64-sdl-[0-9.]+\\.zip$",
                }
            ],
        }
        release_payload = {
            "tag_name": "v0.10.0",
            "assets": [
                {
                    "name": "shadps4-win64-sdl-0.10.0.zip",
                    "browser_download_url": "https://example.test/shadps4-win64-sdl-0.10.0.zip",
                },
                {
                    "name": "shadps4-linux-sdl-0.10.0.tar.xz",
                    "browser_download_url": "https://example.test/shadps4-linux-sdl-0.10.0.tar.xz",
                },
                {
                    "name": "shadps4-macos-sdl-0.10.0.zip",
                    "browser_download_url": "https://example.test/shadps4-macos-sdl-0.10.0.zip",
                },
            ],
        }

        with patch.object(worker, "_load_json", return_value=release_payload):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "shadps4-win64-sdl-0.10.0.zip")
        self.assertEqual(
            resolved["download_url"],
            "https://example.test/shadps4-win64-sdl-0.10.0.zip",
        )

    def test_source_metadata_shadps4_qt_launcher_regex_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("shadps4-qtlauncher.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "shadps4-emu",
            "repo": "shadps4-qtlauncher",
            "release_tag": "latest",
            "windows_assets": [
                {
                    "arch": "x64",
                    "asset_name_regex": "^shadPS4QtLauncher-win64-qt-v[0-9]+\\.zip$",
                }
            ],
        }
        release_payload = {
            "tag_name": "v224",
            "assets": [
                {
                    "name": "shadPS4QtLauncher-win64-qt-v224.zip",
                    "browser_download_url": "https://example.test/shadPS4QtLauncher-win64-qt-v224.zip",
                },
                {
                    "name": "shadPS4QtLauncher-linux-qt-v224.zip",
                    "browser_download_url": "https://example.test/shadPS4QtLauncher-linux-qt-v224.zip",
                },
            ],
        }

        with patch.object(worker, "_load_json", return_value=release_payload):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "shadPS4QtLauncher-win64-qt-v224.zip")
        self.assertEqual(
            resolved["download_url"],
            "https://example.test/shadPS4QtLauncher-win64-qt-v224.zip",
        )

    def test_source_metadata_azahar_regex_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("azahar.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "azahar-emu",
            "repo": "azahar",
            "release_tag": "latest",
            "windows_assets": [
                {
                    "arch": "x64",
                    "asset_name_regex": "^azahar-windows-msvc-[0-9.]+\\.zip$",
                }
            ],
        }
        release_payload = {
            "tag_name": "v1.2.3",
            "assets": [
                {
                    "name": "azahar-windows-msvc-1.2.3.zip",
                    "browser_download_url": "https://example.test/azahar-windows-msvc-1.2.3.zip",
                },
                {
                    "name": "azahar-linux-appimage-1.2.3.tar.gz",
                    "browser_download_url": "https://example.test/azahar-linux-appimage-1.2.3.tar.gz",
                },
            ],
        }

        with patch.object(worker, "_load_json", return_value=release_payload):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "azahar-windows-msvc-1.2.3.zip")
        self.assertEqual(
            resolved["download_url"],
            "https://example.test/azahar-windows-msvc-1.2.3.zip",
        )

    def test_source_metadata_pcsx2_qt_7z_regex_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("pcsx2.7z"))
        source_metadata = {
            "provider": "github-release",
            "owner": "PCSX2",
            "repo": "pcsx2",
            "release_tag": "latest",
            "windows_assets": [
                {
                    "arch": "x64",
                    "asset_name_regex": "^pcsx2-v[0-9.]+-windows-x64-Qt\\.7z$",
                }
            ],
        }
        release_payload = {
            "tag_name": "v2.6.3",
            "assets": [
                {
                    "name": "pcsx2-v2.6.3-windows-x64-Qt-symbols.7z",
                    "browser_download_url": "https://example.test/pcsx2-symbols.7z",
                },
                {
                    "name": "pcsx2-v2.6.3-windows-x64-Qt.7z",
                    "browser_download_url": "https://example.test/pcsx2.7z",
                },
                {
                    "name": "PCSX2-v2.6.3-windows-x64-installer.exe",
                    "browser_download_url": "https://example.test/pcsx2-installer.exe",
                },
            ],
        }

        with patch.object(worker, "_load_json", return_value=release_payload):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "pcsx2-v2.6.3-windows-x64-Qt.7z")
        self.assertEqual(resolved["download_url"], "https://example.test/pcsx2.7z")

    def test_source_metadata_download_rewrites_archive_suffix_from_resolved_asset_name(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            initial_archive_path = Path(temp_dir) / "pcsx2.zip"
            expected_archive_path = Path(temp_dir) / "pcsx2.7z"
            worker = InstallDownloadWorker(
                "",
                {},
                initial_archive_path,
                source_metadata={
                    "provider": "github-release",
                    "owner": "PCSX2",
                    "repo": "pcsx2",
                    "release_tag": "latest",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name_regex": "^pcsx2-v[0-9.]+-windows-x64-Qt\\.7z$",
                        }
                    ],
                },
            )
            results: list[tuple[str, str]] = []
            worker.finished.connect(lambda path, error: results.append((path, error)))

            release_payload = json.dumps(
                {
                    "tag_name": "v2.6.3",
                    "assets": [
                        {
                            "name": "pcsx2-v2.6.3-windows-x64-Qt.7z",
                            "browser_download_url": "https://example.test/pcsx2.7z",
                        }
                    ],
                }
            ).encode("utf-8")
            archive_payload = b"7z-data"

            with patch(
                "rom_mate.background.workers.urlopen",
                side_effect=[
                    self._ResponseStub(release_payload),
                    self._ResponseStub(archive_payload, content_length=len(archive_payload)),
                ],
            ):
                worker.run()

            self.assertEqual(results, [(str(expected_archive_path), "")])
            self.assertFalse(initial_archive_path.exists())
            self.assertTrue(expected_archive_path.exists())
            self.assertEqual(expected_archive_path.read_bytes(), archive_payload)


class TestRetroAchievementsWorker(unittest.TestCase):
    def _run_worker(self, worker):
        results = []
        worker.finished.connect(lambda rid, achs, err: results.append((rid, achs, err)))
        worker.run()
        return results

    def test_worker_emits_achievements_on_success(self):
        with patch("rom_mate.server.retroachievements.fetch_game_achievements") as mock_fetch:
            mock_fetch.return_value = [{"id": 1, "title": "Test"}]
            worker = RetroAchievementsWorker(42, 100, "user", "key")
            results = self._run_worker(worker)
        self.assertEqual(len(results), 1)
        rid, achs, err = results[0]
        self.assertEqual(rid, 42)
        self.assertEqual(achs, [{"id": 1, "title": "Test"}])
        self.assertEqual(err, "")

    def test_worker_emits_error_on_failure(self):
        from rom_mate.server.retroachievements import RetroAchievementsError

        with patch("rom_mate.server.retroachievements.fetch_game_achievements") as mock_fetch:
            mock_fetch.side_effect = RetroAchievementsError("network error")
            worker = RetroAchievementsWorker(7, 50, "user", "key")
            results = self._run_worker(worker)
        self.assertEqual(len(results), 1)
        rid, achs, err = results[0]
        self.assertEqual(rid, 7)
        self.assertEqual(achs, [])
        self.assertIn("network error", err)


class TestPCGamingWikiWorker(unittest.TestCase):
    def _run_worker(self, worker):
        results = []
        worker.finished.connect(lambda rid, paths, err: results.append((rid, paths, err)))
        worker.run()
        return results

    def test_pcgamingwiki_worker_emits_paths_on_success(self):
        with patch("rom_mate.server.pcgamingwiki.fetch_windows_save_paths") as mock_fetch:
            mock_fetch.return_value = ["%APPDATA%\\Game"]
            worker = PCGamingWikiWorker(42, "Game")
            results = self._run_worker(worker)
        self.assertEqual(results, [(42, ["%APPDATA%\\Game"], "")])

    def test_pcgamingwiki_worker_emits_error_on_failure(self):
        with patch("rom_mate.server.pcgamingwiki.fetch_windows_save_paths") as mock_fetch:
            mock_fetch.side_effect = Exception("timeout")
            worker = PCGamingWikiWorker(9, "Game")
            results = self._run_worker(worker)
        self.assertEqual(results, [(9, [], "timeout")])


if __name__ == "__main__":
    unittest.main()
