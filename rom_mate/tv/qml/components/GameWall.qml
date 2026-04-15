import QtQuick
import QtQuick.Controls

Item {
    id: root
    anchors.fill: parent

    property var games: []
    property int columns: 6
    property bool navigationActive: false

    signal gameSelected(var game)

    GridView {
        id: grid
        anchors.fill: parent
        anchors.margins: 16
        cellWidth:  Math.floor(width / root.columns)
        cellHeight: 290
        clip: true
        focus: true
        keyNavigationEnabled: true
        keyNavigationWraps: false

        model: root.games

        delegate: Item {
            width:  grid.cellWidth
            height: grid.cellHeight

            required property var modelData
            required property int index

            GameCard {
                anchors.centerIn: parent
                width:  grid.cellWidth - 16
                height: grid.cellHeight - 20
                coverUrl:  modelData.cover_url  || ""
                gameTitle: modelData.title      || ""
                focus: grid.currentIndex === index
                isFocused: grid.currentIndex === index

                Keys.onUpPressed: {
                    if (index >= root.columns) grid.currentIndex = index - root.columns
                    event.accepted = true
                }
                Keys.onDownPressed: {
                    if (index + root.columns < root.games.length) grid.currentIndex = index + root.columns
                    event.accepted = true
                }
                Keys.onLeftPressed: {
                    if (index > 0) grid.currentIndex = index - 1
                    event.accepted = true
                }
                Keys.onRightPressed: {
                    if (index < root.games.length - 1) grid.currentIndex = index + 1
                    event.accepted = true
                }

                onSelected: root.gameSelected(modelData)
            }
        }

        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
    }

    Text {
        anchors.centerIn: parent
        visible: root.games.length === 0
        text: "No games found"
        color: "#6272a4"
        font.pixelSize: 20
    }

    Connections {
        target: controllerBackend
        enabled: root.navigationActive

        function onNavigationEvent(direction) {
            if (direction === "up")    { grid.moveCurrentIndexUp();    return }
            if (direction === "down")  { grid.moveCurrentIndexDown();  return }
            if (direction === "left")  { grid.moveCurrentIndexLeft();  return }
            if (direction === "right") { grid.moveCurrentIndexRight(); return }
            if (direction === "confirm") {
                var idx = grid.currentIndex
                if (idx >= 0 && idx < root.games.length)
                    root.gameSelected(root.games[idx])
            }
        }
    }
}