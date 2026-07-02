from __future__ import annotations

import json
import tempfile
import unittest
from io import BytesIO
from pathlib import Path
from unittest.mock import MagicMock, patch
from urllib.error import HTTPError

from grid_launcher.background.workers import (
    DetailsCloudRecordsWorker,
    FlatpakInstallWorker,
    InstallDownloadWorker,
    InstallFinalizeWorker,
    MissingCoverReplenishWorker,
    PCGamingWikiWorker,
    RetroAchievementsWorker,
)
from grid_launcher.emulator.source import EmulatorSourceResolutionError


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
        results: list[dict[str, object]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        worker.run()

        self.assertEqual(window.called, [("save", "99")])
        self.assertEqual(
            results,
            [{"request_id": 7, "save_type": "save", "records": [{"id": "1", "file_name": "save-1.zip"}], "error": ""}],
        )

    def test_worker_emits_error_for_failed_requests(self) -> None:
        window = _FailingWindow()
        worker = DetailsCloudRecordsWorker(window, 9, "88", "save")
        results: list[dict[str, object]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        worker.run()

        self.assertEqual(results, [{"request_id": 9, "save_type": "save", "records": [], "error": "boom"}])


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
            results: list[dict[str, str]] = []
            worker.finished.connect(lambda payload: results.append(payload))

            http_error = HTTPError(
                "https://server.example/api/roms/1/content/game.zip",
                403,
                "Forbidden",
                None,
                BytesIO(b'{"detail":"Token invalid for this ROM"}'),
            )
            with patch("grid_launcher.background.workers.urlopen", side_effect=http_error):
                worker.run()

            self.assertEqual(len(results), 1)
            self.assertEqual(results[0].get("archive_path", ""), "")
            self.assertIn("HTTP 403 Forbidden", results[0].get("error", ""))
            self.assertIn("url=https://server.example/api/roms/1/content/game.zip", results[0].get("error", ""))
            self.assertIn("Token invalid for this ROM", results[0].get("error", ""))

    def test_debug_logging_prints_url_and_error_details(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "game.zip"
            worker = InstallDownloadWorker(
                "https://server.example/api/roms/1/content/game.zip",
                {"Accept": "*/*"},
                archive_path,
                debug_enabled=True,
            )
            results: list[dict[str, str]] = []
            worker.finished.connect(lambda payload: results.append(payload))

            http_error = HTTPError(
                "https://server.example/api/roms/1/content/game.zip",
                401,
                "Unauthorized",
                None,
                BytesIO(b"access denied"),
            )
            with patch("grid_launcher.background.workers.urlopen", side_effect=http_error):
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
            results: list[dict[str, str]] = []
            worker.finished.connect(lambda payload: results.append(payload))

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
                "grid_launcher.background.workers.urlopen",
                side_effect=[
                    self._ResponseStub(release_payload),
                    self._ResponseStub(archive_payload, content_length=len(archive_payload)),
                ],
            ):
                worker.run()

            self.assertEqual(results, [{"archive_path": str(archive_path), "error": ""}])
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
            results: list[dict[str, str]] = []
            worker.finished.connect(lambda payload: results.append(payload))

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
                "grid_launcher.background.workers.urlopen",
                side_effect=[
                    self._ResponseStub(release_payload),
                    download_error,
                ],
            ):
                worker.run()

            self.assertEqual(len(results), 1)
            self.assertEqual(results[0].get("archive_path", ""), "")
            self.assertIn("HTTP 404 Not Found", results[0].get("error", ""))
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

            with patch("sys.platform", "win32"):
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
            "asset_patterns": ["shadps4-win64-sdl-*.zip"],
            "launch_executable": "shadPS4.exe",
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["shadps4-linux-sdl-*.zip"],
                    "launch_executable": "shadPS4",
                },
                "darwin": {
                    "asset_patterns": ["shadps4-macos-sdl-*.zip"],
                    "launch_executable": "shadPS4",
                },
            },
        }
        release_payload = {
            "tag_name": "v0.16.0",
            "assets": [
                {
                    "name": "shadps4-win64-sdl-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-win64-sdl-0.16.0.zip",
                },
                {
                    "name": "shadps4-linux-sdl-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-linux-sdl-0.16.0.zip",
                },
                {
                    "name": "shadps4-macos-sdl-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-macos-sdl-0.16.0.zip",
                },
                {
                    "name": "shadps4-ubuntu64-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-ubuntu64-0.16.0.zip",
                },
            ],
        }

        with patch("sys.platform", "win32"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "shadps4-win64-sdl-0.16.0.zip")
        self.assertEqual(
            resolved["download_url"],
            "https://example.test/shadps4-win64-sdl-0.16.0.zip",
        )

    def test_source_metadata_shadps4_sdl_core_linux_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("shadps4-sdl.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "shadps4-emu",
            "repo": "shadPS4",
            "release_tag": "latest",
            "asset_patterns": ["shadps4-win64-sdl-*.zip"],
            "launch_executable": "shadPS4.exe",
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["shadps4-linux-sdl-*.zip"],
                    "launch_executable": "shadPS4",
                },
                "darwin": {
                    "asset_patterns": ["shadps4-macos-sdl-*.zip"],
                    "launch_executable": "shadPS4",
                },
            },
        }
        release_payload = {
            "tag_name": "v0.16.0",
            "assets": [
                {
                    "name": "shadps4-win64-sdl-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-win64-sdl-0.16.0.zip",
                },
                {
                    "name": "shadps4-linux-sdl-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-linux-sdl-0.16.0.zip",
                },
                {
                    "name": "shadps4-macos-sdl-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-macos-sdl-0.16.0.zip",
                },
                {
                    "name": "shadps4-ubuntu64-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-ubuntu64-0.16.0.zip",
                },
            ],
        }

        with patch("sys.platform", "linux"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "shadps4-linux-sdl-0.16.0.zip")
        self.assertEqual(
            resolved["download_url"],
            "https://example.test/shadps4-linux-sdl-0.16.0.zip",
        )

    def test_source_metadata_shadps4_sdl_core_macos_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("shadps4-sdl.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "shadps4-emu",
            "repo": "shadPS4",
            "release_tag": "latest",
            "asset_patterns": ["shadps4-win64-sdl-*.zip"],
            "launch_executable": "shadPS4.exe",
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["shadps4-linux-sdl-*.zip"],
                    "launch_executable": "shadPS4",
                },
                "darwin": {
                    "asset_patterns": ["shadps4-macos-sdl-*.zip"],
                    "launch_executable": "shadPS4",
                },
            },
        }
        release_payload = {
            "tag_name": "v0.16.0",
            "assets": [
                {
                    "name": "shadps4-win64-sdl-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-win64-sdl-0.16.0.zip",
                },
                {
                    "name": "shadps4-linux-sdl-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-linux-sdl-0.16.0.zip",
                },
                {
                    "name": "shadps4-macos-sdl-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-macos-sdl-0.16.0.zip",
                },
                {
                    "name": "shadps4-ubuntu64-0.16.0.zip",
                    "browser_download_url": "https://example.test/shadps4-ubuntu64-0.16.0.zip",
                },
            ],
        }

        with patch("sys.platform", "darwin"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "shadps4-macos-sdl-0.16.0.zip")
        self.assertEqual(
            resolved["download_url"],
            "https://example.test/shadps4-macos-sdl-0.16.0.zip",
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

    def test_source_metadata_duckstation_linux_appimage_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("DuckStation-x64.AppImage"))
        source_metadata = {
            "provider": "github-release",
            "owner": "stenzek",
            "repo": "duckstation",
            "release_tag": "latest",
            "windows_assets": [
                {
                    "arch": "x64",
                    "asset_name": "duckstation-windows-x64-release.zip",
                    "launch_executable": "duckstation-qt-x64-ReleaseLTCG.exe",
                },
                {
                    "arch": "arm64",
                    "asset_name": "duckstation-windows-arm64-release.zip",
                    "launch_executable": "duckstation-qt-arm64-ReleaseLTCG.exe",
                },
            ],
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["DuckStation-x64.AppImage"],
                    "launch_executable": "DuckStation-x64.AppImage",
                }
            },
        }
        release_payload = {
            "tag_name": "latest",
            "assets": [
                {
                    "name": "duckstation-windows-x64-release.zip",
                    "browser_download_url": "https://example.test/duckstation-windows-x64-release.zip",
                },
                {
                    "name": "duckstation-windows-arm64-release.zip",
                    "browser_download_url": "https://example.test/duckstation-windows-arm64-release.zip",
                },
                {
                    "name": "DuckStation-x64.AppImage",
                    "browser_download_url": "https://example.test/DuckStation-x64.AppImage",
                },
                {
                    "name": "DuckStation-arm64.AppImage",
                    "browser_download_url": "https://example.test/DuckStation-arm64.AppImage",
                },
            ],
        }

        with patch("sys.platform", "linux"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "DuckStation-x64.AppImage")
        self.assertEqual(
            resolved["download_url"],
            "https://example.test/DuckStation-x64.AppImage",
        )

    def test_source_metadata_supermodel_windows_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("supermodel-windows.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "trzy",
            "repo": "Supermodel",
            "release_tag": "latest",
            "asset_patterns": ["supermodel-*-windows.zip"],
            "launch_executable": "supermodel.exe",
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["supermodel-*-linux.tar.gz"],
                    "launch_executable": "supermodel",
                },
                "darwin": {
                    "asset_patterns": ["supermodel-*-macos.tar.gz"],
                    "launch_executable": "supermodel",
                },
            },
        }
        release_payload = {
            "tag_name": "v0.3a-20260528-git-77d28ee",
            "assets": [
                {
                    "name": "supermodel-0.3a-20260528-git-77d28ee-windows.zip",
                    "browser_download_url": "https://example.test/supermodel-0.3a-windows.zip",
                },
                {
                    "name": "supermodel-0.3a-20260528-git-77d28ee-linux.tar.gz",
                    "browser_download_url": "https://example.test/supermodel-0.3a-linux.tar.gz",
                },
                {
                    "name": "supermodel-0.3a-20260528-git-77d28ee-macos.tar.gz",
                    "browser_download_url": "https://example.test/supermodel-0.3a-macos.tar.gz",
                },
            ],
        }

        with patch("sys.platform", "win32"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "supermodel-0.3a-20260528-git-77d28ee-windows.zip")
        self.assertEqual(resolved["download_url"], "https://example.test/supermodel-0.3a-windows.zip")

    def test_source_metadata_supermodel_linux_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("supermodel-linux.tar.gz"))
        source_metadata = {
            "provider": "github-release",
            "owner": "trzy",
            "repo": "Supermodel",
            "release_tag": "latest",
            "asset_patterns": ["supermodel-*-windows.zip"],
            "launch_executable": "supermodel.exe",
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["supermodel-*-linux.tar.gz"],
                    "launch_executable": "supermodel",
                },
                "darwin": {
                    "asset_patterns": ["supermodel-*-macos.tar.gz"],
                    "launch_executable": "supermodel",
                },
            },
        }
        release_payload = {
            "tag_name": "v0.3a-20260528-git-77d28ee",
            "assets": [
                {
                    "name": "supermodel-0.3a-20260528-git-77d28ee-windows.zip",
                    "browser_download_url": "https://example.test/supermodel-0.3a-windows.zip",
                },
                {
                    "name": "supermodel-0.3a-20260528-git-77d28ee-linux.tar.gz",
                    "browser_download_url": "https://example.test/supermodel-0.3a-linux.tar.gz",
                },
                {
                    "name": "supermodel-0.3a-20260528-git-77d28ee-macos.tar.gz",
                    "browser_download_url": "https://example.test/supermodel-0.3a-macos.tar.gz",
                },
            ],
        }

        with patch("sys.platform", "linux"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "supermodel-0.3a-20260528-git-77d28ee-linux.tar.gz")
        self.assertEqual(resolved["download_url"], "https://example.test/supermodel-0.3a-linux.tar.gz")

    def test_source_metadata_supermodel_macos_asset_name_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("supermodel-macos.tar.gz"))
        source_metadata = {
            "provider": "github-release",
            "owner": "trzy",
            "repo": "Supermodel",
            "release_tag": "latest",
            "asset_patterns": ["supermodel-*-windows.zip"],
            "launch_executable": "supermodel.exe",
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["supermodel-*-linux.tar.gz"],
                    "launch_executable": "supermodel",
                },
                "darwin": {
                    "asset_patterns": ["supermodel-*-macos.tar.gz"],
                    "launch_executable": "supermodel",
                },
            },
        }
        release_payload = {
            "tag_name": "v0.3a-20260528-git-77d28ee",
            "assets": [
                {
                    "name": "supermodel-0.3a-20260528-git-77d28ee-windows.zip",
                    "browser_download_url": "https://example.test/supermodel-0.3a-windows.zip",
                },
                {
                    "name": "supermodel-0.3a-20260528-git-77d28ee-linux.tar.gz",
                    "browser_download_url": "https://example.test/supermodel-0.3a-linux.tar.gz",
                },
                {
                    "name": "supermodel-0.3a-20260528-git-77d28ee-macos.tar.gz",
                    "browser_download_url": "https://example.test/supermodel-0.3a-macos.tar.gz",
                },
            ],
        }

        with patch("sys.platform", "darwin"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "supermodel-0.3a-20260528-git-77d28ee-macos.tar.gz")
        self.assertEqual(resolved["download_url"], "https://example.test/supermodel-0.3a-macos.tar.gz")

    def test_source_metadata_vita3k_windows_asset_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("windows-latest.zip"))
        source_metadata = {
            "provider": "github-release",
            "owner": "Vita3K",
            "repo": "Vita3K",
            "release_tag": "continuous",
            "asset_patterns": ["windows-latest.zip"],
            "launch_executable": "Vita3K.exe",
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["Vita3K-x86_64.AppImage"],
                    "launch_executable": "Vita3K-x86_64.AppImage",
                },
            },
        }
        release_payload = {
            "tag_name": "continuous",
            "assets": [
                {
                    "name": "windows-latest.zip",
                    "browser_download_url": "https://example.test/windows-latest.zip",
                },
                {
                    "name": "windows-arm64-latest.zip",
                    "browser_download_url": "https://example.test/windows-arm64-latest.zip",
                },
                {
                    "name": "Vita3K-x86_64.AppImage",
                    "browser_download_url": "https://example.test/Vita3K-x86_64.AppImage",
                },
                {
                    "name": "Vita3K-aarch64.AppImage",
                    "browser_download_url": "https://example.test/Vita3K-aarch64.AppImage",
                },
            ],
        }

        with patch("sys.platform", "win32"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "windows-latest.zip")
        self.assertEqual(resolved["download_url"], "https://example.test/windows-latest.zip")

    def test_source_metadata_vita3k_linux_appimage_asset_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("Vita3K-x86_64.AppImage"))
        source_metadata = {
            "provider": "github-release",
            "owner": "Vita3K",
            "repo": "Vita3K",
            "release_tag": "continuous",
            "asset_patterns": ["windows-latest.zip"],
            "launch_executable": "Vita3K.exe",
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["Vita3K-x86_64.AppImage"],
                    "launch_executable": "Vita3K-x86_64.AppImage",
                },
            },
        }
        release_payload = {
            "tag_name": "continuous",
            "assets": [
                {
                    "name": "windows-latest.zip",
                    "browser_download_url": "https://example.test/windows-latest.zip",
                },
                {
                    "name": "windows-arm64-latest.zip",
                    "browser_download_url": "https://example.test/windows-arm64-latest.zip",
                },
                {
                    "name": "Vita3K-x86_64.AppImage",
                    "browser_download_url": "https://example.test/Vita3K-x86_64.AppImage",
                },
                {
                    "name": "Vita3K-aarch64.AppImage",
                    "browser_download_url": "https://example.test/Vita3K-aarch64.AppImage",
                },
            ],
        }

        with patch("sys.platform", "linux"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "Vita3K-x86_64.AppImage")
        self.assertEqual(resolved["download_url"], "https://example.test/Vita3K-x86_64.AppImage")

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

    def test_source_metadata_direct_download_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("redream.zip"))
        source_metadata = {
            "provider": "direct",
            "owner": "redream.io",
            "repo": "redream",
            "download_url": "https://redream.io/download/redream.x86_64-windows-v1.5.0.zip",
            "asset_name": "redream.x86_64-windows-v1.5.0.zip",
        }

        resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["provider"], "direct")
        self.assertEqual(resolved["asset_name"], "redream.x86_64-windows-v1.5.0.zip")
        self.assertEqual(
            resolved["download_url"],
            "https://redream.io/download/redream.x86_64-windows-v1.5.0.zip",
        )

    def test_source_metadata_direct_page_url_regex_resolution(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("retroarch.zip"))
        source_metadata = {
            "provider": "direct",
            "owner": "buildbot.libretro.com",
            "repo": "retroarch-nightly",
            "page_url": "https://buildbot.libretro.com/nightly/windows/x86_64/",
            "download_url_regex": r'https://buildbot\.libretro\.com/nightly/windows/x86_64/RetroArch\.7z',
            "asset_name": "RetroArch.7z",
        }
        page_payload = self._ResponseStub(
            b'<a href="https://buildbot.libretro.com/nightly/windows/x86_64/RetroArch.7z">RetroArch.7z</a>'
        )

        with patch("grid_launcher.background.workers.urlopen", return_value=page_payload):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["provider"], "direct")
        self.assertEqual(resolved["asset_name"], "RetroArch.7z")
        self.assertEqual(
            resolved["download_url"],
            "https://buildbot.libretro.com/nightly/windows/x86_64/RetroArch.7z",
        )

    def test_source_metadata_direct_page_url_regex_resolution_prefers_redream_nightly_build(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("redream.zip"))
        source_metadata = {
            "provider": "direct",
            "owner": "inolen",
            "repo": "redream",
            "page_url": "https://redream.io/download",
            "download_url_regex": r'https://redream\.io/download/redream\.x86_64-windows-v[0-9.]+-[0-9]+-g[0-9a-f]+\.zip',
        }
        page_payload = self._ResponseStub(
            b'\n'.join(
                [
                    b'<a href="/download/redream.x86_64-windows-v1.5.0.zip">stable</a>',
                    b'<a href="/download/redream.x86_64-windows-v1.5.0-1133-g03c2ae9.zip">nightly</a>',
                ]
            )
        )

        with patch("grid_launcher.background.workers.urlopen", return_value=page_payload):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(
            resolved["download_url"],
            "https://redream.io/download/redream.x86_64-windows-v1.5.0-1133-g03c2ae9.zip",
        )
        self.assertEqual(resolved["asset_name"], "redream.x86_64-windows-v1.5.0-1133-g03c2ae9.zip")

    def test_source_metadata_direct_platform_overrides_resolve_linux_retroarch_asset(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("retroarch.zip"))
        source_metadata = {
            "provider": "direct",
            "owner": "buildbot.libretro.com",
            "repo": "retroarch-nightly",
            "page_url": "https://buildbot.libretro.com/nightly/windows/x86_64/",
            "download_url_regex": r'https://buildbot\.libretro\.com/nightly/windows/x86_64/RetroArch\.7z',
            "asset_name": "RetroArch.7z",
            "platform_overrides": {
                "linux": {
                    "page_url": "https://buildbot.libretro.com/nightly/linux/x86_64/",
                    "download_url_regex": r'https://buildbot\.libretro\.com/nightly/linux/x86_64/RetroArch\.7z',
                    "asset_name": "RetroArch.7z",
                }
            },
        }
        page_payload = self._ResponseStub(
            b'<a href="https://buildbot.libretro.com/nightly/linux/x86_64/RetroArch.7z">RetroArch.7z</a>'
        )

        with patch("sys.platform", "linux"):
            with patch("grid_launcher.background.workers.urlopen", return_value=page_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["provider"], "direct")
        self.assertEqual(resolved["asset_name"], "RetroArch.7z")
        self.assertEqual(
            resolved["download_url"],
            "https://buildbot.libretro.com/nightly/linux/x86_64/RetroArch.7z",
        )

    def test_source_metadata_direct_platform_overrides_resolve_linux_redream_asset(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("redream.zip"))
        source_metadata = {
            "provider": "direct",
            "owner": "inolen",
            "repo": "redream",
            "page_url": "https://redream.io/download",
            "download_url_regex": r'https://redream\.io/download/redream\.x86_64-windows-v[0-9.]+-[0-9]+-g[0-9a-f]+\.zip',
            "platform_overrides": {
                "linux": {
                    "page_url": "https://redream.io/download",
                    "download_url_regex": r'https://redream\.io/download/redream\.x86_64-linux-v[0-9.]+-[0-9]+-g[0-9a-f]+\.tar\.gz',
                }
            },
        }
        page_payload = self._ResponseStub(
            b'\n'.join(
                [
                    b'<a href="/download/redream.x86_64-windows-v1.5.0-1133-g03c2ae9.zip">windows nightly</a>',
                    b'<a href="/download/redream.x86_64-linux-v1.5.0-1133-g03c2ae9.tar.gz">linux nightly</a>',
                ]
            )
        )

        with patch("sys.platform", "linux"):
            with patch("grid_launcher.background.workers.urlopen", return_value=page_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(
            resolved["download_url"],
            "https://redream.io/download/redream.x86_64-linux-v1.5.0-1133-g03c2ae9.tar.gz",
        )
        self.assertEqual(resolved["asset_name"], "redream.x86_64-linux-v1.5.0-1133-g03c2ae9.tar.gz")

    def test_source_metadata_direct_platforms_restriction_raises_on_unsupported_platform(self) -> None:
        worker = InstallDownloadWorker("", {}, Path("dolphin.zip"))
        source_metadata = {
            "provider": "direct",
            "name": "Dolphin",
            "owner": "dolphin-emu",
            "repo": "dolphin-master",
            "page_url": "https://dolphin-emu.org/download/list/master/1/",
            "download_url_regex": r'https://dl\.dolphin-emu\.org/builds/[^\"\s]+/dolphin-master-[0-9-]+-x64\.7z',
            "platforms": ["win32"],
            "manual_install_hint": "Install it via Flatpak instead.",
        }

        with patch("sys.platform", "linux"):
            with self.assertRaises(EmulatorSourceResolutionError):
                worker._resolve_source_download(source_metadata)

    def test_source_metadata_direct_without_platform_overrides_is_platform_independent(self) -> None:
        source_metadata = {
            "provider": "direct",
            "owner": "buildbot.libretro.com",
            "repo": "retroarch-nightly",
            "page_url": "https://buildbot.libretro.com/nightly/windows/x86_64/",
            "download_url_regex": r'https://buildbot\.libretro\.com/nightly/windows/x86_64/RetroArch\.7z',
            "asset_name": "RetroArch.7z",
        }
        page_payload_factory = lambda: self._ResponseStub(
            b'<a href="https://buildbot.libretro.com/nightly/windows/x86_64/RetroArch.7z">RetroArch.7z</a>'
        )

        worker_win = InstallDownloadWorker("", {}, Path("retroarch.zip"))
        with patch("sys.platform", "win32"):
            with patch("grid_launcher.background.workers.urlopen", return_value=page_payload_factory()):
                resolved_win = worker_win._resolve_source_download(source_metadata)

        worker_linux = InstallDownloadWorker("", {}, Path("retroarch.zip"))
        with patch("sys.platform", "linux"):
            with patch("grid_launcher.background.workers.urlopen", return_value=page_payload_factory()):
                resolved_linux = worker_linux._resolve_source_download(source_metadata)

        self.assertEqual(resolved_win["download_url"], resolved_linux["download_url"])
        self.assertEqual(resolved_win["asset_name"], resolved_linux["asset_name"])

    def test_source_metadata_gitea_builds_correct_api_url(self):
        """Worker uses Gitea API URL for gitea provider."""
        captured_urls = []
        worker = InstallDownloadWorker("", {}, Path("eden.zip"))
        source_metadata = {
            "provider": "gitea",
            "base_url": "https://git.example.com",
            "owner": "my-org",
            "repo": "my-repo",
            "release_tag": "latest",
            "asset_patterns": ["MyEmulator-Windows-*-amd64.zip"],
        }
        release_payload = {
            "tag_name": "v1.2.3",
            "assets": [
                {
                    "name": "MyEmulator-Windows-v1.2.3-amd64.zip",
                    "browser_download_url": "https://git.example.com/my-org/my-repo/releases/download/v1.2.3/MyEmulator-Windows-v1.2.3-amd64.zip",
                }
            ],
        }

        def _capture_load_json(url: str, headers: dict[str, str]):
            captured_urls.append(url)
            self.assertEqual(headers, {})
            return release_payload

        with patch.object(worker, "_load_json", side_effect=_capture_load_json):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["provider"], "gitea")
        self.assertTrue(captured_urls)
        self.assertTrue(
            captured_urls[0].startswith("https://git.example.com/api/v1/repos/my-org/my-repo/releases")
        )

    def test_source_metadata_gitea_platform_overrides_resolve_linux_asset(self) -> None:
        """Worker merges gitea platform_overrides so Linux picks the AppImage asset over the Windows zip."""
        worker = InstallDownloadWorker("", {}, Path("eden.zip"))
        source_metadata = {
            "provider": "gitea",
            "base_url": "https://git.eden-emu.dev",
            "owner": "eden-emu",
            "repo": "eden",
            "release_tag": "latest",
            "asset_patterns": ["Eden-Windows-*-amd64-msvc-standard.zip"],
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["Eden-Linux-*-amd64-clang-pgo.AppImage"],
                    "launch_executable": "Eden-Linux-*-amd64-clang-pgo.AppImage",
                }
            },
        }
        release_payload = {
            "tag_name": "v0.2.1",
            "assets": [
                {
                    "name": "Eden-Windows-v0.2.1-amd64-msvc-standard.zip",
                    "browser_download_url": (
                        "https://git.eden-emu.dev/eden-emu/eden/releases/download/"
                        "v0.2.1/Eden-Windows-v0.2.1-amd64-msvc-standard.zip"
                    ),
                },
                {
                    "name": "Eden-Linux-v0.2.1-amd64-clang-pgo.AppImage",
                    "browser_download_url": (
                        "https://git.eden-emu.dev/eden-emu/eden/releases/download/"
                        "v0.2.1/Eden-Linux-v0.2.1-amd64-clang-pgo.AppImage"
                    ),
                },
            ],
        }

        with patch("sys.platform", "linux"):
            with patch.object(worker, "_load_json", return_value=release_payload):
                resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(resolved["asset_name"], "Eden-Linux-v0.2.1-amd64-clang-pgo.AppImage")
        self.assertTrue(resolved["download_url"].endswith("Eden-Linux-v0.2.1-amd64-clang-pgo.AppImage"))

    def test_source_metadata_downloads_supplemental_archives_for_direct_sources(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            initial_archive_path = Path(temp_dir) / "retroarch.zip"
            expected_archive_path = Path(temp_dir) / "retroarch.7z"
            expected_supplemental_path = Path(temp_dir) / "retroarch-supplemental-1.7z"
            worker = InstallDownloadWorker(
                "",
                {},
                initial_archive_path,
                source_metadata={
                    "provider": "direct",
                    "owner": "libretro",
                    "repo": "retroarch-nightly",
                    "download_url": "https://example.test/RetroArch.7z",
                    "asset_name": "RetroArch.7z",
                    "supplemental_downloads": [
                        {
                            "provider": "direct",
                            "owner": "libretro",
                            "repo": "retroarch-cores-nightly",
                            "download_url": "https://example.test/RetroArch_cores.7z",
                            "asset_name": "RetroArch_cores.7z",
                        }
                    ],
                },
            )
            results: list[dict[str, str]] = []
            worker.finished.connect(lambda payload: results.append(payload))

            main_payload = b"retroarch-main"
            supplemental_payload = b"retroarch-cores"

            with patch(
                "grid_launcher.background.workers.urlopen",
                side_effect=[
                    self._ResponseStub(main_payload, content_length=len(main_payload)),
                    self._ResponseStub(supplemental_payload, content_length=len(supplemental_payload)),
                ],
            ):
                worker.run()

            self.assertEqual(results, [{"archive_path": str(expected_archive_path), "error": ""}])
            self.assertTrue(expected_archive_path.exists())
            self.assertEqual(expected_archive_path.read_bytes(), main_payload)
            self.assertTrue(expected_supplemental_path.exists())
            self.assertEqual(expected_supplemental_path.read_bytes(), supplemental_payload)

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
            results: list[dict[str, str]] = []
            worker.finished.connect(lambda payload: results.append(payload))

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
                "grid_launcher.background.workers.urlopen",
                side_effect=[
                    self._ResponseStub(release_payload),
                    self._ResponseStub(archive_payload, content_length=len(archive_payload)),
                ],
            ):
                worker.run()

            self.assertEqual(results, [{"archive_path": str(expected_archive_path), "error": ""}])
            self.assertFalse(initial_archive_path.exists())
            self.assertTrue(expected_archive_path.exists())
            self.assertEqual(expected_archive_path.read_bytes(), archive_payload)

