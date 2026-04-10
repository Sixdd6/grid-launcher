from __future__ import annotations

import json
import platform
import re
import time
import zipfile
from pathlib import Path
from typing import TYPE_CHECKING, Any
from urllib.parse import quote
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from PySide6.QtCore import QObject, Signal

from ..core.api import format_http_error_details
from ..emulator.source import (
    EmulatorSourceResolutionError,
    normalize_emulator_source_metadata,
    resolve_emulator_source_release_asset,
)

if TYPE_CHECKING:
    from ..core.types import MainWindowProtocol


class InstallDownloadWorker(QObject):
    finished = Signal(str, str)
    progress = Signal(object, object, float)

    def __init__(
        self,
        download_url: str,
        headers: dict[str, str],
        archive_path: Path,
        *,
        source_metadata: dict[str, Any] | None = None,
        debug_enabled: bool = False,
    ) -> None:
        super().__init__()
        self.download_url = str(download_url).strip()
        self.headers = dict(headers)
        self.archive_path = archive_path
        self.source_metadata = dict(source_metadata) if isinstance(source_metadata, dict) else None
        self.cancel_requested = False
        self.debug_enabled = bool(debug_enabled)

    def request_cancel(self) -> None:
        self.cancel_requested = True

    def run(self) -> None:
        try:
            resolved_download_url, resolved_archive_path = self._resolved_download_target()
            self.archive_path = resolved_archive_path
            if self.debug_enabled:
                print(f"[DEBUG][InstallDownload] url={resolved_download_url}")
            request = Request(resolved_download_url, headers=self.headers, method="GET")
            with urlopen(request, timeout=60) as response:
                content_length = response.headers.get("Content-Length", "").strip()
                total_bytes = int(content_length) if content_length.isdigit() else 0
                downloaded_bytes = 0
                started_at = time.monotonic()
                with self.archive_path.open("wb") as archive_file:
                    while True:
                        if self.cancel_requested:
                            raise OSError("Download cancelled by user")
                        chunk = response.read(64 * 1024)
                        if not chunk:
                            break
                        archive_file.write(chunk)
                        downloaded_bytes += len(chunk)
                        elapsed = max(time.monotonic() - started_at, 1e-6)
                        speed_bps = downloaded_bytes / elapsed
                        self.progress.emit(downloaded_bytes, total_bytes, speed_bps)
            self.finished.emit(str(self.archive_path), "")
        except HTTPError as error:
            detail = format_http_error_details(error)
            if self.debug_enabled:
                print(f"[DEBUG][InstallDownload] error={detail}")
            if self.archive_path.exists() and self.archive_path.is_file():
                try:
                    self.archive_path.unlink()
                except OSError:
                    pass
            self.finished.emit("", detail)
        except (URLError, OSError, ValueError, OverflowError) as error:
            if self.debug_enabled:
                print(f"[DEBUG][InstallDownload] error={error}")
            if self.archive_path.exists() and self.archive_path.is_file():
                try:
                    self.archive_path.unlink()
                except OSError:
                    pass
            self.finished.emit("", str(error))

    def _resolved_download_target(self) -> tuple[str, Path]:
        if self.download_url:
            return self.download_url, self.archive_path
        if not isinstance(self.source_metadata, dict):
            raise ValueError("No download URL or emulator source metadata was provided.")

        resolved = self._resolve_source_download(self.source_metadata)
        download_url = str(resolved.get("download_url", "")).strip()
        if not download_url:
            raise ValueError("Resolved emulator source metadata did not include a download URL.")
        asset_name = str(resolved.get("asset_name", "")).strip()
        return download_url, self._archive_path_with_asset_suffix(asset_name)

    def _archive_path_with_asset_suffix(self, asset_name: str) -> Path:
        if not asset_name:
            return self.archive_path
        suffix = Path(asset_name).suffix
        if not suffix:
            return self.archive_path
        if self.archive_path.suffix.casefold() == suffix.casefold():
            return self.archive_path
        return self.archive_path.with_suffix(suffix)

    def _resolve_source_download(self, source_metadata: dict[str, Any]) -> dict[str, Any]:
        source = normalize_emulator_source_metadata(source_metadata)
        provider = source.get("provider", "")
        if provider != "github":
            raise EmulatorSourceResolutionError(
                f"Unsupported source provider '{provider}'. Supported providers: github."
            )

        github_headers = self._github_release_headers()
        release_tag = str(source.get("release_tag", "")).strip()
        owner = source["owner"]
        repo = source["repo"]

        if release_tag and release_tag.casefold() != "latest":
            tag_path = quote(release_tag, safe="")
            release_metadata = self._load_json(
                f"https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag_path}",
                github_headers,
            )
        elif release_tag.casefold() == "latest":
            release_metadata = self._load_json(
                f"https://api.github.com/repos/{owner}/{repo}/releases/latest",
                github_headers,
            )
        else:
            release_metadata = self._load_json(
                f"https://api.github.com/repos/{owner}/{repo}/releases",
                github_headers,
            )

        windows_asset = self._resolve_windows_asset_download(source_metadata, release_metadata)
        if windows_asset is not None:
            return {
                "provider": "github",
                "owner": owner,
                "repo": repo,
                "release_tag": windows_asset.get("release_tag", ""),
                "asset_name": windows_asset.get("asset_name", ""),
                "download_url": windows_asset.get("download_url", ""),
            }

        resolved = resolve_emulator_source_release_asset(source, release_metadata)
        return {
            "provider": "github",
            "owner": owner,
            "repo": repo,
            "release_tag": resolved.get("release_tag", ""),
            "asset_name": resolved.get("asset_name", ""),
            "download_url": resolved.get("download_url", ""),
        }

    def _github_release_headers(self) -> dict[str, str]:
        headers = dict(self.headers)
        if not str(headers.get("Accept", "")).strip():
            headers["Accept"] = "application/vnd.github+json"
        if not str(headers.get("X-GitHub-Api-Version", "")).strip():
            headers["X-GitHub-Api-Version"] = "2022-11-28"
        if not str(headers.get("User-Agent", "")).strip():
            headers["User-Agent"] = "rom-mate-neo"
        return headers

    def _load_json(self, url: str, headers: dict[str, str]) -> dict[str, Any] | list[dict[str, Any]]:
        request = Request(url, headers=headers, method="GET")
        with urlopen(request, timeout=60) as response:
            raw = response.read()
        payload = json.loads(raw.decode("utf-8"))
        if isinstance(payload, dict):
            return payload
        if isinstance(payload, list):
            return [item for item in payload if isinstance(item, dict)]
        raise ValueError("Source release API returned an unsupported payload shape.")

    def _resolve_windows_asset_download(
        self,
        source_metadata: dict[str, Any],
        release_metadata: dict[str, Any] | list[dict[str, Any]],
    ) -> dict[str, str] | None:
        has_windows_assets = isinstance(source_metadata, dict) and "windows_assets" in source_metadata
        windows_assets = source_metadata.get("windows_assets") if isinstance(source_metadata, dict) else None
        if not isinstance(windows_assets, list):
            if has_windows_assets:
                raise EmulatorSourceResolutionError("Source metadata 'windows_assets' must be a list when provided.")
            return None

        specs: list[dict[str, str]] = []
        for item in windows_assets:
            if not isinstance(item, dict):
                continue
            specs.append(
                {
                    "arch": str(item.get("arch", "")).strip().casefold(),
                    "asset_name": str(item.get("asset_name", "")).strip(),
                    "asset_name_regex": str(item.get("asset_name_regex", "")).strip(),
                }
            )
        if not specs:
            if has_windows_assets:
                raise EmulatorSourceResolutionError(
                    "Source metadata includes 'windows_assets' but no valid Windows asset spec entries were found."
                )
            return None

        target_arch = self._target_windows_arch(source_metadata)
        if target_arch:
            matching_specs = [spec for spec in specs if spec["arch"] == target_arch]
            if matching_specs:
                specs = matching_specs
            elif has_windows_assets:
                raise EmulatorSourceResolutionError(
                    f"Source metadata includes 'windows_assets' but none matched windows_arch='{target_arch}'."
                )

        release = self._first_release(release_metadata)
        if release is None:
            raise EmulatorSourceResolutionError("No usable GitHub release was found for source metadata.")

        assets_value = release.get("assets", [])
        assets = assets_value if isinstance(assets_value, list) else []
        for spec in specs:
            matched = self._match_release_asset(assets, spec)
            if matched is None:
                continue
            return {
                "release_tag": str(release.get("tag_name", "")).strip(),
                "asset_name": str(matched.get("name", "")).strip(),
                "download_url": str(matched.get("browser_download_url", "")).strip(),
            }

        if has_windows_assets:
            available_assets = [
                str(asset.get("name", "")).strip()
                for asset in assets
                if isinstance(asset, dict) and str(asset.get("name", "")).strip()
            ]
            raise EmulatorSourceResolutionError(
                "Source metadata includes 'windows_assets' but no release asset matched the configured Windows patterns. "
                f"available_assets={available_assets}"
            )

        return None

    def _target_windows_arch(self, source_metadata: dict[str, Any]) -> str:
        explicit_arch = source_metadata.get("windows_arch")
        if isinstance(explicit_arch, str) and explicit_arch.strip():
            normalized_arch = explicit_arch.strip().casefold()
            if normalized_arch in {"x64", "amd64"}:
                return "x64"
            if normalized_arch in {"arm64", "aarch64"}:
                return "arm64"

        machine = platform.machine().strip().casefold()
        if machine in {"arm64", "aarch64"}:
            return "arm64"
        return "x64"

    def _first_release(self, release_metadata: dict[str, Any] | list[dict[str, Any]]) -> dict[str, Any] | None:
        if isinstance(release_metadata, dict):
            return release_metadata
        if isinstance(release_metadata, list) and release_metadata:
            return release_metadata[0]
        return None

    def _match_release_asset(self, assets: list[Any], spec: dict[str, str]) -> dict[str, Any] | None:
        asset_name = spec.get("asset_name", "")
        if asset_name:
            for asset in assets:
                if not isinstance(asset, dict):
                    continue
                candidate_name = str(asset.get("name", "")).strip()
                candidate_url = str(asset.get("browser_download_url", "")).strip()
                if candidate_url and candidate_name.casefold() == asset_name.casefold():
                    return asset

        asset_name_regex = spec.get("asset_name_regex", "")
        if asset_name_regex:
            pattern = re.compile(asset_name_regex, flags=re.IGNORECASE)
            for asset in assets:
                if not isinstance(asset, dict):
                    continue
                candidate_name = str(asset.get("name", "")).strip()
                candidate_url = str(asset.get("browser_download_url", "")).strip()
                if candidate_url and pattern.search(candidate_name):
                    return asset

        return None


