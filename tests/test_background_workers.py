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
    InstallFinalizeWorker,
    MissingCoverReplenishWorker,
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
            with patch("rom_mate.background.workers.urlopen", side_effect=http_error):
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
                "rom_mate.background.workers.urlopen",
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
                "rom_mate.background.workers.urlopen",
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

        with patch("rom_mate.background.workers.urlopen", return_value=page_payload):
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

        with patch("rom_mate.background.workers.urlopen", return_value=page_payload):
            resolved = worker._resolve_source_download(source_metadata)

        self.assertEqual(
            resolved["download_url"],
            "https://redream.io/download/redream.x86_64-windows-v1.5.0-1133-g03c2ae9.zip",
        )
        self.assertEqual(resolved["asset_name"], "redream.x86_64-windows-v1.5.0-1133-g03c2ae9.zip")

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
                "rom_mate.background.workers.urlopen",
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
                "rom_mate.background.workers.urlopen",
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
            with patch("rom_mate.background.workers.urlopen") as mock_urlopen:
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
            with patch("rom_mate.background.workers.urlopen", return_value=mock_response):
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
            with patch("rom_mate.background.workers.urlopen", return_value=mock_response):
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
            with patch("rom_mate.background.workers.urlopen", side_effect=HTTPError(
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


if __name__ == "__main__":
    unittest.main()
