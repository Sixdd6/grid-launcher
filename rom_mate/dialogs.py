from __future__ import annotations

from pathlib import Path
from typing import Any

from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFormLayout,
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
        form.addRow("API Token", self.api_token_input)

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
            QMessageBox.warning(self, "Setup Required", "Enter an API token to continue.")
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
