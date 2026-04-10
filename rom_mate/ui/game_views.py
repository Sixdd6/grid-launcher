from __future__ import annotations

from typing import Any, Protocol

from PySide6.QtCore import QTimer, Qt
from PySide6.QtWidgets import QLabel, QPushButton, QVBoxLayout

from rom_mate.library import rom_file_name_version


class GameCardWindowProtocol(Protocol):
    def _open_game_details(self, game: dict[str, str], source: str) -> None:
        ...

    def _queue_game_cover_load(self, game: dict[str, str], label: QLabel) -> None:
        ...

    def _theme_color(self, role: str, fallback: str) -> str:
        ...


class GameDetailsWindowProtocol(Protocol):
    current_details_game: dict[str, str] | None
    current_details_source: str
    current_details_cloud_mode: str
    details_title_label: QLabel | None
    details_cover_label: QLabel | None
    details_platform_label: QLabel | None
    details_version_label: QLabel | None
    details_rating_label: QLabel | None
    details_description_label: QLabel | None
    details_primary_button: QPushButton | None
    details_config_button: QPushButton | None
    details_details_button: QPushButton | None
    details_manage_saves_button: QPushButton | None
    details_manage_states_button: QPushButton | None
    details_ps4_content_button: QPushButton | None
    details_secondary_button: QPushButton | None
    details_update_button: QPushButton | None
    stack: Any
    nav_buttons: list[QPushButton]
    install_in_progress: bool
    install_finalize_in_progress: bool
    install_pending_game: dict[str, str] | None
    install_finalize_game: dict[str, str] | None

    def _queue_game_cover_load(self, game: dict[str, str], label: QLabel) -> None:
        ...

    def _update_details_screenshots(self, game: dict[str, str]) -> None:
        ...

    def _update_details_action_buttons(self) -> None:
        ...

    def _update_details_layout_metrics(self) -> None:
        ...

    def _show_details_overview(self) -> None:
        ...

    def _is_emulators_platform(self, game: dict[str, str]) -> bool:
        ...

    def _is_game_installed(self, game: dict[str, str]) -> bool:
        ...

    def _install_block_reason_for_game(self, game: dict[str, str]) -> str:
        ...

    def _is_game_install_queued(self, game: dict[str, str]) -> bool:
        ...

    def _game_key(self, game: dict[str, str]) -> tuple[str, str]:
        ...

    def _is_native_executable_platform(self, game: dict[str, str]) -> bool:
        ...

    def _details_ps4_content_button_text(self, game: dict[str, str]) -> str:
        ...

    def _ps4_content_install_block_reason(self, game: dict[str, str]) -> str:
        ...

    def _resolved_emulator_entry_for_game(self, game: dict[str, str]) -> tuple[str, dict[str, str] | None]:
        ...

    def _details_cloud_mode_supported(self, game: dict[str, str], save_type: str) -> bool:
        ...

    def _details_cloud_button_text(self, game: dict[str, str], save_type: str) -> str:
        ...

    def _is_rpcs3_emulator_name(self, emulator_name: str) -> bool:
        ...

    def _details_version_label_text_for_game(self, game: dict[str, str]) -> str:
        ...

    def _details_update_button_text_for_game(self, game: dict[str, str]) -> str:
        ...


def _is_windows_pc_platform(platform_value: object) -> bool:
    if not isinstance(platform_value, str):
        return False
    platform = platform_value.strip().casefold()
    if not platform:
        return False
    return "windows" in platform or platform == "pc"


def _default_details_version_label_text(game: dict[str, str]) -> str:
    if not _is_windows_pc_platform(game.get("platform", "")):
        return ""
    rom_file_name = game.get("rom_file_name", "")
    version_tag = rom_file_name_version(rom_file_name)
    if version_tag is None:
        return ""
    return f"Version: v{version_tag:05d}"


def _is_truthy_flag(value: object) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, (int, float)):
        return value != 0
    if isinstance(value, str):
        normalized = value.strip().casefold()
        return normalized in {"1", "true", "yes", "on", "y", "t"}
    return False


def _has_update_available(window: GameDetailsWindowProtocol, game: dict[str, str], installed: bool) -> bool:
    if not installed:
        return False

    details_update_available_fn = getattr(window, "_details_update_available_for_game", None)
    if callable(details_update_available_fn):
        return bool(details_update_available_fn(game))

    for field in ("update_available", "has_update", "ps4_has_update"):
        if _is_truthy_flag(game.get(field)):
            return True

    return False


