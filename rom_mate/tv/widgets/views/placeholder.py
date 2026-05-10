from PySide6.QtCore import Qt
from PySide6.QtWidgets import QLabel, QVBoxLayout, QWidget

from rom_mate.tv.widgets import theme


class PlaceholderView(QWidget):
    def __init__(self, label: str = "Coming Soon", parent=None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        lbl = QLabel(label)
        lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
        lbl.setStyleSheet(f"color: {theme.TEXT_SECONDARY}; font-size: 18px;")
        layout.addWidget(lbl)
        self.setStyleSheet(f"background: {theme.BG};")
