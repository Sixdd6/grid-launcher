import QtQuick
import QtQuick.Controls

Item {
    id: root
    width: 380
    height: 130

    property string coverUrl: ""
    property string gameTitle: ""
    property string platform: ""
    property string releaseYear: ""
    property string genres: ""
    property bool isFocused: false

    signal selected()

    function _firstGenre() {
        if (!root.genres) return ""
        var parts = root.genres.split(",")
        return parts.length > 0 ? parts[0].trim() : ""
    }

    scale: root.isFocused ? 1.04 : 1.0

    // Outer glow — extends OUTSIDE the card body, centered on Item
    Rectangle {
        anchors.centerIn: parent
        width: parent.width + 20
        height: parent.height + 20
        radius: 14
        color: "#ff79c6"
        opacity: root.isFocused ? 0.15 : 0.0
    }

    // Inner glow
    Rectangle {
        anchors.centerIn: parent
        width: parent.width + 10
        height: parent.height + 10
        radius: 11
        color: "#ff79c6"
        opacity: root.isFocused ? 0.25 : 0.0
    }

    // Main card body — layer.enabled clips image to rounded corners
    Rectangle {
        id: cardBody
        anchors.fill: parent
        radius: 8
        color: "#1e1f29"
        border.color: root.isFocused ? "#ff79c6" : "#44475a"
        border.width: root.isFocused ? 2 : 1

        Image {
            id: coverImage
            width: 96
            anchors {
                left: parent.left
                top: parent.top
                bottom: parent.bottom
                leftMargin: 6
                rightMargin: 10
                topMargin: 6
                bottomMargin: 6
            }
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            smooth: true
            source: root.coverUrl ? ("image://covers/" + root.coverUrl) : ""

            Rectangle {
                anchors.fill: parent
                color: "#282a36"
                visible: coverImage.status !== Image.Ready

                Text {
                    anchors.centerIn: parent
                    text: "?"
                    color: "#6272a4"
                    font.pixelSize: 28
                }
            }
        }

        Rectangle {
            id: separator
            width: 1
            color: "#44475a"
            anchors {
                left: coverImage.right
                top: parent.top
                bottom: parent.bottom
                leftMargin: 10
                topMargin: 8
                bottomMargin: 8
            }
        }

        Column {
            anchors {
                left: separator.right
                right: parent.right
                top: parent.top
                bottom: parent.bottom
                leftMargin: 10
                rightMargin: 14
                topMargin: 12
                bottomMargin: 12
            }
            spacing: 6

            Text {
                width: parent.width
                text: root.gameTitle
                color: "#f8f8f2"
                font.pixelSize: 16
                font.bold: true
                elide: Text.ElideRight
                wrapMode: Text.NoWrap
            }

            Rectangle {
                height: 22
                width: platformText.width + 14
                color: "#383a59"
                radius: 5
                border.color: "#6272a4"
                border.width: 1
                visible: root.platform !== ""

                Text {
                    id: platformText
                    anchors.centerIn: parent
                    text: root.platform
                    color: "#f8f8f2"
                    font.pixelSize: 11
                }
            }

            Text {
                width: parent.width
                text: root.releaseYear !== "" ? root.releaseYear : "Unavailable"
                color: root.releaseYear !== "" ? "#6272a4" : "#44475a"
                font.pixelSize: 12
                elide: Text.ElideRight
                wrapMode: Text.NoWrap
            }

            Text {
                width: parent.width
                text: root._firstGenre() !== "" ? root._firstGenre() : "Unavailable"
                color: root._firstGenre() !== "" ? "#6272a4" : "#44475a"
                font.pixelSize: 12
                elide: Text.ElideRight
                wrapMode: Text.NoWrap
            }
        }
    }

    Keys.onReturnPressed: root.selected()
    Keys.onEnterPressed:  root.selected()

    focus: true
}