class InstallFinalizeWorker(QObject):
    finished = Signal(object, str, str, str)
    progress = Signal(object, object)

    def __init__(
        self,
        window: MainWindowProtocol,
        game: dict[str, str],
        archive_path: Path,
        *,
        content_kind: str = "",
    ) -> None:
        super().__init__()
        self.window = window
        self.game = dict(game)
        self.archive_path = archive_path
        self.content_kind = content_kind.strip().lower()

    def run(self) -> None:
        try:
            if self.content_kind in {"update", "dlc"}:
                prepared_game, warning_text = self.window._apply_ps4_content_archive_without_ui(
                    self.game,
                    self.archive_path,
                    content_kind=self.content_kind,
                    install_progress_callback=self._emit_progress,
                )
            else:
                prepared_game, warning_text = self.window._prepare_installed_game_without_ui(
                    self.game,
                    self.archive_path,
                    configure_ps3_links=False,
                    install_progress_callback=self._emit_progress,
                )
            if prepared_game is None:
                self.finished.emit(None, str(self.archive_path), warning_text, "Install preparation failed")
                return
            self.finished.emit(prepared_game, str(self.archive_path), warning_text, "")
        except (OSError, zipfile.BadZipFile) as error:
            self.finished.emit(None, str(self.archive_path), "", str(error))

    def _emit_progress(self, installed_bytes: int, total_bytes: int) -> None:
        self.progress.emit(max(0, installed_bytes), max(0, total_bytes))


