from __future__ import annotations

import json
import platform
import re
import subprocess
import sys
import time
import zipfile
from pathlib import Path
from typing import TYPE_CHECKING, Any
from urllib.parse import quote, urljoin, urlparse
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from PySide6.QtCore import QObject, Signal, Slot

from ..core.api import api_get_json, format_http_error_details
from ..core.path import sanitize_path_component
from ..emulator.selection import is_native_executable_platform
from ..emulator.source import (
    EmulatorSourceResolutionError,
    normalize_emulator_source_metadata,
    resolve_emulator_source_release_asset,
)

if TYPE_CHECKING:
    from ..core.types import MainWindowProtocol


class InstallDownloadWorker(QObject):
    finished = Signal(object)
    progress = Signal(object)

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
            self._download_to_path(resolved_download_url, self.archive_path)
            self._download_supplemental_archives(self.archive_path)
            self.finished.emit({"archive_path": str(self.archive_path), "error": ""})
        except HTTPError as error:
            detail = format_http_error_details(error)
            if self.debug_enabled:
                print(f"[DEBUG][InstallDownload] error={detail}")
            if self.archive_path.exists() and self.archive_path.is_file():
                try:
                    self.archive_path.unlink()
                except OSError:
                    pass
            self.finished.emit({"archive_path": "", "error": detail})
        except (URLError, OSError, ValueError, OverflowError) as error:
            if self.debug_enabled:
                print(f"[DEBUG][InstallDownload] error={error}")
            if self.archive_path.exists() and self.archive_path.is_file():
                try:
                    self.archive_path.unlink()
                except OSError:
                    pass
            self.finished.emit({"archive_path": "", "error": str(error)})

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

    def _download_to_path(self, download_url: str, target_path: Path) -> None:
        request_headers = self._github_release_headers()
        request = Request(download_url, headers=request_headers, method="GET")
        with urlopen(request, timeout=60) as response:
            content_length = response.headers.get("Content-Length", "").strip()
            total_bytes = int(content_length) if content_length.isdigit() else 0
            downloaded_bytes = 0
            started_at = time.monotonic()
            last_emit_at = started_at
            _PROGRESS_EMIT_INTERVAL = 0.1
            with target_path.open("wb") as archive_file:
                while True:
                    if self.cancel_requested:
                        raise OSError("Download cancelled by user")
                    chunk = response.read(64 * 1024)
                    if not chunk:
                        break
                    archive_file.write(chunk)
                    downloaded_bytes += len(chunk)
                    now = time.monotonic()
                    if now - last_emit_at >= _PROGRESS_EMIT_INTERVAL:
                        elapsed = max(now - started_at, 1e-6)
                        speed_bps = downloaded_bytes / elapsed
                        self.progress.emit({"downloaded": downloaded_bytes, "total": total_bytes, "speed": speed_bps})
                        last_emit_at = now

    def _download_supplemental_archives(self, primary_archive_path: Path) -> None:
        supplemental_value = self.source_metadata.get("supplemental_downloads", []) if isinstance(self.source_metadata, dict) else []
        if not isinstance(supplemental_value, list):
            return

        for index, raw_spec in enumerate(supplemental_value, start=1):
            if not isinstance(raw_spec, dict):
                continue
            resolved = self._resolve_source_download(raw_spec)
            download_url = str(resolved.get("download_url", "")).strip()
            if not download_url:
                continue
            asset_name = str(resolved.get("asset_name", "")).strip()
            supplemental_path = self._supplemental_archive_path(primary_archive_path, index, asset_name)
            self._download_to_path(download_url, supplemental_path)

    def _supplemental_archive_path(self, primary_archive_path: Path, index: int, asset_name: str) -> Path:
        suffix = Path(asset_name).suffix or primary_archive_path.suffix or ".zip"
        return primary_archive_path.with_name(f"{primary_archive_path.stem}-supplemental-{index}{suffix}")

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
        platform_overrides = source.get("platform_overrides")
        if isinstance(platform_overrides, dict):
            for platform_key, override in platform_overrides.items():
                if not isinstance(override, dict):
                    continue
                if sys.platform.startswith(str(platform_key)):
                    source = {**source, **override}
                    break

        provider = source.get("provider", "")
        if provider == "direct":
            allowed_platforms = source_metadata.get("platforms")
            if isinstance(allowed_platforms, list) and allowed_platforms:
                if not any(sys.platform.startswith(str(entry)) for entry in allowed_platforms):
                    raise EmulatorSourceResolutionError(
                        f"{source_metadata.get('name', 'This emulator')} has no auto-install source "
                        f"available for this platform. {source_metadata.get('manual_install_hint', '')}".strip()
                    )
            request_headers = self._github_release_headers()
            return self._resolve_direct_source_download(source, request_headers)

        request_headers: dict[str, str]
        if provider == "github":
            owner = source["owner"]
            repo = source["repo"]
            api_base = f"https://api.github.com/repos/{owner}/{repo}"
            request_headers = self._github_release_headers()
        elif provider == "gitea":
            owner = source["owner"]
            repo = source["repo"]
            api_base = f"{source.get('base_url', '').rstrip('/')}/api/v1/repos/{owner}/{repo}"
            request_headers = {}
        else:
            raise EmulatorSourceResolutionError(
                f"Unsupported source provider '{provider}'. Supported providers: github, gitea, direct."
            )

        release_tag = str(source.get("release_tag", "")).strip()

        if release_tag and release_tag.casefold() != "latest":
            tag_path = quote(release_tag, safe="")
            release_metadata = self._load_json(
                f"{api_base}/releases/tags/{tag_path}",
                request_headers,
            )
        elif release_tag.casefold() == "latest":
            release_metadata = self._load_json(
                f"{api_base}/releases/latest",
                request_headers,
            )
        else:
            release_metadata = self._load_json(
                f"{api_base}/releases",
                request_headers,
            )

        windows_asset = self._resolve_windows_asset_download(source_metadata, release_metadata)
        if windows_asset is not None:
            return {
                "provider": provider,
                "owner": owner,
                "repo": repo,
                "release_tag": windows_asset.get("release_tag", ""),
                "asset_name": windows_asset.get("asset_name", ""),
                "download_url": windows_asset.get("download_url", ""),
            }

        resolved = resolve_emulator_source_release_asset(source, release_metadata)
        return {
            "provider": provider,
            "owner": owner,
            "repo": repo,
            "release_tag": resolved.get("release_tag", ""),
            "asset_name": resolved.get("asset_name", ""),
            "download_url": resolved.get("download_url", ""),
        }

    def _resolve_direct_source_download(self, source: dict[str, Any], headers: dict[str, str]) -> dict[str, str]:
        download_url = str(source.get("download_url", "")).strip()
        page_url = str(source.get("page_url", "")).strip()
        download_url_regex = str(source.get("download_url_regex", "")).strip()
        asset_name = str(source.get("asset_name", "")).strip()

        if not download_url and page_url:
            page_text = self._load_text(page_url, headers)
            if download_url_regex:
                pattern = re.compile(download_url_regex, flags=re.IGNORECASE)
                href_matches = re.findall(r'href\s*=\s*["\']([^"\']+)["\']', page_text, flags=re.IGNORECASE)
                for href in href_matches:
                    candidate = href.strip()
                    if not candidate:
                        continue
                    resolved_candidate = urljoin(page_url, candidate)
                    if pattern.search(candidate) or pattern.search(resolved_candidate):
                        download_url = resolved_candidate
                        break

                if not download_url:
                    match = pattern.search(page_text)
                    if match is not None:
                        if match.groups():
                            for group in match.groups():
                                if isinstance(group, str) and group.strip():
                                    download_url = urljoin(page_url, group.strip())
                                    break
                        if not download_url:
                            download_url = urljoin(page_url, match.group(0).strip())
            if not download_url:
                raise EmulatorSourceResolutionError(
                    "Direct source metadata did not resolve a download URL from the configured page. "
                    f"page_url='{page_url}'"
                )

        if not download_url:
            raise EmulatorSourceResolutionError("Direct source metadata did not include a download URL.")

        if not asset_name:
            asset_name = Path(urlparse(download_url).path).name

        return {
            "provider": "direct",
            "owner": str(source.get("owner", "")).strip(),
            "repo": str(source.get("repo", "")).strip(),
            "release_tag": str(source.get("release_tag", "")).strip() or "latest",
            "asset_name": asset_name,
            "download_url": download_url,
        }

    def _github_release_headers(self) -> dict[str, str]:
        headers = dict(self.headers)
        if not str(headers.get("Accept", "")).strip():
            headers["Accept"] = "application/vnd.github+json"
        if not str(headers.get("X-GitHub-Api-Version", "")).strip():
            headers["X-GitHub-Api-Version"] = "2022-11-28"
        if not str(headers.get("User-Agent", "")).strip():
            headers["User-Agent"] = "grid-launcher"
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

    def _load_text(self, url: str, headers: dict[str, str]) -> str:
        request = Request(url, headers=headers, method="GET")
        with urlopen(request, timeout=60) as response:
            raw = response.read()
        return raw.decode("utf-8", errors="ignore")

    def _resolve_windows_asset_download(
        self,
        source_metadata: dict[str, Any],
        release_metadata: dict[str, Any] | list[dict[str, Any]],
    ) -> dict[str, str] | None:
        if not sys.platform.startswith("win32"):
            return None
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