class _FinalizeWindowStub:
    def __init__(self) -> None:
        self.calls: list[tuple[str, object]] = []

    def _prepare_installed_game_without_ui(
        self,
        game: dict[str, str],
        archive_path: Path,
        *,
        cleanup_archive_on_success: bool = True,
        install_progress_callback=None,
    ):
        del game, archive_path, install_progress_callback
        self.calls.append(("prepare", cleanup_archive_on_success))
        return ({"title": "RetroArch", "extracted_dir": "C:/Emulators/RetroArch", "extracted_path": "C:/Emulators/RetroArch/retroarch.exe"}, "")

    def _apply_source_supplemental_archives_without_ui(self, game, archive_path, prepared_game, *, install_progress_callback=None) -> None:
        del game, archive_path, prepared_game, install_progress_callback
        self.calls.append(("supplementals", None))

    def _cleanup_install_archives_without_ui(
        self,
        game,
        archive_path,
        *,
        include_main: bool = True,
        include_supplementals: bool = True,
        install_progress_callback=None,
    ) -> str:
        del game, archive_path, install_progress_callback
        self.calls.append(("cleanup", (include_main, include_supplementals)))
        return ""


class _FinalizeWindowDirectFileStub(_FinalizeWindowStub):
    def _prepare_installed_game_without_ui(
        self,
        game: dict[str, str],
        archive_path: Path,
        *,
        cleanup_archive_on_success: bool = True,
        install_progress_callback=None,
    ):
        del game, archive_path, install_progress_callback
        self.calls.append(("prepare", cleanup_archive_on_success))
        return ({"title": "Gran Turismo 4", "extracted_dir": "", "extracted_path": ""}, "")


