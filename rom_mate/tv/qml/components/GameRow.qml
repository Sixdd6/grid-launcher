import QtQuick
import QtQuick.Controls

Item {
    id: root
    height: root._cardHeight + (root.homeStyle ? 68 : 48)
    width: parent ? parent.width : 800

    property string rowTitle: ""
    property var games: []
    property bool navigationActive: false
    property int sharedIndex: -1

    property bool homeStyle: false
    readonly property int _cardWidth:  homeStyle ? 380 : 180
    readonly property int _cardHeight: homeStyle ? 130 : 260
    readonly property real currentContentX: listView.contentX

    onNavigationActiveChanged: {
        if (navigationActive && listView.currentIndex >= 0 && listView.currentIndex < games.length) {
            activeFocusGameChanged(games[listView.currentIndex])
        }
    }

    signal gameSelected(var game)

    Column {
        anchors.fill: parent
        spacing: 8

        Text {
            text: root.rowTitle
            color: "#bd93f9"
            font.pixelSize: 14
            font.bold: true
            leftPadding: 16
        }

        ListView {
            id: listView
            width: parent.width - (root.homeStyle ? 48 : 0)
            height: root._cardHeight + (root.homeStyle ? 38 : 18)
            orientation: ListView.Horizontal
            spacing: root.homeStyle ? 20 : 12
            clip: !root.homeStyle
            cacheBuffer: 0
            leftMargin: root.homeStyle ? 28 : 16
            rightMargin: root.homeStyle ? 48 : 16
            keyNavigationEnabled: true
            focus: root.activeFocus
            currentIndex: root.sharedIndex >= 0 ? root.sharedIndex : 0
            highlightMoveDuration: 0
            highlightFollowsCurrentItem: false

            onCurrentIndexChanged: {
                if (currentIndex <= 0) {
                    contentX = -leftMargin
                } else {
                    positionViewAtIndex(currentIndex, ListView.Contain)
                }
                if (root.navigationActive && currentIndex >= 0 && currentIndex < root.games.length) {
                    root.activeFocusGameChanged(root.games[currentIndex])
                }
            }

            model: root.games

            delegate: Item {
                id: delegateItem
                required property var modelData
                required property int index
                width:  root._cardWidth
                height: root._cardHeight

                Component {
                    id: gameCardComp
                    GameCard {
                        coverUrl:  delegateItem.modelData.cover_url || ""
                        gameTitle: delegateItem.modelData.title     || ""
                        isFocused: listView.currentIndex === delegateItem.index && root.navigationActive
                        onSelected: root.gameSelected(delegateItem.modelData)
                        onActiveFocusChanged: {
                            if (activeFocus) {
                                listView.positionViewAtIndex(delegateItem.index, ListView.Contain)
                                root.activeFocusGameChanged(delegateItem.modelData)
                            }
                        }
                    }
                }

                Component {
                    id: homeCardComp
                    HomeGameCard {
                        coverUrl:    delegateItem.modelData.cover_url    || ""
                        gameTitle:   delegateItem.modelData.title        || ""
                        platform:    delegateItem.modelData.platform     || ""
                        releaseYear: delegateItem.modelData.release_year || ""
                        genres:      delegateItem.modelData.genres       || ""
                        isFocused:   listView.currentIndex === delegateItem.index && root.navigationActive
                        onSelected: root.gameSelected(delegateItem.modelData)
                        onActiveFocusChanged: {
                            if (activeFocus) {
                                listView.positionViewAtIndex(delegateItem.index, ListView.Contain)
                                root.activeFocusGameChanged(delegateItem.modelData)
                            }
                        }
                    }
                }

                Loader {
                    anchors.fill: parent
                    sourceComponent: root.homeStyle ? homeCardComp : gameCardComp
                }
            }
        }
    }

    signal activeFocusGameChanged(var game)
}