import QtQuick
import "../core"

Item {
    id: bar

    property Flickable flickable: null
    property bool shown: true
    property bool dimmed: false
    // The measured stretch of the content, in content coordinates: the whole of it by default.
    property real from: 0
    property real to: flickable ? flickable.contentHeight : 0
    property real window: flickable ? flickable.height : 0

    readonly property real span: to - from
    readonly property bool needed: shown && flickable !== null && flickable.visible && span > window + 1
    readonly property real thumbHeight: needed ? Math.max(Theme.dp(40), height * window / span) : 0
    readonly property real progress: needed ? Math.max(0, Math.min(1, (flickable.contentY - from) / (span - window))) : 0

    width: Theme.dp(6)
    opacity: needed ? (dimmed ? 0.45 : 1.0) : 0.0
    visible: opacity > 0.01

    Behavior on opacity {
        Ease {
            duration: Theme.durQuick
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        y: (bar.height - bar.thumbHeight) * bar.progress
        height: bar.thumbHeight
        radius: width / 2
        color: Theme.thumb
    }
}
