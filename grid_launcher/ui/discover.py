"""Discover page widget and UI components."""

from __future__ import annotations

import math
import time
from typing import Any, Callable, Protocol

from PySide6.QtCore import Qt, QTimer, Signal
from PySide6.QtWidgets import (
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QScrollArea,
    QSizePolicy,
    QVBoxLayout,
    QWidget,
)

from ..ui.spinner import LoadingSpinnerWidget


class DiscoverWindowProtocol(Protocol):
    """Protocol for MainWindow interaction."""

    def _open_game_details(self, game: dict[str, str], source: str) -> None:
        ...

    def _theme_color(self, role: str, fallback: str) -> str:
        ...

    def _make_game_card(self, game: dict[str, str], source: str) -> QWidget:
        ...

    def _clear_layout(self, layout: QGridLayout) -> None:
        ...

    def navigate_to_server_platform(self, platform_display_name: str | None) -> None:
        ...

    def record_discover_event(self, event: str, section_id: str, rom_id: str) -> None:
        ...

    def toggle_watchlist(self, rom_id: str) -> None:
        ...

    def is_watchlisted(self, rom_id: str) -> bool:
        ...


class DiscoverCarouselSection(QWidget):
    """A carousel section displaying games in a horizontal grid."""

    collapsed_changed = Signal(str, bool)

    def __init__(
        self,
        section_id: str,
        title: str,
        games: list[dict[str, Any]],
        window: DiscoverWindowProtocol,
        see_all_callback: Callable[[], None] | None = None,
        parent: QWidget | None = None,
    ) -> None:
        """Initialize carousel section.
        
        Args:
            title: Section title
            games: List of game dicts to display
            window: MainWindow instance for callbacks
            parent: Parent widget
        """
        super().__init__(parent)
        self.title = title
        self.games = games
        self.window = window
        self.see_all_callback = see_all_callback
        self.game_cards: list[Any] = []
        self.section_id = section_id
        self.collapsed = False

        self._init_ui()

    def _init_ui(self) -> None:
        """Build the UI layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(8)

        # Section header
        header_row = QHBoxLayout()
        header_row.setSpacing(6)
        header_row.setContentsMargins(0, 0, 0, 0)

        self._toggle_btn = QPushButton("\u25bc")
        self._toggle_btn.setFlat(True)
        self._toggle_btn.setFixedWidth(24)
        self._toggle_btn.clicked.connect(self.toggle_collapsed)
        header_row.addWidget(self._toggle_btn)

        title_label = QLabel(self.title)
        title_label.setStyleSheet("font-size: 18px; font-weight: 700;")
        header_row.addWidget(title_label)
        if self.see_all_callback is not None:
            see_all_btn = QPushButton("\u2192 See All")
            see_all_btn.setFlat(True)
            see_all_btn.clicked.connect(self.see_all_callback)
            header_row.addWidget(see_all_btn)
        header_row.addStretch()
        layout.addLayout(header_row)

        # Horizontal carousel
        scroll = QScrollArea()
        scroll.setObjectName("discoverCarouselScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        scroll.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        scroll.setFixedHeight(290)
        scroll.viewport().setObjectName("discoverCarouselScrollViewport")

        content = QWidget()
        content.setObjectName("discoverCarouselContent")
        cards_layout = QHBoxLayout(content)
        cards_layout.setSpacing(12)
        cards_layout.setContentsMargins(4, 4, 4, 4)
        cards_layout.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignTop)

        for game in self.games:
            card = self.window._make_game_card(game, "discover")
            rom_id = game.get("rom_id", "")
            if rom_id and hasattr(card, "clicked"):
                card.clicked.connect(
                    lambda checked=False, sid=self.section_id, rid=rom_id: self.window.record_discover_event("card_opened", sid, rid)
                )
            cards_layout.addWidget(card)
            self.game_cards.append(card)

        content.setMinimumWidth(len(self.games) * 192)
        scroll.setWidget(content)
        layout.addWidget(scroll)
        self._content_scroll = scroll

    def toggle_collapsed(self) -> None:
        self.collapsed = not self.collapsed
        self._content_scroll.setVisible(not self.collapsed)
        self._toggle_btn.setText("\u25b6" if self.collapsed else "\u25bc")
        self.collapsed_changed.emit(self.section_id, self.collapsed)

    def apply_collapsed(self, collapsed: bool) -> None:
        self.collapsed = collapsed
        self._content_scroll.setVisible(not collapsed)
        self._toggle_btn.setText("\u25b6" if collapsed else "\u25bc")

    def update_games(self, games: list[dict]) -> None:
        content = self._content_scroll.widget()
        if content is None:
            return
        cards_layout = content.layout()
        if cards_layout is None:
            return
        while cards_layout.count() > 0:
            item = cards_layout.takeAt(0)
            w = item.widget() if item else None
            if w:
                w.deleteLater()
        self.game_cards.clear()
        for game in games:
            card = self.window._make_game_card(game, "discover")
            rom_id = game.get("rom_id", "")
            if rom_id and hasattr(card, "clicked"):
                card.clicked.connect(
                    lambda checked=False, sid=self.section_id, rid=rom_id: self.window.record_discover_event("card_opened", sid, rid)
                )
            cards_layout.addWidget(card)
            self.game_cards.append(card)
        content.setMinimumWidth(len(games) * 192)
        self._content_scroll.setVisible(not self.collapsed and bool(games))


class DiscoverGenreSection(QWidget):
    """Genre filter section with genre pills and game carousel."""

    games_selected = Signal(str)  # Emits selected genre
    collapsed_changed = Signal(str, bool)

    def __init__(
        self,
        section_id: str = "genres",
        genres: list[str] | None = None,
        games_by_genre: dict[str, list[dict[str, Any]]] | None = None,
        window: DiscoverWindowProtocol | None = None,
        parent: QWidget | None = None,
    ) -> None:
        """Initialize genre section.
        
        Args:
            genres: List of genre names
            games_by_genre: Dict mapping genre to list of games
            window: MainWindow instance
            parent: Parent widget
        """
        super().__init__(parent)
        self.genres = genres or []
        self.games_by_genre = games_by_genre or {}
        self.window = window
        self.selected_genre: str | None = None
        self.carousel_section: DiscoverCarouselSection | None = None
        self.section_id = section_id
        self.collapsed = False

        self._init_ui()

    def _init_ui(self) -> None:
        """Build the UI layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(12)

        # Header
        header_row = QHBoxLayout()
        header_row.setSpacing(6)
        header_row.setContentsMargins(0, 0, 0, 0)

        self._toggle_btn = QPushButton("\u25bc")
        self._toggle_btn.setFlat(True)
        self._toggle_btn.setFixedWidth(24)
        self._toggle_btn.clicked.connect(self.toggle_collapsed)
        header_row.addWidget(self._toggle_btn)

        header_label = QLabel("Browse by Genre")
        header_label.setStyleSheet("font-size: 18px; font-weight: 700;")
        header_row.addWidget(header_label)
        header_row.addStretch()
        layout.addLayout(header_row)

        self._content_widget = QWidget()
        content_inner = QVBoxLayout(self._content_widget)
        content_inner.setContentsMargins(0, 0, 0, 0)
        content_inner.setSpacing(12)

        # Genre pills
        pills_layout = QHBoxLayout()
        pills_layout.setSpacing(8)
        pills_layout.setContentsMargins(0, 0, 0, 0)

        self.genre_buttons: dict[str, QPushButton] = {}
        for genre in self.genres:
            button = QPushButton(genre)
            button.setCheckable(True)
            button.setMaximumWidth(100)
            button.clicked.connect(lambda checked=False, g=genre: self._on_genre_selected(g))
            self.genre_buttons[genre] = button
            pills_layout.addWidget(button)

        pills_layout.addStretch()
        content_inner.addLayout(pills_layout)

        # Game carousel (for selected genre)
        self.carousel_container = QVBoxLayout()
        self.carousel_container.setContentsMargins(0, 0, 0, 0)
        content_inner.addLayout(self.carousel_container)

        layout.addWidget(self._content_widget)

        # Select first genre by default
        if self.genres:
            self._on_genre_selected(self.genres[0])

    def toggle_collapsed(self) -> None:
        self.collapsed = not self.collapsed
        self._content_widget.setVisible(not self.collapsed)
        self._toggle_btn.setText("\u25b6" if self.collapsed else "\u25bc")
        self.collapsed_changed.emit(self.section_id, self.collapsed)

    def apply_collapsed(self, collapsed: bool) -> None:
        self.collapsed = collapsed
        self._content_widget.setVisible(not collapsed)
        self._toggle_btn.setText("\u25b6" if collapsed else "\u25bc")

    def _on_genre_selected(self, genre: str) -> None:
        """Handle genre selection.
        
        Args:
            genre: Selected genre name
        """
        # Uncheck all buttons
        for button in self.genre_buttons.values():
            button.setChecked(False)

        # Check selected button
        if genre in self.genre_buttons:
            self.genre_buttons[genre].setChecked(True)

    def set_genre_stats(self, stats: dict[str, tuple[int, int]]) -> None:
        for genre, button in self.genre_buttons.items():
            total, installed = stats.get(genre, (0, 0))
            if total > 0 and installed > 0:
                button.setText(f"{genre} ({total} / {installed})")
            elif total > 0:
                button.setText(f"{genre} ({total})")
            else:
                button.setText(genre)

        # Clear old carousel
        if self.carousel_section is not None:
            self.carousel_container.removeWidget(self.carousel_section)
            self.carousel_section.deleteLater()

        # Create new carousel for selected genre
        games = self.games_by_genre.get(genre, [])
        if games:
            self.carousel_section = DiscoverCarouselSection(
                f"genre_{genre}",
                f"Top {genre} Games",
                games,
                self.window,
                None,
                self,
            )
            self.carousel_container.addWidget(self.carousel_section)

        self.selected_genre = genre
        self.games_selected.emit(genre)


