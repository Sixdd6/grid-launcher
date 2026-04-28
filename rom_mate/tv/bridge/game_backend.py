from __future__ import annotations

import subprocess
import threading
import time
from pathlib import Path
from typing import Any
from urllib.parse import quote

from PySide6.QtCore import Property, QObject, Qt, QThread, Signal, Slot

from rom_mate.core import sanitize_path_component
from rom_mate.core.config import write_config_file as _write_config_file
from rom_mate.emulator.launch import (
    apply_launch_placeholders_to_args,
    launchable_native_game_file,
    launch_placeholders_for_game,
    normalized_retroarch_core_args,
    prepare_native_launch_command,
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
from rom_mate.emulator.selection import (
    is_arcade_platform,
    is_native_executable_platform,
    is_ps3_platform,
    mapping_value_for_platform,
)
from rom_mate.library.archive_preparation import (
    cleanup_install_archive,
    extract_archive_for_game,
    extracted_dir_for_archive_path,
    prepare_installed_game_without_ui,
    select_extracted_launch_file,
    should_extract_archive_for_game,
)
from rom_mate.library.install_paths import (
    native_executable_candidates_for_game,
    native_install_dir_for_game,
    resolved_native_executable_path_for_game,
)
from rom_mate.background.workers import InstallDownloadWorker
from rom_mate.server.state import credentials_present as _credentials_present_fn
from rom_mate.tv.bridge.cloud_helpers import (
    _TvAutoRestoreWorker,
    perform_tv_save_upload,
    resolve_emulator_entry_for_game,
)

# Module-level aliases for test patchability.
_subprocess_popen = subprocess.Popen
_time_sleep = time.sleep
_credentials_present = _credentials_present_fn
_InstallDownloadWorker = InstallDownloadWorker

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


class _TvAutoUploadWorker(QObject):
    finished = Signal(bool, str)

    def __init__(self, config: dict[str, Any], game: dict[str, str], emulator_name: str, emulator_entry: dict[str, str]) -> None:
        super().__init__()
        self._config = config
        self._game = game
        self._emulator_name = emulator_name
        self._emulator_entry = emulator_entry

    @Slot()
    def run(self) -> None:
        try:
            uploaded, total, failed = perform_tv_save_upload(
                self._config,
                self._game,
                self._emulator_name,
                self._emulator_entry,
                "save",
            )
            del failed
            if total == 0:
                self.finished.emit(False, "No save files found.")
            elif uploaded == total:
                self.finished.emit(True, f"Auto-uploaded {uploaded} save file(s).")
            else:
                self.finished.emit(uploaded > 0, f"Auto-uploaded {uploaded}/{total} save file(s).")
        except Exception as exc:
            self.finished.emit(False, str(exc))


class GameBackend(QObject):
    sessionStarted = Signal(str)
    sessionEnded = Signal(str)
    launchError = Signal(str)
    sessionPaused = Signal()
    sessionResumed = Signal()
    pauseRequested = Signal()
    installProgress = Signal(int, int, float)
    installComplete = Signal(bool, str, object)
    uninstallComplete = Signal(bool, str, object)
    cloudSyncStatus = Signal(str)
    nativeExecPickerNeeded = Signal(list)
    _installStateChanged = Signal()
    _sessionStateChanged = Signal()
    _finalizeResult = Signal(object, str, str, str)

    def __init__(self, config: dict[str, Any], main_window: Any, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._config = config
        self._main_window = main_window
        self._active_emulator_name: str = ""
        self._process: subprocess.Popen | None = None
        self._watch_thread: QThread | None = None
        self._install_thread: QThread | None = None
        self._install_worker = None
        self._install_target_game: dict[str, str] | None = None
        self._finalize_thread: threading.Thread | None = None
        self._session_started_at = 0.0
        self._session_game: dict[str, str] | None = None
        self._pending_native_game: dict[str, str] | None = None
        self._restore_thread: QThread | None = None
        self._restore_worker = None
        self._auto_upload_thread: QThread | None = None
        self._auto_upload_worker = None
        self._finalizeResult.connect(
            self._on_install_finalize_done,
            Qt.ConnectionType.QueuedConnection,
        )

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

    @Property(bool, notify=_installStateChanged)
    def isInstallActive(self) -> bool:
        download_active = self._install_thread is not None and self._install_thread.isRunning()
        finalize_active = self._finalize_thread is not None and self._finalize_thread.is_alive()
        return download_active or finalize_active

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

        if is_native_executable_platform(game_dict):
            self._handle_native_launch(game_dict)
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
            rom_path_text = resolved_rom_path_for_game(current_game)
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

        self._session_game = game_dict
        self._session_started_at = time.time()

        if self._restore_thread is not None and self._restore_thread.isRunning():
            self._restore_thread.quit()
            self._restore_thread.wait(500)

        config = dict(self._config)
        if config.get("auto_cloud_save_download_on_launch", True) and _credentials_present(config):
            em_name, em_entry = resolve_emulator_entry_for_game(game_dict, config)
            if em_entry is not None:
                self.cloudSyncStatus.emit("Restoring save…")
                worker = _TvAutoRestoreWorker(config, game_dict, em_name, em_entry)
                thread = QThread(self)
                worker.moveToThread(thread)
                thread.started.connect(worker.run)
                worker.finished.connect(
                    lambda ok, msg, en=emulator_name, cmd=command, d=cwd: self._on_restore_done(ok, msg, en, cmd, d),
                    Qt.ConnectionType.QueuedConnection,
                )
                worker.finished.connect(thread.quit)
                worker.finished.connect(worker.deleteLater)
                thread.finished.connect(thread.deleteLater)
                self._restore_thread = thread
                self._restore_worker = worker
                thread.start()
                return

        self._do_launch(emulator_name, command, cwd)

    def _native_install_dir(self, game_dict: dict[str, str]) -> "Path | None":
        candidate_paths: list[Path] = []
        for key in ("archive_path", "local_path", "extracted_path"):
            val = game_dict.get(key, "")
            if isinstance(val, str) and val.strip():
                candidate_paths.append(Path(val).expanduser())
        return native_install_dir_for_game(game_dict, candidate_paths)

    def _handle_native_launch(self, game_dict: dict[str, str]) -> None:
        install_dir = self._native_install_dir(game_dict)
        if install_dir is None:
            self.launchError.emit("No install directory found for this game.")
            return

        candidates = native_executable_candidates_for_game(install_dir, launchable_native_game_file)
        exe_path = resolved_native_executable_path_for_game(game_dict, candidates, launchable_native_game_file)

        if exe_path is None and not candidates:
            self.launchError.emit("No executable found in the install directory.")
            return

        if exe_path is None:
            candidate_dicts: list[dict[str, str]] = []
            for c in candidates:
                try:
                    label = str(c.relative_to(install_dir))
                except ValueError:
                    label = str(c)
                candidate_dicts.append({"label": label, "path": str(c)})
            self._pending_native_game = game_dict
            self.nativeExecPickerNeeded.emit(candidate_dicts)
            return

        try:
            command, cwd = prepare_native_launch_command(
                game_dict,
                lambda g: exe_path,
                split_launch_template_args,
            )
        except ValueError as err:
            self.launchError.emit(str(err))
            return

        self._session_game = game_dict
        self._session_started_at = time.time()
        self._do_launch("", command, cwd)

    def _do_launch(self, emulator_name: str, command: list[str], cwd: str | None) -> None:
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

    def _on_restore_done(self, success: bool, message: str, emulator_name: str, command: list[str], cwd: str | None) -> None:
        del success, message
        self._restore_thread = None
        self._restore_worker = None
        self.cloudSyncStatus.emit("")
        self._do_launch(emulator_name, command, cwd)

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

    @Slot("QVariant")
    def installGame(self, game: Any) -> None:
        game_dict = self._normalize_game(game)
        if not game_dict:
            self.launchError.emit("Invalid game payload.")
            return

        rom_id = str(game_dict.get("id") or game_dict.get("rom_id") or "").strip()
        if not rom_id:
            self.launchError.emit("Game is missing a ROM id.")
            return

        server_url_value = self._config.get("server_url") or self._config.get("base_url") or ""
        server_url = str(server_url_value).strip()
        if not server_url:
            self.launchError.emit("No server URL configured.")
            return

        if self.isInstallActive:
            self.launchError.emit("An install is already in progress.")
            return

        if self._install_target_game is not None:
            self.installComplete.emit(False, "An install is already in progress.", {})
            return

        library_path_value = self._config.get("library_path", "")
        library_path = str(library_path_value).strip()
        if not library_path:
            self.launchError.emit("No library path configured.")
            return

        api_token = str(self._config.get("api_token", "")).strip()
        headers = {"Authorization": f"Bearer {api_token}"} if api_token else {}
        rom_file_name = str(game_dict.get("rom_file_name", "")).strip()
        if rom_file_name:
            download_url = f"{server_url.rstrip('/')}/api/roms/{rom_id}/content/{quote(rom_file_name)}"
        else:
            download_url = f"{server_url.rstrip('/')}/api/roms/{rom_id}/content"
        platform_name = str(game_dict.get("platform", "")).strip()
        if platform_name:
            platform_dir = Path(library_path).expanduser() / sanitize_path_component(platform_name, "platform")
        else:
            platform_dir = Path(library_path).expanduser()
        try:
            platform_dir.mkdir(parents=True, exist_ok=True)
        except OSError as e:
            self.installComplete.emit(False, str(e), game_dict)
            return
        archive_name = rom_file_name if rom_file_name else f"_tv_download_{rom_id}.tmp"
        archive_path = platform_dir / archive_name

        worker = _InstallDownloadWorker(download_url, headers, archive_path)
        thread = QThread(self)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.progress.connect(self._on_install_progress)
        worker.finished.connect(self._on_install_download_done)
        worker.finished.connect(thread.quit)
        worker.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        self._install_thread = thread
        self._install_worker = worker
        self._install_target_game = dict(game_dict)
        thread.start()
        self._installStateChanged.emit()

    @Slot(object, object, float)
    def _on_install_progress(self, downloaded: int, total: int, speed: float) -> None:
        self.installProgress.emit(downloaded, total, speed)

    @Slot(str, str)
    def _on_install_download_done(self, archive_path: str, error: str) -> None:
        game_dict = self._install_target_game or {}
        self._install_thread = None
        self._install_worker = None
        if error:
            self.installComplete.emit(False, error, game_dict)
            self._install_target_game = None
            self._installStateChanged.emit()
            return

        library_path = str(self._config.get("library_path", "")).strip()
        if not library_path:
            self.installComplete.emit(False, "No library path configured.", game_dict)
            self._install_target_game = None
            self._installStateChanged.emit()
            return

        archive_path_obj = Path(archive_path)
        _game = dict(game_dict)

        # Resolve all required callables on the main thread NOW, before the thread starts.
        # These are all pure-Python functions / method references that do NOT touch Qt objects.
        main_window = self._main_window
        _is_ps3 = main_window._is_ps3_platform
        _ps3_dev_hdd0 = main_window._ps3_dev_hdd0_for_game
        _ps3_games_root = main_window._ps3_games_dir_for_game

        def _select_launch_file(game, exdir, apath):
            return select_extracted_launch_file(game, exdir, apath, is_ps3_platform=_is_ps3)

        def _should_extract(game, apath):
            return should_extract_archive_for_game(
                game, apath,
                is_native_executable_platform=is_native_executable_platform,
                is_arcade_platform=is_arcade_platform,
                is_ps3_platform=_is_ps3,
            )

        def _extract(game, apath, progress_cb=None):
            return extract_archive_for_game(
                game, apath,
                extracted_dir_for_archive_path=extracted_dir_for_archive_path,
                select_extracted_launch_file=_select_launch_file,
                install_progress_callback=progress_cb,
            )

        def _run_finalize() -> None:
            try:
                prepared_game, warning_text = prepare_installed_game_without_ui(
                    _game, archive_path_obj,
                    should_extract_archive_for_game=_should_extract,
                    extract_archive_for_game=_extract,
                    is_ps3_platform=_is_ps3,
                    ps3_dev_hdd0_root=_ps3_dev_hdd0,
                    ps3_games_root=_ps3_games_root,
                    cleanup_archive_on_success=False,
                )
                if prepared_game is None:
                    _error = (warning_text or "").strip() or "Install preparation failed"
                    self._finalizeResult.emit(None, str(archive_path_obj), "", _error)
                    return

                # Cleanup main archive (pure filesystem, no MainWindow)
                if prepared_game.get("extracted_path", ""):
                    _cleanup_err = cleanup_install_archive(archive_path_obj)
                    if _cleanup_err:
                        title = _game.get("title", "Game") or "Game"
                        warning_text = f"Extracted {title}, but could not delete archive:\n{_cleanup_err}"

                # Skip supplemental archives and firmware in TV mode — they are desktop-only features
                # that require MainWindow context and are not needed for basic ROM installs

                self._finalizeResult.emit(prepared_game, str(archive_path_obj), warning_text, "")

            except Exception as _e:
                self._finalizeResult.emit(None, str(archive_path_obj), "", str(_e))

        finalize_thread = threading.Thread(target=_run_finalize, daemon=True)
        self._finalize_thread = finalize_thread
        finalize_thread.start()
        self._installStateChanged.emit()

    @Slot(object, str, str, str)
    def _on_install_finalize_done(self, prepared_game: Any, archive_path: str, warning_text: str, error: str) -> None:
        game_dict = self._install_target_game or {}
        self._finalize_thread = None
        self._install_target_game = None
        if prepared_game is not None and not error:
            installed_game = dict(prepared_game)
            local_path = str(installed_game.get("extracted_path", "")).strip() or archive_path
            installed_game["local_path"] = local_path
            if not installed_game.get("extracted_path", "").strip():
                installed_game["archive_path"] = archive_path
            installed = self._config.setdefault("installed_games", [])
            if not isinstance(installed, list):
                installed = []
                self._config["installed_games"] = installed
            rom_id = str(installed_game.get("id") or installed_game.get("rom_id") or "")
            installed[:] = [
                entry
                for entry in installed
                if isinstance(entry, dict)
                and str(entry.get("id") or entry.get("rom_id") or "") != rom_id
            ]
            installed.append(installed_game)
            self.installComplete.emit(True, "Game installed.", installed_game)
            self._main_window._save_config(self._main_window.config)
        else:
            self.installComplete.emit(False, error or "Install preparation failed.", game_dict)
        self._installStateChanged.emit()

    @Slot("QVariant")
    def uninstallGame(self, game: Any) -> None:
        game_dict = self._normalize_game(game)
        if not game_dict and isinstance(game, dict):
            game_dict = {str(key): str(value) for key, value in game.items() if isinstance(key, str)}

        try:
            removed = self._main_window._uninstall_game(game_dict)
        except Exception as error:
            self.uninstallComplete.emit(False, str(error), game_dict)
            return

        if removed:
            self.uninstallComplete.emit(True, "Game uninstalled.", game_dict)
        else:
            self.uninstallComplete.emit(False, "Game not found in library.", game_dict)

    @Slot(str, result=list)
    def getNativeExecutableCandidates(self, rom_id: str) -> list:
        if not rom_id:
            return []
        installed_games = self._config.get("installed_games", [])
        if not isinstance(installed_games, list):
            return []
        game_dict: dict[str, str] | None = None
        for g in installed_games:
            if isinstance(g, dict) and str(g.get("rom_id", "")) == rom_id:
                game_dict = g
                break
        if game_dict is None:
            return []
        install_dir = self._native_install_dir(game_dict)
        if install_dir is None:
            return []
        candidates = native_executable_candidates_for_game(install_dir, launchable_native_game_file)
        result: list[dict[str, str]] = []
        for c in candidates:
            try:
                label = str(c.relative_to(install_dir))
            except ValueError:
                label = str(c)
            result.append({"label": label, "path": str(c)})
        return result

    @Slot(str, str)
    def saveNativeExecutable(self, rom_id: str, exe_path: str) -> None:
        if not rom_id:
            return
        installed_games = self._config.get("installed_games", [])
        if not isinstance(installed_games, list):
            return
        for g in installed_games:
            if isinstance(g, dict) and str(g.get("rom_id", "")) == rom_id:
                g["native_executable_path"] = exe_path
                break
        config_dir = Path.home() / ".rom-mate"
        config_file = config_dir / "config.json"
        try:
            _write_config_file(config_dir, config_file, self._config)
        except OSError:
            pass

    @Slot(str, str)
    def launchWithNativeExecutable(self, rom_id: str, exe_path: str) -> None:
        self.saveNativeExecutable(rom_id, exe_path)
        installed_games = self._config.get("installed_games", [])
        game_dict: dict[str, str] | None = None
        if isinstance(installed_games, list):
            for g in installed_games:
                if isinstance(g, dict) and str(g.get("rom_id", "")) == rom_id:
                    game_dict = g
                    break
        if game_dict is None:
            self.launchError.emit("Game not found.")
            return
        self._handle_native_launch(game_dict)

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

        if self._session_game is not None:
            config = dict(self._config)
            if config.get("auto_cloud_save_upload_on_exit", True) and _credentials_present(config):
                em_name, em_entry = resolve_emulator_entry_for_game(self._session_game, config)
                if em_entry is not None:
                    worker = _TvAutoUploadWorker(config, self._session_game, em_name, em_entry)
                    thread = QThread(self)
                    worker.moveToThread(thread)
                    thread.started.connect(worker.run)
                    worker.finished.connect(self._on_auto_upload_done, Qt.ConnectionType.QueuedConnection)
                    worker.finished.connect(thread.quit)
                    worker.finished.connect(worker.deleteLater)
                    thread.finished.connect(thread.deleteLater)
                    self._auto_upload_thread = thread
                    self._auto_upload_worker = worker
                    thread.start()

    def _on_auto_upload_done(self, success: bool, message: str) -> None:
        del success
        self._auto_upload_thread = None
        self._auto_upload_worker = None
        if message:
            self.cloudSyncStatus.emit(message)

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