class SourceVersionCheckWorker(QObject):
    finished = Signal(object)

    def __init__(self, source_metadata: dict, installed_tag: str) -> None:
        super().__init__()
        self.source_metadata = source_metadata
        self.installed_tag = installed_tag

    def run(self) -> None:
        try:
            source = normalize_emulator_source_metadata(self.source_metadata)
            provider = str(source.get("provider", "")).strip().casefold()
            print(
                f"[DEBUG] SourceVersionCheckWorker.run() started, provider={provider}, "
                f"installed_tag={self.installed_tag}"
            )

            if provider == "direct":
                print(
                    f"[DEBUG] SourceVersionCheckWorker emitting: installed={self.installed_tag} "
                    f"available=direct error="
                )
                self.finished.emit({"installed_tag": self.installed_tag, "available_tag": "direct", "error": ""})
                return

            owner = source["owner"]
            repo = source["repo"]
            release_tag = str(source.get("release_tag", "")).strip()

            if provider == "github":
                api_base = f"https://api.github.com/repos/{owner}/{repo}"
                request_headers = self._github_release_headers()
                if release_tag and release_tag.casefold() != "latest":
                    tag_path = quote(release_tag, safe="")
                    payload = self._load_json(f"{api_base}/releases/tags/{tag_path}", request_headers)
                else:
                    payload = self._load_json(f"{api_base}/releases/latest", request_headers)
            elif provider == "gitea":
                base_url = str(source.get("base_url", "")).strip().rstrip("/")
                payload = self._load_json(
                    f"{base_url}/api/v1/repos/{owner}/{repo}/releases/latest",
                    {},
                )
            else:
                print(
                    f"[DEBUG] SourceVersionCheckWorker emitting: installed= available= "
                    f"error=Unsupported provider: {provider}"
                )
                self.finished.emit({"installed_tag": "", "available_tag": "", "error": f"Unsupported provider: {provider}"})
                return

            if not isinstance(payload, dict):
                raise ValueError("Source release API returned an unsupported payload shape.")
            resolved_tag = str(payload.get("tag_name", "")).strip()
            if not resolved_tag:
                raise ValueError("Source release API response did not include tag_name.")
            print(
                f"[DEBUG] SourceVersionCheckWorker emitting: installed={self.installed_tag} "
                f"available={resolved_tag} error="
            )
            self.finished.emit({"installed_tag": self.installed_tag, "available_tag": resolved_tag, "error": ""})
        except Exception as error:
            print(
                f"[DEBUG] SourceVersionCheckWorker emitting: installed= available= error={error}"
            )
            self.finished.emit({"installed_tag": "", "available_tag": "", "error": str(error)})

    def _github_release_headers(self) -> dict[str, str]:
        headers: dict[str, str] = {}
        if not str(headers.get("Accept", "")).strip():
            headers["Accept"] = "application/vnd.github+json"
        if not str(headers.get("X-GitHub-Api-Version", "")).strip():
            headers["X-GitHub-Api-Version"] = "2022-11-28"
        if not str(headers.get("User-Agent", "")).strip():
            headers["User-Agent"] = "grid-launcher"
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


