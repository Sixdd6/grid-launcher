import QtQuick
import QtQuick.Layouts
import "../components"

Item {
    id: root

    property var stackView: null
    property var outerStackRef: null
    property int _activeRowIndex: 0
    property int _rowIndex0: 0
    property int _rowIndex1: 0
    property int _rowIndex2: 0
    property int _rowIndex3: 0

    // Screenshot URLs for fanart (updated as focus moves)
    property var _fanartUrls: []
    property var _pendingFanartUrls: []

    Timer {
        id: fanartDebounce
        interval: 300
        repeat: false
        onTriggered: root._fanartUrls = root._pendingFanartUrls
    }

    onVisibleChanged: {
        if (visible) {
            continueRow.games = root._buildRecentGames()
        } else {
            fanartDebounce.stop()
        }
    }

    Component.onCompleted: {
        continueRow.games = root._buildRecentGames()
    }

    Component.onDestruction: {
        fanartDebounce.stop()
    }

    FanartBackground {
        id: fanart
        screenshotUrls: root._fanartUrls
    }

    Connections {
        target: appBackend
        function onLibraryGamesChanged() {
            if (root.visible) {
                continueRow.games = root._buildRecentGames()
            }
            if (root._fanartUrls.length === 0) {
                var recent = root._buildRecentGames()
                if (recent.length > 0) {
                    root._pendingFanartUrls = root._screenshotUrlsFromGame(recent[0])
                    fanartDebounce.restart()
                }
            }
        }
    }

    function _screenshotUrlsFromGame(game) {
        if (!game || !game.screenshot_urls) return []
        return game.screenshot_urls.split("\n").filter(function(u) { return u.trim().length > 0 })
    }

    function _buildRecentGames() {
        var games = appBackend.libraryGames
        return games.slice().reverse().slice(0, 20)
    }

    function _getActiveIndex() {
        if (_activeRowIndex === 0) return _rowIndex0
        if (_activeRowIndex === 1) return _rowIndex1
        if (_activeRowIndex === 2) return _rowIndex2
        return _rowIndex3
    }

    function _setActiveIndex(val) {
        if (_activeRowIndex === 0) _rowIndex0 = val
        else if (_activeRowIndex === 1) _rowIndex1 = val
        else if (_activeRowIndex === 2) _rowIndex2 = val
        else _rowIndex3 = val
    }

    function _visuallyClosestIndex(fromRow, fromIndex, toRow) {
        var step = fromRow._cardWidth + (fromRow.homeStyle ? 20 : 12)
        var contentXDiff = toRow.currentContentX - fromRow.currentContentX
        var targetFloat = fromIndex + contentXDiff / step
        var best = Math.round(targetFloat)
        return Math.max(0, Math.min(best, Math.max(0, toRow.games.length - 1)))
    }

    function _navBlocked() {
        if (pauseBackend.visible) return true
        if (appBackend.uiOverlayActive) return true
        if (root.outerStackRef && root.outerStackRef.depth > 1) return true
        return false
    }

    Column {
        anchors.fill: parent
        spacing: 24
        topPadding: 24
        bottomPadding: 24

        GameRow {
            id: continueRow
            homeStyle: true
            width: root.width
            rowTitle: "Continue Playing"
            games: []
            focus: true
            navigationActive: root._activeRowIndex === 0 && !root._navBlocked()
            sharedIndex: root._rowIndex0

            onGameSelected: function(game) {
                if (root.outerStackRef) {
                    root.outerStackRef.push(detailsViewComponent, { game: game })
                }
            }
            onActiveFocusGameChanged: function(game) {
                root._pendingFanartUrls = root._screenshotUrlsFromGame(game)
                fanartDebounce.restart()
            }
        }

        GameRow {
            homeStyle: true
            id: favoritesRow
            width: root.width
            rowTitle: "Favorites"
            games: appBackend.favoritesGames
            navigationActive: root._activeRowIndex === 1 && !root._navBlocked()
            sharedIndex: root._rowIndex1

            KeyNavigation.up: continueRow
            KeyNavigation.down: newAdditionsRow
            onGameSelected: function(game) {
                if (root.outerStackRef) {
                    root.outerStackRef.push(detailsViewComponent, { game: game })
                }
            }
            onActiveFocusGameChanged: function(game) {
                root._pendingFanartUrls = root._screenshotUrlsFromGame(game)
                fanartDebounce.restart()
            }
        }

        GameRow {
            homeStyle: true
            id: newAdditionsRow
            width: root.width
            rowTitle: "New Additions"
            games: appBackend.newAdditionsGames
            navigationActive: root._activeRowIndex === 2 && !root._navBlocked()
            sharedIndex: root._rowIndex2

            KeyNavigation.up: favoritesRow
            KeyNavigation.down: highlyRatedRow

            onGameSelected: function(game) {
                if (root.outerStackRef) {
                    root.outerStackRef.push(detailsViewComponent, { game: game })
                }
            }
            onActiveFocusGameChanged: function(game) {
                root._pendingFanartUrls = root._screenshotUrlsFromGame(game)
                fanartDebounce.restart()
            }
        }

        GameRow {
            homeStyle: true
            id: highlyRatedRow
            width: root.width
            rowTitle: "Highly Rated"
            games: appBackend.highlyRatedGames
            navigationActive: root._activeRowIndex === 3 && !root._navBlocked()
            sharedIndex: root._rowIndex3

            KeyNavigation.up: newAdditionsRow
            onGameSelected: function(game) {
                if (root.outerStackRef) {
                    root.outerStackRef.push(detailsViewComponent, { game: game })
                }
            }
            onActiveFocusGameChanged: function(game) {
                root._pendingFanartUrls = root._screenshotUrlsFromGame(game)
                fanartDebounce.restart()
            }
        }
    }

    // Controller navigation for rows and per-row horizontal indices
    Connections {
        target: controllerBackend
        function onNavigationEvent(direction) {
            if (root._navBlocked()) return

            if (direction === "up") {
                var newRowUp = Math.max(0, root._activeRowIndex - 1)
                if (newRowUp !== root._activeRowIndex) {
                    var rowsUp = [continueRow, favoritesRow, newAdditionsRow, highlyRatedRow]
                    var closestUp = root._visuallyClosestIndex(rowsUp[root._activeRowIndex], root._getActiveIndex(), rowsUp[newRowUp])
                    root._activeRowIndex = newRowUp
                    root._setActiveIndex(closestUp)
                }
            } else if (direction === "down") {
                var newRowDown = Math.min(3, root._activeRowIndex + 1)
                if (newRowDown !== root._activeRowIndex) {
                    var rowsDown = [continueRow, favoritesRow, newAdditionsRow, highlyRatedRow]
                    var closestDown = root._visuallyClosestIndex(rowsDown[root._activeRowIndex], root._getActiveIndex(), rowsDown[newRowDown])
                    root._activeRowIndex = newRowDown
                    root._setActiveIndex(closestDown)
                }
            } else if (direction === "left") {
                var activeRow = [continueRow, favoritesRow, newAdditionsRow, highlyRatedRow][root._activeRowIndex]
                var maxIdxLeft = Math.max(0, activeRow.games.length - 1)
                root._setActiveIndex(Math.min(maxIdxLeft, Math.max(0, root._getActiveIndex() - 1)))
            } else if (direction === "right") {
                var activeRowR = [continueRow, favoritesRow, newAdditionsRow, highlyRatedRow][root._activeRowIndex]
                var maxIdx = Math.max(0, activeRowR.games.length - 1)
                root._setActiveIndex(Math.min(maxIdx, root._getActiveIndex() + 1))
            } else if (direction === "confirm") {
                var rows = [continueRow, favoritesRow, newAdditionsRow, highlyRatedRow]
                var game = rows[root._activeRowIndex].games[root._getActiveIndex()]
                if (game && root.outerStackRef) {
                    root.outerStackRef.push(detailsViewComponent, { game: game })
                }
            }
        }
    }
}