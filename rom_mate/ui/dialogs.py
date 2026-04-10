from __future__ import annotations

from pathlib import Path
from typing import Any

from PySide6.QtWidgets import (
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
