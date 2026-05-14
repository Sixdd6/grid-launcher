from __future__ import annotations

from typing import Any, Callable, Protocol

from PySide6.QtCore import QTimer, Qt
from PySide6.QtWidgets import QFrame, QHBoxLayout, QLabel, QLayout, QPushButton, QScrollArea, QVBoxLayout, QWidget

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
    details_platform_group: QWidget | None
    details_rating_group: QWidget | None
    details_regions_group: QWidget | None
    details_filesize_group: QWidget | None
    details_version_group: QWidget | None
    details_genres_group: QWidget | None
    details_genres_layout: QHBoxLayout | None
    details_companies_group: QWidget | None
    details_companies_label: QLabel | None
    details_release_date_group: QWidget | None
    details_release_date_label: QLabel | None
    details_languages_group: QWidget | None
    details_languages_label: QLabel | None
    details_platform_label: QLabel | None
    details_genres_label: QLabel | None
    details_regions_label: QLabel | None
    details_filesize_label: QLabel | None
    details_version_label: QLabel | None
    details_rating_label: QLabel | None
    details_description_label: QLabel | None
    details_primary_button: QPushButton | None
    details_config_button: QPushButton | None
    details_details_button: QPushButton | None
    details_manage_saves_button: QPushButton | None
    details_manage_states_button: QPushButton | None
    details_ps4_content_button: QPushButton | None
    details_xbox360_content_button: QPushButton | None
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

    def _details_xbox360_content_button_text(self, game: dict[str, str]) -> str:
        ...

    def _xbox360_content_install_block_reason(self, game: dict[str, str]) -> str:
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

    def _format_size(self, size_bytes: float) -> str:
        ...


class GameAchievementsWindowProtocol(Protocol):
    details_achievements_button: QPushButton


def _is_windows_pc_platform(platform_value: object) -> bool:
    if not isinstance(platform_value, str):
        return False
    platform = platform_value.strip().casefold()
    if not platform:
        return False
    return "windows" in platform or platform == "pc"


