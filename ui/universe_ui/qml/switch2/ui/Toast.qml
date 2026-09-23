import QtQuick
import "../../core" as Base
import "../core"

Item {
    id: toast

    readonly property bool shown: Base.Notices.current !== null

    x: Theme.dp(72)
    y: shown ? Theme.dp(40) : -height
    width: Math.min(label.implicitWidth + Theme.dp(64), parent ? parent.width - Theme.dp(144) : 0)
    height: Theme.dp(84)
    visible: y > -height + 1

    Behavior on y {
        Ease {}
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(8)
        color: Theme.card
        border.width: 1
        border.color: Base.Notices.error ? Theme.danger : Theme.hairline
    }

    Rectangle {
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.margins: 1
        width: Theme.dp(6)
        radius: Theme.dp(2)
        color: Theme.danger
        visible: Base.Notices.error
    }

    Label {
        id: label
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Theme.dp(32)
        anchors.verticalCenter: parent.verticalCenter
        text: Base.Notices.text
        font.pixelSize: Theme.dp(Theme.fontSmall)
        elide: Text.ElideRight
    }
}
