import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "components"
import "views"

ApplicationWindow {
    id: root
    visibility: Window.FullScreen
    title: "Rom Mate — TV Mode"
    color: "#282a36"

    onClosing: function(close) {
        close.accepted = false
        root.hide()
    }

    // Outer stack: full-screen pushes (Details, Settings overlay the tab bar)
    StackView {
        id: outerStack
        anchors.fill: parent
        initialItem: tabRootComponent
        focus: true

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Delete) {
                controllerBackend.emitNavigation("tab_prev")
                event.accepted = true
            } else if (event.key === Qt.Key_End) {
                controllerBackend.emitNavigation("tab_next")
                event.accepted = true
            } else if (event.key === Qt.Key_Escape) {
                if (outerStack.depth > 1) {
                    outerStack.pop()
                }
                event.accepted = true
            }
        }

    }

    PauseOverlay {
        id: pauseOverlay
        anchors.fill: parent
        visible: false
        gameName: gameBackend.activeEmulatorName
        z: 100
        onResumed: pauseOverlay.visible = false
        onQuitted: pauseOverlay.visible = false
    }

    Component { id: detailsViewComponent; DetailsView {} }
    Component { id: settingsViewComponent; SettingsView {} }

    Component {
        id: tabRootComponent

        Item {
            id: tabRoot

            width: parent ? parent.width : 0
            height: parent ? parent.height : 0

            ViewTabBar {
                id: tabBar
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
            }

            StackView {
                id: innerStack
                anchors.top: tabBar.bottom
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                initialItem: homeViewComponent
            }

            Component { id: homeViewComponent;    HomeView    { stackView: innerStack; outerStackRef: outerStack } }
            Component { id: libraryViewComponent; LibraryView { stackView: innerStack; outerStackRef: outerStack } }
            Component { id: serverViewComponent;  ServerView  { stackView: innerStack; outerStackRef: outerStack } }

            Connections {
                target: tabBar
                function onTabSelected(index) {
                    if (index === 0) innerStack.replace(homeViewComponent)
                    else if (index === 1) innerStack.replace(libraryViewComponent)
                    else if (index === 2) innerStack.replace(serverViewComponent)
                }
            }

            // Controller navigation
            Connections {
                target: controllerBackend
                function onNavigationEvent(direction) {
                    if (direction === "tab_prev") {
                        tabBar.selectPrev()
                    } else if (direction === "tab_next") {
                        tabBar.selectNext()
                    } else if (direction === "back") {
                        if (outerStack.depth > 1) {
                            outerStack.pop()
                        }
                    } else if (direction === "guide_button") {
                        if (gameBackend.isSessionActive) {
                            pauseOverlay.gameName = gameBackend.activeEmulatorName
                            pauseOverlay.visible = true
                        } else if (outerStack.depth <= 1) {
                            outerStack.push(settingsViewComponent)
                        }
                    }
                    // directional events are handled by focused items via Keys
                }
            }
        }
    }

    Connections {
        target: gameBackend
        function onPauseRequested() {
            pauseOverlay.gameName = gameBackend.activeEmulatorName
            pauseOverlay.visible = true
        }
        function onSessionEnded(emulatorName) {
            pauseOverlay.visible = false
        }
    }

}

