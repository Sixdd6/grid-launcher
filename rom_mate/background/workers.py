from __future__ import annotations

import json
import time
import zipfile
from pathlib import Path
from typing import TYPE_CHECKING, Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from PySide6.QtCore import QObject, Signal

from ..core.api import format_http_error_details

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
        debug_enabled: bool = False,
    ) -> None:
        super().__init__()
        self.download_url = download_url
        self.headers = headers
        self.archive_path = archive_path
        self.cancel_requested = False
        self.debug_enabled = bool(debug_enabled)

    def request_cancel(self) -> None:
        self.cancel_requested = True

    def run(self) -> None:
        try:
            if self.debug_enabled:
                print(f"[DEBUG][InstallDownload] url={self.download_url}")
            request = Request(self.download_url, headers=self.headers, method="GET")
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