def is_hidden_library_platform(game: dict[str, str]) -> bool:
    platform_value = game.get("platform", "")
    platform = platform_value.strip().casefold() if isinstance(platform_value, str) else ""
    return platform in {"emulator", "emulators"}


def visible_library_games(library_games: list[dict[str, str]]) -> list[dict[str, str]]:
    visible_games = [game for game in library_games if not is_hidden_library_platform(game)]
    return sorted(
        visible_games,
        key=lambda game: (
            game.get("title", "").strip().casefold(),
            game.get("platform", "").strip().casefold(),
        ),
    )


def make_game_card(window: GameCardWindowProtocol, game: dict[str, str], source: str) -> QPushButton:
    frame = QPushButton()
    frame.setObjectName("gameCard")
    frame.setFixedSize(180, 250)
    frame.clicked.connect(lambda: window._open_game_details(game, source))

    layout = QVBoxLayout(frame)
    layout.setContentsMargins(8, 8, 8, 8)
    layout.setSpacing(8)

    cover = QLabel("Cover Art")
    cover.setObjectName("gameCardCover")
    cover.setAlignment(Qt.AlignmentFlag.AlignCenter)
    cover.setFixedHeight(170)
    cover.setStyleSheet("background-color: transparent;")

    window._queue_game_cover_load(game, cover)
    layout.addWidget(cover)

    title_label = QLabel(game["title"])
    title_label.setWordWrap(True)
    title_label.setStyleSheet("font-weight: 600;")
    layout.addWidget(title_label)

    platform_label = QLabel(game["platform"])
    platform_label.setStyleSheet(f"color: {window._theme_color('muted', '#6272a4')};")
    layout.addWidget(platform_label)

    update_indicator = QLabel("Update Available")
    update_indicator.setObjectName("gameCardUpdateIndicator")
    update_indicator.setStyleSheet(f"color: {window._theme_color('warning', '#ffb86c')}; font-size: 12px; font-weight: 600;")
    update_indicator.setVisible(_is_truthy_flag(game.get("update_available")))
    layout.addWidget(update_indicator)

    return frame


def open_game_details(window: GameDetailsWindowProtocol, game: dict[str, str], source: str) -> None:
    window.current_details_game = game
    window.current_details_source = source
    if window.details_title_label is not None:
        window.details_title_label.setText(game["title"])
    if window.details_cover_label is not None:
        window.details_cover_label.clear()
        window.details_cover_label.setText("Cover Art")
        window._queue_game_cover_load(game, window.details_cover_label)
    if window.details_platform_label is not None:
        window.details_platform_label.setText(f"Platform: {game['platform']}")
    version_label_text_fn = getattr(window, "_details_version_label_text_for_game", None)
    version_label_text = (
        version_label_text_fn(game)
        if callable(version_label_text_fn)
        else _default_details_version_label_text(game)
    )
    if window.details_version_label is not None:
        window.details_version_label.setText(version_label_text)
        window.details_version_label.setVisible(bool(version_label_text))
    if window.details_rating_label is not None:
        window.details_rating_label.setText(f"Rating: {game['rating']}")
    if window.details_description_label is not None:
        window.details_description_label.setText(game["description"])
    window._update_details_screenshots(game)
    window._show_details_overview()
    window._update_details_action_buttons()

    window.stack.setCurrentIndex(5)
    window._update_details_layout_metrics()
    QTimer.singleShot(0, window._update_details_layout_metrics)
    for button in window.nav_buttons:
        button.setChecked(False)


