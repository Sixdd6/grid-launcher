from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QCompleter,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFormLayout,
    QFrame,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QMessageBox,
    QPushButton,
    QSizePolicy,
    QVBoxLayout,
    QWidget,
)


def _default_emulator_profiles_path() -> Path:
    return Path(__file__).resolve().parents[2] / "emulator-autoprofiles.json"


def _load_supported_emulator_profiles() -> list[dict[str, str]]:
    profiles_path = _default_emulator_profiles_path()
    if not profiles_path.exists():
        return []

    try:
        parsed = json.loads(profiles_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return []

    if not isinstance(parsed, list):
        return []

    normalized: list[dict[str, str]] = []
    for item in parsed:
        if not isinstance(item, dict):
            continue

        name = item.get("name", "")
        if not isinstance(name, str) or not name.strip():
            continue

        args = item.get("args", "%rom%")
        save_strategy = item.get("save_strategy", "auto")
        ignore_files = item.get("ignore_files", [])
        ignore_extensions = item.get("ignore_extensions", [])
        save_directories = item.get("save_directories", [])
        state_directories = item.get("state_directories", [])

        normalized.append(
            {
                "name": name.strip(),
                "args": args.strip() if isinstance(args, str) and args.strip() else "%rom%",
                "save_strategy": save_strategy.strip()
                if isinstance(save_strategy, str) and save_strategy.strip()
                else "auto",
                "ignore_files": ";".join(
                    value.strip() for value in ignore_files if isinstance(value, str) and value.strip()
                ),
                "ignore_extensions": ";".join(
                    value.strip() for value in ignore_extensions if isinstance(value, str) and value.strip()
                ),
                "save_paths": ";".join(
                    value.strip() for value in save_directories if isinstance(value, str) and value.strip()
                ),
                "state_paths": ";".join(
                    value.strip() for value in state_directories if isinstance(value, str) and value.strip()
                ),
            }
        )

    return normalized


class FirstRunSetupDialog(QDialog):
    def __init__(self, parent: QWidget | None, config: dict[str, Any], message_text: str = "") -> None:
        super().__init__(parent)
        self.setWindowTitle("First Run Setup")
        self.setModal(True)
        self.resize(560, 320)

        server_url = config.get("server_url", "")
        token = config.get("api_token", "")
        library_path = config.get("library_path", "")

        server_url_text = server_url.strip() if isinstance(server_url, str) else ""
        token_text = token.strip() if isinstance(token, str) else ""
        library_path_text = library_path.strip() if isinstance(library_path, str) else ""
        if not library_path_text:
            library_path_text = str(Path.home() / "rom-mate-library")

        layout = QVBoxLayout(self)
        layout.setSpacing(12)

        title = QLabel("Welcome to Rom Mate Neo")
        title.setStyleSheet("font-size: 22px; font-weight: 700;")
        layout.addWidget(title)

        description_text = (
            message_text.strip()
            if isinstance(message_text, str) and message_text.strip()
            else "Set up your server connection and game install folder to continue. You can change these later in Settings."
        )
        description = QLabel(description_text)
        description.setWordWrap(True)
        layout.addWidget(description)

        form = QFormLayout()
        self.server_url_input = QLineEdit(server_url_text)
        form.addRow("Server URL", self.server_url_input)

        self.api_token_input = QLineEdit(token_text)
        self.api_token_input.setEchoMode(QLineEdit.EchoMode.Password)
        form.addRow("Client Token", self.api_token_input)

        self.library_path_input = QLineEdit(library_path_text)
        library_row = QWidget()
        library_row_layout = QHBoxLayout(library_row)
        library_row_layout.setContentsMargins(0, 0, 0, 0)
        library_row_layout.setSpacing(8)
        library_row_layout.addWidget(self.library_path_input)

        browse_button = QPushButton("Browse...")
        browse_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        browse_button.clicked.connect(self._browse_library_path)
        library_row_layout.addWidget(browse_button)
        form.addRow("Library Path", library_row)

        layout.addLayout(form)

        button_box = QDialogButtonBox(QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel)
        ok_button = button_box.button(QDialogButtonBox.StandardButton.Ok)
        if ok_button is not None:
            ok_button.setText("Save and Continue")
        cancel_button = button_box.button(QDialogButtonBox.StandardButton.Cancel)
        if cancel_button is not None:
            cancel_button.setText("Cancel and Exit")
        button_box.accepted.connect(self._accept_if_valid)
        button_box.rejected.connect(self.reject)
        layout.addWidget(button_box)

    def _browse_library_path(self) -> None:
        current_path = self.library_path_input.text().strip()
        selected_directory = QFileDialog.getExistingDirectory(self, "Select Library Folder", current_path)
        if selected_directory:
            self.library_path_input.setText(selected_directory)

    def _accept_if_valid(self) -> None:
        if not self.server_url():
            QMessageBox.warning(self, "Setup Required", "Enter a server URL to continue.")
            return
        if not self.api_token():
            QMessageBox.warning(self, "Setup Required", "Enter a Client Token to continue.")
            return
        if not self.library_path():
            QMessageBox.warning(self, "Setup Required", "Select a library path to continue.")
            return
        self.accept()

    def server_url(self) -> str:
        return self.server_url_input.text().strip()

    def api_token(self) -> str:
        return self.api_token_input.text().strip()

    def library_path(self) -> str:
        return self.library_path_input.text().strip()


class NativeGameSettingsDialog(QDialog):
    def __init__(
        self,
        parent: QWidget | None,
        *,
        game_title: str,
        install_dir: Path,
        executable_candidates: list[Path],
        selected_executable_path: str = "",
        existing_launch_parameters: str = "",
        section_title_factory: callable | None = None,
    ) -> None:
        super().__init__(parent)
        self.setWindowTitle(f"Game Settings - {game_title}")
        self.setModal(True)
        self.resize(700, 300)

        dialog_layout = QVBoxLayout(self)
        form_layout = QFormLayout()

        install_dir_label = QLabel(str(install_dir))
        install_dir_label.setWordWrap(True)
        form_layout.addRow("Install Directory", install_dir_label)

        self.executable_combo = QComboBox()
        self.executable_combo.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        for candidate in executable_candidates:
            try:
                display_name = str(candidate.relative_to(install_dir))
            except ValueError:
                display_name = str(candidate)
            self.executable_combo.addItem(display_name, str(candidate))

        if selected_executable_path:
            selected_index = self.executable_combo.findData(selected_executable_path)
            if selected_index >= 0:
                self.executable_combo.setCurrentIndex(selected_index)
        form_layout.addRow("Executable", self.executable_combo)

        dialog_layout.addLayout(form_layout)

        launch_panel = QFrame()
        launch_panel.setObjectName("panel")
        launch_layout = QVBoxLayout(launch_panel)
        launch_layout.setContentsMargins(12, 10, 12, 10)

        panel_title = "Custom Launch Parameters"
        if callable(section_title_factory):
            launch_layout.addWidget(section_title_factory(panel_title))
        else:
            title_label = QLabel(panel_title)
            title_label.setStyleSheet("font-size: 15px; font-weight: 600;")
            launch_layout.addWidget(title_label)

        custom_launch_form = QFormLayout()
        self.native_launch_parameters_input = QLineEdit()
        self.native_launch_parameters_input.setText(existing_launch_parameters)
        custom_launch_form.addRow("Parameters", self.native_launch_parameters_input)
        launch_layout.addLayout(custom_launch_form)

        launch_hint = QLabel("Arguments are optional and appended when launching this game.")
        launch_hint.setWordWrap(True)
        launch_layout.addWidget(launch_hint)

        dialog_layout.addWidget(launch_panel)

        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Save | QDialogButtonBox.StandardButton.Cancel)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        dialog_layout.addWidget(buttons)

    def selected_executable_path(self) -> str:
        selected_value = self.executable_combo.currentData()
        return selected_value.strip() if isinstance(selected_value, str) else ""

    def launch_parameters(self) -> str:
        return self.native_launch_parameters_input.text().strip()