class DiscoverFilterPanel(QWidget):
    filters_changed = Signal(set, set)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._genre_checks: dict[str, QPushButton] = {}
        self._platform_checks: dict[str, QPushButton] = {}
        self._init_ui()

    def _init_ui(self) -> None:
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 4, 0, 4)
        layout.setSpacing(8)

        genre_row = QHBoxLayout()
        genre_row.setSpacing(6)
        genre_label = QLabel("Genre:")
        genre_label.setFixedWidth(60)
        genre_row.addWidget(genre_label)
        self._genre_pills_layout = QHBoxLayout()
        self._genre_pills_layout.setSpacing(4)
        genre_row.addLayout(self._genre_pills_layout)
        genre_row.addStretch()
        layout.addLayout(genre_row)

        platform_row = QHBoxLayout()
        platform_row.setSpacing(6)
        platform_label = QLabel("Platform:")
        platform_label.setFixedWidth(60)
        platform_row.addWidget(platform_label)
        self._platform_pills_layout = QHBoxLayout()
        self._platform_pills_layout.setSpacing(4)
        platform_row.addLayout(self._platform_pills_layout)
        platform_row.addStretch()
        layout.addLayout(platform_row)

        clear_row = QHBoxLayout()
        clear_btn = QPushButton("Clear Filters")
        clear_btn.setFixedWidth(100)
        clear_btn.clicked.connect(self.clear)
        clear_row.addWidget(clear_btn)
        clear_row.addStretch()
        layout.addLayout(clear_row)

    def populate(self, genres: list[str], platforms: list[str]) -> None:
        for btn in list(self._genre_checks.values()):
            btn.deleteLater()
        self._genre_checks.clear()
        for btn in list(self._platform_checks.values()):
            btn.deleteLater()
        self._platform_checks.clear()

        for genre in genres:
            btn = QPushButton(genre)
            btn.setCheckable(True)
            btn.clicked.connect(self._emit_filters)
            self._genre_checks[genre] = btn
            self._genre_pills_layout.addWidget(btn)

        for platform in platforms:
            btn = QPushButton(platform)
            btn.setCheckable(True)
            btn.clicked.connect(self._emit_filters)
            self._platform_checks[platform] = btn
            self._platform_pills_layout.addWidget(btn)

    def clear(self) -> None:
        for btn in self._genre_checks.values():
            btn.setChecked(False)
        for btn in self._platform_checks.values():
            btn.setChecked(False)
        self._emit_filters()

    def _emit_filters(self) -> None:
        selected_genres = {g for g, btn in self._genre_checks.items() if btn.isChecked()}
        selected_platforms = {p for p, btn in self._platform_checks.items() if btn.isChecked()}
        self.filters_changed.emit(selected_genres, selected_platforms)

    @property
    def selected_genres(self) -> set[str]:
        return {g for g, btn in self._genre_checks.items() if btn.isChecked()}

    @property
    def selected_platforms(self) -> set[str]:
        return {p for p, btn in self._platform_checks.items() if btn.isChecked()}


