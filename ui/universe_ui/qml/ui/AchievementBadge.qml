import QtQuick
import "../core"

// The store's icon when it loads, a trophy (or a lock) drawn in its place otherwise.
Rectangle {
    id: badge

    property string icon: ""
    property bool unlocked: true
    property color tint: Theme.text

    radius: width * 0.18
    color: Qt.rgba(1, 1, 1, unlocked ? 0.1 : 0.05)
    clip: true

    Image {
        id: art
        anchors.fill: parent
        source: badge.icon
        asynchronous: true
        fillMode: Image.PreserveAspectCrop
        sourceSize.width: width * 2
        sourceSize.height: height * 2
        opacity: badge.unlocked ? 1.0 : 0.75
        visible: status === Image.Ready
    }

    MenuGlyph {
        anchors.centerIn: parent
        width: parent.width * 0.52
        height: width
        visible: !art.visible
        kind: badge.unlocked ? "trophy" : "lock"
        tint: badge.unlocked ? badge.tint : Qt.rgba(badge.tint.r, badge.tint.g, badge.tint.b, 0.45)
    }
}
