from __future__ import annotations

from pathlib import Path
from typing import NamedTuple

from PySide6.QtCore import Qt
from PySide6.QtGui import QPixmap
from PySide6.QtWidgets import (
    QHBoxLayout,
    QLabel,
    QSizePolicy,
    QWidget,
)

from rom_mate.tv.widgets import theme

_ASSETS = Path(__file__).resolve().parents[4] / "assets" / "retroarch-assets"
_ICON_SIZE = 20


class ControlHint(NamedTuple):
    """A single control binding shown in the controls bar.

    Attributes:
        label: Human-readable action name, e.g. ``"Confirm"``.
        gamepad_icon: File stem in ``assets/retroarch-assets``,
            e.g. ``"input_BTN-D"``, or ``None`` to omit the icon.
        kbd_key: Keyboard key badge text, e.g. ``"Enter"``, or ``None``.
    """

    label: str
    gamepad_icon: str | None
    kbd_key: str | None


class ControlsBar(QWidget):
    """Horizontal hint strip shown at the bottom of the TV window.

    Call :meth:`update_hints` to replace the displayed set of hints whenever
    the active view changes.
    """

    HEIGHT = 44

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setFixedHeight(self.HEIGHT)
        self.setStyleSheet(f"background: {theme.PANEL};")

        outer = QHBoxLayout(self)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)

        # Top border line
        border = QWidget(self)
        border.setFixedHeight(1)
        border.setStyleSheet(f"background: {theme.BORDER_INACTIVE};")

        # Inner content row
        self._inner = QWidget(self)
        self._row = QHBoxLayout(self._inner)
        self._row.setContentsMargins(20, 0, 20, 0)
        self._row.setSpacing(0)
        self._row.addStretch()

        from PySide6.QtWidgets import QVBoxLayout

        vbox = QVBoxLayout()
        vbox.setContentsMargins(0, 0, 0, 0)
        vbox.setSpacing(0)
        vbox.addWidget(border)
        vbox.addWidget(self._inner)
        outer.addLayout(vbox)

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def update_hints(self, hints: list[ControlHint]) -> None:
        """Replace the currently displayed hints."""
        while self._row.count():
            item = self._row.takeAt(0)
            w = item.widget()
            if w is not None:
                w.deleteLater()

        self._row.addStretch()
        for i, hint in enumerate(hints):
            if i > 0:
                sep = QLabel("·", self._inner)
                sep.setStyleSheet(
                    f"color: {theme.BORDER_INACTIVE}; font-size: 14px;"
                )
                sep.setContentsMargins(14, 0, 14, 0)
                self._row.addWidget(sep)
            self._row.addWidget(self._make_hint_widget(hint))
        self._row.addStretch()

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _make_hint_widget(self, hint: ControlHint) -> QWidget:
        container = QWidget(self._inner)
        layout = QHBoxLayout(container)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(5)

        if hint.gamepad_icon is not None:
            icon_path = _ASSETS / f"{hint.gamepad_icon}.png"
            icon_lbl = QLabel(container)
            icon_lbl.setFixedSize(_ICON_SIZE, _ICON_SIZE)
            icon_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
            if icon_path.exists():
                pix = QPixmap(str(icon_path)).scaled(
                    _ICON_SIZE,
                    _ICON_SIZE,
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.SmoothTransformation,
                )
                icon_lbl.setPixmap(pix)
            layout.addWidget(icon_lbl)

        if hint.kbd_key is not None:
            kbd_lbl = QLabel(hint.kbd_key, container)
            kbd_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
            kbd_lbl.setStyleSheet(
                f"color: {theme.TEXT_PRIMARY};"
                f"background: {theme.TERTIARY};"
                f"border: 1px solid {theme.BORDER_INACTIVE};"
                f"border-radius: 4px;"
                f"font-size: 10px;"
                f"padding: 1px 6px;"
            )
            kbd_lbl.setSizePolicy(
                QSizePolicy.Policy.Minimum, QSizePolicy.Policy.Fixed
            )
            layout.addWidget(kbd_lbl)

        action_lbl = QLabel(hint.label, container)
        action_lbl.setStyleSheet(
            f"color: {theme.TEXT_SECONDARY}; font-size: 12px;"
        )
        layout.addWidget(action_lbl)

        return container