def _default_details_version_label_text(game: dict[str, str]) -> str:
    if _is_windows_pc_platform(game.get("platform", "")):
        rom_file_name = game.get("rom_file_name", "")
        version_tag = rom_file_name_version(rom_file_name)
        if version_tag is not None:
            return f"Version: v{version_tag:05d}"
    revision = game.get("revision", "")
    if isinstance(revision, str) and revision.strip():
        return revision.strip()
    return ""


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

    layout.addWidget(cover)
    layout.activate()
    window._queue_game_cover_load(game, cover)

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
        platform_value = game.get("platform", "")
        platform_text = platform_value.strip() if isinstance(platform_value, str) else ""
        platform_group = getattr(window, "details_platform_group", None)
        if platform_group is not None:
            window.details_platform_label.setText(platform_text)
            platform_group.setVisible(True)
        else:
            window.details_platform_label.setText(f"Platform: {platform_text}")
            window.details_platform_label.setVisible(bool(platform_text))

    genres_value = game.get("genres", "")
    genres_text = genres_value.strip() if isinstance(genres_value, str) else ""
    genres_layout = getattr(window, "details_genres_layout", None)
    genres_group = getattr(window, "details_genres_group", None)
    if genres_layout is not None:
        while genres_layout.count() > 0:
            item = genres_layout.takeAt(0)
            chip_widget = item.widget()
            if chip_widget is not None:
                chip_widget.deleteLater()
        genre_parts = [part.strip() for part in genres_text.split(",")]
        surface = window._theme_color("surface", "#44475a")
        text_color = window._theme_color("text", "#f8f8f2")
        for genre in genre_parts:
            if not genre:
                continue
            chip = QLabel(genre)
            chip.setStyleSheet(
                f"background-color: {surface}; color: {text_color}; border-radius: 10px; padding: 3px 10px; font-size: 12px;"
            )
            genres_layout.addWidget(chip)
        genres_layout.addStretch()
        if genres_group is not None:
            genres_group.setVisible(bool(genres_text))
    elif window.details_genres_label is not None:
        # Legacy fallback for test stubs that don't have the chip layout
        window.details_genres_label.setText(f"Genres: {genres_text}")
        window.details_genres_label.setVisible(bool(genres_text))
        if genres_group is not None:
            genres_group.setVisible(bool(genres_text))

    if window.details_regions_label is not None:
        regions_value = game.get("regions", "")
        regions_text = regions_value.strip() if isinstance(regions_value, str) else ""
        regions_group = getattr(window, "details_regions_group", None)
        if regions_group is not None:
            window.details_regions_label.setText(regions_text)
            regions_group.setVisible(bool(regions_text))
        else:
            window.details_regions_label.setText(f"Regions: {regions_text}")
            window.details_regions_label.setVisible(bool(regions_text))

    if window.details_filesize_label is not None:
        filesize_text = ""
        filesize_value = game.get("filesize_bytes", "")
        raw_filesize = filesize_value.strip() if isinstance(filesize_value, str) else ""
        if raw_filesize.isdigit():
            format_size_fn = getattr(window, "_format_size", None)
            if callable(format_size_fn):
                filesize_text = format_size_fn(float(raw_filesize))
            else:
                filesize_text = raw_filesize
        filesize_group = getattr(window, "details_filesize_group", None)
        if filesize_group is not None:
            window.details_filesize_label.setText(filesize_text)
            filesize_group.setVisible(bool(filesize_text))
        else:
            window.details_filesize_label.setText(f"Filesize: {filesize_text}")
            window.details_filesize_label.setVisible(bool(filesize_text))
    version_label_text_fn = getattr(window, "_details_version_label_text_for_game", None)
    version_label_text = (
        version_label_text_fn(game)
        if callable(version_label_text_fn)
        else _default_details_version_label_text(game)
    )
    if window.details_version_label is not None:
        version_group = getattr(window, "details_version_group", None)
        if version_group is not None:
            normalized_version_text = version_label_text.strip() if isinstance(version_label_text, str) else ""
            if normalized_version_text.casefold().startswith("version:"):
                normalized_version_text = normalized_version_text.split(":", 1)[1].strip()
            window.details_version_label.setText(normalized_version_text)
            version_group.setVisible(bool(normalized_version_text))
        else:
            fallback_version_text = version_label_text if isinstance(version_label_text, str) else ""
            window.details_version_label.setText(fallback_version_text)
            window.details_version_label.setVisible(bool(fallback_version_text))
    if window.details_rating_label is not None:
        rating_value = game.get("rating", "")
        rating_text = rating_value.strip() if isinstance(rating_value, str) else ""
        if rating_text.casefold() == "n/a":
            rating_text = ""
        rating_group = getattr(window, "details_rating_group", None)
        if rating_group is not None:
            theme_color_fn = getattr(window, "_theme_color", None)
            accent = theme_color_fn("accent", "#8be9fd") if callable(theme_color_fn) else "#8be9fd"
            if rating_text:
                window.details_rating_label.setText(f'<span style="color:{accent}">★</span> {rating_text}')
            else:
                window.details_rating_label.setText("")
            rating_group.setVisible(bool(rating_text))
        else:
            window.details_rating_label.setText(f"Rating: {rating_text}")
            window.details_rating_label.setVisible(bool(rating_text))
    if window.details_description_label is not None:
        description_value = game.get("description", "")
        description_text = description_value.strip() if isinstance(description_value, str) else ""
        if description_text.casefold() == "no description available.":
            description_text = ""
        window.details_description_label.setText(description_text)
        window.details_description_label.setVisible(bool(description_text))

    companies_value = game.get("companies", "")
    companies_text = companies_value.strip() if isinstance(companies_value, str) else ""
    companies_group = getattr(window, "details_companies_group", None)
    companies_label = getattr(window, "details_companies_label", None)
    if companies_group is not None and companies_label is not None:
        companies_label.setText(companies_text)
        companies_group.setVisible(bool(companies_text))

    release_date_value = game.get("first_release_date", "")
    release_date_text = release_date_value.strip() if isinstance(release_date_value, str) else ""
    release_date_group = getattr(window, "details_release_date_group", None)
    release_date_label = getattr(window, "details_release_date_label", None)
    if release_date_group is not None and release_date_label is not None:
        release_date_label.setText(release_date_text)
        release_date_group.setVisible(bool(release_date_text))

    languages_value = game.get("languages", "")
    languages_text = languages_value.strip() if isinstance(languages_value, str) else ""
    languages_group = getattr(window, "details_languages_group", None)
    languages_label = getattr(window, "details_languages_label", None)
    if languages_group is not None and languages_label is not None:
        languages_label.setText(languages_text)
        languages_group.setVisible(bool(languages_text))

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

    details_xbox360_content_button = getattr(window, "details_xbox360_content_button", None)
    if details_xbox360_content_button is not None:
        xbox360_button_text_fn = getattr(window, "_details_xbox360_content_button_text", None)
        xbox360_block_reason_fn = getattr(window, "_xbox360_content_install_block_reason", None)
        xbox360_button_text = xbox360_button_text_fn(current_game) if callable(xbox360_button_text_fn) else ""
        xbox360_block_reason = xbox360_block_reason_fn(current_game) if callable(xbox360_block_reason_fn) else ""
        show_xbox360_content = installed and bool(xbox360_button_text)
        details_xbox360_content_button.setText(xbox360_button_text)
        details_xbox360_content_button.setVisible(show_xbox360_content)
        details_xbox360_content_button.setEnabled(show_xbox360_content and not installing_current and not xbox360_block_reason)
        details_xbox360_content_button.setToolTip(xbox360_block_reason)

    is_native = window._is_native_executable_platform(current_game)
    cloud_sync_supported = not is_native
    # Native games get their own save panel (PCGamingWiki-backed) - show button if installed
    if is_native:
        save_mode_supported = installed and window._details_cloud_mode_supported(current_game, "save")
        state_mode_supported = False
    else:
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


