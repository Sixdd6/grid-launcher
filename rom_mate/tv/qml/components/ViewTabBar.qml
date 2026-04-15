import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: root
    height: 56

    signal tabSelected(int index)

    property int currentIndex: 0

    function selectPrev() {
        if (currentIndex > 0) {
            currentIndex -= 1
            tabSelected(currentIndex)
        }
    }

    function selectNext() {
        if (currentIndex < 2) {
            currentIndex += 1
            tabSelected(currentIndex)
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "#1e1f29"

        Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            height: 1
            color: "#44475a"
        }

        Row {
            anchors.centerIn: parent
            spacing: 48

            Repeater {
                model: [
                    { label: "Home",    icon: "⌂" },
                    { label: "Library", icon: "▤" },
                    { label: "Server",  icon: "☁" }
                ]

                delegate: Item {
                    width: 80
                    height: 48

                    Column {
                        anchors.centerIn: parent
                        spacing: 2

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: modelData.icon
                            font.pixelSize: index === root.currentIndex ? 24 : 20
                            color: index === root.currentIndex ? "#ff79c6" : "#bd93f9"

                            Behavior on font.pixelSize {
                                NumberAnimation { duration: 120 }
                            }
                            Behavior on color {
                                ColorAnimation { duration: 120 }
                            }
                        }

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: modelData.label
                            font.pixelSize: 11
                            font.bold: index === root.currentIndex
                            color: index === root.currentIndex ? "#f8f8f2" : "#6272a4"

                            Behavior on color {
                                ColorAnimation { duration: 120 }
                            }
                        }
                    }

                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: parent.width * 0.8
                        height: 2
                        color: "#ff79c6"
                        visible: index === root.currentIndex
                        radius: 1
                    }

                    MouseArea {
                        anchors.fill: parent
                        onClicked: {
                            root.currentIndex = index
                            root.tabSelected(index)
                        }
                    }
                }
            }
        }
    }
}