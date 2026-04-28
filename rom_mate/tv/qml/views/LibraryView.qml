import QtQuick
import QtQuick.Controls
import "../components"

Item {
    id: root

    property var stackView: null
    property var outerStackRef: null

    // Game wall
    GameWall {
        id: wall
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom

        games: appBackend.libraryGames
        navigationActive: !appBackend.uiOverlayActive && (!root.outerStackRef || root.outerStackRef.depth <= 1)

        onGameSelected: function(game) {
            if (root.outerStackRef) {
                root.outerStackRef.push(detailsViewComponent, { game: game })
            }
        }
    }

    // Reload when library changes
    Connections {
        target: appBackend
        function onLibraryGamesChanged() {
            wall.games = appBackend.libraryGames
        }
    }

    // Empty state when no games installed
    Text {
        anchors.centerIn: parent
        visible: appBackend.libraryGames.length === 0
        text: "No games installed yet"
        color: "#6272a4"
        font.pixelSize: 22
    }
}