def _format_achievement_date(raw: str) -> str:
    from PySide6.QtCore import QDateTime, QLocale

    dt = QDateTime.fromString(raw.strip(), "yyyy-MM-dd HH:mm:ss")
    if not dt.isValid():
        return raw.strip()
    locale = QLocale.system()
    return locale.toString(dt, QLocale.FormatType.ShortFormat)


def build_achievements_panel(
    achievements: list[dict],
    load_image_fn: Callable[[str, QLabel], None] | None = None,
) -> QWidget:
    container = QFrame()
    container.setObjectName("panel")
    layout = QVBoxLayout(container)
    layout.setContentsMargins(12, 12, 12, 12)
    layout.setSpacing(4)

    if not achievements:
        empty_label = QLabel("No achievements found.")
        empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        empty_label.setObjectName("achievementsEmptyLabel")
        layout.addWidget(empty_label)
        layout.addStretch()
        return container

    scroll = QScrollArea()
    scroll.setWidgetResizable(True)
    scroll.setFrameShape(QFrame.Shape.NoFrame)
    scroll.setStyleSheet("QScrollArea { background: transparent; border: none; } QScrollArea > QWidget > QWidget { background: transparent; }")
    inner = QWidget()
    inner.setAutoFillBackground(False)
    inner_layout = QVBoxLayout(inner)
    inner_layout.setContentsMargins(0, 0, 0, 0)
    inner_layout.setSpacing(6)

    achievements = sorted(achievements, key=lambda a: (not bool(a.get("date_earned", "")), str(a.get("title", ""))))
    for ach in achievements:
        row = QFrame()
        row.setObjectName("achievementRow")
        row_layout = QHBoxLayout(row)
        row_layout.setContentsMargins(8, 6, 8, 6)
        row_layout.setSpacing(10)

        earned = bool(ach.get("date_earned", ""))
        badge_label = QLabel()
        badge_label.setFixedSize(48, 48)
        badge_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        badge_label.setObjectName("achievementBadge")
        badge_name = str(ach.get("badge_name", "")).strip()
        if badge_name and load_image_fn is not None:
            badge_url = f"https://media.retroachievements.org/Badge/{badge_name}{'_lock' if not earned else ''}.png"
            load_image_fn(badge_url, badge_label)
        row_layout.addWidget(badge_label)

        indicator = QLabel("✓" if earned else "○")
        indicator.setFixedWidth(18)
        indicator.setObjectName("achievementEarned" if earned else "achievementLocked")
        row_layout.addWidget(indicator)

        text_col = QVBoxLayout()
        text_col.setSpacing(2)
        name_label = QLabel(str(ach.get("title", "")))
        name_label.setObjectName("achievementTitle")
        name_label.setWordWrap(True)
        desc_label = QLabel(str(ach.get("description", "")))
        desc_label.setObjectName("achievementDesc")
        desc_label.setWordWrap(True)
        text_col.addWidget(name_label)
        text_col.addWidget(desc_label)
        date_earned = str(ach.get("date_earned", "")).strip()
        if date_earned:
            date_label = QLabel(f"Unlocked: {_format_achievement_date(date_earned)}")
            date_label.setObjectName("achievementDate")
            text_col.addWidget(date_label)
        row_layout.addLayout(text_col, 1)

        pts_label = QLabel(f"{ach.get('points', 0)} pts")
        pts_label.setObjectName("achievementPoints")
        pts_label.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        row_layout.addWidget(pts_label)

        inner_layout.addWidget(row)

    inner_layout.addStretch()
    scroll.setWidget(inner)
    layout.addWidget(scroll)
    return container
