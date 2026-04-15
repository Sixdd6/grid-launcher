from __future__ import annotations

from pathlib import Path
from typing import Any

from PySide6.QtCore import QObject, QThread, Signal, Slot

from rom_mate.core.api import api_get_bytes, api_get_json, api_post_json
from rom_mate.library.cloud_restore import (
    latest_server_records_by_slot,
    relative_timestamp_text,
    restore_single_save_payload,
    save_record_timestamp,
    server_records_from_payload,
)
from rom_mate.server.state import credentials_present, server_base_url

# Module-level aliases for test patchability.
_api_get_json = api_get_json
_api_get_bytes = api_get_bytes
_api_post_json = api_post_json
_server_records_from_payload = server_records_from_payload
_latest_server_records_by_slot = latest_server_records_by_slot
_save_record_timestamp = save_record_timestamp
_relative_timestamp_text = relative_timestamp_text
_restore_single_save_payload = restore_single_save_payload


class _SlotFetchWorker(QObject):
    finished = Signal(str, list)  # save_type, slot_dicts
    error = Signal(str, str)      # save_type, error_message

    def __init__(self, *, config: dict[str, Any], rom_id: str, save_type: str, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._config = config
        self._rom_id = rom_id
        self._save_type = save_type

    @Slot()
    def run(self) -> None:
        try:
            base_url = server_base_url(self._config)
            api_token = self._config.get("api_token", "")
            endpoint = "/api/saves" if self._save_type == "save" else "/api/states"
            raw = _api_get_json(base_url, api_token, endpoint, {"rom_id": self._rom_id})
            records = _server_records_from_payload(raw)
            emulator_name = ""
            slots = _latest_server_records_by_slot(records, emulator_name, _save_record_timestamp)

            slot_dicts: list[dict[str, str]] = []
            for record in slots:
                timestamp = _save_record_timestamp(record)
                slot_dicts.append(
                    {
                        "id": str(record.get("id", "") or ""),
                        "file_name": str(record.get("file_name", "") or ""),
                        "slot": str(record.get("slot", "") or ""),
                        "emulator": str(record.get("emulator", "") or ""),
                        "timestamp_text": _relative_timestamp_text(timestamp),
                        "updated_at": str(record.get("updated_at", "") or ""),
                    }
                )
            self.finished.emit(self._save_type, slot_dicts)
        except Exception as error:
            self.error.emit(self._save_type, str(error))


class CloudBackend(QObject):
    slotsLoaded = Signal(str, list)       # save_type ("save"|"state"), list of slot dicts
    slotsError = Signal(str, str)         # save_type, error message
    restoreComplete = Signal(bool, str)   # success, message
    deleteComplete = Signal(bool, str)    # success, message
    uploadComplete = Signal(bool, str)    # success, message

    def __init__(self, config: dict[str, Any], *, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._config = config
        self._fetch_thread: QThread | None = None
        self._fetch_worker: _SlotFetchWorker | None = None

    @Slot(object)
    def syncConfig(self, config: dict[str, Any]) -> None:
        self._config = config

    @Slot("QVariant", str)
    def loadSlotsForGame(self, game: Any, save_type: str) -> None:
        game_dict = self._normalize_game(game)

        if not credentials_present(self._config):
            self.slotsError.emit(save_type, "Not connected to server.")
            return

        rom_id = str(game_dict.get("rom_id", "") or game_dict.get("id", "")).strip()
        if not rom_id:
            self.slotsError.emit(save_type, "Game has no server ID.")
            return

        self._cancel_fetch_thread()

        worker = _SlotFetchWorker(config=self._config, rom_id=rom_id, save_type=save_type)
        thread = QThread(self)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.finished.connect(self._on_slots_loaded)
        worker.error.connect(self._on_slots_error)
        worker.finished.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        worker.error.connect(lambda *_: thread.quit())
        worker.error.connect(lambda *_: worker.deleteLater())
        thread.finished.connect(thread.deleteLater)

        self._fetch_thread = thread
        self._fetch_worker = worker
        thread.start()

    @Slot(str, str)
    def deleteSlot(self, save_id: str, save_type: str) -> None:
        if not credentials_present(self._config):
            self.deleteComplete.emit(False, "Not connected to server.")
            return

        try:
            base_url = server_base_url(self._config)
            api_token = str(self._config.get("api_token", "") or "")
            save_id_int = int(save_id)

            if save_type == "save":
                endpoint = "/api/saves/delete"
                payload = {"saves": [save_id_int]}
            elif save_type == "state":
                endpoint = "/api/states/delete"
                payload = {"states": [save_id_int]}
            else:
                self.deleteComplete.emit(False, "Unknown save type.")
                return

            _api_post_json(base_url, api_token, endpoint, payload)
            self.deleteComplete.emit(True, "Save deleted.")
        except Exception as error:
            self.deleteComplete.emit(False, str(error))

    @Slot("QVariant", str, str)
    def restoreSlot(self, game: Any, save_id: str, save_type: str) -> None:
        game_dict = self._normalize_game(game)

        try:
            if not credentials_present(self._config):
                self.restoreComplete.emit(False, "Not connected to server.")
                return

            rom_id = str(game_dict.get("rom_id", "") or game_dict.get("id", "")).strip()
            if not rom_id:
                self.restoreComplete.emit(False, "Game has no server ID.")
                return

            base_url = server_base_url(self._config)
            api_token = str(self._config.get("api_token", "") or "")
            payload = _api_get_bytes(base_url, api_token, f"/api/saves/{save_id}/content")

            install_dir_value = game_dict.get("install_dir", "")
            target_dir_str = ""
            if isinstance(install_dir_value, str) and install_dir_value.strip():
                target_dir_str = install_dir_value.strip()
            else:
                local_path_value = game_dict.get("local_path", "")
                if isinstance(local_path_value, str) and local_path_value.strip():
                    target_dir_str = str(Path(local_path_value.strip()).parent)

            if not target_dir_str:
                self.restoreComplete.emit(False, "Cannot determine save location. Use Desktop Mode to restore.")
                return

            target_dir = Path(target_dir_str)
            target_dir.mkdir(parents=True, exist_ok=True)

            game_name_value = game_dict.get("name", "save")
            game_name = game_name_value if isinstance(game_name_value, str) and game_name_value else "save"
            save_record = {"file_name": game_name, "slot": ""}
            _restore_single_save_payload([target_dir], save_record, payload, [], game_name)
            self.restoreComplete.emit(True, "Save restored successfully.")
        except Exception as error:
            self.restoreComplete.emit(False, str(error))

    def _cancel_fetch_thread(self) -> None:
        if self._fetch_thread and self._fetch_thread.isRunning():
            self._fetch_thread.quit()
            self._fetch_thread.wait(2000)
        self._fetch_thread = None
        self._fetch_worker = None

    @Slot(str, list)
    def _on_slots_loaded(self, save_type: str, slots: list) -> None:
        self.slotsLoaded.emit(save_type, slots)
        self._fetch_thread = None
        self._fetch_worker = None

    @Slot(str, str)
    def _on_slots_error(self, save_type: str, error: str) -> None:
        self.slotsError.emit(save_type, error)
        self._fetch_thread = None
        self._fetch_worker = None

    def _normalize_game(self, game: Any) -> dict[str, Any]:
        payload = game
        to_variant = getattr(game, "toVariant", None)
        if callable(to_variant):
            try:
                payload = to_variant()
            except Exception:
                return {}

        if isinstance(payload, dict):
            return payload
        return {}