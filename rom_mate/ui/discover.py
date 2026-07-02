"""Discover page widget and UI components."""

from __future__ import annotations

import math
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


class DiscoverCarouselSection(QWidget):
    """A carousel section displaying games in a horizontal grid."""

    def __init__(
        self,
        title: str,
        games: list[dict[str, Any]],
        window: DiscoverWindowProtocol,
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
        self.game_cards: list[Any] = []

        self._init_ui()

    def _init_ui(self) -> None:
        """Build the UI layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(8)

        # Section header
        title_label = QLabel(self.title)
        title_label.setStyleSheet("font-size: 18px; font-weight: 700;")
        layout.addWidget(title_label)

        # Horizontal carousel
        scroll = QScrollArea()
        scroll.setObjectName("discoverCarouselScroll")
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAsNeeded)
        scroll.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        scroll.setFixedHeight(290)

        content = QWidget()
        content.setObjectName("discoverCarouselContent")
        cards_layout = QHBoxLayout(content)
        cards_layout.setSpacing(12)
        cards_layout.setContentsMargins(4, 4, 4, 4)
        cards_layout.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignTop)

        for game in self.games:
            card = self.window._make_game_card(game, "discover")
            cards_layout.addWidget(card)
            self.game_cards.append(card)

        content.setMinimumWidth(len(self.games) * 192)
        scroll.setWidget(content)
        layout.addWidget(scroll)


class DiscoverGenreSection(QWidget):
    """Genre filter section with genre pills and game carousel."""

    games_selected = Signal(str)  # Emits selected genre

    def __init__(
        self,
        genres: list[str],
        games_by_genre: dict[str, list[dict[str, Any]]],
        window: DiscoverWindowProtocol,
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
        self.genres = genres
        self.games_by_genre = games_by_genre
        self.window = window
        self.selected_genre: str | None = None
        self.carousel_section: DiscoverCarouselSection | None = None

        self._init_ui()

    def _init_ui(self) -> None:
        """Build the UI layout."""
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(12)

        # Header
        header_label = QLabel("Browse by Genre")
        header_label.setStyleSheet("font-size: 18px; font-weight: 700;")
        layout.addWidget(header_label)

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
        layout.addLayout(pills_layout)

        # Game carousel (for selected genre)
        self.carousel_container = QVBoxLayout()
        self.carousel_container.setContentsMargins(0, 0, 0, 0)
        layout.addLayout(self.carousel_container)

        # Select first genre by default
        if self.genres:
            self._on_genre_selected(self.genres[0])

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

        # Clear old carousel
        if self.carousel_section is not None:
            self.carousel_container.removeWidget(self.carousel_section)
            self.carousel_section.deleteLater()

        # Create new carousel for selected genre
        games = self.games_by_genre.get(genre, [])
        if games:
            self.carousel_section = DiscoverCarouselSection(
                f"Top {genre} Games",
                games,
                self.window,
                self,
            )
            self.carousel_container.addWidget(self.carousel_section)

        self.selected_genre = genre
        self.games_selected.emit(genre)


class DiscoverPageWidget(QWidget):
    """Main Discover page widget."""

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

        refresh_button = QPushButton("Refresh")
        refresh_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        refresh_button.clicked.connect(self._on_refresh_clicked)
        self.refresh_button = refresh_button
        header_layout.addWidget(refresh_button)

        layout.addLayout(header_layout)

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
        self.status_label.setStyleSheet("color: #6272a4; font-size: 14px;")
        self.status_label.hide()
        layout.addWidget(self.status_label)

        layout.addWidget(scroll, 1)

        # Loading spinner overlays the scroll area
        self.loading_spinner = LoadingSpinnerWidget(self)
        self.loading_spinner.hide()

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
    ) -> None:
        """Add a carousel section.
        
        Args:
            section_id: Unique section identifier
            title: Section title
            games: List of game dicts
        """
        section = DiscoverCarouselSection(title, games, self.window, self)
        self.content_layout.addWidget(section)
        self.sections[section_id] = section

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
        section = DiscoverGenreSection(genres, games_by_genre, self.window, self)
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
