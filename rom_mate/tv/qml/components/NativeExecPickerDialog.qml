import QtQuick 2.15
import QtQuick.Controls 2.15

Item {
    id: root
    
    property string romId: ""
    property var candidates: []
    property string currentPath: ""
    property int _cursorIndex: 0
    property int _selectedIndex: -1
    property bool _readyForInput: false
    signal closed()

    opacity: visible ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 150 } }

    onVisibleChanged: {
        if (visible) {
            root._readyForInput = false
            root._cursorIndex = 0
            
            var found = -1
            if (currentPath !== "") {
                for (var i = 0; i < root.candidates.length; i++) {
                    if (root.candidates[i].path === currentPath) {
                        found = i
                        break
                    }
                }
            }
            if (found >= 0) {
                root._selectedIndex = found
            } else if (root.candidates.length > 0) {
                root._selectedIndex = 0
            } else {
                root._selectedIndex = -1
            }

            forceActiveFocus()
            appBackend.setUiOverlayActive(true)
            Qt.callLater(function() { root._readyForInput = true })
        } else {
            root._readyForInput = false
            Qt.callLater(function() { appBackend.setUiOverlayActive(false) })
        }
    }

    function _selectCurrent() {
        if (root.candidates.length === 0) return
        var candidate = root.candidates[root._cursorIndex]
        if (!candidate) return
        root._selectedIndex = root._cursorIndex
        gameBackend.saveNativeExecutable(root.romId, candidate.path)
    }

    Rectangle {
        anchors.fill: parent
        color: "#CC000000"
        
        MouseArea {
            anchors.fill: parent
        }
    }

    Rectangle {
        width: Math.min(parent.width * 0.55, 640)
        height: Math.min(parent.height * 0.7, 520)
        anchors.centerIn: parent
        color: "#1e1f29"
        radius: 12
        border.color: "#44475a"
        border.width: 1

        Column {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 16

            Text {
                text: "Game Executable"
                color: "#f8f8f2"
                font.pixelSize: 20
                font.bold: true
            }

            Text {
                text: "Select the executable to use when launching"
                color: "#6272a4"
                font.pixelSize: 13
            }

            Rectangle {
                width: parent.width
                height: parent.height - 160
                color: "#282a36"
                radius: 6

                ListView {
                    anchors.fill: parent
                    model: root.candidates
                    clip: true

                    delegate: Rectangle {
                        width: parent.width
                        height: 52
                        color: index === root._cursorIndex ? "#383a59" : "transparent"
                        radius: 4
                        border.color: index === root._cursorIndex ? "#ff79c6" : "transparent"
                        border.width: index === root._cursorIndex ? 2 : 0
                        Behavior on border.color { ColorAnimation { duration: 60 } }

                        Rectangle {
                            id: checkbox
                            width: 20
                            height: 20
                            radius: 10
                            anchors.left: parent.left
                            anchors.leftMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            color: index === root._selectedIndex ? "#ff79c6" : "transparent"
                            border.color: index === root._selectedIndex ? "#ff79c6" : "#44475a"
                            border.width: index === root._selectedIndex ? 0 : 2

                            Text {
                                text: "✓"
                                color: "#ffffff"
                                font.pixelSize: 14
                                font.bold: true
                                anchors.centerIn: parent
                                visible: index === root._selectedIndex
                            }
                        }

                        Text {
                            text: modelData.label
                            color: "#f8f8f2"
                            font.pixelSize: 14
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: checkbox.right
                            anchors.leftMargin: 10
                            anchors.right: parent.right
                            anchors.rightMargin: 12
                            elide: Text.ElideMiddle
                        }
                    }
                }
            }

            Rectangle {
                width: parent.width
                height: 44
                color: "transparent"
                border.color: "#6272a4"
                border.width: 1
                radius: 8

                Text {
                    anchors.centerIn: parent
                    text: "Close"
                    color: "#6272a4"
                    font.pixelSize: 14
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: root.closed()
                }
            }
        }
    }

    Connections {
        target: controllerBackend
        function onNavigationEvent(direction) {
            if (!root.visible || !root._readyForInput) return
            if (direction === "back") { root.closed(); return }
            if (direction === "up") root._cursorIndex = Math.max(0, root._cursorIndex - 1)
            if (direction === "down") root._cursorIndex = Math.min(root.candidates.length - 1, root._cursorIndex + 1)
            if (direction === "confirm") root._selectCurrent()
        }
    }

    Keys.onUpPressed: { if (!root._readyForInput) return; root._cursorIndex = Math.max(0, root._cursorIndex - 1) }
    Keys.onDownPressed: { if (!root._readyForInput) return; root._cursorIndex = Math.min(root.candidates.length - 1, root._cursorIndex + 1) }
    Keys.onReturnPressed: { if (!root._readyForInput) return; root._selectCurrent() }
    Keys.onEnterPressed: { if (!root._readyForInput) return; root._selectCurrent() }
    Keys.onEscapePressed: { if (!root._readyForInput) return; root.closed() }
}