class InstallFinalizeWorker(QObject):
    finished = Signal(object)
    progress = Signal(object)

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
            if self.content_kind == "xenia_content":
                prepared_game, warning_text = self.window._apply_xenia_content_archive_without_ui(
                    self.game,
                    self.archive_path,
                    install_progress_callback=self._emit_progress,
                )
            elif self.content_kind in {"update", "dlc"}:
                prepared_game, warning_text = self.window._apply_ps4_content_archive_without_ui(
                    self.game,
                    self.archive_path,
                    content_kind=self.content_kind,
                    install_progress_callback=self._emit_progress,
                )
            elif self.content_kind == "native_update":
                prepared_game, warning_text = self.window._apply_native_game_update_without_ui(
                    self.game,
                    self.archive_path,
                    install_progress_callback=self._emit_progress,
                )
            else:
                prepared_game, warning_text = self.window._prepare_installed_game_without_ui(
                    self.game,
                    self.archive_path,
                    cleanup_archive_on_success=False,
                    install_progress_callback=self._emit_progress,
                )
            if prepared_game is None:
                error_detail = warning_text.strip() if isinstance(warning_text, str) and warning_text.strip() else "Install preparation failed"
                self.finished.emit({"game": None, "archive_path": str(self.archive_path), "warning": "", "error": error_detail})
                return
            if is_native_executable_platform(prepared_game) and sys.platform.startswith("linux"):
                extracted_dir = Path(prepared_game.get("extracted_dir", ""))
                if str(extracted_dir) and extracted_dir.is_dir():
                    native_game_dir = str(prepared_game.get("native_game_dir", "")).strip()
                    if native_game_dir:
                        prefix_dir = Path(native_game_dir) / "prefix"
                    else:
                        prefix_dir = extracted_dir / "prefix"
                    prefix_dir.mkdir(exist_ok=True)
                    prepared_game["native_wineprefix"] = str(prefix_dir)
            install_mode = str(self.game.get("_install_mode", "")).strip().lower()
            if install_mode == "compat_tool":
                install_dir_value = str(self.game.get("_compat_tool_install_dir", "")).strip()
                extracted_dir_value = str(prepared_game.get("extracted_dir", "")).strip()
                if extracted_dir_value:
                    compat_tool_install_path = extracted_dir_value
                elif install_dir_value:
                    safe_name = sanitize_path_component(
                        str(self.game.get("title", "")), "compat-tool"
                    )
                    compat_tool_install_path = str(Path(install_dir_value) / safe_name)
                else:
                    compat_tool_install_path = ""
                if compat_tool_install_path:
                    prepared_game["_compat_tool_install_path"] = compat_tool_install_path
            cleanup_install_archives = getattr(self.window, "_cleanup_install_archives_without_ui", None)
            if callable(cleanup_install_archives):
                if bool(prepared_game.get("extracted_path", "")):
                    cleanup_warning = cleanup_install_archives(
                        self.game,
                        self.archive_path,
                        include_main=True,
                        include_supplementals=False,
                        install_progress_callback=self._emit_progress,
                    )
                    if isinstance(cleanup_warning, str) and cleanup_warning.strip():
                        warning_text = "\n\n".join(part for part in (warning_text.strip(), cleanup_warning.strip()) if part)
            apply_source_supplemental_archives = getattr(self.window, "_apply_source_supplemental_archives_without_ui", None)
            if callable(apply_source_supplemental_archives):
                apply_source_supplemental_archives(
                    self.game,
                    self.archive_path,
                    prepared_game,
                    install_progress_callback=self._emit_progress,
                )
            if callable(cleanup_install_archives):
                cleanup_warning = cleanup_install_archives(
                    self.game,
                    self.archive_path,
                    include_main=False,
                    include_supplementals=True,
                    install_progress_callback=self._emit_progress,
                )
                if isinstance(cleanup_warning, str) and cleanup_warning.strip():
                    warning_text = "\n\n".join(part for part in (warning_text.strip(), cleanup_warning.strip()) if part)
            install_firmware = getattr(self.window, "_install_firmware_for_game_without_ui", None)
            if callable(install_firmware):
                try:
                    firmware_warning = install_firmware(self.game, prepared_game)
                    if isinstance(firmware_warning, str) and firmware_warning.strip():
                        warning_text = "\n\n".join(part for part in (warning_text.strip(), firmware_warning.strip()) if part)
                except Exception as firmware_error:
                    firmware_warning = f"Firmware install error: {firmware_error}"
                    warning_text = "\n\n".join(part for part in (warning_text.strip(), firmware_warning.strip()) if part)
            self.finished.emit({"game": prepared_game, "archive_path": str(self.archive_path), "warning": warning_text, "error": ""})
        except Exception as error:
            self.finished.emit({"game": None, "archive_path": str(self.archive_path), "warning": "", "error": str(error)})

    def _emit_progress(self, installed_bytes: int, total_bytes: int) -> None:
        self.progress.emit({"installed": max(0, installed_bytes), "total": max(0, total_bytes)})


