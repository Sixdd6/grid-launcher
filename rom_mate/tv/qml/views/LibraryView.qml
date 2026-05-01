import QtQuick
import "../components"

Item {
    id: root

    property var stackView: null
    property var outerStackRef: null
    property bool navigationActive: !appBackend.uiOverlayActive && !pauseBackend.visible && (!root.outerStackRef || root.outerStackRef.depth <= 1)

    signal gameSelected(var game)

    GridView {
        id: grid
        anchors.fill: parent
        anchors.margins: 40
        anchors.rightMargin: 56
        cacheBuffer: 0
        cellWidth: Math.floor(width / 5)
        cellHeight: Math.ceil((cellWidth - 40) * 1.5) + 50 + 60
        clip: true
        keyNavigationEnabled: true
        keyNavigationWraps: false
        model: []

        delegate: Item {
            width: grid.cellWidth
            height: grid.cellHeight

            required property var modelData
            required property int index

            LibraryCard {
                anchors.centerIn: parent
                width: grid.cellWidth - 40

                coverUrl: parent.modelData.cover_url || ""
                gameTitle: parent.modelData.title || ""
                platform: parent.modelData.platform || ""
                releaseYear: parent.modelData.release_year || ""
                cloudSaveEnabled: (parent.modelData.rom_id !== undefined && parent.modelData.rom_id !== "")
                hasSaves: (parent.modelData.has_cloud_saves === "true")
                isFavorite: (parent.modelData.is_favorite === "true")

                focus: grid.currentIndex === parent.index
                isFocused: grid.currentIndex === parent.index

                onSelected: root.gameSelected(parent.modelData)

                Keys.onUpPressed: {
                    if (grid.currentIndex >= 5) {
                        grid.currentIndex -= 5
                    }
                }
                Keys.onDownPressed: {
                    if (grid.currentIndex + 5 < grid.count) {
                        grid.currentIndex += 5
                    } else if (grid.currentIndex < grid.count - 1) {
                        grid.currentIndex = grid.count - 1
                    }
                }
                Keys.onLeftPressed: {
                    if (grid.currentIndex > 0) {
                        grid.currentIndex -= 1
                    }
                }
                Keys.onRightPressed: {
                    if (grid.currentIndex < grid.count - 1) {
                        grid.currentIndex += 1
                    }
                }
            }
        }
    }

    Rectangle {
        anchors.top: grid.top
        anchors.bottom: grid.bottom
        anchors.right: root.right
        anchors.rightMargin: 16
        width: 6
        radius: 3
        color: "#44475a"
        visible: grid.visibleArea.heightRatio > 0.0 && grid.visibleArea.heightRatio < 1.0

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            y: grid.visibleArea.yPosition * grid.height
            height: grid.visibleArea.heightRatio * grid.height
            radius: 3
            color: grid.moving ? "#ff79c6" : "#6272a4"
        }
    }

    Connections {
        target: controllerBackend
        enabled: root.navigationActive
        
        function onNavigationEvent(event) {
            if (event === "up") {
                grid.moveCurrentIndexUp()
            } else if (event === "down") {
                grid.moveCurrentIndexDown()
            } else if (event === "left") {
                grid.moveCurrentIndexLeft()
            } else if (event === "right") {
                grid.moveCurrentIndexRight()
            } else if (event === "confirm") {
                if (grid.currentIndex >= 0 && grid.currentIndex < grid.count) {
                    root.gameSelected(appBackend.libraryGames[grid.currentIndex])
                }
            }
        }
    }

    onGameSelected: function(game) {
        if (root.outerStackRef) {
            root.outerStackRef.push(detailsViewComponent, { game: game })
        }
    }

    Connections {
        target: appBackend
        function onLibraryGamesChanged() {
            if (root.visible) {
                grid.model = appBackend.libraryGames
            }
        }
    }

    Component.onCompleted: {
        grid.model = appBackend.libraryGames
    }

    onVisibleChanged: {
        if (visible) {
            grid.model = appBackend.libraryGames
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