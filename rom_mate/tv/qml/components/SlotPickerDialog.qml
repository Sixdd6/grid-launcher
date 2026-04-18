import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts

Item {
    id: root
    
    property var game: ({})
    property string saveType: "save"
    signal closed()

    property var _slots: []
    property bool _loading: false
    property string _errorText: ""

    opacity: visible ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 150 } }

    onVisibleChanged: {
        if (visible) {
            appBackend.setUiOverlayActive(true)
            _loading = true
            _slots = []
            _errorText = ""
            cloudBackend.loadSlotsForGame(root.game, root.saveType)
            forceActiveFocus()
        } else {
            Qt.callLater(function() { appBackend.setUiOverlayActive(false) })
        }
    }

    Connections {
        target: cloudBackend
        
        function onSlotsLoaded(saveType, slots) {
            if (saveType !== root.saveType) return
            _loading = false
            _slots = slots
        }
        
        function onSlotsError(saveType, error) {
            if (saveType !== root.saveType) return
            _loading = false
            _errorText = error
        }
        
        function onRestoreComplete(success, message) {
            root.closed()
        }
        
        function onDeleteComplete(success, message) {
            if (success) {
                cloudBackend.loadSlotsForGame(root.game, root.saveType)
            }
        }
    }

    Connections {
        target: controllerBackend
        function onNavigationEvent(event) {
            if (!root.visible) return
            if (event === "back") root.closed()
            if (event === "up") listView.decrementCurrentIndex()
            if (event === "down") listView.incrementCurrentIndex()
            if (event === "confirm") {
                var slot = _slots[listView.currentIndex]
                if (slot) cloudBackend.restoreSlot(root.game, slot.id, root.saveType)
            }
        }
    }

    Keys.onEscapePressed: root.closed()
    Keys.onUpPressed: listView.decrementCurrentIndex()
    Keys.onDownPressed: listView.incrementCurrentIndex()
    Keys.onReturnPressed: {
        var slot = _slots[listView.currentIndex]
        if (slot) cloudBackend.restoreSlot(root.game, slot.id, root.saveType)
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
        width: Math.min(parent.width * 0.6, 700)
        height: Math.min(parent.height * 0.75, 600)
        anchors.centerIn: parent
        color: "#1e1f29"
        radius: 12
        border.color: "#44475a"
        border.width: 1

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            // Header
            Item {
                Layout.fillWidth: true
                height: 80

                Column {
                    anchors.centerIn: parent
                    spacing: 4

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "☁ Cloud Saves"
                        color: "#f8f8f2"
                        font.pixelSize: 22
                        font.bold: true
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: game.title || game.name || ""
                        color: "#6272a4"
                        font.pixelSize: 14
                    }
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: "#44475a"
                }
            }

            // Body
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                BusyIndicator {
                    anchors.centerIn: parent
                    visible: _loading
                }

                Text {
                    anchors.centerIn: parent
                    text: _errorText
                    color: "#ff5555"
                    font.pixelSize: 16
                    visible: _errorText !== ""
                }

                Text {
                    anchors.centerIn: parent
                    text: "No cloud saves found."
                    color: "#6272a4"
                    font.pixelSize: 18
                    visible: !_loading && _errorText === "" && _slots.length === 0
                }

                ListView {
                    id: listView
                    anchors.fill: parent
                    anchors.margins: 10
                    visible: !_loading && _errorText === "" && _slots.length > 0
                    model: _slots.length
                    clip: true
                    spacing: 4

                    delegate: Rectangle {
                        id: rowRect
                        width: listView.width
                        height: 52
                        color: listView.currentIndex === index || ma.containsMouse ? "#383a59" : "transparent"
                        radius: 6

                        property var slotData: _slots[index]

                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: 16
                            spacing: 4

                            Text {
                                text: slotData.name || slotData.id || "Slot"
                                color: "#f8f8f2"
                                font.pixelSize: 14
                                font.bold: true
                            }

                            Text {
                                text: (slotData.emulator || "Unknown") + "    " + (slotData.timestamp || "")
                                color: "#6272a4"
                                font.pixelSize: 12
                            }
                        }

                        // Delete Box
                        Rectangle {
                            id: deleteBtn
                            anchors.right: parent.right
                            anchors.rightMargin: 16
                            anchors.verticalCenter: parent.verticalCenter
                            width: 28
                            height: 28
                            radius: 4
                            color: "#ff5555"
                            visible: listView.currentIndex === index
                            
                            Text {
                                anchors.centerIn: parent
                                text: "✕"
                                color: "#f8f8f2"
                                font.pixelSize: 14
                                font.bold: true
                            }

                            MouseArea {
                                anchors.fill: parent
                                onClicked: cloudBackend.deleteSlot(slotData.id, root.saveType)
                            }
                        }

                        MouseArea {
                            id: ma
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: {
                                listView.currentIndex = index
                                cloudBackend.restoreSlot(root.game, slotData.id, root.saveType)
                            }
                        }

                        Keys.onReturnPressed: {
                            cloudBackend.restoreSlot(root.game, slotData.id, root.saveType)
                        }

                        Keys.onPressed: (event) => {
                            if (event.key === Qt.Key_Backspace || event.key === Qt.Key_Delete) {
                                cloudBackend.deleteSlot(slotData.id, root.saveType)
                                event.accepted = true
                            }
                        }
                    }
                }
            }

            // Footer
            Item {
                Layout.fillWidth: true
                height: 60

                Rectangle {
                    anchors.top: parent.top
                    width: parent.width
                    height: 1
                    color: "#44475a"
                }

                Rectangle {
                    anchors.centerIn: parent
                    width: 140
                    height: 36
                    border.color: "#6272a4"
                    border.width: 1
                    color: "transparent"
                    radius: 6

                    Text {
                        anchors.centerIn: parent
                        text: "Cancel"
                        color: "#f8f8f2"
                        font.pixelSize: 16
                    }

                    MouseArea {
                        anchors.fill: parent
                        onClicked: root.closed()
                    }
                }
            }
        }
    }
}