class FlatpakInstallWorker(QObject):
    finished = Signal(object)  # {"app_id": str, "error": str}

    def __init__(self, app_id: str, *, flatpak_binary: str = "flatpak") -> None:
        super().__init__()
        self.app_id = app_id.strip()
        self.flatpak_binary = flatpak_binary.strip() or "flatpak"

    def run(self) -> None:
        try:
            result = subprocess.run(
                [self.flatpak_binary, "install", "--noninteractive", "flathub", self.app_id],
                capture_output=True,
                text=True,
            )
            if result.returncode != 0:
                error_text = (result.stderr.strip() or result.stdout.strip() or
                              f"flatpak install exited with code {result.returncode}")
                self.finished.emit({"app_id": self.app_id, "error": error_text})
            else:
                self.finished.emit({"app_id": self.app_id, "error": ""})
        except OSError as exc:
            self.finished.emit({"app_id": self.app_id, "error": str(exc)})


class AutoCloudSaveUploadWorker(QObject):
    finished = Signal(object)

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
                {
                    "game": self.game,
                    "result": {
                    "per_type": per_type,
                    "local_latest_mtimes": self.local_latest_mtimes,
                },
                },
            )
        except (HTTPError, URLError, OSError, ValueError, json.JSONDecodeError) as error:
            self.finished.emit(
                {
                    "game": self.game,
                    "result": {
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
                },
            )


class DetailsCloudRecordsWorker(QObject):
    finished = Signal(object)

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
            self.finished.emit({"request_id": self.request_id, "save_type": self.save_type, "records": records, "error": ""})
        except (HTTPError, URLError, OSError, ValueError, json.JSONDecodeError) as error:
            if debug_enabled:
                elapsed_ms = max(0.0, (time.perf_counter() - started_at) * 1000.0)
                print(
                    f"[DEBUG][Timing] exit DetailsCloudRecordsWorker.run elapsed_ms={elapsed_ms:.1f} "
                    f"result=error message={error}"
                )
            self.finished.emit({"request_id": self.request_id, "save_type": self.save_type, "records": [], "error": str(error)})


class RomDetailWorker(QObject):
    finished = Signal(object)

    def __init__(self, base_url: str, api_token: str, rom_id: str) -> None:
        super().__init__()
        self.base_url = str(base_url).strip()
        self.api_token = str(api_token).strip()
        self.rom_id = str(rom_id).strip()

    def run(self) -> None:
        try:
            payload = api_get_json(self.base_url, self.api_token, f"/api/roms/{self.rom_id}")
            if not isinstance(payload, dict):
                raise ValueError("ROM detail API returned an unsupported payload shape.")
            self.finished.emit({"rom_id": self.rom_id, "payload": payload, "error": ""})
        except Exception as error:
            self.finished.emit({"rom_id": self.rom_id, "payload": {}, "error": str(error)})


class RetroAchievementsWorker(QObject):
    finished = Signal(object)

    def __init__(self, request_id: int, ra_game_id: int, username: str, api_key: str) -> None:
        super().__init__()
        self.request_id = int(request_id)
        self.ra_game_id = int(ra_game_id)
        self.username = str(username)
        self.api_key = str(api_key)

    def run(self) -> None:
        from grid_launcher.server.retroachievements import fetch_game_achievements, RetroAchievementsError

        try:
            achievements = fetch_game_achievements(self.ra_game_id, self.username, self.api_key)
            self.finished.emit({"request_id": self.request_id, "achievements": achievements, "error": ""})
        except (RetroAchievementsError, ValueError, OSError) as error:
            self.finished.emit({"request_id": self.request_id, "achievements": [], "error": str(error)})


class RALoginWorker(QObject):
    """Worker that authenticates with RetroAchievements and returns a login token."""

    finished = Signal(object)

    def __init__(self, username: str, password: str) -> None:
        super().__init__()
        self._username = username
        self._password = password

    def run(self) -> None:
        from grid_launcher.server.retroachievements import ra_login, RetroAchievementsError

        try:
            result = ra_login(self._username, self._password)
            self.finished.emit({"username": result["username"], "token": result["token"], "error": ""})
        except (RetroAchievementsError, ValueError, OSError) as error:
            self.finished.emit({"username": "", "token": "", "error": str(error)})


class PCGamingWikiWorker(QObject):
    finished = Signal(object)

    def __init__(self, request_id: int, title: str) -> None:
        super().__init__()
        self._request_id = request_id
        self._title = title

    def run(self) -> None:
        try:
            from grid_launcher.server.pcgamingwiki import fetch_windows_save_paths, PCGamingWikiError
            paths = fetch_windows_save_paths(self._title)
            self.finished.emit({"request_id": self._request_id, "paths": paths, "error": ""})
        except Exception as exc:
            self.finished.emit({"request_id": self._request_id, "paths": [], "error": str(exc)})


class MissingCoverReplenishWorker(QObject):
    game_cover_cached = Signal(object)
    finished = Signal()

    def __init__(
        self,
        games: list[tuple[str, dict, str]],
        auth_headers: dict,
        image_cache_dir: Path,
    ) -> None:
        super().__init__()
        self._games = games
        self._auth_headers = auth_headers
        self._image_cache_dir = image_cache_dir

    @Slot()
    def run(self) -> None:
        from grid_launcher.cover.utils import cached_cover_path_from_game, cover_cache_extension_from_payload, installed_cover_cache_key

        for game_key, game_snapshot, cover_url in self._games:
            if not cover_url:
                continue
            cached_path = cached_cover_path_from_game(game_snapshot)
            if cached_path is not None and cached_path.exists() and cached_path.is_file():
                continue
            try:
                headers = dict(self._auth_headers)
                headers.setdefault("Accept", "image/*")
                request = Request(cover_url, headers=headers, method="GET")
                with urlopen(request, timeout=30) as response:
                    payload = response.read()
                    content_type = response.headers.get("Content-Type", "")
            except (HTTPError, URLError, OSError, ValueError, OverflowError):
                continue
            if not payload:
                continue
            extension = cover_cache_extension_from_payload(cover_url, payload, content_type)
            cache_file_name = f"{installed_cover_cache_key(game_snapshot)}{extension}"
            cache_file = self._image_cache_dir / cache_file_name
            try:
                self._image_cache_dir.mkdir(parents=True, exist_ok=True)
                cache_file.write_bytes(payload)
            except OSError:
                continue
            self.game_cover_cached.emit({"game_key": game_key, "path": str(cache_file)})

        self.finished.emit()


class DiscoverLoadWorker(QObject):
    """Background worker for loading Discover tab sections from the server."""

    finished = Signal(object)
    error = Signal(str)

    def __init__(
        self,
        base_url: str,
        api_token: str,
        cache: Any,
        force_refresh: bool,
    ) -> None:
        super().__init__()
        self.base_url = base_url
        self.api_token = api_token
        self.cache = cache
        self.force_refresh = force_refresh

    @Slot()
    def run(self) -> None:
        from ..server.discover import (
            fetch_all_games,
            fetch_games_by_genre,
            filter_games_by_installed,
        )
        result: dict[str, Any] = {}

        # Single call to get games + genre list
        all_games: list[dict[str, Any]] = []
        genres_available: list[str] = []
        try:
            cached = self.cache.get_section("all_games", self.force_refresh)
            if cached is not None:
                all_games = cached.get("games", [])
                genres_available = cached.get("genres", [])
            else:
                all_games, genres_available = fetch_all_games(
                    self.base_url, self.api_token, limit=20
                )
                all_games = filter_games_by_installed(all_games, self.cache.installed_game_keys)
                self.cache.set_section("all_games", {"games": all_games, "genres": genres_available})
        except Exception:
            pass

        if all_games:
            result["all_games"] = {"games": all_games}

        # Per-genre sections (capped at 6)
        if genres_available:
            try:
                cached_genres = self.cache.get_section("genres", self.force_refresh)
                if cached_genres is not None:
                    result["genres"] = cached_genres
                else:
                    games_by_genre: dict[str, list] = {}
                    for genre in genres_available[:6]:
                        try:
                            genre_games = fetch_games_by_genre(
                                self.base_url, self.api_token, genre, limit=10
                            )
                            genre_games = filter_games_by_installed(
                                genre_games, self.cache.installed_game_keys
                            )
                            if genre_games:
                                games_by_genre[genre] = genre_games
                        except Exception:
                            continue
                    if games_by_genre:
                        genres_section = {
                            "genres": list(games_by_genre.keys()),
                            "games_by_genre": games_by_genre,
                        }
                        self.cache.set_section("genres", genres_section)
                        result["genres"] = genres_section
            except Exception:
                pass

        self.finished.emit(result)
