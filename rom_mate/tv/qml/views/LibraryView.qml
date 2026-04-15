import QtQuick
import QtQuick.Controls
import "../components"

Item {
    id: root

    property var stackView: null
    property var outerStackRef: null

    // Header bar
    Rectangle {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: 48
        color: "#1e1f29"

        Text {
            anchors.centerIn: parent
            text: "Library"
            color: "#f8f8f2"
            font.pixelSize: 18
            font.bold: true
        }

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            height: 1
            color: "#44475a"
        }
    }

    // Game wall
    GameWall {
        id: wall
        anchors.top: header.bottom
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