class AutoCloudSaveUploadWorker(QObject):
    finished = Signal(object, object)

    def __init__(
        self,
        window: MainWindowProtocol,
        game: dict[str, str],
        upload_types: list[str],
        local_latest_mtimes: dict[str, float],
    ) -> None:
        super().__init__()
        self.window = window
        self.game = dict(game)
        self.upload_types = [item for item in upload_types if item in {"save", "state"}]
        self.local_latest_mtimes = {
            key: float(value)
            for key, value in local_latest_mtimes.items()
            if key in {"save", "state"} and isinstance(value, (int, float))
        }

    def run(self) -> None:
        try:
            per_type: dict[str, dict[str, Any]] = {}
            for save_type in self.upload_types:
                uploaded_count, total_count, failed_files = self.window._upload_cloud_files_for_game(
                    self.game,
                    save_type,
                    show_dialogs=False,
                )
                per_type[save_type] = {
                    "uploaded_count": uploaded_count,
                    "total_count": total_count,
                    "failed_files": failed_files,
                }

            self.finished.emit(
                self.game,
                {
                    "per_type": per_type,
                    "local_latest_mtimes": self.local_latest_mtimes,
                },
            )
        except (HTTPError, URLError, OSError, ValueError, json.JSONDecodeError) as error:
            self.finished.emit(
                self.game,
                {
                    "per_type": {
                        save_type: {
                            "uploaded_count": 0,
                            "total_count": 0,
                            "failed_files": [str(error)],
                        }
                        for save_type in self.upload_types
                    },
                    "local_latest_mtimes": self.local_latest_mtimes,
                },
            )


