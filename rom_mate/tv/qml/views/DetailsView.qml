import QtQuick 2.15
import QtQuick.Controls 2.15
import "../components"

Item {
    id: root
    width: parent ? parent.width : 0
    height: parent ? parent.height : 0

    Component.onCompleted: {
        appBackend.logHandleDiag("details-open")
    }

    property var game: ({})
    property string _pendingMetadataRomId: ""
    property bool _metadataLoading: false
    onGameChanged: {
        root._metadataLoading = false
        if (root.game && root.game.rom_id) {
            var rid = String(root.game.rom_id)
            if (rid === root._pendingMetadataRomId) {
                root._pendingMetadataRomId = ""
                return
            }
            appBackend.fetchRomMetadata(JSON.stringify(root.game))
        }
    }
    property int _focusedColumn: 0
    property int _leftButtonIndex: 0
    property real _installProgress: 0.0
    property real _installSpeed: 0.0
    property string _bannerText: ""
    property bool _bannerSuccess: true
    property var _screenshotList: {
        var raw = root.game.screenshot_urls || ""
        if (!raw) return []
        return raw.split("\n").filter(function(s) { return s.trim() !== "" })
    }
    property var _installedGameEntry: {
        var rid = root.game ? (root.game.rom_id || root.game.id || "") : ""
        if (rid !== "") {
            var lib = appBackend.libraryGames
            for (var i = 0; i < lib.length; i++) {
                var entry = lib[i]
                if ((entry.rom_id || entry.id || "") === rid) return entry
            }
        }
        return null
    }
    property string _installedLocalPath: root._installedGameEntry
        ? (root._installedGameEntry.local_path || "") : ""

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
        if (root._installedLocalPath) count++
        if (root._installedLocalPath && _isNativePcGame()) count++
        if (appBackend.isConnected) count++
        if (appBackend.isConnected) count++
        return count
    }

    function _navBlocked() {
        return appBackend.uiOverlayActive || pauseBackend.visible
    }

    function _isNativePcGame() {
        if (!root.game) return false
        var p = root.game.platform || ""
        return p === "Windows" || p === "Windows 9x"
    }

    function _triggerLeftButton() {
        var idx = 0

        // Index 0: Play / Install / Cancel
        if (_leftButtonIndex === idx) {
            if (gameBackend.isInstallActive) {
                gameBackend.cancelInstall()
            } else if (!gameBackend.isSessionActive) {
                if (root._installedLocalPath) {
                    gameBackend.launchGame(root._installedGameEntry || root.game)
                } else if (appBackend.isConnected) {
                    gameBackend.installGame(root.game)
                }
            }
            return
        }
        idx++

        // Index 1 (if installed): Uninstall
        if (root._installedLocalPath) {
            if (_leftButtonIndex === idx) {
                gameBackend.uninstallGame(root.game)
                return
            }
            idx++
        }

        // Next index (if installed native game): Change Executable
        if (root._installedLocalPath && _isNativePcGame()) {
            if (_leftButtonIndex === idx) {
                nativeExecPicker.candidates = gameBackend.getNativeExecutableCandidates(root.game.rom_id || "")
                nativeExecPicker.romId = root.game.rom_id || ""
                nativeExecPicker.currentPath = root.game.native_executable_path || ""
                nativeExecPicker.visible = true
                return
            }
            idx++
        }

        // Next index (if server connected): Cloud Saves
        if (appBackend.isConnected) {
            if (_leftButtonIndex === idx) {
                cloudSavesOverlay.visible = true
                return
            }
            idx++
        }

        if (appBackend.isConnected && _leftButtonIndex === idx) {
            appBackend.toggleFavorite(root.game.rom_id || root.game.id || "")
            return
        }
        idx++
    }

    Connections {
        target: controllerBackend
        function onNavigationEvent(direction) {
            if (pauseBackend.visible) return
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
        target: appBackend
        function onRomMetadataFetchStarted(romId) {
            if (root.game && String(root.game.rom_id) === String(romId)) {
                root._metadataLoading = true
            }
        }
        function onRomMetadataReady(payload) {
            var romId = payload.rom_id
            var metadataJson = payload.metadata_json
            if (!root.game || String(root.game.rom_id) !== String(romId)) return
            var metadata = {}
            try { metadata = JSON.parse(metadataJson) } catch(e) {}
            root._pendingMetadataRomId = String(romId)
            root.game = Object.assign({}, root.game, metadata)
            root._metadataLoading = false
        }
        function onConnectionStatusChanged() {
            if (root.game && root.game.rom_id && appBackend.isConnected) {
                appBackend.fetchRomMetadata(JSON.stringify(root.game))
            }
        }
        function onFavoriteToggleComplete(payload) {
            if (!root.game) return
            var isNowFavorite = payload.is_now_favorite
            root.game = Object.assign({}, root.game, { is_favorite: isNowFavorite ? "true" : "false" })
        }
    }

    Connections {
        target: gameBackend
        function onInstallProgress(bundle) {
            if (bundle.total > 0) root._installProgress = Math.min(1.0, Math.max(0.0, bundle.downloaded / bundle.total))
            root._installSpeed = bundle.speed
        }
        function onInstallComplete(bundle) {
            root._bannerText = bundle.message
            root._bannerSuccess = bundle.success
            bannerTimer.restart()
            if (bundle.success && bundle.game) {
                root.game = Object.assign({}, root.game, bundle.game)
            }
            root._leftButtonIndex = 0
        }
        function onUninstallComplete(bundle) {
            root._bannerText = bundle.message
            root._bannerSuccess = bundle.success
            bannerTimer.restart()
            if (bundle.success) {
                var updated = Object.assign({}, root.game)
                delete updated.local_path
                delete updated.extracted_path
                delete updated.archive_path
                root.game = updated
            }
            root._leftButtonIndex = 0
        }
        function onLaunchError(msg) {
            root._bannerText = msg
            root._bannerSuccess = false
            bannerTimer.restart()
        }
        function onSessionStarted(emulatorName) {
            root._bannerText = emulatorName ? "Launched with " + emulatorName : "Game launched"
            root._bannerSuccess = true
            bannerTimer.restart()
        }
        function onSessionEnded(emulatorName) {
            root._bannerText = "Session ended"
            root._bannerSuccess = true
            bannerTimer.restart()
        }
        function onNativeExecPickerNeeded(candidates) {
            nativeExecPicker.candidates = candidates
            nativeExecPicker.romId = root.game ? (root.game.rom_id || "") : ""
            nativeExecPicker.currentPath = root.game ? (root.game.native_executable_path || "") : ""
            nativeExecPicker.visible = true
        }
    }

    Timer {
        id: bannerTimer
        interval: 4000
        onTriggered: root._bannerText = ""
    }

    Component.onDestruction: {
        appBackend.logHandleDiag("details-close")
        bannerTimer.stop()
    }

    // Background fanart
    Image {
        anchors.fill: parent
        source: root.game.fanart_url ? "image://covers/" + root.game.fanart_url : (root._screenshotList.length > 0 ? "image://covers/" + root._screenshotList[0] : "")
        fillMode: Image.PreserveAspectCrop
        cache: false
        asynchronous: true
        opacity: 0.3
    }
    
    Rectangle {
        anchors.fill: parent
        color: "#282a36"
        opacity: 0.7
    }

    // Status banner
    Rectangle {
        width: parent.width
        height: 40
        anchors.top: parent.top
        anchors.topMargin: 48
        z: 10
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
                    text: root.game.title || root.game.name || "Details"
                    color: "#f8f8f2"
                    font.pixelSize: 16
                    font.bold: true
                    anchors.verticalCenter: parent.verticalCenter
                }
            }
        }

        // Main content
        Row {
            width: parent.width
            height: parent.height - 48
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
                        height: coverImage.implicitHeight > 0 && coverImage.implicitWidth > 0 ? Math.round(width * coverImage.implicitHeight / coverImage.implicitWidth) : width * 1.33
                        color: "#1e1f29"
                        radius: 8
                        clip: true

                        Image {
                            id: coverImage
                            width: parent.width
                            height: implicitHeight > 0 ? implicitWidth > 0 ? Math.round(width * implicitHeight / implicitWidth) : width * 1.33 : width * 1.33
                            source: root.game.cover_url ? "image://covers/" + root.game.cover_url : ""
                            fillMode: Image.PreserveAspectFit
                            cache: false
                            asynchronous: true
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
                        color: "#ff79c6"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? "#f8f8f2" : "transparent"
                        border.width: 2
                        visible: !!root._installedLocalPath && !gameBackend.isInstallActive

                        Text {
                            anchors.centerIn: parent
                            text: gameBackend.isSessionActive ? "▐▐  Playing..." : "▶  Play"
                            color: "#282a36"
                            font.pixelSize: 16
                            font.bold: true
                        }
                    }

                    // Install Button
                    Rectangle {
                        width: parent.width
                        height: 48
                        radius: 8
                        color: "#50fa7b"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? "#f8f8f2" : "transparent"
                        border.width: 2
                        visible: !root._installedLocalPath && appBackend.isConnected && !gameBackend.isInstallActive

                        Text {
                            anchors.centerIn: parent
                            text: "⬇  Install"
                            color: "#282a36"
                            font.pixelSize: 16
                            font.bold: true
                        }
                    }

                    // Install Progress + Cancel (Visible when installing)
                    Column {
                        width: parent.width
                        spacing: 8
                        visible: gameBackend.isInstallActive

                        Text {
                            text: {
                                var pct = Math.round(root._installProgress * 100) + "%"
                                var spd = root._installSpeed
                                var spdStr
                                if (spd >= 1048576) spdStr = (spd / 1048576).toFixed(1) + " MB/s"
                                else if (spd >= 1024) spdStr = (spd / 1024).toFixed(1) + " KB/s"
                                else spdStr = spd.toFixed(0) + " B/s"
                                return "Installing... " + pct + "  " + spdStr
                            }
                            color: "#f8f8f2"
                            font.pixelSize: 14
                        }

                        Rectangle {
                            id: track
                            width: parent.width
                            height: 6
                            color: "#44475a"
                            radius: 3
                            clip: true

                            Rectangle {
                                width: track.width * root._installProgress
                                height: parent.height
                                color: "#ff79c6"
                                radius: 3
                            }
                        }

                        // Cancel Button
                        Rectangle {
                            width: parent.width
                            height: 48
                            radius: 8
                            color: "#383a59"
                            border.color: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? "#ff79c6" : "#ff5555"
                            border.width: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? 2 : 1

                            Text {
                                anchors.centerIn: parent
                                text: "✕  Cancel"
                                color: (root._focusedColumn === 0 && root._leftButtonIndex === 0) ? "#f8f8f2" : "#ff5555"
                                font.pixelSize: 16
                                font.bold: true
                            }
                        }
                    }

                    // Uninstall Button
                    Rectangle {
                        property int myIndex: 1
                        width: parent.width
                        height: 48
                        radius: 8
                        color: "#383a59"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? "#ff79c6" : "#ff5555"
                        border.width: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? 2 : 1
                        visible: !!root._installedLocalPath

                        Text {
                            anchors.centerIn: parent
                            text: "✕  Uninstall"
                            color: (root._focusedColumn === 0 && root._leftButtonIndex === parent.myIndex) ? "#f8f8f2" : "#ff5555"
                            font.pixelSize: 16
                            font.bold: true
                        }
                    }

                    // Change Executable Button (native PC games only)
                    Rectangle {
                        property int myIndex: root._installedLocalPath ? 2 : -1
                        width: parent.width
                        height: 48
                        radius: 8
                        color: "#1e1f29"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? "#ff79c6" : "#bd93f9"
                        border.width: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? 2 : 1
                        visible: !!root._installedLocalPath && _isNativePcGame()

                        Text {
                            anchors.centerIn: parent
                            text: "⚙  Change Executable"
                            color: (root._focusedColumn === 0 && root._leftButtonIndex === parent.myIndex) ? "#f8f8f2" : "#bd93f9"
                            font.pixelSize: 16
                            font.bold: true
                        }
                    }

                    // Cloud Saves Button
                    Rectangle {
                        property int myIndex: root._installedLocalPath ? (_isNativePcGame() ? 3 : 2) : 1
                        width: parent.width
                        height: 48
                        radius: 8
                        color: "#1e1f29"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? "#ff79c6" : "#bd93f9"
                        border.width: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? 2 : 1
                        visible: appBackend.isConnected

                        Text {
                            anchors.centerIn: parent
                            text: "☁  Cloud Saves"
                            color: (root._focusedColumn === 0 && root._leftButtonIndex === parent.myIndex) ? "#f8f8f2" : "#bd93f9"
                            font.pixelSize: 16
                            font.bold: true
                        }
                    }

                    // Favorite Button
                    Rectangle {
                        property int myIndex: root._installedLocalPath ? (_isNativePcGame() ? 4 : 3) : 2
                        width: parent.width
                        height: 48
                        radius: 8
                        color: "#1e1f29"
                        border.color: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? "#ff79c6" : (root.game.is_favorite === "true" ? "#f1fa8c" : "#6272a4")
                        border.width: (root._focusedColumn === 0 && root._leftButtonIndex === myIndex) ? 2 : 1
                        visible: appBackend.isConnected

                        Text {
                            anchors.centerIn: parent
                            text: root.game.is_favorite === "true" ? "★  Remove from Favorites" : "☆  Add to Favorites"
                            color: (root._focusedColumn === 0 && root._leftButtonIndex === parent.myIndex) ? "#f8f8f2" : (root.game.is_favorite === "true" ? "#f1fa8c" : "#6272a4")
                            font.pixelSize: 16
                            font.bold: true
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

                Text {
                    id: detailsTitleText
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.topMargin: 16
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    text: root.game.title || root.game.name || ""
                    font.pixelSize: 32
                    font.bold: true
                    color: "#50fa7b"
                    wrapMode: Text.WordWrap
                    elide: Text.ElideRight
                    maximumLineCount: 2
                }

                // Zone B — Metadata Panel (fixed, always visible at bottom)
                Rectangle {
                    id: metadataPanel
                    anchors.bottom: parent.bottom
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottomMargin: 1
                    anchors.leftMargin: 1
                    anchors.rightMargin: 1
                    color: "#1e1f29"
                    height: metadataPanelColumn.implicitHeight + 24

                    Column {
                        id: metadataPanelColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 12
                        spacing: 8

                        Row {
                            spacing: 4
                            visible: !!root.game.platform
                            Text { text: "Platform:"; color: "#6272a4"; font.pixelSize: 14 }
                            Text { text: root.game.platform || ""; color: "#f8f8f2"; font.pixelSize: 14 }
                        }

                        Row {
                            spacing: 4
                            property string _releaseText: root.game.first_release_date || root.game.release_year || ""
                            visible: _releaseText !== ""
                            Text { text: "Released:"; color: "#6272a4"; font.pixelSize: 14 }
                            Text { text: parent._releaseText; color: "#f8f8f2"; font.pixelSize: 14 }
                        }

                        Row {
                            spacing: 4
                            visible: !!root.game.companies
                            Text { text: "By:"; color: "#6272a4"; font.pixelSize: 14 }
                            Text {
                                text: root.game.companies || ""
                                color: "#f8f8f2"
                                font.pixelSize: 14
                                wrapMode: Text.Wrap
                                width: metadataPanel.width - 80
                            }
                        }

                        Row {
                            spacing: 4
                            visible: !!root.game.revision && root.game.revision !== ""
                            Text { text: "Version:"; color: "#6272a4"; font.pixelSize: 14 }
                            Text { text: root.game.revision || ""; color: "#f8f8f2"; font.pixelSize: 14 }
                        }

                        Row {
                            spacing: 4
                            visible: !!root.game.filesize_bytes
                            Text { text: "Size:"; color: "#6272a4"; font.pixelSize: 14 }
                            Text { text: root.formatFilesize(root.game.filesize_bytes); color: "#f8f8f2"; font.pixelSize: 14 }
                        }

                        Row {
                            spacing: 4
                            property string _ratingRaw: root.game.average_rating || root.game.rating || ""
                            property real _ratingNum: {
                                var s = _ratingRaw
                                var slashIdx = s.indexOf("/")
                                if (slashIdx !== -1) s = s.substring(0, slashIdx)
                                return parseFloat(s) || 0
                            }
                            visible: _ratingRaw !== "" && _ratingRaw !== "N/A"
                            Text { text: "Rating:"; color: "#6272a4"; font.pixelSize: 14 }
                            Repeater {
                                model: 5
                                delegate: Text {
                                    text: "★"
                                    color: (index + 1) <= Math.round(parent.parent._ratingNum) ? "#ff79c6" : "#44475a"
                                    font.pixelSize: 16
                                }
                            }
                            Text { text: parent._ratingNum.toFixed(1) + "/5"; color: "#6272a4"; font.pixelSize: 13 }
                        }

                        Row {
                            spacing: 4
                            visible: !!root.game.regions && root.game.regions.length > 0
                            Text { text: "Region:"; color: "#6272a4"; font.pixelSize: 14 }
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

                        Row {
                            spacing: 6
                            visible: !!root.game.genres && typeof(root.game.genres) === "string" && root.game.genres.length > 0
                            Text { text: "Genres:"; color: "#6272a4"; font.pixelSize: 14; anchors.verticalCenter: parent.verticalCenter }
                            Flow {
                                width: metadataPanel.width - 80
                                spacing: 6
                                Repeater {
                                    model: (root.game.genres || "").split(",").filter(function(s) { return s.trim() !== ""; })
                                    delegate: Rectangle {
                                        color: "#383a59"
                                        radius: 10
                                        width: genreChipText.width + 16
                                        height: genreChipText.height + 8
                                        Text {
                                            id: genreChipText
                                            anchors.centerIn: parent
                                            text: modelData.trim()
                                            color: "#bd93f9"
                                            font.pixelSize: 12
                                        }
                                    }
                                }
                            }
                        }

                        Text {
                            id: metadataLoadingText
                            opacity: root._metadataLoading ? 1.0 : 0.0
                            visible: opacity > 0
                            text: "Loading metadata\u2026"
                            color: "#6272a4"
                            font.pixelSize: 12
                            anchors.horizontalCenter: parent.horizontalCenter
                            topPadding: 4

                            Behavior on opacity { NumberAnimation { duration: 200 } }
                        }
                    }
                }

                // Zone A — Description Flickable (scrollable, fills between title and metadata panel)
                Flickable {
                    id: descFlickable
                    anchors.top: detailsTitleText.bottom
                    anchors.topMargin: 8
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: metadataPanel.top
                    anchors.bottomMargin: 0
                    anchors.leftMargin: 1
                    anchors.rightMargin: 1
                    clip: true
                    contentHeight: descColumn.implicitHeight

                    Column {
                        id: descColumn
                        width: descFlickable.width
                        padding: 16
                        spacing: 8

                        Text {
                            text: root.game.summary || root.game.description || "No description available."
                            color: "#f8f8f2"
                            wrapMode: Text.Wrap
                            font.pixelSize: 14
                            width: parent.width - 32
                        }

                    }
                }

                // Scroll indicator
                Rectangle {
                    anchors.right: parent.right
                    width: 3
                    color: "#44475a"
                    opacity: 0.6
                    y: descFlickable.visibleArea.yPosition * descFlickable.height
                    height: descFlickable.visibleArea.heightRatio * descFlickable.height
                    visible: descFlickable.contentHeight > descFlickable.height
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
                                    source: root._screenshotList[index] ? "image://covers/" + root._screenshotList[index] : ""
                                    fillMode: Image.PreserveAspectCrop
                                    cache: false
                                    asynchronous: true
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

    NativeExecPickerDialog {
        id: nativeExecPicker
        visible: false
        anchors.fill: parent
        romId: root.game ? (root.game.rom_id || "") : ""
        candidates: []
        currentPath: ""
        onClosed: visible = false
    }
}