class InstallFinalizeWorkerTests(unittest.TestCase):
    def test_worker_defers_archive_cleanup_until_after_supplementals(self) -> None:
        window = _FinalizeWindowStub()
        worker = InstallFinalizeWorker(
            window,
            {"title": "RetroArch", "_install_mode": "source_emulator"},
            Path("retroarch.7z"),
        )
        results: list[dict[str, object]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        worker.run()

        self.assertEqual(
            window.calls,
            [
                ("prepare", False),
                ("cleanup", (True, False)),
                ("supplementals", None),
                ("cleanup", (False, True)),
            ],
        )
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].get("error", ""), "")
        self.assertEqual(results[0].get("warning", ""), "")

    def test_worker_skips_main_cleanup_for_direct_file_installs(self) -> None:
        window = _FinalizeWindowDirectFileStub()
        worker = InstallFinalizeWorker(
            window,
            {"title": "Gran Turismo 4", "_install_mode": "native_game"},
            Path("gran_turismo_4.chd"),
        )
        results: list[dict[str, object]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        worker.run()

        self.assertEqual(
            window.calls,
            [
                ("prepare", False),
                ("supplementals", None),
                ("cleanup", (False, True)),
            ],
        )
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].get("error", ""), "")
        self.assertEqual(results[0].get("warning", ""), "")


