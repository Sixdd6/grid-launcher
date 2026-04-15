import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts
import "../components"

Item {
    id: root
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0

    // Background
    Rectangle {
        anchors.fill: parent
        color: "#282a36"
    }

    // Header bar
    Rectangle {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: 56
        color: "#1e1f29"

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            height: 1
            color: "#44475a"
        }

        // Back button
        Rectangle {
            id: backBtn
            width: 80
            height: parent.height
            color: "transparent"
            anchors.left: parent.left
            
            Text {
                anchors.centerIn: parent
                text: "← Back"
                color: "#ff79c6"
                font.pixelSize: 16
                font.bold: true
            }

            MouseArea {
                anchors.fill: parent
                onClicked: {
                    if (root.StackView.view) {
                        root.StackView.view.pop()
                    }
                }
            }
        }

        Text {
            anchors.right: parent.right
            anchors.rightMargin: 20
            anchors.verticalCenter: parent.verticalCenter
            text: "Settings"
            color: "#f8f8f2"
            font.pixelSize: 20
            font.bold: true
        }
    }

    ScrollView {
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        clip: true

        Column {
            width: parent.width
            spacing: 0

            // Section: GENERAL
            Rectangle {
                width: parent.width; height: 36
                color: "#1e1f29"
                Text { 
                    text: "GENERAL"
                    color: "#6272a4"; font.pixelSize: 11
                    font.letterSpacing: 1.5
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                }
            }

            // Row 1: Default Startup Tab
            Rectangle {
                width: parent.width; height: 56; color: "transparent"
                Text {
                    text: "Default startup tab"
                    color: "#f8f8f2"
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
                Row {
                    anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    spacing: 10
                    
                    Repeater {
                        model: ["home", "library", "server"]
                        Rectangle {
                            width: 80; height: 30; radius: 6
                            property bool isActive: appBackend.homeViewTab === modelData
                            color: isActive ? "#ff79c6" : "#383a59"
                            Text {
                                text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
                                color: isActive ? "#1e1f29" : "#6272a4"
                                anchors.centerIn: parent
                            }
                            MouseArea {
                                anchors.fill: parent
                                onClicked: appBackend.setHomeViewTab(modelData)
                            }
                        }
                    }
                }
            }

            // Row 2: Server URL
            Rectangle {
                width: parent.width; height: 56; color: "transparent"
                Text {
                    text: "Server"
                    color: "#f8f8f2"
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
                Text {
                    text: appBackend.serverUrl ? appBackend.serverUrl : "Not configured"
                    color: "#6272a4"
                    anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
            }

            // Row 3: Auto cloud sync
            Rectangle {
                width: parent.width; height: 56; color: "transparent"
                Text {
                    text: "Auto cloud sync"
                    color: "#f8f8f2"
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
                Rectangle {
                    anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    width: 44; height: 24; radius: 12
                    property bool checked: appBackend.isAutoSync
                    color: checked ? "#ff79c6" : "#44475a"
                    
                    Rectangle {
                        id: knob
                        width: 20; height: 20; radius: 10
                        color: "#f8f8f2"
                        y: 2
                        x: parent.checked ? parent.width - width - 2 : 2
                        Behavior on x { SmoothedAnimation { velocity: 150 } }
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: appBackend.setAutoSync(!appBackend.isAutoSync)
                    }
                    Behavior on color { ColorAnimation { duration: 150 } }
                }
            }

            // Row 4: Return to Desktop Mode
            Item {
                width: parent.width; height: 56
                Rectangle {
                    anchors { left: parent.left; right: parent.right; leftMargin: 20; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    height: 44; radius: 8
                    color: "#bd93f9"
                    Text {
                        text: "Return to Desktop Mode ↩"
                        color: "#1e1f29"
                        anchors.centerIn: parent
                        font.pixelSize: 16
                        font.bold: true
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: appBackend.requestDesktopMode()
                    }
                }
            }

            // Section: CONTROLLER
            Rectangle {
                width: parent.width; height: 36
                color: "#1e1f29"
                Text { 
                    text: "CONTROLLER"
                    color: "#6272a4"; font.pixelSize: 11
                    font.letterSpacing: 1.5
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                }
            }

            // Guide Button Exclusion List Header
            Rectangle {
                width: parent.width; height: 56; color: "transparent"
                property bool addMode: false
                id: exclusionHeaderRow
                Text {
                    text: "Guide button exclusions"
                    color: "#f8f8f2"
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
                Rectangle {
                    anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    width: 60; height: 30; radius: 6; color: "transparent"
                    Text {
                        text: "+ Add"
                        color: "#ff79c6"
                        anchors.centerIn: parent
                        font.bold: true
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: exclusionHeaderRow.addMode = !exclusionHeaderRow.addMode
                    }
                }
            }

            // Add new exclusion input row
            Rectangle {
                width: parent.width; height: exclusionHeaderRow.addMode ? 56 : 0; color: "transparent"
                visible: exclusionHeaderRow.addMode
                clip: true
                Behavior on height { NumberAnimation { duration: 150 } }

                Row {
                    anchors { left: parent.left; right: parent.right; leftMargin: 20; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    spacing: 12
                    Rectangle {
                        width: parent.width - 80; height: 36
                        color: "#1e1f29"
                        border.color: "#44475a"
                        border.width: 1
                        radius: 4
                        TextInput {
                            id: newExclusionInput
                            anchors.fill: parent
                            anchors.leftMargin: 10; anchors.rightMargin: 10
                            color: "#f8f8f2"
                            verticalAlignment: TextInput.AlignVCenter
                            font.pixelSize: 16
                            clip: true
                        }
                    }
                    Rectangle {
                        width: 68; height: 36; radius: 4
                        color: "#ff79c6"
                        Text {
                            text: "Add"
                            color: "#1e1f29"
                            anchors.centerIn: parent
                            font.bold: true
                        }
                        MouseArea {
                            anchors.fill: parent
                            onClicked: {
                                if (newExclusionInput.text.trim().length > 0) {
                                    appBackend.addExclusionEntry(newExclusionInput.text.trim())
                                    newExclusionInput.text = ""
                                    exclusionHeaderRow.addMode = false
                                }
                            }
                        }
                    }
                }
            }

            // Exclusion List Repeater
            Repeater {
                id: exclusionRepeater
                property var _exclusions: appBackend.tvGuideExclusionList || []
                model: _exclusions
                Rectangle {
                    width: parent.width; height: 48; color: "transparent"
                    Text {
                        text: modelData
                        color: "#f8f8f2"
                        anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                        font.pixelSize: 16
                    }
                    Rectangle {
                        anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                        width: 90; height: 30; radius: 4; color: "#1e1f29"
                        Text {
                            text: "✕ Remove"
                            color: "#ff5555"
                            anchors.centerIn: parent
                        }
                        MouseArea {
                            anchors.fill: parent
                            onClicked: appBackend.removeExclusionEntry(modelData)
                        }
                    }
                }
            }

            // Section: KEYBINDS 
            Rectangle {
                width: parent.width; height: 36
                color: "#1e1f29"
                Text { 
                    text: "KEYBINDS"
                    color: "#6272a4"; font.pixelSize: 11
                    font.letterSpacing: 1.5
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                }
            }

            Repeater {
                model: [
                    { button: "LB / Del", action: "Previous tab" },
                    { button: "RB / End", action: "Next tab" },
                    { button: "D-pad / Analog", action: "Navigate" },
                    { button: "A / Cross", action: "Confirm" },
                    { button: "B / Circle", action: "Back" },
                    { button: "Guide", action: "Pause overlay" }
                ]
                Rectangle {
                    width: parent.width; height: 44
                    color: index % 2 === 0 ? "#1e1f29" : "#282a36"
                    Text {
                        text: modelData.button
                        color: "#6272a4"
                        width: 160
                        anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                        font.pixelSize: 16
                    }
                    Text {
                        text: modelData.action
                        color: "#f8f8f2"
                        anchors { left: parent.left; leftMargin: 180; verticalCenter: parent.verticalCenter }
                        font.pixelSize: 16
                    }
                }
            }

            // Section: THEME
            Rectangle {
                width: parent.width; height: 36
                color: "#1e1f29"
                Text { 
                    text: "THEME"
                    color: "#6272a4"; font.pixelSize: 11
                    font.letterSpacing: 1.5
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                }
            }

            Rectangle {
                width: parent.width; height: 56; color: "transparent"
                Text {
                    text: "Theme"
                    color: "#f8f8f2"
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
                Text {
                    text: "Dark"
                    color: "#6272a4"
                    anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
            }

            // Section: SOUND
            Rectangle {
                width: parent.width; height: 36
                color: "#1e1f29"
                Text { 
                    text: "SOUND"
                    color: "#6272a4"; font.pixelSize: 11
                    font.letterSpacing: 1.5
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                }
            }

            Rectangle {
                width: parent.width; height: 56; color: "transparent"
                Text {
                    text: "Sound"
                    color: "#f8f8f2"
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
                Text {
                    text: "Coming soon"
                    color: "#6272a4"
                    anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
            }
        }
    }

    Connections {
        target: appBackend
        function onExclusionListChanged(list) {
            exclusionRepeater._exclusions = list
        }
    }

    Connections {
        target: controllerBackend
        function onNavigationEvent(direction) {
            if (direction === "back") {
                if (root.StackView.view) {
                    root.StackView.view.pop()
                }
            }
        }
    }
}
