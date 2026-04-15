import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts
import "../components"

Item {
    id: root
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0

    // --- Focus tracking ---
    property int currentFocusIndex: 0
    property int _tabSubIndex: 0
    property int _exclusionStartIndex: exclusionHeaderRow.addMode ? 6 : 5
    property int _totalFocusCount: _exclusionStartIndex + (exclusionRepeater._exclusions ? exclusionRepeater._exclusions.length : 0)
    property bool inputActive: exclusionHeaderRow.addMode && newExclusionInput.activeFocus
    readonly property int idxBack: 0
    readonly property int idxTabs: 1
    readonly property int idxCloudSync: 2
    readonly property int idxDesktop: 3
    readonly property int idxAddButton: 4
    readonly property int idxAddRow: 5

    function _snapTabSubIndex() {
        var tabs = ["home", "library", "server"]
        var idx = tabs.indexOf(appBackend.homeViewTab)
        _tabSubIndex = (idx >= 0) ? idx : 0
    }

    function _activateCurrentControl() {
        if (currentFocusIndex === idxBack) {
            if (root.StackView.view) root.StackView.view.pop()
        } else if (currentFocusIndex === idxTabs) {
            appBackend.setHomeViewTab(["home", "library", "server"][_tabSubIndex])
        } else if (currentFocusIndex === idxCloudSync) {
            appBackend.setAutoSync(!appBackend.isAutoSync)
        } else if (currentFocusIndex === idxDesktop) {
            appBackend.requestDesktopMode()
        } else if (currentFocusIndex === idxAddButton) {
            exclusionHeaderRow.addMode = !exclusionHeaderRow.addMode
            if (exclusionHeaderRow.addMode) currentFocusIndex = idxAddRow
        } else if (exclusionHeaderRow.addMode && currentFocusIndex === idxAddRow) {
            if (newExclusionInput.text.trim().length > 0) {
                appBackend.addExclusionEntry(newExclusionInput.text.trim())
                newExclusionInput.text = ""
                exclusionHeaderRow.addMode = false
                currentFocusIndex = idxAddButton
            } else {
                newExclusionInput.forceActiveFocus()
            }
        } else {
            var exclusionIdx = currentFocusIndex - _exclusionStartIndex
            var list = exclusionRepeater._exclusions || []
            if (exclusionIdx >= 0 && exclusionIdx < list.length) {
                appBackend.removeExclusionEntry(list[exclusionIdx])
                currentFocusIndex = Math.min(currentFocusIndex, _totalFocusCount - 2)
            }
        }
    }

    function _ensureFocusVisible() {
        var y = 0
        if (currentFocusIndex === idxBack)           { y = 0 }
        else if (currentFocusIndex === idxTabs)      { y = 36 }
        else if (currentFocusIndex === idxCloudSync) { y = 36 + 112 }
        else if (currentFocusIndex === idxDesktop)   { y = 36 + 168 }
        else if (currentFocusIndex === idxAddButton) { y = 36 + 168 + 56 + 36 }
        else if (exclusionHeaderRow.addMode && currentFocusIndex === idxAddRow) {
            y = 36 + 168 + 56 + 36 + 56
        } else {
            var exIdx = currentFocusIndex - _exclusionStartIndex
            var baseY = 36 + 168 + 56 + 36 + 56 + (exclusionHeaderRow.addMode ? 56 : 0)
            y = baseY + exIdx * 48
        }
        var fl = settingsScroll.contentItem
        if (fl) fl.contentY = Math.max(0, Math.min(y - 80, fl.contentHeight - settingsScroll.height))
    }

    onCurrentFocusIndexChanged: _ensureFocusVisible()

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
            anchors.left: parent.left
            radius: 4
            color: currentFocusIndex === idxBack ? "#1e1f29" : "transparent"
            border.color: currentFocusIndex === idxBack ? "#ff79c6" : "transparent"
            border.width: currentFocusIndex === idxBack ? 2 : 0
            Behavior on color { ColorAnimation { duration: 80 } }
            Behavior on border.color { ColorAnimation { duration: 80 } }

            Text {
                anchors.centerIn: parent
                text: "← Back"
                color: "#ff79c6"
                font.pixelSize: 16
                font.bold: true
            }
            MouseArea {
                anchors.fill: parent
                onClicked: { if (root.StackView.view) root.StackView.view.pop() }
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
        id: settingsScroll
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
                width: parent.width; height: 56
                color: currentFocusIndex === idxTabs ? "#383a59" : "transparent"
                Behavior on color { ColorAnimation { duration: 100 } }

                Rectangle {
                    width: 3; height: parent.height * 0.6
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    color: "#ff79c6"
                    opacity: currentFocusIndex === idxTabs ? 1.0 : 0.0
                    Behavior on opacity { NumberAnimation { duration: 100 } }
                }

                Text {
                    text: "Default startup tab"
                    color: "#f8f8f2"
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }

                Rectangle {
                    anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    width: 268; height: 38
                    color: "transparent"
                    border.color: currentFocusIndex === idxTabs ? "#ff79c6" : "transparent"
                    border.width: 1
                    radius: 8
                    Behavior on border.color { ColorAnimation { duration: 80 } }

                    Row {
                        anchors.centerIn: parent
                        spacing: 10
                        Repeater {
                            model: ["home", "library", "server"]
                            Rectangle {
                                width: 80; height: 30; radius: 6
                                property bool isActive: appBackend.homeViewTab === modelData
                                property bool isSubFocused: currentFocusIndex === idxTabs && index === root._tabSubIndex
                                color: isActive ? "#ff79c6" : "#383a59"
                                border.color: isSubFocused && !isActive ? "#ff79c6" : "transparent"
                                border.width: isSubFocused && !isActive ? 2 : 0
                                Behavior on border.color { ColorAnimation { duration: 80 } }

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
                width: parent.width; height: 56
                color: currentFocusIndex === idxCloudSync ? "#383a59" : "transparent"
                Behavior on color { ColorAnimation { duration: 100 } }

                Rectangle {
                    width: 3; height: parent.height * 0.6
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    color: "#ff79c6"
                    opacity: currentFocusIndex === idxCloudSync ? 1.0 : 0.0
                    Behavior on opacity { NumberAnimation { duration: 100 } }
                }

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
                    border.color: currentFocusIndex === idxCloudSync ? "#ff79c6" : "transparent"
                    border.width: currentFocusIndex === idxCloudSync ? 2 : 0
                    Behavior on border.color { ColorAnimation { duration: 100 } }
                    Behavior on color { ColorAnimation { duration: 150 } }

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
                }
            }

            // Row 4: Return to Desktop Mode
            Item {
                id: desktopModeRow
                width: parent.width; height: 56
                property bool isFocused: currentFocusIndex === idxDesktop

                Rectangle {
                    width: 3; height: parent.height * 0.6
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    color: "#ff79c6"
                    opacity: desktopModeRow.isFocused ? 1.0 : 0.0
                    Behavior on opacity { NumberAnimation { duration: 100 } }
                }

                // Outer glow
                Rectangle {
                    x: 20 - 16; y: (parent.height - 44) / 2 - 16
                    width: parent.width - 40 + 32; height: 44 + 32
                    radius: 24
                    color: "#ff79c6"
                    opacity: desktopModeRow.isFocused ? 0.15 : 0.0
                    Behavior on opacity { NumberAnimation { duration: 100 } }
                }

                // Inner glow
                Rectangle {
                    x: 20 - 8; y: (parent.height - 44) / 2 - 8
                    width: parent.width - 40 + 16; height: 44 + 16
                    radius: 16
                    color: "#ff79c6"
                    opacity: desktopModeRow.isFocused ? 0.20 : 0.0
                    Behavior on opacity { NumberAnimation { duration: 100 } }
                }

                Rectangle {
                    anchors { left: parent.left; right: parent.right; leftMargin: 20; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    height: 44; radius: 8
                    color: "#bd93f9"
                    border.color: desktopModeRow.isFocused ? "#ff79c6" : "transparent"
                    border.width: desktopModeRow.isFocused ? 2 : 0
                    Behavior on border.color { ColorAnimation { duration: 80 } }

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
                id: exclusionHeaderRow
                width: parent.width; height: 56
                color: currentFocusIndex === idxAddButton ? "#383a59" : "transparent"
                Behavior on color { ColorAnimation { duration: 100 } }
                property bool addMode: false

                Rectangle {
                    width: 3; height: parent.height * 0.6
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    color: "#ff79c6"
                    opacity: currentFocusIndex === idxAddButton ? 1.0 : 0.0
                    Behavior on opacity { NumberAnimation { duration: 100 } }
                }

                Text {
                    text: "Guide button exclusions"
                    color: "#f8f8f2"
                    anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                    font.pixelSize: 16
                }
                Rectangle {
                    anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    width: 60; height: 30; radius: 6
                    color: "transparent"
                    border.color: currentFocusIndex === idxAddButton ? "#ff79c6" : "#44475a"
                    border.width: currentFocusIndex === idxAddButton ? 2 : 1
                    Behavior on border.color { ColorAnimation { duration: 80 } }

                    Rectangle {
                        anchors.fill: parent
                        radius: 6
                        color: "#ff79c6"
                        opacity: currentFocusIndex === idxAddButton ? 0.1 : 0.0
                        Behavior on opacity { NumberAnimation { duration: 100 } }
                    }

                    Text {
                        text: "+ Add"
                        color: "#ff79c6"
                        anchors.centerIn: parent
                        font.bold: true
                        z: 1
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: exclusionHeaderRow.addMode = !exclusionHeaderRow.addMode
                    }
                }
            }

            // Add new exclusion input row
            Rectangle {
                width: parent.width
                height: exclusionHeaderRow.addMode ? 56 : 0
                color: (exclusionHeaderRow.addMode && currentFocusIndex === idxAddRow) ? "#383a59" : "transparent"
                Behavior on color { ColorAnimation { duration: 100 } }
                visible: exclusionHeaderRow.addMode
                clip: true
                Behavior on height { NumberAnimation { duration: 150 } }

                Rectangle {
                    width: 3; height: parent.height * 0.6
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    color: "#ff79c6"
                    opacity: (exclusionHeaderRow.addMode && currentFocusIndex === idxAddRow) ? 1.0 : 0.0
                    Behavior on opacity { NumberAnimation { duration: 100 } }
                }

                Row {
                    anchors { left: parent.left; right: parent.right; leftMargin: 20; rightMargin: 20; verticalCenter: parent.verticalCenter }
                    spacing: 12
                    Rectangle {
                        width: parent.width - 80; height: 36
                        color: "#1e1f29"
                        border.color: currentFocusIndex === idxAddRow ? "#ff79c6" : "#44475a"
                        border.width: 1
                        radius: 4
                        Behavior on border.color { ColorAnimation { duration: 80 } }
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
                        Text { text: "Add"; color: "#1e1f29"; anchors.centerIn: parent; font.bold: true }
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
                    width: parent.width; height: 48
                    property bool isFocused: currentFocusIndex === (root._exclusionStartIndex + index)
                    color: isFocused ? "#383a59" : "transparent"
                    Behavior on color { ColorAnimation { duration: 100 } }

                    Rectangle {
                        width: 3; height: parent.height * 0.6
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        color: "#ff79c6"
                        opacity: isFocused ? 1.0 : 0.0
                        Behavior on opacity { NumberAnimation { duration: 100 } }
                    }

                    Text {
                        text: modelData
                        color: "#f8f8f2"
                        anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
                        font.pixelSize: 16
                        z: 1
                    }
                    Rectangle {
                        anchors { right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
                        width: 90; height: 30; radius: 4
                        color: "#1e1f29"
                        border.color: isFocused ? "#ff79c6" : "#44475a"
                        border.width: isFocused ? 2 : 1
                        Behavior on border.color { ColorAnimation { duration: 80 } }

                        Rectangle {
                            anchors.fill: parent
                            radius: 4
                            color: "#ff79c6"
                            opacity: isFocused ? 0.1 : 0.0
                            Behavior on opacity { NumberAnimation { duration: 100 } }
                        }

                        Text { text: "✕ Remove"; color: "#ff5555"; anchors.centerIn: parent; z: 1 }
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
                        color: "#6272a4"; width: 160
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
        target: controllerBackend
        function onNavigationEvent(direction) {
            if (appBackend.uiOverlayActive) return
            if (inputActive) {
                if (direction === "back") { newExclusionInput.focus = false; root.forceActiveFocus() }
                return
            }
            if (direction === "back") {
                if (exclusionHeaderRow.addMode && currentFocusIndex === idxAddRow) {
                    exclusionHeaderRow.addMode = false
                    currentFocusIndex = idxAddButton
                    return
                }
                if (root.StackView.view) root.StackView.view.pop()
                return
            }
            if (direction === "up") {
                if (currentFocusIndex > 0) currentFocusIndex--
                if (currentFocusIndex === idxTabs) _snapTabSubIndex()
            } else if (direction === "down") {
                if (currentFocusIndex < _totalFocusCount - 1) currentFocusIndex++
                if (currentFocusIndex === idxTabs) _snapTabSubIndex()
            } else if (direction === "left") {
                if (currentFocusIndex === idxTabs) {
                    _tabSubIndex = Math.max(0, _tabSubIndex - 1)
                    appBackend.setHomeViewTab(["home", "library", "server"][_tabSubIndex])
                }
            } else if (direction === "right") {
                if (currentFocusIndex === idxTabs) {
                    _tabSubIndex = Math.min(2, _tabSubIndex + 1)
                    appBackend.setHomeViewTab(["home", "library", "server"][_tabSubIndex])
                }
            } else if (direction === "confirm") {
                _activateCurrentControl()
            }
        }
    }

    Connections {
        target: exclusionHeaderRow
        function onAddModeChanged() {
            if (!exclusionHeaderRow.addMode && currentFocusIndex === idxAddRow)
                currentFocusIndex = idxAddButton
        }
    }

    Connections {
        target: appBackend
        function onExclusionListChanged(list) {
            exclusionRepeater._exclusions = list
            var maxIdx = root._totalFocusCount - 1
            if (currentFocusIndex > maxIdx) currentFocusIndex = Math.max(idxAddButton, maxIdx)
        }
    }
}


