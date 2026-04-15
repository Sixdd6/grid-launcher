from __future__ import annotations

import subprocess
import time
from pathlib import Path
from typing import Any

from PySide6.QtCore import Property, QObject, QThread, Signal, Slot

from rom_mate.emulator.launch import (
    apply_launch_placeholders_to_args,
    launch_placeholders_for_game,
    normalized_retroarch_core_args,
    prepare_emulator_launch_command,
    resolve_launch_arguments_for_game,
    resolve_rom_path_for_game,
    retroarch_core_argument_path,
    retroarch_core_value,
    split_launch_template_args,
    validate_launch_placeholders,
)
def is_retroarch_emulator_name(emulator_name: str) -> bool:
    return "retroarch" in emulator_name.strip().casefold()


from rom_mate.emulator.selection import is_rpcs3_emulator_name
from rom_mate.emulator.selection import is_arcade_platform, mapping_value_for_platform

# Module-level aliases for test patchability.
_subprocess_popen = subprocess.Popen
_time_sleep = time.sleep

try:
    import psutil as _psutil
except ImportError:
    _psutil = None


class _ProcessWatchThread(QThread):
    def __init__(self, backend: "GameBackend") -> None:
        super().__init__(backend)
        self._backend = backend

    def run(self) -> None:
        process = self._backend._process
        emulator_name = self._backend._active_emulator_name
        if process is None or not emulator_name:
            return

        try:
            process.wait()
        except Exception:
            return

        self._backend._on_process_exited(emulator_name)


