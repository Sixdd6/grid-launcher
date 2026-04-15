import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts
import "../components"

Item {
    id: root
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0

    property var game: ({})
    property var outerStack: null

    // Background
    Rectangle {
        anchors.fill: parent
        color: "#282a36"
    }

    // Helper: Format file size
    function formatSize(bytes) {
        if (!bytes) return ""
        var gb = bytes / (1024 * 1024 * 1024)
        if (gb >= 1) return gb.toFixed(2) + " GB"
        var mb = bytes / (1024 * 1024)
        return mb.toFixed(2) + " MB"
    }

    function _screenshotUrls() {
        if (!game || !game.screenshot_urls) return []
        var raw = game.screenshot_urls
        if (typeof raw !== "string") return []
        return raw.split("\n").filter(function(u) { return u.trim().length > 0 })
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

        // Action / Error banner (placed bottom of screen, see below)

        // Title
        Text {
            anchors.right: parent.right
            anchors.rightMargin: 20
            anchors.left: backBtn.right
            anchors.verticalCenter: parent.verticalCenter
            text: game.title || game.name || ""
            color: "#f8f8f2"
            font.pixelSize: 20
            font.bold: true
            elide: Text.ElideRight
            horizontalAlignment: Text.AlignRight
        }
    }

    // Content container
    Row {
        id: contentRow
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom

        // Left column
        Item {
            id: leftCol
            width: 260
            height: parent.height
            
            Column {
                anchors.fill: parent
                anchors.margins: 20
                spacing: 16

                // Cover Image
                Rectangle {
                    width: 220
                    height: 300
                    color: "transparent"
                    
                    Text {
                        anchors.centerIn: parent
                        text: "?"
                        color: "#44475a"
                        font.pixelSize: 48
                        visible: !coverImage.source || coverImage.status !== Image.Ready
                    }

                    Image {
                        id: coverImage
                        anchors.fill: parent
                        source: game.cover_url ? "image://covers/" + game.cover_url : ""
                        fillMode: Image.PreserveAspectFit
                    }
                }

                // Launch Button
                Rectangle {
                    width: parent.width
                    height: 44
                    radius: 8
                    color: gameBackend.isSessionActive ? "#6272a4" : "#ff79c6"
                    
                    Text {
                        anchors.centerIn: parent
                        text: gameBackend.isSessionActive ? "▐▐  Playing..." : "▶  Launch"
                        color: "#1e1f29"
                        font.pixelSize: 16
                        font.bold: true
                    }

                    MouseArea {
                        anchors.fill: parent
                        enabled: !gameBackend.isSessionActive
                        onClicked: {
                            gameBackend.launchGame(game)
                        }
                    }
                }

                // Cloud Saves Button
                Rectangle {
                    width: parent.width
                    height: 44
                    radius: 8
                    color: "#bd93f9"
                    visible: appBackend.isConnected
                    
                    Text {
                        anchors.centerIn: parent
                        text: "☁  Cloud Saves"
                        color: "#1e1f29"
                        font.pixelSize: 16
                        font.bold: true
                    }

                    MouseArea {
                        anchors.fill: parent
                        onClicked: {
                            slotPicker.saveType = "save"
                            slotPicker.visible = true
                        }
                    }
                }
            }
        }

        // Center column
        Item {
            id: centerCol
            width: parent.width - leftCol.width - rightCol.width
            height: parent.height

            Column {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                anchors.leftMargin: 20
                anchors.rightMargin: 20
                anchors.topMargin: 20
                spacing: 12

                Text {
                    width: parent.width
                    text: game.title || game.name || ""
                    color: "#f8f8f2"
                    font.pixelSize: 24
                    font.bold: true
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    height: 24
                    width: platformText.width + 16
                    color: "#383a59"
                    radius: 6
                    border.color: "#6272a4"
                    border.width: 1
                    visible: game.platform !== undefined && game.platform !== ""

                    Text {
                        id: platformText
                        anchors.centerIn: parent
                        text: game.platform || ""
                        color: "#f8f8f2"
                        font.pixelSize: 12
                    }
                }

                Item {
                    width: parent.width
                    height: Math.min(summaryText.implicitHeight, 200)
                    clip: true
                    
                    Text {
                        id: summaryText
                        width: parent.width
                        text: game.summary || game.description || ""
                        color: "#6272a4"
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                    }
                }

                Row {
                    spacing: 8
                    visible: !!(game.developer || game.publisher)

                    Text {
                        text: (game.developer || "") + (game.developer && game.publisher ? " / " : "") + (game.publisher || "")
                        color: "#6272a4"
                        font.pixelSize: 13
                    }
                }

                Text {
                    visible: game.file_size_bytes !== undefined && game.file_size_bytes > 0
                    text: root.formatSize(game.file_size_bytes)
                    color: "#6272a4"
                    font.pixelSize: 13
                }
            }
        }

        // Right column
        Item {
            id: rightCol
            width: 320
            height: parent.height

            Column {
                anchors.fill: parent
                anchors.margins: 20
                spacing: 12

                Text {
                    text: "SCREENSHOTS"
                    color: "#6272a4"
                    font.pixelSize: 12
                    font.capitalization: Font.AllUppercase
                    visible: root._screenshotUrls().length > 0
                }

                ListView {
                    width: parent.width
                    height: parent.height - 30
                    clip: true
                    spacing: 16
                    model: root._screenshotUrls()
                    
                    delegate: Item {
                        width: 300
                        height: 170
                        
                        Rectangle {
                            anchors.fill: parent
                            radius: 8
                            color: "#1e1f29"
                            clip: true

                            Image {
                                anchors.fill: parent
                                source: modelData
                                fillMode: Image.PreserveAspectCrop
                            }
                        }
                    }
                }
            }
        }
    }

    // Launch error banner
    Rectangle {
        id: errorBanner
        height: 48
        color: "#ff5555"
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        visible: false
        z: 10

        Text {
            id: errorBannerText
            anchors.centerIn: parent
            color: "#f8f8f2"
            font.pixelSize: 16
            font.bold: true
        }

        Timer {
            id: errorTimer
            interval: 4000
            onTriggered: errorBanner.visible = false
        }
    }

    Connections {
        target: gameBackend
        function onLaunchError(msg) {
            errorBanner.color = "#ff5555"
            errorBannerText.text = msg
            errorBanner.visible = true
            errorTimer.restart()
        }
    }

    Connections {
        target: cloudBackend
        function onRestoreComplete(success, message) {
            errorBanner.color = success ? "#50fa7b" : "#ff5555"
            errorBannerText.text = message
            errorBanner.visible = true
            errorTimer.restart()
        }
    }

    // Controller navigation event for Back
    Connections {
        target: controllerBackend
        function onNavigationEvent(direction) {
            if (root.StackView.status === StackView.Active && root.StackView.view) {
                if (direction === "back") {
                    root.StackView.view.pop()
                }
            }
        }
    }

    SlotPickerDialog {
        id: slotPicker
        anchors.fill: parent
        game: root.game
        saveType: "save"
        visible: false
        onClosed: slotPicker.visible = false
    }
}