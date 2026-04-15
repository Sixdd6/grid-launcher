import QtQuick
import QtQuick.Controls
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

    FanartBackground {
        id: fanart
        screenshotUrls: root._fanartUrls
    }

    Connections {
        target: appBackend
        function onLibraryGamesChanged() {
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
        if (appBackend.uiOverlayActive) return true
        if (root.outerStackRef && root.outerStackRef.depth > 1) return true
        return false
    }

    ScrollView {
        anchors.fill: parent
        contentWidth: parent.width
        clip: false

        Column {
            width: root.width
            spacing: 24
            topPadding: 24
            bottomPadding: 24

            GameRow {
                id: continueRow
                homeStyle: true
                width: root.width
                rowTitle: "Continue Playing"
                games: root._buildRecentGames()
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
    }

    // Controller navigation for rows and per-row horizontal indices
    Connections {
        target: controllerBackend
        function onNavigationEvent(direction) {
            if (root._navBlocked()) return

            if (direction === "up") {
                var fromRowUp = _activeRowIndex === 0 ? continueRow
                              : _activeRowIndex === 1 ? favoritesRow
                              : _activeRowIndex === 2 ? newAdditionsRow
                              : highlyRatedRow
                var fromIdxUp = root._getActiveIndex()
                root._activeRowIndex = Math.max(0, root._activeRowIndex - 1)
                var toRowUp = _activeRowIndex === 0 ? continueRow
                            : _activeRowIndex === 1 ? favoritesRow
                            : _activeRowIndex === 2 ? newAdditionsRow
                            : highlyRatedRow
                root._setActiveIndex(root._visuallyClosestIndex(fromRowUp, fromIdxUp, toRowUp))
            } else if (direction === "down") {
                var fromRowDown = _activeRowIndex === 0 ? continueRow
                                : _activeRowIndex === 1 ? favoritesRow
                                : _activeRowIndex === 2 ? newAdditionsRow
                                : highlyRatedRow
                var fromIdxDown = root._getActiveIndex()
                root._activeRowIndex = Math.min(3, root._activeRowIndex + 1)
                var toRowDown = _activeRowIndex === 0 ? continueRow
                              : _activeRowIndex === 1 ? favoritesRow
                              : _activeRowIndex === 2 ? newAdditionsRow
                              : highlyRatedRow
                root._setActiveIndex(root._visuallyClosestIndex(fromRowDown, fromIdxDown, toRowDown))
            } else if (direction === "left") {
                var activeRowLeft = root._activeRowIndex === 0 ? continueRow
                                  : root._activeRowIndex === 1 ? favoritesRow
                                  : root._activeRowIndex === 2 ? newAdditionsRow
                                  : highlyRatedRow
                var maxIdxLeft = Math.max(0, activeRowLeft.games.length - 1)
                root._setActiveIndex(Math.min(maxIdxLeft, Math.max(0, root._getActiveIndex() - 1)))
            } else if (direction === "right") {
                var activeRow = root._activeRowIndex === 0 ? continueRow
                              : root._activeRowIndex === 1 ? favoritesRow
                              : root._activeRowIndex === 2 ? newAdditionsRow
                              : highlyRatedRow
                var maxIdx = Math.max(0, activeRow.games.length - 1)
                root._setActiveIndex(Math.min(maxIdx, root._getActiveIndex() + 1))
            } else if (direction === "confirm") {
                var row = root._activeRowIndex === 0 ? continueRow
                        : root._activeRowIndex === 1 ? favoritesRow
                        : root._activeRowIndex === 2 ? newAdditionsRow
                        : highlyRatedRow
                var game = row.games[root._getActiveIndex()]
                if (game && root.outerStackRef) {
                    root.outerStackRef.push(detailsViewComponent, { game: game })
                }
            }
        }
    }
}