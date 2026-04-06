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
from PySide6.QtGui import QCloseEvent, QDesktopServices, QPalette, QPixmap
from PySide6.QtNetwork import QNetworkAccessManager, QNetworkReply, QNetworkRequest
from PySide6.QtWidgets import (
    QApplication,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFrame,
    QFormLayout,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
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
            if self.archive_path.exists() and self.archive_path.is_file():
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
        self.active_theme_choice = self._normalized_theme_choice(self.config.get("theme", "system"))
        self.active_theme_variant = self._resolved_theme_variant(self.active_theme_choice)
        self.active_theme_colors = self._theme_colors(self.active_theme_variant)
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
        self.details_config_button: QPushButton | None = None
        self.details_secondary_button: QPushButton | None = None
        self.server_platforms_list: QListWidget | None = None
        self.server_games_grid: QGridLayout | None = None
        self.server_games_content: QWidget | None = None
        self.server_search_input: QLineEdit | None = None
        self.server_search_clear_button: QPushButton | None = None
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
        self.emulator_autoprofiles: list[dict[str, Any]] | None = None

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
        download_count.setStyleSheet(f"font-weight: 600; color: {self._theme_color('text', '#f8f8f2')};")
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
        download_speed.setStyleSheet(f"font-weight: 600; color: {self._theme_color('accent', '#8be9fd')};")
        self.download_speed_label = download_speed
        download_layout.addWidget(download_speed)

        download_widget.setObjectName("downloadStatusWidget")
        download_widget.setVisible(False)
        self.download_status_widget = download_widget
        nav_row.addWidget(download_widget, 0, Qt.AlignmentFlag.AlignHCenter)
        nav_row.addStretch()

        self.account_status_label = QLabel()
        self.account_status_label.setStyleSheet(f"font-weight: 600; color: {self._theme_color('text', '#f8f8f2')};")
        nav_row.addWidget(self.account_status_label)

        nav_row.addWidget(nav_buttons_by_label["Emulators"])
        nav_row.addWidget(nav_buttons_by_label["Settings"])
        self._update_top_bar_identity()
        self._switch_page(0)
        self._apply_theme(self.active_theme_choice)

        self._refresh_library_grid()
        self._refresh_emulator_views()
        self._restore_window_geometry()
        self._connect_to_server(show_errors=False)

    def closeEvent(self, event: QCloseEvent) -> None:
        self._persist_window_geometry()
        super().closeEvent(event)

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
        empty_label.setStyleSheet("color: #6272a4; font-size: 16px; font-weight: 600;")
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

        header_row = QHBoxLayout()
        header_row.setContentsMargins(0, 0, 0, 0)
        header_row.setSpacing(8)

        header = QLabel("Server Games")
        header.setStyleSheet("font-size: 20px; font-weight: 700;")
        header_row.addWidget(header)
        header_row.addStretch()

        search_container = QFrame()
        search_container.setObjectName("serverSearchContainer")
        search_container.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        search_container.setFixedWidth(260)
        search_layout = QHBoxLayout(search_container)
        search_layout.setContentsMargins(0, 0, 0, 0)
        search_layout.setSpacing(0)

        server_search_input = QLineEdit()
        server_search_input.setObjectName("serverSearchInput")
        server_search_input.setPlaceholderText("Search games...")
        server_search_input.setClearButtonEnabled(False)
        server_search_input.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        server_search_input.textChanged.connect(self._on_server_search_changed)
        self.server_search_input = server_search_input
        search_layout.addWidget(server_search_input)

        server_search_clear_button = QPushButton("X")
        server_search_clear_button.setObjectName("serverSearchClearButton")
        server_search_clear_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        server_search_clear_button.setCursor(Qt.CursorShape.PointingHandCursor)
        server_search_clear_button.setFixedWidth(26)
        server_search_clear_button.setFixedHeight(28)
        server_search_clear_button.setVisible(False)
        server_search_clear_button.clicked.connect(self._clear_server_search)
        self.server_search_clear_button = server_search_clear_button
        search_layout.addWidget(server_search_clear_button)

        header_row.addWidget(search_container)

        right_layout.addLayout(header_row)

        self.server_status_label = QLabel("Not connected")
        self.server_status_label.setStyleSheet("color: #ff5555;")
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
        empty_label.setStyleSheet("color: #6272a4; font-size: 16px; font-weight: 600;")
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
        self.theme_input.addItems(["system", "dark", "light"])
        self.theme_input.setCurrentText(self._normalized_theme_choice(self.config.get("theme", "system")))
        self.theme_input.currentTextChanged.connect(self._on_theme_selection_changed)
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
            "background-color: #282a36; border: 1px dashed #6272a4; border-radius: 8px; font-size: 20px;"
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

        config_button = QPushButton("Config")
        config_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        config_button.clicked.connect(self._perform_game_config_action)
        self.details_config_button = config_button
        action_row.addWidget(config_button)

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
                "background-color: #282a36; border: 1px dashed #6272a4; border-radius: 8px;"
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

    def _normalized_theme_choice(self, value: Any) -> str:
        if not isinstance(value, str):
            return "system"
        normalized = value.strip().casefold()
        if normalized in {"system", "dark", "light"}:
            return normalized
        return "system"

    def _resolved_theme_variant(self, theme_choice: str) -> str:
        if theme_choice != "system":
            return theme_choice
        app = QApplication.instance()
        if app is None:
            return "dark"
        window_color = app.palette().color(QPalette.ColorRole.Window)
        return "dark" if window_color.lightness() < 128 else "light"

    def _theme_colors(self, theme_variant: str) -> dict[str, str]:
        if theme_variant == "light":
            return {
                "window": "#f6f6fb",
                "text": "#282a36",
                "surface": "#e9eaf3",
                "surface_alt": "#dde0ee",
                "surface_press": "#cfd4e6",
                "border": "#aeb7d6",
                "input_bg": "#ffffff",
                "input_text": "#282a36",
                "accent": "#268bd2",
                "active": "#7f5fd1",
                "active_text": "#ffffff",
                "success": "#1f9d55",
                "muted": "#5f6aa8",
                "error": "#d13f4b",
                "warning": "#c37a2c",
            }
        return {
            "window": "#282a36",
            "text": "#f8f8f2",
            "surface": "#44475a",
            "surface_alt": "#535873",
            "surface_press": "#3b3f51",
            "border": "#6272a4",
            "input_bg": "#282a36",
            "input_text": "#f8f8f2",
            "accent": "#8be9fd",
            "active": "#bd93f9",
            "active_text": "#282a36",
            "success": "#50fa7b",
            "muted": "#6272a4",
            "error": "#ff5555",
            "warning": "#ffb86c",
        }

    def _theme_color(self, key: str, fallback: str) -> str:
        value = self.active_theme_colors.get(key, "") if isinstance(self.active_theme_colors, dict) else ""
        if isinstance(value, str) and value.strip():
            return value
        return fallback

    def _theme_stylesheet(self) -> str:
        colors = self.active_theme_colors
        return f"""
            QMainWindow {{
                background-color: {colors['window']};
            }}
            QDialog,
            QMessageBox,
            QInputDialog,
            QFileDialog,
            QColorDialog,
            QFontDialog,
            QDialog QWidget,
            QMessageBox QWidget,
            QInputDialog QWidget,
            QFileDialog QWidget,
            QColorDialog QWidget,
            QFontDialog QWidget {{
                background-color: {colors['surface']};
                color: {colors['text']};
            }}
            QMessageBox QLabel,
            QDialog QLabel,
            QInputDialog QLabel,
            QFileDialog QLabel,
            QColorDialog QLabel,
            QFontDialog QLabel {{
                color: {colors['text']};
            }}
            QMenu {{
                background-color: {colors['surface']};
                color: {colors['text']};
                border: 1px solid {colors['border']};
            }}
            QMenu::item:selected {{
                background-color: {colors['surface_alt']};
                color: {colors['text']};
            }}
            QToolTip {{
                background-color: {colors['surface']};
                color: {colors['text']};
                border: 1px solid {colors['border']};
            }}
            QLabel {{
                color: {colors['text']};
            }}
            QPushButton {{
                background-color: {colors['surface']};
                color: {colors['text']};
                border: 1px solid {colors['border']};
                border-radius: 8px;
                padding: 8px 14px;
                font-weight: 600;
            }}
            QPushButton:hover {{
                border-color: {colors['accent']};
                background-color: {colors['surface_alt']};
            }}
            QPushButton:pressed {{
                background-color: {colors['surface_press']};
                border-color: {colors['accent']};
            }}
            QPushButton:checked {{
                background-color: {colors['active']};
                border-color: {colors['active']};
                color: {colors['active_text']};
            }}
            QWidget#downloadStatusWidget {{
                background-color: {colors['surface']};
                border: 1px solid {colors['border']};
                border-radius: 8px;
            }}
            QProgressBar {{
                border: 1px solid {colors['border']};
                border-radius: 6px;
                background-color: {colors['window']};
                color: {colors['text']};
                text-align: center;
            }}
            QProgressBar::chunk {{
                background-color: {colors['success']};
                border-radius: 5px;
            }}
            QPushButton#gameCard {{
                text-align: left;
                background-color: {colors['surface']};
                border: 1px solid {colors['border']};
                border-radius: 10px;
                padding: 0;
            }}
            QPushButton#gameCard:hover {{
                border-color: {colors['accent']};
            }}
            QListWidget {{
                background-color: {colors['surface']};
                color: {colors['text']};
                border: 1px solid {colors['border']};
                border-radius: 8px;
                padding: 4px;
            }}
            QListWidget::item:selected {{
                background-color: {colors['border']};
                color: {colors['text']};
                border-radius: 5px;
            }}
            QLineEdit, QComboBox {{
                background-color: {colors['input_bg']};
                color: {colors['input_text']};
                border: 1px solid {colors['border']};
                border-radius: 6px;
                padding: 6px 8px;
            }}
            QLineEdit:focus, QComboBox:focus {{
                border: 1px solid {colors['accent']};
            }}
            QFrame#serverSearchContainer {{
                background-color: {colors['input_bg']};
                border: 1px solid {colors['border']};
                border-radius: 6px;
            }}
            QLineEdit#serverSearchInput {{
                background-color: transparent;
                border: none;
                padding: 6px 2px 6px 8px;
            }}
            QPushButton#serverSearchClearButton {{
                background-color: transparent;
                border: none;
                color: {colors['text']};
                font-weight: 700;
                border-radius: 0;
                padding: 0;
            }}
            QPushButton#serverSearchClearButton:hover {{
                border: none;
                color: {colors['text']};
                background-color: {colors['surface']};
            }}
            QPushButton#serverSearchClearButton:pressed {{
                border: none;
                color: {colors['text']};
                background-color: {colors['surface_press']};
            }}
            QFrame#panel {{
                background-color: {colors['surface']};
                border: 1px solid {colors['border']};
                border-radius: 10px;
            }}
            QComboBox QAbstractItemView {{
                background-color: {colors['surface']};
                color: {colors['text']};
                border: 1px solid {colors['border']};
                selection-background-color: {colors['border']};
                selection-color: {colors['text']};
            }}
            QScrollArea#libraryScroll,
            QScrollArea#serverGamesScroll,
            QScrollArea#downloadsScroll {{
                background-color: transparent;
                border: none;
            }}
            QWidget#libraryScrollViewport,
            QWidget#serverGamesScrollViewport,
            QWidget#downloadsScrollViewport,
            QWidget#libraryGridContent,
            QWidget#serverGamesContent,
            QWidget#downloadsContent {{
                background-color: transparent;
            }}
            QListWidget#serverPlatformsList {{
                background-color: transparent;
            }}
            QListWidget#serverPlatformsList::item:hover {{
                background-color: {colors['surface_alt']};
                border-radius: 5px;
            }}
        """

    def _apply_theme_inline_styles(self) -> None:
        text = self._theme_color("text", "#f8f8f2")
        muted = self._theme_color("muted", "#6272a4")
        accent = self._theme_color("accent", "#8be9fd")
        window = self._theme_color("window", "#282a36")
        border = self._theme_color("border", "#6272a4")

        if self.download_count_label is not None:
            self.download_count_label.setStyleSheet(f"font-weight: 600; color: {text};")
        if self.download_speed_label is not None:
            self.download_speed_label.setStyleSheet(f"font-weight: 600; color: {accent};")
        if self.account_status_label is not None:
            self.account_status_label.setStyleSheet(f"font-weight: 600; color: {text};")
        if self.library_empty_label is not None:
            self.library_empty_label.setStyleSheet(f"color: {muted}; font-size: 16px; font-weight: 600;")
        if self.downloads_empty_label is not None:
            self.downloads_empty_label.setStyleSheet(f"color: {muted}; font-size: 16px; font-weight: 600;")
        if self.details_cover_label is not None:
            self.details_cover_label.setStyleSheet(
                f"background-color: {window}; border: 1px dashed {border}; border-radius: 8px; font-size: 20px;"
            )
        for screenshot_label in self.details_screenshot_labels:
            screenshot_label.setStyleSheet(
                f"background-color: {window}; border: 1px dashed {border}; border-radius: 8px;"
            )

    def _apply_theme(self, theme_choice: str) -> None:
        normalized = self._normalized_theme_choice(theme_choice)
        self.active_theme_choice = normalized
        self.active_theme_variant = self._resolved_theme_variant(normalized)
        self.active_theme_colors = self._theme_colors(self.active_theme_variant)
        self.setStyleSheet(self._theme_stylesheet())
        self._apply_theme_inline_styles()

    def _on_theme_selection_changed(self, selected_theme: str) -> None:
        normalized = self._normalized_theme_choice(selected_theme)
        self.config["theme"] = normalized
        self._apply_theme(normalized)
        self._refresh_library_grid()
        self._refresh_downloads_page()
        saved = self._save_config(self.config)
        if saved and self.settings_status_label is not None:
            self.settings_status_label.setText("Theme saved")

    def _config_defaults(self) -> dict[str, Any]:
        return {
            "server_url": "",
            "api_token": "",
            "username": "",
            "library_path": "",
            "launch_args": "",
            "theme": "system",
            "window_geometry": "",
            "window_state": "normal",
            "emulators": [],
            "default_emulators": {},
            "default_retroarch_cores": {},
            "installed_games": [],
        }

    def _persist_window_geometry(self) -> None:
        try:
            geometry_payload = base64.b64encode(bytes(self.saveGeometry())).decode("ascii")
        except (RuntimeError, ValueError):
            return

        self.config["window_geometry"] = geometry_payload
        self.config["window_state"] = "maximized" if self.isMaximized() else "normal"
        self._save_config(self.config)

    def _restore_window_geometry(self) -> None:
        geometry_value = self.config.get("window_geometry", "")
        if isinstance(geometry_value, str) and geometry_value.strip():
            try:
                geometry_bytes = base64.b64decode(geometry_value.encode("ascii"), validate=True)
            except (ValueError, UnicodeEncodeError):
                geometry_bytes = b""
            if geometry_bytes:
                self.restoreGeometry(geometry_bytes)

        window_state = self.config.get("window_state", "normal")
        if isinstance(window_state, str) and window_state.strip().casefold() == "maximized":
            self.setWindowState(self.windowState() | Qt.WindowState.WindowMaximized)

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
            values["theme"] = self._normalized_theme_choice(self.theme_input.currentText())
        values["emulators"] = self.config.get("emulators", [])
        values["default_emulators"] = self.config.get("default_emulators", {})
        values["default_retroarch_cores"] = self.config.get("default_retroarch_cores", {})
        values["window_geometry"] = self.config.get("window_geometry", "")
        values["window_state"] = self.config.get("window_state", "normal")
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
        self._set_server_status("Not connected", self._theme_color("error", "#ff5555"))
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
        self._set_server_status("Disconnected", self._theme_color("error", "#ff5555"))
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

        for key in (
            "url_cover",
            "path_cover_large",
            "path_cover_small",
            "cover_url",
            "cover_image",
            "cover_path",
            "image_url",
        ):
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

    def _set_server_status(self, text: str, color: str | None = None) -> None:
        if self.server_status_label is None:
            return
        self.server_status_label.setText(text)
        if color is None:
            color = self._theme_color("muted", "#6272a4")
        self.server_status_label.setStyleSheet(f"color: {color};")

    def _connect_to_server(self, checked: bool = False, show_errors: bool = True) -> None:
        del checked
        if not self._credentials_present():
            self._clear_server_connection_data()
            self._set_server_status("Missing server URL or API token", self._theme_color("error", "#ff5555"))
            self._update_top_bar_identity()
            return

        self._set_server_status("Connecting...", self._theme_color("accent", "#8be9fd"))
        last_error: Exception | None = None
        try:
            me = self._api_get("/api/users/me")
            platforms = self._api_get("/api/platforms")
            self.server_connected = True
            self._apply_connected_user(me)
            self._populate_server_platforms(platforms)
            self._set_server_status("Connected", self._theme_color("success", "#50fa7b"))
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
        self._set_server_status(error_text, self._theme_color("error", "#ff5555"))

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

    def _on_server_search_changed(self, search_text: str) -> None:
        if self.server_search_clear_button is not None:
            self.server_search_clear_button.setVisible(bool(search_text.strip()))
        if self.server_platforms_list is None:
            return
        selected_item = self.server_platforms_list.currentItem()
        if selected_item is None:
            return
        self._render_server_games(selected_item.text())

    def _clear_server_search(self) -> None:
        if self.server_search_input is None:
            return
        self.server_search_input.clear()

    def _load_server_games(self, platform_label: str) -> None:
        if not self.server_connected:
            return

        platform_id = self.server_platform_ids.get(platform_label)
        if platform_id is None:
            return

        page_size = 200
        all_items: list[dict[str, Any]] = []
        offset = 0
        try:
            while True:
                payload = self._api_get(
                    "/api/roms",
                    {
                        "platform_ids": [platform_id],
                        "limit": page_size,
                        "offset": offset,
                        "with_char_index": "false",
                        "with_filter_values": "false",
                    },
                )
                if not isinstance(payload, dict):
                    break

                page_items = payload.get("items")
                if not isinstance(page_items, list) or not page_items:
                    break

                for item in page_items:
                    if isinstance(item, dict):
                        all_items.append(item)

                total = payload.get("total")
                if isinstance(total, int) and len(all_items) >= total:
                    break

                if len(page_items) < page_size:
                    break

                offset += page_size
        except (HTTPError, URLError, ValueError, json.JSONDecodeError):
            self.server_games_by_platform[platform_label] = []
            self._set_server_status("Connected, but failed to load games", self._theme_color("warning", "#ffb86c"))
            return

        games: list[dict[str, str]] = []
        for item in all_items:
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
            f"background-color: {self._theme_color('window', '#282a36')}; "
            f"border: 1px dashed {self._theme_color('border', '#6272a4')}; border-radius: 6px;"
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
        platform_label.setStyleSheet(f"color: {self._theme_color('muted', '#6272a4')};")
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
        query = ""
        if self.server_search_input is not None:
            query = self.server_search_input.text().strip().casefold()
        if query:
            filtered_games: list[dict[str, str]] = []
            for game in games:
                title = game.get("title", "").casefold()
                game_platform = game.get("platform", "").casefold()
                if query in title or query in game_platform:
                    filtered_games.append(game)
            games = filtered_games
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

    def _rom_id_key(self, game: dict[str, str]) -> str:
        rom_id_value = game.get("rom_id", "")
        if not isinstance(rom_id_value, str):
            return ""
        return rom_id_value.strip().casefold()

    def _games_match_identity(self, left: dict[str, str], right: dict[str, str]) -> bool:
        left_rom_id = self._rom_id_key(left)
        right_rom_id = self._rom_id_key(right)
        if left_rom_id and right_rom_id:
            return left_rom_id == right_rom_id
        return self._game_key(left) == self._game_key(right)

    def _is_game_installed(self, game: dict[str, str]) -> bool:
        return any(self._games_match_identity(installed, game) for installed in self.library_games)

    def _installed_game_record(self, game: dict[str, str]) -> dict[str, str] | None:
        for installed in self.library_games:
            if self._games_match_identity(installed, game):
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
        rom_id = game.get("rom_id", "").strip()
        self.library_games.append(
            {
                "title": game.get("title", "").strip(),
                "platform": game.get("platform", "").strip(),
                "rating": game.get("rating", "N/A").strip() or "N/A",
                "description": game.get("description", "No description available.").strip() or "No description available.",
                "cover_url": game.get("cover_url", "").strip(),
                "screenshot_urls": game.get("screenshot_urls", "").strip(),
                "rom_id": rom_id,
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

    def _is_ps3_platform(self, game: dict[str, str]) -> bool:
        platform_value = game.get("platform", "")
        platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
        return platform in {"playstation 3", "ps3"}

    def _should_extract_archive_for_game(self, game: dict[str, str], archive_path: Path) -> bool:
        if self._is_native_executable_platform(game):
            return True
        if self._is_arcade_platform(game):
            return False
        if self._is_ps3_platform(game):
            return archive_path.suffix.casefold() in {".zip", ".7z", ".rar", ".tar", ".gz", ".bz2", ".xz"}
        return archive_path.suffix.lower() in {".7z", ".zip"}

    def _extracted_dir_for_archive_path(self, archive_path: Path) -> Path:
        extracted_name = archive_path.stem or archive_path.name
        extracted_dir = archive_path.parent / extracted_name
        if extracted_dir == archive_path or (extracted_dir.exists() and extracted_dir.is_file()):
            return archive_path.parent / f"{extracted_name}_extracted"
        return extracted_dir

    def _select_extracted_launch_file(self, game: dict[str, str], extracted_dir: Path, archive_path: Path) -> Path | None:
        files = [candidate for candidate in extracted_dir.rglob("*") if candidate.is_file()]
        if not files:
            return None

        archive_suffixes = {".zip", ".7z", ".rar", ".tar", ".gz", ".bz2", ".xz"}
        non_archive_files = [candidate for candidate in files if candidate.suffix.lower() not in archive_suffixes]
        pool = non_archive_files if non_archive_files else files

        preferred_extensions = [
            ".m3u",
            ".cue",
            ".chd",
            ".iso",
            ".bin",
            ".pbp",
            ".cso",
            ".img",
            ".ccd",
            ".nrg",
            ".mdf",
            ".gdi",
            ".rvz",
            ".gcz",
            ".wbfs",
            ".gcm",
            ".dol",
            ".elf",
            ".nes",
            ".fds",
            ".sfc",
            ".smc",
            ".gba",
            ".gb",
            ".gbc",
            ".n64",
            ".z64",
            ".v64",
            ".nds",
            ".3ds",
            ".cia",
            ".xci",
            ".nsp",
            ".gen",
            ".smd",
            ".md",
            ".32x",
            ".sms",
            ".gg",
            ".pce",
            ".sgx",
            ".a26",
            ".a52",
            ".a78",
            ".lnx",
            ".ws",
            ".wsc",
            ".ngp",
            ".ngc",
            ".jag",
            ".rom",
        ]
        if self._is_ps3_platform(game):
            preferred_extensions = [".pkg", *preferred_extensions]
        extension_priority = {extension: index for index, extension in enumerate(preferred_extensions)}
        support_extensions = {
            ".txt",
            ".nfo",
            ".diz",
            ".log",
            ".json",
            ".xml",
            ".ini",
            ".cfg",
            ".conf",
            ".url",
            ".pdf",
            ".html",
            ".htm",
            ".png",
            ".jpg",
            ".jpeg",
            ".gif",
            ".bmp",
            ".webp",
            ".svg",
            ".ico",
            ".dll",
            ".so",
            ".dylib",
            ".py",
            ".lua",
            ".js",
            ".css",
            ".db",
            ".sqlite",
            ".tmp",
            ".cache",
            ".sav",
            ".srm",
            ".state",
            ".states",
            ".cht",
            ".slangp",
            ".slang",
            ".glsl",
            ".vert",
            ".frag",
        }
        support_directories = {
            "__macosx",
            "glcache",
            "cache",
            "caches",
            "shadercache",
            "shaders",
            "docs",
            "doc",
            "manual",
            "manuals",
            "readme",
            "licenses",
            "license",
            "resources",
        }

        archive_stem = archive_path.stem.casefold()

        def _candidate_sort_key(candidate: Path) -> tuple[int, int, int, int, str]:
            try:
                relative_parts = [part.casefold() for part in candidate.relative_to(extracted_dir).parts]
            except ValueError:
                relative_parts = [part.casefold() for part in candidate.parts]

            suffix = candidate.suffix.casefold()
            support_dir_penalty = 1 if any(part in support_directories for part in relative_parts[:-1]) else 0
            support_ext_penalty = 1 if suffix in support_extensions else 0
            extension_rank = extension_priority.get(suffix, len(extension_priority) + 10)
            stem = candidate.stem.casefold()
            stem_rank = 0 if stem == archive_stem else 1
            return (
                support_dir_penalty + support_ext_penalty,
                extension_rank,
                stem_rank,
                len(relative_parts),
                str(candidate).casefold(),
            )

        playable_candidates = [candidate for candidate in pool if candidate.suffix.casefold() in extension_priority]
        if playable_candidates:
            playable_candidates.sort(key=_candidate_sort_key)
            return playable_candidates[0]

        non_support_candidates = [candidate for candidate in pool if _candidate_sort_key(candidate)[0] == 0]
        selection_pool = non_support_candidates if non_support_candidates else pool

        stem_matches = [candidate for candidate in selection_pool if candidate.stem.casefold() == archive_stem]
        if stem_matches:
            stem_matches.sort(key=_candidate_sort_key)
            return stem_matches[0]

        selection_pool.sort(key=_candidate_sort_key)
        return selection_pool[0]

    def _extract_archive_for_game(self, game: dict[str, str], archive_path: Path) -> tuple[Path, Path]:
        extracted_dir = self._extracted_dir_for_archive_path(archive_path)
        if extracted_dir.exists():
            if extracted_dir.is_dir():
                shutil.rmtree(extracted_dir, ignore_errors=True)
            else:
                try:
                    extracted_dir.unlink()
                except OSError:
                    pass
        extracted_dir.mkdir(parents=True, exist_ok=True)

        try:
            if zipfile.is_zipfile(archive_path):
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

        launch_file = self._select_extracted_launch_file(game, extracted_dir, archive_path)
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

    def _is_native_executable_platform(self, game: dict[str, str]) -> bool:
        platform_value = game.get("platform", "")
        if not isinstance(platform_value, str):
            return False
        platform = platform_value.strip().casefold()
        return platform in {"windows", "windows 9x"}

    def _launchable_native_game_file(self, path: Path) -> bool:
        launchable_suffixes = {".exe", ".bat", ".cmd", ".ps1", ".sh"}
        return path.suffix.casefold() in launchable_suffixes

    def _native_install_dir_for_game(self, game: dict[str, str]) -> Path | None:
        extracted_dir_value = game.get("extracted_dir", "")
        if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
            extracted_dir = Path(extracted_dir_value).expanduser()
            if extracted_dir.exists() and extracted_dir.is_dir():
                return extracted_dir

        extracted_path_value = game.get("extracted_path", "")
        if isinstance(extracted_path_value, str) and extracted_path_value.strip():
            extracted_path = Path(extracted_path_value).expanduser()
            if extracted_path.exists() and extracted_path.is_file():
                return extracted_path.parent

        for archive_path in self._candidate_archive_paths_for_game(game):
            if archive_path.exists() and archive_path.is_file():
                return archive_path.parent
        return None

    def _native_executable_candidates_for_game(self, game: dict[str, str]) -> list[Path]:
        install_dir = self._native_install_dir_for_game(game)
        if install_dir is None:
            return []

        candidates = [
            candidate
            for candidate in install_dir.rglob("*")
            if candidate.is_file() and self._launchable_native_game_file(candidate)
        ]
        candidates.sort(key=lambda candidate: (len(candidate.parts), str(candidate).casefold()))
        return candidates

    def _resolved_native_executable_path_for_game(self, game: dict[str, str]) -> Path | None:
        selected_value = game.get("native_executable_path", "")
        selected_path_text = selected_value.strip() if isinstance(selected_value, str) else ""
        if selected_path_text:
            selected_path = Path(selected_path_text).expanduser()
            if selected_path.exists() and selected_path.is_file() and self._launchable_native_game_file(selected_path):
                return selected_path

        executable_candidates = self._native_executable_candidates_for_game(game)
        if executable_candidates:
            return executable_candidates[0]
        return None

    def _default_assignable_server_platforms(self) -> list[str]:
        hidden_platforms = {"windows", "windows 9x", "emulators"}
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
                    preferred_executable_names: set[str] = set()
                    if "nintendo switch" in title or "switch" in title:
                        preferred_executable_names.add("eden.exe")
                    if "nintendo 3ds" in title or "3ds" in title:
                        preferred_executable_names.add("azahar.exe")

                    def score(candidate: Path) -> tuple[int, int, int, str]:
                        candidate_file_name = candidate.name.casefold()
                        preferred_name = 0 if candidate_file_name in preferred_executable_names else 1
                        candidate_name = candidate.stem.casefold()
                        token_hits = sum(1 for token in title_tokens if token in candidate_name)
                        preferred_binary = 0 if candidate.suffix.casefold() == ".exe" else 1
                        return (preferred_name, -token_hits, preferred_binary, len(candidate.parts), str(candidate).casefold())

                    candidates.sort(key=score)
                    return str(candidates[0])

        if archive_path.exists() and archive_path.is_file() and self._launchable_emulator_file(archive_path):
            return str(archive_path)

        return ""

    def _matching_platforms_for_emulator_keywords(self, keywords: list[str]) -> list[str]:
        if not keywords:
            return []

        def token_set(value: str) -> set[str]:
            return set(re.findall(r"[a-z0-9]+", value.casefold()))

        matches: list[str] = []
        for platform in self._default_assignable_server_platforms():
            platform_tokens = token_set(platform)
            if not platform_tokens:
                continue

            for keyword in keywords:
                if not isinstance(keyword, str):
                    continue
                keyword_tokens = token_set(keyword.strip())
                if not keyword_tokens:
                    continue

                if not keyword_tokens.issubset(platform_tokens):
                    continue

                extra_tokens = platform_tokens - keyword_tokens
                keyword_has_numeric_token = any(token.isdigit() for token in keyword_tokens)
                extra_has_numeric_token = any(token.isdigit() for token in extra_tokens)
                if extra_has_numeric_token and not keyword_has_numeric_token:
                    continue

                if keyword_tokens.issubset(platform_tokens):
                    if platform not in matches:
                        matches.append(platform)
                    break
        return matches

    def _emulator_profile_for_game(self, game: dict[str, str], executable_path: str) -> dict[str, Any]:
        title_value = game.get("title", "")
        title = title_value.strip() if isinstance(title_value, str) else ""
        executable_name = Path(executable_path).name.strip().casefold()

        profiles = self._emulator_autoprofiles()

        for profile in profiles:
            if executable_name and any(token == executable_name for token in profile["match_tokens"]):
                resolved_name = profile["name"]
                if profile.get("use_game_title_as_name", False):
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

        executable_path = self._select_emulator_executable_path(game, archive_path)
        if not executable_path:
            return False
        profile = self._emulator_profile_for_game(game, executable_path)
        emulator_name = profile["name"]

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
                cores = self._installed_retroarch_cores_for_platform(platform, emulator_name)
                if cores:
                    core_defaults[platform] = cores[0]
        self.config["default_retroarch_cores"] = core_defaults

        self._refresh_emulator_views()
        self._save_config(self.config)

    def _archive_name_for_game(self, game: dict[str, str]) -> str:
        rom_file_name_value = game.get("rom_file_name", "")
        if isinstance(rom_file_name_value, str):
            rom_file_name = rom_file_name_value.strip().replace("\\", "/").split("/")[-1]
            if rom_file_name:
                return rom_file_name

        title_value = game.get("title", "Game")
        platform_value = game.get("platform", "Platform")
        title = title_value if isinstance(title_value, str) else "Game"
        platform = platform_value if isinstance(platform_value, str) else "Platform"
        safe_title = self._sanitize_path_component(title, "game")
        safe_platform = self._sanitize_path_component(platform, "platform")
        return f"{safe_title}-{safe_platform}.zip"

    def _server_content_file_name_for_game(self, game: dict[str, str]) -> str:
        rom_file_name_value = game.get("rom_file_name", "")
        if isinstance(rom_file_name_value, str):
            rom_file_name = rom_file_name_value.strip().replace("\\", "/").lstrip("/")
            if rom_file_name:
                return rom_file_name
        return ""

    def _rom_file_name_from_payload(self, payload: dict[str, Any]) -> str:
        candidate_fields = (
            "fs_name",
            "file_name",
            "filename",
            "rom_file_name",
            "download_path",
            "file_path",
            "full_path",
            "path",
            "url",
        )
        candidates: list[str] = []
        for field in candidate_fields:
            value = payload.get(field, "")
            if not isinstance(value, str):
                continue
            candidate = value.strip().replace("\\", "/")
            if field == "url":
                candidate = urlsplit(candidate).path
            candidate = candidate.strip().lstrip("/")
            if candidate:
                candidates.append(candidate)

        if not candidates:
            return ""

        with_suffix = [candidate for candidate in candidates if Path(candidate.split("/")[-1]).suffix]
        return with_suffix[0] if with_suffix else candidates[0]

    def _fetch_server_rom_payload(self, rom_id: str, force_refresh: bool = False) -> dict[str, Any] | None:
        rom_id_key = rom_id.strip()
        if not rom_id_key:
            return None

        cached_payload = self.server_rom_payloads.get(rom_id_key)
        if not force_refresh and isinstance(cached_payload, dict):
            return cached_payload

        rom_id_path = quote(rom_id_key, safe="")
        try:
            payload = self._api_get(f"/api/roms/{rom_id_path}")
        except (HTTPError, URLError, ValueError, json.JSONDecodeError):
            return None

        detail_payload: dict[str, Any] | None = None
        if isinstance(payload, dict):
            if any(key in payload for key in ("fs_name", "file_name", "filename", "rom_file_name")):
                detail_payload = payload
            else:
                for nested_key in ("item", "rom", "data"):
                    nested = payload.get(nested_key)
                    if isinstance(nested, dict):
                        detail_payload = nested
                        break

        if detail_payload is None:
            return None

        self.server_rom_payloads[rom_id_key] = detail_payload
        return detail_payload

    def _resolved_rom_file_name_for_game(self, game: dict[str, str], rom_id: str) -> str:
        current_value = game.get("rom_file_name", "")
        current_name = current_value.strip().replace("\\", "/").lstrip("/") if isinstance(current_value, str) else ""
        if current_name and Path(current_name.split("/")[-1]).suffix:
            return current_name

        fallback_name = current_name if current_name else ""

        payload = self.server_rom_payloads.get(rom_id)
        if not isinstance(payload, dict):
            payload = self._fetch_server_rom_payload(rom_id)
        if isinstance(payload, dict):
            payload_name = self._rom_file_name_from_payload(payload)
            if payload_name and Path(payload_name.split("/")[-1]).suffix:
                return payload_name
            if payload_name and not fallback_name:
                fallback_name = payload_name

        payload = self._fetch_server_rom_payload(rom_id, force_refresh=True)
        if isinstance(payload, dict):
            payload_name = self._rom_file_name_from_payload(payload)
            if payload_name and Path(payload_name.split("/")[-1]).suffix:
                return payload_name
            if payload_name and not fallback_name:
                fallback_name = payload_name

        return fallback_name

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

        resolved_file_name = self._resolved_rom_file_name_for_game(game, rom_id)
        if not resolved_file_name:
            QMessageBox.warning(
                self,
                "Install Error",
                "Server did not return a usable ROM filename/path for this title. Refresh server metadata and try again.",
            )
            return None
        game["rom_file_name"] = resolved_file_name

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
        file_name_path = quote(self._server_content_file_name_for_game(game), safe="")

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
        extracted_dir_value = game.get("extracted_dir", "")
        if isinstance(extracted_dir_value, str) and extracted_dir_value.strip():
            extracted_dir = Path(extracted_dir_value).expanduser()
            if extracted_dir.exists() and extracted_dir.is_dir():
                selected = self._select_extracted_launch_file(game, extracted_dir, Path(game.get("archive_path", "") or "archive"))
                if selected is not None:
                    candidates.append(selected)

        extracted_path_value = game.get("extracted_path", "")
        if isinstance(extracted_path_value, str) and extracted_path_value.strip():
            extracted_path = Path(extracted_path_value).expanduser()
            if extracted_path.exists() and extracted_path.is_file():
                candidates.append(extracted_path)

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
        if self._is_native_executable_platform(game):
            for extracted_dir in self._candidate_extracted_dirs_for_game(game):
                if not extracted_dir.exists() or not extracted_dir.is_dir():
                    continue
                try:
                    shutil.rmtree(extracted_dir)
                except OSError as error:
                    QMessageBox.warning(self, "Uninstall Error", f"Could not remove folder: {extracted_dir}\n{error}")
                    return False
            return True

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

    def _path_key(self, path: Path) -> str:
        expanded = path.expanduser()
        try:
            return str(expanded.resolve(strict=False)).casefold()
        except OSError:
            return str(expanded).casefold()

    def _path_within_path(self, path: Path, root: Path) -> bool:
        path_key = self._path_key(path)
        root_key = self._path_key(root).rstrip("\\/")
        if not root_key:
            return False
        return path_key == root_key or path_key.startswith(f"{root_key}\\") or path_key.startswith(f"{root_key}/")

    def _matching_installed_emulator_games(self, emulator_path: Path) -> list[dict[str, str]]:
        matches: list[dict[str, str]] = []
        target_key = self._path_key(emulator_path)
        for game in self.library_games:
            if not self._is_emulators_platform(game):
                continue

            file_candidates = self._candidate_archive_paths_for_game(game) + self._candidate_extracted_paths_for_game(game)
            if any(self._path_key(candidate) == target_key for candidate in file_candidates):
                matches.append(game)
                continue

            dir_candidates = self._candidate_extracted_dirs_for_game(game)
            if any(self._path_within_path(emulator_path, candidate) for candidate in dir_candidates):
                matches.append(game)
        return matches

    def _uninstall_emulator_files(self, emulator: dict[str, str]) -> bool:
        emulator_path_value = emulator.get("path", "")
        emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
        if not emulator_path_text:
            return True

        emulator_path = Path(emulator_path_text)
        matches = self._matching_installed_emulator_games(emulator_path)
        if not matches:
            return True

        keys_to_remove = {self._game_key(game) for game in matches}
        for game in matches:
            if not self._remove_game_files(game):
                return False

        before = len(self.library_games)
        self.library_games = [entry for entry in self.library_games if self._game_key(entry) not in keys_to_remove]
        if len(self.library_games) != before:
            self._refresh_library_grid()
            self._persist_installed_games()
        return True

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
        self._update_details_action_buttons()

        self.stack.setCurrentIndex(5)
        self._update_details_layout_metrics()
        QTimer.singleShot(0, self._update_details_layout_metrics)
        for button in self.nav_buttons:
            button.setChecked(False)

    def _update_details_action_buttons(self) -> None:
        if self.current_details_game is None:
            return
        is_emulator_entry = self._is_emulators_platform(self.current_details_game)
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
            show_primary = not (is_emulator_entry and installed)
            self.details_primary_button.setText(button_text)
            self.details_primary_button.setVisible(show_primary)
            self.details_primary_button.setEnabled(show_primary and not installing_current and not queued_current)
        if self.details_config_button is not None:
            show_config = installed and self._is_native_executable_platform(self.current_details_game)
            self.details_config_button.setVisible(show_config)
            self.details_config_button.setEnabled(show_config and not installing_current)
        if self.details_secondary_button is not None:
            self.details_secondary_button.setVisible(installed and not is_emulator_entry)
            self.details_secondary_button.setEnabled(installed and not is_emulator_entry and not installing_current)

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
        direct = str(game.get("rom_id", "")).strip()
        if direct:
            return direct

        cache_key = self._details_rom_id_cache_key(game)
        cache = self._details_rom_id_cache()
        cached = str(cache.get(cache_key, "")).strip()
        if cached:
            return cached

        target = self._game_key(game)
        for games in self.server_games_by_platform.values():
            for server_game in games:
                if self._game_key(server_game) != target:
                    continue
                server_rom_id = str(server_game.get("rom_id", "")).strip()
                if server_rom_id:
                    return server_rom_id
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
        resolved_file_name = self._resolved_rom_file_name_for_game(install_game, rom_id)
        if not resolved_file_name:
            QMessageBox.warning(
                self,
                "Install Error",
                "Server did not return a usable ROM filename/path for this title. Refresh server metadata and try again.",
            )
            return False
        install_game["rom_file_name"] = resolved_file_name
        if self.current_details_game is not None and self._game_key(self.current_details_game) == self._game_key(install_game):
            self.current_details_game["rom_id"] = rom_id
            self.current_details_game["rom_file_name"] = resolved_file_name

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
        file_name_path = quote(self._server_content_file_name_for_game(install_game), safe="")
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
        detail_label.setStyleSheet(f"color: {self._theme_color('muted', '#6272a4')};")
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
                    core_value = self._retroarch_core_argument_path(configured_core)

        return {
            "%rom%": rom_path,
            "%core%": core_value,
        }

    def _retroarch_core_argument_path(self, configured_core: str) -> str:
        core = configured_core.strip()
        if not core:
            return ""

        normalized = core.replace("\\", "/")
        if "/" in normalized:
            return normalized

        if normalized.casefold().endswith(".dll"):
            core_file = normalized
        elif normalized.casefold().endswith("_libretro"):
            core_file = f"{normalized}.dll"
        else:
            core_file = f"{normalized}_libretro.dll"
        return f"cores/{core_file}"

    def _strip_wrapping_quotes(self, token: str) -> str:
        stripped = token.strip()
        if len(stripped) >= 2 and stripped[0] == stripped[-1] and stripped[0] in {'"', "'"}:
            return stripped[1:-1]
        return stripped

    def _apply_launch_placeholders_to_args(self, args: list[str], placeholders: dict[str, str]) -> list[str]:
        resolved_args: list[str] = []
        core_value = placeholders.get("%core%", "")
        core_missing = not core_value.strip()
        for arg in args:
            had_core_placeholder = "%core%" in arg
            resolved = arg
            for token, value in placeholders.items():
                resolved = resolved.replace(token, value)
            resolved = self._strip_wrapping_quotes(resolved)
            if had_core_placeholder and core_missing:
                if resolved_args and resolved_args[-1] in {"-L", "--libretro", "--core"}:
                    resolved_args.pop()
                continue
            if resolved:
                resolved_args.append(resolved)
        return resolved_args

    def _split_launch_template_args(self, template: str) -> list[str]:
        if not template.strip():
            return []

        try:
            return shlex.split(template, posix=True)
        except ValueError:
            return shlex.split(template, posix=False)

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

        parsed_template_args = self._split_launch_template_args(combined_template)
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

    def _normalized_retroarch_core_args(self, emulator_dir: Path, args: list[str]) -> list[str]:
        normalized_args = list(args)
        core_option_tokens = {"-L", "--libretro", "--core"}
        for index, token in enumerate(normalized_args[:-1]):
            if token not in core_option_tokens:
                continue

            core_token = normalized_args[index + 1].strip()
            if not core_token:
                continue

            core_path = Path(core_token).expanduser()
            if core_path.is_absolute():
                continue

            candidate = (emulator_dir / core_path).resolve(strict=False)
            if candidate.exists() and candidate.is_file():
                normalized_args[index + 1] = str(candidate)
        return normalized_args

    def _launch_installed_game(self, game: dict[str, str]) -> bool:
        if self._is_native_executable_platform(game):
            native_executable = self._resolved_native_executable_path_for_game(game)
            if native_executable is None:
                QMessageBox.warning(
                    self,
                    "Launch Error",
                    "No launchable native executable is configured for this game. Use Game Settings to select one.",
                )
                return False

            command = [str(native_executable)]
            try:
                process = subprocess.Popen(command, cwd=str(native_executable.parent))
                QTimer.singleShot(500, lambda p=process, c=command: self._warn_if_process_exited_early(p, c))
                return True
            except OSError as error:
                QMessageBox.warning(self, "Launch Error", f"Failed to launch game:\n{error}")
                return False

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

        if self._is_retroarch_emulator_name(emulator_name):
            parsed_args = self._normalized_retroarch_core_args(emulator_path.parent, parsed_args)

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
            f"Process exited immediately (code {exit_code}).\nCommand:\n{command_text}",
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

    def _perform_game_config_action(self) -> None:
        if self.current_details_game is None:
            return
        installed_game = self._installed_game_record(self.current_details_game)
        if installed_game is None:
            return
        if not self._is_native_executable_platform(self.current_details_game):
            return

        install_dir = self._native_install_dir_for_game(installed_game)
        executable_candidates = self._native_executable_candidates_for_game(installed_game)
        if install_dir is None or not executable_candidates:
            QMessageBox.warning(
                self,
                "Game Settings",
                "No launchable executables were found in this game's install directory.",
            )
            return

        dialog = QDialog(self)
        dialog.setWindowTitle(f"Game Settings - {installed_game.get('title', 'Game')}")
        dialog.setModal(True)
        dialog.resize(700, 220)

        dialog_layout = QVBoxLayout(dialog)
        form_layout = QFormLayout()

        install_dir_label = QLabel(str(install_dir))
        install_dir_label.setWordWrap(True)
        form_layout.addRow("Install Directory", install_dir_label)

        executable_combo = QComboBox()
        executable_combo.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        for candidate in executable_candidates:
            try:
                display_name = str(candidate.relative_to(install_dir))
            except ValueError:
                display_name = str(candidate)
            executable_combo.addItem(display_name, str(candidate))

        selected_path = installed_game.get("native_executable_path", "")
        selected_executable_path = selected_path.strip() if isinstance(selected_path, str) else ""
        if selected_executable_path:
            selected_index = executable_combo.findData(selected_executable_path)
            if selected_index >= 0:
                executable_combo.setCurrentIndex(selected_index)
        form_layout.addRow("Executable", executable_combo)

        dialog_layout.addLayout(form_layout)

        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Save | QDialogButtonBox.StandardButton.Cancel)
        buttons.accepted.connect(dialog.accept)
        buttons.rejected.connect(dialog.reject)
        dialog_layout.addWidget(buttons)

        if dialog.exec() != int(QDialog.DialogCode.Accepted):
            return

        selected_value = executable_combo.currentData()
        selected_executable = selected_value.strip() if isinstance(selected_value, str) else ""
        if not selected_executable:
            QMessageBox.warning(self, "Game Settings", "Please select an executable to launch.")
            return

        installed_game["native_executable_path"] = selected_executable
        if self.current_details_game is not None:
            self.current_details_game["native_executable_path"] = selected_executable
        self._persist_installed_games()

        QMessageBox.information(self, "Game Settings", "Saved game executable selection.")

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

    def _emulator_autoprofiles_path(self) -> Path:
        return Path(__file__).resolve().parent / "emulator-autoprofiles.json"

    def _default_emulator_autoprofiles(self) -> list[dict[str, Any]]:
        return [
            {
                "match_tokens": ["retroarch.exe"],
                "name": "RetroArch",
                "args": '-L "%core%" "%rom%"',
                "all_platforms": True,
                "platform_keywords": [],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["duckstation.exe"],
                "name": "DuckStation",
                "args": '-fullscreen -batch "%rom%"',
                "all_platforms": False,
                "platform_keywords": ["playstation", "ps1"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["pcsx2-qt.exe"],
                "name": "PCSX2",
                "args": '-fullscreen -batch "%rom%"',
                "all_platforms": False,
                "platform_keywords": ["playstation 2", "ps2"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["ppssppqt.exe"],
                "name": "PPSSPP",
                "args": '--fullscreen --pause-menu-exit "%rom%"',
                "all_platforms": False,
                "platform_keywords": ["playstation portable", "psp"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["rpcs3.exe"],
                "name": "RPCS3",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["playstation 3", "ps3"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["dolphin.exe"],
                "name": "Dolphin",
                "args": '-b -e "%rom%"',
                "all_platforms": False,
                "platform_keywords": ["gamecube", "wii"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["cemu.exe"],
                "name": "Cemu",
                "args": '-f -g "%rom%"',
                "all_platforms": False,
                "platform_keywords": ["wii u"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["azahar.exe"],
                "name": "Azahar",
                "args": '-f "%rom%"',
                "all_platforms": False,
                "platform_keywords": ["nintendo 3ds", "3ds"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["pico8.exe"],
                "name": "Pico",
                "args": '-run "%rom%"',
                "all_platforms": False,
                "platform_keywords": ["pico-8", "pico 8"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["xemu.exe"],
                "name": "Xemu",
                "args": '-full-screen -dvd_path "%rom%"',
                "all_platforms": False,
                "platform_keywords": ["xbox"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["eden.exe"],
                "name": "Eden",
                "args": '-f -g "%rom%"',
                "all_platforms": False,
                "platform_keywords": ["switch", "nintendo switch"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["yuzu.exe"],
                "name": "Yuzu",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["switch", "nintendo switch"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["ryujinx.exe"],
                "name": "Ryujinx",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["switch", "nintendo switch"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["sudachi.exe"],
                "name": "Sudachi",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["switch", "nintendo switch"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["mame.exe"],
                "name": "MAME",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["arcade", "mame", "final burn", "fbneo"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["fbneo.exe"],
                "name": "FBNeo",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["arcade", "mame", "final burn", "fbneo"],
                "use_game_title_as_name": False,
            },
            {
                "match_tokens": ["finalburnneo.exe"],
                "name": "FinalBurn Neo",
                "args": "%rom%",
                "all_platforms": False,
                "platform_keywords": ["arcade", "mame", "final burn", "fbneo"],
                "use_game_title_as_name": False,
            },
        ]

    def _normalize_emulator_autoprofiles(self, value: Any) -> list[dict[str, Any]]:
        if not isinstance(value, list):
            return []

        normalized: list[dict[str, Any]] = []
        for item in value:
            if not isinstance(item, dict):
                continue

            match_tokens = item.get("match_tokens", [])
            if not isinstance(match_tokens, list):
                continue
            normalized_tokens = [
                token.strip().casefold()
                for token in match_tokens
                if isinstance(token, str) and token.strip()
            ]
            if not normalized_tokens:
                continue
            primary_token = normalized_tokens[0]

            name = item.get("name", "")
            if not isinstance(name, str) or not name.strip():
                continue

            args = item.get("args", "%rom%")
            if not isinstance(args, str):
                args = "%rom%"

            all_platforms = bool(item.get("all_platforms", False))

            platform_keywords = item.get("platform_keywords", [])
            normalized_keywords: list[str] = []
            if isinstance(platform_keywords, list):
                normalized_keywords = [
                    keyword.strip()
                    for keyword in platform_keywords
                    if isinstance(keyword, str) and keyword.strip()
                ]

            use_game_title_as_name = bool(item.get("use_game_title_as_name", False))

            normalized.append(
                {
                    "match_tokens": [primary_token],
                    "name": name.strip(),
                    "args": args.strip() or "%rom%",
                    "all_platforms": all_platforms,
                    "platform_keywords": normalized_keywords,
                    "use_game_title_as_name": use_game_title_as_name,
                }
            )

        return normalized

    def _emulator_autoprofiles(self) -> list[dict[str, Any]]:
        if isinstance(self.emulator_autoprofiles, list):
            return self.emulator_autoprofiles

        defaults = self._normalize_emulator_autoprofiles(self._default_emulator_autoprofiles())
        autoprofiles_path = self._emulator_autoprofiles_path()
        if not autoprofiles_path.exists():
            self.emulator_autoprofiles = defaults
            return defaults

        try:
            parsed = json.loads(autoprofiles_path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            self.emulator_autoprofiles = defaults
            return defaults

        normalized = self._normalize_emulator_autoprofiles(parsed)
        if normalized:
            self.emulator_autoprofiles = normalized
            return normalized

        self.emulator_autoprofiles = defaults
        return defaults

    def _retroarch_core_list_path(self) -> Path:
        return Path(__file__).resolve().parent / "retroarch-core-list.json"

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

    def _retroarch_core_id_from_file_name(self, core_file_name: str) -> str:
        normalized = core_file_name.strip().replace("\\", "/")
        if not normalized:
            return ""

        file_name = normalized.rsplit("/", 1)[-1].casefold()
        if file_name.endswith(".dll"):
            file_name = file_name[:-4]
        if file_name.endswith("_libretro"):
            file_name = file_name[: -len("_libretro")]
        return file_name.strip()

    def _retroarch_compatibility_map_from_markdown(self) -> dict[str, list[str]]:
        if isinstance(self.retroarch_compatibility_map, dict):
            return self.retroarch_compatibility_map

        path = self._retroarch_core_list_path()
        compatibility: dict[str, list[str]] = {}
        if not path.exists():
            self.retroarch_compatibility_map = compatibility
            return compatibility

        try:
            raw_content = path.read_text(encoding="utf-8")
        except OSError:
            self.retroarch_compatibility_map = compatibility
            return compatibility

        try:
            entries = json.loads(raw_content)
        except json.JSONDecodeError:
            entries = None

        if isinstance(entries, list):
            for entry in entries:
                if not isinstance(entry, dict):
                    continue

                core_file = entry.get("core_file", "")
                if not isinstance(core_file, str) or not core_file.strip():
                    continue
                core_id = self._retroarch_core_id_from_file_name(core_file)
                if not core_id:
                    continue

                platforms = entry.get("platforms", [])
                if not isinstance(platforms, list):
                    continue
                for platform in platforms:
                    if not isinstance(platform, str):
                        continue
                    system_key = self._normalize_retroarch_platform_key(platform)
                    if not system_key:
                        continue
                    known = compatibility.setdefault(system_key, [])
                    if core_id not in known:
                        known.append(core_id)

            self.retroarch_compatibility_map = compatibility
            return compatibility

        lines = raw_content.splitlines()

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
            system_key = self._normalize_retroarch_platform_key(system_cell)
            if not core_id or not system_key:
                continue

            known = compatibility.setdefault(system_key, [])
            if core_id not in known:
                known.append(core_id)

        self.retroarch_compatibility_map = compatibility
        return compatibility

    def _normalize_retroarch_platform_key(self, value: str) -> str:
        normalized = value.strip().casefold()
        if not normalized:
            return ""

        normalized = normalized.replace("\\", "/")
        normalized = re.sub(r"[^a-z0-9]+", " ", normalized)
        normalized = re.sub(r"\s+", " ", normalized)
        return normalized.strip()

    def _retroarch_platform_tokens(self, value: str) -> set[str]:
        normalized = re.sub(r"[^a-z0-9]+", " ", value.strip().casefold())
        ignored_tokens = {"the", "and", "of", "for", "system"}
        return {token for token in normalized.split() if token and token not in ignored_tokens}

    def _all_retroarch_cores(self, compatibility: dict[str, list[str]]) -> list[str]:
        cores: list[str] = []
        for mapped_cores in compatibility.values():
            for core in mapped_cores:
                if core not in cores:
                    cores.append(core)
        return cores

    def _retroarch_installed_core_ids_for_emulator(self, emulator_name: str) -> set[str]:
        if not self._is_retroarch_emulator_name(emulator_name):
            return set()

        emulator_entry = self._emulator_entry_by_name(emulator_name)
        if emulator_entry is None:
            return set()

        emulator_path_value = emulator_entry.get("path", "")
        emulator_path_text = emulator_path_value.strip() if isinstance(emulator_path_value, str) else ""
        if not emulator_path_text:
            return set()

        emulator_path = Path(emulator_path_text).expanduser()
        if not emulator_path.exists() or not emulator_path.is_file():
            return set()

        cores_dir = emulator_path.parent / "cores"
        if not cores_dir.exists() or not cores_dir.is_dir():
            return set()

        installed_core_ids: set[str] = set()
        for candidate in cores_dir.glob("*.dll"):
            if not candidate.is_file():
                continue
            core_id = self._retroarch_core_id_from_file_name(candidate.name)
            if core_id:
                installed_core_ids.add(core_id)
        return installed_core_ids

    def _installed_retroarch_cores_for_platform(self, platform: str, emulator_name: str) -> list[str]:
        platform_cores = self._retroarch_cores_for_platform(platform)
        installed_core_ids = self._retroarch_installed_core_ids_for_emulator(emulator_name)
        if not installed_core_ids:
            return []
        return [core for core in platform_cores if core in installed_core_ids]

    def _retroarch_system_keys_for_platform(self, platform: str) -> list[str]:
        compatibility = self._retroarch_compatibility_map_from_markdown()
        normalized = self._normalize_retroarch_platform_key(platform)
        if not normalized or not compatibility:
            return []

        if normalized in compatibility:
            return [normalized]
        return []

    def _retroarch_cores_for_platform(self, platform: str) -> list[str]:
        compatibility = self._retroarch_compatibility_map_from_markdown()
        if not compatibility:
            return ["fbneo", "mame2003_plus"]

        resolved_cores: list[str] = []
        for system_key in self._retroarch_system_keys_for_platform(platform):
            for core in compatibility.get(system_key, []):
                if core not in resolved_cores:
                    resolved_cores.append(core)

        if resolved_cores:
            return resolved_cores
        return []

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
        for core in self._installed_retroarch_cores_for_platform(platform, emulator_name):
            self.default_core_combo.addItem(core, core)

        if not is_retroarch:
            self.default_core_combo.setCurrentIndex(-1)
        else:
            if saved_core:
                saved_index = self.default_core_combo.findData(saved_core)
                if saved_index >= 0:
                    self.default_core_combo.setCurrentIndex(saved_index)
                elif self.default_core_combo.count() > 0:
                    self.default_core_combo.setCurrentIndex(0)
                else:
                    self.default_core_combo.setCurrentIndex(-1)
            else:
                self.default_core_combo.setCurrentIndex(0 if self.default_core_combo.count() > 0 else -1)

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
            rom_id = item.get("rom_id")
            rom_file_name = item.get("rom_file_name")
            extracted_path = item.get("extracted_path")
            extracted_dir = item.get("extracted_dir")
            archive_path = item.get("archive_path")
            native_executable_path = item.get("native_executable_path")
            normalized_game = {
                "title": title.strip(),
                "platform": platform.strip(),
                "rating": rating.strip() if isinstance(rating, str) and rating.strip() else "N/A",
                "description": description.strip() if isinstance(description, str) and description.strip() else "No description available.",
                "cover_url": cover_url.strip() if isinstance(cover_url, str) else "",
                "screenshot_urls": screenshot_urls.strip() if isinstance(screenshot_urls, str) else "",
                "rom_id": rom_id.strip() if isinstance(rom_id, str) else "",
                "rom_file_name": rom_file_name.strip() if isinstance(rom_file_name, str) else "",
                "extracted_path": extracted_path.strip() if isinstance(extracted_path, str) else "",
                "extracted_dir": extracted_dir.strip() if isinstance(extracted_dir, str) else "",
                "archive_path": archive_path.strip() if isinstance(archive_path, str) else "",
                "native_executable_path": native_executable_path.strip() if isinstance(native_executable_path, str) else "",
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

        selected_name = ""
        selected_index = self.emulator_list.currentRow()
        if 0 <= selected_index < len(emulators):
            existing_name = emulators[selected_index].get("name", "")
            if isinstance(existing_name, str):
                selected_name = existing_name.strip()

        self.emulator_list.clear()
        for row, entry in enumerate(emulators):
            item = QListWidgetItem()
            row_widget = QWidget()
            row_layout = QHBoxLayout(row_widget)
            row_layout.setContentsMargins(6, 2, 6, 2)
            row_layout.setSpacing(8)

            name_button = QPushButton(entry["name"])
            name_button.setFlat(True)
            name_button.setStyleSheet("text-align: left; padding: 4px 0;")
            name_button.clicked.connect(lambda checked=False, current_row=row: self.emulator_list.setCurrentRow(current_row))
            row_layout.addWidget(name_button, 1)

            uninstall_button = QPushButton("Uninstall")
            uninstall_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
            uninstall_button.clicked.connect(lambda checked=False, current_row=row: self._remove_emulator_at_index(current_row))
            row_layout.addWidget(uninstall_button)

            item.setSizeHint(row_widget.sizeHint())
            self.emulator_list.addItem(item)
            self.emulator_list.setItemWidget(item, row_widget)

        if selected_name:
            for row, emulator in enumerate(emulators):
                emulator_name = emulator.get("name", "")
                if isinstance(emulator_name, str) and emulator_name.strip().casefold() == selected_name.casefold():
                    self.emulator_list.setCurrentRow(row)
                    break

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

    def _remove_emulator_at_index(self, index: int) -> None:
        emulators = self._emulators()
        if index < 0 or index >= len(emulators):
            return

        emulator_to_remove = emulators[index]
        if not self._uninstall_emulator_files(emulator_to_remove):
            return

        removed_name = emulator_to_remove["name"]
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

    def _remove_emulator(self) -> None:
        if self.emulator_list is None:
            return
        index = self.emulator_list.currentRow()
        self._remove_emulator_at_index(index)

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
