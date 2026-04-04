import sys
import json
import base64
import ctypes
import re
import time
import shlex
import shutil
import subprocess
import zipfile
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import parse_qsl, quote, urlencode, urlsplit, urlunsplit
from urllib.request import Request, urlopen

from PySide6.QtCore import QObject, QThread, QTimer, Qt, QUrl, Signal
from PySide6.QtGui import QDesktopServices, QPixmap
from PySide6.QtNetwork import QNetworkAccessManager, QNetworkReply, QNetworkRequest
from PySide6.QtWidgets import (
    QApplication,
    QComboBox,
    QFileDialog,
    QFrame,
    QFormLayout,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QLineEdit,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QProgressBar,
    QScrollArea,
    QSizePolicy,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)


class InstallDownloadWorker(QObject):
    finished = Signal(str, str)
    progress = Signal(object, object, float)

    def __init__(self, download_url: str, headers: dict[str, str], archive_path: Path) -> None:
        super().__init__()
        self.download_url = download_url
        self.headers = headers
        self.archive_path = archive_path
        self.cancel_requested = False

    def request_cancel(self) -> None:
        self.cancel_requested = True

    def run(self) -> None:
        try:
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
        except (HTTPError, URLError, OSError, ValueError, OverflowError) as error:
            if self.archive_path.exists():
                try:
                    self.archive_path.unlink()
                except OSError:
                    pass
            self.finished.emit("", str(error))