def update_details_action_buttons(window: GameDetailsWindowProtocol) -> None:
    if window.current_details_game is None:
        return

    current_game = window.current_details_game
    is_emulator_entry = window._is_emulators_platform(current_game)
    installed = window._is_game_installed(current_game)
    update_available = _has_update_available(window, current_game, installed)
    install_block_reason = "" if installed else window._install_block_reason_for_game(current_game)
    install_blocked = bool(install_block_reason)
    queued_current = window._is_game_install_queued(current_game)
    installing_current = (
        window.install_in_progress
        and window.install_pending_game is not None
        and window._game_key(current_game) == window._game_key(window.install_pending_game)
    )
    if not installing_current:
        installing_current = (
            window.install_finalize_in_progress
            and window.install_finalize_game is not None
            and window._game_key(current_game) == window._game_key(window.install_finalize_game)
        )

    if window.details_primary_button is not None:
        if installing_current:
            button_text = "Installing..."
        elif queued_current:
            button_text = "Queued..."
        elif installed:
            button_text = "Play"
        else:
            button_text = "Install App" if is_emulator_entry else "Install Game"
        show_primary = not (is_emulator_entry and installed)
        window.details_primary_button.setText(button_text)
        window.details_primary_button.setVisible(show_primary)
        window.details_primary_button.setEnabled(
            show_primary and not installing_current and not queued_current and not install_blocked
        )
        window.details_primary_button.setToolTip(install_block_reason if install_blocked else "")

    if window.details_config_button is not None:
        show_config = installed and window._is_native_executable_platform(current_game)
        window.details_config_button.setVisible(show_config)
        window.details_config_button.setEnabled(show_config and not installing_current)

    details_ps4_content_button = getattr(window, "details_ps4_content_button", None)
    if details_ps4_content_button is not None:
        ps4_button_text_fn = getattr(window, "_details_ps4_content_button_text", None)
        ps4_block_reason_fn = getattr(window, "_ps4_content_install_block_reason", None)
        ps4_button_text = ps4_button_text_fn(current_game) if callable(ps4_button_text_fn) else ""
        ps4_block_reason = ps4_block_reason_fn(current_game) if callable(ps4_block_reason_fn) else ""
        show_ps4_content = installed and bool(ps4_button_text)
        details_ps4_content_button.setText(ps4_button_text)
        details_ps4_content_button.setVisible(show_ps4_content)
        details_ps4_content_button.setEnabled(show_ps4_content and not installing_current and not ps4_block_reason)
        details_ps4_content_button.setToolTip(ps4_block_reason)

    cloud_sync_supported = not window._is_native_executable_platform(current_game)
    save_mode_supported = cloud_sync_supported and window._details_cloud_mode_supported(current_game, "save")
    state_mode_supported = cloud_sync_supported and window._details_cloud_mode_supported(current_game, "state")

    if window.details_details_button is not None:
        window.details_details_button.setVisible(True)
        window.details_details_button.setEnabled(True)
        window.details_details_button.setChecked(window.current_details_cloud_mode == "overview")
    if window.details_manage_saves_button is not None:
        window.details_manage_saves_button.setText(window._details_cloud_button_text(current_game, "save"))
        window.details_manage_saves_button.setVisible(save_mode_supported)
        window.details_manage_saves_button.setEnabled(save_mode_supported and not installing_current)
        if not save_mode_supported:
            window.details_manage_saves_button.setChecked(False)
    if window.details_manage_states_button is not None:
        window.details_manage_states_button.setText(window._details_cloud_button_text(current_game, "state"))
        window.details_manage_states_button.setVisible(state_mode_supported)
        window.details_manage_states_button.setEnabled(state_mode_supported and not installing_current)
        if not state_mode_supported:
            window.details_manage_states_button.setChecked(False)

    if (
        (window.current_details_cloud_mode == "save" and not save_mode_supported)
        or (window.current_details_cloud_mode == "state" and not state_mode_supported)
        or ((not save_mode_supported and not state_mode_supported) and window.current_details_cloud_mode != "overview")
    ):
        window._show_details_overview()

    show_secondary = installed and not is_emulator_entry
    if window.details_secondary_button is not None:
        window.details_secondary_button.setText("Uninstall")
        window.details_secondary_button.setVisible(show_secondary)
        window.details_secondary_button.setEnabled(show_secondary and not installing_current)

    details_update_button = getattr(window, "details_update_button", None)
    if details_update_button is not None:
        show_update = show_secondary and update_available
        update_button_text_fn = getattr(window, "_details_update_button_text_for_game", None)
        update_button_text = (
            update_button_text_fn(current_game)
            if callable(update_button_text_fn)
            else "Update"
        )
        details_update_button.setText(update_button_text)
        details_update_button.setVisible(show_update)
        details_update_button.setEnabled(show_update and not installing_current)
