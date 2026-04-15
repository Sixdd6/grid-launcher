import QtQuick
import QtQuick.Controls

Item {
    id: root
    width: parent ? parent.width : 800
    height: 120

    property var platformLabels: []
    property bool navigationActive: false

    signal platformSelected(string label)

    ListView {
        id: listView
        anchors.fill: parent
        leftMargin: 24
        rightMargin: 24
        orientation: ListView.Horizontal
        spacing: 16
        clip: true
        focus: true
        keyNavigationEnabled: true
        keyNavigationWraps: false

        model: root.platformLabels

        delegate: Item {
            required property string modelData
            required property int index

            width:  160
            height: listView.height

            Rectangle {
                anchors.fill: parent
                anchors.margins: 4
                radius: 10
                color: listView.currentIndex === index ? "#44475a" : "#1e1f29"
                border.color: listView.currentIndex === index ? "#ff79c6" : "#44475a"
                border.width: listView.currentIndex === index ? 2 : 1

                Behavior on color        { ColorAnimation { duration: 150 } }
                Behavior on border.color { ColorAnimation { duration: 150 } }

                scale: listView.currentIndex === index ? 1.08 : 1.0
                Behavior on scale { NumberAnimation { duration: 150 } }

                Text {
                    anchors.centerIn: parent
                    width: parent.width - 16
                    text: modelData
                    color: listView.currentIndex === index ? "#f8f8f2" : "#bd93f9"
                    font.pixelSize: listView.currentIndex === index ? 14 : 12
                    font.bold: listView.currentIndex === index
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    elide: Text.ElideRight
                    maximumLineCount: 3

                    Behavior on color      { ColorAnimation { duration: 150 } }
                    Behavior on font.pixelSize { NumberAnimation { duration: 150 } }
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        listView.currentIndex = index
                        root.platformSelected(modelData)
                    }
                }
            }

            Keys.onReturnPressed: root.platformSelected(modelData)
            Keys.onEnterPressed:  root.platformSelected(modelData)
            Keys.onLeftPressed: {
                if (index > 0) { listView.currentIndex = index - 1; event.accepted = true }
            }
            Keys.onRightPressed: {
                if (index < root.platformLabels.length - 1) { listView.currentIndex = index + 1; event.accepted = true }
            }
        }

        onCurrentIndexChanged: {
            positionViewAtIndex(currentIndex, ListView.Contain)
        }
    }

    Text {
        anchors.centerIn: parent
        visible: root.platformLabels.length === 0
        text: appBackend.isConnected ? "Loading platforms..." : "Not connected to server"
        color: "#6272a4"
        font.pixelSize: 16
    }

    Connections {
        target: controllerBackend
        enabled: root.navigationActive

        function onNavigationEvent(direction) {
            if (direction === "left") {
                if (listView.currentIndex > 0) {
                    listView.currentIndex -= 1
                }
            } else if (direction === "right") {
                if (listView.currentIndex < root.platformLabels.length - 1) {
                    listView.currentIndex += 1
                }
            } else if (direction === "confirm") {
                var label = root.platformLabels[listView.currentIndex]
                if (label) root.platformSelected(label)
            }
        }
    }
}