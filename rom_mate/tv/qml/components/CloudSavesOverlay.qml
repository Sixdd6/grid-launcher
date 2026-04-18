import QtQuick 2.15
import QtQuick.Controls 2.15

Item {
    id: root

    property var game: null
    property string saveType: "save"
    property var _slots: []
    property bool _loading: false
    property string _errorText: ""
    property string _statusText: ""
    property bool _statusSuccess: true
    property bool _uploading: false
    property int _currentIndex: 0
    property int _actionMode: 0

    property var _currentSlot: _currentIndex > 0 && (_currentIndex - 1) < _slots.length ? _slots[_currentIndex - 1] : null

    signal closed()

    z: 10

    onVisibleChanged: {
        if (visible) {
            appBackend.setUiOverlayActive(true)
            _currentIndex = 0
            _actionMode = 0
            _loading = true
            _slots = []
            _errorText = ""
            _statusText = ""
            _uploading = false
            cloudBackend.loadSlotsForGame(root.game, root.saveType)
        } else {
            Qt.callLater(function() { appBackend.setUiOverlayActive(false) })
        }
    }

    Timer {
        id: statusTimer
        interval: 4000
        onTriggered: root._statusText = ""
    }

    Connections {
        target: controllerBackend
        function onNavigationEvent(event) {
            if (!root.visible) return

            if (event === "back" || event === "guide_button") {
                root.visible = false
                root.closed()
                return
            }

            if (event === "up") {
                if (_currentIndex > 0) {
                    _currentIndex--
                }
                _actionMode = 0
            } else if (event === "down") {
                if (_currentIndex < _slots.length) {
                    _currentIndex++
                }
                _actionMode = 0
            } else if (event === "left") {
                if (_currentIndex > 0 && _actionMode > 0) {
                    _actionMode--
                }
            } else if (event === "right") {
                if (_currentIndex > 0) {
                    if (_actionMode === 0) _actionMode = 1
                    else if (_actionMode === 1) _actionMode = 2
                    else if (_actionMode === 2) _actionMode = 1
                }
            } else if (event === "confirm") {
                if (_currentIndex === 0 && !_uploading) {
                    _uploading = true
                    cloudBackend.uploadSave(root.game, root.saveType)
                } else if (_currentIndex > 0 && root._currentSlot) {
                    if (_actionMode === 1) {
                        cloudBackend.restoreSlot(root.game, root._currentSlot.id, root.saveType)
                    } else if (_actionMode === 2) {
                        cloudBackend.deleteSlot(root._currentSlot.id, root.saveType)
                    } else if (_actionMode === 0) {
                        _actionMode = 1
                    }
                }
            }
        }
    }

    Connections {
        target: cloudBackend

        function onSlotsLoaded(saveType, slots) {
            if (saveType !== root.saveType) return
            _slots = slots
            _loading = false
            _errorText = ""
        }

        function onSlotsError(saveType, error) {
            if (saveType !== root.saveType) return
            _loading = false
            _errorText = error
        }

        function onRestoreComplete(success, message) {
            _statusText = message
            _statusSuccess = success
            statusTimer.restart()
            if (success) {
                _loading = true
                cloudBackend.loadSlotsForGame(root.game, root.saveType)
            }
        }

        function onDeleteComplete(success, message) {
            _statusText = message
            _statusSuccess = success
            statusTimer.restart()
            if (success) {
                _loading = true
                cloudBackend.loadSlotsForGame(root.game, root.saveType)
            }
        }

        function onUploadComplete(success, message) {
            _uploading = false
            _statusText = message
            _statusSuccess = success
            statusTimer.restart()
            if (success) {
                _loading = true
                cloudBackend.loadSlotsForGame(root.game, root.saveType)
            }
        }
    }

    // Full screen blocker
    Rectangle {
        anchors.fill: parent
        color: "#CC000000"

        MouseArea {
            anchors.fill: parent
            hoverEnabled: true // Block clicks
        }
    }

    // Centered panel
    Rectangle {
        width: 700
        height: 560
        anchors.centerIn: parent
        color: "#1e1f29"
        radius: 12
        border.color: "#44475a"
        border.width: 1

        // Fixed top section
        Column {
            id: topSection
            width: parent.width
            spacing: 0

            // HEADER
            Rectangle {
                width: parent.width
                height: 56
                color: "#282a36"
                radius: 12

                Rectangle {
                    width: parent.width
                    height: 12
                    anchors.bottom: parent.bottom
                    color: "#282a36"
                }

                Row {
                    anchors.centerIn: parent
                    spacing: 12

                    Text {
                        text: "☁  Cloud Saves"
                        color: "#bd93f9"
                        font.pixelSize: 18
                        font.bold: true
                    }

                    Text {
                        text: root.game ? (root.game.name || root.game.fs_name || "") : ""
                        color: "#6272a4"
                        font.pixelSize: 13
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
            }

            // STATUS BANNER
            Rectangle {
                width: parent.width
                height: 36
                visible: _statusText !== ""
                color: _statusSuccess ? "#2650fa7b" : "#26ff5555"

                Text {
                    anchors.centerIn: parent
                    text: _statusText
                    color: _statusSuccess ? "#50fa7b" : "#ff5555"
                    font.pixelSize: 13
                }
            }

            // ERROR TEXT
            Text {
                width: parent.width
                text: _errorText
                color: "#ff5555"
                visible: _errorText !== ""
                padding: 12
                wrapMode: Text.WordWrap
            }

            // UPLOAD BUTTON ROW
            Item {
                width: parent.width
                height: 76

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: 12
                    color: (_currentIndex === 0 && !_uploading) ? "#383a59" : "transparent"
                    radius: 6
                    border.color: _currentIndex === 0 ? "#ff79c6" : "transparent"
                    border.width: 2

                    Row {
                        anchors.centerIn: parent
                        spacing: 8

                        Text {
                            text: _uploading ? "⬆  Uploading..." : "⬆  Upload New Save"
                            color: "#f8f8f2"
                            font.pixelSize: 15
                            font.bold: _currentIndex === 0
                        }
                    }
                }
            }

            // DIVIDER
            Rectangle {
                width: parent.width - 24
                height: 1
                color: "#44475a"
                anchors.horizontalCenter: parent.horizontalCenter
            }
        }

        // Body area - anchored between topSection and panel bottom
        // Uses explicit anchors so Flickable height is never circular
        Item {
            anchors.top: topSection.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 12

            // LOADING INDICATOR
            Text {
                anchors.centerIn: parent
                text: "Loading saves..."
                color: "#6272a4"
                font.pixelSize: 14
                visible: _loading
            }

            // EMPTY STATE
            Text {
                anchors.centerIn: parent
                text: "No cloud saves found."
                color: "#6272a4"
                font.pixelSize: 14
                visible: !_loading && _slots.length === 0 && _errorText === ""
            }

            // SLOTS LIST - Flickable fills the body Item using anchors (no circular height)
            Flickable {
                anchors.fill: parent
                contentHeight: slotsColumn.implicitHeight
                clip: true
                visible: !_loading && _slots.length > 0
                interactive: false

                Column {
                    id: slotsColumn
                    width: parent.width
                    spacing: 4

                    Item { width: 1; height: 8 }

                    Repeater {
                        model: root._slots
                        delegate: Rectangle {
                            width: parent.width - 24
                            height: 72
                            anchors.horizontalCenter: parent.horizontalCenter

                            property bool itemFocused: root._currentIndex === (index + 1)

                            color: itemFocused ? "#383a59" : "transparent"
                            radius: 6
                            border.color: (itemFocused && root._actionMode === 0) ? "#ff79c6" : "#44475a"
                            border.width: itemFocused ? 2 : 1

                            Row {
                                anchors.fill: parent
                                anchors.margins: 12
                                spacing: 12

                                Column {
                                    width: parent.width - (itemFocused ? 172 : 0)
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 4

                                    Text {
                                        text: modelData.timestamp_text || ""
                                        color: "#f8f8f2"
                                        font.pixelSize: 14
                                        font.bold: true
                                        elide: Text.ElideRight
                                        width: parent.width
                                    }
                                    Text {
                                        text: modelData.file_name || modelData.emulator || ("Slot " + (index + 1))
                                        color: "#6272a4"
                                        font.pixelSize: 12
                                        elide: Text.ElideRight
                                        width: parent.width
                                    }
                                }

                                Row {
                                    visible: itemFocused
                                    spacing: 8
                                    anchors.verticalCenter: parent.verticalCenter

                                    Rectangle {
                                        width: 80
                                        height: 32
                                        radius: 6
                                        color: root._actionMode === 1 ? "#ff79c6" : "#44475a"
                                        border.color: root._actionMode === 1 ? "#f8f8f2" : "transparent"
                                        border.width: root._actionMode === 1 ? 2 : 0

                                        Text {
                                            anchors.centerIn: parent
                                            text: "Restore"
                                            color: root._actionMode === 1 ? "#282a36" : "#f8f8f2"
                                            font.pixelSize: 13
                                            font.bold: true
                                        }
                                    }

                                    Rectangle {
                                        width: 72
                                        height: 32
                                        radius: 6
                                        color: root._actionMode === 2 ? "#ff5555" : "#383a59"
                                        border.color: root._actionMode === 2 ? "#f8f8f2" : "#ff5555"
                                        border.width: root._actionMode === 2 ? 2 : 1

                                        Text {
                                            anchors.centerIn: parent
                                            text: "Delete"
                                            color: root._actionMode === 2 ? "#f8f8f2" : "#ff5555"
                                            font.pixelSize: 13
                                            font.bold: true
                                        }
                                    }
                                }
                            }
                        }
                    }

                    Item { width: 1; height: 12 }
                }
            }
        }
    }
}