class MainWindow(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("Rom Mate Neo")
        self.resize(1200, 760)

        self.config = self._load_config()
        self.server_url_input: QLineEdit | None = None
        self.api_token_input: QLineEdit | None = None
        self.library_path_input: QLineEdit | None = None
        self.launch_args_input: QLineEdit | None = None
        self.theme_input: QComboBox | None = None
        self.settings_status_label: QLabel | None = None
        self.account_status_label: QLabel | None = None
        self.emulator_list: QListWidget | None = None
        self.emulator_name_input: QLineEdit | None = None
        self.emulator_path_input: QLineEdit | None = None
        self.emulator_args_input: QLineEdit | None = None
        self.default_platform_combo: QComboBox | None = None
        self.default_emulator_combo: QComboBox | None = None
        self.default_core_combo: QComboBox | None = None
        self.default_mapping_list: QListWidget | None = None
        self.details_title_label: QLabel | None = None
        self.details_content_frame: QFrame | None = None
        self.details_cover_label: QLabel | None = None
        self.details_platform_label: QLabel | None = None
        self.details_rating_label: QLabel | None = None
        self.details_description_label: QLabel | None = None
        self.details_screenshot_labels: list[QLabel] = []
        self.details_screenshots_scroll: QScrollArea | None = None
        self.details_primary_button: QPushButton | None = None
        self.details_secondary_button: QPushButton | None = None
        self.server_platforms_list: QListWidget | None = None
        self.server_games_grid: QGridLayout | None = None
        self.server_games_content: QWidget | None = None
        self.library_scroll: QScrollArea | None = None
        self.library_empty_label: QLabel | None = None
        self.server_games_scroll: QScrollArea | None = None
        self.server_status_label: QLabel | None = None
        self.server_platform_ids: dict[str, int] = {}
        self.server_connected = False
        self.server_auto_reconnect = True
        self.cover_cache: dict[str, QPixmap | None] = {}
        self.cover_waiters: dict[str, list[QLabel]] = {}
        self.cover_loading: set[str] = set()
        self.cover_network = QNetworkAccessManager(self)
        self.current_main_page_index = 0
        self.current_details_game: dict[str, str] | None = None
        self.current_details_source = "library"
        self.install_in_progress = False
        self.install_pending_game: dict[str, str] | None = None
        self.install_queue: list[dict[str, str]] = []
        self.install_thread: QThread | None = None
        self.install_worker: InstallDownloadWorker | None = None
        self.download_status_widget: QWidget | None = None
        self.download_count_label: QLabel | None = None
        self.download_progress_bar: QProgressBar | None = None
        self.download_speed_label: QLabel | None = None
        self.downloads_scroll: QScrollArea | None = None
        self.downloads_list_layout: QVBoxLayout | None = None
        self.downloads_empty_label: QLabel | None = None
        self.downloads_refresh_timer = QTimer(self)
        self.downloads_refresh_timer.setSingleShot(True)
        self.downloads_refresh_timer.setInterval(120)
        self.downloads_refresh_timer.timeout.connect(self._refresh_downloads_page)
        self.download_entry_detail_labels: dict[str, QLabel] = {}
        self.active_download_count = 0
        self.active_download_bytes = 0
        self.active_download_total = 0
        self.active_download_speed_bps = 0.0
        self.active_download_entry_id: str | None = None
        self.download_entries: list[dict[str, Any]] = []
        self.library_games = self._normalize_installed_games(self.config.get("installed_games", []))
        self.server_games_by_platform: dict[str, list[dict[str, str]]] = {}
        self.server_rom_payloads: dict[str, dict[str, Any]] = {}
        self.retroarch_compatibility_map: dict[str, list[str]] | None = None

        root = QWidget()
        self.setCentralWidget(root)

        layout = QVBoxLayout(root)
        layout.setContentsMargins(16, 16, 16, 16)
        layout.setSpacing(12)

        nav_row = QHBoxLayout()
        nav_row.setSpacing(8)
        layout.addLayout(nav_row)

        self.stack = QStackedWidget()
        self.stack.addWidget(self._build_library_page())
        self.stack.addWidget(self._build_server_page())
        self.stack.addWidget(self._build_downloads_page())
        self.stack.addWidget(self._build_emulators_page())
        self.stack.addWidget(self._build_settings_page())
        self.stack.addWidget(self._build_game_details_page())
        layout.addWidget(self.stack)

        nav_buttons_by_label: dict[str, QPushButton] = {}
        for index, label in enumerate(("Library", "Server", "Downloads", "Emulators", "Settings")):
            button = QPushButton(label)
            button.setCheckable(True)
            button.clicked.connect(lambda checked, idx=index: self._switch_page(idx))
            button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            nav_buttons_by_label[label] = button

        self.nav_buttons = [
            nav_buttons_by_label["Library"],
            nav_buttons_by_label["Server"],
            nav_buttons_by_label["Downloads"],
            nav_buttons_by_label["Emulators"],
            nav_buttons_by_label["Settings"],
        ]

        nav_row.addWidget(nav_buttons_by_label["Library"])
        nav_row.addWidget(nav_buttons_by_label["Server"])
        nav_row.addWidget(nav_buttons_by_label["Downloads"])

        nav_row.addStretch()

        download_widget = QWidget()
        download_layout = QHBoxLayout(download_widget)
        download_layout.setContentsMargins(8, 4, 8, 4)
        download_layout.setSpacing(8)

        download_count = QLabel("0 active downloads")
        download_count.setStyleSheet("font-weight: 600; color: #cbd5e1;")
        self.download_count_label = download_count
        download_layout.addWidget(download_count)

        download_progress = QProgressBar()
        download_progress.setRange(0, 100)
        download_progress.setValue(0)
        download_progress.setTextVisible(True)
        download_progress.setFormat("0%")
        download_progress.setFixedWidth(220)
        self.download_progress_bar = download_progress
        download_layout.addWidget(download_progress)

        download_speed = QLabel("0 B/s")
        download_speed.setStyleSheet("font-weight: 600; color: #93c5fd;")
        self.download_speed_label = download_speed
        download_layout.addWidget(download_speed)

        download_widget.setObjectName("downloadStatusWidget")
        download_widget.setVisible(False)
        self.download_status_widget = download_widget
        nav_row.addWidget(download_widget, 0, Qt.AlignmentFlag.AlignHCenter)
        nav_row.addStretch()

        self.account_status_label = QLabel()
        self.account_status_label.setStyleSheet("font-weight: 600; color: #cbd5e1;")
        nav_row.addWidget(self.account_status_label)

        nav_row.addWidget(nav_buttons_by_label["Emulators"])
        nav_row.addWidget(nav_buttons_by_label["Settings"])
        self._update_top_bar_identity()
        self._switch_page(0)

        self.setStyleSheet(
            """
            QMainWindow {
                background-color: #111827;
            }
            QLabel {
                color: #e5e7eb;
            }
            QPushButton {
                background-color: #1f2937;
                color: #e5e7eb;
                border: 1px solid #374151;
                border-radius: 8px;
                padding: 8px 14px;
                font-weight: 600;
            }
            QPushButton:hover {
                border-color: #60a5fa;
            }
            QPushButton:pressed {
                background-color: #374151;
                border-color: #60a5fa;
            }
            QPushButton:checked {
                background-color: #2563eb;
                border-color: #3b82f6;
                color: #ffffff;
            }
            QWidget#downloadStatusWidget {
                background-color: #0f172a;
                border: 1px solid #334155;
                border-radius: 8px;
            }
            QProgressBar {
                border: 1px solid #334155;
                border-radius: 6px;
                background-color: #111827;
                color: #e5e7eb;
                text-align: center;
            }
            QProgressBar::chunk {
                background-color: #22c55e;
                border-radius: 5px;
            }
            QPushButton#gameCard {
                text-align: left;
                background-color: #1f2937;
                border: 1px solid #374151;
                border-radius: 10px;
                padding: 0;
            }
            QPushButton#gameCard:hover {
                border-color: #60a5fa;
            }
            QListWidget {
                background-color: #1f2937;
                color: #e5e7eb;
                border: 1px solid #374151;
                border-radius: 8px;
                padding: 4px;
            }
            QLineEdit, QComboBox {
                background-color: #111827;
                color: #e5e7eb;
                border: 1px solid #4b5563;
                border-radius: 6px;
                padding: 6px 8px;
            }
            QFrame#panel {
                background-color: #1f2937;
                border: 1px solid #374151;
                border-radius: 10px;
            }
            QScrollArea#libraryScroll,
            QScrollArea#serverGamesScroll,
            QScrollArea#downloadsScroll {
                background-color: transparent;
                border: none;
            }
            QWidget#libraryScrollViewport,
            QWidget#serverGamesScrollViewport,
            QWidget#downloadsScrollViewport,
            QWidget#libraryGridContent,
            QWidget#serverGamesContent,
            QWidget#downloadsContent {
                background-color: transparent;
            }
            QListWidget#serverPlatformsList {
                background-color: transparent;
            }
            """
        )

        self._refresh_library_grid()
        self._refresh_emulator_views()
        self._connect_to_server(show_errors=False)

    def _switch_page(self, index: int) -> None:
        previous_index = self.stack.currentIndex()
        if previous_index == 5 and index != 5:
            self._cleanup_details_view_state()
        self.stack.setCurrentIndex(index)
        self.current_main_page_index = index
        for i, button in enumerate(self.nav_buttons):
            button.setChecked(i == index)
        if index == 1 and not self.server_connected and self.server_auto_reconnect:
            self._connect_to_server(show_errors=False)
        QTimer.singleShot(0, self._reflow_current_page_grid)

    def _reflow_current_page_grid(self) -> None:
        current_index = self.stack.currentIndex()
        if current_index == 0:
            self._refresh_library_grid()
            return
        if current_index == 1 and self.server_platforms_list is not None:
            selected_item = self.server_platforms_list.currentItem()
            if selected_item is not None:
                self._render_server_games(selected_item.text())

    def _build_library_page(self) -> QWidget:
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.setSpacing(10)

        header = QLabel("Installed Games")
        header.setStyleSheet("font-size: 20px; font-weight: 700;")
        layout.addWidget(header)

        content_frame = QFrame()
        content_frame.setObjectName("panel")
        content_layout = QVBoxLayout(content_frame)
        content_layout.setContentsMargins(12, 10, 12, 10)
        content_layout.setSpacing(10)

        empty_label = QLabel("No games installed...")
        empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        empty_label.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
        empty_label.setStyleSheet("color: #9ca3af; font-size: 16px; font-weight: 600;")
        self.library_empty_label = empty_label
        content_layout.addWidget(empty_label)

        scroll = QScrollArea()
        scroll.setObjectName("libraryScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.viewport().setObjectName("libraryScrollViewport")
        self.library_scroll = scroll

        content = QWidget()
        content.setObjectName("libraryGridContent")
        grid = QGridLayout(content)
        grid.setSpacing(12)
        grid.setContentsMargins(8, 8, 8, 8)
        self.library_grid = grid

        scroll.setWidget(content)
        content_layout.addWidget(scroll)
        layout.addWidget(content_frame)
        return page

    def _build_server_page(self) -> QWidget:
        page = QWidget()
        layout = QHBoxLayout(page)
        layout.setSpacing(12)

        platforms_frame = QFrame()
        platforms_frame.setObjectName("panel")
        platforms_layout = QVBoxLayout(platforms_frame)
        platforms_layout.setContentsMargins(10, 10, 10, 10)

        platforms = QListWidget()
        platforms.setObjectName("serverPlatformsList")
        platforms.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Expanding)
        platforms.setMinimumWidth(120)
        platforms.setMaximumWidth(220)
        platforms.currentTextChanged.connect(self._on_server_platform_selected)
        self.server_platforms_list = platforms
        platforms_layout.addWidget(platforms)
        layout.addWidget(platforms_frame, 0)

        right = QFrame()
        right.setObjectName("panel")
        right_layout = QVBoxLayout(right)
        right_layout.setContentsMargins(12, 10, 12, 10)
        right_layout.setSpacing(10)

        header = QLabel("Server Games")
        header.setStyleSheet("font-size: 20px; font-weight: 700;")
        right_layout.addWidget(header)

        self.server_status_label = QLabel("Not connected")
        self.server_status_label.setStyleSheet("color: #fca5a5;")
        right_layout.addWidget(self.server_status_label)

        scroll = QScrollArea()
        scroll.setObjectName("serverGamesScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.viewport().setObjectName("serverGamesScrollViewport")
        self.server_games_scroll = scroll

        content = QWidget()
        content.setObjectName("serverGamesContent")
        grid = QGridLayout(content)
        grid.setSpacing(12)
        grid.setContentsMargins(8, 8, 8, 8)
        self.server_games_grid = grid
        self.server_games_content = content

        scroll.setWidget(content)
        right_layout.addWidget(scroll)

        layout.addWidget(right, 1)
        return page

    def _build_downloads_page(self) -> QWidget:
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.setSpacing(10)

        header = QLabel("Downloads")
        header.setStyleSheet("font-size: 20px; font-weight: 700;")
        layout.addWidget(header)

        content_frame = QFrame()
        content_frame.setObjectName("panel")
        content_layout = QVBoxLayout(content_frame)
        content_layout.setContentsMargins(12, 10, 12, 10)
        content_layout.setSpacing(10)

        empty_label = QLabel("No downloads yet.")
        empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        empty_label.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
        empty_label.setStyleSheet("color: #9ca3af; font-size: 16px; font-weight: 600;")
        self.downloads_empty_label = empty_label
        content_layout.addWidget(empty_label)

        scroll = QScrollArea()
        scroll.setObjectName("downloadsScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.viewport().setObjectName("downloadsScrollViewport")
        self.downloads_scroll = scroll

        scroll_content = QWidget()
        scroll_content.setObjectName("downloadsContent")
        list_layout = QVBoxLayout(scroll_content)
        list_layout.setContentsMargins(0, 0, 0, 0)
        list_layout.setSpacing(10)
        self.downloads_list_layout = list_layout

        scroll.setWidget(scroll_content)
        content_layout.addWidget(scroll)
        layout.addWidget(content_frame)

        self._refresh_downloads_page()
        return page

    def _build_emulators_page(self) -> QWidget:
        page = QWidget()
        layout = QHBoxLayout(page)
        layout.setSpacing(12)

        left_panel = QFrame()
        left_panel.setObjectName("panel")
        left_layout = QVBoxLayout(left_panel)
        left_layout.setContentsMargins(12, 10, 12, 10)
        left_layout.addWidget(self._make_section_title("Installed Emulators"))

        emulator_list = QListWidget()
        emulator_list.currentRowChanged.connect(self._load_emulator_from_selection)
        self.emulator_list = emulator_list
        left_layout.addWidget(emulator_list)

        launch_emulator_list_button = QPushButton("Launch Selected")
        launch_emulator_list_button.clicked.connect(self._launch_selected_emulator)
        left_layout.addWidget(launch_emulator_list_button)

        layout.addWidget(left_panel, 1)

        right_column = QWidget()
        right_column_layout = QVBoxLayout(right_column)
        right_column_layout.setContentsMargins(0, 0, 0, 0)
        right_column_layout.setSpacing(12)

        emulator_details_panel = QFrame()
        emulator_details_panel.setObjectName("panel")
        emulator_details_layout = QVBoxLayout(emulator_details_panel)
        emulator_details_layout.setContentsMargins(12, 10, 12, 10)
        emulator_details_layout.addWidget(self._make_section_title("Emulator Details"))

        details_form = QFormLayout()
        self.emulator_name_input = QLineEdit()
        self.emulator_path_input = QLineEdit()
        self.emulator_args_input = QLineEdit("%rom%")

        path_row = QWidget()
        path_row_layout = QHBoxLayout(path_row)
        path_row_layout.setContentsMargins(0, 0, 0, 0)
        path_row_layout.setSpacing(8)
        path_row_layout.addWidget(self.emulator_path_input)

        browse_emulator_path_button = QPushButton("Browse...")
        browse_emulator_path_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        browse_emulator_path_button.clicked.connect(self._browse_emulator_path)
        path_row_layout.addWidget(browse_emulator_path_button)

        details_form.addRow("Name", self.emulator_name_input)
        details_form.addRow("Executable Path", path_row)
        details_form.addRow("Arguments (%rom%, %core%)", self.emulator_args_input)
        emulator_details_layout.addLayout(details_form)

        emulator_actions = QHBoxLayout()
        save_emulator_button = QPushButton("Add / Update")
        save_emulator_button.clicked.connect(self._save_emulator)
        emulator_actions.addWidget(save_emulator_button)

        clear_selection_button = QPushButton("Clear Selection")
        clear_selection_button.clicked.connect(self._clear_emulator_selection)
        emulator_actions.addWidget(clear_selection_button)

        remove_emulator_button = QPushButton("Remove")
        remove_emulator_button.clicked.connect(self._remove_emulator)
        emulator_actions.addWidget(remove_emulator_button)
        emulator_actions.addStretch()
        emulator_details_layout.addLayout(emulator_actions)
        right_column_layout.addWidget(emulator_details_panel)

        defaults_panel = QFrame()
        defaults_panel.setObjectName("panel")
        defaults_layout = QVBoxLayout(defaults_panel)
        defaults_layout.setContentsMargins(12, 10, 12, 10)
        defaults_layout.addWidget(self._make_section_title("Default Emulator by Platform"))

        default_form = QFormLayout()
        self.default_platform_combo = QComboBox()
        self.default_emulator_combo = QComboBox()
        self.default_core_combo = QComboBox()
        self.default_emulator_combo.setMaximumWidth(220)
        self.default_core_combo.setMaximumWidth(220)
        self.default_platform_combo.currentTextChanged.connect(self._on_default_platform_changed)
        self.default_emulator_combo.currentTextChanged.connect(self._refresh_retroarch_core_options)
        default_form.addRow("Platform", self.default_platform_combo)
        default_form.addRow("Emulator", self.default_emulator_combo)
        default_form.addRow("Core", self.default_core_combo)
        defaults_layout.addLayout(default_form)

        set_default_button = QPushButton("Set Default")
        set_default_button.clicked.connect(self._set_default_emulator)
        defaults_layout.addWidget(set_default_button)

        self.default_mapping_list = QListWidget()
        defaults_layout.addWidget(self.default_mapping_list)

        right_column_layout.addWidget(defaults_panel)
        right_column_layout.addStretch()
        layout.addWidget(right_column, 2)
        return page

    def _build_settings_page(self) -> QWidget:
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.setSpacing(12)

        header = QLabel("Settings")
        header.setStyleSheet("font-size: 20px; font-weight: 700;")
        layout.addWidget(header)

        server_panel = QFrame()
        server_panel.setObjectName("panel")
        server_layout = QVBoxLayout(server_panel)
        server_layout.setContentsMargins(12, 10, 12, 10)
        server_layout.addWidget(self._make_section_title("Server Connection"))

        server_form = QFormLayout()
        self.server_url_input = QLineEdit(self.config["server_url"])
        self.api_token_input = QLineEdit(self.config["api_token"])
        self.api_token_input.setEchoMode(QLineEdit.EchoMode.Password)
        server_form.addRow("Server URL", self.server_url_input)
        server_form.addRow("API Token", self.api_token_input)
        server_layout.addLayout(server_form)

        connect_button = QPushButton("Connect")
        connect_button.clicked.connect(self._connect_from_settings)
        connect_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)

        disconnect_button = QPushButton("Disconnect")
        disconnect_button.clicked.connect(self._disconnect_from_server)
        disconnect_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)

        connection_actions = QHBoxLayout()
        connection_actions.addWidget(connect_button)
        connection_actions.addWidget(disconnect_button)
        connection_actions.addStretch()
        server_layout.addLayout(connection_actions)
        layout.addWidget(server_panel)

        paths_panel = QFrame()
        paths_panel.setObjectName("panel")
        paths_layout = QVBoxLayout(paths_panel)
        paths_layout.setContentsMargins(12, 10, 12, 10)
        paths_layout.addWidget(self._make_section_title("Library Paths"))

        paths_form = QFormLayout()
        self.library_path_input = QLineEdit(self.config["library_path"])

        library_path_row = QWidget()
        library_path_row_layout = QHBoxLayout(library_path_row)
        library_path_row_layout.setContentsMargins(0, 0, 0, 0)
        library_path_row_layout.setSpacing(8)
        library_path_row_layout.addWidget(self.library_path_input)

        browse_library_path_button = QPushButton("Browse...")
        browse_library_path_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        browse_library_path_button.clicked.connect(self._browse_library_path)
        library_path_row_layout.addWidget(browse_library_path_button)

        paths_form.addRow("Library Path", library_path_row)
        paths_layout.addLayout(paths_form)
        layout.addWidget(paths_panel)

        launch_panel = QFrame()
        launch_panel.setObjectName("panel")
        launch_layout = QVBoxLayout(launch_panel)
        launch_layout.setContentsMargins(12, 10, 12, 10)
        launch_layout.addWidget(self._make_section_title("Launch Options"))

        launch_form = QFormLayout()
        self.launch_args_input = QLineEdit(self.config["launch_args"])
        launch_form.addRow("Extra Arguments (%rom%, %core%)", self.launch_args_input)
        launch_layout.addLayout(launch_form)
        layout.addWidget(launch_panel)

        appearance_panel = QFrame()
        appearance_panel.setObjectName("panel")
        appearance_layout = QVBoxLayout(appearance_panel)
        appearance_layout.setContentsMargins(12, 10, 12, 10)
        appearance_layout.addWidget(self._make_section_title("Appearance"))

        appearance_form = QFormLayout()
        self.theme_input = QComboBox()
        self.theme_input.addItems(["system", "light", "dark"])
        self.theme_input.setCurrentText(self.config["theme"])
        appearance_form.addRow("Theme", self.theme_input)
        appearance_layout.addLayout(appearance_form)
        layout.addWidget(appearance_panel)

        controls_row = QHBoxLayout()
        save_button = QPushButton("Save Settings")
        save_button.clicked.connect(self._save_settings)
        controls_row.addWidget(save_button)

        open_folder_button = QPushButton("Open Config Folder")
        open_folder_button.clicked.connect(self._open_config_folder)
        controls_row.addWidget(open_folder_button)

        self.settings_status_label = QLabel("Loaded settings from ~/.rom-mate/config.json")
        controls_row.addWidget(self.settings_status_label)
        controls_row.addStretch()
        layout.addLayout(controls_row)

        layout.addStretch()
        return page

    def _build_game_details_page(self) -> QWidget:
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.setSpacing(12)

        back_button = QPushButton("Back")
        back_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        back_button.clicked.connect(self._return_from_details)
        layout.addWidget(back_button)

        content = QFrame()
        content.setObjectName("panel")
        self.details_content_frame = content
        content_layout = QHBoxLayout(content)
        content_layout.setContentsMargins(12, 16, 12, 16)
        content_layout.setSpacing(20)

        cover_col = QVBoxLayout()
        cover_col.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)

        cover = QLabel("Cover Art")
        cover.setAlignment(Qt.AlignmentFlag.AlignCenter)
        cover.setMinimumSize(260, 340)
        cover.setMaximumSize(860, 1120)
        cover.setStyleSheet(
            "background-color: #111827; border: 1px dashed #4b5563; border-radius: 8px; font-size: 20px;"
        )
        self.details_cover_label = cover
        cover_col.addWidget(cover)
        cover_col.addStretch()
        content_layout.addLayout(cover_col, 2)

        details_col = QVBoxLayout()
        details_col.setSpacing(10)
        details_col.setAlignment(Qt.AlignmentFlag.AlignTop)

        action_row = QHBoxLayout()
        action_row.setSpacing(8)
        action_row.setAlignment(Qt.AlignmentFlag.AlignHCenter)

        primary = QPushButton("Launch Game")
        primary.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        primary.clicked.connect(self._perform_game_action)
        self.details_primary_button = primary
        action_row.addWidget(primary)

        secondary = QPushButton("Uninstall Game")
        secondary.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        secondary.clicked.connect(self._perform_game_secondary_action)
        self.details_secondary_button = secondary
        action_row.addWidget(secondary)
        details_col.addLayout(action_row)

        title = QLabel("Game Title")
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        title.setStyleSheet("font-size: 30px; font-weight: 700;")
        self.details_title_label = title
        details_col.addWidget(title)

        platform = QLabel("Platform: -")
        platform.setAlignment(Qt.AlignmentFlag.AlignCenter)
        platform.setStyleSheet("font-size: 18px;")
        self.details_platform_label = platform
        details_col.addWidget(platform)

        rating = QLabel("Rating: -")
        rating.setAlignment(Qt.AlignmentFlag.AlignCenter)
        rating.setStyleSheet("font-size: 18px;")
        self.details_rating_label = rating
        details_col.addWidget(rating)

        description = QLabel("Description")
        description.setWordWrap(True)
        description.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)
        description.setMinimumWidth(0)
        description.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
        description.setStyleSheet("font-size: 17px;")
        self.details_description_label = description
        details_col.addWidget(description)
        details_col.addStretch()
        content_layout.addLayout(details_col, 4)

        screenshots_col = QVBoxLayout()
        screenshots_col.setSpacing(8)

        screenshots_title = QLabel("Screenshots")
        screenshots_title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        screenshots_title.setStyleSheet("font-size: 18px; font-weight: 600;")
        screenshots_col.addWidget(screenshots_title)

        screenshots_scroll = QScrollArea()
        screenshots_scroll.setWidgetResizable(True)
        screenshots_scroll.setMinimumWidth(230)
        screenshots_scroll.setMaximumWidth(600)
        screenshots_scroll.setStyleSheet("background-color: transparent; border: none;")
        self.details_screenshots_scroll = screenshots_scroll

        screenshots_content = QWidget()
        screenshots_content_layout = QVBoxLayout(screenshots_content)
        screenshots_content_layout.setContentsMargins(0, 0, 0, 0)
        screenshots_content_layout.setSpacing(10)

        self.details_screenshot_labels = []
        for index in range(5):
            screenshot_label = QLabel(f"Screenshot {index + 1}")
            screenshot_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
            screenshot_label.setMinimumSize(210, 118)
            screenshot_label.setMaximumSize(520, 320)
            screenshot_label.setStyleSheet(
                "background-color: #111827; border: 1px dashed #4b5563; border-radius: 8px;"
            )
            self.details_screenshot_labels.append(screenshot_label)
            screenshots_content_layout.addWidget(screenshot_label)
        screenshots_content_layout.addStretch()

        screenshots_scroll.setWidget(screenshots_content)
        screenshots_col.addWidget(screenshots_scroll)
        content_layout.addLayout(screenshots_col, 2)

        layout.addWidget(content)
        self._update_details_layout_metrics()
        return page

    def _make_section_title(self, title: str) -> QLabel:
        label = QLabel(title)
        label.setStyleSheet("font-size: 15px; font-weight: 600;")
        return label

    def _config_defaults(self) -> dict[str, Any]:
        return {
            "server_url": "",
            "api_token": "",
            "username": "",
            "library_path": "",
            "launch_args": "",
            "theme": "system",
            "emulators": [],
            "default_emulators": {},
            "default_retroarch_cores": {},
            "installed_games": [],
        }

    def _config_dir(self) -> Path:
        return Path.home() / ".rom-mate"

    def _config_file(self) -> Path:
        return self._config_dir() / "config.json"

    def _token_file(self) -> Path:
        return self._config_dir() / "token.bin"

    def _windows_protect_data(self, raw: bytes) -> bytes:
        class DataBlob(ctypes.Structure):
            _fields_ = [("cbData", ctypes.c_uint32), ("pbData", ctypes.POINTER(ctypes.c_byte))]

        if not raw:
            return b""

        in_buffer = ctypes.create_string_buffer(raw, len(raw))
        in_blob = DataBlob(len(raw), ctypes.cast(in_buffer, ctypes.POINTER(ctypes.c_byte)))
        out_blob = DataBlob()

        if not ctypes.windll.crypt32.CryptProtectData(
            ctypes.byref(in_blob),
            None,
            None,
            None,
            None,
            0,
            ctypes.byref(out_blob),
        ):
            raise OSError("Could not securely protect token")

        try:
            return ctypes.string_at(out_blob.pbData, out_blob.cbData)
        finally:
            ctypes.windll.kernel32.LocalFree(out_blob.pbData)

    def _windows_unprotect_data(self, protected: bytes) -> bytes:
        class DataBlob(ctypes.Structure):
            _fields_ = [("cbData", ctypes.c_uint32), ("pbData", ctypes.POINTER(ctypes.c_byte))]

        if not protected:
            return b""

        in_buffer = ctypes.create_string_buffer(protected, len(protected))
        in_blob = DataBlob(len(protected), ctypes.cast(in_buffer, ctypes.POINTER(ctypes.c_byte)))
        out_blob = DataBlob()

        if not ctypes.windll.crypt32.CryptUnprotectData(
            ctypes.byref(in_blob),
            None,
            None,
            None,
            None,
            0,
            ctypes.byref(out_blob),
        ):
            raise OSError("Could not securely unprotect token")

        try:
            return ctypes.string_at(out_blob.pbData, out_blob.cbData)
        finally:
            ctypes.windll.kernel32.LocalFree(out_blob.pbData)

    def _load_api_token(self) -> str:
        token_file = self._token_file()
        if not token_file.exists():
            return ""

        try:
            payload = token_file.read_bytes()
        except OSError:
            return ""

        if not payload:
            return ""

        try:
            if sys.platform.startswith("win"):
                raw = self._windows_unprotect_data(payload)
            else:
                raw = base64.b64decode(payload, validate=True)
            return raw.decode("utf-8")
        except (OSError, ValueError, UnicodeDecodeError):
            return ""

    def _save_api_token(self, token: str) -> bool:
        normalized = token.strip()
        token_file = self._token_file()

        if not normalized:
            try:
                if token_file.exists():
                    token_file.unlink()
                return True
            except OSError:
                return False

        try:
            self._config_dir().mkdir(parents=True, exist_ok=True)
            raw = normalized.encode("utf-8")
            if sys.platform.startswith("win"):
                payload = self._windows_protect_data(raw)
            else:
                payload = base64.b64encode(raw)
            token_file.write_bytes(payload)
            return True
        except OSError:
            return False

    def _set_api_token(self, token: str) -> bool:
        normalized = token.strip()
        saved = self._save_api_token(normalized)
        if saved:
            self.config["api_token"] = normalized
        return saved

    def _load_config(self) -> dict[str, Any]:
        defaults = self._config_defaults()
        config_path = self._config_file()

        if not config_path.exists():
            return defaults

        try:
            content = json.loads(config_path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            return defaults

        if not isinstance(content, dict):
            return defaults

        merged = defaults.copy()
        for key, default_value in defaults.items():
            value = content.get(key)
            if isinstance(default_value, str) and isinstance(value, str):
                merged[key] = value
            elif key == "emulators":
                merged[key] = self._normalize_emulators(value)
            elif key == "default_emulators":
                merged[key] = self._normalize_default_emulators(value)
            elif key == "default_retroarch_cores":
                merged[key] = self._normalize_default_retroarch_cores(value)
            elif key == "installed_games":
                merged[key] = self._normalize_installed_games(value)

        stored_token = self._load_api_token()
        if stored_token:
            merged["api_token"] = stored_token
        else:
            legacy_token = merged.get("api_token", "")
            if isinstance(legacy_token, str) and legacy_token.strip() and self._save_api_token(legacy_token):
                merged["api_token"] = legacy_token.strip()
                migrated = merged.copy()
                migrated["api_token"] = ""
                self._save_config(migrated)
        return merged

    def _collect_settings(self) -> dict[str, Any]:
        values = self._config_defaults()
        if self.server_url_input is not None:
            values["server_url"] = self.server_url_input.text().strip()
        if self.api_token_input is not None:
            values["api_token"] = self.api_token_input.text().strip()
        existing_username = self.config.get("username", "")
        if isinstance(existing_username, str):
            values["username"] = existing_username.strip()
        if self.library_path_input is not None:
            values["library_path"] = self.library_path_input.text().strip()
        if self.launch_args_input is not None:
            values["launch_args"] = self.launch_args_input.text().strip()
        if self.theme_input is not None:
            values["theme"] = self.theme_input.currentText().strip() or "system"
        values["emulators"] = self.config.get("emulators", [])
        values["default_emulators"] = self.config.get("default_emulators", {})
        values["default_retroarch_cores"] = self.config.get("default_retroarch_cores", {})
        values["installed_games"] = self.library_games
        return values

    def _save_settings(self) -> None:
        self.config = self._collect_settings()
        token_value = self.config.get("api_token", "")
        if not isinstance(token_value, str) or not self._set_api_token(token_value):
            if self.settings_status_label is not None:
                self.settings_status_label.setText("Failed to securely save API token")
            QMessageBox.warning(self, "Save Error", "Could not securely save API token")
            return
        if not self._ensure_library_path_exists():
            return
        saved = self._save_config(self.config)
        if self.settings_status_label is not None and saved:
            self.settings_status_label.setText("Settings saved")
        self._clear_server_connection_data()
        self._set_server_status("Not connected", "#fca5a5")
        self._update_top_bar_identity()

        if self._credentials_present() and self.server_auto_reconnect:
            self._connect_to_server(show_errors=False)

    def _ensure_library_path_exists(self) -> bool:
        value = self.config.get("library_path", "")
        if not isinstance(value, str):
            return False

        path = value.strip()
        if not path:
            return True

        try:
            Path(path).expanduser().mkdir(parents=True, exist_ok=True)
            return True
        except OSError as error:
            if self.settings_status_label is not None:
                self.settings_status_label.setText("Failed to create library folder")
            QMessageBox.warning(self, "Save Error", f"Could not create library folder: {error}")
            return False

    def _save_server_connection_settings(self) -> bool:
        if self.server_url_input is not None:
            self.config["server_url"] = self.server_url_input.text().strip()
        if self.api_token_input is not None:
            if not self._set_api_token(self.api_token_input.text()):
                if self.settings_status_label is not None:
                    self.settings_status_label.setText("Failed to securely save API token")
                QMessageBox.warning(self, "Save Error", "Could not securely save API token")
                return False

        saved = self._save_config(self.config)
        if saved and self.settings_status_label is not None:
            self.settings_status_label.setText("Server connection settings saved")
        return saved

    def _connect_from_settings(self, checked: bool = False) -> None:
        del checked
        self.server_auto_reconnect = True
        self._save_server_connection_settings()
        self._connect_to_server()

    def _disconnect_from_server(self, checked: bool = False) -> None:
        del checked
        self.server_auto_reconnect = False
        self._clear_server_connection_data()
        self._set_server_status("Disconnected", "#fca5a5")
        self.config["username"] = ""
        self._update_top_bar_identity()

    def _clear_server_connection_data(self) -> None:
        self.server_connected = False
        self.server_platform_ids = {}
        self.server_games_by_platform = {}
        self.server_rom_payloads = {}
        if self.server_platforms_list is not None:
            self.server_platforms_list.clear()
            self._resize_server_platform_list()
        self._refresh_emulator_views()
        if self.server_games_grid is not None:
            self._clear_layout(self.server_games_grid)

    def _resize_server_platform_list(self) -> None:
        if self.server_platforms_list is None:
            return
        content_width = self.server_platforms_list.sizeHintForColumn(0)
        if content_width < 0:
            content_width = 0
        scroll_width = self.server_platforms_list.verticalScrollBar().sizeHint().width()
        frame_width = self.server_platforms_list.frameWidth() * 2
        target_width = content_width + scroll_width + frame_width + 20
        target_width = max(120, min(220, target_width))
        self.server_platforms_list.setFixedWidth(target_width)

    def _server_connected(self) -> bool:
        return self.server_connected

    def _update_top_bar_identity(self) -> None:
        if self.account_status_label is None:
            return
        username = self.config.get("username", "")
        if isinstance(username, str) and username.strip() and self._server_connected():
            self.account_status_label.setText(f"Logged in as: {username.strip()}")
            return
        self.account_status_label.setText("Offline")

    def _credentials_present(self) -> bool:
        server_url = self.config.get("server_url", "")
        api_token = self.config.get("api_token", "")
        if not isinstance(server_url, str) or not isinstance(api_token, str):
            return False
        return bool(server_url.strip() and api_token.strip())

    def _server_base_url(self) -> str:
        server_url = self.config.get("server_url", "")
        if not isinstance(server_url, str):
            return ""
        return server_url.strip().rstrip("/")

    def _resolve_cover_url(self, value: Any) -> str:
        if not isinstance(value, str) or not value.strip():
            return ""
        candidate = value.strip()
        if not (candidate.startswith("http://") or candidate.startswith("https://")):
            base_url = self._server_base_url()
            if not base_url:
                return ""
            if candidate.startswith("/"):
                candidate = f"{base_url}{candidate}"
            else:
                candidate = f"{base_url}/{candidate}"

        split = urlsplit(candidate)
        safe_path = quote(split.path, safe="/%._-~")
        query_items = parse_qsl(split.query, keep_blank_values=True)
        safe_query = urlencode(query_items, doseq=True)
        return urlunsplit((split.scheme, split.netloc, safe_path, safe_query, split.fragment))

    def _cover_url_from_rom_payload(self, payload: dict[str, Any]) -> str:
        def resolve_cover_value(value: Any) -> str:
            if isinstance(value, str):
                return self._resolve_cover_url(value)
            if isinstance(value, dict):
                for key in ("url", "path", "image", "src", "download_path", "file_path", "full_path"):
                    candidate = value.get(key)
                    if isinstance(candidate, str):
                        resolved = self._resolve_cover_url(candidate)
                        if resolved:
                            return resolved
            return ""

        for key in ("cover_url", "cover_image", "cover_path", "image_url"):
            value = payload.get(key)
            resolved = resolve_cover_value(value)
            if resolved:
                return resolved

        return ""

    def _screenshot_urls_from_rom_payload(self, payload: dict[str, Any]) -> list[str]:
        urls: list[str] = []

        def append_url(value: Any) -> None:
            if isinstance(value, str):
                resolved = self._resolve_cover_url(value)
                if resolved and resolved not in urls:
                    urls.append(resolved)
                return
            if isinstance(value, dict):
                for key in ("url", "path", "image", "src"):
                    candidate = value.get(key)
                    if isinstance(candidate, str):
                        resolved = self._resolve_cover_url(candidate)
                        if resolved and resolved not in urls:
                            urls.append(resolved)
                            return

        merged_screenshots = payload.get("merged_screenshots")
        if isinstance(merged_screenshots, list):
            for item in merged_screenshots:
                append_url(item)

        user_screenshots = payload.get("user_screenshots")
        if isinstance(user_screenshots, list):
            for item in user_screenshots:
                if not isinstance(item, dict):
                    continue
                for key in ("download_path", "file_path", "full_path"):
                    append_url(item.get(key))

        gamelist_metadata = payload.get("gamelist_metadata")
        if isinstance(gamelist_metadata, dict):
            for key in ("screenshot_url", "title_screen_url", "image_url"):
                append_url(gamelist_metadata.get(key))

        ss_metadata = payload.get("ss_metadata")
        if isinstance(ss_metadata, dict):
            for key in ("screenshot_url", "title_screen_url", "fanart_url"):
                append_url(ss_metadata.get(key))

        launchbox_metadata = payload.get("launchbox_metadata")
        if isinstance(launchbox_metadata, dict):
            images = launchbox_metadata.get("images")
            if isinstance(images, list):
                for image in images:
                    if not isinstance(image, dict):
                        continue
                    append_url(image.get("url"))

        for key in ("url_screenshots", "path_screenshots", "screenshots", "images"):
            value = payload.get(key)
            if isinstance(value, list):
                for item in value:
                    append_url(item)
            else:
                append_url(value)

        for key in ("url_screenshot", "path_screenshot"):
            append_url(payload.get(key))

        return urls

    def _screenshot_urls_from_game(self, game: dict[str, str]) -> list[str]:
        raw = game.get("screenshot_urls", "")
        if not isinstance(raw, str) or not raw.strip():
            return []
        unique: list[str] = []
        for item in raw.splitlines():
            value = item.strip()
            if value and value not in unique:
                unique.append(value)
        return unique

    def _update_details_screenshots(self, game: dict[str, str]) -> None:
        if not self.details_screenshot_labels:
            return
        screenshot_urls = self._screenshot_urls_from_game(game)
        for index, label in enumerate(self.details_screenshot_labels):
            label.clear()
            if index < len(screenshot_urls):
                label.setVisible(True)
                self._queue_cover_load(screenshot_urls[index], label)
            else:
                label.setVisible(False)

    def _update_details_layout_metrics(self) -> None:
        if self.details_content_frame is None:
            return

        content_width = self.details_content_frame.width()
        if content_width <= 0:
            content_width = self.width() - 64
        content_width = max(content_width, 900)

        cover_width = max(300, min(780, int(content_width * 0.36)))
        cover_height = max(400, min(1040, int(cover_width * 1.35)))
        if self.details_cover_label is not None:
            self.details_cover_label.setFixedSize(cover_width, cover_height)

        screenshot_width = max(210, min(520, int(content_width * 0.25)))
        screenshot_height = max(118, min(320, int(screenshot_width * 0.62)))
        if self.details_screenshots_scroll is not None:
            self.details_screenshots_scroll.setFixedWidth(screenshot_width + 28)
        for label in self.details_screenshot_labels:
            label.setFixedSize(screenshot_width, screenshot_height)

        if self.details_description_label is not None:
            description_width = max(420, min(1600, int(content_width * 0.66)))
            self.details_description_label.setMaximumWidth(description_width)

        self._rescale_details_media_for_current_sizes()

    def _rescale_details_media_for_current_sizes(self) -> None:
        game = self.current_details_game
        if game is None:
            return

        if self.details_cover_label is not None:
            cover_url = game.get("cover_url", "")
            if isinstance(cover_url, str) and cover_url.strip():
                normalized_cover = cover_url.strip()
                if normalized_cover in self.cover_cache:
                    self._apply_cover_to_label(self.details_cover_label, self.cover_cache[normalized_cover])
                else:
                    self._queue_cover_load(normalized_cover, self.details_cover_label)

        screenshot_urls = self._screenshot_urls_from_game(game)
        for index, label in enumerate(self.details_screenshot_labels):
            if index >= len(screenshot_urls):
                continue
            screenshot_url = screenshot_urls[index].strip()
            if not screenshot_url:
                continue
            if screenshot_url in self.cover_cache:
                self._apply_cover_to_label(label, self.cover_cache[screenshot_url])
            else:
                self._queue_cover_load(screenshot_url, label)

    def _apply_cover_to_label(self, label: QLabel, pixmap: QPixmap | None) -> None:
        if pixmap is None or pixmap.isNull():
            return
        try:
            label.setText("")
            label.setPixmap(
                pixmap.scaled(
                    label.size(),
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.SmoothTransformation,
                )
            )
        except RuntimeError:
            return

    def _queue_cover_load(self, cover_url: str, label: QLabel) -> None:
        normalized = cover_url.strip()
        if not normalized:
            return

        if normalized in self.cover_cache:
            self._apply_cover_to_label(label, self.cover_cache[normalized])
            return

        waiters = self.cover_waiters.setdefault(normalized, [])
        waiters.append(label)
        if normalized in self.cover_loading:
            return

        self.cover_loading.add(normalized)
        request = QNetworkRequest(QUrl(normalized))
        request.setRawHeader(b"Accept", b"image/*")
        reply = self.cover_network.get(request)
        reply.finished.connect(lambda url=normalized, rep=reply: self._on_cover_reply(url, rep))

    def _on_cover_reply(self, cover_url: str, reply: QNetworkReply) -> None:
        pixmap: QPixmap | None = None
        if reply.error() == QNetworkReply.NetworkError.NoError:
            payload = bytes(reply.readAll())
            parsed = QPixmap()
            if payload and parsed.loadFromData(payload):
                pixmap = parsed

        self.cover_cache[cover_url] = pixmap
        self.cover_loading.discard(cover_url)
        waiters = self.cover_waiters.pop(cover_url, [])
        for label in waiters:
            self._apply_cover_to_label(label, pixmap)
        reply.deleteLater()

    def _auth_headers(self) -> dict[str, str]:
        api_token = self.config.get("api_token", "")
        if not isinstance(api_token, str):
            api_token = ""
        return {"Accept": "application/json", "Authorization": f"Bearer {api_token.strip()}"}

    def _api_get(self, path: str, params: dict[str, Any] | None = None) -> Any:
        base_url = self._server_base_url()
        if not base_url:
            raise ValueError("Server URL is required")

        query = ""
        if params:
            query = urlencode(params, doseq=True)
        url = f"{base_url}{path}"
        if query:
            url = f"{url}?{query}"

        request = Request(url, headers=self._auth_headers(), method="GET")
        with urlopen(request, timeout=10) as response:
            raw = response.read().decode("utf-8")
        return json.loads(raw)

    def _api_get_bytes(self, path: str, params: dict[str, Any] | None = None) -> bytes:
        base_url = self._server_base_url()
        if not base_url:
            raise ValueError("Server URL is required")

        query = ""
        if params:
            query = urlencode(params, doseq=True)
        url = f"{base_url}{path}"
        if query:
            url = f"{url}?{query}"

        request = Request(url, headers=self._auth_headers(), method="GET")
        with urlopen(request, timeout=60) as response:
            return response.read()

    def _set_server_status(self, text: str, color: str = "#9ca3af") -> None:
        if self.server_status_label is None:
            return
        self.server_status_label.setText(text)
        self.server_status_label.setStyleSheet(f"color: {color};")

    def _connect_to_server(self, checked: bool = False, show_errors: bool = True) -> None:
        del checked
        if not self._credentials_present():
            self._clear_server_connection_data()
            self._set_server_status("Missing server URL or API token", "#fca5a5")
            self._update_top_bar_identity()
            return

        self._set_server_status("Connecting...", "#93c5fd")
        last_error: Exception | None = None
        try:
            me = self._api_get("/api/users/me")
            platforms = self._api_get("/api/platforms")
            self.server_connected = True
            self._apply_connected_user(me)
            self._populate_server_platforms(platforms)
            self._set_server_status("Connected", "#86efac")
            self._update_top_bar_identity()
            return
        except (HTTPError, URLError, ValueError, json.JSONDecodeError) as error:
            last_error = error

        self._clear_server_connection_data()
        self._update_top_bar_identity()

        error_text = "Failed to connect"
        if isinstance(last_error, HTTPError):
            error_text = f"Connection failed ({last_error.code})"
        elif isinstance(last_error, URLError):
            error_text = "Connection failed (network error)"
        self._set_server_status(error_text, "#fca5a5")

        if show_errors:
            QMessageBox.warning(self, "Server Connection", error_text)

    def _apply_connected_user(self, me_payload: Any) -> None:
        if not isinstance(me_payload, dict):
            return
        username = me_payload.get("username")
        if not isinstance(username, str) or not username.strip():
            return
        self.config["username"] = username.strip()

    def _populate_server_platforms(self, payload: Any) -> None:
        if self.server_platforms_list is None:
            return
        if not isinstance(payload, list):
            self.server_platforms_list.clear()
            self.server_platform_ids = {}
            self._refresh_emulator_views()
            return

        self.server_platforms_list.clear()
        self.server_platform_ids = {}
        self.server_games_by_platform = {}
        self.server_rom_payloads = {}

        for entry in payload:
            if not isinstance(entry, dict):
                continue
            platform_id = entry.get("id")
            label = entry.get("display_name") or entry.get("name") or entry.get("slug")
            rom_count = entry.get("rom_count")
            if (
                not isinstance(platform_id, int)
                or not isinstance(label, str)
                or (isinstance(rom_count, int) and rom_count <= 0)
            ):
                continue

            base_label = label.strip() or f"Platform {platform_id}"
            display_label = base_label
            counter = 2
            while display_label in self.server_platform_ids:
                display_label = f"{base_label} ({counter})"
                counter += 1

            self.server_platform_ids[display_label] = platform_id
            self.server_platforms_list.addItem(display_label)

        if self.server_platforms_list.count() > 0:
            self.server_platforms_list.setCurrentRow(0)
        self._resize_server_platform_list()
        self._refresh_emulator_views()

    def _on_server_platform_selected(self, platform_label: str) -> None:
        if not platform_label:
            return
        if platform_label not in self.server_games_by_platform:
            self._load_server_games(platform_label)
        self._render_server_games(platform_label)

    def _load_server_games(self, platform_label: str) -> None:
        if not self.server_connected:
            return

        platform_id = self.server_platform_ids.get(platform_label)
        if platform_id is None:
            return

        try:
            payload = self._api_get(
                "/api/roms",
                {
                    "platform_ids": [platform_id],
                    "limit": 200,
                    "offset": 0,
                    "with_char_index": "false",
                    "with_filter_values": "false",
                },
            )
        except (HTTPError, URLError, ValueError, json.JSONDecodeError):
            self.server_games_by_platform[platform_label] = []
            self._set_server_status("Connected, but failed to load games", "#fbbf24")
            return

        games: list[dict[str, str]] = []
        items = payload.get("items") if isinstance(payload, dict) else None
        if isinstance(items, list):
            for item in items:
                if not isinstance(item, dict):
                    continue
                title = item.get("name") or item.get("fs_name_no_ext")
                if not isinstance(title, str) or not title.strip():
                    continue
                rom_id = str(item.get("id", "")).strip()
                if rom_id:
                    self.server_rom_payloads[rom_id] = item
                platform_name = item.get("platform_display_name")
                summary = item.get("summary")
                cover_url = self._cover_url_from_rom_payload(item)
                screenshot_urls = self._screenshot_urls_from_rom_payload(item)
                games.append(
                    {
                        "title": title.strip(),
                        "platform": platform_name.strip() if isinstance(platform_name, str) and platform_name.strip() else platform_label,
                        "rating": "N/A",
                        "description": summary.strip() if isinstance(summary, str) and summary.strip() else "No description available.",
                        "cover_url": cover_url,
                        "screenshot_urls": "\n".join(screenshot_urls),
                        "rom_id": rom_id,
                        "rom_file_name": item.get("fs_name", "").strip() if isinstance(item.get("fs_name", ""), str) else "",
                    }
                )

        self.server_games_by_platform[platform_label] = games

    def _save_config(self, config: dict[str, Any]) -> bool:
        config_dir = self._config_dir()
        config_file = self._config_file()
        serialized = config.copy()
        serialized["api_token"] = ""

        try:
            config_dir.mkdir(parents=True, exist_ok=True)
            config_file.write_text(
                json.dumps(serialized, indent=2, sort_keys=True),
                encoding="utf-8",
            )
            return True
        except OSError as error:
            if self.settings_status_label is not None:
                self.settings_status_label.setText("Failed to save settings")
            QMessageBox.warning(self, "Save Error", f"Could not save config: {error}")
            return False

    def _open_config_folder(self) -> None:
        config_dir = self._config_dir()
        try:
            config_dir.mkdir(parents=True, exist_ok=True)
        except OSError as error:
            QMessageBox.warning(self, "Open Folder Error", f"Could not open config folder: {error}")
            return

        QDesktopServices.openUrl(QUrl.fromLocalFile(str(config_dir)))

    def _make_game_card(self, game: dict[str, str], source: str) -> QPushButton:
        frame = QPushButton()
        frame.setObjectName("gameCard")
        frame.setFixedSize(180, 250)
        frame.clicked.connect(lambda: self._open_game_details(game, source))

        layout = QVBoxLayout(frame)
        layout.setContentsMargins(8, 8, 8, 8)
        layout.setSpacing(8)

        cover = QLabel("Cover Art")
        cover.setAlignment(Qt.AlignmentFlag.AlignCenter)
        cover.setFixedHeight(170)
        cover.setStyleSheet(
            "background-color: #111827; border: 1px dashed #4b5563; border-radius: 6px;"
        )

        cover_url = game.get("cover_url", "")
        if isinstance(cover_url, str) and cover_url.strip():
            self._queue_cover_load(cover_url, cover)
        layout.addWidget(cover)

        title_label = QLabel(game["title"])
        title_label.setWordWrap(True)
        title_label.setStyleSheet("font-weight: 600;")
        layout.addWidget(title_label)

        platform_label = QLabel(game["platform"])
        platform_label.setStyleSheet("color: #9ca3af;")
        layout.addWidget(platform_label)

        return frame

    def _refresh_library_grid(self) -> None:
        if not hasattr(self, "library_grid") or self.library_scroll is None:
            return

        visible_games = self._visible_library_games()
        has_games = bool(visible_games)
        self.library_scroll.setVisible(has_games)
        if self.library_empty_label is not None:
            self.library_empty_label.setVisible(not has_games)

        self._clear_layout(self.library_grid)
        if not has_games:
            return

        columns = self._grid_columns_for_width(self.library_scroll, self.library_grid)
        for i, game in enumerate(visible_games):
            card = self._make_game_card(game, "library")
            row = i // columns
            col = i % columns
            self.library_grid.addWidget(card, row, col)

    def _visible_library_games(self) -> list[dict[str, str]]:
        return [game for game in self.library_games if not self._is_hidden_library_platform(game)]

    def _is_hidden_library_platform(self, game: dict[str, str]) -> bool:
        platform_value = game.get("platform", "")
        platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
        return platform in {"emulator", "emulators"}

    def _render_server_games(self, platform: str) -> None:
        if self.server_games_grid is None or self.server_games_scroll is None:
            return
        games = self.server_games_by_platform.get(platform, [])
        self._clear_layout(self.server_games_grid)
        columns = self._grid_columns_for_width(self.server_games_scroll, self.server_games_grid)
        for i, game in enumerate(games):
            card = self._make_game_card(game, "server")
            row = i // columns
            col = i % columns
            self.server_games_grid.addWidget(card, row, col)

    def _grid_columns_for_width(self, scroll: QScrollArea, grid: QGridLayout) -> int:
        viewport_width = scroll.viewport().width()
        margins = grid.contentsMargins()
        usable_width = viewport_width - margins.left() - margins.right()
        spacing = grid.horizontalSpacing()
        if spacing < 0:
            spacing = 12
        card_width = 180
        column_span = card_width + spacing
        if usable_width <= 0 or column_span <= 0:
            return 1
        return max(1, (usable_width + spacing) // column_span)

    def _game_key(self, game: dict[str, str]) -> tuple[str, str]:
        return (game.get("title", "").strip().lower(), game.get("platform", "").strip().lower())

    def _is_game_installed(self, game: dict[str, str]) -> bool:
        target = self._game_key(game)
        return any(self._game_key(installed) == target for installed in self.library_games)

    def _installed_game_record(self, game: dict[str, str]) -> dict[str, str] | None:
        target = self._game_key(game)
        for installed in self.library_games:
            if self._game_key(installed) == target:
                return installed
        return None

    def _persist_installed_games(self) -> bool:
        self.library_games = self._normalize_installed_games(self.library_games)
        self.config["installed_games"] = self.library_games
        return self._save_config(self.config)

    def _install_game(self, game: dict[str, str]) -> bool:
        if self._is_game_installed(game):
            return False
        archive_path = self._download_game_archive(game)
        if archive_path is None:
            return False
        installed_game = self._prepare_installed_game(game, archive_path)
        if installed_game is None:
            return False
        self._auto_configure_installed_emulator(installed_game, archive_path)
        self._register_installed_game(installed_game, archive_path)
        return True

    def _register_installed_game(self, game: dict[str, str], archive_path: Path) -> None:
        extracted_path = game.get("extracted_path", "").strip()
        stored_archive_path = "" if extracted_path else str(archive_path)
        self.library_games.append(
            {
                "title": game.get("title", "").strip(),
                "platform": game.get("platform", "").strip(),
                "rating": game.get("rating", "N/A").strip() or "N/A",
                "description": game.get("description", "No description available.").strip() or "No description available.",
                "cover_url": game.get("cover_url", "").strip(),
                "screenshot_urls": game.get("screenshot_urls", "").strip(),
                "rom_file_name": game.get("rom_file_name", "").strip(),
                "extracted_path": extracted_path,
                "extracted_dir": game.get("extracted_dir", "").strip(),
                "archive_path": stored_archive_path,
            }
        )
        self.library_games = self._normalize_installed_games(self.library_games)
        self._refresh_library_grid()
        self._persist_installed_games()

    def _is_arcade_platform(self, game: dict[str, str]) -> bool:
        platform_value = game.get("platform", "")
        platform = platform_value.strip().lower() if isinstance(platform_value, str) else ""
        arcade_tokens = ("arcade", "mame", "fbneo", "final burn")
        return any(token in platform for token in arcade_tokens)

    def _should_extract_archive_for_game(self, game: dict[str, str], archive_path: Path) -> bool:
        if self._is_arcade_platform(game):
            return False
        return archive_path.suffix.lower() in {".7z", ".zip"}

    def _extracted_dir_for_archive_path(self, archive_path: Path) -> Path:
        return archive_path.parent / archive_path.stem

    def _select_extracted_launch_file(self, extracted_dir: Path, archive_path: Path) -> Path | None:
        files = [candidate for candidate in extracted_dir.rglob("*") if candidate.is_file()]
        if not files:
            return None

        archive_suffixes = {".zip", ".7z", ".rar", ".tar", ".gz", ".bz2", ".xz"}
        non_archive_files = [candidate for candidate in files if candidate.suffix.lower() not in archive_suffixes]
        pool = non_archive_files if non_archive_files else files

        preferred_extensions = [".m3u", ".cue", ".chd", ".iso", ".bin"]
        for extension in preferred_extensions:
            for candidate in pool:
                if candidate.suffix.lower() == extension:
                    return candidate

        archive_stem = archive_path.stem.lower()
        stem_matches = [candidate for candidate in pool if candidate.stem.lower() == archive_stem]
        if stem_matches:
            stem_matches.sort(key=lambda candidate: (len(candidate.parts), str(candidate).lower()))
            return stem_matches[0]

        pool.sort(key=lambda candidate: (len(candidate.parts), str(candidate).lower()))
        return pool[0]

    def _extract_archive_for_game(self, game: dict[str, str], archive_path: Path) -> tuple[Path, Path]:
        extracted_dir = self._extracted_dir_for_archive_path(archive_path)
        if extracted_dir.exists():
            shutil.rmtree(extracted_dir, ignore_errors=True)
        extracted_dir.mkdir(parents=True, exist_ok=True)

        try:
            if archive_path.suffix.lower() == ".zip":
                with zipfile.ZipFile(archive_path) as archive:
                    archive.extractall(extracted_dir)
            else:
                result = subprocess.run(
                    ["tar", "-xf", str(archive_path), "-C", str(extracted_dir)],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if result.returncode != 0:
                    message = result.stderr.strip() or result.stdout.strip() or "Unknown extraction error"
                    raise OSError(message)
        except (OSError, zipfile.BadZipFile):
            shutil.rmtree(extracted_dir, ignore_errors=True)
            raise

        launch_file = self._select_extracted_launch_file(extracted_dir, archive_path)
        if launch_file is None:
            shutil.rmtree(extracted_dir, ignore_errors=True)
            raise OSError("Archive extracted but no ROM file was found")

        return launch_file, extracted_dir

    def _prepare_installed_game(self, game: dict[str, str], archive_path: Path) -> dict[str, str] | None:
        prepared = dict(game)
        prepared["extracted_path"] = ""
        prepared["extracted_dir"] = ""
        if not self._should_extract_archive_for_game(prepared, archive_path):
            return prepared

        try:
            extracted_file, extracted_dir = self._extract_archive_for_game(prepared, archive_path)
        except (OSError, zipfile.BadZipFile) as error:
            QMessageBox.warning(
                self,
                "Install Error",
                f"Failed to extract archive for {prepared.get('title', 'Game')}: {error}",
            )
            return None

        if archive_path.exists() and archive_path.is_file():
            try:
                archive_path.unlink()
            except OSError as error:
                QMessageBox.warning(
                    self,
                    "Install Warning",
                    f"Extracted {prepared.get('title', 'Game')}, but could not delete archive:\n{archive_path}\n{error}",
                )

        prepared["extracted_path"] = str(extracted_file)
        prepared["extracted_dir"] = str(extracted_dir)
        return prepared

    def _is_emulators_platform(self, game: dict[str, str]) -> bool:
        platform_value = game.get("platform", "")
        if not isinstance(platform_value, str):
            return False
        return platform_value.strip().casefold() == "emulators"

    def _default_assignable_server_platforms(self) -> list[str]:
        hidden_platforms = {"windows", "emulators"}
        return [
            platform
            for platform in self.server_platform_ids.keys()
            if platform.strip().casefold() not in hidden_platforms
        ]

    def _launchable_emulator_file(self, path: Path) -> bool:
        launchable_suffixes = {".exe", ".bat", ".cmd", ".ps1", ".sh", ".appimage"}
        return path.suffix.casefold() in launchable_suffixes

    def _select_emulator_executable_path(self, game: dict[str, str], archive_path: Path) -> str:
        extracted_path_value = game.get("extracted_path", "")
        if isinstance(extracted_path_value, str) and extracted_path_value.strip():
            extracted_path = Path(extracted_path_value).expanduser()
            if extracted_path.exists() and extracted_path.is_file() and self._launchable_emulator_file(extracted_path):
                return str(extracted_path)

        extracted_dir_value = game.get("extracted_dir", "")
        if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
            extracted_dir = Path(extracted_dir_value).expanduser()
            if extracted_dir.exists() and extracted_dir.is_dir():
                candidates = [candidate for candidate in extracted_dir.rglob("*") if candidate.is_file() and self._launchable_emulator_file(candidate)]
                if candidates:
                    title_text = game.get("title", "")
                    title = title_text.strip().casefold() if isinstance(title_text, str) else ""
                    title_tokens = [token for token in re.split(r"[^a-z0-9]+", title) if len(token) > 2]

                    def score(candidate: Path) -> tuple[int, int, int, str]:
                        candidate_name = candidate.stem.casefold()
                        token_hits = sum(1 for token in title_tokens if token in candidate_name)
                        preferred_binary = 0 if candidate.suffix.casefold() == ".exe" else 1
                        return (-token_hits, preferred_binary, len(candidate.parts), str(candidate).casefold())

                    candidates.sort(key=score)
                    return str(candidates[0])

        if archive_path.exists() and archive_path.is_file() and self._launchable_emulator_file(archive_path):
            return str(archive_path)

        return ""

    def _matching_platforms_for_emulator_keywords(self, keywords: list[str]) -> list[str]:
        if not keywords:
            return []

        matches: list[str] = []
        for platform in self._default_assignable_server_platforms():
            normalized_platform = platform.casefold()
            for keyword in keywords:
                normalized_keyword = keyword.casefold().strip()
                if not normalized_keyword:
                    continue
                if normalized_keyword in normalized_platform or normalized_platform in normalized_keyword:
                    if platform not in matches:
                        matches.append(platform)
                    break
        return matches

    def _emulator_profile_for_game(self, game: dict[str, str]) -> dict[str, Any]:
        title_value = game.get("title", "")
        title = title_value.strip() if isinstance(title_value, str) else ""
        normalized = title.casefold()

        profiles = [
            {
                "match_tokens": ["retroarch"],
                "name": "RetroArch",
                "args": '-L "%core%" "%rom%"',
                "all_platforms": True,
                "platform_keywords": [],
            },
            {
                "match_tokens": ["duckstation"],
                "name": "DuckStation",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["playstation", "ps1"],
            },
            {
                "match_tokens": ["pcsx2"],
                "name": "PCSX2",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["playstation 2", "ps2"],
            },
            {
                "match_tokens": ["ppsspp"],
                "name": "PPSSPP",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["playstation portable", "psp"],
            },
            {
                "match_tokens": ["rpcs3"],
                "name": "RPCS3",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["playstation 3", "ps3"],
            },
            {
                "match_tokens": ["dolphin"],
                "name": "Dolphin",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["gamecube", "wii"],
            },
            {
                "match_tokens": ["cemu"],
                "name": "Cemu",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["wii u"],
            },
            {
                "match_tokens": ["yuzu", "ryujinx", "sudachi"],
                "name": "Nintendo Switch Emulator",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["switch", "nintendo switch"],
            },
            {
                "match_tokens": ["mame", "fbneo", "final burn"],
                "name": "Arcade Emulator",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["arcade", "mame", "final burn", "fbneo"],
            },
        ]

        for profile in profiles:
            if any(token in normalized for token in profile["match_tokens"]):
                resolved_name = profile["name"]
                if profile["name"] in {"Nintendo Switch Emulator", "Arcade Emulator"}:
                    resolved_name = title or profile["name"]
                return {
                    "name": resolved_name,
                    "args": profile["args"],
                    "all_platforms": profile["all_platforms"],
                    "platform_keywords": profile["platform_keywords"],
                }

        return {
            "name": title or "Emulator",
            "args": "%rom%",
            "all_platforms": False,
            "platform_keywords": [],
        }

    def _auto_configure_installed_emulator(self, game: dict[str, str], archive_path: Path) -> bool:
        if not self._is_emulators_platform(game):
            return False

        profile = self._emulator_profile_for_game(game)
        emulator_name = profile["name"]
        executable_path = self._select_emulator_executable_path(game, archive_path)
        if not executable_path:
            return False

        emulators = self._normalize_emulators(self._emulators())
        args_template = profile["args"].strip() or "%rom%"
        target_index = -1
        for index, emulator in enumerate(emulators):
            existing_name = emulator.get("name", "")
            if isinstance(existing_name, str) and existing_name.strip().casefold() == emulator_name.casefold():
                target_index = index
                break

        if target_index >= 0:
            existing = emulators[target_index]
            existing_args = existing.get("args", "%rom%")
            should_update_args = self._is_retroarch_emulator_name(emulator_name) or not isinstance(existing_args, str) or not existing_args.strip() or existing_args.strip() == "%rom%"
            emulators[target_index] = {
                "name": emulator_name,
                "path": executable_path,
                "args": args_template if should_update_args else existing_args.strip(),
            }
        else:
            emulators.append({"name": emulator_name, "path": executable_path, "args": args_template})
        self.config["emulators"] = self._normalize_emulators(emulators)

        defaults = self._normalize_default_emulators(self.config.get("default_emulators", {}))
        if profile["all_platforms"]:
            target_platforms = self._default_assignable_server_platforms()
        else:
            target_platforms = self._matching_platforms_for_emulator_keywords(profile["platform_keywords"])

        for platform in target_platforms:
            current_value = defaults.get(platform, "")
            current_default = current_value.strip() if isinstance(current_value, str) else ""
            if not current_default:
                defaults[platform] = emulator_name
                continue
            if not self._is_retroarch_emulator_name(emulator_name) and self._is_retroarch_emulator_name(current_default):
                defaults[platform] = emulator_name
        self.config["default_emulators"] = defaults

        core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
        if self._is_retroarch_emulator_name(emulator_name):
            for platform in target_platforms:
                if defaults.get(platform, "").strip().casefold() != emulator_name.casefold():
                    continue
                existing_core = core_defaults.get(platform, "")
                if isinstance(existing_core, str) and existing_core.strip():
                    continue
                cores = self._retroarch_cores_for_platform(platform)
                if cores:
                    core_defaults[platform] = cores[0]
        self.config["default_retroarch_cores"] = core_defaults

        self._refresh_emulator_views()
        self._save_config(self.config)
        return True

    def _archive_name_for_game(self, game: dict[str, str]) -> str:
        rom_file_name_value = game.get("rom_file_name", "")
        if isinstance(rom_file_name_value, str):
            rom_file_name = rom_file_name_value.strip().replace("\\", "/").split("/")[-1]
            if rom_file_name:
                fallback_name = f"{self._sanitize_path_component(game.get('title', 'Game'), 'game')}.zip"
                return self._sanitize_path_component(rom_file_name, fallback_name)

        title_value = game.get("title", "Game")
        platform_value = game.get("platform", "Platform")
        title = title_value if isinstance(title_value, str) else "Game"
        platform = platform_value if isinstance(platform_value, str) else "Platform"
        safe_title = self._sanitize_path_component(title, "game")
        safe_platform = self._sanitize_path_component(platform, "platform")
        return f"{safe_title}-{safe_platform}.zip"

    def _sanitize_path_component(self, value: str, fallback: str) -> str:
        illegal_characters = set('<>:"/\\|?*')
        sanitized = "".join("_" if ch in illegal_characters or ord(ch) < 32 else ch for ch in value)
        while sanitized.endswith((" ", ".")):
            sanitized = f"{sanitized[:-1]}_"
        return sanitized if sanitized.strip(" _.") else fallback

    def _platform_library_dir(self, game: dict[str, str]) -> Path | None:
        library_path = self._library_path_dir()
        if library_path is None:
            return None
        platform_value = game.get("platform", "Platform")
        platform = platform_value if isinstance(platform_value, str) else "Platform"
        safe_platform = self._sanitize_path_component(platform, "platform")
        return library_path / safe_platform

    def _library_path_dir(self) -> Path | None:
        library_path_value = self.config.get("library_path", "")
        if not isinstance(library_path_value, str) or not library_path_value.strip():
            return None
        return Path(library_path_value).expanduser()

    def _download_game_archive(self, game: dict[str, str]) -> Path | None:
        rom_id = game.get("rom_id", "").strip()
        if not rom_id:
            QMessageBox.warning(self, "Install Error", "This game cannot be installed because it has no ROM id.")
            return None

        install_path = self._platform_library_dir(game)
        if install_path is None:
            QMessageBox.warning(self, "Install Error", "Set a Library Path in Settings before installing games.")
            return None

        try:
            install_path.mkdir(parents=True, exist_ok=True)
        except OSError as error:
            QMessageBox.warning(self, "Install Error", f"Could not prepare library folder: {error}")
            return None

        title = game.get("title", "Game")
        archive_name = self._archive_name_for_game(game)
        archive_path = install_path / archive_name
        rom_id_path = quote(rom_id, safe="")
        file_name_path = quote(archive_name, safe="")

        try:
            payload = self._api_get_bytes(f"/api/roms/{rom_id_path}/content/{file_name_path}")
            archive_path.write_bytes(payload)
            return archive_path
        except (HTTPError, URLError, OSError, ValueError):
            QMessageBox.warning(self, "Install Error", f"Failed to download {title} from server.")
            return None

    def _candidate_archive_paths_for_game(self, game: dict[str, str]) -> list[Path]:
        candidates: list[Path] = []
        archive_path_value = game.get("archive_path", "")
        if isinstance(archive_path_value, str) and archive_path_value.strip():
            candidates.append(Path(archive_path_value).expanduser())

        platform_library_path = self._platform_library_dir(game)
        if platform_library_path is not None:
            candidates.append(platform_library_path / self._archive_name_for_game(game))

        library_path = self._library_path_dir()
        if library_path is not None:
            candidates.append(library_path / self._archive_name_for_game(game))

        unique: list[Path] = []
        seen: set[str] = set()
        for candidate in candidates:
            key = str(candidate)
            if key in seen:
                continue
            seen.add(key)
            unique.append(candidate)
        return unique

    def _candidate_extracted_paths_for_game(self, game: dict[str, str]) -> list[Path]:
        candidates: list[Path] = []
        extracted_path_value = game.get("extracted_path", "")
        if isinstance(extracted_path_value, str) and extracted_path_value.strip():
            candidates.append(Path(extracted_path_value).expanduser())

        extracted_dir_value = game.get("extracted_dir", "")
        if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
            extracted_dir = Path(extracted_dir_value).expanduser()
            if extracted_dir.exists() and extracted_dir.is_dir():
                selected = self._select_extracted_launch_file(extracted_dir, Path(game.get("archive_path", "") or "archive"))
                if selected is not None:
                    candidates.append(selected)

        unique: list[Path] = []
        seen: set[str] = set()
        for candidate in candidates:
            key = str(candidate)
            if key in seen:
                continue
            seen.add(key)
            unique.append(candidate)
        return unique

    def _candidate_extracted_dirs_for_game(self, game: dict[str, str]) -> list[Path]:
        candidates: list[Path] = []
        extracted_dir_value = game.get("extracted_dir", "")
        if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
            candidates.append(Path(extracted_dir_value).expanduser())

        for archive_path in self._candidate_archive_paths_for_game(game):
            candidates.append(self._extracted_dir_for_archive_path(archive_path))

        unique: list[Path] = []
        seen: set[str] = set()
        for candidate in candidates:
            key = str(candidate)
            if key in seen:
                continue
            seen.add(key)
            unique.append(candidate)
        return unique

    def _remove_game_files(self, game: dict[str, str]) -> bool:
        removed_any = False
        for candidate in self._candidate_archive_paths_for_game(game):
            if not candidate.exists() or not candidate.is_file():
                continue
            try:
                candidate.unlink()
                removed_any = True
            except OSError as error:
                QMessageBox.warning(self, "Uninstall Error", f"Could not remove file: {candidate}\n{error}")
                return False

        for extracted_dir in self._candidate_extracted_dirs_for_game(game):
            if not extracted_dir.exists() or not extracted_dir.is_dir():
                continue
            try:
                shutil.rmtree(extracted_dir)
                removed_any = True
            except OSError as error:
                QMessageBox.warning(self, "Uninstall Error", f"Could not remove folder: {extracted_dir}\n{error}")
                return False
        return True if removed_any else True

    def _uninstall_game(self, game: dict[str, str]) -> bool:
        target = self._game_key(game)
        matching_games = [entry for entry in self.library_games if self._game_key(entry) == target]
        for entry in matching_games:
            if not self._remove_game_files(entry):
                return False
        before = len(self.library_games)
        self.library_games = [entry for entry in self.library_games if self._game_key(entry) != target]
        removed = len(self.library_games) != before
        if removed:
            self._refresh_library_grid()
            self._persist_installed_games()
        return removed

    def resizeEvent(self, event) -> None:
        super().resizeEvent(event)
        self._reflow_current_page_grid()
        self._update_details_layout_metrics()
        QTimer.singleShot(0, self._update_details_layout_metrics)

    def _open_game_details(self, game: dict[str, str], source: str) -> None:
        self.current_details_game = game
        self.current_details_source = source
        if self.details_title_label is not None:
            self.details_title_label.setText(game["title"])
        if self.details_cover_label is not None:
            self.details_cover_label.clear()
            self.details_cover_label.setText("Cover Art")
            cover_url = game.get("cover_url", "")
            if isinstance(cover_url, str) and cover_url.strip():
                self._queue_cover_load(cover_url, self.details_cover_label)
        if self.details_platform_label is not None:
            self.details_platform_label.setText(f"Platform: {game['platform']}")
        if self.details_rating_label is not None:
            self.details_rating_label.setText(f"Rating: {game['rating']}")
        if self.details_description_label is not None:
            self.details_description_label.setText(game["description"])
        self._update_details_screenshots(game)
        self._update_details_layout_metrics()
        self._update_details_action_buttons()

        self.stack.setCurrentIndex(5)
        for button in self.nav_buttons:
            button.setChecked(False)

    def _update_details_action_buttons(self) -> None:
        if self.current_details_game is None:
            return
        installed = self._is_game_installed(self.current_details_game)
        queued_current = self._is_game_install_queued(self.current_details_game)
        installing_current = (
            self.install_in_progress
            and self.install_pending_game is not None
            and self._game_key(self.current_details_game) == self._game_key(self.install_pending_game)
        )
        if self.details_primary_button is not None:
            if installing_current:
                button_text = "Installing..."
            elif queued_current:
                button_text = "Queued..."
            elif installed:
                button_text = "Play"
            else:
                button_text = "Install Game"
            self.details_primary_button.setText(button_text)
            self.details_primary_button.setEnabled(not installing_current and not queued_current)
        if self.details_secondary_button is not None:
            self.details_secondary_button.setVisible(installed)
            self.details_secondary_button.setEnabled(installed and not installing_current)

    def _is_game_install_queued(self, game: dict[str, str]) -> bool:
        target = self._game_key(game)
        return any(self._game_key(queued_game) == target for queued_game in self.install_queue)

    def _start_next_queued_install(self) -> None:
        if self.install_in_progress or not self.install_queue:
            self._update_details_action_buttons()
            self._update_download_status_ui()
            return
        next_game = self.install_queue.pop(0)
        self._start_async_install(next_game)

    def _details_rom_id_cache_key(self, game: dict[str, str] | None) -> str:
        if game is None:
            return ""
        title, platform = self._game_key(game)
        if not title or not platform:
            return ""
        return f"{title}::{platform}"

    def _details_rom_id_cache(self) -> dict[str, str]:
        raw = self.config.get("details_rom_id_cache", {})
        if not isinstance(raw, dict):
            return {}
        normalized: dict[str, str] = {}
        for key, value in raw.items():
            if not isinstance(key, str) or not isinstance(value, str):
                continue
            if not key.strip() or not value.strip():
                continue
            normalized[key.strip()] = value.strip()
        return normalized

    def _cache_rom_id_for_details_game(self, game: dict[str, str], rom_id: str) -> None:
        cache_key = self._details_rom_id_cache_key(game)
        if not cache_key or not rom_id.strip():
            return
        cache = self._details_rom_id_cache()
        cache[cache_key] = rom_id.strip()
        self.config["details_rom_id_cache"] = cache
        self._save_config(self.config)

    def _clear_cached_rom_id_for_details_game(self, game: dict[str, str] | None) -> None:
        cache_key = self._details_rom_id_cache_key(game)
        if not cache_key:
            return
        cache = self._details_rom_id_cache()
        if cache_key not in cache:
            return
        cache.pop(cache_key, None)
        if cache:
            self.config["details_rom_id_cache"] = cache
        else:
            self.config.pop("details_rom_id_cache", None)
        self._save_config(self.config)

    def _resolve_rom_id_for_game(self, game: dict[str, str]) -> str:
        direct = game.get("rom_id", "")
        if isinstance(direct, str) and direct.strip():
            return direct.strip()

        cache_key = self._details_rom_id_cache_key(game)
        cache = self._details_rom_id_cache()
        cached = cache.get(cache_key, "")
        if isinstance(cached, str) and cached.strip():
            return cached.strip()

        target = self._game_key(game)
        for games in self.server_games_by_platform.values():
            for server_game in games:
                if self._game_key(server_game) != target:
                    continue
                server_rom_id = server_game.get("rom_id", "")
                if isinstance(server_rom_id, str) and server_rom_id.strip():
                    return server_rom_id.strip()
        return ""

    def _cleanup_details_view_state(self) -> None:
        self._clear_cached_rom_id_for_details_game(self.current_details_game)
        self.current_details_game = None

    def _start_async_install(self, game: dict[str, str]) -> bool:
        rom_id = self._resolve_rom_id_for_game(game)
        if not rom_id:
            QMessageBox.warning(self, "Install Error", "Selected game is missing a ROM id and cannot be downloaded.")
            return False

        install_game = dict(game)
        install_game["rom_id"] = rom_id
        if self.current_details_game is not None and self._game_key(self.current_details_game) == self._game_key(install_game):
            self.current_details_game["rom_id"] = rom_id

        install_path = self._platform_library_dir(install_game)
        if install_path is None:
            QMessageBox.warning(self, "Install Error", "Set a Library Path in Settings before installing games.")
            return False

        try:
            install_path.mkdir(parents=True, exist_ok=True)
        except OSError as error:
            QMessageBox.warning(self, "Install Error", f"Could not prepare library folder: {error}")
            return False

        base_url = self._server_base_url()
        if not base_url:
            QMessageBox.warning(self, "Install Error", "Set a Server URL in Settings before installing games.")
            return False

        archive_name = self._archive_name_for_game(install_game)
        archive_path = install_path / archive_name
        rom_id_path = quote(rom_id, safe="")
        file_name_path = quote(archive_name, safe="")
        download_url = f"{base_url}/api/roms/{rom_id_path}/content/{file_name_path}"

        install_key = self._game_key(install_game)
        pending_key = self._game_key(self.install_pending_game) if self.install_pending_game is not None else None
        queued_keys = {self._game_key(queued_game) for queued_game in self.install_queue}

        if self.install_in_progress:
            if install_key == pending_key or install_key in queued_keys:
                QMessageBox.information(
                    self,
                    "Install In Progress",
                    f"{install_game['title']} is already downloading or queued.",
                )
                return False
            queued_game = dict(install_game)
            entry_id = self._create_download_entry(queued_game, "queued")
            queued_game["_download_entry_id"] = entry_id
            self.install_queue.append(queued_game)
            self._update_details_action_buttons()
            self._update_download_status_ui()
            QMessageBox.information(
                self,
                "Install Queued",
                f"Queued {install_game['title']} for download.",
            )
            return True

        pending_entry_id = game.get("_download_entry_id", "") if isinstance(game.get("_download_entry_id", ""), str) else ""
        entry_id = pending_entry_id.strip()
        if entry_id:
            self._set_download_entry_status(entry_id, "downloading")
        else:
            entry_id = self._create_download_entry(install_game, "downloading")

        self.install_in_progress = True
        install_game["_download_entry_id"] = entry_id
        self.install_pending_game = install_game
        self.active_download_entry_id = entry_id
        self.active_download_count += 1
        self.active_download_bytes = 0
        self.active_download_total = 0
        self.active_download_speed_bps = 0.0
        self._update_download_status_ui()
        self._update_details_action_buttons()

        thread = QThread(self)
        worker = InstallDownloadWorker(download_url, self._auth_headers(), archive_path)
        worker.moveToThread(thread)
        thread.started.connect(worker.run)
        worker.progress.connect(self._on_async_install_progress)
        worker.finished.connect(self._on_async_install_finished)
        worker.finished.connect(thread.quit)
        thread.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(self._on_install_thread_finished)

        self.install_thread = thread
        self.install_worker = worker
        thread.start()
        return True

    def _on_async_install_finished(self, archive_path: str, error: str) -> None:
        game = self.install_pending_game
        entry_id = self.active_download_entry_id
        self.install_in_progress = False
        self.install_pending_game = None
        self.active_download_entry_id = None
        self.active_download_count = max(0, self.active_download_count - 1)
        if self.active_download_count == 0:
            self.active_download_bytes = 0
            self.active_download_total = 0
            self.active_download_speed_bps = 0.0

        if game is None:
            if entry_id:
                status = "cancelled" if error and "cancel" in error.lower() else ("failed" if error else "completed")
                self._set_download_entry_status(entry_id, status, error)
            self._update_download_status_ui()
            self._update_details_action_buttons()
            self._start_next_queued_install()
            return

        title = game.get("title", "Game")
        if error:
            if entry_id:
                status = "cancelled" if "cancel" in error.lower() else "failed"
                self._set_download_entry_status(entry_id, status, error)
            self._update_download_status_ui()
            self._update_details_action_buttons()
            if "cancel" in error.lower():
                QMessageBox.information(self, "Install Cancelled", f"Cancelled download for {title}.")
            else:
                QMessageBox.warning(self, "Install Error", f"Failed to download {title} from server.")
            self._start_next_queued_install()
            return

        if self._is_game_installed(game):
            if entry_id:
                self._set_download_entry_status(entry_id, "completed")
            self._update_download_status_ui()
            self._update_details_action_buttons()
            QMessageBox.information(
                self,
                "Game Action",
                f"{title} is already installed.",
            )
            self._start_next_queued_install()
            return

        archive_file = Path(archive_path)
        installed_game = self._prepare_installed_game(game, archive_file)
        if installed_game is None:
            if entry_id:
                self._set_download_entry_status(entry_id, "failed", "Failed to extract downloaded archive")
            if archive_file.exists() and archive_file.is_file():
                try:
                    archive_file.unlink()
                except OSError:
                    pass
            self._update_download_status_ui()
            self._update_details_action_buttons()
            self._start_next_queued_install()
            return

        self._register_installed_game(installed_game, archive_file)
        configured_emulator = self._auto_configure_installed_emulator(installed_game, archive_file)
        if entry_id:
            self._set_download_entry_status(entry_id, "completed")
        self._update_download_status_ui()
        self._update_details_action_buttons()
        install_message = f"Installed {title} to Library."
        if configured_emulator:
            install_message = f"Installed {title} to Library and updated emulator settings."
        QMessageBox.information(
            self,
            "Install Complete",
            install_message,
        )
        self._start_next_queued_install()

    def _on_install_thread_finished(self) -> None:
        self.install_thread = None
        self.install_worker = None

    def _on_async_install_progress(self, downloaded_bytes: int, total_bytes: int, speed_bps: float) -> None:
        downloaded = int(downloaded_bytes) if isinstance(downloaded_bytes, (int, float)) else 0
        total = int(total_bytes) if isinstance(total_bytes, (int, float)) else 0
        self.active_download_bytes = max(0, downloaded)
        self.active_download_total = max(0, total)
        self.active_download_speed_bps = max(0.0, speed_bps)
        if self.active_download_entry_id:
            self._set_download_entry_progress(
                self.active_download_entry_id,
                self.active_download_bytes,
                self.active_download_total,
                self.active_download_speed_bps,
            )
        self._update_download_status_ui()

    def _format_size(self, size_bytes: float) -> str:
        units = ["B", "KB", "MB", "GB", "TB"]
        size = float(max(0.0, size_bytes))
        unit_index = 0
        while size >= 1024.0 and unit_index < len(units) - 1:
            size /= 1024.0
            unit_index += 1
        precision = 0 if unit_index == 0 else 1
        return f"{size:.{precision}f} {units[unit_index]}"

    def _update_download_status_ui(self) -> None:
        if self.download_status_widget is None:
            return
        queued_count = len(self.install_queue)
        has_active_downloads = self.active_download_count > 0
        has_install_work = has_active_downloads or queued_count > 0
        self.download_status_widget.setVisible(has_install_work)
        if self.download_count_label is not None:
            active_suffix = "s" if self.active_download_count != 1 else ""
            if queued_count > 0:
                queued_suffix = "s" if queued_count != 1 else ""
                self.download_count_label.setText(
                    f"{self.active_download_count} active download{active_suffix} ({queued_count} queued download{queued_suffix})"
                )
            else:
                self.download_count_label.setText(f"{self.active_download_count} active download{active_suffix}")
        if self.download_progress_bar is not None:
            if has_active_downloads and self.active_download_total > 0:
                percent = int((self.active_download_bytes * 100) / self.active_download_total)
                percent = max(0, min(100, percent))
                self.download_progress_bar.setRange(0, 100)
                self.download_progress_bar.setValue(percent)
                self.download_progress_bar.setFormat(f"{percent}%")
            elif has_active_downloads:
                self.download_progress_bar.setRange(0, 0)
                self.download_progress_bar.setFormat("Downloading...")
            elif queued_count > 0:
                self.download_progress_bar.setRange(0, 100)
                self.download_progress_bar.setValue(0)
                self.download_progress_bar.setFormat("Queued")
            else:
                self.download_progress_bar.setRange(0, 100)
                self.download_progress_bar.setValue(0)
                self.download_progress_bar.setFormat("0%")
        if self.download_speed_label is not None:
            speed_text = self._format_size(self.active_download_speed_bps)
            self.download_speed_label.setText(f"{speed_text}/s")

    def _create_download_entry(self, game: dict[str, Any], status: str, error: str = "") -> str:
        title_value = game.get("title", "Game")
        platform_value = game.get("platform", "")
        title = title_value.strip() if isinstance(title_value, str) and title_value.strip() else "Game"
        platform = platform_value.strip() if isinstance(platform_value, str) else ""
        entry_id = f"{time.time_ns()}-{len(self.download_entries)}"
        self.download_entries.append(
            {
                "id": entry_id,
                "game": dict(game),
                "title": title,
                "platform": platform,
                "status": status,
                "downloaded_bytes": 0,
                "total_bytes": 0,
                "speed_bps": 0.0,
                "error": error.strip(),
            }
        )
        self._schedule_downloads_page_refresh()
        return entry_id

    def _find_download_entry(self, entry_id: str) -> dict[str, Any] | None:
        for entry in self.download_entries:
            if entry.get("id") == entry_id:
                return entry
        return None

    def _set_download_entry_status(self, entry_id: str, status: str, error: str = "") -> None:
        entry = self._find_download_entry(entry_id)
        if entry is None:
            return
        entry["status"] = status
        entry["error"] = error.strip()
        if status in ("completed", "failed", "cancelled"):
            entry["speed_bps"] = 0.0
        self._schedule_downloads_page_refresh()

    def _set_download_entry_progress(self, entry_id: str, downloaded_bytes: int, total_bytes: int, speed_bps: float) -> None:
        entry = self._find_download_entry(entry_id)
        if entry is None:
            return
        entry["downloaded_bytes"] = max(0, downloaded_bytes)
        entry["total_bytes"] = max(0, total_bytes)
        entry["speed_bps"] = max(0.0, speed_bps)
        detail_label = self.download_entry_detail_labels.get(entry_id)
        if detail_label is not None:
            detail_label.setText(self._download_entry_detail_text(entry))

    def _dismiss_download_entry(self, entry_id: str) -> None:
        self.download_entries = [entry for entry in self.download_entries if entry.get("id") != entry_id]
        self._schedule_downloads_page_refresh()

    def _schedule_downloads_page_refresh(self) -> None:
        if self.downloads_refresh_timer.isActive():
            return
        self.downloads_refresh_timer.start()

    def _retry_download_entry(self, entry_id: str) -> None:
        entry = self._find_download_entry(entry_id)
        if entry is None:
            return
        status = entry.get("status", "")
        if status not in ("failed", "cancelled"):
            return
        game_value = entry.get("game")
        if not isinstance(game_value, dict):
            return
        game_copy = dict(game_value)
        game_copy.pop("_download_entry_id", None)
        self._dismiss_download_entry(entry_id)
        self._start_async_install(game_copy)

    def _cancel_download_entry(self, entry_id: str) -> None:
        if self.active_download_entry_id == entry_id and self.install_worker is not None:
            self.install_worker.request_cancel()
            self._set_download_entry_status(entry_id, "cancelling")
            return

        queued_before = len(self.install_queue)
        self.install_queue = [
            queued_game
            for queued_game in self.install_queue
            if queued_game.get("_download_entry_id") != entry_id
        ]
        if len(self.install_queue) != queued_before:
            self._set_download_entry_status(entry_id, "cancelled", "Cancelled while queued")
            self._update_download_status_ui()
            self._update_details_action_buttons()

    def _download_entry_detail_text(self, entry: dict[str, Any]) -> str:
        status = str(entry.get("status", ""))
        downloaded = int(entry.get("downloaded_bytes", 0))
        total = int(entry.get("total_bytes", 0))
        speed_bps = float(entry.get("speed_bps", 0.0))
        if status == "queued":
            return "Queued"
        if status == "downloading":
            if total > 0:
                percent = max(0, min(100, int((downloaded * 100) / total)))
                return (
                    f"Downloading {percent}% • {self._format_size(downloaded)} / {self._format_size(total)}"
                    f" • {self._format_size(speed_bps)}/s"
                )
            return f"Downloading • {self._format_size(downloaded)} • {self._format_size(speed_bps)}/s"
        if status == "cancelling":
            return "Cancelling..."
        if status == "completed":
            size_text = self._format_size(downloaded) if downloaded > 0 else "Unknown size"
            return f"Completed • {size_text}"
        if status == "failed":
            error_text = str(entry.get("error", "")).strip() or "Unknown error"
            return f"Failed • {error_text}"
        if status == "cancelled":
            return "Cancelled"
        return status.capitalize() or "Unknown"

    def _make_download_entry_widget(self, entry: dict[str, Any]) -> tuple[QWidget, QLabel]:
        frame = QFrame()
        frame.setObjectName("panel")
        frame_layout = QHBoxLayout(frame)
        frame_layout.setContentsMargins(12, 10, 12, 10)
        frame_layout.setSpacing(10)

        text_col = QVBoxLayout()
        text_col.setSpacing(4)

        title = str(entry.get("title", "Game"))
        platform = str(entry.get("platform", "")).strip()
        title_label = QLabel(title if not platform else f"{title} ({platform})")
        title_label.setStyleSheet("font-size: 16px; font-weight: 700;")
        text_col.addWidget(title_label)

        detail_label = QLabel(self._download_entry_detail_text(entry))
        detail_label.setWordWrap(True)
        detail_label.setStyleSheet("color: #cbd5e1;")
        text_col.addWidget(detail_label)

        frame_layout.addLayout(text_col, 1)

        actions = QHBoxLayout()
        actions.setSpacing(8)
        entry_id = str(entry.get("id", ""))
        status = str(entry.get("status", ""))

        if status in ("queued", "downloading", "cancelling"):
            cancel_button = QPushButton("Cancel")
            cancel_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            cancel_button.setEnabled(status != "cancelling")
            cancel_button.clicked.connect(lambda checked=False, target=entry_id: self._cancel_download_entry(target))
            actions.addWidget(cancel_button)
        elif status in ("failed", "cancelled"):
            retry_button = QPushButton("Retry")
            retry_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            retry_button.clicked.connect(lambda checked=False, target=entry_id: self._retry_download_entry(target))
            actions.addWidget(retry_button)

            cancel_button = QPushButton("Cancel")
            cancel_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            cancel_button.clicked.connect(lambda checked=False, target=entry_id: self._dismiss_download_entry(target))
            actions.addWidget(cancel_button)
        else:
            dismiss_button = QPushButton("Dismiss")
            dismiss_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            dismiss_button.clicked.connect(lambda checked=False, target=entry_id: self._dismiss_download_entry(target))
            actions.addWidget(dismiss_button)

        frame_layout.addLayout(actions)
        return frame, detail_label

    def _refresh_downloads_page(self) -> None:
        if self.downloads_list_layout is None or self.downloads_empty_label is None or self.downloads_scroll is None:
            return

        self.download_entry_detail_labels = {}
        while self.downloads_list_layout.count() > 0:
            item = self.downloads_list_layout.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.deleteLater()

        has_entries = len(self.download_entries) > 0
        self.downloads_empty_label.setVisible(not has_entries)
        self.downloads_scroll.setVisible(has_entries)
        if not has_entries:
            return

        for entry in reversed(self.download_entries):
            entry_id = str(entry.get("id", ""))
            widget, detail_label = self._make_download_entry_widget(entry)
            if entry_id:
                self.download_entry_detail_labels[entry_id] = detail_label
            self.downloads_list_layout.addWidget(widget)
        self.downloads_list_layout.addStretch()

    def _return_from_details(self) -> None:
        self._switch_page(self.current_main_page_index)

    def _mapping_value_for_platform(self, mapping: dict[str, str], platform: str) -> str:
        target = platform.strip()
        if not target:
            return ""

        direct = mapping.get(target, "")
        if isinstance(direct, str) and direct.strip():
            return direct.strip()

        folded = target.casefold()
        for key, value in mapping.items():
            if not isinstance(key, str) or not isinstance(value, str):
                continue
            if key.strip().casefold() != folded:
                continue
            if value.strip():
                return value.strip()
        return ""

    def _default_emulator_name_for_platform(self, platform: str) -> str:
        defaults = self._normalize_default_emulators(self.config.get("default_emulators", {}))
        configured = self._mapping_value_for_platform(defaults, platform)
        if configured:
            return configured

        emulators = self._normalize_emulators(self._emulators())
        if emulators:
            fallback_name = emulators[0].get("name", "")
            if isinstance(fallback_name, str):
                return fallback_name.strip()
        return ""

    def _emulator_entry_by_name(self, emulator_name: str) -> dict[str, str] | None:
        target = emulator_name.strip().lower()
        if not target:
            return None
        for emulator in self._normalize_emulators(self._emulators()):
            name = emulator.get("name", "")
            if isinstance(name, str) and name.strip().lower() == target:
                return emulator
        return None

    def _launch_placeholders_for_game(self, game: dict[str, str], emulator_name: str) -> dict[str, str]:
        rom_path = self._resolved_rom_path_for_game(game)

        core_value = ""
        if self._is_retroarch_emulator_name(emulator_name):
            core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
            platform = game.get("platform", "")
            if isinstance(platform, str) and platform.strip():
                configured_core = self._mapping_value_for_platform(core_defaults, platform)
                if configured_core:
                    core_value = configured_core

        return {
            "%rom%": rom_path,
            "%core%": core_value,
        }

    def _apply_launch_placeholders_to_args(self, args: list[str], placeholders: dict[str, str]) -> list[str]:
        resolved_args: list[str] = []
        core_value = placeholders.get("%core%", "")
        core_missing = not core_value.strip()
        for arg in args:
            had_core_placeholder = "%core%" in arg
            resolved = arg
            for token, value in placeholders.items():
                resolved = resolved.replace(token, value)
            if had_core_placeholder and core_missing:
                if resolved_args and resolved_args[-1] in {"-L", "--libretro", "--core"}:
                    resolved_args.pop()
                continue
            if resolved:
                resolved_args.append(resolved)
        return resolved_args

    def _resolved_launch_arguments_for_game(self, game: dict[str, str]) -> tuple[str, list[str]]:
        platform_value = game.get("platform", "")
        platform = platform_value.strip() if isinstance(platform_value, str) else ""
        emulator_name = self._default_emulator_name_for_platform(platform)
        emulator_entry = self._emulator_entry_by_name(emulator_name)

        emulator_args = "%rom%"
        if emulator_entry is not None:
            args_value = emulator_entry.get("args", "%rom%")
            if isinstance(args_value, str) and args_value.strip():
                emulator_args = args_value.strip()

        global_args_value = self.config.get("launch_args", "")
        global_args = global_args_value.strip() if isinstance(global_args_value, str) else ""
        combined_template = " ".join(part for part in (emulator_args, global_args) if part).strip()

        parsed_template_args = shlex.split(combined_template, posix=False) if combined_template else []
        placeholders = self._launch_placeholders_for_game(game, emulator_name)
        if "%core%" in combined_template and not placeholders.get("%core%", "").strip():
            raise ValueError("No RetroArch core is configured for this platform. Set one in Emulators > Defaults.")
        resolved_args = self._apply_launch_placeholders_to_args(parsed_template_args, placeholders)
        return emulator_name, resolved_args

    def _resolved_rom_path_for_game(self, game: dict[str, str]) -> str:
        if not self._is_arcade_platform(game):
            for candidate in self._candidate_extracted_paths_for_game(game):
                if candidate.exists() and candidate.is_file():
                    return str(candidate)

        for candidate in self._candidate_archive_paths_for_game(game):
            if candidate.exists() and candidate.is_file():
                return str(candidate)
        archive_path_value = game.get("archive_path", "")
        if isinstance(archive_path_value, str):
            return archive_path_value.strip()
        return ""

    def _launch_installed_game(self, game: dict[str, str]) -> bool:
        platform_value = game.get("platform", "")
        platform = platform_value.strip() if isinstance(platform_value, str) else ""
        emulator_name = self._default_emulator_name_for_platform(platform)
        if not emulator_name:
            QMessageBox.warning(self, "Launch Error", "No emulator is configured. Add one in Emulators settings.")
            return False

        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None:
            QMessageBox.warning(self, "Launch Error", f"Default emulator '{emulator_name}' was not found.")
            return False

        emulator_path_value = emulator_entry.get("path", "")
        emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
        if not emulator_path_text:
            QMessageBox.warning(self, "Launch Error", f"Emulator '{emulator_name}' has no executable path configured.")
            return False

        emulator_path = Path(emulator_path_text).expanduser()
        if not emulator_path.exists() or not emulator_path.is_file():
            QMessageBox.warning(self, "Launch Error", f"Emulator executable not found:\n{emulator_path}")
            return False

        rom_path = self._resolved_rom_path_for_game(game)
        if not rom_path:
            QMessageBox.warning(self, "Launch Error", "No ROM file is available for this game.")
            return False

        rom_file = Path(rom_path).expanduser()
        if not rom_file.exists() or not rom_file.is_file():
            QMessageBox.warning(self, "Launch Error", f"ROM file not found:\n{rom_file}")
            return False

        try:
            _, parsed_args = self._resolved_launch_arguments_for_game(game)
        except ValueError as error:
            QMessageBox.warning(self, "Launch Error", f"Invalid launch arguments: {error}")
            return False

        command = [str(emulator_path), *parsed_args]
        try:
            process = subprocess.Popen(command, cwd=str(emulator_path.parent))
            QTimer.singleShot(500, lambda p=process, c=command: self._warn_if_process_exited_early(p, c))
            return True
        except OSError as error:
            QMessageBox.warning(self, "Launch Error", f"Failed to launch game:\n{error}")
            return False

    def _warn_if_process_exited_early(self, process: subprocess.Popen, command: list[str]) -> None:
        exit_code = process.poll()
        if exit_code is None:
            return
        command_text = " ".join(command)
        QMessageBox.warning(
            self,
            "Launch Error",
            f"Emulator exited immediately (code {exit_code}).\nCommand:\n{command_text}",
        )

    def _perform_game_action(self) -> None:
        if self.current_details_game is None:
            return
        if self._is_game_installed(self.current_details_game):
            installed_game = self._installed_game_record(self.current_details_game)
            launch_game = installed_game if installed_game is not None else self.current_details_game
            self._launch_installed_game(launch_game)
            return

        self._start_async_install(self.current_details_game)

    def _perform_game_secondary_action(self) -> None:
        if self.current_details_game is None:
            return
        if not self._is_game_installed(self.current_details_game):
            return

        rom_id = self._resolve_rom_id_for_game(self.current_details_game)
        if rom_id:
            self.current_details_game["rom_id"] = rom_id
            self._cache_rom_id_for_details_game(self.current_details_game, rom_id)

        if self._uninstall_game(self.current_details_game):
            self._update_details_action_buttons()
            QMessageBox.information(
                self,
                "Uninstall Complete",
                f"Removed {self.current_details_game['title']} from Library.",
            )
            return

        QMessageBox.information(
            self,
            "Game Action",
            f"{self.current_details_game['title']} is not currently installed.",
        )

    def _normalize_emulators(self, value: Any) -> list[dict[str, str]]:
        if not isinstance(value, list):
            return []
        normalized: list[dict[str, str]] = []
        for item in value:
            if not isinstance(item, dict):
                continue
            name = item.get("name")
            path = item.get("path")
            args = item.get("args")
            if not isinstance(name, str) or not name.strip():
                continue
            if not isinstance(path, str):
                path = ""
            if not isinstance(args, str):
                args = "%rom%"
            normalized.append(
                {
                    "name": name.strip(),
                    "path": path.strip(),
                    "args": args.strip() or "%rom%",
                }
            )
        normalized.sort(key=lambda emulator: emulator["name"].lower())
        return normalized

    def _normalize_default_emulators(self, value: Any) -> dict[str, str]:
        if not isinstance(value, dict):
            return {}
        normalized: dict[str, str] = {}
        for key, item in value.items():
            if isinstance(key, str) and key.strip() and isinstance(item, str):
                normalized[key.strip()] = item
        return normalized

    def _normalize_default_retroarch_cores(self, value: Any) -> dict[str, str]:
        if not isinstance(value, dict):
            return {}
        normalized: dict[str, str] = {}
        for key, item in value.items():
            if isinstance(key, str) and key.strip() and isinstance(item, str) and item.strip():
                normalized[key.strip()] = item.strip()
        return normalized

    def _is_retroarch_emulator_name(self, emulator_name: str) -> bool:
        return "retroarch" in emulator_name.strip().lower()

    def _retroarch_core_list_path(self) -> Path:
        return Path(__file__).resolve().parent / "retroarch-core-list.md"

    def _retroarch_markdown_label(self, value: str) -> str:
        text = value.strip()
        if not text.startswith("["):
            return text
        marker = text.find("](")
        if marker <= 1 or not text.endswith(")"):
            return text
        return text[1:marker].strip()

    def _retroarch_core_id_from_name(self, core_name: str) -> str:
        normalized_name = self._retroarch_markdown_label(core_name).strip().casefold()
        overrides = {
            "beetle psx": "mednafen_psx",
            "beetle psx hw": "mednafen_psx_hw",
            "beetle saturn": "mednafen_saturn",
            "beetle vb": "mednafen_vb",
            "fb neo": "fbneo",
            "fceumm": "fceumm",
            "flycast gles2": "flycast",
            "lrps2": "lrps2",
            "mame 2003-plus": "mame2003_plus",
            "mesen-s": "mesen_s",
            "mupen64plus-next": "mupen64plus_next",
            "mupen64plus-next gles2": "mupen64plus_next",
            "mupen64plus-next gles3": "mupen64plus_next",
            "parallel n64": "parallel_n64",
            "pcsx rearmed": "pcsx_rearmed",
            "snes9x 2002": "snes9x2002",
            "snes9x 2005": "snes9x2005",
            "snes9x 2005 plus": "snes9x2005_plus",
            "snes9x 2010": "snes9x2010",
            "same cdi": "same_cdi",
            "vba-m": "vbam",
            "vba next": "vba_next",
        }
        mapped = overrides.get(normalized_name)
        if mapped:
            return mapped

        sanitized: list[str] = []
        previous_underscore = False
        for character in normalized_name:
            if character.isalnum():
                sanitized.append(character)
                previous_underscore = False
                continue
            if previous_underscore:
                continue
            sanitized.append("_")
            previous_underscore = True

        return "".join(sanitized).strip("_")

    def _retroarch_compatibility_map_from_markdown(self) -> dict[str, list[str]]:
        if isinstance(self.retroarch_compatibility_map, dict):
            return self.retroarch_compatibility_map

        path = self._retroarch_core_list_path()
        compatibility: dict[str, list[str]] = {}
        if not path.exists():
            self.retroarch_compatibility_map = compatibility
            return compatibility

        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except OSError:
            self.retroarch_compatibility_map = compatibility
            return compatibility

        for line in lines:
            if not line.strip().startswith("|"):
                continue
            columns = [column.strip() for column in line.split("|")]
            if len(columns) < 4:
                continue

            core_cell = columns[1]
            system_cell = columns[2]
            if not core_cell or not system_cell:
                continue
            if core_cell.casefold() == "core" or system_cell.startswith(":") or system_cell == "-":
                continue

            core_id = self._retroarch_core_id_from_name(core_cell)
            system_key = system_cell.casefold()
            if not core_id or not system_key:
                continue

            known = compatibility.setdefault(system_key, [])
            if core_id not in known:
                known.append(core_id)

        self.retroarch_compatibility_map = compatibility
        return compatibility

    def _retroarch_system_keys_for_platform(self, platform: str) -> list[str]:
        normalized = platform.strip().casefold()
        if not normalized:
            return []

        aliases = (
            ("arcade", ["arcade", "arcade/console/various"]),
            ("mame", ["arcade", "arcade/console/various"]),
            ("final burn", ["arcade/console/various"]),
            ("fbneo", ["arcade/console/various"]),
            ("nintendo entertainment", ["nintendo nes/famicom"]),
            ("nes", ["nintendo nes/famicom"]),
            ("super nintendo", ["nintendo snes/sfc"]),
            ("snes", ["nintendo snes/sfc"]),
            ("nintendo 64", ["nintendo 64"]),
            ("n64", ["nintendo 64"]),
            ("game boy advance", ["game boy advance"]),
            ("gba", ["game boy advance"]),
            ("game boy color", ["game boy/color"]),
            ("game boy", ["game boy/color"]),
            ("gbc", ["game boy/color"]),
            ("gb", ["game boy/color"]),
            ("nintendo ds", ["nintendo ds", "nintendo ds/dsi"]),
            ("nintendo 3ds", ["nintendo 3ds"]),
            ("playstation 2", ["sony playstation 2"]),
            ("ps2", ["sony playstation 2"]),
            ("playstation portable", ["playstation portable"]),
            ("psp", ["playstation portable"]),
            ("playstation", ["sony playstation"]),
            ("ps1", ["sony playstation"]),
            ("dreamcast", ["sega dreamcast/naomi"]),
            ("naomi", ["sega dreamcast/naomi"]),
            ("saturn", ["sega saturn", "sega saturn/st-v"]),
            ("megadrive", ["sega genesis (mega drive)", "sega ms/gg/md/cd", "sega ms/gg/md/cd/32x"]),
            ("mega drive", ["sega genesis (mega drive)", "sega ms/gg/md/cd", "sega ms/gg/md/cd/32x"]),
            ("genesis", ["sega genesis (mega drive)", "sega ms/gg/md/cd", "sega ms/gg/md/cd/32x"]),
            ("sega cd", ["sega md/cd", "sega ms/gg/md/cd", "sega ms/gg/md/cd/32x"]),
            ("32x", ["sega ms/gg/md/cd/32x"]),
            ("master system", ["sega master system", "sega ms/gg", "sega ms/gg/sg-1000"]),
            ("game gear", ["sega ms/gg", "sega ms/gg/sg-1000"]),
            ("pc engine", ["nec pc engine/supergrafx/cd", "nec pc engine/supergrafx", "nec pc engine/cd"]),
            ("turbografx", ["nec pc engine/supergrafx/cd", "nec pc engine/supergrafx", "nec pc engine/cd"]),
            ("atari lynx", ["atari lynx"]),
            ("atari jaguar", ["atari jaguar"]),
            ("atari 7800", ["atari 7800"]),
            ("atari 5200", ["atari 5200"]),
            ("neo geo pocket", ["neo geo pocket/color"]),
            ("neo geo cd", ["neo geo cd"]),
            ("neo geo", ["snk neo geo aes/mvs", "neo geo", "arcade/console/various"]),
            ("3do", ["3do"]),
            ("wonderswan", ["bandai wonderswan/color"]),
            ("virtual boy", ["nintendo virtual boy"]),
            ("dos", ["dos"]),
            ("amiga", ["commodore amiga"]),
            ("msx", ["msx/msx2/msx2+", "msx/svi/colecovision/sg-1000"]),
            ("coleco", ["coleco colecovision", "colecovision/creativision/my vision", "msx/svi/colecovision/sg-1000"]),
        )

        keys: list[str] = []
        for alias, systems in aliases:
            if alias in normalized:
                for system in systems:
                    if system not in keys:
                        keys.append(system)
        return keys

    def _retroarch_cores_for_platform(self, platform: str) -> list[str]:
        compatibility = self._retroarch_compatibility_map_from_markdown()
        if not compatibility:
            return ["fbneo", "mame2003_plus"]

        resolved_cores: list[str] = []
        for system_key in self._retroarch_system_keys_for_platform(platform):
            for core in compatibility.get(system_key, []):
                if core not in resolved_cores:
                    resolved_cores.append(core)

        normalized_platform = platform.strip().casefold()
        if normalized_platform:
            for system_key, cores in compatibility.items():
                if normalized_platform not in system_key and system_key not in normalized_platform:
                    continue
                for core in cores:
                    if core not in resolved_cores:
                        resolved_cores.append(core)

        if resolved_cores:
            return resolved_cores
        return ["fbneo", "mame2003_plus"]

    def _refresh_retroarch_core_options(self) -> None:
        if self.default_core_combo is None or self.default_platform_combo is None or self.default_emulator_combo is None:
            return

        platform = self.default_platform_combo.currentText().strip()
        emulator_name = self.default_emulator_combo.currentText().strip()
        is_retroarch = bool(platform) and self._is_retroarch_emulator_name(emulator_name)

        core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
        self.config["default_retroarch_cores"] = core_defaults
        saved_core = core_defaults.get(platform, "") if platform else ""

        self.default_core_combo.blockSignals(True)
        self.default_core_combo.clear()
        for core in self._retroarch_cores_for_platform(platform):
            self.default_core_combo.addItem(core, core)

        if not is_retroarch:
            self.default_core_combo.setCurrentIndex(-1)
        else:
            if saved_core:
                saved_index = self.default_core_combo.findData(saved_core)
                if saved_index < 0:
                    self.default_core_combo.addItem(saved_core, saved_core)
                    saved_index = self.default_core_combo.findData(saved_core)
                if saved_index >= 0:
                    self.default_core_combo.setCurrentIndex(saved_index)
            else:
                self.default_core_combo.setCurrentIndex(0)

        self.default_core_combo.setEnabled(is_retroarch)
        self.default_core_combo.blockSignals(False)

    def _on_default_platform_changed(self, platform: str) -> None:
        if self.default_emulator_combo is None:
            return

        defaults = self._normalize_default_emulators(self.config.get("default_emulators", {}))
        self.config["default_emulators"] = defaults
        preferred_emulator = self._mapping_value_for_platform(defaults, platform)

        if preferred_emulator:
            self.default_emulator_combo.setCurrentText(preferred_emulator)

        if not self.default_emulator_combo.currentText().strip() and self.default_emulator_combo.count() > 0:
            self.default_emulator_combo.setCurrentIndex(0)

        self._refresh_retroarch_core_options()

    def _normalize_installed_games(self, value: Any) -> list[dict[str, str]]:
        if not isinstance(value, list):
            return []

        normalized: list[dict[str, str]] = []
        seen: set[tuple[str, str]] = set()
        for item in value:
            if not isinstance(item, dict):
                continue
            title = item.get("title")
            platform = item.get("platform")
            if not isinstance(title, str) or not title.strip():
                continue
            if not isinstance(platform, str) or not platform.strip():
                continue
            rating = item.get("rating")
            description = item.get("description")
            cover_url = item.get("cover_url")
            screenshot_urls = item.get("screenshot_urls")
            rom_file_name = item.get("rom_file_name")
            extracted_path = item.get("extracted_path")
            extracted_dir = item.get("extracted_dir")
            archive_path = item.get("archive_path")
            normalized_game = {
                "title": title.strip(),
                "platform": platform.strip(),
                "rating": rating.strip() if isinstance(rating, str) and rating.strip() else "N/A",
                "description": description.strip() if isinstance(description, str) and description.strip() else "No description available.",
                "cover_url": cover_url.strip() if isinstance(cover_url, str) else "",
                "screenshot_urls": screenshot_urls.strip() if isinstance(screenshot_urls, str) else "",
                "rom_file_name": rom_file_name.strip() if isinstance(rom_file_name, str) else "",
                "extracted_path": extracted_path.strip() if isinstance(extracted_path, str) else "",
                "extracted_dir": extracted_dir.strip() if isinstance(extracted_dir, str) else "",
                "archive_path": archive_path.strip() if isinstance(archive_path, str) else "",
            }
            key = self._game_key(normalized_game)
            if key in seen:
                continue
            seen.add(key)
            normalized.append(normalized_game)
        return normalized

    def _emulators(self) -> list[dict[str, str]]:
        emulators = self.config.get("emulators", [])
        if not isinstance(emulators, list):
            return []
        return emulators

    def _refresh_emulator_views(self) -> None:
        if self.emulator_list is None:
            return
        emulators = self._normalize_emulators(self._emulators())
        self.config["emulators"] = emulators

        self.emulator_list.clear()
        self.emulator_list.addItems([entry["name"] for entry in emulators])

        if self.default_emulator_combo is not None:
            selected_name = self.default_emulator_combo.currentText()
            self.default_emulator_combo.clear()
            self.default_emulator_combo.addItems([entry["name"] for entry in emulators])
            if selected_name:
                self.default_emulator_combo.setCurrentText(selected_name)

        server_platforms = self._default_assignable_server_platforms()
        if self.default_platform_combo is not None:
            selected_platform = self.default_platform_combo.currentText()
            self.default_platform_combo.clear()
            self.default_platform_combo.addItems(server_platforms)
            if selected_platform:
                self.default_platform_combo.setCurrentText(selected_platform)

        if self.default_mapping_list is not None:
            self.default_mapping_list.clear()
            defaults = self._normalize_default_emulators(self.config.get("default_emulators", {}))
            core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
            self.config["default_emulators"] = defaults
            self.config["default_retroarch_cores"] = core_defaults
            for platform in server_platforms:
                emulator_name = defaults.get(platform, "(none)")
                if emulator_name != "(none)" and self._is_retroarch_emulator_name(emulator_name):
                    core_name = core_defaults.get(platform, "")
                    suffix = f" ({core_name})" if core_name else ""
                    self.default_mapping_list.addItem(f"{platform}: {emulator_name}{suffix}")
                else:
                    self.default_mapping_list.addItem(f"{platform}: {emulator_name}")

        selected_platform = self.default_platform_combo.currentText() if self.default_platform_combo is not None else ""
        self._on_default_platform_changed(selected_platform)

    def _load_emulator_from_selection(self, row: int) -> None:
        if self.emulator_name_input is None or self.emulator_path_input is None or self.emulator_args_input is None:
            return
        emulators = self._emulators()
        if row < 0 or row >= len(emulators):
            self.emulator_name_input.clear()
            self.emulator_path_input.clear()
            self.emulator_args_input.setText("%rom%")
            return
        emulator = emulators[row]
        self.emulator_name_input.setText(emulator["name"])
        self.emulator_path_input.setText(emulator["path"])
        self.emulator_args_input.setText(emulator["args"])

    def _save_emulator(self) -> None:
        if self.emulator_name_input is None or self.emulator_path_input is None or self.emulator_args_input is None:
            return
        name = self.emulator_name_input.text().strip()
        if not name:
            QMessageBox.warning(self, "Validation", "Emulator name is required")
            return

        path = self.emulator_path_input.text().strip()
        args = self.emulator_args_input.text().strip() or "%rom%"

        emulators = self._emulators()
        target_index = -1
        if self.emulator_list is not None:
            target_index = self.emulator_list.currentRow()

        if 0 <= target_index < len(emulators):
            emulators[target_index] = {"name": name, "path": path, "args": args}
        else:
            emulators.append({"name": name, "path": path, "args": args})

        self.config["emulators"] = self._normalize_emulators(emulators)
        self._refresh_emulator_views()
        self._save_config(self.config)

    def _remove_emulator(self) -> None:
        if self.emulator_list is None:
            return
        index = self.emulator_list.currentRow()
        emulators = self._emulators()
        if index < 0 or index >= len(emulators):
            return

        removed_name = emulators[index]["name"]
        emulators.pop(index)
        self.config["emulators"] = self._normalize_emulators(emulators)

        defaults = self._normalize_default_emulators(self.config.get("default_emulators", {}))
        for platform in list(defaults.keys()):
            if defaults[platform] == removed_name:
                defaults.pop(platform)
        self.config["default_emulators"] = defaults

        core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
        for platform in list(core_defaults.keys()):
            if platform not in defaults:
                core_defaults.pop(platform)
        self.config["default_retroarch_cores"] = core_defaults

        self._refresh_emulator_views()
        self._save_config(self.config)

    def _clear_emulator_selection(self) -> None:
        if self.emulator_list is not None:
            self.emulator_list.setCurrentRow(-1)
        self._load_emulator_from_selection(-1)

    def _launch_selected_emulator(self) -> None:
        if self.emulator_list is None:
            return

        index = self.emulator_list.currentRow()
        emulators = self._emulators()
        if index < 0 or index >= len(emulators):
            QMessageBox.information(self, "Launch Emulator", "Select an emulator to launch.")
            return

        emulator = emulators[index]
        emulator_name = emulator.get("name", "Emulator")
        emulator_path_value = emulator.get("path", "")
        emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
        if not emulator_path_text:
            QMessageBox.warning(self, "Launch Error", f"Emulator '{emulator_name}' has no executable path configured.")
            return

        emulator_path = Path(emulator_path_text).expanduser()
        if not emulator_path.exists() or not emulator_path.is_file():
            QMessageBox.warning(self, "Launch Error", f"Emulator executable not found:\n{emulator_path}")
            return

        command = [str(emulator_path)]
        try:
            process = subprocess.Popen(command, cwd=str(emulator_path.parent))
            QTimer.singleShot(500, lambda p=process, c=command: self._warn_if_process_exited_early(p, c))
        except OSError as error:
            QMessageBox.warning(self, "Launch Error", f"Failed to launch emulator:\n{error}")

    def _set_default_emulator(self) -> None:
        if self.default_platform_combo is None or self.default_emulator_combo is None:
            return
        platform = self.default_platform_combo.currentText().strip()
        emulator_name = self.default_emulator_combo.currentText().strip()
        if not platform or not emulator_name:
            return

        defaults = self._normalize_default_emulators(self.config.get("default_emulators", {}))
        defaults[platform] = emulator_name
        self.config["default_emulators"] = defaults

        core_defaults = self._normalize_default_retroarch_cores(self.config.get("default_retroarch_cores", {}))
        if self._is_retroarch_emulator_name(emulator_name):
            selected_core = ""
            if self.default_core_combo is not None:
                data = self.default_core_combo.currentData()
                if isinstance(data, str):
                    selected_core = data.strip()
            if selected_core:
                core_defaults[platform] = selected_core
            else:
                QMessageBox.warning(self, "Validation", "Select a RetroArch core before setting this default.")
                return
        else:
            core_defaults.pop(platform, None)
        self.config["default_retroarch_cores"] = core_defaults

        self._refresh_emulator_views()
        self._save_config(self.config)

    def _browse_emulator_path(self) -> None:
        if self.emulator_path_input is None:
            return

        selected_file, _ = QFileDialog.getOpenFileName(
            self,
            "Select Emulator Executable",
            self.emulator_path_input.text().strip(),
            "Executables (*.exe);;All Files (*)",
        )
        if selected_file:
            self.emulator_path_input.setText(selected_file)

    def _browse_library_path(self) -> None:
        if self.library_path_input is None:
            return

        current_path = self.library_path_input.text().strip()
        selected_directory = QFileDialog.getExistingDirectory(
            self,
            "Select Library Folder",
            current_path,
        )
        if selected_directory:
            self.library_path_input.setText(selected_directory)

    def _clear_layout(self, layout: QGridLayout) -> None:
        while layout.count() > 0:
            item = layout.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.deleteLater()


def main() -> None:
    app = QApplication(sys.argv)
    window = MainWindow()
    window.show()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
