import QtQuick
import QtQuick.Controls
import "../components"

Item {
    id: root

    property var stackView: null
    property var outerStackRef: null
    property string _selectedPlatform: ""
    property var _platformGames: []
    property bool _loadingGames: false
    property bool navigationActive: !appBackend.uiOverlayActive && !pauseBackend.visible && (!root.outerStackRef || root.outerStackRef.depth <= 1)

    // Platform grid (visible when no platform selected)
    GridView {
        id: platformGrid
        anchors.fill: parent
        anchors.margins: 40
        anchors.rightMargin: 56
        cacheBuffer: 0
        cellWidth: Math.floor(width / 4)
        cellHeight: 170
        clip: false
        visible: root._selectedPlatform === ""
        focus: root._selectedPlatform === ""
        keyNavigationEnabled: true
        keyNavigationWraps: false
        model: appBackend.platformDetails

        delegate: Item {
            width: platformGrid.cellWidth
            height: platformGrid.cellHeight

            required property var modelData
            required property int index

            PlatformCard {
                anchors.centerIn: parent
                width: platformGrid.cellWidth - 24
                height: 150

                platformName: parent.modelData.name || ""
                manufacturer: parent.modelData.manufacturer || ""
                releaseYear: parent.modelData.release_year ? String(parent.modelData.release_year) : ""
                playerCount: parent.modelData.player_count ? String(parent.modelData.player_count) : ""
                romCount: parent.modelData.rom_count || 0
                logoUrl: parent.modelData.local_logo_path
                    || (parent.modelData.url_logo ? "image://covers/" + parent.modelData.url_logo : "")

                focus: platformGrid.currentIndex === parent.index
                isFocused: platformGrid.currentIndex === parent.index

                onSelected: {
                    appBackend.logHandleDiag("platform-selected")
                    root._selectPlatform(parent.modelData.name)
                }
            }
        }
    }

    Rectangle {
        anchors.top: platformGrid.top
        anchors.bottom: platformGrid.bottom
        anchors.right: root.right
        anchors.rightMargin: 16
        width: 6
        radius: 3
        color: "#44475a"
        visible: platformGrid.visible && platformGrid.visibleArea.heightRatio > 0.0 && platformGrid.visibleArea.heightRatio < 1.0

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            y: platformGrid.visibleArea.yPosition * platformGrid.height
            height: platformGrid.visibleArea.heightRatio * platformGrid.height
            radius: 3
            color: platformGrid.moving ? "#ff79c6" : "#6272a4"
        }
    }

    Connections {
        target: controllerBackend
        enabled: root.navigationActive
        
        function onNavigationEvent(event) {
            if (outerStackRef && outerStackRef.depth > 1) return
            if (root._selectedPlatform === "") {
                if (event === "up") {
                    platformGrid.moveCurrentIndexUp()
                } else if (event === "down") {
                    platformGrid.moveCurrentIndexDown()
                } else if (event === "left") {
                    platformGrid.moveCurrentIndexLeft()
                } else if (event === "right") {
                    platformGrid.moveCurrentIndexRight()
                } else if (event === "confirm") {
                    if (appBackend.platformDetails && appBackend.platformDetails.length > 0 && platformGrid.currentIndex >= 0 && platformGrid.currentIndex < platformGrid.count) {
                        root._selectPlatform(appBackend.platformDetails[platformGrid.currentIndex].name)
                    }
                }
            } else {
                if (event === "back") {
                    root._clearPlatformSelection()
                } else if (event === "up") {
                    gameGrid.moveCurrentIndexUp()
                } else if (event === "down") {
                    gameGrid.moveCurrentIndexDown()
                } else if (event === "left") {
                    gameGrid.moveCurrentIndexLeft()
                } else if (event === "right") {
                    gameGrid.moveCurrentIndexRight()
                } else if (event === "confirm") {
                    if (gameGrid.currentIndex >= 0 && gameGrid.currentIndex < root._platformGames.length) {
                        if (root.outerStackRef) {
                            root.outerStackRef.push(detailsViewComponent, { game: root._platformGames[gameGrid.currentIndex] })
                        }
                    }
                }
            }
        }
    }

    // Connection prompt when offline and no platform selected
    Text {
        anchors.centerIn: parent
        visible: !appBackend.isConnected && root._selectedPlatform === ""
        text: "Connect to a RomM server to browse games"
        color: "#6272a4"
        font.pixelSize: 18
    }

    // Empty state when connected but no platforms
    Text {
        anchors.centerIn: parent
        visible: appBackend.isConnected && (!appBackend.platformDetails || appBackend.platformDetails.length === 0) && root._selectedPlatform === ""
        text: "Loading platforms..."
        color: "#6272a4"
        font.pixelSize: 18
    }

    // Loading indicator
    BusyIndicator {
        anchors.centerIn: parent
        visible: root._loadingGames
        running: root._loadingGames
    }

    GridView {
        id: gameGrid
        anchors.fill: parent
        anchors.margins: 40
        anchors.rightMargin: 56
        cacheBuffer: 0
        cellWidth: Math.floor(width / 5)
        cellHeight: Math.ceil((cellWidth - 40) * 1.5) + 50 + 60
        clip: true
        visible: root._selectedPlatform !== "" && !root._loadingGames
        focus: root._selectedPlatform !== ""
        keyNavigationEnabled: true
        keyNavigationWraps: false
        model: root._platformGames

        delegate: Item {
            width: gameGrid.cellWidth
            height: gameGrid.cellHeight

            required property var modelData
            required property int index

            LibraryCard {
                anchors.centerIn: parent
                width: gameGrid.cellWidth - 40

                coverUrl: parent.modelData.cover_url || ""
                gameTitle: parent.modelData.title || ""
                platform: parent.modelData.platform || ""
                releaseYear: parent.modelData.release_year ? String(parent.modelData.release_year) : ""
                cloudSaveEnabled: (parent.modelData.rom_id !== undefined && parent.modelData.rom_id !== "")
                hasSaves: (parent.modelData.has_cloud_saves === "true")
                isFavorite: (parent.modelData.is_favorite === "true")

                focus: gameGrid.currentIndex === parent.index
                isFocused: gameGrid.currentIndex === parent.index

                onSelected: {
                    appBackend.logHandleDiag("game-card-tapped")
                    if (root.outerStackRef) {
                        root.outerStackRef.push(detailsViewComponent, { game: parent.modelData })
                    }
                }

                Keys.onUpPressed: {
                    if (gameGrid.currentIndex >= 5) gameGrid.currentIndex -= 5
                }
                Keys.onDownPressed: {
                    if (gameGrid.currentIndex + 5 < gameGrid.count) {
                        gameGrid.currentIndex += 5
                    } else if (gameGrid.currentIndex < gameGrid.count - 1) {
                        gameGrid.currentIndex = gameGrid.count - 1
                    }
                }
                Keys.onLeftPressed: {
                    if (gameGrid.currentIndex > 0) gameGrid.currentIndex -= 1
                }
                Keys.onRightPressed: {
                    if (gameGrid.currentIndex < gameGrid.count - 1) gameGrid.currentIndex += 1
                }
            }
        }
    }

    Rectangle {
        anchors.top: gameGrid.top
        anchors.bottom: gameGrid.bottom
        anchors.right: root.right
        anchors.rightMargin: 16
        width: 6
        radius: 3
        color: "#44475a"
        visible: gameGrid.visible && gameGrid.visibleArea.heightRatio > 0.0 && gameGrid.visibleArea.heightRatio < 1.0

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            y: gameGrid.visibleArea.yPosition * gameGrid.height
            height: gameGrid.visibleArea.heightRatio * gameGrid.height
            radius: 3
            color: gameGrid.moving ? "#ff79c6" : "#6272a4"
        }
    }

    Text {
        anchors.centerIn: parent
        visible: root._selectedPlatform !== "" && !root._loadingGames && root._platformGames.length === 0
        text: "No games found for this platform"
        color: "#6272a4"
        font.pixelSize: 22
    }

    // React to async ROM list arriving
    Connections {
        target: appBackend
        function onServerGamesChanged(platformLabel) {
            if (platformLabel === root._selectedPlatform) {
                root._platformGames = appBackend.serverGamesForPlatform(platformLabel)
                root._loadingGames = false
            }
        }
    }

    function _selectPlatform(label) {
        root._selectedPlatform = label
        root._platformGames = appBackend.serverGamesForPlatform(label)
        if (root._platformGames.length === 0) {
            root._loadingGames = true
            appBackend.loadPlatformGames(label)
        }
    }

    function _clearPlatformSelection() {
        root._selectedPlatform = ""
        root._platformGames = []
        root._loadingGames = false
    }
}