class DiscoverPageWidget(QWidget):
    """Main Discover page widget."""

    collapsed_states_changed = Signal(dict)
    preferences_requested = Signal()

    def __init__(
        self,
        window: DiscoverWindowProtocol,
        parent: QWidget | None = None,
    ) -> None:
        """Initialize Discover page.
        
        Args:
            window: MainWindow instance
            parent: Parent widget
        """
        super().__init__(parent)
        self.window = window
        self.is_loading = False
        self.sections: dict[str, QWidget] = {}
        self._collapsed_states: dict[str, bool] = {}
        self._active_genre_filter: set[str] = set()
        self._active_platform_filter: set[str] = set()
        self._section_games: dict[str, list[dict]] = {}

        self._init_ui()

    def _init_ui(self) -> None:
        """Build the UI layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(12, 12, 12, 12)
        layout.setSpacing(24)

        # Header
        header_layout = QHBoxLayout()
        header_layout.setSpacing(8)

        title_label = QLabel("Discover")
        title_label.setStyleSheet("font-size: 20px; font-weight: 700;")
        header_layout.addWidget(title_label)
        header_layout.addStretch()

        self._last_refresh_time: float = 0.0
        self.last_refresh_label = QLabel("")
        self.last_refresh_label.hide()
        header_layout.addWidget(self.last_refresh_label)

        filter_btn = QPushButton("Filter \u25be")
        filter_btn.setCheckable(True)
        filter_btn.clicked.connect(self._toggle_filter_panel)
        self.filter_btn = filter_btn
        header_layout.addWidget(filter_btn)

        prefs_button = QPushButton("\u2699")
        prefs_button.setFlat(True)
        prefs_button.setFixedWidth(28)
        prefs_button.clicked.connect(lambda: self.preferences_requested.emit())
        self.prefs_button = prefs_button
        header_layout.addWidget(prefs_button)

        refresh_button = QPushButton("Refresh")
        refresh_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        refresh_button.clicked.connect(self._on_refresh_clicked)
        self.refresh_button = refresh_button
        header_layout.addWidget(refresh_button)

        layout.addLayout(header_layout)

        self._filter_panel = DiscoverFilterPanel(self)
        self._filter_panel.hide()
        self._filter_panel.filters_changed.connect(self._on_filters_changed)
        layout.addWidget(self._filter_panel)

        # Main scroll area for sections
        scroll = QScrollArea()
        scroll.setObjectName("discoverMainScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.viewport().setObjectName("discoverMainScrollViewport")

        content = QWidget()
        content.setObjectName("discoverMainContent")
        content_layout = QVBoxLayout(content)
        content_layout.setSpacing(32)
        content_layout.setContentsMargins(0, 0, 0, 0)

        self.content_layout = content_layout
        self.content_widget = content

        scroll.setWidget(content)

        self.status_label = QLabel("Loading discover sections...")
        self.status_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        muted_color = self.window._theme_color("muted", "#9baed6")
        self.status_label.setStyleSheet(f"color: {muted_color}; font-size: 14px;")
        self.status_label.hide()
        layout.addWidget(self.status_label)

        layout.addWidget(scroll, 1)

        # Loading spinner overlays the scroll area
        self.loading_spinner = LoadingSpinnerWidget(self)
        self.loading_spinner.hide()

    def refresh_theme(self, colors: dict) -> None:
        from grid_launcher.ui.theme import theme_color as _tc
        muted = _tc(colors, "muted", "#9baed6")
        self.status_label.setStyleSheet(f"color: {muted}; font-size: 14px;")

    def update_last_refresh_time(self, ts: float) -> None:
        self._last_refresh_time = ts
        if ts == 0:
            self.last_refresh_label.hide()
            return
        delta = time.time() - ts
        if delta < 60:
            text = "Updated just now"
        elif delta < 3600:
            text = f"Updated {int(delta // 60)} minutes ago"
        elif delta < 86400:
            text = f"Updated {int(delta // 3600)} hours ago"
        else:
            text = f"Updated {int(delta // 86400)} days ago"
        self.last_refresh_label.setText(text)
        self.last_refresh_label.show()

    def set_collapsed_states(self, states: dict[str, bool]) -> None:
        self._collapsed_states = dict(states)

    def _on_section_collapse_changed(self, section_id: str, collapsed: bool) -> None:
        self._collapsed_states[section_id] = collapsed
        self.collapsed_states_changed.emit(dict(self._collapsed_states))

    def set_loading(self, loading: bool) -> None:
        """Set loading state.
        
        Args:
            loading: True to show loading spinner, False to hide
        """
        self.is_loading = loading
        if loading:
            self.status_label.show()
            self.loading_spinner.show()
            self.loading_spinner.raise_()
        else:
            self.status_label.hide()
            self.loading_spinner.hide()

    def _clear_sections(self) -> None:
        """Clear all sections from content layout."""
        while self.content_layout.count() > 0:
            item = self.content_layout.takeAt(0)
            if item and item.widget():
                item.widget().deleteLater()
        self.sections.clear()

    def add_carousel_section(
        self,
        section_id: str,
        title: str,
        games: list[dict[str, Any]],
        see_all_callback: Callable[[], None] | None = None,
    ) -> None:
        """Add a carousel section.
        
        Args:
            section_id: Unique section identifier
            title: Section title
            games: List of game dicts
        """
        section = DiscoverCarouselSection(
            section_id, title, games, self.window, see_all_callback, self
        )
        section.collapsed_changed.connect(self._on_section_collapse_changed)
        if self._collapsed_states.get(section_id):
            section.apply_collapsed(True)
        self.content_layout.addWidget(section)
        self.sections[section_id] = section
        self._section_games[section_id] = list(games)

    def add_watchlist_section(self, games: list[dict]) -> None:
        if "watchlist" in self.sections:
            old = self.sections.pop("watchlist")
            old.deleteLater()

        if not games:
            empty_label = QLabel("No saved games yet. Click \u2606 on any game to save it here.")
            empty_label.setObjectName("watchlistEmpty")
            muted = self.window._theme_color("muted", "#9baed6")
            empty_label.setStyleSheet(f"color: {muted}; font-size: 13px;")
            empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
            container = QWidget()
            container.setObjectName("watchlistEmptyContainer")
            container_layout = QVBoxLayout(container)
            container_layout.setContentsMargins(0, 8, 0, 8)
            container_layout.addWidget(empty_label)
            self.content_layout.addWidget(container)
            self.sections["watchlist"] = container
        else:
            section = DiscoverCarouselSection("watchlist", "Your Watchlist", games, self.window, None, self)
            section.collapsed_changed.connect(self._on_section_collapse_changed)
            if self._collapsed_states.get("watchlist"):
                section.apply_collapsed(True)
            self.content_layout.addWidget(section)
            self.sections["watchlist"] = section
            self._section_games["watchlist"] = list(games)

    def _toggle_filter_panel(self) -> None:
        visible = not self._filter_panel.isVisible()
        self._filter_panel.setVisible(visible)
        self.filter_btn.setText("Filter \u25b4" if visible else "Filter \u25be")
        self.filter_btn.setChecked(visible)

    def set_filter_options(self, genres: list[str], platforms: list[str]) -> None:
        self._filter_panel.populate(genres, platforms)

    def _on_filters_changed(self, genres: set[str], platforms: set[str]) -> None:
        self._active_genre_filter = genres
        self._active_platform_filter = platforms
        self._apply_filters()

    def _apply_filters(self) -> None:
        from grid_launcher.server.discover import client_filter_games
        for section_id, section in self.sections.items():
            if isinstance(section, DiscoverCarouselSection):
                games = self._section_games.get(section_id, section.games)
                filtered = client_filter_games(games, self._active_genre_filter, self._active_platform_filter)
                section.update_games(filtered)

    def add_genre_section(
        self,
        genres: list[str],
        games_by_genre: dict[str, list[dict[str, Any]]],
    ) -> None:
        """Add genre filter section.
        
        Args:
            genres: List of available genres
            games_by_genre: Dict mapping genre to games
        """
        section = DiscoverGenreSection("genres", genres, games_by_genre, self.window, self)
        section.collapsed_changed.connect(self._on_section_collapse_changed)
        if self._collapsed_states.get("genres"):
            section.apply_collapsed(True)
        self.content_layout.addWidget(section)
        self.sections["genres"] = section

    def add_stretch(self) -> None:
        """Add stretch to push content to top."""
        self.content_layout.addStretch()

    def _on_refresh_clicked(self) -> None:
        """Handle refresh button click."""
        # This will be connected to MainWindow's refresh handler
        pass

    def set_refresh_callback(self, callback: Callable[[], None]) -> None:
        """Set callback for refresh button.
        
        Args:
            callback: Function to call when refresh is clicked
        """
        self.refresh_button.clicked.disconnect()
        self.refresh_button.clicked.connect(callback)
