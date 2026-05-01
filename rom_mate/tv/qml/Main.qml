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

    onVisibleChanged: {
        if (visible && outerStack.depth > 1) {
            outerStack.pop(null)
        }
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

    Component { id: detailsViewComponent; DetailsView {} }
    Component { id: settingsViewComponent; SettingsView {} }

    Component {
        id: tabRootComponent

        Item {
            id: tabRoot

            width: parent ? parent.width : 0
            height: parent ? parent.height : 0
            property int _prevTabIndex: 0

            Component.onCompleted: {
                var tab = appBackend.homeViewTab
                var idx = tab === "library" ? 1 : tab === "server" ? 2 : 0
                if (idx !== 0) {
                    tabBar.currentIndex = idx
                    tabRoot._prevTabIndex = idx
                    if (idx === 1) innerStack.replace(libraryViewComponent)
                    else if (idx === 2) innerStack.replace(serverViewComponent)
                }
            }

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

                // slideDir: 1 = new view enters from right (navigating to higher index)
                //           -1 = new view enters from left (navigating to lower index)
                property int slideDir: 1

                replaceEnter: Transition {
                    NumberAnimation {
                        property: "x"
                        from: innerStack.slideDir * innerStack.width
                        to: 0
                        duration: 200
                        easing.type: Easing.OutCubic
                    }
                }

                replaceExit: Transition {
                    NumberAnimation {
                        property: "x"
                        from: 0
                        to: -innerStack.slideDir * innerStack.width
                        duration: 200
                        easing.type: Easing.OutCubic
                    }
                }
            }

            Component { id: homeViewComponent;    HomeView    { stackView: innerStack; outerStackRef: outerStack } }
            Component { id: libraryViewComponent; LibraryView { stackView: innerStack; outerStackRef: outerStack } }
            Component { id: serverViewComponent;  ServerView  { stackView: innerStack; outerStackRef: outerStack } }

            Connections {
                target: tabBar
                function onTabSelected(index) {
                    innerStack.slideDir = index > tabRoot._prevTabIndex ? 1 : -1
                    tabRoot._prevTabIndex = index
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
                        Qt.callLater(function() {
                            if (outerStack.depth > 1 && !appBackend.uiOverlayActive && !pauseBackend.visible) {
                                outerStack.pop()
                            }
                        })
                    } else if (direction === "guide_button") {
                        if (!gameBackend.isSessionActive && outerStack.depth <= 1) {
                            outerStack.push(settingsViewComponent)
                        }
                    }
                    // directional events are handled by focused items via Keys
                }
            }
        }
    }

}
