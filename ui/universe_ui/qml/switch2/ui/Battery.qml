import QtQuick
import "../core"

Item {
    id: cell

    property int percent: 100
    property bool charging: false
    readonly property bool low: percent <= 15 && !charging
    readonly property color ink: low ? Theme.danger : Theme.text

    width: Theme.dp(46)
    height: Theme.dp(24)

    Rectangle {
        id: shell

        anchors.fill: parent
        anchors.rightMargin: Theme.dp(5)
        radius: Theme.dp(5)
        color: "transparent"
        border.width: Theme.dp(2.5)
        border.color: cell.ink
    }

    Rectangle {
        anchors.left: shell.right
        anchors.leftMargin: Theme.dp(1)
        anchors.verticalCenter: shell.verticalCenter
        width: Theme.dp(3.5)
        height: shell.height * 0.4
        radius: width / 2
        color: cell.ink
    }

    Rectangle {
        anchors.left: shell.left
        anchors.top: shell.top
        anchors.bottom: shell.bottom
        anchors.margins: Theme.dp(4.5)
        width: Math.max(Theme.dp(3), (shell.width - Theme.dp(9)) * Math.min(100, cell.percent) / 100)
        radius: Theme.dp(2)
        color: cell.charging ? Theme.okGreen : cell.ink
    }
}
