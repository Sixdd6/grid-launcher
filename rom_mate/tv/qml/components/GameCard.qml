import QtQuick
import QtQuick.Controls

Item {
    id: root
    width: 180
    height: 260

    property string coverUrl: ""
    property string gameTitle: ""
    property bool isActive: activeFocus
    property bool isFocused: false

    signal selected()

    scale: (activeFocus || root.isFocused) ? 1.05 : 1.0
    Behavior on scale { NumberAnimation { duration: 80 } }

    // Outer glow
    Rectangle {
        anchors.centerIn: parent
        width: parent.width + 24
        height: parent.height + 24
        radius: 14
        color: "#ff79c6"
        opacity: (activeFocus || root.isFocused) ? 0.15 : 0.0
        Behavior on opacity { NumberAnimation { duration: 120 } }
    }

    // Inner glow
    Rectangle {
        anchors.centerIn: parent
        width: parent.width + 12
        height: parent.height + 12
        radius: 11
        color: "#ff79c6"
        opacity: (activeFocus || root.isFocused) ? 0.25 : 0.0
        Behavior on opacity { NumberAnimation { duration: 120 } }
    }

    Rectangle {
        anchors.fill: parent
        radius: 8
        color: "#282a36"
        border.color: (root.activeFocus || root.isFocused) ? "#ff79c6" : "#44475a"
        border.width: (root.activeFocus || root.isFocused) ? 2 : 1

        Behavior on border.color { ColorAnimation { duration: 60 } }

        Image {
            id: coverImage
            anchors {
                top: parent.top
                left: parent.left
                right: parent.right
                bottom: titleText.top
                bottomMargin: 6
            }
            source: root.coverUrl ? ("image://covers/" + root.coverUrl) : ""
            fillMode: Image.PreserveAspectCrop
            clip: true
            asynchronous: true
            smooth: true

            Rectangle {
                anchors.fill: parent
                color: (root.activeFocus || root.isFocused)
                visible: coverImage.status !== Image.Ready
                radius: 6

                Text {
                    anchors.centerIn: parent
                    text: "?"
                    color: "#6272a4"
                    font.pixelSize: 32
                }
            }
        }

        Text {
            id: titleText
            anchors {
                left: parent.left
                right: parent.right
                bottom: parent.bottom
                margins: 8
                bottomMargin: 10
            }
            text: root.gameTitle
            color: "#f8f8f2"
            font.pixelSize: 12
            font.bold: root.activeFocus
            elide: Text.ElideRight
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.NoWrap
        }
    }

    Keys.onReturnPressed: root.selected()
    Keys.onEnterPressed:  root.selected()

    focus: true
}