class _FinalizeWindowNativePrefixStub:
    def __init__(self, prepared_game: dict[str, str]) -> None:
        self.prepared_game = prepared_game
        self.calls: list[tuple[str, object]] = []

    def _prepare_installed_game_without_ui(
        self,
        game: dict[str, str],
        archive_path: Path,
        *,
        cleanup_archive_on_success: bool = True,
        install_progress_callback=None,
    ):
        del game, archive_path, install_progress_callback
        self.calls.append(("prepare", cleanup_archive_on_success))
        return (dict(self.prepared_game), "")


class InstallFinalizeWorkerNativePrefixTests(unittest.TestCase):
    def test_prefix_placed_in_native_game_dir_on_linux(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            native_game_dir = Path(temp_dir) / "Windows Game"
            extracted_dir = native_game_dir / "Some Game"
            extracted_dir.mkdir(parents=True)
            window = _FinalizeWindowNativePrefixStub(
                {
                    "title": "Some Game",
                    "platform": "Windows",
                    "extracted_dir": str(extracted_dir),
                    "native_game_dir": str(native_game_dir),
                }
            )
            worker = InstallFinalizeWorker(
                window,
                {"title": "Some Game", "platform": "Windows", "_install_mode": "native_game"},
                Path("some_game.zip"),
            )
            results: list[dict[str, object]] = []
            worker.finished.connect(lambda payload: results.append(payload))

            with patch("grid_launcher.background.workers.sys.platform", "linux"):
                worker.run()

            self.assertEqual(len(results), 1)
            prepared_game = results[0].get("game", {})
            expected_prefix = native_game_dir / "prefix"
            self.assertEqual(prepared_game.get("native_wineprefix"), str(expected_prefix))
            self.assertTrue(str(prepared_game.get("native_wineprefix", "")).endswith("/prefix"))
            self.assertTrue(expected_prefix.is_dir())

    def test_prefix_placed_in_extracted_dir_on_linux_when_no_native_game_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            extracted_dir = Path(temp_dir) / "Some Game"
            extracted_dir.mkdir(parents=True)
            window = _FinalizeWindowNativePrefixStub(
                {
                    "title": "Some Game",
                    "platform": "Windows",
                    "extracted_dir": str(extracted_dir),
                }
            )
            worker = InstallFinalizeWorker(
                window,
                {"title": "Some Game", "platform": "Windows", "_install_mode": "native_game"},
                Path("some_game.zip"),
            )
            results: list[dict[str, object]] = []
            worker.finished.connect(lambda payload: results.append(payload))

            with patch("grid_launcher.background.workers.sys.platform", "linux"):
                worker.run()

            self.assertEqual(len(results), 1)
            prepared_game = results[0].get("game", {})
            expected_prefix = extracted_dir / "prefix"
            self.assertEqual(prepared_game.get("native_wineprefix"), str(expected_prefix))
            self.assertTrue(expected_prefix.is_dir())

    def test_no_prefix_on_non_linux(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            extracted_dir = Path(temp_dir) / "Some Game"
            extracted_dir.mkdir(parents=True)
            window = _FinalizeWindowNativePrefixStub(
                {
                    "title": "Some Game",
                    "platform": "Windows",
                    "extracted_dir": str(extracted_dir),
                    "native_game_dir": str(Path(temp_dir)),
                }
            )
            worker = InstallFinalizeWorker(
                window,
                {"title": "Some Game", "platform": "Windows", "_install_mode": "native_game"},
                Path("some_game.zip"),
            )
            results: list[dict[str, object]] = []
            worker.finished.connect(lambda payload: results.append(payload))

            with patch("grid_launcher.background.workers.sys.platform", "win32"):
                worker.run()

            self.assertEqual(len(results), 1)
            prepared_game = results[0].get("game", {})
            self.assertFalse(prepared_game.get("native_wineprefix", ""))
            self.assertFalse((extracted_dir / "prefix").exists())


class TestRetroAchievementsWorker(unittest.TestCase):
    def _run_worker(self, worker):
        results = []
        worker.finished.connect(
            lambda bundle: results.append(
                (
                    bundle.get("request_id", -1) if isinstance(bundle, dict) else -1,
                    bundle.get("achievements", []) if isinstance(bundle, dict) else [],
                    bundle.get("error", "") if isinstance(bundle, dict) else str(bundle),
                )
            )
        )
        worker.run()
        return results

    def test_worker_emits_achievements_on_success(self):
        with patch("grid_launcher.server.retroachievements.fetch_game_achievements") as mock_fetch:
            mock_fetch.return_value = [{"id": 1, "title": "Test"}]
            worker = RetroAchievementsWorker(42, 100, "user", "key")
            results = self._run_worker(worker)
        self.assertEqual(len(results), 1)
        rid, achs, err = results[0]
        self.assertEqual(rid, 42)
        self.assertEqual(achs, [{"id": 1, "title": "Test"}])
        self.assertEqual(err, "")

    def test_worker_emits_error_on_failure(self):
        from grid_launcher.server.retroachievements import RetroAchievementsError

        with patch("grid_launcher.server.retroachievements.fetch_game_achievements") as mock_fetch:
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
        worker.finished.connect(
            lambda bundle: results.append(
                (
                    bundle.get("request_id", -1) if isinstance(bundle, dict) else -1,
                    bundle.get("paths", []) if isinstance(bundle, dict) else [],
                    bundle.get("error", "") if isinstance(bundle, dict) else str(bundle),
                )
            )
        )
        worker.run()
        return results

    def test_pcgamingwiki_worker_emits_paths_on_success(self):
        with patch("grid_launcher.server.pcgamingwiki.fetch_windows_save_paths") as mock_fetch:
            mock_fetch.return_value = ["%APPDATA%\\Game"]
            worker = PCGamingWikiWorker(42, "Game")
            results = self._run_worker(worker)
        self.assertEqual(results, [(42, ["%APPDATA%\\Game"], "")])

    def test_pcgamingwiki_worker_emits_error_on_failure(self):
        with patch("grid_launcher.server.pcgamingwiki.fetch_windows_save_paths") as mock_fetch:
            mock_fetch.side_effect = Exception("timeout")
            worker = PCGamingWikiWorker(9, "Game")
            results = self._run_worker(worker)
        self.assertEqual(results, [(9, [], "timeout")])


class MissingCoverReplenishWorkerTests(unittest.TestCase):
    def _run_worker(self, worker):
        cached: list[tuple[str, str]] = []
        finished: list[bool] = []
        worker.game_cover_cached.connect(
            lambda payload: cached.append((
                payload.get("game_key", "") if isinstance(payload, dict) else "",
                payload.get("path", "") if isinstance(payload, dict) else "",
            ))
        )
        worker.finished.connect(lambda: finished.append(True))
        worker.run()
        return cached, finished

    def test_skips_game_with_empty_cover_url(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            worker = MissingCoverReplenishWorker(
                [("key1", {}, "")],
                {},
                Path(tmp),
            )
            cached, finished = self._run_worker(worker)
        self.assertEqual(cached, [])
        self.assertEqual(len(finished), 1)

    def test_skips_game_with_valid_cached_cover(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            cover_file = Path(tmp) / "existing.png"
            cover_file.write_bytes(b"\x89PNG\r\n\x1a\n" + b"\x00" * 20)
            game = {"cached_cover_path": str(cover_file)}
            with patch("grid_launcher.background.workers.urlopen") as mock_urlopen:
                worker = MissingCoverReplenishWorker(
                    [("key1", game, "https://romm.local/cover.png")],
                    {},
                    Path(tmp),
                )
                cached, finished = self._run_worker(worker)
            mock_urlopen.assert_not_called()
        self.assertEqual(cached, [])
        self.assertEqual(len(finished), 1)

    def test_fetches_and_writes_cover_for_game_with_missing_path(self):
        import tempfile
        from unittest.mock import MagicMock
        png_bytes = b"\x89PNG\r\n\x1a\n" + b"\x00" * 20
        mock_response = MagicMock()
        mock_response.__enter__ = lambda s: s
        mock_response.__exit__ = MagicMock(return_value=False)
        mock_response.read.return_value = png_bytes
        mock_response.headers.get.return_value = "image/png"
        with tempfile.TemporaryDirectory() as tmp:
            game = {"title": "Test Game", "platform": "NES"}
            with patch("grid_launcher.background.workers.urlopen", return_value=mock_response):
                worker = MissingCoverReplenishWorker(
                    [("key1", game, "https://romm.local/cover.png")],
                    {},
                    Path(tmp),
                )
                cached, finished = self._run_worker(worker)
            self.assertEqual(len(cached), 1)
            self.assertEqual(cached[0][0], "key1")
            self.assertTrue(Path(cached[0][1]).exists())
        self.assertEqual(len(finished), 1)

    def test_emits_finished_after_processing_all_games(self):
        import tempfile
        from unittest.mock import MagicMock
        png_bytes = b"\x89PNG\r\n\x1a\n" + b"\x00" * 20
        mock_response = MagicMock()
        mock_response.__enter__ = lambda s: s
        mock_response.__exit__ = MagicMock(return_value=False)
        mock_response.read.return_value = png_bytes
        mock_response.headers.get.return_value = "image/png"
        with tempfile.TemporaryDirectory() as tmp:
            cover_file = Path(tmp) / "existing.png"
            cover_file.write_bytes(b"\x89PNG\r\n\x1a\n" + b"\x00" * 20)
            game_cached = {"cached_cover_path": str(cover_file)}
            game_missing = {"title": "Missing", "platform": "SNES"}
            with patch("grid_launcher.background.workers.urlopen", return_value=mock_response):
                worker = MissingCoverReplenishWorker(
                    [
                        ("key_cached", game_cached, "https://romm.local/a.png"),
                        ("key_missing", game_missing, "https://romm.local/b.png"),
                    ],
                    {},
                    Path(tmp),
                )
                cached, finished = self._run_worker(worker)
        self.assertEqual(len(cached), 1)
        self.assertEqual(len(finished), 1)

    def test_http_error_during_fetch_skips_game_gracefully(self):
        import tempfile
        from urllib.error import HTTPError
        with tempfile.TemporaryDirectory() as tmp:
            game = {"title": "Bad Game", "platform": "GBA"}
            with patch("grid_launcher.background.workers.urlopen", side_effect=HTTPError(
                "https://romm.local/cover.png", 404, "Not Found", {}, None
            )):
                worker = MissingCoverReplenishWorker(
                    [("key1", game, "https://romm.local/cover.png")],
                    {},
                    Path(tmp),
                )
                cached, finished = self._run_worker(worker)
        self.assertEqual(cached, [])
        self.assertEqual(len(finished), 1)


class FlatpakInstallWorkerTests(unittest.TestCase):
    def test_success_emits_empty_error(self) -> None:
        results: list[object] = []
        worker = FlatpakInstallWorker("org.ppsspp.PPSSPP")
        worker.finished.connect(lambda value: results.append(value))

        with patch("grid_launcher.background.workers.subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout="", stderr="")
            worker.run()

        self.assertEqual(results, [{"app_id": "org.ppsspp.PPSSPP", "error": ""}])

    def test_nonzero_returncode_emits_stderr_as_error(self) -> None:
        results: list[dict[str, str]] = []
        worker = FlatpakInstallWorker("org.ppsspp.PPSSPP")
        worker.finished.connect(lambda value: results.append(value))

        with patch("grid_launcher.background.workers.subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=1, stdout="", stderr="error msg")
            worker.run()

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0]["app_id"], "org.ppsspp.PPSSPP")
        self.assertTrue(results[0]["error"])

    def test_oserror_emits_error(self) -> None:
        results: list[dict[str, str]] = []
        worker = FlatpakInstallWorker("org.ppsspp.PPSSPP")
        worker.finished.connect(lambda value: results.append(value))

        with patch(
            "grid_launcher.background.workers.subprocess.run",
            side_effect=OSError("flatpak not found"),
        ):
            worker.run()

        self.assertEqual(len(results), 1)
        self.assertTrue(results[0]["error"])

    def test_uses_correct_cli_args(self) -> None:
        worker = FlatpakInstallWorker("org.ppsspp.PPSSPP")

        with patch("grid_launcher.background.workers.subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout="", stderr="")
            worker.run()

        args, kwargs = mock_run.call_args
        self.assertEqual(
            args[0],
            ["flatpak", "install", "--noninteractive", "flathub", "org.ppsspp.PPSSPP"],
        )
        self.assertNotEqual(kwargs.get("shell"), True)

    def test_uses_custom_flatpak_binary(self) -> None:
        worker = FlatpakInstallWorker("org.ppsspp.PPSSPP", flatpak_binary="/usr/local/bin/flatpak")

        with patch("grid_launcher.background.workers.subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=0, stdout="", stderr="")
            worker.run()

        args, _ = mock_run.call_args
        self.assertEqual(args[0][0], "/usr/local/bin/flatpak")


if __name__ == "__main__":
    unittest.main()