class EmulatorConfigDialog(QDialog):
    def __init__(
        self,
        parent: QWidget | None,
        *,
        emulator: dict[str, Any] | None = None,
        is_new_entry: bool = False,
        save_strategy_values: list[str] | None = None,
        supported_emulator_profiles: list[dict[str, str]] | None = None,
    ) -> None:
        super().__init__(parent)

        self._is_new_entry = is_new_entry
        self._save_strategy_values = save_strategy_values or ["auto", "single_file", "folder"]
        self._supported_emulator_profiles = (
            supported_emulator_profiles if isinstance(supported_emulator_profiles, list) else _load_supported_emulator_profiles()
        )
        self._profile_by_name = {
            str(profile.get("name", "")).strip(): profile
            for profile in self._supported_emulator_profiles
            if isinstance(profile, dict) and isinstance(profile.get("name", ""), str) and str(profile.get("name", "")).strip()
        }
        self._archive_extensions = {".7z", ".zip", ".rar", ".tar", ".gz", ".bz2", ".xz"}
        self._executable_file_filter = "Executable Files (*.exe *.bat *.cmd *.ps1 *.sh)"
        self._universal_file_filter = (
            "Executable or Archive (*.exe *.bat *.cmd *.ps1 *.sh *.7z *.zip *.rar *.tar *.gz *.bz2 *.xz);;All Files (*)"
        )

        dialog_title = "Add Emulator" if self._is_new_entry else "Edit Emulator"
        self.setWindowTitle(dialog_title)
        self.setModal(True)
        self.resize(760, 430)

        layout = QVBoxLayout(self)
        layout.setSpacing(12)

        form = QFormLayout()

        self.emulator_name_input = QLineEdit()
        self._lock_field_height(self.emulator_name_input)
        form.addRow("Name", self.emulator_name_input)
        if self._is_new_entry:
            self._configure_name_suggestions()
        else:
            self.emulator_name_input.setCompleter(None)

        self.emulator_path_input = QLineEdit()
        self._lock_field_height(self.emulator_path_input)
        path_row = self._path_row(
            self.emulator_path_input,
            browse_text="Browse...",
            browse_handler=self._browse_emulator_path,
        )
        form.addRow("Executable Path", path_row)

        self.emulator_archive_path_input = QLineEdit()
        self._lock_field_height(self.emulator_archive_path_input)
        self.emulator_archive_path_input.setVisible(False)

        self.emulator_args_input = QLineEdit("%rom%")
        self._lock_field_height(self.emulator_args_input)
        form.addRow("Arguments (%rom%, %core%, %RPCS3_GAMEID%, %ps3_gameid%)", self.emulator_args_input)

        self.emulator_save_strategy_input = QComboBox()
        self._lock_field_height(self.emulator_save_strategy_input)
        self.emulator_save_strategy_input.addItems(self._save_strategy_values)
        form.addRow("Save Strategy", self.emulator_save_strategy_input)

        self.emulator_ignore_files_input = QLineEdit()
        self._lock_field_height(self.emulator_ignore_files_input)
        form.addRow("Ignore Files (; separated)", self.emulator_ignore_files_input)

        self.emulator_ignore_extensions_input = QLineEdit()
        self._lock_field_height(self.emulator_ignore_extensions_input)
        form.addRow("Ignore Extensions (; separated)", self.emulator_ignore_extensions_input)

        self.emulator_save_paths_input = QLineEdit()
        self._lock_field_height(self.emulator_save_paths_input)
        form.addRow("Save Dirs (; separated)", self.emulator_save_paths_input)

        self.emulator_state_paths_input = QLineEdit()
        self._lock_field_height(self.emulator_state_paths_input)
        form.addRow("State Dirs (; separated)", self.emulator_state_paths_input)

        layout.addLayout(form)

        button_box = QDialogButtonBox(QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel)
        ok_button = button_box.button(QDialogButtonBox.StandardButton.Ok)
        if ok_button is not None:
            ok_button.setText("Add" if self._is_new_entry else "Save")
        button_box.accepted.connect(self._accept_if_valid)
        button_box.rejected.connect(self.reject)
        layout.addWidget(button_box)

        self._apply_emulator_values(emulator or {})

    @staticmethod
    def _lock_field_height(widget: QLineEdit | QComboBox, height: int = 34) -> None:
        widget.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        widget.setMinimumHeight(height)
        widget.setMaximumHeight(height)

    def _path_row(self, input_widget: QLineEdit, *, browse_text: str, browse_handler: callable | None) -> QWidget:
        row = QWidget()
        row_layout = QHBoxLayout(row)
        row_layout.setContentsMargins(0, 0, 0, 0)
        row_layout.setSpacing(8)
        row_layout.addWidget(input_widget)

        if browse_handler is not None and browse_text.strip():
            browse_button = QPushButton(browse_text)
            browse_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            browse_button.clicked.connect(browse_handler)
            row_layout.addWidget(browse_button)
        return row

    def _configure_name_suggestions(self) -> None:
        suggested_names = sorted(self._profile_by_name.keys())
        completer = QCompleter(suggested_names, self.emulator_name_input)
        completer.setCaseSensitivity(Qt.CaseSensitivity.CaseInsensitive)
        completer.setFilterMode(Qt.MatchFlag.MatchContains)
        completer.activated[str].connect(self._apply_profile_for_suggestion)
        self.emulator_name_input.setCompleter(completer)

    def _apply_profile_for_suggestion(self, suggested_name: str) -> None:
        selected_name = suggested_name.strip()
        if not selected_name:
            return

        self.emulator_name_input.setText(selected_name)
        profile = self._profile_by_name.get(selected_name)
        if not isinstance(profile, dict):
            return

        args = profile.get("args", "")
        save_strategy = profile.get("save_strategy", "")
        ignore_files = profile.get("ignore_files", "")
        ignore_extensions = profile.get("ignore_extensions", "")
        save_paths = profile.get("save_paths", "")
        state_paths = profile.get("state_paths", "")

        if isinstance(args, str) and args.strip():
            self.emulator_args_input.setText(args.strip())

        if isinstance(save_strategy, str) and save_strategy.strip():
            strategy_value = save_strategy.strip()
            if self.emulator_save_strategy_input.findText(strategy_value) < 0:
                self.emulator_save_strategy_input.addItem(strategy_value)
            self.emulator_save_strategy_input.setCurrentText(strategy_value)

        self.emulator_ignore_files_input.setText(ignore_files.strip() if isinstance(ignore_files, str) else "")
        self.emulator_ignore_extensions_input.setText(ignore_extensions.strip() if isinstance(ignore_extensions, str) else "")
        self.emulator_save_paths_input.setText(save_paths.strip() if isinstance(save_paths, str) else "")
        self.emulator_state_paths_input.setText(state_paths.strip() if isinstance(state_paths, str) else "")

    def _browse_emulator_path(self) -> None:
        title = "Select Emulator Executable or Archive" if self._is_new_entry else "Select Emulator Executable"
        file_filter = self._universal_file_filter if self._is_new_entry else self._executable_file_filter
        selected_file, _ = QFileDialog.getOpenFileName(
            self,
            title,
            self.emulator_path_input.text().strip(),
            file_filter,
        )
        if selected_file:
            self._route_selected_path(selected_file)

    def _route_selected_path(self, selected_file: str) -> None:
        selected_suffix = Path(selected_file).suffix.strip().casefold()
        if selected_suffix in self._archive_extensions:
            self.emulator_archive_path_input.setText(selected_file)
            self.emulator_path_input.clear()
            return

        self.emulator_path_input.setText(selected_file)
        self.emulator_archive_path_input.clear()

    def _apply_emulator_values(self, emulator: dict[str, Any]) -> None:
        name = emulator.get("name", "")
        path = emulator.get("path", "")
        archive_path = emulator.get("archive_path", "")
        args = emulator.get("args", "%rom%")
        save_strategy = emulator.get("save_strategy", "auto")
        ignore_files = emulator.get("ignore_files", "")
        ignore_extensions = emulator.get("ignore_extensions", "")
        save_paths = emulator.get("save_paths", "")
        state_paths = emulator.get("state_paths", "")

        self.emulator_name_input.setText(name.strip() if isinstance(name, str) else "")
        self.emulator_path_input.setText(path.strip() if isinstance(path, str) else "")
        self.emulator_archive_path_input.setText(archive_path.strip() if isinstance(archive_path, str) else "")
        self.emulator_args_input.setText(args.strip() if isinstance(args, str) and args.strip() else "%rom%")

        save_strategy_value = save_strategy.strip() if isinstance(save_strategy, str) else "auto"
        if not save_strategy_value:
            save_strategy_value = "auto"
        if self.emulator_save_strategy_input.findText(save_strategy_value) < 0:
            self.emulator_save_strategy_input.addItem(save_strategy_value)
        self.emulator_save_strategy_input.setCurrentText(save_strategy_value)

        self.emulator_ignore_files_input.setText(ignore_files.strip() if isinstance(ignore_files, str) else "")
        self.emulator_ignore_extensions_input.setText(
            ignore_extensions.strip() if isinstance(ignore_extensions, str) else ""
        )
        self.emulator_save_paths_input.setText(save_paths.strip() if isinstance(save_paths, str) else "")
        self.emulator_state_paths_input.setText(state_paths.strip() if isinstance(state_paths, str) else "")

    def _accept_if_valid(self) -> None:
        if not self.emulator_name_input.text().strip():
            QMessageBox.warning(self, "Validation", "Emulator name is required")
            return
        self.accept()

    def entry_payload(self) -> dict[str, str]:
        return {
            "name": self.emulator_name_input.text().strip(),
            "path": self.emulator_path_input.text().strip(),
            "archive_path": self.emulator_archive_path_input.text().strip(),
            "args": self.emulator_args_input.text().strip() or "%rom%",
            "save_strategy": self.emulator_save_strategy_input.currentText().strip() or "auto",
            "ignore_files": self.emulator_ignore_files_input.text().strip(),
            "ignore_extensions": self.emulator_ignore_extensions_input.text().strip(),
            "save_paths": self.emulator_save_paths_input.text().strip(),
            "state_paths": self.emulator_state_paths_input.text().strip(),
        }
