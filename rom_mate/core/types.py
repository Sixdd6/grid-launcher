from __future__ import annotations

from pathlib import Path
from typing import Protocol


class MainWindowProtocol(Protocol):
    def _prepare_installed_game_without_ui(self, game: dict[str, str], archive_path: Path, configure_ps3_links: bool = False, install_progress_callback=None):
        ...

    def _upload_cloud_files_for_game(self, game: dict[str, str], save_type: str, show_dialogs: bool = False):
        ...

    def _server_save_records_for_rom(self, rom_id: str):
        ...

    def _server_state_records_for_rom(self, rom_id: str):
        ...
