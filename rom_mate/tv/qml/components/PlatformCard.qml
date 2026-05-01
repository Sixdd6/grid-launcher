import QtQuick
import QtQuick.Controls

Rectangle {
    id: root
    width: 260
    height: 150
    radius: 6

    property string platformName: ""
    property string manufacturer: ""
    property string releaseYear: ""
    property string playerCount: ""
    property int romCount: 0
    property string logoUrl: ""
    property bool isFocused: activeFocus

    signal selected()

    color: isFocused ? "#282a36" : "#1e1f29"
    border.color: isFocused ? "#ff79c6" : "#44475a"
    border.width: isFocused ? 2 : 1

    scale: isFocused ? 1.05 : 1.0
    
    Image {
        id: logoImage
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        anchors.margins: 8
        width: height * 1.2
        fillMode: Image.PreserveAspectFit
        source: root.logoUrl
        visible: root.logoUrl !== ""
        smooth: true
        mipmap: true
        opacity: 0.85
    }

    Item {
        anchors.fill: parent
        anchors.margins: 12

        Column {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.rightMargin: root.logoUrl !== "" ? logoImage.width + 4 : 0
            spacing: 4

            Text {
                width: parent.width
                text: root.platformName
                font.pixelSize: 16
                font.bold: true
                color: "#f8f8f2"
                elide: Text.ElideRight
                wrapMode: Text.WordWrap
                maximumLineCount: 2
            }

            Text {
                width: parent.width
                text: root.manufacturer
                visible: root.manufacturer !== ""
                font.pixelSize: 12
                color: "#bd93f9"
            }

            Text {
                width: parent.width
                property var parts: {
                    var p = []
                    if (root.releaseYear !== "") p.push(root.releaseYear)
                    if (root.playerCount !== "") p.push(root.playerCount)
                    return p
                }
                text: parts.join(" · ")
                visible: parts.length > 0
                font.pixelSize: 12
                color: "#6272a4"
            }
        }

        Text {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            text: root.romCount + " games"
            font.pixelSize: 11
            color: "#6272a4"
        }
    }

    Keys.onReturnPressed: root.selected()
    Keys.onEnterPressed: root.selected()

    focus: true
}
