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

    // Platform carousel (visible when no platform selected)
    PlatformCarousel {
        id: carousel
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        visible: root._selectedPlatform === ""
        platformLabels: appBackend.platforms
        navigationActive: !appBackend.uiOverlayActive && !pauseBackend.visible && root._selectedPlatform === "" && (!root.outerStackRef || root.outerStackRef.depth <= 1)

        onPlatformSelected: function(label) {
            root._selectPlatform(label)
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

    // Loading indicator
    BusyIndicator {
        anchors.centerIn: parent
        visible: root._loadingGames
        running: root._loadingGames
    }

    // Game wall (visible when platform selected and games loaded)
    GameWall {
        id: gameWall
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        visible: root._selectedPlatform !== "" && !root._loadingGames
        games: root._platformGames
        navigationActive: !appBackend.uiOverlayActive && !pauseBackend.visible && (!root.outerStackRef || root.outerStackRef.depth <= 1)

        onGameSelected: function(game) {
            if (root.outerStackRef) {
                root.outerStackRef.push(detailsViewComponent, { game: game })
            }
        }
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
        function onPlatformsChanged() {
            carousel.platformLabels = appBackend.platforms
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

    // Controller back navigates to carousel when in game wall
    Connections {
        target: controllerBackend
        function onNavigationEvent(direction) {
            if (direction === "back" && root._selectedPlatform !== "") {
                root._clearPlatformSelection()
            }
        }
    }
}