from __future__ import annotations

from typing import Any, Callable

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QFrame, QHBoxLayout, QLabel, QPushButton, QScrollArea, QSizePolicy, QVBoxLayout, QWidget

from ..library.downloads import display_download_entries, download_entry_action_mode, download_entry_title


def build_downloads_page() -> tuple[QWidget, QLabel, QScrollArea, QVBoxLayout]:
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
    content_layout.addWidget(empty_label)

    scroll = QScrollArea()
    scroll.setObjectName("downloadsScroll")
    scroll.setWidgetResizable(True)
    scroll.setFrameShape(QFrame.Shape.NoFrame)
    scroll.viewport().setObjectName("downloadsScrollViewport")

    scroll_content = QWidget()
    scroll_content.setObjectName("downloadsContent")
    list_layout = QVBoxLayout(scroll_content)
    list_layout.setContentsMargins(0, 0, 0, 0)
    list_layout.setSpacing(10)

    scroll.setWidget(scroll_content)
    scroll.setVisible(False)
    content_layout.addWidget(scroll)

    layout.addWidget(content_frame)
    return page, empty_label, scroll, list_layout


def make_download_entry_widget(
    entry: dict[str, Any],
    detail_text: str,
    muted_color: str,
    on_cancel: Callable[[str], None],
    on_retry: Callable[[str], None],
    on_dismiss: Callable[[str], None],
) -> tuple[QWidget, QLabel]:
    frame = QFrame()
    frame.setObjectName("panel")
    frame_layout = QHBoxLayout(frame)
    frame_layout.setContentsMargins(12, 10, 12, 10)
    frame_layout.setSpacing(10)

    text_col = QVBoxLayout()
    text_col.setSpacing(4)

    title_label = QLabel(download_entry_title(entry))
    title_label.setStyleSheet("font-size: 16px; font-weight: 700;")
    text_col.addWidget(title_label)

    detail_label = QLabel(detail_text)
    detail_label.setWordWrap(True)
    detail_label.setStyleSheet(f"color: {muted_color};")
    text_col.addWidget(detail_label)

    frame_layout.addLayout(text_col, 1)

    actions = QHBoxLayout()
    actions.setSpacing(8)
    entry_id = str(entry.get("id", ""))
    status = str(entry.get("status", ""))
    action_mode = download_entry_action_mode(status)

    if action_mode == "cancel":
        cancel_button = QPushButton("Cancel")
        cancel_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        cancel_button.setEnabled(status != "cancelling")
        cancel_button.clicked.connect(lambda checked=False, target=entry_id: on_cancel(target))
        actions.addWidget(cancel_button)
    elif action_mode == "installing":
        installing_label = QLabel("Installing...")
        installing_label.setStyleSheet(f"color: {muted_color};")
        actions.addWidget(installing_label)
    elif action_mode == "retry-dismiss":
        retry_button = QPushButton("Retry")
        retry_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        retry_button.clicked.connect(lambda checked=False, target=entry_id: on_retry(target))
        actions.addWidget(retry_button)

        cancel_button = QPushButton("Cancel")
        cancel_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        cancel_button.clicked.connect(lambda checked=False, target=entry_id: on_dismiss(target))
        actions.addWidget(cancel_button)
    else:
        dismiss_button = QPushButton("Dismiss")
        dismiss_button.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        dismiss_button.clicked.connect(lambda checked=False, target=entry_id: on_dismiss(target))
        actions.addWidget(dismiss_button)

    frame_layout.addLayout(actions)
    return frame, detail_label


def refresh_downloads_page(
    downloads_list_layout: QVBoxLayout,
    downloads_empty_label: QLabel,
    downloads_scroll: QScrollArea,
    download_entries: list[dict[str, Any]],
    make_widget: Callable[[dict[str, Any]], tuple[QWidget, QLabel]],
) -> dict[str, QLabel]:
    detail_labels: dict[str, QLabel] = {}
    while downloads_list_layout.count() > 0:
        item = downloads_list_layout.takeAt(0)
        widget = item.widget()
        if widget is not None:
            widget.deleteLater()

    has_entries = len(download_entries) > 0
    downloads_empty_label.setVisible(not has_entries)
    downloads_scroll.setVisible(has_entries)
    if not has_entries:
        return detail_labels

    for entry in display_download_entries(download_entries):
        entry_id = str(entry.get("id", ""))
        widget, detail_label = make_widget(entry)
        if entry_id:
            detail_labels[entry_id] = detail_label
        downloads_list_layout.addWidget(widget)
    downloads_list_layout.addStretch()
    return detail_labels
