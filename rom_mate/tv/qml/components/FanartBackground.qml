import QtQuick
import QtQuick.Controls
import QtQuick.Effects

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
        layer.enabled: true
        layer.effect: MultiEffect {
            blurEnabled: true
            blur: 0.7
            blurMax: 48
        }
    }

    Image {
        id: imageB
        anchors.fill: parent
        fillMode: Image.PreserveAspectCrop
        opacity: 0.0
        asynchronous: true
        cache: true
        layer.enabled: true
        layer.effect: MultiEffect {
            blurEnabled: true
            blur: 0.7
            blurMax: 48
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "#000000"
        opacity: 0.55
    }

    // Hold timer: waits 5s then triggers the crossfade
    Timer {
        id: holdTimer
        interval: 5000
        repeat: false
        running: false
        onTriggered: {
            if (root.screenshotUrls.length === 0) return
            root._currentIndex = (root._currentIndex + 1) % root.screenshotUrls.length
            if (root._aOnTop) {
                imageB.source = root.screenshotUrls[root._currentIndex]
            } else {
                imageA.source = root.screenshotUrls[root._currentIndex]
            }
            crossfadeAnim.start()
        }
    }

    // Crossfade animation: 1s fade, then flips state and restarts hold timer
    SequentialAnimation {
        id: crossfadeAnim
        running: false

        NumberAnimation {
            target: root._aOnTop ? imageB : imageA
            property: "opacity"
            from: 0.0; to: 1.0
            duration: 1000
            easing.type: Easing.InOutQuad
        }
        NumberAnimation {
            target: root._aOnTop ? imageA : imageB
            property: "opacity"
            from: 1.0; to: 0.0
            duration: 1000
            easing.type: Easing.InOutQuad
        }
        ScriptAction {
            script: {
                root._aOnTop = !root._aOnTop
                if (root.screenshotUrls.length > 0) holdTimer.restart()
            }
        }
    }

    function _startCycle() {
        crossfadeAnim.stop()
        holdTimer.stop()
        _aOnTop = true
        _currentIndex = 0
        if (screenshotUrls.length > 0) {
            imageA.opacity = 1.0
            imageB.opacity = 0.0
            imageA.source = screenshotUrls[0]
            imageB.source = screenshotUrls.length > 1 ? screenshotUrls[1] : screenshotUrls[0]
            if (screenshotUrls.length > 1) holdTimer.restart()
        } else {
            imageA.source = ""
            imageB.source = ""
        }
    }

    onScreenshotUrlsChanged: {
        _startCycle()
    }

    Component.onDestruction: {
        holdTimer.stop()
        crossfadeAnim.stop()
    }
}