class DetailsCloudRecordsWorker(QObject):
    finished = Signal(int, str, object, str)

    def __init__(self, window: MainWindowProtocol, request_id: int, rom_id: str, save_type: str) -> None:
        super().__init__()
        self.window = window
        self.request_id = int(request_id)
        self.rom_id = str(rom_id).strip()
        self.save_type = save_type if save_type in {"save", "state"} else "save"

    def run(self) -> None:
        debug_enabled = False
        debug_enabled_fn = getattr(self.window, "_debug_prints_enabled", None)
        if callable(debug_enabled_fn):
            try:
                debug_enabled = bool(debug_enabled_fn())
            except (TypeError, ValueError):
                debug_enabled = False

        started_at = time.perf_counter()
        if debug_enabled:
            print(
                f"[DEBUG][Timing] enter DetailsCloudRecordsWorker.run request_id={self.request_id} "
                f"save_type={self.save_type} rom_id={self.rom_id}"
            )
        try:
            if self.save_type == "save":
                records = self.window._server_save_records_for_rom(self.rom_id)
            else:
                records = self.window._server_state_records_for_rom(self.rom_id)
            if debug_enabled:
                elapsed_ms = max(0.0, (time.perf_counter() - started_at) * 1000.0)
                print(
                    f"[DEBUG][Timing] exit DetailsCloudRecordsWorker.run elapsed_ms={elapsed_ms:.1f} "
                    f"result=success count={len(records) if isinstance(records, list) else 0}"
                )
            self.finished.emit(self.request_id, self.save_type, records, "")
        except (HTTPError, URLError, OSError, ValueError, json.JSONDecodeError) as error:
            if debug_enabled:
                elapsed_ms = max(0.0, (time.perf_counter() - started_at) * 1000.0)
                print(
                    f"[DEBUG][Timing] exit DetailsCloudRecordsWorker.run elapsed_ms={elapsed_ms:.1f} "
                    f"result=error message={error}"
                )
            self.finished.emit(self.request_id, self.save_type, [], str(error))
