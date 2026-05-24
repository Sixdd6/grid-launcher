from __future__ import annotations

from PySide6.QtCore import QEasingCurve, QParallelAnimationGroup, QPropertyAnimation, QRect, Qt
from PySide6.QtGui import QKeyEvent, QPixmap
from PySide6.QtWidgets import QLabel, QStackedWidget, QVBoxLayout, QWidget

from rom_mate.tv.widgets import theme
from rom_mate.tv.widgets.components.controls_bar import ControlsBar
from rom_mate.tv.widgets.pause_window import PauseWindow
from rom_mate.tv.widgets.tab_bar import ViewTabBar
from rom_mate.tv.widgets.views.home_view import HomeView
from rom_mate.tv.widgets.views.library_view import LibraryView
from rom_mate.tv.widgets.views.settings_view import SettingsView
from rom_mate.tv.widgets.views.server_view import ServerView


class TVWindow(QWidget):
    def __init__(
        self,
        app_backend,
        cloud_backend,
        game_backend,
        pause_backend,
        controller_backend,
        cover_loader,
        parent=None,
    ):
        super().__init__(parent)
        self._app_backend = app_backend
        self._cloud_backend = cloud_backend
        self._game_backend = game_backend
        self._pause_backend = pause_backend
        self._controller_backend = controller_backend
        self._cover_loader = cover_loader
        self._pause_window = None
        self._pause_geometry_initialized = False
        self._slide_anim: QParallelAnimationGroup | None = None
        self._transition_overlay: QWidget | None = None
        self._tab_slide_anim: QParallelAnimationGroup | None = None
        self._tab_transition_overlay: QWidget | None = None

        self.setStyleSheet(f"background: {theme.BG};")

        root_layout = QVBoxLayout(self)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        self._outer_stack = QStackedWidget(self)
        root_layout.addWidget(self._outer_stack)

        root_widget = QWidget(self._outer_stack)
        root_inner_layout = QVBoxLayout(root_widget)
        root_inner_layout.setContentsMargins(0, 0, 0, 0)
        root_inner_layout.setSpacing(0)

        self._tab_bar = ViewTabBar(root_widget)
        root_inner_layout.addWidget(self._tab_bar)

        self._inner_stack = QStackedWidget(root_widget)
        root_inner_layout.addWidget(self._inner_stack)

        self._home_view = HomeView(
            self._app_backend,
            self._cloud_backend,
            self._game_backend,
            self._pause_backend,
            self._controller_backend,
            self._cover_loader,
            self.push_view,
            self.pop_view,
            parent=self._inner_stack,
        )
        self._library_view = LibraryView(
            self._app_backend,
            self._cloud_backend,
            self._game_backend,
            self._pause_backend,
            self._controller_backend,
            self._cover_loader,
            self.push_view,
            self.pop_view,
            parent=self._inner_stack,
        )
        self._server_view = ServerView(
            self._app_backend,
            self._cloud_backend,
            self._game_backend,
            self._pause_backend,
            self._controller_backend,
            self._cover_loader,
            self.push_view,
            self.pop_view,
            parent=self._inner_stack,
        )
        self._inner_stack.addWidget(self._home_view)
        self._inner_stack.addWidget(self._library_view)
        self._inner_stack.addWidget(self._server_view)

        self._outer_stack.addWidget(root_widget)
        self._outer_stack.setCurrentIndex(0)

        self._controls_bar = ControlsBar(self)
        root_layout.addWidget(self._controls_bar)

        self._tab_bar.tabChanged.connect(self._on_tab_changed)
        self._controller_backend.navigationEvent.connect(self._on_nav_event)
        self._controller_backend.pauseNavigationEvent.connect(self._on_nav_event)

        self._pause_window = PauseWindow(self._pause_backend, parent=self)
        self._pause_backend.visibleChanged.connect(self._update_controls_bar)

        self._update_controls_bar()

    def _on_tab_changed(self, index: object) -> None:
        try:
            idx = int(index)
        except (TypeError, ValueError):
            return
        old_idx = self._inner_stack.currentIndex()
        forward = idx > old_idx
        from_pixmap = self._capture_widget_pixmap(self._inner_stack.currentWidget())
        self._inner_stack.setCurrentIndex(idx)
        if idx == 0:
            self._home_view.focus_default_row()
        if idx == 1:
            self._library_view.activate()
        if idx == 2:
            self._server_view.activate()
        to_pixmap = self._capture_widget_pixmap(self._inner_stack.currentWidget())
        if idx != old_idx:
            self._start_tab_slide_transition(from_pixmap, to_pixmap, forward=forward)
        self._update_controls_bar()

    def _on_nav_event(self, direction: object) -> None:
        if self._pause_window and self._pause_window.isVisible():
            self._pause_window.handle_nav(str(direction))
            return

        if direction == "tab_prev":
            self._tab_bar.set_current_index(max(0, self._tab_bar.current_index - 1))
            return
        if direction == "tab_next":
            self._tab_bar.set_current_index(min(2, self._tab_bar.current_index + 1))
            return
        if direction == "guide_button":
            if self._outer_stack.currentIndex() <= 0 and not bool(getattr(self._game_backend, "isSessionActive", False)):
                self.push_view(SettingsView(self._app_backend, self.pop_view, parent=self))
            return
        if direction == "back" and self._outer_stack.currentIndex() > 0:
            current_view = self.get_current_view()
            intercepts = getattr(current_view, "intercepts_back", None)
            if callable(intercepts) and intercepts():
                handler = getattr(current_view, "handle_nav", None)
                if callable(handler):
                    handler(direction)
                return
            self.pop_view()
            return

        current_view = self.get_current_view()
        handler = getattr(current_view, "handle_nav", None)
        if callable(handler):
            handler(direction)

    def push_view(self, widget: QWidget) -> None:
        previous = self._outer_stack.currentWidget()
        previous_pixmap = self._capture_widget_pixmap(previous)

        self._outer_stack.addWidget(widget)
        self._outer_stack.setCurrentWidget(widget)
        self.setFocus()

        sig = getattr(widget, "controlHintsChanged", None)
        if sig is not None:
            sig.connect(self._update_controls_bar)

        target_pixmap = self._capture_widget_pixmap(widget)
        self._start_slide_transition(previous_pixmap, target_pixmap, forward=True)
        self._update_controls_bar()

    def pop_view(self) -> None:
        if self._outer_stack.currentIndex() <= 0:
            return

        current = self._outer_stack.currentWidget()
        current_pixmap = self._capture_widget_pixmap(current)

        self._outer_stack.setCurrentIndex(0)
        target = self._outer_stack.currentWidget()
        target_pixmap = self._capture_widget_pixmap(target)

        if current is not None:
            self._outer_stack.removeWidget(current)
            current.setParent(None)

        self._start_slide_transition(current_pixmap, target_pixmap, forward=False)
        self.setFocus()
        self._update_controls_bar()

    def _capture_widget_pixmap(self, widget: QWidget | None) -> QPixmap | None:
        if widget is None:
            return None
        pixmap = widget.grab()
        if pixmap.isNull():
            return None
        return pixmap

    def _clear_transition_overlay(self) -> None:
        if self._slide_anim is not None and self._slide_anim.state() == QPropertyAnimation.State.Running:
            self._slide_anim.stop()
        self._slide_anim = None
        if self._transition_overlay is not None:
            self._transition_overlay.deleteLater()
            self._transition_overlay = None

    def _clear_tab_transition_overlay(self) -> None:
        if self._tab_slide_anim is not None and self._tab_slide_anim.state() == QPropertyAnimation.State.Running:
            self._tab_slide_anim.stop()
        self._tab_slide_anim = None
        if self._tab_transition_overlay is not None:
            self._tab_transition_overlay.deleteLater()
            self._tab_transition_overlay = None

    def _start_slide_transition(self, from_pixmap: QPixmap | None, to_pixmap: QPixmap | None, forward: bool) -> None:
        if from_pixmap is None or to_pixmap is None:
            return

        width = self._outer_stack.width()
        height = self._outer_stack.height()
        if width <= 0 or height <= 0:
            return

        self._clear_transition_overlay()

        overlay = QWidget(self._outer_stack)
        overlay.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        overlay.setGeometry(self._outer_stack.rect())

        from_label = QLabel(overlay)
        from_label.setGeometry(0, 0, width, height)
        from_label.setPixmap(
            from_pixmap.scaled(
                width,
                height,
                Qt.AspectRatioMode.IgnoreAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            )
        )

        to_start_x = width if forward else -width
        to_end_x = 0
        from_end_x = -width if forward else width

        to_label = QLabel(overlay)
        to_label.setGeometry(to_start_x, 0, width, height)
        to_label.setPixmap(
            to_pixmap.scaled(
                width,
                height,
                Qt.AspectRatioMode.IgnoreAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            )
        )

        overlay.show()
        overlay.raise_()

        anim_from = QPropertyAnimation(from_label, b"geometry", self)
        anim_from.setDuration(200)
        anim_from.setStartValue(QRect(0, 0, width, height))
        anim_from.setEndValue(QRect(from_end_x, 0, width, height))
        anim_from.setEasingCurve(QEasingCurve.Type.OutCubic)

        anim_to = QPropertyAnimation(to_label, b"geometry", self)
        anim_to.setDuration(200)
        anim_to.setStartValue(QRect(to_start_x, 0, width, height))
        anim_to.setEndValue(QRect(to_end_x, 0, width, height))
        anim_to.setEasingCurve(QEasingCurve.Type.OutCubic)

        group = QParallelAnimationGroup(self)
        group.addAnimation(anim_from)
        group.addAnimation(anim_to)

        def _finish() -> None:
            if self._transition_overlay is overlay:
                self._transition_overlay.deleteLater()
                self._transition_overlay = None
            self._slide_anim = None

        group.finished.connect(_finish)
        self._transition_overlay = overlay
        self._slide_anim = group
        self._slide_anim.start()

    def _start_tab_slide_transition(self, from_pixmap: QPixmap | None, to_pixmap: QPixmap | None, forward: bool) -> None:
        if from_pixmap is None or to_pixmap is None:
            return

        width = self._inner_stack.width()
        height = self._inner_stack.height()
        if width <= 0 or height <= 0:
            return

        self._clear_tab_transition_overlay()

        overlay = QWidget(self._inner_stack)
        overlay.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents, True)
        overlay.setGeometry(self._inner_stack.rect())

        from_label = QLabel(overlay)
        from_label.setGeometry(0, 0, width, height)
        from_label.setPixmap(
            from_pixmap.scaled(
                width,
                height,
                Qt.AspectRatioMode.IgnoreAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            )
        )

        to_start_x = width if forward else -width
        to_end_x = 0
        from_end_x = -width if forward else width

        to_label = QLabel(overlay)
        to_label.setGeometry(to_start_x, 0, width, height)
        to_label.setPixmap(
            to_pixmap.scaled(
                width,
                height,
                Qt.AspectRatioMode.IgnoreAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            )
        )

        overlay.show()
        overlay.raise_()

        anim_from = QPropertyAnimation(from_label, b"geometry", self)
        anim_from.setDuration(200)
        anim_from.setStartValue(QRect(0, 0, width, height))
        anim_from.setEndValue(QRect(from_end_x, 0, width, height))
        anim_from.setEasingCurve(QEasingCurve.Type.OutCubic)

        anim_to = QPropertyAnimation(to_label, b"geometry", self)
        anim_to.setDuration(200)
        anim_to.setStartValue(QRect(to_start_x, 0, width, height))
        anim_to.setEndValue(QRect(to_end_x, 0, width, height))
        anim_to.setEasingCurve(QEasingCurve.Type.OutCubic)

        group = QParallelAnimationGroup(self)
        group.addAnimation(anim_from)
        group.addAnimation(anim_to)

        def _finish() -> None:
            if self._tab_transition_overlay is overlay:
                self._tab_transition_overlay.deleteLater()
                self._tab_transition_overlay = None
            self._tab_slide_anim = None

        group.finished.connect(_finish)
        self._tab_transition_overlay = overlay
        self._tab_slide_anim = group
        self._tab_slide_anim.start()

    def get_current_view(self) -> QWidget:
        if self._outer_stack.currentIndex() > 0:
            return self._outer_stack.currentWidget()
        return self._inner_stack.currentWidget()

    def _update_controls_bar(self) -> None:
        """Refresh the controls bar to match the currently active view/state."""
        if self._pause_window is not None and self._pause_window.isVisible():
            hints = getattr(PauseWindow, "CONTROL_HINTS", [])
        else:
            hints = getattr(self.get_current_view(), "CONTROL_HINTS", [])
        self._controls_bar.update_hints(hints)

    def showEvent(self, event) -> None:
        super().showEvent(event)
        if self._pause_window is None:
            return
        screen = self.screen()
        if screen is None:
            return
        if not self._pause_geometry_initialized:
            self._pause_window.setGeometry(screen.geometry())
            self._pause_geometry_initialized = True
            return
        self._pause_window.setGeometry(screen.geometry())

    def keyPressEvent(self, event: QKeyEvent) -> None:
        key = event.key()

        if key == Qt.Key.Key_End:
            self._on_nav_event("tab_prev")
            return
        if key == Qt.Key.Key_PageDown:
            self._on_nav_event("tab_next")
            return
        if key == Qt.Key.Key_Escape:
            self._on_nav_event("guide_button")
            return
        if key == Qt.Key.Key_Backspace:
            self._on_nav_event("back")
            return
        if key == Qt.Key.Key_Up:
            self._on_nav_event("up")
            return
        if key == Qt.Key.Key_Down:
            self._on_nav_event("down")
            return
        if key == Qt.Key.Key_Left:
            self._on_nav_event("left")
            return
        if key == Qt.Key.Key_Right:
            self._on_nav_event("right")
            return
        if key in (Qt.Key.Key_Return, Qt.Key.Key_Enter):
            self._on_nav_event("confirm")
            return

        super().keyPressEvent(event)
