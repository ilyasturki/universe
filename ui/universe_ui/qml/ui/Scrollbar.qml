import QtQuick
import "../core"

Item {
    id: bar

    property Flickable flickable: null
    property bool dimmed: false

    readonly property real span: flickable ? flickable.contentHeight : 0
    readonly property real window: flickable ? flickable.height : 0
    readonly property bool needed: flickable !== null && flickable.visible && span > window + 1
    readonly property real thumbHeight: needed ? Math.max(Theme.dp(40), height * window / span) : 0

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
        y: bar.needed ? (bar.height - bar.thumbHeight) * Math.max(0, Math.min(1, bar.flickable.contentY / (bar.span - bar.window))) : 0
        height: bar.thumbHeight
        radius: width / 2
        color: Theme.thumb
    }
}
