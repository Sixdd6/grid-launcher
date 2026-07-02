from __future__ import annotations

from typing import Any

from PySide6.QtCore import Property, QObject, Signal, Slot


class PauseBackend(QObject):
    visibleChanged = Signal()
    gameTitleChanged = Signal()
    emulatorNameChanged = Signal()

    def __init__(self, game_backend: Any, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._game_backend = game_backend
        self._game_title = ""
        self._emulator_name = ""
        self._visible = False

    @Property(bool, notify=visibleChanged)
    def visible(self) -> bool:
        return self._visible

    @Property(str, notify=gameTitleChanged)
    def gameTitle(self) -> str:
        return self._game_title

    @Property(str, notify=emulatorNameChanged)
    def emulatorName(self) -> str:
        return self._emulator_name

    @Property(list, constant=True)
    def actions(self) -> list[str]:
        return ["Resume Game", "Quit to TV Mode"]

    @Slot()
    def openForActiveSession(self) -> None:
        if not self._game_backend.isSessionActive:
            return

        self._game_backend.pauseEmulator()

        self._game_title = self._game_backend.activeGameTitle
        raw_name = self._game_backend.activeEmulatorName
        self._emulator_name = "Native Game" if not raw_name else raw_name
        self.gameTitleChanged.emit()
        self.emulatorNameChanged.emit()

        self._visible = True
        self.visibleChanged.emit()

    @Slot()
    def resumeGame(self) -> None:
        if self._visible:
            self._visible = False
            self.visibleChanged.emit()
            self._game_backend.resumeEmulator()

    @Slot()
    def quitGame(self) -> None:
        self._game_backend.stopGame()

        self._visible = False
        self.visibleChanged.emit()

        self._game_title = ""
        self._emulator_name = ""
        self.gameTitleChanged.emit()
        self.emulatorNameChanged.emit()

    @Slot()
    def dismiss(self) -> None:
        self.resumeGame()

    @Slot()
    def forceClose(self) -> None:
        """Force-clear visible state without resuming the emulator (used during teardown)."""
        if self._visible:
            self._visible = False
            self.visibleChanged.emit()
        self._game_title = ""
        self._emulator_name = ""
        self.gameTitleChanged.emit()
        self.emulatorNameChanged.emit()
