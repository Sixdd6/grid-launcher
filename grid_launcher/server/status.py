from __future__ import annotations

from typing import Protocol


class StatusLabelProtocol(Protocol):
    def setText(self, text: str) -> None:
        ...

    def setStyleSheet(self, style: str) -> None:
        ...


def apply_server_status(label: StatusLabelProtocol | None, text: str, color: str | None, default_color: str) -> None:
    if label is None:
        return
    resolved_color = color if color is not None else default_color
    label.setText(text)
    label.setStyleSheet(f"color: {resolved_color};")
