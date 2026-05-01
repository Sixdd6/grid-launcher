import QtQuick
import QtQuick.Window

Window {
    id: pauseWindow
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "transparent"
    visible: pauseBackend.visible
    x: Screen.virtualX
    y: Screen.virtualY
    width: Screen.width
    height: Screen.height

    property int _currentIndex: 0
    property real _panelScale: pauseBackend.visible ? 1.0 : 0.985
    Behavior on _panelScale { NumberAnimation { duration: 160 } }

    onVisibleChanged: {
        if (visible) {
            opacity = 1.0
            _currentIndex = 0
            keyHandler.forceActiveFocus()
        } else {
            opacity = 0.0
        }
    }

    Timer {
        id: resumeTimer
        interval: 150
        onTriggered: pauseBackend.resumeGame()
    }

    Component.onDestruction: {
        resumeTimer.stop()
    }

    Item {
        id: keyHandler
        anchors.fill: parent
        focus: true

        Keys.onEscapePressed: resumeTimer.start()
        Keys.onReturnPressed: resumeTimer.start()
        Keys.onEnterPressed: resumeTimer.start()
    }

    Rectangle {
        anchors.fill: parent
        color: "#CC000000"
    }

    Rectangle {
        id: panelCard
        width: Math.min(parent.width * 0.42, 560)
        height: cardColumn.implicitHeight + 64
        anchors.centerIn: parent
        radius: 14
        color: "#1e1f29"
        border.color: "#44475a"
        border.width: 1
        scale: _panelScale

        Column {
            id: cardColumn
            anchors.centerIn: parent
            width: parent.width - 72

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: pauseBackend.gameTitle
                color: "#f8f8f2"
                font.pixelSize: 22
                font.bold: true
                elide: Text.ElideRight
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: pauseBackend.emulatorName
                color: "#6272a4"
                font.pixelSize: 15
                topPadding: 4
                bottomPadding: 32
            }

            Repeater {
                model: pauseBackend.actions
                delegate: Rectangle {
                    id: actionDelegate
                    width: parent.width
                    height: 54
                    radius: 8

                    property bool isFocused: pauseWindow._currentIndex === index

                    color: isFocused ? (index === 0 ? "#ff79c6" : "#383a59") : "#383a59"
                    border.color: isFocused ? (index === 0 ? "#f8f8f2" : "#ff5555") : "#44475a"
                    border.width: isFocused ? 2 : 1
                    scale: isFocused ? 1.02 : 1.0

                    Behavior on scale { NumberAnimation { duration: 110 } }
                    Behavior on border.color { ColorAnimation { duration: 110 } }
                    Behavior on color { ColorAnimation { duration: 110 } }

                    Text {
                        anchors.centerIn: parent
                        text: modelData
                        color: isFocused && index === 0 ? "#282a36" : (isFocused && index === 1 ? "#ff5555" : "#f8f8f2")
                        font.pixelSize: 17
                        font.bold: true
                    }
                    
                    Rectangle {
                        width: parent.width; height: 12; color: "transparent"; visible: index < pauseBackend.actions.length - 1
                    }
                }
            }
            
            Item { width: 1; height: 32 }

            Row {
                spacing: 24
                anchors.horizontalCenter: parent.horizontalCenter
                Text { text: "A  Confirm"; color: "#6272a4"; font.pixelSize: 13 }
                Text { text: "B  Back"; color: "#6272a4"; font.pixelSize: 13 }
                Text { text: "Guide  Resume"; color: "#6272a4"; font.pixelSize: 13 }
            }
        }
    }

    function _dispatchAction(index) {
        if (index === 0) resumeTimer.start()
        else if (index === 1) pauseBackend.quitGame()
    }

    Connections {
        target: controllerBackend
        function onPauseNavigationEvent(event) {
            if (event === "up") { if (_currentIndex > 0) _currentIndex-- }
            else if (event === "down") { if (_currentIndex < pauseBackend.actions.length - 1) _currentIndex++ }
            else if (event === "confirm") { _dispatchAction(_currentIndex) }
            else if (event === "back" || event === "guide_button") { pauseBackend.resumeGame() }
        }
    }

}