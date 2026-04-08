from __future__ import annotations

from collections.abc import Iterable, Mapping
from typing import Any

from PySide6.QtGui import QPalette
from PySide6.QtWidgets import QApplication, QLabel


_LIGHT_THEME_COLORS: dict[str, str] = {
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

_DARK_THEME_COLORS: dict[str, str] = {
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


def normalized_theme_choice(value: Any) -> str:
    if not isinstance(value, str):
        return "system"
    normalized = value.strip().casefold()
    if normalized in {"system", "dark", "light"}:
        return normalized
    return "system"


def resolved_theme_variant(theme_choice: str, app: QApplication | None = None) -> str:
    normalized = normalized_theme_choice(theme_choice)
    if normalized != "system":
        return normalized
    active_app = app or QApplication.instance()
    if active_app is None:
        return "dark"
    palette = active_app.palette()
    if isinstance(palette, QPalette):
        window_color = palette.color(QPalette.ColorRole.Window)
        if window_color.value() < 128:
            return "dark"
    return "light"


def theme_colors(theme_variant: str) -> dict[str, str]:
    if theme_variant == "light":
        return dict(_LIGHT_THEME_COLORS)
    return dict(_DARK_THEME_COLORS)


def theme_color(colors: Mapping[str, str] | None, key: str, fallback: str) -> str:
    value = colors.get(key, "") if isinstance(colors, Mapping) else ""
    if isinstance(value, str) and value.strip():
        return value
    return fallback


def theme_stylesheet(colors: Mapping[str, str] | None) -> str:
    resolved_colors = theme_colors("dark")
    if isinstance(colors, Mapping):
        for key, value in colors.items():
            if isinstance(value, str) and value.strip():
                resolved_colors[key] = value
    return f"""
        QMainWindow {{
            background-color: {resolved_colors['window']};
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
            background-color: {resolved_colors['surface']};
            color: {resolved_colors['text']};
        }}
        QMessageBox QLabel,
        QDialog QLabel,
        QInputDialog QLabel,
        QFileDialog QLabel,
        QColorDialog QLabel,
        QFontDialog QLabel {{
            color: {resolved_colors['text']};
        }}
        QMenu {{
            background-color: {resolved_colors['surface']};
            color: {resolved_colors['text']};
            border: 1px solid {resolved_colors['border']};
        }}
        QMenu::item:selected {{
            background-color: {resolved_colors['surface_alt']};
            color: {resolved_colors['text']};
        }}
        QToolTip {{
            background-color: {resolved_colors['surface']};
            color: {resolved_colors['text']};
            border: 1px solid {resolved_colors['border']};
        }}
        QLabel {{
            color: {resolved_colors['text']};
        }}
        QCheckBox {{
            color: {resolved_colors['text']};
        }}
        QPushButton {{
            background-color: {resolved_colors['surface']};
            color: {resolved_colors['text']};
            border: 1px solid {resolved_colors['border']};
            border-radius: 8px;
            padding: 8px 14px;
            font-weight: 600;
        }}
        QPushButton:hover {{
            border-color: {resolved_colors['accent']};
            background-color: {resolved_colors['surface_alt']};
        }}
        QPushButton:pressed {{
            background-color: {resolved_colors['surface_press']};
            border-color: {resolved_colors['accent']};
        }}
        QPushButton:checked {{
            background-color: {resolved_colors['active']};
            border-color: {resolved_colors['active']};
            color: {resolved_colors['active_text']};
        }}
        QPushButton#detailsCloudActionButton {{
            background-color: {resolved_colors['surface_alt']};
            color: {resolved_colors['text']};
            border: 1px solid {resolved_colors['border']};
            border-radius: 8px;
            min-width: 90px;
            padding: 8px 12px;
        }}
        QPushButton#detailsCloudActionButton:hover {{
            background-color: {resolved_colors['surface']};
            border-color: {resolved_colors['accent']};
        }}
        QPushButton#detailsCloudActionButton:pressed {{
            background-color: {resolved_colors['surface_press']};
            border-color: {resolved_colors['accent']};
        }}
        QPushButton[role="danger"],
        QPushButton#detailsCloudActionButton[role="danger"] {{
            background-color: {resolved_colors['surface']};
            color: {resolved_colors['error']};
            border: 1px solid {resolved_colors['error']};
        }}
        QPushButton[role="danger"]:hover,
        QPushButton#detailsCloudActionButton[role="danger"]:hover {{
            background-color: {resolved_colors['error']};
            border-color: {resolved_colors['error']};
            color: #ffffff;
        }}
        QPushButton[role="danger"]:pressed,
        QPushButton#detailsCloudActionButton[role="danger"]:pressed {{
            background-color: {resolved_colors['error']};
            border-color: {resolved_colors['warning']};
            color: #ffffff;
        }}
        QWidget#downloadStatusWidget {{
            background-color: {resolved_colors['surface']};
            border: 1px solid {resolved_colors['border']};
            border-radius: 8px;
        }}
        QProgressBar {{
            border: 1px solid {resolved_colors['border']};
            border-radius: 6px;
            background-color: {resolved_colors['window']};
            color: {resolved_colors['text']};
            text-align: center;
        }}
        QProgressBar::chunk {{
            background-color: {resolved_colors['success']};
            border-radius: 5px;
        }}
        QPushButton#gameCard {{
            text-align: left;
            background-color: {resolved_colors['surface']};
            border: 1px solid {resolved_colors['border']};
            border-radius: 10px;
            padding: 0;
        }}
        QPushButton#gameCard:hover {{
            border-color: {resolved_colors['accent']};
        }}
        QListWidget {{
            background-color: {resolved_colors['surface']};
            color: {resolved_colors['text']};
            border: 1px solid {resolved_colors['border']};
            border-radius: 8px;
            padding: 4px;
        }}
        QListWidget::item:selected {{
            background-color: {resolved_colors['border']};
            color: {resolved_colors['text']};
            border-radius: 5px;
        }}
        QListWidget#defaultMappingList::item:alternate {{
            background-color: {resolved_colors['surface_alt']};
        }}
        QLineEdit, QComboBox {{
            background-color: {resolved_colors['input_bg']};
            color: {resolved_colors['input_text']};
            border: 1px solid {resolved_colors['border']};
            border-radius: 6px;
            padding: 6px 8px;
        }}
        QLineEdit:focus, QComboBox:focus {{
            border: 1px solid {resolved_colors['accent']};
        }}
        QFrame#serverSearchContainer {{
            background-color: {resolved_colors['input_bg']};
            border: 1px solid {resolved_colors['border']};
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
            color: {resolved_colors['text']};
            font-weight: 700;
            border-radius: 0;
            padding: 0;
        }}
        QPushButton#serverSearchClearButton:hover {{
            border: none;
            color: {resolved_colors['text']};
            background-color: {resolved_colors['surface']};
        }}
        QPushButton#serverSearchClearButton:pressed {{
            border: none;
            color: {resolved_colors['text']};
            background-color: {resolved_colors['surface_press']};
        }}
        QFrame#panel {{
            background-color: {resolved_colors['surface']};
            border: 1px solid {resolved_colors['border']};
            border-radius: 10px;
        }}
        QFrame#detailsCloudListPanel {{
            background-color: transparent;
            border: none;
            border-radius: 0;
        }}
        QFrame#detailsCloudRecord {{
            background-color: {resolved_colors['input_bg']};
            border: 1px solid {resolved_colors['accent']};
            border-radius: 10px;
        }}
        QComboBox QAbstractItemView {{
            background-color: {resolved_colors['surface']};
            color: {resolved_colors['text']};
            border: 1px solid {resolved_colors['border']};
            selection-background-color: {resolved_colors['border']};
            selection-color: {resolved_colors['text']};
        }}
        QScrollArea#libraryScroll,
        QScrollArea#serverGamesScroll,
        QScrollArea#downloadsScroll,
        QScrollArea#detailsCloudScroll {{
            background-color: transparent;
            border: none;
        }}
        QWidget#libraryScrollViewport,
        QWidget#serverGamesScrollViewport,
        QWidget#downloadsScrollViewport,
        QWidget#detailsCloudScrollViewport,
        QWidget#libraryGridContent,
        QWidget#serverGamesContent,
        QWidget#downloadsContent {{
            background-color: transparent;
        }}
        QListWidget#serverPlatformsList {{
            background-color: transparent;
        }}
        QListWidget#serverPlatformsList::item:hover {{
            background-color: {resolved_colors['surface_alt']};
            border-radius: 5px;
        }}
    """


def apply_theme_inline_styles(
    colors: Mapping[str, str] | None,
    *,
    download_count_label: QLabel | None = None,
    download_speed_label: QLabel | None = None,
    account_status_label: QLabel | None = None,
    library_empty_label: QLabel | None = None,
    downloads_empty_label: QLabel | None = None,
    details_cover_label: QLabel | None = None,
    details_cloud_status_label: QLabel | None = None,
    details_cloud_empty_label: QLabel | None = None,
    screenshot_labels: Iterable[QLabel] | None = None,
) -> None:
    text = theme_color(colors, "text", "#f8f8f2")
    muted = theme_color(colors, "muted", "#6272a4")
    accent = theme_color(colors, "accent", "#8be9fd")
    window = theme_color(colors, "window", "#282a36")
    border = theme_color(colors, "border", "#6272a4")

    if download_count_label is not None:
        download_count_label.setStyleSheet(f"font-weight: 600; color: {text};")
    if download_speed_label is not None:
        download_speed_label.setStyleSheet(f"font-weight: 600; color: {accent};")
    if account_status_label is not None:
        account_status_label.setStyleSheet(f"font-weight: 600; color: {text};")
    if library_empty_label is not None:
        library_empty_label.setStyleSheet(f"color: {muted}; font-size: 16px; font-weight: 600;")
    if downloads_empty_label is not None:
        downloads_empty_label.setStyleSheet(f"color: {muted}; font-size: 16px; font-weight: 600;")
    if details_cover_label is not None:
        details_cover_label.setStyleSheet(
            f"background-color: {window}; border: 1px dashed {border}; border-radius: 8px; font-size: 20px;"
        )
    if details_cloud_status_label is not None:
        details_cloud_status_label.setStyleSheet(f"color: {muted};")
    if details_cloud_empty_label is not None:
        details_cloud_empty_label.setStyleSheet(f"color: {muted}; font-size: 15px;")
    for screenshot_label in screenshot_labels or ():
        screenshot_label.setStyleSheet(
            f"background-color: {window}; border: 1px dashed {border}; border-radius: 8px;"
        )


__all__ = [
    "apply_theme_inline_styles",
    "normalized_theme_choice",
    "resolved_theme_variant",
    "theme_color",
    "theme_colors",
    "theme_stylesheet",
]
