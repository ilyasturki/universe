import QtQuick
import "../core"

Item {
    id: toast

    property string text: ""
    property bool shown: false

    function show(message) {
        text = message;
        shown = true;
        hideTimer.restart();
    }

    x: Theme.dp(72)
    y: shown ? Theme.dp(40) : -height
    width: Math.min(label.implicitWidth + Theme.dp(64), parent ? parent.width - Theme.dp(144) : 0)
    height: Theme.dp(84)
    visible: y > -height + 1

    Behavior on y {
        NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic }
    }

    Timer {
        id: hideTimer
        interval: 4000
        onTriggered: toast.shown = false
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(8)
        color: Theme.card
        border.width: 1
        border.color: Theme.hairline
    }

    Text {
        id: label
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Theme.dp(32)
        anchors.verticalCenter: parent.verticalCenter
        text: toast.text
        color: Theme.text
        font.family: Theme.sans
        font.pixelSize: Theme.dp(Theme.fontSmall)
        elide: Text.ElideRight
    }
}
