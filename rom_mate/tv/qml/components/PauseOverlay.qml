import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts

Item {
    id: root
    
    property string gameName: ""
    signal resumed()
    signal quitted()

    property int _currentIndex: 0

    width: parent.width
    height: parent.height
    opacity: visible ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 200 } }
    z: 100

    onVisibleChanged: {
        if (visible) {
            _currentIndex = 0
            gameBackend.pauseEmulator()
            forceActiveFocus()
        }
        appBackend.setUiOverlayActive(visible)
    }

    // Modal Background Overlay
    Rectangle {
        anchors.fill: parent
        color: "#CC000000"

        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            onClicked: {} // Block clicks
        }
    }

    // Dialog Panel
    Rectangle {
        width: Math.min(parent.width * 0.4, 500)
        height: panelColumn.implicitHeight + 48
        anchors.centerIn: parent
        color: "#1e1f29"
        radius: 14
        border.color: "#44475a"
        border.width: 1

        Column {
            id: panelColumn
            anchors.centerIn: parent
            width: parent.width - 48
            spacing: 12

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "🎮"
                font.pixelSize: 32
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.gameName
                color: "#f8f8f2"
                font.pixelSize: 18
                font.bold: true
                elide: Text.ElideRight
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Emulator paused"
                color: "#6272a4"
                font.pixelSize: 13
            }

            Rectangle {
                width: parent.width
                height: 1
                color: "#44475a"
            }

            // Resume Button
            Rectangle {
                width: parent.width
                height: 48
                radius: 8
                color: "#ff79c6"
                border.color: root._currentIndex === 0 ? "#f8f8f2" : "transparent"
                border.width: root._currentIndex === 0 ? 2 : 0

                Text {
                    anchors.centerIn: parent
                    text: "▶  Resume"
                    color: "#282a36"
                    font.pixelSize: 16
                    font.bold: true
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        root._currentIndex = 0
                        root.confirmAction()
                    }
                }
            }

            // Quit Button
            Rectangle {
                width: parent.width
                height: 48
                radius: 8
                color: "#383a59"
                border.color: root._currentIndex === 1 ? "#ff79c6" : "#44475a"
                border.width: root._currentIndex === 1 ? 2 : 1

                Text {
                    anchors.centerIn: parent
                    text: "🔇  Quit to TV Mode"
                    color: "#f8f8f2"
                    font.pixelSize: 16
                    font.bold: true
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        root._currentIndex = 1
                        root.confirmAction()
                    }
                }
            }

            Item {
                // Version label placeholder
                width: parent.width
                height: 0
                visible: false
            }
        }
    }

    function confirmAction() {
        if (_currentIndex === 0) {
            root.visible = false
            gameBackend.resumeEmulator()
            root.resumed()
        } else if (_currentIndex === 1) {
            gameBackend.stopGame()
            root.visible = false
            root.quitted()
        }
    }

    Connections {
        target: controllerBackend
        function onNavigationEvent(event) {
            if (!root.visible) return
            if (event === "up") root._currentIndex = Math.max(0, root._currentIndex - 1)
            if (event === "down") root._currentIndex = Math.min(1, root._currentIndex + 1)
            if (event === "back" || event === "guide_button") {
                root.visible = false
                gameBackend.resumeEmulator()
                root.resumed()
            }
            if (event === "confirm") {
                root.confirmAction()
            }
        }
    }

    Keys.onUpPressed: root._currentIndex = Math.max(0, root._currentIndex - 1)
    Keys.onDownPressed: root._currentIndex = Math.min(1, root._currentIndex + 1)
    Keys.onReturnPressed: root.confirmAction()
    Keys.onEscapePressed: {
        root.visible = false
        gameBackend.resumeEmulator()
        root.resumed()
    }
}
