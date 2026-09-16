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

    anchors.horizontalCenter: parent.horizontalCenter
    y: parent.height - Theme.dp(Theme.hintBarHeight) - height - Theme.dp(24)
    width: label.width + Theme.dp(56)
    height: Theme.dp(60)
    opacity: shown ? 1.0 : 0.0
    visible: opacity > 0.01

    Behavior on opacity { Ease {} }

    Timer {
        id: hideTimer
        interval: 4000
        onTriggered: toast.shown = false
    }

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: "#1b1d24"
        border.width: 1
        border.color: Theme.surfaceBorder
    }

    Text {
        id: label
        anchors.centerIn: parent
        text: toast.text
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Medium
        font.pixelSize: Theme.dp(22)
        elide: Text.ElideRight
        width: Math.min(implicitWidth, toast.parent ? toast.parent.width - Theme.dp(200) : implicitWidth)
    }
}
