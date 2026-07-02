from __future__ import annotations

from pathlib import Path
from typing import Protocol


class MainWindowProtocol(Protocol):
    def _prepare_installed_game_without_ui(self, game: dict[str, str], archive_path: Path, *, cleanup_archive_on_success: bool = True, install_progress_callback=None):
        ...

    def _apply_ps4_content_archive_without_ui(
        self,
        installed_game: dict[str, str],
        archive_path: Path,
        *,
        content_kind: str,
        install_progress_callback=None,
    ):
        ...

    def _upload_cloud_files_for_game(self, game: dict[str, str], save_type: str, show_dialogs: bool = False):
        ...

    def _server_save_records_for_rom(self, rom_id: str):
        ...

    def _server_state_records_for_rom(self, rom_id: str):
        ...
