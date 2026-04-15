import QtQuick 2.15
import QtQuick.Controls 2.15
import "../components"

Item {
    id: root
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0

    property var game: ({})
    property int _focusedColumn: 0
    property int _leftButtonIndex: 0
    property real _installProgress: 0.0
    property int _cloudSlotCount: 0
    property string _cloudMostRecent: ""
    property string _bannerText: ""
    property bool _bannerSuccess: true
    property var _screenshotList: {
        var raw = root.game.screenshot_urls || ""
        if (!raw) return []
        return raw.split("\n").filter(function(s) { return s.trim() !== "" })
    }

    function formatFilesize(bytes) {
        if (!bytes) return ""
        var b = parseInt(bytes)
        if (b < 1024) return b + " B"
        if (b < 1048576) return (b/1024).toFixed(1) + " KB"
        if (b < 1073741824) return (b/1048576).toFixed(1) + " MB"
        return (b/1073741824).toFixed(2) + " GB"
    }

    function _visibleButtonCount() {
        var count = 1
        if (root.game.local_path) count++
        if (appBackend.isConnected) count++
        return count
    }

    function _navBlocked() {
        return appBackend.uiOverlayActive
    }

    function _triggerLeftButton() {
        if (_leftButtonIndex === 0) {
            if (!gameBackend.isInstallActive) {
                if (root.game.local_path) {
                    gameBackend.launchGame(root.game)
                } else if (appBackend.isConnected) {
                    gameBackend.installGame(root.game)
                }
            }
            return
        }
        var idx = 1
        if (root.game.local_path && _leftButtonIndex === idx) {
            gameBackend.uninstallGame(root.game)
            return
        }
        if (root.game.local_path) idx++
        
        if (appBackend.isConnected && _leftButtonIndex === idx) {
            cloudSavesOverlay.visible = true
        }
    }

    onVisibleChanged: {
        if (visible && appBackend.isConnected && root.game) {
            cloudBackend.loadSlotsForGame(root.game, "save")
        }
    }

    Connections {
        target: controllerBackend
        function onNavigationEvent(direction) {
            if (!root.visible) return
            if (root.StackView.status !== StackView.Active) return
            if (root._navBlocked()) return
            
            if (direction === "back") {
                root.StackView.view.pop()
                return
            }
            if (direction === "left") {
                root._focusedColumn = Math.max(0, root._focusedColumn - 1)
                return
            }
            if (direction === "right") {
                root._focusedColumn = Math.min(2, root._focusedColumn + 1)
                return
            }
            if (direction === "up") {
                if (root._focusedColumn === 0) {
                    root._leftButtonIndex = Math.max(0, root._leftButtonIndex - 1)
                } else if (root._focusedColumn === 1) {
                    centerFlickable.contentY = Math.max(0, centerFlickable.contentY - 80)
                } else if (root._focusedColumn === 2) {
                    screenshotFlickable.contentY = Math.max(0, screenshotFlickable.contentY - 120)
                }
                return
            }
            if (direction === "down") {
                if (root._focusedColumn === 0) {
                    root._leftButtonIndex = Math.min(root._visibleButtonCount() - 1, root._leftButtonIndex + 1)
                } else if (root._focusedColumn === 1) {
                    var maxY = Math.max(0, centerFlickable.contentHeight - centerFlickable.height)
                    centerFlickable.contentY = Math.min(maxY, centerFlickable.contentY + 80)
                } else if (root._focusedColumn === 2) {
                    var maxSY = Math.max(0, screenshotFlickable.contentHeight - screenshotFlickable.height)
                    screenshotFlickable.contentY = Math.min(maxSY, screenshotFlickable.contentY + 120)
                }
                return
            }
            if (direction === "confirm") {
                if (root._focusedColumn === 0) {
                    root._triggerLeftButton()
                }
                return
            }
        }
    }

    Connections {
        target: gameBackend
        function onInstallProgress(downloaded, total, speed) {
            if (total > 0) root._installProgress = downloaded / total
        }
        function onInstallComplete(success, message, returnedGame) {
            root._bannerText = message
            root._bannerSuccess = success
            bannerTimer.restart()
            if (success && returnedGame && returnedGame.local_path) {
                root.game = Object.assign({}, root.game, { local_path: returnedGame.local_path })
            }
        }
        function onUninstallComplete(success, message, returnedGame) {
            root._bannerText = message
            root._bannerSuccess = success
            bannerTimer.restart()
            if (success) {
                var updated = Object.assign({}, root.game)
                delete updated.local_path
                root.game = updated
            }
        }
        function onLaunchError(msg) {
            root._bannerText = msg
            root._bannerSuccess = false
            bannerTimer.restart()
        }
    }

    Connections {
        target: cloudBackend
        function onSlotsLoaded(slots) {
            root._cloudSlotCount = slots.length
            if (slots.length > 0 && slots[0].timestamp_text) {
                root._cloudMostRecent = slots[0].timestamp_text
            } else {
                root._cloudMostRecent = ""
            }
        }
    }

    Timer {
        id: bannerTimer
        interval: 4000
        onTriggered: root._bannerText = ""
    }

    // Background fanart
    Image {
        anchors.fill: parent
        source: root.game.fanart_url ? root.game.fanart_url : (root._screenshotList.length > 0 ? root._screenshotList[0] : "")
        fillMode: Image.PreserveAspectCrop
        opacity: 0.3
    }
    
    Rectangle {
        anchors.fill: parent
        color: "#282a36"
        opacity: 0.7
    }

    Column {
        anchors.fill: parent

        // Header
        Rectangle {
            width: parent.width
            height: 48
            color: "#1e1f29"
            
            Row {
                anchors.fill: parent
                anchors.margins: 12
                spacing: 16
                
                Text {
                    text: "← Back"
                    color: "#f8f8f2"
                    font.pixelSize: 16
                    anchors.verticalCenter: parent.verticalCenter
                }
                
                Text {
                    text: root.game.name || root.game.fs_name || "Details"
                    color: "#f8f8f2"
                    font.pixelSize: 16
                    font.bold: true
                    anchors.verticalCenter: parent.verticalCenter
                }
            }
        }

        // Status banner
        Rectangle {
            width: parent.width
            height: 40
            color: root._bannerSuccess ? "#50fa7b" : "#ff5555"
            visible: root._bannerText !== ""
            
            Text {
                anchors.centerIn: parent
                text: root._bannerText
                color: "#1e1f29"
                font.pixelSize: 14
                font.bold: true
            }
        }

        // Main content
        Row {
            width: parent.width
            height: parent.height - 48 - (root._bannerText !== "" ? 40 : 0)
            spacing: 0

            // Left Column
            Rectangle {
                width: parent.width * 0.25
                height: parent.height
                color: "transparent"
                border.color: "#44475a"
                border.width: 1
                radius: 6

                Column {
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.margins: 16
                    spacing: 12

                    // Cover Art
                    Rectangle {
                        width: parent.width
                        height: coverImage.implicitHeight > 0 ? coverImage.implicitHeight : width * 1.33
                        color: "#1e1f29"
                        radius: 8
                        clip: true

                        Image {
                            id: coverImage
                            width: parent.width
                            height: implicitHeight > 0 ? implicitWidth > 0 ? Math.round(width * implicitHeight / implicitWidth) : width * 1.33 : width * 1.33
                            source: typeof(root.game.cover_url) === "string" && root.game.cover_url.length > 0 ? "image://covers/" + root.game.cover_url : ("image://covers/" + (root.game.path_cover_l || root.game.path_cover_s || ""))
                            fillMode: Image.PreserveAspectFit
                            onStatusChanged: fallbackText.visible = (status === Image.Error || status === Image.Null)
                        }

                        Text {
                            id: fallbackText
                            text: "?"
                            color: "#f8f8f2"
                            font.pixelSize: 48
                            anchors.centerIn: parent
                            visible: false
                        }
                    }

                    // Play Button
                    Rectangle {
                        width: parent.width
                        height: 48
                        radius: 8
                        color: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? "#ff79c6" : "#1e1f29"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? "#ff79c6" : "transparent"
                        border.width: 2
                        visible: !!root.game.local_path && !gameBackend.isInstallActive

                        Text {
                            anchors.centerIn: parent
                            text: gameBackend.isSessionActive ? "▐▐  Playing..." : "▶  Play"
                            color: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? "#282a36" : "#f8f8f2"
                            font.pixelSize: 16
                            font.bold: true
                        }
                    }

                    // Install Button
                    Rectangle {
                        width: parent.width
                        height: 48
                        radius: 8
                        color: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? "#ff79c6" : "#50fa7b"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? "#ff79c6" : "transparent"
                        border.width: 2
                        visible: !root.game.local_path && appBackend.isConnected && !gameBackend.isInstallActive

                        Text {
                            anchors.centerIn: parent
                            text: "⬇  Install"
                            color: "#282a36"
                            font.pixelSize: 16
                            font.bold: true
                        }
                    }

                    // Install Progress (Visible when installing)
                    Column {
                        width: parent.width
                        spacing: 4
                        visible: gameBackend.isInstallActive

                        Text {
                            text: "Installing..."
                            color: "#f8f8f2"
                            font.pixelSize: 14
                        }

                        Rectangle {
                            id: track
                            width: parent.width
                            height: 6
                            color: "#44475a"
                            radius: 3

                            Rectangle {
                                width: track.width * root._installProgress
                                height: parent.height
                                color: "#ff79c6"
                                radius: 3
                            }
                        }
                    }

                    // Uninstall Button
                    Rectangle {
                        property int myIndex: 1
                        width: parent.width
                        height: 44
                        radius: 8
                        color: "#383a59"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? "#ff79c6" : "#ff5555"
                        border.width: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? 2 : 1
                        visible: !!root.game.local_path

                        Text {
                            anchors.centerIn: parent
                            text: "✕  Uninstall"
                            color: (root._focusedColumn === 0 && root._leftButtonIndex === parent.myIndex) ? "#f8f8f2" : "#ff5555"
                            font.pixelSize: 14
                        }
                    }

                    // Cloud Saves Button
                    Rectangle {
                        property int myIndex: (root.game.local_path ? 2 : 1)
                        width: parent.width
                        height: 44
                        radius: 8
                        color: "#1e1f29"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? "#ff79c6" : "#bd93f9"
                        border.width: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? 2 : 1
                        visible: appBackend.isConnected

                        Text {
                            anchors.centerIn: parent
                            text: "☁  Cloud Saves"
                            color: (root._focusedColumn === 0 && root._leftButtonIndex === parent.myIndex) ? "#f8f8f2" : "#bd93f9"
                            font.pixelSize: 14
                        }
                    }
                }
            }

            // Center Column
            Rectangle {
                width: parent.width * 0.5
                height: parent.height
                color: "transparent"
                border.color: root._focusedColumn === 1 ? "#ff79c6" : "#44475a"
                border.width: root._focusedColumn === 1 ? 2 : 1
                radius: 6
                
                Behavior on border.color { ColorAnimation { duration: 150 } }

                Flickable {
                    id: centerFlickable
                    anchors.fill: parent
                    anchors.margins: 1
                    clip: true
                    contentHeight: metaColumn.implicitHeight
                    
                    Column {
                        id: metaColumn
                        width: centerFlickable.width
                        padding: 16
                        spacing: 8
                        
                        Text {
                            text: root.game.name || root.game.fs_name || ""
                            font.pixelSize: 22
                            font.bold: true
                            color: "#f8f8f2"
                            wrapMode: Text.Wrap
                            width: parent.width - 32
                        }
                        
                        Text {
                            text: root.game.platform_name || root.game.platform || ""
                            font.pixelSize: 13
                            color: "#6272a4"
                            visible: text !== ""
                        }
                        
                        Row {
                            spacing: 4
                            visible: !!root.game.average_rating && root.game.average_rating !== ""
                            Text { text: "★"; color: "#ff79c6"; font.pixelSize: 14 }
                            Text { text: root.game.average_rating || ""; color: "#f8f8f2"; font.pixelSize: 14 }
                        }
                        
                        Row {
                            spacing: 4
                            visible: !!root.game.first_release_date && root.game.first_release_date !== ""
                            Text { text: "Released:"; color: "#6272a4"; font.pixelSize: 14 }
                            Text { text: root.game.first_release_date || ""; color: "#f8f8f2"; font.pixelSize: 14 }
                        }
                        
                        Row {
                            spacing: 4
                            visible: !!root.game.companies || !!root.game.developer
                            Text { text: "By:"; color: "#6272a4"; font.pixelSize: 14 }
                            Text { 
                                text: root.game.companies || root.game.developer || ""
                                color: "#f8f8f2"
                                font.pixelSize: 14
                                wrapMode: Text.Wrap
                                width: metaColumn.width - 64
                            }
                        }
                        
                        Flow {
                            width: parent.width - 32
                            spacing: 6
                            visible: !!root.game.genres && typeof(root.game.genres) === "string" && root.game.genres.length > 0
                            
                            Repeater {
                                model: (root.game.genres || "").split(",").filter(function(s) { return s.trim() !== ""; })
                                delegate: Rectangle {
                                    color: "#383a59"
                                    radius: 10
                                    width: genreText.width + 16
                                    height: genreText.height + 8
                                    Text {
                                        id: genreText
                                        anchors.centerIn: parent
                                        text: modelData.trim()
                                        color: "#6272a4"
                                        font.pixelSize: 12
                                    }
                                }
                            }
                        }
                        
                        Text {
                            text: root.game.summary || root.game.description || "No description available."
                            color: "#f8f8f2"
                            wrapMode: Text.Wrap
                            font.pixelSize: 14
                            topPadding: 8
                            width: parent.width - 32
                        }
                        
                        Text {
                            visible: !!root.game.filesize_bytes || !!root.game.file_size_bytes
                            color: "#6272a4"
                            text: "Size: " + root.formatFilesize(root.game.filesize_bytes || root.game.file_size_bytes)
                            font.pixelSize: 14
                        }
                        
                        Row {
                            spacing: 4
                            visible: !!root.game.regions && root.game.regions.length > 0
                            Text { text: "Regions:"; color: "#6272a4"; font.pixelSize: 14 }
                            Text { 
                                text: (Array.isArray(root.game.regions) ? root.game.regions.join(", ") : root.game.regions) || ""
                                color: "#f8f8f2"
                                font.pixelSize: 14
                            }
                        }
                        
                        Row {
                            spacing: 4
                            visible: !!root.game.languages && root.game.languages.length > 0
                            Text { text: "Languages:"; color: "#6272a4"; font.pixelSize: 14 }
                            Text { 
                                text: (Array.isArray(root.game.languages) ? root.game.languages.join(", ") : root.game.languages) || ""
                                color: "#f8f8f2"
                                font.pixelSize: 14
                            }
                        }
                        
                        Flow {
                            width: parent.width - 32
                            spacing: 6
                            topPadding: 4
                            visible: !!root.game.tags && root.game.tags.length > 0
                            
                            Repeater {
                                model: Array.isArray(root.game.tags) ? root.game.tags : []
                                delegate: Rectangle {
                                    color: "#1e1f29"
                                    border.color: "#44475a"
                                    border.width: 1
                                    radius: 10
                                    width: tagText.width + 16
                                    height: tagText.height + 8
                                    Text {
                                        id: tagText
                                        anchors.centerIn: parent
                                        text: modelData.name || modelData
                                        color: "#6272a4"
                                        font.pixelSize: 12
                                    }
                                }
                            }
                        }

                        // Cloud Saves Summary
                        Item { width: parent.width - 32; height: 8 }

                        Rectangle {
                            width: parent.width - 32
                            height: 1
                            color: "#44475a"
                        }

                        Text {
                            text: "☁  Cloud Saves"
                            color: "#bd93f9"
                            font.pixelSize: 15
                            font.bold: true
                            topPadding: 4
                        }

                        Text {
                            visible: !appBackend.isConnected
                            text: "Connect to a RomM server to manage cloud saves."
                            color: "#6272a4"
                            font.pixelSize: 13
                            wrapMode: Text.Wrap
                            width: parent.width - 32
                        }

                        Column {
                            visible: appBackend.isConnected
                            spacing: 6
                            width: parent.width - 32

                            Text {
                                text: root._cloudSlotCount + " save(s) found"
                                color: "#f8f8f2"
                                font.pixelSize: 13
                            }

                            Text {
                                text: root._cloudMostRecent !== "" ? "Latest: " + root._cloudMostRecent : "No recent saves"
                                color: "#6272a4"
                                font.pixelSize: 12
                            }

                            Rectangle {
                                width: parent.width
                                height: 40
                                color: "#1e1f29"
                                border.color: "#bd93f9"
                                border.width: 1
                                radius: 8

                                Text {
                                    anchors.centerIn: parent
                                    text: "☁  Open Cloud Saves"
                                    color: "#bd93f9"
                                    font.pixelSize: 13
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: cloudSavesOverlay.visible = true
                                }
                            }
                        }
                    }
                }
                
                // Scroll indicator
                Rectangle {
                    anchors.right: parent.right
                    width: 3
                    color: "#44475a"
                    opacity: 0.6
                    y: centerFlickable.visibleArea.yPosition * centerFlickable.height
                    height: centerFlickable.visibleArea.heightRatio * centerFlickable.height
                    visible: centerFlickable.contentHeight > centerFlickable.height
                }
            }

            // Right Column
            Rectangle {
                width: parent.width * 0.25
                height: parent.height
                color: "transparent"
                border.color: root._focusedColumn === 2 ? "#ff79c6" : "#44475a"
                border.width: root._focusedColumn === 2 ? 2 : 1
                radius: 6

                Behavior on border.color { ColorAnimation { duration: 150 } }

                Item {
                    anchors.fill: parent
                    anchors.margins: 16

                    Text {
                        id: screenshotHeader
                        text: "Screenshots"
                        color: "#6272a4"
                        font.pixelSize: 13
                        font.bold: true
                        visible: root._screenshotList.length > 0
                        anchors.top: parent.top
                        anchors.left: parent.left
                    }

                    Text {
                        anchors.centerIn: parent
                        text: "No screenshots\navailable."
                        color: "#6272a4"
                        font.pixelSize: 13
                        visible: root._screenshotList.length === 0
                        horizontalAlignment: Text.AlignHCenter
                    }

                    Flickable {
                        id: screenshotFlickable
                        anchors.top: screenshotHeader.bottom
                        anchors.topMargin: 8
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        clip: true
                        contentHeight: screenshotColumn.implicitHeight
                        visible: root._screenshotList.length > 0

                        Column {
                            id: screenshotColumn
                            width: screenshotFlickable.width
                            spacing: 8

                            Repeater {
                                model: root._screenshotList.length
                                delegate: Image {
                                    width: screenshotColumn.width
                                    height: width > 0 ? Math.round(width * 9 / 16) : 0
                                    source: root._screenshotList[index] || ""
                                    fillMode: Image.PreserveAspectCrop
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    CloudSavesOverlay {
        id: cloudSavesOverlay
        anchors.fill: parent
        game: root.game
        saveType: "save"
        visible: false
        z: 10
        onClosed: {
            visible = false
        }
    }
}