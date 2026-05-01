import QtQuick

Item {
    id: root
    anchors.fill: parent

    property var screenshotUrls: []
    property int _currentIndex: 0
    property bool _aOnTop: true

    Image {
        id: imageA
        anchors.fill: parent
        fillMode: Image.PreserveAspectCrop
        opacity: 1.0
        asynchronous: true
        cache: true
    }

    Image {
        id: imageB
        anchors.fill: parent
        fillMode: Image.PreserveAspectCrop
        opacity: 0.0
        asynchronous: true
        cache: true
    }

    Rectangle {
        anchors.fill: parent
        color: "#000000"
        opacity: 0.70
    }

    // Hold timer: waits 5s then instantly swaps to the next image.
    // Image sources use image://covers/<url> so loading goes through the Python
    // CoverImageProvider (urllib thread pool) instead of QNetworkAccessManager.
    // Direct HTTP URLs in QML Image elements use QNetworkAccessManager whose Win32
    // event dispatcher threads accumulate USER timer objects under load.
    Timer {
        id: holdTimer
        interval: 5000
        repeat: false
        running: false
        onTriggered: {
            if (root.screenshotUrls.length === 0) return
            root._currentIndex = (root._currentIndex + 1) % root.screenshotUrls.length
            if (root._aOnTop) {
                imageB.source = "image://covers/" + root.screenshotUrls[root._currentIndex]
                imageB.opacity = 1.0
                imageA.opacity = 0.0
                root._aOnTop = false
            } else {
                imageA.source = "image://covers/" + root.screenshotUrls[root._currentIndex]
                imageA.opacity = 1.0
                imageB.opacity = 0.0
                root._aOnTop = true
            }
            if (root.screenshotUrls.length > 1) holdTimer.restart()
        }
    }

    function _startCycle() {
        holdTimer.stop()
        imageA.opacity = 1.0
        imageB.opacity = 0.0
        _aOnTop = true
        _currentIndex = 0
        if (screenshotUrls.length > 0) {
            imageA.source = "image://covers/" + screenshotUrls[0]
            imageB.source = "image://covers/" + (screenshotUrls.length > 1 ? screenshotUrls[1] : screenshotUrls[0])
            if (screenshotUrls.length > 1) holdTimer.restart()
        } else {
            imageA.source = ""
            imageB.source = ""
        }
    }

    onScreenshotUrlsChanged: {
        _startCycle()
    }

    onVisibleChanged: {
        if (!visible) {
            holdTimer.stop()
        } else if (screenshotUrls.length > 0) {
            _startCycle()
        }
    }

    Component.onDestruction: {
        holdTimer.stop()
        imageA.source = ""
        imageB.source = ""
    }
}