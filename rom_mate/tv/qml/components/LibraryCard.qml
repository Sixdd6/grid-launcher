import QtQuick
import QtQuick.Controls

Item {
    id: root
    width: 200
    height: coverArea.height + 50

    property string coverUrl: ""
    property string gameTitle: ""
    property string platform: ""
    property string releaseYear: ""
    property bool cloudSaveEnabled: false
    property bool hasSaves: false
    property bool isFavorite: false
    property bool isFocused: false

    signal selected()

    scale: (activeFocus || root.isFocused) ? 1.05 : 1.0

    Rectangle {
        anchors.centerIn: parent
        width: parent.width + 24
        height: parent.height + 24
        radius: 14
        color: "#ff79c6"
        opacity: (activeFocus || root.isFocused) ? 0.15 : 0.0
    }

    Rectangle {
        anchors.centerIn: parent
        width: parent.width + 12
        height: parent.height + 12
        radius: 11
        color: "#ff79c6"
        opacity: (activeFocus || root.isFocused) ? 0.25 : 0.0
    }

    Rectangle {
        anchors.fill: parent
        radius: 8
        color: "#1e1f29"
        border.color: (root.activeFocus || root.isFocused) ? "#ff79c6" : "#44475a"
        border.width: (root.activeFocus || root.isFocused) ? 2 : 1
        clip: true

        Item {
            id: coverArea
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: coverImage.status === Image.Ready
                ? Math.round(width * coverImage.sourceSize.height / coverImage.sourceSize.width)
                : width

            Image {
                id: coverImage
                anchors.fill: parent
                source: root.coverUrl ? ("image://covers/" + root.coverUrl) : ""
                fillMode: Image.Stretch
                asynchronous: true
                smooth: true

                Rectangle {
                    anchors.fill: parent
                    color: "#1e1f29"
                    visible: coverImage.status !== Image.Ready

                    Text {
                        anchors.centerIn: parent
                        text: "?"
                        color: "#6272a4"
                        font.pixelSize: 32
                    }
                }
            }
        }
        
        // Favorite badge
        Rectangle {
            width: 18
            height: 18
            radius: 9
            visible: root.isFavorite
            color: "#282a36"
            border.color: "#f1fa8c"
            border.width: 2
            anchors.top: coverArea.top
            anchors.left: coverArea.left
            anchors.topMargin: 8
            anchors.leftMargin: 8

            Text {
                text: "\u2605"
                color: "#f1fa8c"
                font.pixelSize: 10
                anchors.centerIn: parent
            }
        }

        // Cloud save badge
        Rectangle {
            width: 18
            height: 18
            radius: 9
            visible: root.hasSaves
            color: "#282a36"
            border.color: "#50fa7b"
            border.width: 2
            anchors.top: coverArea.top
            anchors.right: coverArea.right
            anchors.topMargin: 8
            anchors.rightMargin: 8
            
            Text {
                text: "☁"
                color: "#50fa7b"
                font.pixelSize: 10
                anchors.centerIn: parent
                anchors.verticalCenterOffset: -1 // Tweaked for visual centering
            }
        }

        // Info footer
        Item {
            anchors {
                top: coverArea.bottom
                left: parent.left
                right: parent.right
                bottom: parent.bottom
            }
            
            Column {
                anchors.fill: parent
                anchors.margins: 8
                spacing: 2
                
                Text {
                    width: parent.width
                    text: root.gameTitle
                    color: "#f8f8f2"
                    font.pixelSize: 11
                    font.bold: (root.activeFocus || root.isFocused)
                    elide: Text.ElideRight
                    wrapMode: Text.NoWrap
                }
                
                Row {
                    width: parent.width
                    spacing: 6
                    
                    Text {
                        text: root.platform
                        color: "#6272a4"
                        font.pixelSize: 10
                    }
                    
                    Text {
                        text: "•"
                        color: "#6272a4"
                        font.pixelSize: 10
                        visible: root.releaseYear !== "" && root.platform !== ""
                    }
                    
                    Text {
                        text: root.releaseYear
                        color: "#6272a4"
                        font.pixelSize: 10
                        visible: root.releaseYear !== ""
                    }
                }
            }
        }
    }

    Keys.onReturnPressed: root.selected()
    Keys.onEnterPressed: root.selected()

    focus: true
}