class GameBackend(QObject):
    sessionStarted = Signal(str)
    sessionEnded = Signal(str)
    launchError = Signal(str)
    sessionPaused = Signal()
    sessionResumed = Signal()
    pauseRequested = Signal()
    _sessionStateChanged = Signal()

    def __init__(self, config: dict[str, Any], *, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._config = config
        self._active_emulator_name: str = ""
        self._process: subprocess.Popen | None = None
        self._watch_thread: QThread | None = None

    @Property(str, notify=_sessionStateChanged)
    def activeEmulatorName(self) -> str:
        return self._active_emulator_name

    @Property(bool, notify=_sessionStateChanged)
    def isSessionActive(self) -> bool:
        process = self._process
        return bool(process is not None and process.poll() is None)

    @Property(bool, notify=_sessionStateChanged)
    def canPause(self) -> bool:
        process = self._process
        return bool(_psutil is not None and process is not None and process.poll() is None)

    @Slot(object)
    def syncConfig(self, config: dict[str, Any]) -> None:
        self._config = config

    @Slot("QVariant")
    def launchGame(self, game: Any) -> None:
        game_dict = self._normalize_game(game)
        if not game_dict:
            self.launchError.emit("Invalid game payload.")
            return

        if self.isSessionActive:
            self.launchError.emit("A game session is already active.")
            return

        emulators = self._config.get("emulators") or []
        default_emulators = self._config.get("default_emulators") or {}
        launch_args = self._config.get("launch_args", "") or ""
        core_defaults = self._config.get("default_retroarch_cores") or {}

        def default_emulator_name_for_platform(platform: str) -> str:
            if not isinstance(default_emulators, dict):
                return ""
            value = default_emulators.get(platform, "")
            return value.strip() if isinstance(value, str) else ""

        def emulator_entry_by_name(name: str) -> dict[str, str] | None:
            for entry in emulators:
                if isinstance(entry, dict) and entry.get("name", "") == name:
                    return entry
            return None

        def launch_placeholders_for_game_fn(current_game: dict[str, str], emulator_name: str) -> dict[str, str]:
            platform_value = current_game.get("platform", "")
            platform = platform_value.strip() if isinstance(platform_value, str) else ""
            core_value = retroarch_core_value(
                emulator_name,
                platform,
                core_defaults if isinstance(core_defaults, dict) else {},
                is_retroarch_emulator_name,
                mapping_value_for_platform,
                retroarch_core_argument_path,
            )
            ps3_game_id_value = current_game.get("ps3_game_id", "")
            ps3_game_id = ps3_game_id_value.strip() if isinstance(ps3_game_id_value, str) else ""
            rom_path = current_game.get("rom_path", "")
            rom_path_text = rom_path.strip() if isinstance(rom_path, str) else ""
            return launch_placeholders_for_game(
                rom_path_text,
                emulator_name,
                core_value,
                is_rpcs3_emulator_name,
                ps3_game_id,
            )

        def resolved_launch_arguments_for_game(current_game: dict[str, str]) -> tuple[str, list[str]]:
            return resolve_launch_arguments_for_game(
                current_game,
                launch_args,
                default_emulator_name_for_platform,
                emulator_entry_by_name,
                split_launch_template_args,
                launch_placeholders_for_game_fn,
                validate_launch_placeholders,
                apply_launch_placeholders_to_args,
            )

        def candidate_extracted_paths_for_game(current_game: dict[str, str]) -> list[Path]:
            local_path_value = current_game.get("local_path", "")
            local_path = local_path_value.strip() if isinstance(local_path_value, str) else ""
            if local_path:
                return [Path(local_path).expanduser()]
            return []

        def candidate_archive_paths_for_game(current_game: dict[str, str]) -> list[Path]:
            archive_path_value = current_game.get("archive_path", "")
            archive_path = archive_path_value.strip() if isinstance(archive_path_value, str) else ""
            if archive_path:
                return [Path(archive_path).expanduser()]
            return []

        def resolved_rom_path_for_game(current_game: dict[str, str]) -> str:
            return resolve_rom_path_for_game(
                current_game,
                is_arcade_platform,
                candidate_extracted_paths_for_game,
                candidate_archive_paths_for_game,
            )

        try:
            emulator_name, command, cwd = prepare_emulator_launch_command(
                game_dict,
                default_emulator_name_for_platform,
                emulator_entry_by_name,
                resolved_rom_path_for_game,
                resolved_launch_arguments_for_game,
                is_retroarch_emulator_name,
                normalized_retroarch_core_args,
            )
        except ValueError as error:
            self.launchError.emit(str(error))
            return

        try:
            process = _subprocess_popen(command, cwd=cwd, close_fds=True)
        except (OSError, ValueError) as error:
            self.launchError.emit(str(error))
            return

        self._process = process
        self._sessionStateChanged.emit()
        self._active_emulator_name = emulator_name
        self._sessionStateChanged.emit()
        self.sessionStarted.emit(emulator_name)

        watch_thread = _ProcessWatchThread(self)
        self._watch_thread = watch_thread
        watch_thread.finished.connect(watch_thread.deleteLater)
        watch_thread.start()

    @Slot()
    def stopGame(self) -> None:
        process = self._process
        if process is not None and process.poll() is None:
            try:
                process.terminate()
            except OSError:
                pass

        self._active_emulator_name = ""
        self._sessionStateChanged.emit()
        self.sessionEnded.emit("")
        self._process = None
        self._sessionStateChanged.emit()

    @Slot()
    def pauseEmulator(self) -> None:
        process = self._process
        if process is None or process.poll() is not None:
            return
        if _psutil is None:
            return
        try:
            _psutil.Process(process.pid).suspend()
            self.sessionPaused.emit()
        except Exception:
            pass

    @Slot()
    def resumeEmulator(self) -> None:
        process = self._process
        if process is None or process.poll() is not None:
            return
        if _psutil is None:
            return
        try:
            _psutil.Process(process.pid).resume()
            self.sessionResumed.emit()
        except Exception:
            pass

    @Slot()
    def requestPause(self) -> None:
        if self.isSessionActive:
            self.pauseRequested.emit()

    def _on_process_exited(self, emulator_name: str) -> None:
        if self._process is None:
            self._watch_thread = None
            return

        self._process = None
        self._sessionStateChanged.emit()
        self._active_emulator_name = ""
        self._sessionStateChanged.emit()
        self._watch_thread = None
        self.sessionEnded.emit(emulator_name)

    def _normalize_game(self, game: Any) -> dict[str, str]:
        payload = game
        to_variant = getattr(game, "toVariant", None)
        if callable(to_variant):
            try:
                payload = to_variant()
            except Exception:
                return {}

        if not isinstance(payload, dict):
            return {}

        normalized: dict[str, str] = {}
        for key, value in payload.items():
            if not isinstance(key, str):
                continue
            if isinstance(value, str):
                normalized[key] = value
            elif value is None:
                normalized[key] = ""
            else:
                normalized[key] = str(value)
        return normalized
